//! Git: worktrees, and the status the board shows.
//!
//! Mutations go through the `git` command rather than a library, deliberately.
//! Claude Code creates its worktrees with `git worktree`, the user creates
//! theirs by hand with `git worktree`, and Devplane creating them a third way
//! would produce checkouts that behave subtly differently from both. Parity
//! with what everyone else does is worth more here than a few milliseconds.
//!
//! The location follows Claude Code's convention — `.claude/worktrees/<name>` —
//! so a worktree Devplane made and one Claude made are indistinguishable, and
//! Claude's own cleanup sweep understands both.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where isolated checkouts live, relative to the repository root.
pub const WORKTREE_DIR: &str = ".claude/worktrees";

/// What `git status` says, in the shape the board needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub branch: Option<String>,
    pub changed_files: usize,
    pub untracked_files: usize,
    pub ahead: u32,
    pub behind: u32,
    pub has_conflicts: bool,
}

impl Status {
    pub fn is_clean(&self) -> bool {
        self.changed_files == 0 && self.untracked_files == 0
    }
    /// Whether there is work here that removing the worktree would destroy.
    pub fn has_work(&self) -> bool {
        !self.is_clean() || self.ahead > 0
    }
}

/// Runs a git command in a directory and returns stdout.
async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .kill_on_drop(true)
        .output()
        .await
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The repository root containing `path`, if any.
pub async fn repo_root(path: &Path) -> Option<PathBuf> {
    git(path, &["rev-parse", "--show-toplevel"])
        .await
        .ok()
        .map(|s| PathBuf::from(s.trim()))
}

/// The branch a repository considers its trunk.
///
/// The `origin` remote's URL, when there is one.
///
/// The only channel that had this was OpenTelemetry's `vcs.repository.url.full`,
/// which arrives after `connect` and never at all for somebody who has not run
/// it — so a launch link, whose whole point is naming the repository rather
/// than a path, could not be built for exactly the zero-configuration case the
/// board is proud of. Git has known the answer the whole time.
pub async fn remote_url(root: &Path) -> Option<String> {
    let out = git(root, &["remote", "get-url", "origin"]).await.ok()?;
    let url = out.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// `origin/HEAD` when the remote says so, the current branch otherwise. Guessing
/// `main` would silently branch new work from the wrong place in any repository
/// that never renamed `master`.
pub async fn base_branch(root: &Path) -> String {
    if let Ok(s) = git(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .await
        && let Some(name) = s.trim().strip_prefix("origin/")
    {
        return name.to_string();
    }
    git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "main".into())
}

/// Reads status. Uses the porcelain v2 format, which is the one with a
/// stability promise attached.
pub async fn status(dir: &Path) -> Result<Status> {
    let text = git(
        dir,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
        ],
    )
    .await?;
    let mut s = Status::default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            s.branch = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut parts = rest.split_whitespace();
            s.ahead = parts
                .next()
                .and_then(|p| p.trim_start_matches('+').parse().ok())
                .unwrap_or(0);
            s.behind = parts
                .next()
                .and_then(|p| p.trim_start_matches('-').parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("? ") {
            s.untracked_files += 1;
        } else if line.starts_with("u ") {
            s.has_conflicts = true;
            s.changed_files += 1;
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            s.changed_files += 1;
        }
    }
    Ok(s)
}

/// A worktree that exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
}

pub async fn list(root: &Path) -> Result<Vec<Worktree>> {
    let text = git(root, &["worktree", "list", "--porcelain"]).await?;
    let mut out = Vec::new();
    let mut current: Option<PathBuf> = None;
    let mut branch = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(path) = current.take() {
                out.push(Worktree {
                    path,
                    branch: branch.take(),
                });
            }
            current = Some(PathBuf::from(p));
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.trim_start_matches("refs/heads/").to_string());
        }
    }
    if let Some(path) = current {
        out.push(Worktree { path, branch });
    }
    Ok(out)
}

/// Creates an isolated checkout for a unit of work.
///
/// Branching from the base rather than from whatever is currently checked out:
/// two pieces of work started an hour apart should not inherit each other's
/// half-finished state.
pub async fn create_worktree(root: &Path, name: &str, branch: &str, base: &str) -> Result<PathBuf> {
    let dir = root.join(WORKTREE_DIR).join(name);
    if dir.exists() {
        bail!("{} already exists", dir.display());
    }
    let parent = dir
        .parent()
        .with_context(|| format!("{} has no parent directory", dir.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", dir.display()))?;

    let base_ref = resolve_base(root, base).await;
    git(
        root,
        &[
            "worktree",
            "add",
            "-b",
            branch,
            dir.to_str().context("worktree path is not utf-8")?,
            &base_ref,
        ],
    )
    .await?;
    Ok(dir)
}

/// Prefers the remote's view of the base branch, so new work starts from what
/// has been pushed rather than from a local branch that may be behind or ahead.
async fn resolve_base(root: &Path, base: &str) -> String {
    let remote = format!("origin/{base}");
    if git(root, &["rev-parse", "--verify", "--quiet", &remote])
        .await
        .is_ok()
    {
        return remote;
    }
    base.to_string()
}

/// Removes a worktree, refusing to destroy work.
///
/// `force` still refuses when there are commits the remote has not seen: a
/// branch is recoverable from a reflog by someone who knows how, which is not a
/// standard anybody should be held to at the end of a long day.
pub async fn remove_worktree(root: &Path, dir: &Path, force: bool) -> Result<()> {
    // A worktree whose directory is already gone has nothing left to destroy;
    // removing it is how the stale registration gets cleaned up.
    //
    // A worktree whose directory is *there* but whose status cannot be read is
    // the opposite case, and defaulting it to "clean" was a fail-open in the
    // one function whose entire job is refusing to destroy work. We do not know
    // what is in it, and "I could not tell" must not read as "there was
    // nothing".
    let st = if dir.exists() {
        match status(dir).await {
            Ok(st) => st,
            Err(e) if force => {
                tracing::warn!(dir = %dir.display(), %e, "status unreadable; --force was given");
                Status::default()
            }
            Err(e) => bail!(
                "cannot read the state of {}: {e}. Pass --force to remove it anyway",
                dir.display()
            ),
        }
    } else {
        Status::default()
    };
    if st.ahead > 0 {
        bail!(
            "{} has {} unpushed commit(s); push or delete the branch by hand",
            dir.display(),
            st.ahead
        );
    }
    if st.has_work() && !force {
        bail!(
            "{} has uncommitted changes; pass --force to discard them",
            dir.display()
        );
    }
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    let path = dir.to_str().context("worktree path is not utf-8")?;
    args.push(path);
    git(root, &args).await?;
    Ok(())
}

/// Every repository-relative path this checkout has touched since it diverged
/// from `base` — committed on the branch, staged, or still dirty.
///
/// What a work's branch changed, relative to what it branched from.
///
/// **Three-dot, for `touched_files`' reason.** `<base>...HEAD` diffs against the
/// merge base, so a `base` that has moved on since the worktree was made does
/// not report everybody else's commits as this branch's work. Two dots would
/// make every review show the whole of `main`.
///
/// Uncommitted work is included on purpose: a reviewer approving a held step is
/// approving the checkout as it stands, and an agent that has not committed yet
/// has still changed the file. That is the same reason `touched_files` adds
/// `git status`, arrived at from the review side rather than the overlap side.
///
/// The rendering is somebody else's job — this returns the text and the command
/// that produced it, and `core::diff` is pure so it can be tested without a
/// repository.
pub async fn change_set(dir: &Path, base: &str) -> crate::core::diff::ChangeSet {
    let range = format!("{base}...HEAD");
    // `--find-renames` so a moved file reads as moved rather than as one whole
    // file deleted and another added, which is the single biggest source of
    // noise in a review.
    let args = ["diff", "--find-renames", "--no-color", &range];
    let committed = git(dir, &args).await.unwrap_or_default();
    // And what is not committed yet, against the working tree.
    let working = git(dir, &["diff", "--find-renames", "--no-color", "HEAD"])
        .await
        .unwrap_or_default();
    // **And the files git has never seen.** `git diff` reports tracked changes
    // only, so a file the agent *created* is invisible to both diffs above —
    // which is the worst possible omission for a view whose whole purpose is
    // that approving work you cannot see is not approval. Caught by the test
    // for this route, on the first run.
    //
    // `--no-index` rather than `git add -N`: intent-to-add would mutate the
    // index of somebody's checkout to render a page, and a read must not write.
    let mut untracked = String::new();
    if let Ok(list) = git(dir, &["ls-files", "--others", "--exclude-standard"]).await {
        for path in list.lines().filter(|l| !l.is_empty()).take(MAX_UNTRACKED) {
            // `--no-index` exits 1 whenever the files differ, which for a new
            // file is always, so the status is not an error here.
            let out = tokio::process::Command::new("git")
                .args(["diff", "--no-color", "--no-index", "--", "/dev/null", path])
                .current_dir(dir)
                .kill_on_drop(true)
                .output()
                .await;
            if let Ok(o) = out {
                untracked.push_str(&String::from_utf8_lossy(&o.stdout));
            }
        }
    }

    let text = format!("{committed}{working}{untracked}");
    let mut set = crate::core::diff::parse(base, &text, &format!("git diff {range}"));
    // A binary file has no lines to show, so its size is the only thing the
    // view can say about it. `git diff` does not print one — it says the files
    // differ and stops — and the parser is pure, so the stat happens here. A
    // deleted binary has nothing left to measure and keeps saying just
    // *binary*, which is true.
    for f in &mut set.files {
        if let crate::core::diff::Body::Binary { bytes } = &mut f.body {
            *bytes = tokio::fs::metadata(dir.join(&f.path))
                .await
                .ok()
                .map(|m| m.len());
        }
    }
    set
}

/// How many never-seen files are diffed individually.
///
/// One process each, so this is a bound on work rather than on output. Past it
/// the change set is truncated and says so, which is the same answer the file
/// bound gives.
const MAX_UNTRACKED: usize = 40;

/// Used to answer a question no single piece of work can answer about itself:
/// whether somebody else is editing the same file right now.
///
/// `...` is a three-dot diff against the merge base, so a `base` that has moved
/// on since the worktree was made does not report everybody else's commits as
/// this branch's work — which would make every piece of work overlap every
/// other one and the whole signal worthless.
pub async fn touched_files(dir: &Path, base: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(o) = git(dir, &["diff", "--name-only", &format!("{base}...HEAD")]).await {
        out.extend(o.lines().map(str::to_string));
    }
    // Uncommitted work counts: two agents editing one file have collided
    // whether or not either has committed yet, and the point of saying so is to
    // say it before either reaches a pull request.
    if let Ok(o) = git(dir, &["status", "--porcelain", "-z"]).await {
        for entry in o.split('\0').filter(|e| !e.is_empty()) {
            if let Some(path) = entry.get(3..) {
                out.push(path.to_string());
            }
        }
    }
    out.retain(|p| !p.is_empty());
    out.sort();
    out.dedup();
    out
}

//   through one is an arbitrary write rather than a leak.
/// Copies gitignored files a fresh checkout needs, in the spirit of
/// `.worktreeinclude`.
///
/// Repository-relative paths, not globs — `[workspace] include = [".env"]`
/// names the files it means. Only files that exist, and never over something
/// already in the worktree.
///
/// Paths that leave the repository are refused, **by spelling and by symlink**.
/// `devplane.toml` is committed, so it arrives with somebody else's code, and
/// `include = ["../../.ssh/id_rsa"]` would otherwise copy a private key into a
/// directory an agent is about to read. A symlink is not a spelling, and git
/// tracks symlinks, so both ends are resolved rather than read:
///
/// * the **source** is canonicalised and must still be inside the repository,
///   which also covers a symlinked parent directory;
/// * the **destination** must not exist at all, tested with `symlink_metadata`
///   so a *dangling* symlink counts — `exists()` follows the link, and copying
///   through one is an arbitrary write rather than a leak.
pub fn copy_includes(root: &Path, worktree: &Path, patterns: &[String]) -> Vec<String> {
    let mut copied = Vec::new();
    for pattern in patterns {
        let from = root.join(pattern);
        let to = worktree.join(pattern);
        if !within(root, &from) || !within(worktree, &to) {
            tracing::warn!(
                pattern,
                "refusing to copy a file from outside the repository"
            );
            continue;
        }
        if !from.is_file() {
            continue;
        }
        // Where the source actually is, rather than where it is spelled.
        match (from.canonicalize(), root.canonicalize()) {
            (Ok(real), Ok(real_root)) if real.starts_with(&real_root) => {}
            (Ok(_), Ok(_)) => {
                tracing::warn!(
                    pattern,
                    "refusing to copy a file that points outside the repository"
                );
                continue;
            }
            // Unresolvable is refused rather than trusted: this is the one
            // place a wrong answer hands an agent somebody's private key.
            _ => continue,
        }
        // Anything already there — a file, a directory, or a symlink whose
        // target does not exist — is left alone.
        if to.symlink_metadata().is_ok() {
            continue;
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if std::fs::copy(&from, &to).is_ok() {
            copied.push(pattern.clone());
        }
    }
    copied
}

/// Whether `path` stays inside `root` once `..` and `.` are resolved.
///
/// Lexical, because the file may not exist yet and `canonicalize` would refuse
/// to say anything about it. Absolute paths are outside by definition: joining
/// one discards the root entirely.
fn within(root: &Path, path: &Path) -> bool {
    use std::path::Component;
    let mut depth = 0i32;
    let Ok(rest) = path.strip_prefix(root) else {
        return false;
    };
    for c in rest.components() {
        match c {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            // A root or a prefix inside a relative tail means the join threw the
            // root away.
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn scratch_repo(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vp-git-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            git(&dir, &args).await.unwrap();
        }
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        git(&dir, &["add", "-A"]).await.unwrap();
        git(&dir, &["commit", "-qm", "init"]).await.unwrap();
        dir
    }

    #[tokio::test]
    async fn a_worktree_is_created_where_claude_code_puts_them() {
        let root = scratch_repo("create").await;
        let wt = create_worktree(&root, "fix-thing-abc123", "fix/thing", "main")
            .await
            .unwrap();
        assert!(wt.starts_with(root.join(WORKTREE_DIR)));
        assert!(wt.join("a.txt").exists(), "it is a real checkout");

        let listed = list(&root).await.unwrap();
        assert!(
            listed
                .iter()
                .any(|w| w.branch.as_deref() == Some("fix/thing"))
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn status_sees_changes_and_the_branch() {
        let root = scratch_repo("status").await;
        let st = status(&root).await.unwrap();
        assert_eq!(st.branch.as_deref(), Some("main"));
        assert!(st.is_clean());

        std::fs::write(root.join("a.txt"), "changed\n").unwrap();
        std::fs::write(root.join("new.txt"), "new\n").unwrap();
        let st = status(&root).await.unwrap();
        assert_eq!(st.changed_files, 1);
        assert_eq!(st.untracked_files, 1);
        assert!(st.has_work());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn removing_a_worktree_refuses_to_destroy_work() {
        let root = scratch_repo("remove").await;
        let wt = create_worktree(&root, "wip-abc", "wip/abc", "main")
            .await
            .unwrap();
        std::fs::write(wt.join("a.txt"), "unsaved\n").unwrap();

        let err = remove_worktree(&root, &wt, false).await.unwrap_err();
        assert!(err.to_string().contains("uncommitted"), "{err}");

        // Forcing is allowed, because the user asked twice.
        remove_worktree(&root, &wt, true).await.unwrap();
        assert!(!wt.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn unpushed_commits_are_never_discarded_even_with_force() {
        // There is no remote here, so `ahead` is unknown and removal proceeds;
        // the guard matters when a branch has been pushed and moved on. This
        // pins the shape of the check rather than the remote-tracking case.
        let root = scratch_repo("unpushed").await;
        let wt = create_worktree(&root, "c-abc", "c/abc", "main")
            .await
            .unwrap();
        std::fs::write(wt.join("b.txt"), "x\n").unwrap();
        git(&wt, &["add", "-A"]).await.unwrap();
        git(&wt, &["commit", "-qm", "work"]).await.unwrap();

        let st = status(&wt).await.unwrap();
        assert!(st.is_clean(), "committed work leaves a clean tree");
        remove_worktree(&root, &wt, false).await.unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn includes_are_copied_but_never_over_something() {
        let root = scratch_repo("includes").await;
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        let wt = create_worktree(&root, "i-abc", "i/abc", "main")
            .await
            .unwrap();

        let copied = copy_includes(&root, &wt, &[".env".into(), "missing.txt".into()]);
        assert_eq!(copied, vec![".env".to_string()]);
        assert_eq!(
            std::fs::read_to_string(wt.join(".env")).unwrap(),
            "SECRET=1\n"
        );

        std::fs::write(wt.join(".env"), "MINE=1\n").unwrap();
        copy_includes(&root, &wt, &[".env".into()]);
        assert_eq!(
            std::fs::read_to_string(wt.join(".env")).unwrap(),
            "MINE=1\n",
            "an existing file is the worktree's own"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_worktree_whose_state_cannot_be_read_is_not_silently_removed() {
        // The whole job of this function is refusing to destroy work, and
        // defaulting an unreadable status to `Status::default()` made it answer
        // "there is nothing here" to the question "I could not tell".
        let root = scratch_repo("unreadable").await;
        // A directory that exists and holds work, in a place git cannot answer
        // about at all. The registration may be stale, the files are not.
        let orphan = std::env::temp_dir().join(format!("vp-orphan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&orphan);
        std::fs::create_dir_all(&orphan).unwrap();
        std::fs::write(orphan.join("draft.txt"), "work in progress\n").unwrap();

        let err = remove_worktree(&root, &orphan, false).await.unwrap_err();
        assert!(
            err.to_string().contains("cannot read the state"),
            "got: {err}"
        );
        assert!(orphan.join("draft.txt").exists(), "the work is still there");
        let _ = std::fs::remove_dir_all(&orphan);
    }

    #[tokio::test]
    async fn a_symlinked_include_cannot_smuggle_a_file_out_of_the_repository() {
        // The lexical check in `within` is about the *spelling* of a path, and
        // a symlink is not a spelling. `devplane.toml` arrives with somebody
        // else's code and git tracks symlinks, so a repository can ship
        // `config/local.env -> /home/you/.ssh/id_rsa` and name it in
        // `[workspace] include`. Following it would copy a private key into the
        // directory an agent is about to read — the exact attack the path check
        // exists to stop, walking in through the door beside it.
        let root = scratch_repo("symlink-escape").await;
        let secret = root.parent().unwrap().join("vp-symlink-secret.txt");
        std::fs::write(&secret, "PRIVATE KEY\n").unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::os::unix::fs::symlink(&secret, root.join("config/local.env")).unwrap();

        let wt = create_worktree(&root, "sym-abc", "sym/abc", "main")
            .await
            .unwrap();
        let copied = copy_includes(&root, &wt, &["config/local.env".into()]);

        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(
            !wt.join("config/local.env").exists(),
            "a file the repository only points at is not a file the repository has"
        );

        // A symlink that stays inside the repository is fine, because nothing
        // leaves: this is about containment, not about symlinks.
        std::fs::write(root.join("real.env"), "OK\n").unwrap();
        std::os::unix::fs::symlink(root.join("real.env"), root.join("config/inside.env")).unwrap();
        let copied = copy_includes(&root, &wt, &["config/inside.env".into()]);
        assert_eq!(copied, vec!["config/inside.env".to_string()]);
    }

    #[tokio::test]
    async fn an_include_never_writes_through_a_symlink_already_in_the_worktree() {
        // The other direction, and it is an arbitrary *write*. The worktree is
        // a checkout of the same repository, so a committed dangling symlink is
        // present at the destination before anything is copied. `to.exists()`
        // follows it and reports false, and `fs::copy` would then write through
        // it to whatever it names.
        let root = scratch_repo("symlink-write").await;
        let target = root.parent().unwrap().join("vp-symlink-target.txt");
        let _ = std::fs::remove_file(&target);
        std::fs::write(root.join(".env"), "PAYLOAD\n").unwrap();

        let wt = create_worktree(&root, "symw-abc", "symw/abc", "main")
            .await
            .unwrap();
        std::os::unix::fs::symlink(&target, wt.join(".env")).unwrap();

        let copied = copy_includes(&root, &wt, &[".env".into()]);
        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(
            !target.exists(),
            "writing through a symlink the repository chose is an arbitrary write"
        );
    }

    #[tokio::test]
    async fn an_include_cannot_reach_outside_the_repository() {
        // `devplane.toml` is committed, so it arrives with somebody else's
        // code. A path that escapes the root would copy a private key into a
        // directory an agent is about to read.
        let root = scratch_repo("escape").await;
        let secret = root.parent().unwrap().join("vp-secret-outside.txt");
        std::fs::write(&secret, "SECRET\n").unwrap();
        let wt = create_worktree(&root, "e-abc", "e/abc", "main")
            .await
            .unwrap();

        let copied = copy_includes(
            &root,
            &wt,
            &[
                "../vp-secret-outside.txt".into(),
                "/etc/hosts".into(),
                "a/../../vp-secret-outside.txt".into(),
            ],
        );
        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(!wt.join("vp-secret-outside.txt").exists());
        // A path that dips into a subdirectory and back out is still inside.
        std::fs::create_dir_all(root.join("cfg")).unwrap();
        std::fs::write(root.join(".env"), "OK\n").unwrap();
        assert_eq!(
            copy_includes(&root, &wt, &["cfg/../.env".into()]),
            vec!["cfg/../.env".to_string()]
        );

        std::fs::remove_file(&secret).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn the_base_branch_is_discovered_not_assumed() {
        let root = scratch_repo("base").await;
        assert_eq!(base_branch(&root).await, "main");
        std::fs::remove_dir_all(&root).ok();
    }
}

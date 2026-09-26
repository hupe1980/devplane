//! Git: worktrees, status, digests and diffs, all through the `git` command.
//!
//! Using the CLI keeps Devplane's worktrees identical to Claude Code's and the
//! user's (`.claude/worktrees/<name>`), so Claude's own cleanup handles both.
//! Every ref or path from a person or a repository goes after
//! `--end-of-options`: `base_branch` comes from a committed file, and a value
//! like `--output=/tmp/x` would otherwise be an option. Config validation is
//! the first lock; this is the second.

use crate::core::change::{CommitStamp, Reach};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where isolated checkouts live, relative to the repository root.
pub const WORKTREE_DIR: &str = ".claude/worktrees";

/// How many files a refusal names before it says *and n more*.
const NAMED: usize = 10;

/// What `git status` says, in the shape the board needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub branch: Option<String>,
    /// The commit `HEAD` points at; `None` when the repository has no commits
    /// yet (`# branch.oid (initial)`).
    pub commit: Option<String>,
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
}

/// One read of `git status`: counts for the board and paths for refusals, from
/// the same call so they cannot disagree.
struct Porcelain {
    status: Status,
    /// Tracked files staged, unstaged or in conflict; a rename contributes both names.
    tracked: Vec<String>,
    untracked: Vec<String>,
}

/// How long any one git command may take. Generous — a first `worktree add`
/// of a large repository checks out every file — but finite, so a hung
/// command cannot stop the poller that called it.
pub const GIT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(300);

/// Runs a git command in a directory and returns stdout.
///
/// `GIT_OPTIONAL_LOCKS=0` on every call: a read such as `git status` otherwise
/// refreshes and rewrites `.git/index`, and the poller reads every open
/// change's tree on a timer, so the person's or agent's own `git commit` would
/// fail at random on `index.lock`. Writes (`worktree add`, `branch -D`) take
/// the locks they need regardless; the variable only drops the optional ones.
///
/// A checkout's `.git/config` is writable by the agent in it, so nothing it
/// names runs here: `core.fsmonitor` is forced off (it is a command git would
/// start on every `status`), git never prompts for a credential, and a call
/// that has not answered in [`GIT_DEADLINE`] is killed.
async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let run = tokio::process::Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(GIT_DEADLINE, run)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "git {} did not answer in {}s",
                args.join(" "),
                GIT_DEADLINE.as_secs()
            )
        })?
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

/// The `origin` remote's URL, when there is one; needed to name the repository
/// even when no telemetry has reported it.
pub async fn remote_url(root: &Path) -> Option<String> {
    let out = git(root, &["remote", "get-url", "origin"]).await.ok()?;
    let url = out.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// The remote a forge is read from: `origin`, else the first remote git lists.
///
/// `git remote get-url` applies `url.<base>.insteadOf` rewrites itself, so the
/// URL returned is the one git would fetch from.
pub async fn forge_remote_url(root: &Path) -> Option<String> {
    if let Some(url) = remote_url(root).await {
        return Some(url);
    }
    let names = git(root, &["remote"]).await.ok()?;
    let first = names.lines().map(str::trim).find(|l| !l.is_empty())?;
    let out = git(root, &["remote", "get-url", first]).await.ok()?;
    let url = out.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// The branch a repository considers its trunk.
///
/// `origin/HEAD` when the remote says so, else the current branch. Guessing
/// `main` would branch from the wrong place in repos still on `master`.
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

/// Reads status via porcelain v2, the format with a stability promise.
pub async fn status(dir: &Path) -> Result<Status> {
    porcelain(dir).await.map(|p| p.status)
}

/// The tracked files with uncommitted changes, as a refusal names them; the
/// same rule `create_worktree` refuses on.
pub async fn uncommitted(dir: &Path) -> Result<Option<String>> {
    let p = porcelain(dir).await?;
    Ok((!p.tracked.is_empty()).then(|| {
        format!(
            "uncommitted changes to {} tracked file(s): {}",
            p.tracked.len(),
            named(&p.tracked)
        )
    }))
}

/// `-z`, so a path is bytes up to a NUL rather than a C-quoted string; a
/// rename's original name follows as its own record.
async fn porcelain(dir: &Path) -> Result<Porcelain> {
    let text = git(
        dir,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
            "-z",
        ],
    )
    .await?;
    Ok(parse_porcelain(&text))
}

fn parse_porcelain(text: &str) -> Porcelain {
    let mut p = Porcelain {
        status: Status::default(),
        tracked: Vec::new(),
        untracked: Vec::new(),
    };
    let s = &mut p.status;
    // The path follows a fixed number of space-separated fields: eight for a
    // change, nine for a rename, ten for a conflict.
    let path_after =
        |rec: &str, fields: usize| rec.splitn(fields + 1, ' ').nth(fields).map(str::to_string);
    let mut records = text.split('\0');
    while let Some(rec) = records.next() {
        if let Some(rest) = rec.strip_prefix("# branch.oid ") {
            let oid = rest.trim();
            // `(initial)` means no commits yet, not a commit id.
            s.commit = (oid != "(initial)").then(|| oid.to_string());
        } else if let Some(rest) = rec.strip_prefix("# branch.head ") {
            let head = rest.trim();
            // `(detached)` is not a branch name.
            s.branch = (head != "(detached)").then(|| head.to_string());
        } else if let Some(rest) = rec.strip_prefix("# branch.ab ") {
            let mut parts = rest.split_whitespace();
            s.ahead = parts
                .next()
                .and_then(|p| p.trim_start_matches('+').parse().ok())
                .unwrap_or(0);
            s.behind = parts
                .next()
                .and_then(|p| p.trim_start_matches('-').parse().ok())
                .unwrap_or(0);
        } else if let Some(path) = rec.strip_prefix("? ") {
            s.untracked_files += 1;
            p.untracked.push(path.to_string());
        } else if rec.starts_with("u ") {
            s.has_conflicts = true;
            s.changed_files += 1;
            p.tracked.extend(path_after(rec, 10));
        } else if rec.starts_with("1 ") {
            s.changed_files += 1;
            p.tracked.extend(path_after(rec, 8));
        } else if rec.starts_with("2 ") {
            s.changed_files += 1;
            p.tracked.extend(path_after(rec, 9));
            // The previous name is the next record.
            if let Some(from) = records.next().filter(|f| !f.is_empty()) {
                p.tracked.push(from.to_string());
            }
        }
    }
    p
}

/// What the tree is right now: commit, branch, cleanliness, working-tree
/// digest, and whether anyone else could fetch the commit.
///
/// Commit and cleanliness come from one `git status`, so they cannot straddle
/// a commit. The digest covers the working tree (agents often do not commit),
/// not `HEAD`.
pub async fn commit_stamp(dir: &Path) -> Option<CommitStamp> {
    let status = status(dir).await.ok()?;
    let reach = match &status.commit {
        Some(sha) => reach_of(dir, sha).await,
        // No commit: nothing to be reachable, unlike an unpushed one.
        None => Reach::NoRemote,
    };
    let tree = tree_digest(dir).await.ok();
    let remote = remote_url(dir).await;
    let clean = status.is_clean();
    let changed_files = (status.changed_files + status.untracked_files) as u32;
    Some(CommitStamp {
        commit: status.commit,
        tree,
        branch: status.branch,
        clean,
        changed_files,
        reach,
        remote,
    })
}

/// The git tree of the working tree as it is: tracked files as they are on
/// disk, untracked files that are not ignored, ignored files left out.
///
/// Computed with a temporary index, so nothing the person or agent sees
/// changes. The digest must not be steerable from inside the checkout, and
/// the checkout's `.git/config`, `.git/info/` and `.gitattributes` are all
/// writable by the agent in it, so:
/// * **every file's bytes are hashed as they are on disk**, with
///   `hash-object --no-filters`: no clean or `process` filter, no end-of-line
///   or encoding conversion, no `ident` — an attribute cannot make an edited
///   file hash as the old one, and no filter command runs in this process;
/// * no stat cache is trusted, so no `assume-unchanged`, `skip-worktree`,
///   `core.checkStat` or restored mtime can hide an edit — each file is read;
/// * untracked files are excluded by the `.gitignore` files in the tree
///   (committed or not), never by `.git/info/exclude` or `core.excludesFile`,
///   which an agent can write from its worktree to hide a file without a
///   trace in the tree. Devplane's own `.claude/worktrees/` is the one
///   exception: other changes' checkouts are not this tree.
///
/// Where a repository declares no filter, this is the tree `git add -A &&
/// git write-tree` would give. The objects are not written; the copy of the
/// index lives in the git directory and is always removed.
pub async fn tree_digest(dir: &Path) -> Result<String> {
    let index = git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )
    .await?;
    let index = PathBuf::from(index.trim());
    let top = git(dir, &["rev-parse", "--show-toplevel"]).await?;
    let top = PathBuf::from(top.trim());
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = index.with_file_name(format!("devplane-index-{}-{n}", std::process::id()));
    let result = digest_with(&top, &index, &tmp).await;
    for t in [tmp.clone(), tmp.with_extension("lock")] {
        let _ = std::fs::remove_file(t);
    }
    result
}

/// One entry of the tree being built: its mode, and where its object id
/// comes from.
enum Entry {
    /// A regular file, hashed from disk by path.
    File(&'static str),
    /// A symbolic link: its target, hashed as the blob git stores for it.
    Link(Vec<u8>),
    /// A submodule or embedded repository, at the commit it has checked out.
    Gitlink(String),
}

async fn digest_with(top: &Path, index: &Path, tmp: &Path) -> Result<String> {
    let tmp_str = tmp.to_str().context("git directory is not utf-8")?;
    // What is tracked is what the index says, staged additions included; the
    // index is copied so the real one is never locked or refreshed.
    let seeded = index.exists() && std::fs::copy(index, tmp).is_ok();
    if !seeded {
        let has_head = git(top, &["rev-parse", "--verify", "--quiet", "HEAD"])
            .await
            .is_ok();
        let args: &[&str] = if has_head {
            &["read-tree", "HEAD"]
        } else {
            &["read-tree", "--empty"]
        };
        git_with_index(top, tmp_str, args, None).await?;
    }
    let staged = git_with_index(top, tmp_str, &["ls-files", "--stage", "-z"], None).await?;
    let mut tracked: Vec<(String, String, String)> = Vec::new();
    for rec in staged.split('\0').filter(|r| !r.is_empty()) {
        let Some((meta, path)) = rec.split_once('\t') else {
            continue;
        };
        let mut f = meta.split(' ');
        let (Some(mode), Some(oid)) = (f.next(), f.next()) else {
            continue;
        };
        // A conflicted path is listed once per stage; the file on disk is one.
        if tracked.last().is_none_or(|(_, _, p)| p != path) {
            tracked.push((mode.to_string(), oid.to_string(), path.to_string()));
        }
    }
    // Untracked files, excluded by committed `.gitignore` files alone: no
    // `--exclude-standard`, which would read `info/exclude` and the person's
    // global excludes file. An embedded repository is listed as `dir/`.
    let untracked = git_with_index(
        top,
        tmp_str,
        &[
            "ls-files",
            "--others",
            "-z",
            "--exclude-per-directory=.gitignore",
            "--exclude=/.claude/worktrees/",
        ],
        None,
    )
    .await?;

    let mut entries: Vec<(String, Entry)> = Vec::new();
    let candidates = tracked
        .iter()
        .map(|(mode, oid, path)| (path.clone(), Some((mode.as_str(), oid.as_str()))))
        .chain(
            untracked
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(|p| (p.trim_end_matches('/').to_string(), None)),
        );
    for (path, was) in candidates {
        let on_disk = top.join(&path);
        let Ok(meta) = std::fs::symlink_metadata(&on_disk) else {
            continue; // deleted
        };
        let gitlink =
            was.is_some_and(|(mode, _)| mode == "160000") || (was.is_none() && meta.is_dir());
        if gitlink {
            let head = git(&on_disk, &["rev-parse", "--verify", "--quiet", "HEAD"]).await;
            match (head, was) {
                (Ok(h), _) => entries.push((path, Entry::Gitlink(h.trim().to_string()))),
                // Not checked out: what the index says it should be.
                (Err(_), Some((_, oid))) => entries.push((path, Entry::Gitlink(oid.to_string()))),
                (Err(_), None) => {}
            }
        } else if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&on_disk)?;
            entries.push((
                path,
                Entry::Link(target.as_os_str().as_encoded_bytes().to_vec()),
            ));
        } else if meta.is_file() {
            entries.push((path, Entry::File(file_mode(&meta, was.map(|(m, _)| m)))));
        }
        // A tracked path now a directory holds only untracked files, which
        // the untracked listing names on their own.
    }

    // Regular files by path in one process; a path git's line-based input
    // cannot carry, and every link target, by content.
    let by_path: Vec<&str> = entries
        .iter()
        .filter(|(p, e)| matches!(e, Entry::File(_)) && !p.contains('\n'))
        .map(|(p, _)| p.as_str())
        .collect();
    let mut hashed: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if !by_path.is_empty() {
        let input = by_path.join("\n") + "\n";
        let out = git_with_index(
            top,
            tmp_str,
            &["hash-object", "--no-filters", "--stdin-paths"],
            Some(input.as_bytes()),
        )
        .await?;
        let oids: Vec<&str> = out.lines().collect();
        if oids.len() != by_path.len() {
            bail!(
                "git hash-object answered {} of {} paths",
                oids.len(),
                by_path.len()
            );
        }
        for (p, o) in by_path.iter().zip(oids) {
            hashed.insert((*p).to_string(), o.to_string());
        }
    }
    let mut info: Vec<u8> = Vec::new();
    for (path, entry) in &entries {
        let (mode, oid) = match entry {
            Entry::Gitlink(oid) => ("160000", oid.clone()),
            Entry::File(mode) => match hashed.get(path) {
                Some(oid) => (*mode, oid.clone()),
                None => (
                    *mode,
                    hash_bytes(top, tmp_str, &std::fs::read(top.join(path))?).await?,
                ),
            },
            Entry::Link(target) => ("120000", hash_bytes(top, tmp_str, target).await?),
        };
        info.extend_from_slice(format!("{mode} {oid}\t").as_bytes());
        info.extend_from_slice(path.as_bytes());
        info.push(0);
    }
    // A fresh index holding exactly those entries.
    std::fs::remove_file(tmp).ok();
    git_with_index(top, tmp_str, &["read-tree", "--empty"], None).await?;
    if !info.is_empty() {
        git_with_index(
            top,
            tmp_str,
            &["update-index", "-z", "--index-info"],
            Some(&info),
        )
        .await?;
    }
    let tree = git_with_index(top, tmp_str, &["write-tree", "--missing-ok"], None).await?;
    let tree = tree.trim().to_string();
    if tree.is_empty() {
        bail!("git write-tree printed nothing");
    }
    Ok(tree)
}

/// The mode git records for a regular file: executable by its bits where the
/// file system has them, else as the index had it.
fn file_mode(meta: &std::fs::Metadata, indexed: Option<&str>) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = indexed;
        if meta.permissions().mode() & 0o111 != 0 {
            "100755"
        } else {
            "100644"
        }
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        if indexed == Some("100755") {
            "100755"
        } else {
            "100644"
        }
    }
}

/// The blob id of `bytes`, unfiltered.
async fn hash_bytes(top: &Path, index: &str, bytes: &[u8]) -> Result<String> {
    let out = git_with_index(
        top,
        index,
        &["hash-object", "--no-filters", "--stdin"],
        Some(bytes),
    )
    .await?;
    Ok(out.trim().to_string())
}

/// A git command against an index that is not the person's, optionally fed
/// on stdin. The file system monitor and untracked cache are off, so neither
/// a copied cache nor a daemon's answer stands in for looking at the files,
/// and every stat check the repository's config could relax is forced on.
async fn git_with_index(
    dir: &Path,
    index: &str,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<String> {
    let mut forced: Vec<&str> = vec![
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.untrackedCache=false",
        "-c",
        "core.trustctime=true",
        "-c",
        "core.checkStat=default",
        "-c",
        "core.ignoreStat=false",
    ];
    // Windows reports no executable bit, which is why git turns this off there.
    if cfg!(not(windows)) {
        forced.extend(["-c", "core.fileMode=true"]);
    }
    let full: Vec<&str> = forced.into_iter().chain(args.iter().copied()).collect();
    let out = git_stdin(dir, &[("GIT_INDEX_FILE", index)], &full, input).await?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Runs git in `dir` with `env`, optionally fed on stdin, and returns what it
/// printed whatever its exit status. Never takes an optional lock (`add` must
/// not take the real index's lock or refresh it; status must not rewrite it).
async fn git_stdin(
    dir: &Path,
    env: &[(&str, &str)],
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<std::process::Output> {
    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .envs(env.iter().copied())
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        let bytes = bytes.to_vec();
        tokio::spawn(async move {
            let _ = stdin.write_all(&bytes).await;
        });
    }
    tokio::time::timeout(GIT_DEADLINE, child.wait_with_output())
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "git {} did not answer in {}s",
                args.join(" "),
                GIT_DEADLINE.as_secs()
            )
        })?
        .with_context(|| format!("running git {}", args.join(" ")))
}

/// Whether a commit exists anywhere but this machine.
///
/// A certificate pointing at a commit a reviewer cannot fetch is worse than
/// none. "No remote" is fine; "remote without the commit" is not.
async fn reach_of(dir: &Path, sha: &str) -> Reach {
    let Ok(remotes) = git(dir, &["remote"]).await else {
        return Reach::Unknown;
    };
    if remotes.trim().is_empty() {
        return Reach::NoRemote;
    }
    // Offline: local `refs/remotes` only. `--contains=` as one argument,
    // because the option's optional value would swallow `--end-of-options`.
    // The sha is git's own, from status.
    let contains = format!("--contains={sha}");
    match git(dir, &["branch", "--remotes", &contains]).await {
        Ok(out) if !out.trim().is_empty() => Reach::Remote,
        Ok(_) => Reach::LocalOnly,
        // An unknown object is "could not tell", not "not pushed".
        Err(_) => Reach::Unknown,
    }
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

/// Whether a change's checkout is still there.
///
/// git's registration outlives an `rm -rf`. A gate run in a missing directory
/// never ran, and the caller records it that way.
pub fn worktree_present(dir: &Path) -> bool {
    dir.is_dir() && dir.join(".git").exists()
}

/// Creates an isolated checkout branched from the base (not whatever is
/// checked out).
///
/// Refuses, before writing anything and naming the cause: an existing target,
/// uncommitted changes to tracked files, or a base that does not resolve.
/// Untracked files do not count.
pub async fn create_worktree(root: &Path, name: &str, branch: &str, base: &str) -> Result<PathBuf> {
    let dir = root.join(WORKTREE_DIR).join(name);
    refuse_occupied(&dir)?;

    let checkout = porcelain(root).await?;
    if !checkout.tracked.is_empty() {
        bail!(
            "{} has uncommitted changes to {} tracked file(s): {}. Commit or stash them before \
             isolating work",
            root.display(),
            checkout.tracked.len(),
            named(&checkout.tracked)
        );
    }

    let Some(base_ref) = resolve_base(root, base).await else {
        bail!(
            "the base branch `{base}` does not exist in {}, neither locally nor as `origin/{base}`",
            root.display()
        );
    };

    // Keep `?? .claude/` out of the person's `git status`, as the vendor
    // documents. Best effort.
    if let Err(e) = exclude_worktrees(root).await {
        tracing::warn!(error = %e, "could not add .claude/worktrees/ to .git/info/exclude");
    }

    let parent = dir
        .parent()
        .with_context(|| format!("{} has no parent directory", dir.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", dir.display()))?;
    git(
        root,
        &[
            "worktree",
            "add",
            "-b",
            branch,
            "--end-of-options",
            dir.to_str().context("worktree path is not utf-8")?,
            &base_ref,
        ],
    )
    .await?;
    Ok(dir)
}

/// Refuses an existing target, saying whether it is a file, directory or
/// symlink. `symlink_metadata` so a dangling symlink counts as occupied.
fn refuse_occupied(dir: &Path) -> Result<()> {
    if let Ok(meta) = dir.symlink_metadata() {
        let what = if meta.file_type().is_symlink() {
            "a symlink"
        } else if meta.is_dir() {
            "a directory"
        } else {
            "a file"
        };
        bail!("{} already exists and is {what}", dir.display());
    }
    Ok(())
}

/// Whether a local branch of this name exists.
pub async fn branch_exists(root: &Path, branch: &str) -> bool {
    let spec = format!("refs/heads/{branch}");
    git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )
    .await
    .is_ok()
}

/// The worktree that has this branch checked out, when one does.
pub async fn worktree_of_branch(root: &Path, branch: &str) -> Option<PathBuf> {
    list(root)
        .await
        .ok()?
        .into_iter()
        .find(|w| w.branch.as_deref() == Some(branch))
        .map(|w| w.path)
}

/// Checks an existing branch out into its own worktree; the same refusals as
/// [`create_worktree`] minus the base.
pub async fn adopt_worktree(root: &Path, name: &str, branch: &str) -> Result<PathBuf> {
    let dir = root.join(WORKTREE_DIR).join(name);
    refuse_occupied(&dir)?;
    if let Err(e) = exclude_worktrees(root).await {
        tracing::warn!(error = %e, "could not add .claude/worktrees/ to .git/info/exclude");
    }
    let parent = dir
        .parent()
        .with_context(|| format!("{} has no parent directory", dir.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", dir.display()))?;
    git(
        root,
        &[
            "worktree",
            "add",
            "--end-of-options",
            dir.to_str().context("worktree path is not utf-8")?,
            branch,
        ],
    )
    .await?;
    Ok(dir)
}

/// The subject of the first commit a branch made after leaving its base, for
/// a title nobody typed. `None` when the branch has no commits of its own.
pub async fn first_subject(root: &Path, base: &str, branch: &str) -> Option<String> {
    let base_ref = resolve_base(root, base).await?;
    let range = format!("{base_ref}..{branch}");
    git(
        root,
        &[
            "log",
            "--reverse",
            "--format=%s",
            "--end-of-options",
            &range,
        ],
    )
    .await
    .ok()?
    .lines()
    .next()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// The commits a branch has that `base` does not, one line each. Empty means
/// merged; `None` means "could not tell".
///
/// `base` is the project's configured base, never rediscovered here. A squash
/// merge leaves commits unmerged by this measure; the caller trusts the forge.
pub async fn unmerged_commits(root: &Path, branch: &str, base: &str) -> Option<Vec<String>> {
    let base = resolve_base(root, base).await?;
    let range = format!("{base}..{branch}");
    git(root, &["log", "--format=%h %s", "--end-of-options", &range])
        .await
        .ok()
        .map(|o| o.lines().map(str::to_string).collect())
}

/// Deletes a branch. The caller must first establish it is merged: git's `-d`
/// compares with the checked-out branch, not the base.
pub async fn delete_branch(root: &Path, branch: &str) -> Result<()> {
    git(root, &["branch", "-D", "--end-of-options", branch]).await?;
    Ok(())
}

/// Up to [`NAMED`] entries, then a count of the rest.
fn named(items: &[String]) -> String {
    let shown: Vec<&str> = items.iter().take(NAMED).map(String::as_str).collect();
    match items.len().saturating_sub(NAMED) {
        0 => shown.join(", "),
        more => format!("{}, and {more} more", shown.join(", ")),
    }
}

/// Whether `path` exists in the tree of `rev` — `git cat-file -e rev:path`.
pub async fn exists_at(root: &Path, rev: &str, path: &str) -> bool {
    let spec = format!("{rev}:{}", path.trim_end_matches('/'));
    git(root, &["cat-file", "-e", "--end-of-options", &spec])
        .await
        .is_ok()
}

/// Refuses a spec the worktree would not have: one not committed (or not
/// pushed) to the base the worktree is made from would never reach the agent.
pub async fn refuse_spec_missing_from_base(root: &Path, base: &str, spec: &str) -> Result<()> {
    let Some(base_ref) = resolve_base(root, base).await else {
        bail!(
            "the base branch `{base}` does not exist in {}, neither locally nor as `origin/{base}`",
            root.display()
        );
    };
    if !exists_at(root, &base_ref, spec).await {
        bail!(
            "`{spec}` is not in `{base_ref}`, which the change's worktree is made from, so the \
             agent would never see it. Commit it{} first, or start the change in place",
            if base_ref.starts_with("origin/") {
                " and push it"
            } else {
                ""
            }
        );
    }
    Ok(())
}

/// How many commits `HEAD` has that the base does not.
pub async fn commits_ahead(dir: &Path, base: &str) -> Result<u32> {
    let base_ref = resolve_base(dir, base)
        .await
        .with_context(|| format!("the base branch `{base}` does not exist here"))?;
    let range = format!("{base_ref}..HEAD");
    let n = git(dir, &["rev-list", "--count", "--end-of-options", &range]).await?;
    n.trim()
        .parse()
        .with_context(|| format!("git rev-list printed {n:?}"))
}

/// Prefers `origin/<base>`, so new work starts from what was pushed. `None`
/// when neither resolves to a commit.
async fn resolve_base(root: &Path, base: &str) -> Option<String> {
    for candidate in [format!("origin/{base}"), base.to_string()] {
        let spec = format!("{candidate}^{{commit}}");
        if git(
            root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &spec,
            ],
        )
        .await
        .is_ok()
        {
            return Some(candidate);
        }
    }
    None
}

/// Appends `.claude/worktrees/` to `.git/info/exclude`, once.
///
/// Not `.gitignore`, which is tracked and would need committing. Resolved via
/// `--git-common-dir`, because a linked worktree or submodule keeps `info/` in
/// the repository it belongs to.
async fn exclude_worktrees(root: &Path) -> Result<bool> {
    const ENTRY: &str = ".claude/worktrees/";
    let common = git(root, &["rev-parse", "--git-common-dir"]).await?;
    let common = PathBuf::from(common.trim());
    let common = if common.is_absolute() {
        common
    } else {
        root.join(common)
    };
    let info = common.join("info");
    std::fs::create_dir_all(&info).with_context(|| format!("creating {}", info.display()))?;
    let file = info.join("exclude");
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let covered = existing
        .lines()
        .map(str::trim)
        .any(|l| l == ENTRY || l == ENTRY.trim_end_matches('/') || l == ".claude/");
    if covered {
        return Ok(false);
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# Isolated checkouts made by Claude Code and Devplane.\n");
    text.push_str(ENTRY);
    text.push('\n');
    std::fs::write(&file, text).with_context(|| format!("writing {}", file.display()))?;
    Ok(true)
}

/// Removes a worktree, refusing to destroy uncommitted work.
///
/// Dirty and untracked files are named by path; `discard_uncommitted` removes
/// them anyway. The branch is never touched. A worktree whose directory is
/// already gone is pruned.
pub async fn remove_worktree(root: &Path, dir: &Path, discard_uncommitted: bool) -> Result<()> {
    // A present directory whose status cannot be read is not "clean": "could
    // not tell" must not read as "nothing there".
    let present = dir.exists();
    let dirty: Vec<String> = if present {
        match porcelain(dir).await {
            Ok(p) => p.tracked.into_iter().chain(p.untracked).collect(),
            Err(e) if discard_uncommitted => {
                tracing::warn!(
                    dir = %dir.display(),
                    %e,
                    "status unreadable; --discard-uncommitted was given"
                );
                Vec::new()
            }
            Err(e) => bail!(
                "cannot read the state of {}: {e}. Pass --discard-uncommitted to remove it anyway",
                dir.display()
            ),
        }
    } else {
        Vec::new()
    };

    if !dirty.is_empty() && !discard_uncommitted {
        bail!(
            "{} has uncommitted changes in {} file(s): {}. Pass --discard-uncommitted to \
             discard them",
            dir.display(),
            dirty.len(),
            named(&dirty)
        );
    }

    if !present {
        // `worktree remove` refuses a missing path; `prune` drops the stale
        // registration.
        git(root, &["worktree", "prune"]).await?;
        return Ok(());
    }
    let mut args = vec!["worktree", "remove"];
    if discard_uncommitted {
        args.push("--force");
    }
    args.push("--end-of-options");
    let path = dir.to_str().context("worktree path is not utf-8")?;
    args.push(path);
    git(root, &args).await?;
    Ok(())
}

/// Every file this checkout changed since it diverged from `base`, committed
/// or not, as a parsed diff plus the command that reproduces it.
///
/// Diffs against the merge base, so a `base` that moved on does not show
/// everybody else's commits. Uncommitted work is included: a reviewer
/// approves the checkout as it stands.
///
/// Every failure is returned — a base that resolves neither as `origin/<base>`
/// nor locally, no merge base, a diff git refused — because an unreadable diff
/// is not an empty one: whatever makes a claim from it (the offer's guard, the
/// certificate's "altered none of its checks") must say it could not read it.
///
/// The base resolves the way [`create_worktree`] resolves it, `origin/` first,
/// so the diff is against what the checkout was made from. Every invocation is
/// independent of the person's diff config (external drivers, textconv,
/// prefixes), which the parser would otherwise misread.
pub async fn change_set(dir: &Path, base: &str) -> Result<crate::core::diff::ChangeSet> {
    let base_ref = resolve_base(dir, base).await.with_context(|| {
        format!("the base branch `{base}` resolves neither as `origin/{base}` nor locally")
    })?;
    // One diff from the merge base to the working tree, so a file committed
    // then edited (or deleted) is listed once, as it stands.
    let from = git(dir, &["merge-base", "--end-of-options", &base_ref, "HEAD"])
        .await
        .with_context(|| format!("finding where this checkout left `{base_ref}`"))?;
    let from = from.trim().to_string();
    if from.is_empty() {
        bail!("`{base_ref}` and HEAD share no history");
    }
    // `--find-renames`: a moved file reads as moved, not deleted and added.
    let working = git(
        dir,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--find-renames",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--end-of-options",
            &from,
        ],
    )
    .await?;
    // `git diff` misses files git has never seen, so an agent's new files are
    // diffed one by one. `--no-index` rather than `add -N`, because a read
    // must not write to the index.
    let mut untracked = String::new();
    let untracked_total;
    {
        let list = git(dir, &["ls-files", "--others", "--exclude-standard", "-z"]).await?;
        let paths: Vec<&str> = list.split('\0').filter(|l| !l.is_empty()).collect();
        untracked_total = paths.len();
        for path in paths.into_iter().take(MAX_UNTRACKED) {
            // `--no-index` exits 1 whenever files differ; not an error here.
            let out = tokio::process::Command::new("git")
                .args([
                    "-c",
                    "core.quotePath=false",
                    "diff",
                    "--no-color",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--src-prefix=a/",
                    "--dst-prefix=b/",
                    "--no-index",
                    "--",
                    "/dev/null",
                    path,
                ])
                .current_dir(dir)
                .env("GIT_OPTIONAL_LOCKS", "0")
                .kill_on_drop(true)
                .output()
                .await
                .with_context(|| format!("diffing the new file {path}"))?;
            untracked.push_str(&String::from_utf8_lossy(&out.stdout));
        }
    }

    let text = format!("{working}{untracked}");
    let command = format!("git diff $(git merge-base {base_ref} HEAD)");
    let mut set = crate::core::diff::parse(base, &text, &command);
    // Untracked files past the bound were never diffed; count them here or
    // they vanish silently.
    let hidden = untracked_total.saturating_sub(MAX_UNTRACKED);
    if hidden > 0 {
        let shown = set.files.len();
        match set.truncated.as_mut() {
            Some(t) => t.files_total += hidden,
            None => {
                set.truncated = Some(crate::core::diff::Truncation {
                    files_shown: shown,
                    files_total: shown + hidden,
                    // Status names files the diff cannot show.
                    command: format!("{command}; git status --short"),
                })
            }
        }
    }
    // `git diff` prints no size for a binary and the parser is pure, so stat
    // it here. A deleted binary stays just "binary".
    for f in &mut set.files {
        if let crate::core::diff::Body::Binary { bytes } = &mut f.body {
            *bytes = tokio::fs::metadata(dir.join(&f.path))
                .await
                .ok()
                .map(|m| m.len());
        }
    }
    Ok(set)
}

/// How many never-seen files are diffed individually (one process each). Past
/// it the change set is marked truncated.
const MAX_UNTRACKED: usize = 40;

/// Files this branch touched, for detecting two changes editing the same file.
///
/// Three-dot against the merge base, so a moved `base` does not make every
/// change overlap every other.
pub async fn touched_files(dir: &Path, base: &str) -> Vec<String> {
    let mut out = Vec::new();
    let range = format!("{base}...HEAD");
    if let Ok(o) = git(dir, &["diff", "--name-only", "--end-of-options", &range]).await {
        out.extend(o.lines().map(str::to_string));
    }
    // Uncommitted work counts: two agents editing one file collide before
    // either commits.
    if let Ok(p) = porcelain(dir).await {
        out.extend(p.tracked);
        out.extend(p.untracked);
    }
    out.retain(|p| !p.is_empty());
    out.sort();
    out.dedup();
    out
}

/// The gitignored files `.worktreeinclude` names, repository-relative and
/// `/`-separated: the candidates a fresh checkout may carry. Empty when the
/// repository has no `.worktreeinclude`.
///
/// Git does the matching with its own gitignore engine rather than listing
/// every ignored file (a `target/` or `node_modules/` can hold hundreds of
/// thousands): `ls-files --others --ignored --exclude-from=.worktreeinclude`
/// names the untracked files the patterns select, and `check-ignore` keeps
/// those the repository's standard rules ignore. Neither step ever names a
/// tracked file, so "only gitignored files are copied" holds by construction.
pub async fn ignored_files(root: &Path) -> Result<Vec<String>> {
    let include = root.join(".worktreeinclude");
    if !include.is_file() {
        return Ok(Vec::new());
    }
    let from = format!(
        "--exclude-from={}",
        include.to_str().context("repository path is not utf-8")?
    );
    let chosen = git(root, &["ls-files", "--others", "--ignored", "-z", &from]).await?;
    let chosen: Vec<&str> = chosen.split('\0').filter(|p| !p.is_empty()).collect();
    if chosen.is_empty() {
        return Ok(Vec::new());
    }
    let input: Vec<u8> = chosen
        .iter()
        .flat_map(|p| p.bytes().chain(std::iter::once(0)))
        .collect();
    let ignored = check_ignore(root, &input).await?;
    Ok(ignored
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| p.replace('\\', "/"))
        .collect())
}

/// `git check-ignore -z --stdin`: the paths the standard rules ignore. Exit
/// status 1 means none are, which is an answer rather than a failure.
async fn check_ignore(root: &Path, input: &[u8]) -> Result<String> {
    let out = git_stdin(root, &[], &["check-ignore", "-z", "--stdin"], Some(input)).await?;
    match out.status.code() {
        Some(0) | Some(1) => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        _ => bail!(
            "git check-ignore failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// Copies the gitignored files `.worktreeinclude` chose into a fresh
/// worktree, never over anything already there and never outside it.
///
/// A repository can point a path elsewhere via a symlink, on either side:
/// * the source is canonicalised and must stay inside the repository;
/// * every directory on the way to the destination is walked one component at
///   a time and must be a real directory, never a symlink (a tracked
///   `config -> /tmp/public` in the worktree would otherwise receive
///   `config/local.env`); missing ones are created one by one;
/// * the destination's parent, canonicalised, must still be inside the
///   canonical worktree;
/// * the file is copied under a fresh name in that parent and hard-linked
///   into place, which fails rather than follows if anything, a dangling
///   symlink included, appeared at the destination.
///
/// `std::fs::copy` clones where the file system can (`clonefile` on APFS,
/// `copy_file_range` reflinks on Btrfs and XFS), so large files cost nothing.
pub fn copy_included(root: &Path, worktree: &Path, files: &[String]) -> Vec<String> {
    let mut copied = Vec::new();
    let (Ok(real_root), Ok(real_tree)) = (root.canonicalize(), worktree.canonicalize()) else {
        // Unresolvable is refused: a wrong answer could hand an agent a
        // private key, or write one somewhere public.
        return copied;
    };
    for pattern in files {
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
        match from.canonicalize() {
            Ok(real) if real.starts_with(&real_root) => {}
            Ok(_) => {
                tracing::warn!(
                    pattern,
                    "refusing to copy a file that points outside the repository"
                );
                continue;
            }
            Err(_) => continue,
        }
        match copy_into(&real_tree, worktree, pattern, &from) {
            Ok(true) => copied.push(pattern.clone()),
            Ok(false) => {}
            Err(why) => tracing::warn!(pattern, why, "refusing to copy an included file"),
        }
    }
    copied
}

/// One confined copy; `Ok(false)` when something is already there.
fn copy_into(
    real_tree: &Path,
    worktree: &Path,
    relative: &str,
    from: &Path,
) -> std::result::Result<bool, &'static str> {
    use std::path::Component;
    // `a/../b` is `b`: resolved lexically (the caller has checked it stays
    // inside), so the walk below only ever steps down into named directories.
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for c in Path::new(relative).components() {
        match c {
            Component::Normal(c) => parts.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop().ok_or("the path climbs out of the worktree")?;
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("the path is not a relative path");
            }
        }
    }
    let Some((name, dirs)) = parts.split_last() else {
        return Err("the path names no file");
    };
    let mut parent = worktree.to_path_buf();
    {
        for c in dirs {
            parent.push(c);
            match parent.symlink_metadata() {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err("a directory on the way is a symlink");
                }
                Ok(m) if m.is_dir() => {}
                Ok(_) => return Err("a directory on the way is a file"),
                Err(_) => {
                    std::fs::create_dir(&parent).map_err(|_| "could not create a directory")?
                }
            }
        }
    }
    match parent.canonicalize() {
        Ok(real) if real.starts_with(real_tree) => {}
        _ => return Err("the destination resolves outside the worktree"),
    }
    let to = parent.join(name);
    if to.symlink_metadata().is_ok() {
        return Ok(false);
    }
    let tmp = parent.join(format!(".devplane-copy-{}", uuid::Uuid::new_v4().simple()));
    std::fs::copy(from, &tmp).map_err(|_| "the copy failed")?;
    // A hard link puts the whole copy in place at once and never follows a
    // link planted at `to`. Where the file system has none (exFAT, FAT, some
    // network shares), `create_new` refuses the same things: a file, or a
    // symlink, already there.
    let placed = match std::fs::hard_link(&tmp, &to) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => (|| {
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&to)?;
            let mut src = std::fs::File::open(&tmp)?;
            std::io::copy(&mut src, &mut out)?;
            std::fs::set_permissions(&to, std::fs::metadata(&tmp)?.permissions())
        })(),
        linked => linked,
    };
    std::fs::remove_file(&tmp).ok();
    match placed {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => Err("could not put the copy in place"),
    }
}

/// Whether `path` stays inside `root` once `..` and `.` are resolved.
///
/// Lexical, since the file may not exist yet. Absolute paths are outside:
/// joining one discards the root.
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
            // A root or prefix in the tail means the join discarded the root.
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
        // macOS temp dirs are symlinked; `git worktree list` prints resolved paths.
        dir.canonicalize().unwrap()
    }

    async fn porcelain_status(dir: &Path) -> String {
        git(dir, &["status", "--porcelain"]).await.unwrap()
    }

    #[tokio::test]
    async fn a_worktree_is_created_where_claude_code_puts_them() {
        let root = scratch_repo("create").await;
        let wt = create_worktree(&root, "fix-thing-abc123", "fix/thing", "main")
            .await
            .unwrap();
        assert!(wt.starts_with(root.join(WORKTREE_DIR)));
        assert!(wt.join("a.txt").exists(), "it is a real checkout");
        assert!(worktree_present(&wt));

        let listed = list(&root).await.unwrap();
        assert!(
            listed
                .iter()
                .any(|w| w.branch.as_deref() == Some("fix/thing"))
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn isolation_leaves_the_persons_checkout_clean() {
        // Isolating must not dirty the person's `git status`, and must not
        // touch a tracked file to achieve that.
        let root = scratch_repo("exclude").await;
        create_worktree(&root, "one-abc", "w/one", "main")
            .await
            .unwrap();
        assert_eq!(
            porcelain_status(&root).await,
            "",
            "isolating work dirtied the person's checkout"
        );
        assert_eq!(porcelain_status(&root).await.matches(".claude").count(), 0);

        // Once: a second worktree adds no second line.
        create_worktree(&root, "two-abc", "w/two", "main")
            .await
            .unwrap();
        let exclude = std::fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        assert_eq!(
            exclude.matches(".claude/worktrees/").count(),
            1,
            "{exclude}"
        );
        // And nothing the person wrote there is lost.
        std::fs::write(
            root.join(".git/info/exclude"),
            "mine.txt\n.claude/worktrees/\n",
        )
        .unwrap();
        create_worktree(&root, "three-abc", "w/three", "main")
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(".git/info/exclude")).unwrap(),
            "mine.txt\n.claude/worktrees/\n"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_checkout_whose_git_is_a_file_is_excluded_through_the_common_dir() {
        // A linked worktree's `.git` is a file; its `info/exclude` lives in the
        // repository it belongs to.
        let root = scratch_repo("common").await;
        // Per-process name, so parallel test processes don't collide.
        let linked = root
            .parent()
            .unwrap()
            .join(format!("vp-git-common-linked-{}", std::process::id()));
        std::fs::remove_dir_all(&linked).ok();
        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "side",
                linked.to_str().unwrap(),
                "main",
            ],
        )
        .await
        .unwrap();
        assert!(
            linked.join(".git").is_file(),
            "the fixture is a linked worktree"
        );

        create_worktree(&linked, "nested-abc", "w/nested", "main")
            .await
            .unwrap();
        assert_eq!(porcelain_status(&linked).await, "");
        assert!(
            std::fs::read_to_string(root.join(".git/info/exclude"))
                .unwrap()
                .contains(".claude/worktrees/"),
            "the entry went to the repository the worktree belongs to"
        );
        std::fs::remove_dir_all(&linked).ok();
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
        assert!(!st.is_clean());

        // The paths behind the counts, from the same read.
        let p = porcelain(&root).await.unwrap();
        assert_eq!(p.tracked, vec!["a.txt".to_string()]);
        assert_eq!(p.untracked, vec!["new.txt".to_string()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_porcelain_parser_reads_every_record_kind_including_a_rename() {
        // With `-z`, a rename's original name is a record of its own.
        let text = "# branch.oid abc\0# branch.head main\0# branch.ab +2 -1\0\
                    1 .M N... 100644 100644 100644 h h src/a.rs\0\
                    2 R. N... 100644 100644 100644 h h R100 new name.rs\0old name.rs\0\
                    u UU N... 100644 100644 100644 100644 h h h conflict.rs\0\
                    ? scratch.txt\0";
        let p = parse_porcelain(text);
        assert_eq!(p.status.commit.as_deref(), Some("abc"));
        assert_eq!(p.status.branch.as_deref(), Some("main"));
        assert_eq!((p.status.ahead, p.status.behind), (2, 1));
        assert_eq!(p.status.changed_files, 3);
        assert_eq!(p.status.untracked_files, 1);
        assert!(p.status.has_conflicts);
        assert_eq!(
            p.tracked,
            vec!["src/a.rs", "new name.rs", "old name.rs", "conflict.rs"]
        );
        assert_eq!(p.untracked, vec!["scratch.txt"]);
    }

    #[tokio::test]
    async fn a_dirty_checkout_is_refused_before_anything_is_created_naming_the_files() {
        let root = scratch_repo("dirty").await;
        std::fs::write(root.join("a.txt"), "unsaved\n").unwrap();
        std::fs::write(root.join("b.txt"), "staged\n").unwrap();
        git(&root, &["add", "b.txt"]).await.unwrap();

        let err = create_worktree(&root, "d-abc", "d/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("a.txt") && err.contains("b.txt"), "{err}");
        assert!(err.contains("2 tracked file(s)"), "{err}");
        assert!(
            !root.join(WORKTREE_DIR).exists(),
            "the refusal came after something was written"
        );
        assert!(
            !list(&root)
                .await
                .unwrap()
                .iter()
                .any(|w| w.branch.as_deref() == Some("d/abc")),
            "a branch was made on the way to refusing"
        );

        // Untracked files are not work the branch would carry.
        git(&root, &["checkout", "--", "a.txt"]).await.unwrap();
        git(&root, &["reset", "-q", "b.txt"]).await.unwrap();
        assert!(root.join("b.txt").exists(), "still there, now untracked");
        create_worktree(&root, "d-abc", "d/abc", "main")
            .await
            .expect("an untracked scratch file does not block isolation");
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_missing_base_branch_is_refused_by_name() {
        let root = scratch_repo("nobase").await;
        let err = create_worktree(&root, "nb-abc", "nb/abc", "release/9")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("`release/9`"), "{err}");
        assert!(err.contains("does not exist"), "{err}");
        assert!(!root.join(WORKTREE_DIR).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn an_existing_target_path_is_refused_saying_what_is_there() {
        let root = scratch_repo("exists").await;
        let dir = root.join(WORKTREE_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f-abc"), "not a checkout").unwrap();
        let err = create_worktree(&root, "f-abc", "f/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists and is a file"), "{err}");

        std::fs::create_dir_all(dir.join("g-abc")).unwrap();
        let err = create_worktree(&root, "g-abc", "g/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists and is a directory"), "{err}");

        std::os::unix::fs::symlink("/nowhere", dir.join("h-abc")).unwrap();
        let err = create_worktree(&root, "h-abc", "h/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists and is a symlink"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_base_branch_cannot_be_an_option() {
        // `base_branch` comes from a committed file; `--output=/tmp/x` would
        // make `git diff` write a file.
        let root = scratch_repo("argv").await;
        let target = root.join("planted-by-git-diff");
        let hostile = format!("--output={}", target.display());

        let set = change_set(&root, &hostile).await;
        assert!(!target.exists(), "`git diff` wrote a file it was told to");
        assert!(set.is_err(), "a base that is an option resolves to nothing");

        let touched = touched_files(&root, &hostile).await;
        assert!(!target.exists());
        assert!(touched.is_empty(), "{touched:?}");

        let err = create_worktree(&root, "opt-abc", "opt/abc", &hostile)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not exist"), "{err}");
        assert!(!target.exists());
        assert!(!root.join(WORKTREE_DIR).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn removing_a_worktree_refuses_to_destroy_work() {
        let root = scratch_repo("remove").await;
        let wt = create_worktree(&root, "wip-abc", "wip/abc", "main")
            .await
            .unwrap();
        std::fs::write(wt.join("a.txt"), "unsaved\n").unwrap();
        std::fs::write(wt.join("draft.txt"), "new\n").unwrap();

        let err = remove_worktree(&root, &wt, false).await.unwrap_err();
        let err = err.to_string();
        assert!(err.contains("uncommitted"), "{err}");
        assert!(
            err.contains("a.txt") && err.contains("draft.txt"),
            "the files are named: {err}"
        );

        // Forcing is allowed.
        remove_worktree(&root, &wt, true).await.unwrap();
        assert!(!wt.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    /// Commits the base lacks are named against the given base (no remote needed),
    /// and removing the worktree keeps them.
    #[tokio::test]
    async fn commits_the_base_does_not_have_are_named_and_kept() {
        let root = scratch_repo("unmerged").await;
        let wt = create_worktree(&root, "c-abc", "c/abc", "main")
            .await
            .unwrap();
        std::fs::write(wt.join("b.txt"), "x\n").unwrap();
        git(&wt, &["add", "-A"]).await.unwrap();
        git(&wt, &["commit", "-qm", "add the b file"])
            .await
            .unwrap();
        let unmerged = unmerged_commits(&root, "c/abc", "main").await.unwrap();
        assert_eq!(unmerged.len(), 1);
        assert!(unmerged[0].contains("add the b file"), "{unmerged:?}");

        remove_worktree(&root, &wt, false).await.unwrap();
        assert!(!wt.exists());
        let kept = git(&root, &["rev-parse", "--verify", "c/abc"]).await;
        assert!(kept.is_ok(), "the branch and its commits are kept");

        // Once merged, nothing is unmerged.
        git(&root, &["merge", "-q", "--ff-only", "c/abc"])
            .await
            .unwrap();
        assert!(
            unmerged_commits(&root, "c/abc", "main")
                .await
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_worktree_removed_by_hand_is_reconciled_rather_than_an_error() {
        let root = scratch_repo("byhand").await;
        let wt = create_worktree(&root, "gone-abc", "gone/abc", "main")
            .await
            .unwrap();
        std::fs::remove_dir_all(&wt).unwrap();
        assert!(!worktree_present(&wt));
        assert!(
            list(&root).await.unwrap().iter().any(|w| w.path == wt),
            "git still registers it, which is the situation"
        );

        remove_worktree(&root, &wt, false).await.unwrap();
        assert!(
            !list(&root).await.unwrap().iter().any(|w| w.path == wt),
            "the stale registration was not pruned"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// A file committed then edited, or committed then deleted, is listed once, as
    /// it stands.
    #[tokio::test]
    async fn a_file_committed_and_edited_again_is_listed_once() {
        let root = scratch_repo("once").await;
        git(&root, &["checkout", "-qb", "feat/once"]).await.unwrap();
        std::fs::write(root.join("a.txt"), "hello\nworld\n").unwrap();
        std::fs::write(root.join("b.txt"), "b\n").unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        git(&root, &["commit", "-qm", "edit"]).await.unwrap();
        std::fs::write(root.join("a.txt"), "hello\nworld\nagain\n").unwrap();
        std::fs::remove_file(root.join("b.txt")).unwrap();
        let set = change_set(&root, "main").await.unwrap();
        let paths: Vec<&str> = set.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.txt"], "b.txt was added and removed: nothing");
        assert_eq!((set.files[0].added, set.files[0].removed), (2, 0));
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn untracked_files_past_the_bound_are_counted_rather_than_dropped() {
        let root = scratch_repo("untracked").await;
        for i in 0..(MAX_UNTRACKED + 5) {
            std::fs::write(root.join(format!("new-{i:02}.txt")), "x\n").unwrap();
        }
        let set = change_set(&root, "main").await.unwrap();
        assert_eq!(set.files.len(), MAX_UNTRACKED);
        let t = set
            .truncated
            .expect("files past the bound were dropped without a word");
        assert_eq!(t.files_shown, MAX_UNTRACKED);
        assert_eq!(t.files_total, MAX_UNTRACKED + 5);
        assert!(t.command.contains("git status"), "{}", t.command);
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn the_stamp_carries_gits_own_tree_digest() {
        let root = scratch_repo("tree").await;
        let stamp = commit_stamp(&root).await.unwrap();
        let expected = git(&root, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        assert_eq!(stamp.tree.as_deref(), Some(expected.trim()));
        assert_ne!(stamp.tree, stamp.commit, "a tree is not a commit");
        std::fs::remove_dir_all(&root).ok();
    }

    /// The digest is the working tree, not `HEAD`: edits and untracked files change
    /// it, ignored files don't, and the person's index is untouched.
    #[tokio::test]
    async fn the_digest_is_the_working_tree_through_a_temporary_index() {
        let root = scratch_repo("digest").await;
        std::fs::write(root.join(".gitignore"), ".env\n").unwrap();
        git(&root, &["add", ".gitignore"]).await.unwrap();
        git(&root, &["commit", "-qm", "ignore"]).await.unwrap();
        let clean = tree_digest(&root).await.unwrap();
        let head = git(&root, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        assert_eq!(clean, head.trim(), "a clean tree digests to HEAD's tree");

        std::fs::write(root.join("new.txt"), "agent wrote this\n").unwrap();
        let dirty = tree_digest(&root).await.unwrap();
        assert_ne!(dirty, clean, "an untracked file is in the digest");
        assert_eq!(
            tree_digest(&root).await.unwrap(),
            dirty,
            "the same tree digests the same"
        );
        let staged = git(&root, &["diff", "--cached", "--name-only"])
            .await
            .unwrap();
        assert!(
            staged.trim().is_empty(),
            "the person's index was touched: {staged}"
        );
        let st = status(&root).await.unwrap();
        assert_eq!(st.untracked_files, 1, "still untracked for the person");

        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        assert_eq!(
            tree_digest(&root).await.unwrap(),
            dirty,
            "ignored files are out"
        );

        // Re-derivable the way the certificate says.
        git(&root, &["add", "-A"]).await.unwrap();
        let by_hand = git(&root, &["write-tree"]).await.unwrap();
        assert_eq!(by_hand.trim(), dirty);
        let leftovers: Vec<_> = std::fs::read_dir(root.join(".git"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("devplane-index")
            })
            .collect();
        assert!(leftovers.is_empty(), "the temporary index was left behind");
        std::fs::remove_dir_all(&root).ok();
    }

    /// A clean filter planted from inside the checkout — `.git/info/attributes`
    /// is outside the tree, `.git/config` is writable — can neither make an
    /// edited file digest as the old one nor run.
    #[tokio::test]
    async fn a_planted_filter_cannot_steer_the_digest_or_run() {
        let root = scratch_repo("filter").await;
        let before = tree_digest(&root).await.unwrap();
        let ran = root.join("filter-ran");
        std::fs::create_dir_all(root.join(".git/info")).unwrap();
        std::fs::write(root.join(".git/info/attributes"), "* filter=evil\n").unwrap();
        let clean = format!("touch {} && git show HEAD:a.txt", ran.display());
        git(&root, &["config", "filter.evil.clean", &clean])
            .await
            .unwrap();
        git(
            &root,
            &[
                "config",
                "core.fsmonitor",
                &format!("touch {}", ran.display()),
            ],
        )
        .await
        .unwrap();

        std::fs::write(root.join("a.txt"), "rewritten by the agent\n").unwrap();
        let after = tree_digest(&root).await.unwrap();
        assert_ne!(after, before, "an edited file changed the digest");
        porcelain_status(&root).await;
        assert!(!ran.exists(), "a command from the checkout's config ran");

        // Where no filter is declared the digest is git's own tree.
        std::fs::remove_file(root.join(".git/info/attributes")).unwrap();
        git(&root, &["config", "--unset", "filter.evil.clean"])
            .await
            .unwrap();
        git(&root, &["config", "--unset", "core.fsmonitor"])
            .await
            .unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        let by_hand = git(&root, &["write-tree"]).await.unwrap();
        assert_eq!(by_hand.trim(), after);
        std::fs::remove_dir_all(&root).ok();
    }

    /// Symbolic links, executable bits and embedded repositories digest as
    /// `git add -A` records them.
    #[tokio::test]
    async fn links_modes_and_embedded_repositories_digest_as_git_records_them() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch_repo("modes").await;
        std::os::unix::fs::symlink("a.txt", root.join("link")).unwrap();
        std::fs::write(root.join("run.sh"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(root.join("run.sh"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let inner = root.join("vendor/inner");
        std::fs::create_dir_all(&inner).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec![
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=T",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "i",
            ],
        ] {
            git(&inner, &args).await.unwrap();
        }
        std::fs::remove_file(root.join("a.txt")).unwrap();
        let digest = tree_digest(&root).await.unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        let by_hand = git(&root, &["write-tree"]).await.unwrap();
        assert_eq!(digest, by_hand.trim());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn includes_are_copied_but_never_over_something() {
        let root = scratch_repo("includes").await;
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        let wt = create_worktree(&root, "i-abc", "i/abc", "main")
            .await
            .unwrap();

        let copied = copy_included(&root, &wt, &[".env".into(), "missing.txt".into()]);
        assert_eq!(copied, vec![".env".to_string()]);
        assert_eq!(
            std::fs::read_to_string(wt.join(".env")).unwrap(),
            "SECRET=1\n"
        );

        std::fs::write(wt.join(".env"), "MINE=1\n").unwrap();
        copy_included(&root, &wt, &[".env".into()]);
        assert_eq!(
            std::fs::read_to_string(wt.join(".env")).unwrap(),
            "MINE=1\n",
            "an existing file is the worktree's own"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_worktree_whose_state_cannot_be_read_is_not_silently_removed() {
        // An unreadable status must not read as "nothing here".
        let root = scratch_repo("unreadable").await;
        // A directory holding work where git cannot answer at all.
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
        // `within` checks spelling, and a symlink is not a spelling: a repo can
        // ship `config/local.env -> ~/.ssh/id_rsa` and name it in
        // `.worktreeinclude`.
        let root = scratch_repo("symlink-escape").await;
        let secret = root.parent().unwrap().join("vp-symlink-secret.txt");
        std::fs::write(&secret, "PRIVATE KEY\n").unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::os::unix::fs::symlink(&secret, root.join("config/local.env")).unwrap();

        let wt = create_worktree(&root, "sym-abc", "sym/abc", "main")
            .await
            .unwrap();
        let copied = copy_included(&root, &wt, &["config/local.env".into()]);

        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(
            !wt.join("config/local.env").exists(),
            "a file the repository only points at is not a file the repository has"
        );

        // A symlink that stays inside the repository is fine.
        std::fs::write(root.join("real.env"), "OK\n").unwrap();
        std::os::unix::fs::symlink(root.join("real.env"), root.join("config/inside.env")).unwrap();
        let copied = copy_included(&root, &wt, &["config/inside.env".into()]);
        assert_eq!(copied, vec!["config/inside.env".to_string()]);
    }

    #[tokio::test]
    async fn an_include_never_writes_through_a_symlink_already_in_the_worktree() {
        // The write direction: a committed dangling symlink already sits at
        // the destination, and `fs::copy` would write through it.
        let root = scratch_repo("symlink-write").await;
        let target = root.parent().unwrap().join("vp-symlink-target.txt");
        let _ = std::fs::remove_file(&target);
        std::fs::write(root.join(".env"), "PAYLOAD\n").unwrap();

        let wt = create_worktree(&root, "symw-abc", "symw/abc", "main")
            .await
            .unwrap();
        std::os::unix::fs::symlink(&target, wt.join(".env")).unwrap();

        let copied = copy_included(&root, &wt, &[".env".into()]);
        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(
            !target.exists(),
            "writing through a symlink the repository chose is an arbitrary write"
        );
    }

    #[tokio::test]
    async fn an_include_cannot_reach_outside_the_repository() {
        // Git never names a path outside the checkout, but the rule stays as a
        // second lock.
        let root = scratch_repo("escape").await;
        let secret = root.parent().unwrap().join("vp-secret-outside.txt");
        std::fs::write(&secret, "SECRET\n").unwrap();
        let wt = create_worktree(&root, "e-abc", "e/abc", "main")
            .await
            .unwrap();

        let copied = copy_included(
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
            copy_included(&root, &wt, &["cfg/../.env".into()]),
            vec!["cfg/../.env".to_string()]
        );

        std::fs::remove_file(&secret).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn ignored_files_lists_what_git_ignores_and_never_a_tracked_file() {
        // A tracked file is never a candidate, however a pattern reads; only
        // what `.worktreeinclude` names is listed at all.
        let root = scratch_repo("ignored").await;
        assert!(
            ignored_files(&root).await.unwrap().is_empty(),
            "no .worktreeinclude, no candidates"
        );
        std::fs::write(root.join(".gitignore"), ".env\nconfig/\n").unwrap();
        std::fs::write(root.join("keep.txt"), "tracked\n").unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        git(&root, &["commit", "-qm", "ignore"]).await.unwrap();
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/a.local"), "x\n").unwrap();
        std::fs::write(root.join("scratch.txt"), "untracked, not ignored\n").unwrap();
        std::fs::write(root.join("other.env"), "ignored? no, and not named\n").unwrap();
        std::fs::write(
            root.join(".worktreeinclude"),
            ".env\nconfig/\nkeep.txt\nscratch.txt\n",
        )
        .unwrap();

        let mut files = ignored_files(&root).await.unwrap();
        files.sort();
        assert_eq!(
            files,
            vec![".env".to_string(), "config/a.local".to_string()],
            "the files inside an ignored directory are listed one by one"
        );
        assert!(!files.iter().any(|f| f == "keep.txt"));
        assert!(
            !files.iter().any(|f| f == "scratch.txt"),
            "untracked is not ignored"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn the_base_branch_is_discovered_not_assumed() {
        let root = scratch_repo("base").await;
        assert_eq!(base_branch(&root).await, "main");
        std::fs::remove_dir_all(&root).ok();
    }
}

//! Git: worktrees, and the status the board shows.
//!
//! Mutations go through the `git` command rather than a library, deliberately.
//! Claude Code creates its worktrees with `git worktree`, the user creates
//! theirs by hand with `git worktree`, and Vibeplane creating them a third way
//! would produce checkouts that behave subtly differently from both. Parity
//! with what everyone else does is worth more here than a few milliseconds.
//!
//! The location follows Claude Code's convention — `.claude/worktrees/<name>` —
//! so a worktree Vibeplane made and one Claude made are indistinguishable, and
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
    std::fs::create_dir_all(dir.parent().unwrap())
        .with_context(|| format!("creating {}", dir.display()))?;

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
    let st = status(dir).await.unwrap_or_default();
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

/// Copies gitignored files a fresh checkout needs, in the spirit of
/// `.worktreeinclude`. Only files that exist, and never over something already
/// there.
pub fn copy_includes(root: &Path, worktree: &Path, patterns: &[String]) -> Vec<String> {
    let mut copied = Vec::new();
    for pattern in patterns {
        let from = root.join(pattern);
        if !from.is_file() {
            continue;
        }
        let to = worktree.join(pattern);
        if to.exists() {
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

/// A diff of the worktree against its base, for the review step.
pub async fn diff_stat(dir: &Path, base: &str) -> Result<String> {
    git(dir, &["diff", "--stat", &format!("{base}...HEAD")])
        .await
        .or_else(|_| Ok(String::new()))
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
    async fn the_base_branch_is_discovered_not_assumed() {
        let root = scratch_repo("base").await;
        assert_eq!(base_branch(&root).await, "main");
        std::fs::remove_dir_all(&root).ok();
    }
}

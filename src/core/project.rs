//! Projects — a registered repository root and its configuration.

use crate::core::ids::ProjectId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A registered repository. It must be trusted before Devplane spawns an agent
/// in it: a headless run executes the repository's own hooks and MCP servers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root: PathBuf,
    /// Trusted projects may host driven and background runs; untrusted ones
    /// are observe-only.
    pub trusted: bool,
    /// Correlates OpenTelemetry's `vcs.repository.url.full` without a path lookup.
    pub repo_url: Option<String>,
    /// Discovered rather than registered: a session appeared in this directory.
    pub auto_discovered: bool,
}

impl Project {
    /// The `owner/name` slug a `claude-cli://open?repo=` link needs, from an
    /// `https://host/owner/name(.git)` or `git@host:owner/name(.git)` remote.
    pub fn repo_slug(&self) -> Option<String> {
        let url = self.repo_url.as_deref()?.trim_end_matches('/');
        let rest = match url.split_once("://") {
            // `https://github.com/acme/app` → drop the host.
            Some((_, after)) => after.split_once('/').map(|(_, p)| p)?,
            // `git@github.com:acme/app`
            None => url.split_once(':').map(|(_, p)| p)?,
        };
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        let (owner, name) = rest.split_once('/')?;
        // Anything deeper is not a slug; guessing would open the wrong thing.
        (!owner.is_empty() && !name.is_empty() && !name.contains('/'))
            .then(|| format!("{owner}/{name}"))
    }

    pub fn from_root(root: PathBuf) -> Self {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string());
        Self {
            id: ProjectId::from_path(&root),
            name,
            root,
            trusted: false,
            repo_url: None,
            auto_discovered: false,
        }
    }

    /// Whether a path belongs to this project, including worktrees under
    /// `.claude/worktrees/`.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}

/// The checkout a path sits in: the nearest `.git`, file or directory. See
/// [`main_checkout_for`] for which repository owns it.
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// Maps a worktree back to the repository that owns it:
/// `<repo>/.claude/worktrees/<name>` by its path, or a `git worktree add`
/// checkout by its verified `.git` file, without running git.
/// A worktree resolving to itself would escape the owner's `devplane.toml`.
pub fn main_checkout_for(path: &Path) -> Option<PathBuf> {
    // Both separators: on Windows the path arrives with backslashes.
    let s = path.to_string_lossy().replace('\\', "/");
    if let Some(idx) = s.find("/.claude/worktrees/") {
        return Some(PathBuf::from(&s[..idx]));
    }
    linked_worktree_owner(path)
}

/// The repository a linked worktree belongs to, read out of its `.git` file:
/// a bounded read of two small files, since this half may not spawn git.
fn linked_worktree_owner(start: &Path) -> Option<PathBuf> {
    let checkout = find_repo_root(start)?;
    let dot_git = checkout.join(".git");
    if dot_git.is_dir() {
        // The main checkout of a repository is not a worktree of anything.
        return None;
    }
    verified_owner(&dot_git)
}

/// The owner a linked worktree's `.git` file names, only when git agrees.
/// An agent can edit that file, so it is honoured only when the owner's
/// `.git/worktrees/<name>/gitdir` points back at this checkout; otherwise it
/// names nobody, and an agent cannot choose which rules govern it.
fn verified_owner(dot_git: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(dot_git).ok()?;
    let git_dir = PathBuf::from(
        text.lines()
            .find_map(|l| l.trim().strip_prefix("gitdir:"))?
            .trim(),
    );
    // `<repo>/.git/worktrees/<name>` → `<repo>`, checked segment by segment.
    let worktrees = git_dir.parent()?;
    if worktrees.file_name()? != "worktrees" || worktrees.parent()?.file_name()? != ".git" {
        return None;
    }
    let back = std::fs::read_to_string(git_dir.join("gitdir")).ok()?;
    let canonical = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if canonical(Path::new(back.trim())) != canonical(dot_git) {
        return None;
    }
    Some(worktrees.parent()?.parent()?.to_path_buf())
}

/// A path as the filesystem names it: its longest existing ancestor resolved
/// through every link, the not-yet-existing rest appended unchanged. One
/// repository is one project whatever spelling reached it (`/tmp` is
/// `/private/tmp` on macOS), including worktrees not created yet.
pub fn named(path: &Path) -> PathBuf {
    let mut tail = Vec::new();
    let mut cur = path;
    loop {
        if let Ok(real) = std::fs::canonicalize(cur) {
            return tail.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                cur = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The directory whose `devplane.toml` governs a path; the one resolver every
/// surface asks.
///
/// * `<root>/.claude/worktrees/<name>` is governed by `<root>`.
/// * A linked worktree elsewhere is governed by its owner when
///   [`verified_owner`] verifies; otherwise the walk continues upward, so a
///   forged `.git` file yields the machine-wide rules, never its own.
/// * Inside a repository the root wins over a nested `devplane.toml`; the
///   nearest file governs only a path with no repository above it.
pub fn governing_root(path: &Path) -> Option<PathBuf> {
    let resolved = named(path);
    let path = resolved.as_path();
    let text = path.to_string_lossy().replace('\\', "/");
    if let Some(idx) = text.find("/.claude/worktrees/") {
        return Some(PathBuf::from(&text[..idx]));
    }
    let mut config_root = None;
    let mut cur = Some(path);
    while let Some(d) = cur {
        let dot_git = d.join(".git");
        if dot_git.is_dir() {
            return Some(d.to_path_buf());
        }
        if dot_git.is_file() {
            if let Some(owner) = verified_owner(&dot_git) {
                return Some(owner);
            }
        } else if config_root.is_none() && d.join(crate::core::config::CONFIG_FILE).is_file() {
            config_root = Some(d.to_path_buf());
        }
        cur = d.parent();
    }
    config_root
}

/// Whether a path is a checkout that some other repository owns.
pub fn is_worktree(path: &Path) -> bool {
    main_checkout_for(path).is_some()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_repo_slug_is_read_from_either_remote_form() {
        let mut p = Project::from_root(PathBuf::from("/repo"));
        assert_eq!(p.repo_slug(), None, "no remote, no link");

        p.repo_url = Some("https://github.com/acme/payments".into());
        assert_eq!(p.repo_slug().as_deref(), Some("acme/payments"));

        p.repo_url = Some("https://github.com/acme/payments.git".into());
        assert_eq!(p.repo_slug().as_deref(), Some("acme/payments"));

        p.repo_url = Some("git@github.com:acme/payments.git".into());
        assert_eq!(p.repo_slug().as_deref(), Some("acme/payments"));

        p.repo_url = Some("https://gitlab.example.com/group/sub/app.git".into());
        assert_eq!(
            p.repo_slug(),
            None,
            "a nested path is not a slug, and guessing would open the wrong repository"
        );
    }

    use super::*;

    /// `<tmp>/main` with a linked worktree `<tmp>/feature`, laid out as
    /// `git worktree add ../feature` would, without running git.
    fn linked_worktree(tag: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("vp-wt-{tag}-{}", std::process::id()));
        let main = base.join("main");
        let feature = base.join("feature");
        std::fs::create_dir_all(main.join(".git").join("worktrees").join("feature")).unwrap();
        std::fs::create_dir_all(&feature).unwrap();
        std::fs::write(
            feature.join(".git"),
            format!("gitdir: {}/.git/worktrees/feature\n", main.display()),
        )
        .unwrap();
        // The back-reference git writes; without it the pointer is not believed.
        std::fs::write(
            main.join(".git/worktrees/feature/gitdir"),
            format!("{}\n", feature.join(".git").display()),
        )
        .unwrap();
        (main, feature)
    }

    #[test]
    fn a_linked_worktree_resolves_to_the_repository_that_owns_it() {
        let (main, feature) = linked_worktree("owner");
        assert_eq!(main_checkout_for(&feature).as_deref(), Some(main.as_path()));
        assert_eq!(governing_root(&feature).as_deref(), Some(main.as_path()));
        assert!(is_worktree(&feature));
        assert_eq!(
            ProjectId::from_path(&governing_root(&feature).unwrap()),
            ProjectId::from_path(&main),
            "one repository, not two"
        );
        std::fs::remove_dir_all(main.parent().unwrap()).ok();
    }

    #[test]
    fn a_subdirectory_of_a_linked_worktree_resolves_the_same_way() {
        let (main, feature) = linked_worktree("deep");
        let deep = feature.join("crates").join("thing");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(governing_root(&deep).as_deref(), Some(main.as_path()));
        std::fs::remove_dir_all(main.parent().unwrap()).ok();
    }

    #[test]
    fn a_main_checkout_is_not_a_worktree_of_anything() {
        let (main, _) = linked_worktree("main");
        assert_eq!(main_checkout_for(&main), None);
        assert!(!is_worktree(&main));
        // Canonicalized: the temporary directory is behind a link on macOS.
        let named = std::fs::canonicalize(&main).unwrap();
        assert_eq!(governing_root(&main).as_deref(), Some(named.as_path()));
        std::fs::remove_dir_all(main.parent().unwrap()).ok();
    }

    #[test]
    fn claude_codes_own_convention_still_resolves_without_touching_the_disk() {
        let wt = Path::new("/repos/api/.claude/worktrees/fix-login");
        assert_eq!(
            main_checkout_for(wt).as_deref(),
            Some(Path::new("/repos/api"))
        );
        assert!(is_worktree(wt));
    }

    #[test]
    fn a_repository_living_under_a_directory_called_worktrees_is_not_mistaken_for_one() {
        // A substring test would read `/home/me/worktrees/api/.git` as a link.
        let base = std::env::temp_dir().join(format!("vp-wt-plain-{}", std::process::id()));
        let repo = base.join("worktrees").join("api");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        assert_eq!(main_checkout_for(&repo), None);
        let named = std::fs::canonicalize(&repo).unwrap();
        assert_eq!(governing_root(&repo).as_deref(), Some(named.as_path()));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_path_not_yet_on_disk_is_named_under_its_real_ancestor() {
        // An existing repository and its not-yet-created worktree share a spelling.
        let base = std::env::temp_dir().join(format!("vp-named-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let real = std::fs::canonicalize(&base).unwrap();
        let later = base.join(".claude/worktrees/not-yet");
        assert_eq!(super::named(&later), real.join(".claude/worktrees/not-yet"));
        assert_eq!(super::named(&base), real);
        std::fs::remove_dir_all(&base).ok();
    }
}

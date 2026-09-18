//! Projects — a registered repository root and its configuration.

use crate::core::ids::ProjectId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A registered repository.
///
/// Registration is deliberate: a project must be trusted before Devplane will
/// ever spawn an agent in it, because a headless Claude run executes the
/// repository's own hooks and MCP servers with no dialog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root: PathBuf,
    /// Trusted projects may host driven and background runs. Untrusted ones are
    /// observe-only, which is still the whole of M0.
    pub trusted: bool,
    /// Repository URL, when the remote is known. Used to correlate
    /// OpenTelemetry's `vcs.repository.url.full` to a project without a path
    /// lookup.
    pub repo_url: Option<String>,
    /// Discovered rather than registered: a session appeared in this directory.
    pub auto_discovered: bool,
}

impl Project {
    /// The `owner/name` slug, where the remote is a recognisable forge URL.
    ///
    /// What a `claude-cli://open?repo=` link needs: the slug resolves to
    /// whichever clone the person clicking actually has, which is the whole
    /// reason a link is worth more than a path. Handles both the
    /// `https://host/owner/name(.git)` and `git@host:owner/name(.git)` forms.
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
        // Anything deeper is not a slug, and guessing at one would build a
        // link that opens the wrong thing.
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

    /// Whether a path belongs to this project. A worktree under
    /// `.claude/worktrees/` counts as the project it was created from, which is
    /// what makes five worktrees of one repository one row on the board.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}

/// Finds the checkout a path sits in by walking up to the nearest `.git`.
///
/// A linked worktree has a `.git` *file* rather than a directory, so both are
/// accepted: this answers "which checkout", and [`main_checkout_for`] answers
/// "which repository owns it".
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

/// Maps a worktree back to the repository that owns it.
///
/// Two conventions, because there are two ways a worktree gets made and a
/// human does not think of them as different things:
///
/// * Claude Code's — and Devplane's — `<repo>/.claude/worktrees/<name>`, which
///   is a pure string question;
/// * anything `git worktree add` produced, which can be anywhere on the disk.
///   Its `.git` is a *file* reading `gitdir: <repo>/.git/worktrees/<name>`, so
///   the owner is recoverable without running git — which matters, because the
///   answer is needed on the synchronous hook a session is blocked on.
///
/// Getting the second one wrong is not cosmetic. The repository's
/// `devplane.toml` is found from this, so a worktree that resolves to itself
/// is a worktree where the project's `never_auto` rules and its gates silently
/// do not apply.
pub fn main_checkout_for(path: &Path) -> Option<PathBuf> {
    // Both separators, because this is the one string comparison in the
    // product that decides whether five checkouts of a repository are five
    // projects or one, and on Windows the path arrives with backslashes.
    let s = path.to_string_lossy().replace('\\', "/");
    if let Some(idx) = s.find("/.claude/worktrees/") {
        return Some(PathBuf::from(&s[..idx]));
    }
    linked_worktree_owner(path)
}

/// The repository a linked worktree belongs to, read out of its `.git` file.
///
/// A bounded read of one small local file — the same exception `config` and
/// `policy_cache` take, and for the same reason: it is the only way to answer
/// the question without spawning git, which this half may not do.
fn linked_worktree_owner(start: &Path) -> Option<PathBuf> {
    let checkout = find_repo_root(start)?;
    let dot_git = checkout.join(".git");
    if dot_git.is_dir() {
        // The main checkout of a repository is not a worktree of anything.
        return None;
    }
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let target = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("gitdir:"))?;
    let git_dir = PathBuf::from(target.trim());

    // `<repo>/.git/worktrees/<name>` → `<repo>`. Checked segment by segment
    // rather than by trimming a substring, so a repository that happens to live
    // in a directory called `worktrees` is not mistaken for one.
    let name_parent = git_dir.parent()?;
    if name_parent.file_name()? != "worktrees" {
        return None;
    }
    let common = name_parent.parent()?;
    if common.file_name()? != ".git" {
        return None;
    }
    Some(common.parent()?.to_path_buf())
}

/// The directory whose `devplane.toml` governs a path: the repository that
/// owns it, or the checkout itself when it owns nothing.
///
/// One function, because five call sites spelling out the same `or_else` chain
/// is five places for the worktree case to be forgotten in — and it was.
///
/// **The last clause is a fallback and not a third convention.** A directory
/// with no git above it used to have no rules at all, however plainly a
/// `devplane.toml` was sitting in it — so `devplane check` read the file and
/// printed its rules while `devplane explain`, in the same directory, answered
/// `undecided` and never mentioned it. Two commands disagreeing about one file
/// is worse than either answer.
///
/// Inside a repository the root still wins, even if a nested directory carries
/// its own `devplane.toml`: making the *nearest* file win would silently move
/// authority for every existing checkout, in a direction nobody could predict
/// from the outside. This only reaches a path that has no repository above it.
pub fn governing_root(path: &Path) -> Option<PathBuf> {
    main_checkout_for(path)
        .or_else(|| find_repo_root(path))
        .or_else(|| find_config_root(path))
}

/// The nearest ancestor holding a `devplane.toml`, for a path with no
/// repository above it.
fn find_config_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join(crate::core::config::CONFIG_FILE).is_file() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
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

    /// Builds `<tmp>/main` with a linked worktree `<tmp>/feature` beside it,
    /// the way `git worktree add ../feature` leaves them — without running git,
    /// so the test says what the layout is rather than trusting a version of it.
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
        (main, feature)
    }

    #[test]
    fn a_linked_worktree_resolves_to_the_repository_that_owns_it() {
        // `git worktree add` is how most worktrees on a real machine were made,
        // and its output can be anywhere on the disk — there is no string in
        // the path to recognise. Before this, such a checkout resolved to
        // itself: its own project row on the board, its own trust decision, and
        // a `devplane.toml` at the real root whose `never_auto` rules and
        // gates silently did not apply to work happening inside it.
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
        // It still governs itself.
        assert_eq!(governing_root(&main).as_deref(), Some(main.as_path()));
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
        // The `gitdir:` tail is checked segment by segment for this reason: a
        // substring test would read `/home/me/worktrees/api/.git` as a link.
        let base = std::env::temp_dir().join(format!("vp-wt-plain-{}", std::process::id()));
        let repo = base.join("worktrees").join("api");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        assert_eq!(main_checkout_for(&repo), None);
        assert_eq!(governing_root(&repo).as_deref(), Some(repo.as_path()));
        std::fs::remove_dir_all(&base).ok();
    }
}

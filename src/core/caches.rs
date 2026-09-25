//! Build caches shared between a project's changes, by ecosystem. Sharing is
//! declared (`[workspace] share = ["cargo"]`), never inferred, and redirects to a
//! Devplane-owned directory, never the person's own cache. Only cargo has a row:
//! it is the common ecosystem whose cache lives inside the tree; others already
//! share through a home-directory store. Each row says why sharing is safe and
//! when that claim about the vendor was last checked.

use std::path::{Path, PathBuf};

/// Relative to the repository root; under the vendor's worktree directory so
/// one `.gitignore` line covers both.
pub const SHARED_DIR: &str = ".claude/worktrees/.shared";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ecosystem {
    /// The value written in `share = [..]`.
    pub name: &'static str,
    /// The variable that redirects its cache.
    pub env: &'static str,
    /// The directory under [`SHARED_DIR`].
    pub dir: &'static str,
    /// Why two builds in one directory are safe.
    pub because: &'static str,
    /// When `because` was last observed.
    pub checked: &'static str,
}

pub const ECOSYSTEMS: &[Ecosystem] = &[Ecosystem {
    name: "cargo",
    env: "CARGO_TARGET_DIR",
    dir: "cargo",
    because: "cargo takes a file lock on the target directory and a second build waits: two \
              concurrent `cargo build` in one CARGO_TARGET_DIR printed `Blocking waiting for \
              file lock on build directory` for the second",
    checked: "2026-09-25",
}];

/// `None` is a config problem, not a guess.
pub fn lookup(name: &str) -> Option<&'static Ecosystem> {
    ECOSYSTEMS.iter().find(|e| e.name == name)
}

pub fn known_names() -> Vec<&'static str> {
    ECOSYSTEMS.iter().map(|e| e.name).collect()
}

/// One `(variable, directory)` per declared ecosystem. Unknown names are
/// skipped; the configuration already refused them.
pub fn env_for(root: &Path, share: &[String]) -> Vec<(String, PathBuf)> {
    share
        .iter()
        .filter_map(|name| lookup(name))
        .map(|e| (e.env.to_string(), root.join(SHARED_DIR).join(e.dir)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_ecosystem_is_none_rather_than_a_guess() {
        assert!(lookup("npm").is_none());
        assert!(lookup("").is_none());
        assert_eq!(lookup("cargo").map(|e| e.env), Some("CARGO_TARGET_DIR"));
    }

    #[test]
    fn a_declared_cargo_share_is_one_variable_under_the_shared_directory() {
        let root = Path::new("/repo");
        let env = env_for(root, &["cargo".to_string()]);
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, "CARGO_TARGET_DIR");
        assert_eq!(
            env[0].1,
            Path::new("/repo/.claude/worktrees/.shared/cargo"),
            "the cache is Devplane's own, never the person's target/"
        );
    }

    #[test]
    fn nothing_declared_means_nothing_set() {
        assert!(env_for(Path::new("/repo"), &[]).is_empty());
        assert!(env_for(Path::new("/repo"), &["npm".to_string()]).is_empty());
    }

    #[test]
    fn every_row_says_why_and_when() {
        for e in ECOSYSTEMS {
            assert!(!e.because.is_empty(), "{} has no reason", e.name);
            assert_eq!(e.checked.len(), 10, "{} has no date", e.name);
            assert!(known_names().contains(&e.name));
        }
    }
}

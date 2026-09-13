//! Projects — a registered repository root and its configuration.

use crate::ids::ProjectId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A registered repository.
///
/// Registration is deliberate: a project must be trusted before Vibeplane will
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

/// Finds the repository root for a path by walking up to the nearest `.git`.
/// A linked worktree has a `.git` *file* rather than a directory, so both are
/// accepted and the caller resolves the main checkout separately.
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

/// Maps a worktree path created by Claude Code back to the repository that owns
/// it: `<repo>/.claude/worktrees/<name>` → `<repo>`.
pub fn main_checkout_for(path: &Path) -> Option<PathBuf> {
    let s = path.to_string_lossy();
    let idx = s.find("/.claude/worktrees/")?;
    Some(PathBuf::from(&s[..idx]))
}

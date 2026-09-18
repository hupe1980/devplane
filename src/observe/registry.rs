//! The local session registry: `~/.claude/sessions` and `~/.claude/ide`.
//!
//! **These files are Claude Code internals and are not documented.** They are
//! therefore enrichment and never a dependency: everything read here is
//! optional, every field is treated as missing-by-default, and the product
//! degrades to what the documented channels provide if the shape changes or the
//! directories disappear.
//!
//! What they add is worth that care:
//!
//! * `sessions/<pid>.json` names the **entrypoint** of a session — whether it
//!   is running in a terminal, the VS Code extension or the desktop app — which
//!   otherwise only arrives with OpenTelemetry.
//! * `ide/<pid>.lock` maps a workspace folder to the **editor window** that has
//!   it open. That is what turns "this session needs you" into "here is the
//!   window", which is the only useful action for a session Devplane cannot
//!   type into.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One entry of the session registry.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SessionEntry {
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// `cli`, `claude-vscode`, `sdk-ts`, … — the same vocabulary as
    /// OpenTelemetry's `app.entrypoint`.
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

impl SessionEntry {
    /// Whether this entry can be correlated to a run at all.
    ///
    /// Serde will happily build a struct of defaults out of a shape that has
    /// nothing to do with a session — an empty array, for instance — so an
    /// entry earns its place by carrying the one field everything else keys on.
    pub fn is_usable(&self) -> bool {
        self.session_id.is_some()
    }
}

/// An editor window with a workspace open.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct IdeWindow {
    /// The editor's process id — what a focus request needs.
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default, rename = "ideName")]
    pub ide_name: Option<String>,
    #[serde(default, rename = "workspaceFolders")]
    pub workspace_folders: Vec<PathBuf>,
}

/// The Claude Code configuration directory.
pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".claude")))
}

/// Reads every session entry. Unreadable or unexpected files are skipped: one
/// malformed entry must not cost the whole enrichment.
pub fn sessions() -> Vec<SessionEntry> {
    let Some(dir) = config_dir().map(|d| d.join("sessions")) else {
        return Vec::new();
    };
    read_json_dir::<SessionEntry>(&dir, "json")
        .into_iter()
        .filter(SessionEntry::is_usable)
        .collect()
}

/// Reads every editor window lock.
pub fn ide_windows() -> Vec<IdeWindow> {
    let Some(dir) = config_dir().map(|d| d.join("ide")) else {
        return Vec::new();
    };
    read_json_dir(&dir, "lock")
}

fn read_json_dir<T: for<'de> Deserialize<'de>>(dir: &Path, ext: &str) -> Vec<T> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some(ext))
        .filter_map(|p| std::fs::read_to_string(&p).ok())
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect()
}

/// Finds the editor window that has `path` open.
///
/// The longest matching workspace folder wins, so a session inside a worktree
/// resolves to the window holding that worktree rather than to an unrelated
/// parent that happens to share a prefix.
pub fn window_for_path(windows: &[IdeWindow], path: &Path) -> Option<IdeWindow> {
    windows
        .iter()
        .filter_map(|w| {
            w.workspace_folders
                .iter()
                .filter(|f| path.starts_with(f))
                .map(|f| f.components().count())
                .max()
                .map(|depth| (depth, w))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, w)| w.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_session_entry_parses() {
        // Captured from a developer machine running Claude Code 2.1.269.
        let e: SessionEntry = serde_json::from_str(
            r#"{"pid":10274,"sessionId":"fd247e6f-b3fa-4711-b3d0-83689154449e",
                "cwd":"/Users/x/matter-kit","startedAt":1789290021018,
                "version":"2.1.269","peerProtocol":1,"kind":"interactive",
                "entrypoint":"claude-vscode","name":"matter-kit-f7","status":"busy"}"#,
        )
        .unwrap();
        assert_eq!(e.entrypoint.as_deref(), Some("claude-vscode"));
        assert_eq!(e.status.as_deref(), Some("busy"));
        assert_eq!(e.pid, Some(10274));
    }

    #[test]
    fn an_unrecognisable_entry_is_discarded_not_trusted() {
        // These files are internal, so a release that changes them must cost
        // the enrichment and nothing else. Serde builds a struct of defaults
        // out of almost anything — including an empty array — so usefulness is
        // checked rather than assumed.
        let e: SessionEntry = serde_json::from_str(r#"{"somethingElse": 1}"#).unwrap();
        assert!(!e.is_usable());
        let e: SessionEntry = serde_json::from_str("[]").unwrap();
        assert!(!e.is_usable(), "an empty sequence is not a session");
    }

    #[test]
    fn a_real_ide_lock_parses() {
        let w: IdeWindow = serde_json::from_str(
            r#"{"pid":1310,"workspaceFolders":["/Users/x/chronix"],
                "ideName":"Visual Studio Code","transport":"ws",
                "authToken":"4a0f6488-130b-461a-b340-753ac4554230"}"#,
        )
        .unwrap();
        assert_eq!(w.pid, Some(1310));
        assert_eq!(w.ide_name.as_deref(), Some("Visual Studio Code"));
    }

    #[test]
    fn the_deepest_workspace_wins() {
        // One editor can hold the repository and another a worktree inside it.
        // Focusing the parent would raise the wrong window.
        let windows = vec![
            IdeWindow {
                pid: Some(1),
                ide_name: Some("Visual Studio Code".into()),
                workspace_folders: vec!["/repo".into()],
            },
            IdeWindow {
                pid: Some(2),
                ide_name: Some("Visual Studio Code".into()),
                workspace_folders: vec!["/repo/.claude/worktrees/feature".into()],
            },
        ];
        let w =
            window_for_path(&windows, Path::new("/repo/.claude/worktrees/feature/src")).unwrap();
        assert_eq!(w.pid, Some(2));
        let w = window_for_path(&windows, Path::new("/repo/src")).unwrap();
        assert_eq!(w.pid, Some(1));
    }

    #[test]
    fn an_unopened_path_matches_no_window() {
        assert!(window_for_path(&[], Path::new("/repo")).is_none());
    }
}

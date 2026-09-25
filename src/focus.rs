//! Raising the window that owns a session.
//!
//! No documented channel names a window (the `~/.claude` editor and session
//! files are vendor internals), so this always answers [`Focused::Nothing`] and
//! the surfaces offer the resume command instead.

use anyhow::Result;
use std::path::Path;

/// What was raised, so the caller can say so rather than claiming success.
#[derive(Debug, Clone, PartialEq)]
pub enum Focused {
    /// An editor window with the session's directory open.
    Editor { app: String, pid: u32 },
    /// Nothing could be identified; the caller should offer the resume command.
    Nothing,
}

/// Raises the editor window whose workspace contains `path`; nothing
/// documented identifies it, so nothing is raised.
pub fn focus_path(_path: &Path) -> Result<Focused> {
    Ok(Focused::Nothing)
}

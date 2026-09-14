//! Raising the window that owns a session.
//!
//! For a session Vibeplane did not start, this is the only honest action. The
//! product cannot type an answer into somebody else's terminal, so it does the
//! next best thing: it puts the human in front of the session that is asking,
//! instead of leaving them to find which of twenty windows it was.

use anyhow::{Result, bail};
use std::path::Path;

/// What was raised, so the caller can say so rather than claiming success
/// blindly.
#[derive(Debug, Clone, PartialEq)]
pub enum Focused {
    /// An editor window with the session's directory open.
    Editor { app: String, pid: u32 },
    /// Nothing could be identified; the caller should offer the resume command.
    Nothing,
}

/// Raises the editor window whose workspace contains `path`.
pub fn focus_path(path: &Path) -> Result<Focused> {
    let windows = crate::observe::registry::ide_windows();
    let Some(window) = crate::observe::registry::window_for_path(&windows, path) else {
        return Ok(Focused::Nothing);
    };
    let (Some(pid), app) = (
        window.pid,
        window
            .ide_name
            .clone()
            .unwrap_or_else(|| "the editor".to_string()),
    ) else {
        return Ok(Focused::Nothing);
    };

    raise(pid)?;
    Ok(Focused::Editor { app, pid })
}

#[cfg(target_os = "macos")]
fn raise(pid: u32) -> Result<()> {
    use anyhow::Context;
    // Raising by process id rather than by application name: several windows of
    // the same editor are several processes, and the one holding this session
    // is the only right answer.
    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true"
        ))
        .status()
        .context("running osascript")?;
    if !status.success() {
        bail!(
            "could not raise the window (macOS may need Accessibility permission for your terminal)"
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn raise(pid: u32) -> Result<()> {
    // `wmctrl` and `xdotool` are the two that are usually installed; neither is
    // guaranteed, so the failure has to name what is missing.
    for (bin, args) in [
        (
            "wmctrl",
            vec!["-i".to_string(), "-a".to_string(), pid.to_string()],
        ),
        (
            "xdotool",
            vec!["windowactivate".to_string(), pid.to_string()],
        ),
    ] {
        if let Ok(status) = std::process::Command::new(bin).args(&args).status()
            && status.success()
        {
            return Ok(());
        }
    }
    bail!("install wmctrl or xdotool to focus windows")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn raise(_pid: u32) -> Result<()> {
    bail!("focusing a window is not implemented on this platform yet")
}

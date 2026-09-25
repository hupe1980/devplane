//! Quitting: say what it stops, then stop it, the way `devplane quit` does.
//!
//! The answer window's `quit` surface reads `/api/quitting`, prints the CLI's
//! sentence, and offers Quit (posts `/api/quit`) or Keep running. `/api/quit`
//! flips the stop flag; `host::serve` stops driven runs, removes `host.json`
//! and returns, and the app exits with its verdict.

use super::window;
use tauri::AppHandle;

/// Raises the quit question. Nothing stops until the person says so.
pub fn ask(app: &AppHandle) {
    window::raise_answer(app, "surface=quit");
}

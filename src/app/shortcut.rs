//! The one global shortcut.
//!
//! One, because every global key is taken from something else; configurable
//! as `[app] shortcut` in `~/.devplane/app.toml`. A press toggles the answer
//! window on the topmost waiting item.

use super::window;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Parses a combo in the plugin's notation — `CmdOrCtrl+Shift+Space`.
pub fn validate(combo: &str) -> Result<Shortcut, String> {
    combo
        .parse::<Shortcut>()
        .map_err(|e| format!("`{combo}` is not a shortcut: {e}"))
}

/// The plugin, with the one handler: a press toggles the answer window.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if matches!(event.state(), ShortcutState::Pressed) {
                toggle(app);
            }
        })
        .build()
}

fn toggle(app: &AppHandle) {
    if window::answer_shown(app) {
        window::hide_answer(app);
    } else {
        window::raise_answer(app, "surface=answer");
    }
}

/// Registers `combo`, or says why not. An invalid combo registers nothing.
pub fn register(app: &AppHandle, combo: &str) -> Result<(), String> {
    let shortcut = validate(combo)?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("`{combo}` could not be registered: {e}"))
}

/// Swaps a changed setting in without a restart: the old combo is released
/// whatever happens to the new one.
pub fn reregister(app: &AppHandle, old: &str, new: &str) -> Result<(), String> {
    if let Ok(s) = validate(old) {
        let _ = app.global_shortcut().unregister(s);
    }
    register(app, new)
}

/// Re-reads `app.toml` every few seconds and swaps the shortcut when it
/// changed. A file that stops parsing keeps the registered shortcut.
pub async fn watch(app: AppHandle) {
    let mut said = false;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let Ok((cfg, problems)) = crate::config::app_config() else {
            continue;
        };
        if !problems.is_empty() {
            if !said {
                for p in &problems {
                    tracing::warn!(at = %p.where_, "{}", p.what);
                }
                said = true;
            }
            continue;
        }
        said = false;
        let hosted = app.state::<super::Hosted>();
        let old = hosted.shortcut.lock().expect("shortcut").clone();
        if cfg.shortcut == old {
            continue;
        }
        match reregister(&app, &old, &cfg.shortcut) {
            Ok(()) => {
                tracing::info!(from = old, to = cfg.shortcut, "shortcut changed");
                *hosted.shortcut.lock().expect("shortcut") = cfg.shortcut;
            }
            Err(why) => tracing::warn!("{why}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_parses_and_a_dangling_modifier_does_not() {
        assert!(validate(&crate::config::AppConfig::default().shortcut).is_ok());
        assert!(validate("Alt+Space").is_ok());
        let err = validate("Ctrl+").unwrap_err();
        assert!(err.contains("Ctrl+"), "{err}");
        assert!(validate("").is_err());
    }
}

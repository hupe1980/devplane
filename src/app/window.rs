//! The two windows, both on the host's own page.
//!
//! `main` is the page `devplane open` shows, framed; closing hides it and the
//! host keeps running. `answer` is small and on top: the shortcut raises it on
//! the topmost waiting item, and `Esc`, an answer or Keep running hides it and
//! returns focus.

use super::Hosted;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
pub const ANSWER: &str = "answer";

/// The pseudo-URL the answer page navigates to when it wants to hide; the
/// navigation is cancelled here. Asking by navigation needs no IPC.
const HIDE: &str = "devplane-app://hide";

fn page(app: &AppHandle, query: &str) -> tauri::Result<tauri::Url> {
    app.state::<Hosted>()
        .page(query)
        .parse()
        .map_err(tauri::Error::InvalidUrl)
}

/// The main window, on the inbox or on whatever `query` names.
pub fn open_main(app: &AppHandle, query: &str) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::External(page(app, query)?))
        .title("Devplane")
        .inner_size(1200.0, 800.0)
        .build()?;
    // Hidden, not destroyed: the host is still the host with the window gone.
    let hidden = w.clone();
    w.on_window_event(move |e| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
            api.prevent_close();
            let _ = hidden.hide();
        }
    });
    Ok(w)
}

/// Shows the main window on `query` — `""` for the inbox — and focuses it.
pub fn show_main(app: &AppHandle, query: &str) {
    let Some(w) = app.get_webview_window(MAIN) else {
        if let Err(e) = open_main(app, query) {
            tracing::error!(error = %e, "could not reopen the window");
        }
        return;
    };
    match page(app, query) {
        Ok(url) => {
            if let Err(e) = w.navigate(url) {
                tracing::warn!(error = %e, "could not navigate the window");
            }
        }
        Err(e) => tracing::warn!(error = %e, "bad page address"),
    }
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
}

/// Builds the answer window once, hidden. `raise_answer` shows it.
pub fn open_answer(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(ANSWER) {
        return Ok(w);
    }
    let hider = app.clone();
    WebviewWindowBuilder::new(
        app,
        ANSWER,
        WebviewUrl::External(page(app, "surface=answer")?),
    )
    .title("Devplane")
    .inner_size(480.0, 320.0)
    .always_on_top(true)
    .decorations(true)
    .visible(false)
    // The page calls this on `Esc` and after *Keep running*; in a browser the
    // function is absent and the key does nothing.
    .initialization_script(format!(
        "window.__devplane_hide = function () {{ location.assign({HIDE:?}); }};"
    ))
    .on_navigation(move |url| {
        if url.as_str() == HIDE {
            // Deferred: this runs inside the webview's own navigation
            // callback, which is not a place to tear the window down from.
            let app = hider.clone();
            let _ = hider.run_on_main_thread(move || hide_answer(&app));
            return false;
        }
        true
    })
    .build()
}

/// Shows the answer window on `query` — `surface=answer` for the shortcut,
/// `surface=quit` for the quit question — and focuses it.
pub fn raise_answer(app: &AppHandle, query: &str) {
    let w = match app.get_webview_window(ANSWER) {
        Some(w) => w,
        None => match open_answer(app) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "could not build the answer window");
                return;
            }
        },
    };
    match page(app, query) {
        Ok(url) => {
            if let Err(e) = w.navigate(url) {
                tracing::warn!(error = %e, "could not navigate the answer window");
            }
        }
        Err(e) => tracing::warn!(error = %e, "bad page address"),
    }
    let _ = w.show();
    let _ = w.set_focus();
}

/// Hides the answer window and yields focus: on macOS the app steps back so
/// the previous application is front again.
pub fn hide_answer(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(ANSWER) {
        let _ = w.hide();
    }
    #[cfg(target_os = "macos")]
    {
        // Only when nothing else of ours is on screen: hiding the whole app
        // with the main window open would take that away too.
        let main_shown = app
            .get_webview_window(MAIN)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);
        if !main_shown {
            let _ = app.hide();
        }
    }
}

/// Whether the answer window is on screen.
pub fn answer_shown(app: &AppHandle) -> bool {
    app.get_webview_window(ANSWER)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

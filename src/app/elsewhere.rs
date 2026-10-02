//! The second start: the app opened (from Finder, the Start menu, a launcher)
//! while a host it does not own already serves this home — `devplane up
//! --no-browser` in a terminal, say. A process with no terminal cannot say so
//! on stderr, so it says it in a small window on the bundled page, offers to
//! open that host's page in the browser, and quits when the window closes. It
//! starts nothing and owns nothing.

use anyhow::{Context, Result};
use tauri::{WebviewUrl, WebviewWindowBuilder};

/// Shows the *already serving* window until it is closed.
pub fn show(port: u16) -> Result<()> {
    let page = crate::client::Client::connect()
        .map(|c| format!("{}/?token={}", c.base_url(), c.token()))
        .unwrap_or_else(|_| format!("http://127.0.0.1:{port}/"));
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            let page = page.clone();
            WebviewWindowBuilder::new(
                app,
                "elsewhere",
                WebviewUrl::App(format!("index.html?elsewhere={port}").into()),
            )
            .title("Devplane")
            .inner_size(520.0, 220.0)
            .resizable(false)
            // The page's one link is the running host's: it opens in the
            // browser, with the token, and this window never leaves the
            // bundled page.
            .on_navigation(move |url| {
                if url.scheme() == "http" && url.port() == Some(port) {
                    use tauri_plugin_opener::OpenerExt;
                    if let Err(e) = handle.opener().open_url(&page, None::<&str>) {
                        tracing::warn!(error = %e, "could not open the running host's page");
                    }
                    return false;
                }
                true
            })
            .build()?;
            Ok(())
        })
        .build(super::context())
        .context("building the window")?;
    // The default: the app exits when its last window closes.
    app.run(|_, _| {});
    Ok(())
}

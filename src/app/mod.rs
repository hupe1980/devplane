//! The app: the host with a window.
//!
//! `devplane app` runs the same host as `devplane serve`, in-process, and
//! opens a window on the page it serves; nothing crosses Tauri's IPC on the
//! data path. Tauri supplies only native pieces: notifications, the tray
//! count and dock badge, one global shortcut, deep links, and opening a
//! worktree. Closing the window keeps hosting; quitting tears down like
//! `devplane quit`. Nothing is installed and nothing starts at login.

pub mod links;
pub mod quit;
pub mod shortcut;
pub mod tray;
pub mod window;

use crate::host::Shared;
use anyhow::{Context, Result};
use tauri::Manager;

/// What the app holds beside the host, reachable from every callback.
pub struct Hosted {
    pub state: Shared,
    /// `http://127.0.0.1:<port>`, once the host has bound.
    pub base: String,
    pub token: String,
    pub tray: std::sync::Mutex<Option<tauri::tray::TrayIcon>>,
    pub shortcut: std::sync::Mutex<String>,
}

impl Hosted {
    /// The page, with the token and a query the shell reads: `surface=…`,
    /// `change=…`, `ask=…`, `run=…`, `link=…`.
    pub fn page(&self, query: &str) -> String {
        let mut url = format!("{}/?token={}", self.base, self.token);
        if !query.is_empty() {
            url.push('&');
            url.push_str(query);
        }
        url
    }
}

/// Runs the app until it quits. Refuses, like `serve`, when a host answers.
pub async fn run(port: Option<u16>) -> Result<()> {
    crate::cli::init_tracing();
    // Held for the app's whole life, like `serve`'s.
    let _lock = crate::cli::refuse_if_hosting().await?;
    let (cfg, problems) = crate::config::app_config()?;
    for p in &problems {
        tracing::warn!(at = %p.where_, "{}", p.what);
    }
    let port = port.unwrap_or(cfg.port);

    let state = crate::cli::boot_state().await?;
    crate::cli::drain_decision_spool(&state).await;
    crate::poller::reconcile_at_startup(&state).await;
    crate::poller::initial_poll(&state).await;

    // The host runs on the runtime's workers; this thread becomes the event
    // loop. The port is read back from `host.json`, like any client.
    let serving = tokio::spawn(crate::host::serve(state.clone(), port));
    let base = wait_for_host(&serving).await?;
    let hosted = Hosted {
        state: state.clone(),
        base,
        token: state.token.clone(),
        tray: std::sync::Mutex::new(None),
        shortcut: std::sync::Mutex::new(cfg.shortcut.clone()),
    };

    let app = tauri::Builder::default()
        // First, so a second launch hands its URL over before anything else.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            links::from_argv(app, &argv);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(shortcut::plugin())
        .manage(hosted)
        .setup(move |app| {
            setup(app.handle(), &cfg.shortcut);
            Ok(())
        })
        .build(tauri::generate_context!())
        .context("building the window")?;

    // When the host finishes (`/api/quit`, a signal, a failure) the app
    // exits with its verdict; teardown is `serve`'s.
    let handle = app.handle().clone();
    tokio::spawn(async move {
        let code = match serving.await {
            Ok(Ok(())) => 0,
            Ok(Err(e)) => {
                tracing::error!(error = %e, "the host stopped");
                1
            }
            Err(e) => {
                tracing::error!(error = %e, "the host's task ended");
                1
            }
        };
        handle.exit(code);
    });

    app.run(|app, event| {
        // ⌘Q, the tray's Quit, the dock: the person is asked first. A
        // programmatic exit carries a code and passes.
        if let tauri::RunEvent::ExitRequested {
            api, code: None, ..
        } = &event
        {
            api.prevent_exit();
            quit::ask(app);
        }
    });
    Ok(())
}

/// Blocks until `host.json` names this process, or the host gave up.
async fn wait_for_host(serving: &tokio::task::JoinHandle<Result<()>>) -> Result<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(info) = crate::config::read_host()?
            && info.pid == std::process::id()
        {
            return Ok(info.base_url());
        }
        if serving.is_finished() {
            anyhow::bail!("the host did not start; see the log above");
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("the host did not publish its port within ten seconds");
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// Everything the window needs once the host is up, in the order it needs it.
fn setup(app: &tauri::AppHandle, shortcut: &str) {
    let hosted = app.state::<Hosted>();
    // Notifications go through the app from here on, not `osascript`.
    crate::notify::choose(crate::notify::Sink::Tauri(app.clone()));

    // A link the app was started with lands before the window shows.
    let first = links::startup_query(app);
    if let Err(e) = window::open_main(app, &first) {
        tracing::error!(error = %e, "could not open the window");
    }
    if let Err(e) = window::open_answer(app) {
        tracing::error!(error = %e, "could not build the answer window");
    }
    match tray::build(app) {
        Ok(t) => *hosted.tray.lock().expect("tray") = Some(t),
        Err(e) => tracing::error!(error = %e, "could not build the tray item"),
    }
    if let Err(why) = shortcut::register(app, shortcut) {
        tracing::warn!(shortcut, "{why}");
    }
    links::install(app);

    // Opening a path is the app's; without it the route answers with the
    // path alone.
    let opener = app.clone();
    let _ = hosted.state.opener.set(Box::new(move |place, path| {
        links::open_in(&opener, place, path)
    }));

    tokio::spawn(count_loop(app.clone(), hosted.state.clone()));
    tokio::spawn(shortcut::watch(app.clone()));
}

/// Keeps the tray's number and the badge at the inbox's *needs you* count.
///
/// Recomputed from the host's inbox at most four times a second, so a burst
/// of events is one update.
async fn count_loop(app: tauri::AppHandle, state: Shared) {
    let mut rx = state.tx.subscribe();
    let apply = |items: Vec<crate::core::AttentionItem>| {
        tray::apply(&app, tray::needs_you(&items));
    };
    apply(state.current_inbox().await);
    loop {
        match rx.recv().await {
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        while rx.try_recv().is_ok() {}
        apply(state.current_inbox().await);
    }
}

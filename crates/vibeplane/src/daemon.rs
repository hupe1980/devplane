//! The daemon: receivers, API and the authoritative state.
//!
//! Everything lives in one process and one binary. The receivers must be
//! running whether or not a window is open — a hook that arrives while the UI
//! is closed is exactly the observation that matters — so the daemon is the
//! product and every other surface is a client of it.

use anyhow::{Context, Result};
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use vibeplane_core::{Policy, World};
use vibeplane_domain::event::{Event, EventEnvelope, Source};
use vibeplane_domain::ids::RunId;
use vibeplane_domain::run::RunMode;
use vibeplane_store::Store;

/// Shared daemon state.
pub struct AppState {
    pub world: Mutex<World>,
    /// Live ACP sessions, by run. Only driven runs appear here; an observed
    /// session belongs to whoever started it.
    pub sessions: Mutex<std::collections::HashMap<RunId, vibeplane_acp::Session>>,
    pub store: Store,
    pub policy: Mutex<Policy>,
    pub token: String,
    /// Broadcasts every applied event, so the UI and `vibeplane watch` see
    /// changes without polling.
    pub tx: broadcast::Sender<EventEnvelope>,
    pub started_at: jiff::Timestamp,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub async fn new(db: PathBuf, token: String, policy: Policy) -> Result<Shared> {
        let store = Store::open(&db)
            .await
            .with_context(|| format!("opening {}", db.display()))?;
        let (tx, _) = broadcast::channel(1024);

        let mut world = World::new();
        // Come up with what we knew, then correct it against the machine.
        // Loading is not believing: a run recorded as working is a claim about
        // a process, and the process is the authority.
        for p in store.load_projects().await? {
            world.upsert_project(p);
        }
        let runs = store.load_runs().await?;
        let restored = runs.len();
        world.restore_runs(runs);
        tracing::info!(restored, "restored runs from the store");

        Ok(Arc::new(AppState {
            world: Mutex::new(world),
            sessions: Mutex::new(std::collections::HashMap::new()),
            store,
            policy: Mutex::new(policy),
            token,
            tx,
            started_at: jiff::Timestamp::now(),
        }))
    }

    /// Applies an event: reduce, persist, broadcast.
    ///
    /// Persistence follows the reduction rather than preceding it, so the board
    /// is never behind what the store says. A store write that fails is logged
    /// and dropped: losing history is survivable, stalling the receiver is not.
    pub async fn ingest(
        &self,
        run: RunId,
        source: Source,
        event: Event,
        cwd: Option<PathBuf>,
        mode: RunMode,
    ) {
        let env = EventEnvelope::new(run, source, event);
        let (env, _changes) = {
            let mut w = self.world.lock().await;
            w.apply(
                env,
                vibeplane_core::RunHint {
                    cwd,
                    mode,
                    agent: "claude".into(),
                },
            )
        };

        if let Err(e) = self.store.append_event(&env).await {
            tracing::warn!(error = %e, "could not persist event");
        }

        // The project row goes in before the run that references it. The other
        // order loses the run to a foreign-key violation, which shows up only
        // after a restart — as a board that forgot everything.
        if let Some(pid) = &env.project_id {
            let project = {
                let w = self.world.lock().await;
                w.project(pid).cloned()
            };
            if let Some(p) = project
                && let Err(e) = self.store.save_project(&p).await
            {
                tracing::warn!(error = %e, "could not persist project");
            }
        }

        let run_snapshot = {
            let w = self.world.lock().await;
            w.run(&env.run_id).cloned()
        };
        if let Some(run) = run_snapshot
            && let Err(e) = self.store.save_run(&run).await
        {
            tracing::warn!(error = %e, "could not persist run");
        }

        let _ = self.tx.send(env);
    }
}

/// Binds and serves until the process is asked to stop.
pub async fn serve(state: Shared, port: u16) -> Result<()> {
    let app: Router = crate::api::router(state.clone());
    // Loopback only. There is no configuration to expose this on another
    // interface, because a control plane that can approve tool calls has no
    // business listening on a network.
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr} — is another daemon already running?"))?;
    let bound = listener.local_addr()?;

    crate::config::write_daemon_info(&crate::config::DaemonInfo {
        pid: std::process::id(),
        port: bound.port(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at: jiff::Timestamp::now().to_string(),
    })?;

    tracing::info!(%bound, "vibeplane daemon listening");

    let notifications = std::env::var("VIBEPLANE_NOTIFY").as_deref() != Ok("0");
    let poller = tokio::spawn(crate::poller::run(state.clone()));
    let sweeper = tokio::spawn(crate::poller::stall_sweeper(state.clone(), notifications));
    let retention = tokio::spawn(crate::poller::retention(state.clone(), 30, 7));

    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;

    poller.abort();
    sweeper.abort();
    retention.abort();
    crate::config::clear_daemon_info().ok();
    result.context("serving")?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}

//! Background loops: the roster poller and the stall sweeper.

use crate::daemon::Shared;
use std::time::Duration;
use vibeplane_domain::event::{Event, Source};
use vibeplane_domain::ids::RunId;
use vibeplane_domain::run::RunMode;

/// How often the background roster is read. The command is cheap, but it is a
/// process spawn, so the interval backs off when nothing is running.
const POLL_BUSY: Duration = Duration::from_secs(2);
const POLL_IDLE: Duration = Duration::from_secs(10);

/// Polls `claude agents --json` for sessions the provider's own daemon
/// supervises.
///
/// Hooks tell us about background sessions too, but only while they are firing:
/// a session that is blocked emits nothing, and a daemon that was restarted has
/// missed everything. The roster is the correction.
pub async fn run(state: Shared) {
    let mut interval = POLL_BUSY;
    loop {
        tokio::time::sleep(interval).await;
        match poll_once(&state).await {
            Ok(n) if n > 0 => interval = POLL_BUSY,
            Ok(_) => interval = POLL_IDLE,
            Err(e) => {
                tracing::debug!(error = %e, "roster poll failed");
                // `claude` may not be on PATH at all. That is a normal
                // configuration, not an error worth repeating every 2 seconds.
                interval = POLL_IDLE;
            }
        }
    }
}

/// Reads the roster once, before the API is reachable.
///
/// Without this the first `vibeplane ls` after a cold start shows an empty
/// board while twenty sessions are running, and the first thing the product
/// ever says about itself is wrong.
pub async fn initial_poll(state: &Shared) {
    match poll_once(state).await {
        Ok(n) => tracing::info!(sessions = n, "discovered sessions from the roster"),
        Err(e) => tracing::info!(error = %e, "no roster available at startup"),
    }
}

async fn poll_once(state: &Shared) -> anyhow::Result<usize> {
    let started = std::time::Instant::now();
    let Some(bin) = vibeplane_observe::locate::claude_binary() else {
        anyhow::bail!("no claude binary found");
    };
    let out = tokio::process::Command::new(bin)
        .args(["agents", "--json", "--all"])
        .kill_on_drop(true)
        .output()
        .await?;

    if !out.status.success() {
        anyhow::bail!("claude agents --json exited with {}", out.status);
    }
    let rows = vibeplane_observe::agents_json::parse(&out.stdout);

    // The roster does not name the surface a session runs on. The local
    // registry does, and telemetry does — but telemetry only once the session
    // makes a request, and the registry is there the moment it starts.
    let registry = vibeplane_observe::registry::sessions();
    let entrypoint_of = |session: &str| -> Option<String> {
        registry
            .iter()
            .find(|e| e.session_id.as_deref() == Some(session))
            .and_then(|e| e.entrypoint.clone())
    };

    let mut seen = 0;
    for row in &rows {
        let Some(key) = row.run_key() else { continue };
        seen += 1;
        // Every live session belongs on the board, whoever started it. The
        // mode records who supervises the process, which decides whose word
        // counts about the state.
        let mode = if row.is_background() {
            RunMode::Background
        } else {
            RunMode::Observed
        };
        let mut event = row.to_event();
        if let vibeplane_domain::Event::RosterSeen {
            ref mut entrypoint, ..
        } = event
            && entrypoint.is_none()
        {
            *entrypoint = entrypoint_of(&key);
        }
        state
            .ingest(
                RunId::new(key),
                Source::AgentsJson,
                event,
                Some(row.cwd.clone()),
                mode,
            )
            .await;
    }
    state
        .store
        .record_channel("agents_json", started.elapsed().as_micros() as u64, None)
        .await
        .ok();
    Ok(seen)
}

/// Notices runs that stopped producing events.
///
/// A stall is the absence of evidence, so nothing can report it: the only way
/// to know is to look at the clock. The sweeper does not change state — the
/// inbox derives `stalled` from the run's own idle time — it exists to wake
/// subscribers so a stalled run appears without anyone refreshing.
pub async fn stall_sweeper(state: Shared) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let stalled: Vec<RunId> = {
            let w = state.world.lock().await;
            let cfg = w.attention;
            w.runs()
                .filter(|r| {
                    matches!(r.state, vibeplane_domain::RunState::Working)
                        && r.idle_seconds() > cfg.stall_seconds
                })
                .map(|r| r.id.clone())
                .collect()
        };
        for id in stalled {
            // A zero-cost nudge: re-broadcast the run so subscribers re-derive
            // the inbox. No event is recorded, because nothing happened.
            let env = vibeplane_domain::event::EventEnvelope::new(
                id,
                Source::Daemon,
                Event::StatusSample {
                    context_used_percent: None,
                    rate_limit_five_hour: None,
                    rate_limit_seven_day: None,
                    session_name: None,
                },
            );
            let _ = state.tx.send(env);
        }
    }
}

/// Whether a process is still alive. Used by reconciliation at startup.
///
/// On Unix, signal 0 tests for existence without touching the process.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: `kill` with signal 0 performs no action; it only reports
        // whether the pid exists and is signalable.
        unsafe { libc_kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

/// Reconciles restored runs against the machine at startup.
///
/// A run the store believes is live, whose process is gone and which the
/// provider's roster does not list, is lost. Never assume a run still exists
/// because the database says so.
pub async fn reconcile_at_startup(state: &Shared) {
    let Some(bin) = vibeplane_observe::locate::claude_binary() else {
        tracing::warn!("no claude binary found; skipping reconciliation");
        return;
    };
    let roster: Vec<String> = match tokio::process::Command::new(bin)
        .args(["agents", "--json", "--all"])
        .kill_on_drop(true)
        .output()
        .await
    {
        Ok(out) if out.status.success() => vibeplane_observe::agents_json::parse(&out.stdout)
            .iter()
            .filter_map(|r| r.run_key())
            .collect(),
        _ => Vec::new(),
    };

    let lost = {
        let mut w = state.world.lock().await;
        w.reconcile(&|run| {
            if roster.iter().any(|k| k == run.id.as_str()) {
                return true;
            }
            match run.pid {
                Some(pid) => process_alive(pid),
                // An interactive session reports no pid. Its own hooks will
                // correct the record the moment it does anything, and calling
                // it lost on a hunch would fill the inbox with ghosts every
                // time the daemon restarted.
                None => true,
            }
        })
    };

    for env in lost {
        if let Err(e) = state.store.append_event(&env).await {
            tracing::warn!(error = %e, "could not persist reconciliation");
        }
        let _ = state.tx.send(env);
    }
}

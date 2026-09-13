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

/// Notices runs that stopped producing events, and keeps the inbox audible.
///
/// A stall is the absence of evidence, so nothing can report it: the only way
/// to know is to look at the clock. The sweeper also drives desktop
/// notifications, because the inbox changes for reasons no event announces —
/// a run going quiet is one of them.
pub async fn stall_sweeper(state: Shared, notify_enabled: bool) {
    let mut notifier = crate::notify::Notifier::new(notify_enabled);
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;

        let newly_stalled: Vec<(RunId, i64)> = {
            let w = state.world.lock().await;
            let cfg = w.attention;
            w.runs()
                .filter(|r| {
                    // How long is too quiet is a property of the work, not of
                    // the machine: a repository whose suite takes twelve
                    // minutes and one that answers in seconds cannot share a
                    // threshold. The project's `[policy] stall_timeout` wins
                    // where it is set, and the run's worktree resolves back to
                    // the repository that owns it.
                    let dir = r.worktree.as_deref().unwrap_or(&r.cwd);
                    let limit = state.policy.stall_seconds(dir).unwrap_or(cfg.stall_seconds);
                    matches!(r.state, vibeplane_domain::RunState::Working)
                        && !r.stall_noticed
                        && r.idle_seconds() > limit
                })
                .map(|r| (r.id.clone(), r.idle_seconds()))
                .collect()
        };

        for (id, idle_seconds) in newly_stalled {
            state
                .ingest(
                    id,
                    Source::Daemon,
                    Event::Stalled { idle_seconds },
                    None,
                    RunMode::Observed,
                )
                .await;
        }

        let inbox = {
            let w = state.world.lock().await;
            w.inbox()
        };
        notifier.sync(&inbox);
    }
}

/// Refreshes the pull requests Vibeplane opened.
///
/// Checks finish minutes or hours after the agent stopped, which is the clearest
/// illustration of why Work is the durable unit: the session that wrote the code
/// is long gone, and somebody still has to be told the build went red.
///
/// Polled rather than pushed, because a webhook needs a public address and this
/// is a local tool. Slowly, because nothing here is urgent to the second.
pub async fn pull_requests(state: Shared) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        let watched: Vec<(vibeplane_domain::WorkId, std::path::PathBuf, String)> = {
            let works = state.works.lock().await;
            works
                .values()
                .filter(|w| {
                    w.pull_request
                        .as_ref()
                        .map(|p| !matches!(p.status.as_str(), "merged" | "closed"))
                        .unwrap_or(false)
                })
                .filter_map(|w| Some((w.id.clone(), w.worktree.clone()?, w.branch.clone()?)))
                .collect()
        };

        for (id, dir, branch) in watched {
            let Ok(Some(pr)) = vibeplane_github::pr_for_branch(&dir, &branch).await else {
                continue;
            };
            let status = format!("{:?}", pr.status()).to_lowercase();
            let failing: Vec<String> = pr.failing_checks().iter().map(|c| c.name.clone()).collect();

            let changed = {
                let mut works = state.works.lock().await;
                match works.get_mut(&id) {
                    Some(w) => match &mut w.pull_request {
                        Some(existing) if existing.status != status => {
                            existing.status = status.clone();
                            existing.failing_checks = failing;
                            w.updated_at = jiff::Timestamp::now();
                            Some(w.clone())
                        }
                        _ => None,
                    },
                    None => None,
                }
            };
            if let Some(w) = changed {
                tracing::info!(work = %id, %status, "pull request changed");
                state.store.save_work(&w).await.ok();
                state.notify_changed();
            }
        }
    }
}

/// Keeps the store and the board from growing without bound.
///
/// Events age out; runs do not, because a row on the board costs nothing and
/// losing one would make a resumable session invisible. Terminal runs leave
/// memory once they are old enough to be history rather than context.
pub async fn retention(state: Shared, keep_event_days: i64, keep_run_days: i64) {
    loop {
        tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        match state.store.prune_events(keep_event_days).await {
            Ok(n) if n > 0 => tracing::info!(events = n, "pruned old events"),
            Err(e) => tracing::warn!(error = %e, "pruning events failed"),
            _ => {}
        }
        let dropped = {
            let mut w = state.world.lock().await;
            w.prune(keep_run_days * 86_400)
        };
        if dropped > 0 {
            tracing::info!(runs = dropped, "dropped finished runs from the board");
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

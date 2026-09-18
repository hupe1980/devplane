//! Background loops: the roster poller and the stall sweeper.

use crate::core::event::{Event, Source};
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::daemon::Shared;
use std::time::Duration;

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
/// Without this the first `devplane ls` after a cold start shows an empty
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
    let Some(bin) = crate::observe::locate::claude_binary() else {
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
    let rows = crate::observe::agents_json::parse(&out.stdout);

    // The roster does not name the surface a session runs on. The local
    // registry does, and telemetry does — but telemetry only once the session
    // makes a request, and the registry is there the moment it starts.
    let registry = crate::observe::registry::sessions();
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
        if let crate::core::Event::RosterSeen {
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
                    // Only a session on a channel that carries activity can be
                    // seen to go quiet — the same question the inbox asks, for
                    // the reason both must ask it the same way.
                    r.activity_seen
                        && matches!(r.state, crate::core::RunState::Working)
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

        // The same list the person sees, derived the same way — Work items
        // included, and the project's own stall threshold. A notifier reading a
        // different list announces things the inbox does not show, and stays
        // silent about things it does.
        let inbox = state.current_inbox().await;
        notifier.sync(&inbox);

        // The inbox measuring itself. Every item asking right now is recorded
        // once; everything that stopped asking without a person using one of
        // its actions is closed as `elsewhere`. The API has already written
        // `acted` or `dismissed` for anything somebody touched, and the
        // `resolved_at IS NULL` guard means this pass cannot overwrite it — so
        // the numbers are exact rather than correlated after the fact.
        //
        // Errors are logged and dropped: this is a measurement of the product,
        // and a measurement that can take the product down is worse than none.
        let open: Vec<String> = inbox.iter().map(|i| i.id.0.clone()).collect();
        for item in &inbox {
            if let Err(e) = state.store.attention_raise(item).await {
                tracing::warn!(error = %e, "could not record an inbox raise");
                break;
            }
        }
        if let Err(e) = state.store.attention_sweep(&open).await {
            tracing::warn!(error = %e, "could not close resolved inbox items");
        }
    }
}

/// Refreshes the pull requests Devplane opened.
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

        let watched: Vec<(crate::core::WorkId, std::path::PathBuf, String)> = {
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
            let Ok(Some(pr)) = crate::github::pr_for_branch(&dir, &branch).await else {
                continue;
            };
            let status = pr.status().as_str().to_string();
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
                // Not `.ok()`: this write carries the fact behind a `ci_red`
                // item, and dropping it leaves the board ahead of the store.
                crate::work::persist(&state, &w).await;
                state.notify_changed();
            }
        }
    }
}

/// How often every project's forge is read. Slowly: an issue assigned to you
/// five minutes late is still an issue assigned to you, and `gh` is two
/// process spawns per project.
const FORGE_EVERY: Duration = Duration::from_secs(300);
/// How many of each a project contributes. Past this the board shows a count,
/// and a count is the honest shape of four hundred open issues anyway.
const FORGE_LIMIT: u32 = 100;

/// Reads GitHub for every registered project: open issues, open pull
/// requests, and which of them are waiting on the person whose `gh` this is.
///
/// Soon after start, then on a slow timer. Never writes. A project whose
/// directory has no GitHub remote is asked once and then skipped for the life
/// of the daemon; a `gh` that is not logged in stops the whole pass and is
/// reported in `doctor` rather than retried every second.
pub async fn forge_watch(state: Shared) {
    tokio::time::sleep(Duration::from_secs(3)).await;
    loop {
        forge_once(&state).await;
        tokio::time::sleep(FORGE_EVERY).await;
    }
}

async fn forge_once(state: &Shared) {
    let projects: Vec<crate::core::Project> =
        state.world.lock().await.projects().cloned().collect();
    if projects.is_empty() {
        return;
    }

    // Whose forge. Asked once, from a directory that is certainly there.
    let viewer = {
        let known = state.forge.lock().await.viewer.clone();
        match known {
            Some(v) => Some(v),
            None => match crate::github::viewer_login(&std::env::temp_dir()).await {
                Ok(v) => {
                    let mut f = state.forge.lock().await;
                    f.viewer = Some(v.clone());
                    f.error = None;
                    Some(v)
                }
                Err(e) => {
                    let mut f = state.forge.lock().await;
                    f.error = Some(e.to_string());
                    f.last_poll_at = Some(jiff::Timestamp::now());
                    tracing::info!(error = %e, "forge: gh is not usable");
                    return;
                }
            },
        }
    };

    // Which pull requests GitHub says are waiting for this person's review —
    // one search for the whole machine, because whether a team request reaches
    // them is a fact only the server has (see `review_requested_of_me`). A
    // failure costs that one signal and nothing else, so it is logged rather
    // than allowed to fail the pass.
    let asked_of_me = match crate::github::review_requested_of_me(&std::env::temp_dir()).await {
        Ok(set) => set,
        Err(e) => {
            tracing::info!(error = %e, "forge: could not search for review requests");
            Default::default()
        }
    };

    // A project ruled out stays ruled out for an hour, not forever: adding a
    // GitHub remote is a thing people do, and a daemon restart is not a
    // reasonable thing to require of them for it.
    let now = jiff::Timestamp::now();
    let skip: std::collections::BTreeSet<_> = {
        let f = state.forge.lock().await;
        f.skip
            .keys()
            .filter(|id| f.should_skip(id, now))
            .cloned()
            .collect()
    };
    let before = state.forge.lock().await.counts();
    for p in projects {
        if skip.contains(&p.id) {
            continue;
        }
        let issues = crate::github::open_issues(&p.root, FORGE_LIMIT).await;
        let prs = crate::github::open_pull_requests(&p.root, FORGE_LIMIT).await;
        let mut f = state.forge.lock().await;
        match (issues, prs) {
            (Ok(issues), Ok(prs)) => {
                let slug = p.repo_slug();
                f.projects.insert(
                    p.id.clone(),
                    crate::core::ProjectForge {
                        project_id: p.id.clone(),
                        repo: slug.clone(),
                        fetched_at: jiff::Timestamp::now(),
                        issues: issues
                            .iter()
                            .map(|i| i.to_forge(viewer.as_deref()))
                            .collect(),
                        pull_requests: prs
                            .iter()
                            .map(|r| {
                                // Server-resolved, because team membership is
                                // not in the row (see `review_requested_of_me`).
                                let asked = slug.as_deref().is_some_and(|s| {
                                    asked_of_me.contains(&(s.to_string(), r.number))
                                });
                                r.to_forge(viewer.as_deref(), asked)
                            })
                            .collect(),
                        error: None,
                    },
                );
            }
            (Err(e), _) | (_, Err(e)) => {
                let msg = e.to_string();
                if crate::github::is_permanent(&msg) {
                    // Not a GitHub project. Say so once, then stop asking.
                    tracing::debug!(project = %p.name, error = %msg, "forge: no GitHub remote");
                    f.skip.insert(p.id.clone(), (msg, jiff::Timestamp::now()));
                    f.projects.remove(&p.id);
                } else if let Some(existing) = f.projects.get_mut(&p.id) {
                    // Keep what was known; a network blip must not empty the board.
                    existing.error = Some(msg);
                } else {
                    f.projects.insert(
                        p.id.clone(),
                        crate::core::ProjectForge {
                            project_id: p.id.clone(),
                            repo: p.repo_slug(),
                            fetched_at: jiff::Timestamp::now(),
                            issues: vec![],
                            pull_requests: vec![],
                            error: Some(msg),
                        },
                    );
                }
            }
        }
    }
    let after = {
        let mut f = state.forge.lock().await;
        f.last_poll_at = Some(jiff::Timestamp::now());
        f.counts()
    };
    if before != after {
        state.notify_changed();
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
/// On Unix, signal 0 tests for existence without touching the process. On every
/// other platform the honest answer is "we cannot tell", and the caller treats
/// that as alive rather than declaring a run lost on a guess.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: `kill` with signal 0 performs no action; it only reports
        // whether the pid exists and is signalable.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// Reconciles restored runs against the machine at startup.
///
/// A run the store believes is live, whose process is gone and which the
/// provider's roster does not list, is lost. Never assume a run still exists
/// because the database says so.
pub async fn reconcile_at_startup(state: &Shared) {
    // "We could not ask" and "nothing is running" are different answers, and
    // conflating them is how every restored run gets marked lost at once. So a
    // roster that cannot be read is `None` — no evidence — rather than an empty
    // list, and the pid check below carries the weight on its own.
    //
    // Returning early when there is no `claude` on the machine would skip
    // reconciliation entirely for a protocol-only user, and leave every
    // restored run reading "working" for ever.
    let roster: Option<Vec<String>> = match crate::observe::locate::claude_binary() {
        None => None,
        Some(bin) => match tokio::process::Command::new(bin)
            .args(["agents", "--json", "--all"])
            .kill_on_drop(true)
            .output()
            .await
        {
            Ok(out) if out.status.success() => Some(
                crate::observe::agents_json::parse(&out.stdout)
                    .iter()
                    .filter_map(|r| r.run_key())
                    .collect(),
            ),
            other => {
                tracing::warn!(ok = other.is_ok(), "could not read the roster");
                None
            }
        },
    };

    let lost = {
        let mut w = state.world.lock().await;
        w.reconcile(&|run| {
            if roster
                .as_ref()
                .is_some_and(|r| r.iter().any(|k| k == run.id.as_str()))
            {
                return true;
            }
            match run.pid {
                Some(pid) => process_alive(pid),
                // An interactive session reports no pid, and neither does a run
                // Devplane drove: the process is behind an ACP connection that
                // did not survive the restart. Its own hooks correct the record
                // the moment it does anything, and calling it lost on a hunch
                // would fill the inbox with ghosts every time the daemon
                // bounced. Whether a *driven* run can still be reached is a
                // question about sessions, and the inbox asks it separately.
                None => true,
            }
        })
    };

    for env in lost {
        if let Err(e) = state.store.append_event(&env).await {
            tracing::warn!(error = %e, "could not persist reconciliation");
        }
        let _ = state.tx.send(crate::core::Frame::Event(env));
    }
}

/// Runs the installed gate on a slow timer and remembers whether it answered.
///
/// **A broken gate and a quiet machine are the same thing in the event log** —
/// no hook arrives either way — so this is the only mechanism that can tell
/// them apart without a person. `devplane doctor` asks the same question and
/// only helps whoever runs it; this puts the answer in the inbox.
///
/// Slow on purpose. The probe spawns a process, and a gate that broke four
/// minutes ago is caught soon enough: the failure it is looking for is a
/// binary that moved or a settings file somebody edited, neither of which
/// happens between two tool calls. The first check is immediate, because the
/// interesting moment is a daemon starting on a machine whose gate has been
/// broken since the last reboot.
pub async fn gate_watch(state: Shared) {
    const EVERY: Duration = Duration::from_secs(240);
    loop {
        let settings = crate::observe::connect::settings_path()
            .ok()
            .and_then(|p| crate::observe::connect::read_settings(&p).ok());
        if let Some(settings) = settings {
            // The probe spawns a process and reads a file, so it goes on the
            // blocking pool rather than holding a reactor thread that agent
            // sessions are also using.
            let probe =
                tokio::task::spawn_blocking(move || crate::observe::connect::probe_gate(&settings))
                    .await;
            if let Ok(probe) = probe {
                // Not installed is not the same as broken. Somebody who has
                // never run `connect` is being told that by every other
                // surface, and an inbox item saying the gate is down would be
                // the product complaining that it has not been set up.
                let down = (probe.command.is_some() && !probe.answered).then(|| {
                    probe
                        .error
                        .unwrap_or_else(|| "it did not refuse a denied read".into())
                });
                let mut held = state.gate_down.lock().await;
                if *held != down {
                    match &down {
                        Some(why) => tracing::error!(why, "the permission gate is not answering"),
                        None => tracing::info!("the permission gate is answering again"),
                    }
                }
                *held = down;
            }
        }
        tokio::time::sleep(EVERY).await;
    }
}

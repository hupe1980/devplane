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

    // **What the roster calls `idle` is two situations, and one `ps` tells
    // them apart.** A session waiting on a test suite it started reports
    // exactly as one waiting for a person to type, and the board said the same
    // sentence for both. The process table carries the difference and the
    // roster does not.
    //
    // Only asked when a row could be affected, so a machine whose sessions are
    // all busy or all connected pays nothing. One `ps` answers for every
    // session at once, beside the `claude agents --json` this loop already
    // spawns each pass — a second short-lived process on a poll that has one.
    let jobs = {
        let idle_pids: Vec<u32> = rows
            .iter()
            .filter(|r| r.status.as_deref() != Some("busy"))
            .filter_map(|r| r.pid)
            .collect();
        match idle_pids.is_empty() {
            true => std::collections::HashMap::new(),
            false => crate::observe::procs::running_jobs(&idle_pids),
        }
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
            ref mut entrypoint,
            jobs: ref mut jobs_field,
            ..
        } = event
        {
            if entrypoint.is_none() {
                *entrypoint = entrypoint_of(&key);
            }
            // Absent unless this row was actually looked at, because the
            // reducer treats a checked zero as *the job finished* and must
            // never be handed one that means *nobody asked*.
            *jobs_field = row.pid.and_then(|pid| jobs.get(&pid).copied());
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
/// Ends asks whose project-set deadline has passed.
///
/// **The third of the five durable-execution properties, and the one this
/// product does differently from every system it took the other four from.**
/// Temporal, Inngest, Restate, Step Functions and LangGraph all let an
/// unanswered request end; none of them records *who ended it*. An ask that
/// ends here ends with an authority, the duration, and the file the duration
/// came from — and if nothing wrote a duration down, nothing ends.
///
/// It is a query rather than a timer per waiting ask, which is what makes the
/// second property (*the wait costs nothing*) true after a restart as well as
/// before one: the rows are the state, and a daemon that has just started knows
/// exactly as much as one that has been up for a week.
pub async fn expiry_sweeper(state: Shared) {
    loop {
        // A minute is fine and deliberately coarse. The alternative — waking at
        // each deadline — is a timer per waiting ask, which is the design this
        // one exists instead of, and nobody sets a deadline where a minute of
        // slack matters.
        tokio::time::sleep(Duration::from_secs(60)).await;

        let now = jiff::Timestamp::now();
        let overdue: Vec<crate::core::ask::Ask> = match state.store.open_asks().await {
            Ok(a) => a.into_iter().filter(|a| a.is_overdue(now)).collect(),
            Err(e) => {
                tracing::warn!(error = %e, "could not read the open asks");
                continue;
            }
        };

        for mut ask in overdue {
            let crate::core::ask::Deadline::After(after) = ask.deadline else {
                continue;
            };
            // **The file, not "a setting"**: a person reading this row has to be
            // able to go and change the thing that did it.
            //
            // Which means the *path*, not the filename. Both arms of this used
            // to produce the bare string `devplane.toml` — a conditional that
            // computed the same answer either way — so the row that exists to
            // say *where the clock that ended your question is configured* told
            // somebody with six projects to go and look in six places. The
            // project id is the project's path, so the real file is one join
            // away.
            let set_by = deadline_source(ask.project.as_ref());
            let ended = crate::core::ask::Ended::Timer { after, set_by };
            ask.end(ended.clone(), now);
            if let Err(e) = state.store.save_ask(&ask).await {
                tracing::error!(error = %e, ask = %ask.id, "could not close an expired ask");
                continue;
            }

            // **Told to the agent, recorded in the log, and shown to the
            // person — in that order and all three.** The old ten-minute
            // refusal did only the middle one, which is how a call refused in
            // somebody's name became findable only by running `devplane audit`.
            crate::driven::expire(&state, &ask, &ended).await;
        }
    }
}

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
        // **Zero is not a process id, and `kill` does not treat it as one.**
        // `kill(0, sig)` addresses every process in the *caller's own process
        // group*, so `process_alive(0)` asked whether this daemon exists and
        // answered yes — a run that ever recorded a pid of 0 would have been
        // considered alive for ever. The same is true of negative values,
        // which address a group; `pid` is unsigned here, so only zero can
        // reach it. Found by writing the test rather than by anything failing.
        if pid == 0 {
            return false;
        }
        // SAFETY: `kill` with signal 0 performs no action; it only reports
        // whether the pid exists and is signalable.
        //
        // **`EPERM` means the process exists.** `kill(pid, 0)` fails two ways
        // that mean opposite things: `ESRCH` is *no such process*, `EPERM` is
        // *it is there and you may not signal it* — owned by another user, or
        // by root. Comparing the return code to zero conflates them, and reads
        // everything this daemon does not own as dead.
        if unsafe { libc::kill(pid as i32, 0) } == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// Which file set the deadline that ended an ask.
///
/// Pure, and separated from the sweeper for the same reason [`still_running`]
/// is: the rule can then be stated and tested without a store, a clock or a
/// waiting agent.
///
/// **A path, not a filename.** The row this feeds says *a clock refused your
/// question, and here is where that clock is configured* — so `devplane.toml`
/// alone sends somebody with six projects to look in six places. A project id
/// **is** the project's path, so the answer is one join away.
///
/// Without a project there is no path to give, and the bare filename is then
/// the honest answer rather than a misleading one.
pub fn deadline_source(project: Option<&crate::core::ProjectId>) -> String {
    match project {
        Some(p) => std::path::Path::new(p.as_str())
            .join(crate::core::config::CONFIG_FILE)
            .to_string_lossy()
            .into_owned(),
        None => crate::core::config::CONFIG_FILE.to_string(),
    }
}

/// Whether a restored run is still being run by something, at startup.
///
/// Pure, and separated from [`reconcile_at_startup`] so the rule can be stated
/// and tested rather than inferred from a closure inside an async function that
/// needs a database and a roster to call.
///
/// Three cases, and the middle one was wrong for months:
///
/// * the provider's roster names it — alive, and the roster outranks everything
///   because it is the provider speaking about its own sessions;
/// * **Devplane drove it — not alive, whatever the row says.** It was driven
///   over an ACP connection on the old daemon's stdio, and those pipes died
///   with the process that held them. No new daemon can re-establish them:
///   `sessions` is rebuilt empty on every boot by construction. This answered
///   *alive* until 2026-09-19, on the reasoning that "its own hooks correct the
///   record the moment it does anything" — but a driven agent does nothing,
///   because nothing is driving it. A run Devplane started therefore read
///   **working** for ever after a bounce, and a question it was holding sat
///   behind a row that looked busy;
/// * anything else — a pid if it reported one, and otherwise the benefit of the
///   doubt, because an interactive session reports no pid and its own hooks
///   really do correct the record.
///
/// `roster` is `None` when it could not be read, which is not the same as
/// empty: conflating them marks every restored run lost at once.
pub fn still_running(run: &crate::core::Run, roster: Option<&[String]>) -> bool {
    if roster.is_some_and(|r| r.iter().any(|k| k == run.id.as_str())) {
        return true;
    }
    if run.mode == crate::core::RunMode::Driven {
        return false;
    }
    match run.pid {
        Some(pid) => process_alive(pid),
        None => true,
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

    // **Agents a previous daemon abandoned.**
    //
    // Read before reconciliation, because reconciliation is about to mark
    // these runs not-live and this needs their recorded pid and worktree while
    // the rows still carry them.
    //
    // A driven run that this daemon did not start, whose recorded process is
    // still alive and still leads its own group and still looks like the agent
    // it was: that process outlived the daemon that spawned it, which only
    // happens when the daemon was killed rather than stopped. Nothing can
    // reach it — its stdio was the dead daemon's pipes — so it will never
    // finish and never be answered, and until this ran nothing on the machine
    // knew it was there.
    {
        let recorded: Vec<(u32, String, Option<String>)> = {
            let w = state.world.lock().await;
            w.runs()
                .filter(|r| r.mode == crate::core::RunMode::Driven && r.state.is_live())
                .filter_map(|r| {
                    Some((
                        r.pid?,
                        r.agent_command.clone()?,
                        r.worktree
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .or_else(|| Some(r.cwd.display().to_string())),
                    ))
                })
                .collect()
        };
        let pairs: Vec<(u32, String)> = recorded
            .iter()
            .map(|(pid, cmd, _)| (*pid, cmd.clone()))
            .collect();
        let found = crate::observe::procs::leaked(&pairs);
        if !found.is_empty() {
            let leaked: Vec<(u32, String, Option<String>)> = found
                .into_iter()
                .map(|p| {
                    let worktree = recorded
                        .iter()
                        .find(|(pid, _, _)| *pid == p.pid)
                        .and_then(|(_, _, w)| w.clone());
                    (p.pid, p.command, worktree)
                })
                .collect();
            tracing::warn!(
                count = leaked.len(),
                "agents started by a previous daemon are still running and cannot be reached"
            );
            *state.leaked_agents.lock().await = leaked;
        }
    }

    // Questions a driven run was holding when the daemon stopped, captured
    // **before** reconciliation resolves the run, because resolving it clears
    // `blocked_on` and the question would then have never existed.
    //
    // Each one gets a row naming `nobody`: the agent asked, the person was
    // never given the chance, and the moment passed.
    //
    // **This is the killed-daemon case and only that.** A daemon that is
    // *stopped* tears its agents down deliberately, and the teardown records
    // `interrupted` rather than ending the ask — the vendor's session is on
    // disk, the question is still answerable, and the inbox goes on offering
    // it. Such a run is not live by the time this runs, so the filter
    // below never sees it. What reaches here is a run that was still `working`
    // when the process died, which only happens when nothing got to tear
    // anything down. That is the authority the
    // decision log exists to be able to write, and the row it wrote before
    // 2026-09-19 said `daemon` — Devplane taking responsibility for a question
    // it had faithfully delivered and nobody had answered.
    let abandoned: Vec<(crate::core::RunId, String)> = {
        let w = state.world.lock().await;
        w.runs()
            .filter(|r| r.state.is_live() && r.mode == crate::core::RunMode::Driven)
            .filter_map(|r| {
                let b = r.blocked_on.as_ref()?;
                if b.waiting_for != crate::core::WaitingFor::Question {
                    return None;
                }
                Some((r.id.clone(), b.request_id.clone()?))
            })
            .collect()
    };
    for (run, request_id) in abandoned {
        state
            .record(
                crate::core::Decision::new(
                    crate::core::Authority::Nobody,
                    "agent:question",
                    request_id,
                    "unanswered",
                )
                .because("the daemon did not stop cleanly, and the question went with it")
                .for_run(&run),
            )
            .await;
    }

    let lost = {
        let mut w = state.world.lock().await;
        w.reconcile(&|run| {
            if roster
                .as_ref()
                .is_some_and(|r| r.iter().any(|k| k == run.id.as_str()))
            {
                return true;
            }
            still_running(run, roster.as_deref())
        })
    };

    for env in lost {
        if let Err(e) = state.store.append_event(&env).await {
            tracing::warn!(error = %e, "could not persist reconciliation");
        }
        let _ = state.tx.send(crate::core::Frame::Event(Box::new(env)));
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

#[cfg(test)]
mod tests {

    /// **A process you may not signal is still a process.**
    ///
    /// `kill(pid, 0)` fails two ways that mean opposite things: `ESRCH` is *no
    /// such process*, `EPERM` is *it exists and is not yours*. Comparing the
    /// return code to zero — which is what this function did — reports every
    /// process owned by another user as dead. pid 1 is the case every Unix has:
    /// `launchd` or `init`, always running, never signalable by a normal user.
    ///
    /// What believed it: reconciliation, which would mark a live run `lost`,
    /// and the guard that stops a second daemon starting on one home.
    #[cfg(unix)]
    #[test]
    fn a_process_we_may_not_signal_is_still_alive() {
        assert!(
            process_alive(1),
            "pid 1 always exists; EPERM was being read as 'no such process'"
        );
        // Our own pid is the control: alive and signalable, so this passed
        // before the fix too and must still pass after it.
        assert!(process_alive(std::process::id()));
        // And a pid nothing can plausibly hold is still dead, which is the
        // property the fix must not trade away.
        assert!(!process_alive(0), "0 addresses a group, never a process");
    }

    /// **The timer row names a file somebody can open.**
    ///
    /// This is the authority column's whole promise applied to itself: a clock
    /// ended your question, and the row has to say *which* clock. It said
    /// `devplane.toml` for every project on the machine, from a conditional
    /// whose two branches returned the same string.
    #[test]
    fn the_deadline_row_names_the_project_s_own_file() {
        let p = crate::core::ProjectId::from_path(std::path::Path::new("/repos/saas"));
        let named = deadline_source(Some(&p));
        assert_eq!(named, "/repos/saas/devplane.toml");
        assert!(
            named.contains("/repos/saas"),
            "a person with six projects has to be told which one: {named}"
        );

        // Two projects must not produce the same answer, which is the property
        // the old shape violated for every pair.
        let q = crate::core::ProjectId::from_path(std::path::Path::new("/repos/mobile"));
        assert_ne!(deadline_source(Some(&p)), deadline_source(Some(&q)));

        // With no project there is no path to give, and the bare filename is
        // then honest rather than misleading.
        assert_eq!(deadline_source(None), "devplane.toml");
    }

    use super::*;
    use crate::core::RunMode;

    fn run(mode: RunMode) -> crate::core::Run {
        let mut r = crate::core::Run::new(
            crate::core::SessionId::new("s1"),
            std::path::PathBuf::from("/tmp"),
            mode,
            "claude",
        );
        r.state = crate::core::RunState::Working;
        r
    }

    /// **The measured bug.** A run Devplane drove came back from a restart
    /// reading `working` for ever: nothing was driving it, nothing ever would
    /// be, and no hook was going to correct the record because a driven agent
    /// with no connection does nothing at all.
    #[test]
    fn a_driven_run_is_never_still_running_after_a_restart() {
        assert!(
            !still_running(&run(RunMode::Driven), None),
            "its ACP pipes died with the daemon that held them"
        );
        // Not even with an empty roster, and not even if it reported a pid:
        // the process being alive is a fact about a process nobody is talking
        // to. Answering `alive` here is what left the run reading `working`.
        let mut with_pid = run(RunMode::Driven);
        with_pid.pid = Some(std::process::id());
        assert!(!still_running(&with_pid, Some(&[])));
    }

    /// The one thing that outranks it: the provider saying the session is its
    /// own and is live. That is the provider speaking about its own roster,
    /// which beats anything inferred here.
    #[test]
    fn the_providers_roster_outranks_the_rule() {
        assert!(still_running(
            &run(RunMode::Driven),
            Some(&[run(RunMode::Driven).id.to_string()])
        ));
    }

    /// An observed session is given the benefit of the doubt, and this is the
    /// half that must not change: a closed editor tab reports no pid, its own
    /// hooks correct the record the moment it does anything, and calling it
    /// lost on a bounce fills the inbox with ghosts.
    #[test]
    fn an_observed_run_without_a_pid_is_given_the_benefit_of_the_doubt() {
        assert!(still_running(&run(RunMode::Observed), None));
        assert!(still_running(&run(RunMode::Observed), Some(&[])));
    }

    /// And one that reported a pid is checked against the machine.
    #[test]
    fn an_observed_run_with_a_dead_pid_is_not_still_running() {
        let mut r = run(RunMode::Observed);
        // Zero is not a process id: `kill(0, …)` addresses the caller's own
        // process group, so this used to answer "alive".
        r.pid = Some(0);
        assert!(!still_running(&r, Some(&[])));
        r.pid = Some(std::process::id());
        assert!(still_running(&r, Some(&[])), "this test's own process");
    }
}

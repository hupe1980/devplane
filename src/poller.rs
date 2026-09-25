//! The host's background tasks: roster poll, projection tail, sweepers,
//! forge and gate watches, and startup reconciliation.

use crate::core::event::{Event, Source};
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::host::Shared;
use std::time::Duration;

/// Roster poll interval; it backs off when nothing is running.
const POLL_BUSY: Duration = Duration::from_secs(2);
const POLL_IDLE: Duration = Duration::from_secs(10);

/// Polls `claude agents --json` for sessions the vendor supervises.
///
/// Hooks only speak while they fire, and a blocked session emits nothing; the
/// roster is the correction.
pub async fn run(state: Shared) {
    let mut interval = POLL_BUSY;
    loop {
        tokio::time::sleep(interval).await;
        match poll_once(&state).await {
            Ok(n) if n > 0 => interval = POLL_BUSY,
            Ok(_) => interval = POLL_IDLE,
            Err(e) => {
                tracing::debug!(error = %e, "roster poll failed");
                // `claude` may simply not be on PATH; don't retry every 2s.
                interval = POLL_IDLE;
            }
        }
    }
}

/// Folds rows other processes appended to the store into the world.
///
/// A hook appends and exits; this is the one projector. The event is durable
/// the moment the hook writes it; 500 ms is only board latency.
pub async fn tail(state: Shared) {
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Err(e) = tail_once(&state).await {
            tracing::warn!(error = %e, "could not project what other processes wrote");
        }
    }
}

/// One pass over the rows past the projection mark; returns how many folded.
pub async fn tail_once(state: &Shared) -> anyhow::Result<usize> {
    const BATCH: i64 = 500;
    let mut through = state.store.projected_through().await?;
    let mut folded = 0usize;
    loop {
        let rows = state.store.shim_events_since(through, BATCH).await?;
        if rows.is_empty() {
            break;
        }
        // Pick up projects a hook registered, so the run lands in a project
        // the world knows rather than a second copy.
        for p in state.store.load_projects().await? {
            let mut w = state.world.lock().await;
            if w.project(&p.id).is_none() {
                w.upsert_project(p);
            }
        }
        let n = rows.len() as i64;
        for (seq, env) in rows {
            // The run's directory: the event's, else the root of the project the
            // hook resolved, whether or not it still exists. Without it
            // `World::apply` falls back to this process's cwd.
            let cwd = match &env.event {
                Event::SessionStarted { cwd, .. } | Event::CwdChanged { cwd } => Some(cwd.clone()),
                _ => {
                    let w = state.world.lock().await;
                    env.project_id
                        .as_ref()
                        .and_then(|p| w.project(p).map(|p| p.root.clone()))
                }
            };
            let project = env.project_id.clone();
            // At most once per run: a run saved with this event already applied
            // (host stopped before the mark moved) skips it.
            let applied = {
                let mut w = state.world.lock().await;
                w.apply_stored(
                    seq,
                    env,
                    crate::core::RunHint {
                        cwd,
                        mode: RunMode::Observed,
                        ..Default::default()
                    },
                )
            };
            let Some((applied, _)) = applied else {
                through = seq;
                continue;
            };
            if let Some(pid) = project.or_else(|| applied.project_id.clone()) {
                state.learn_project(&pid).await;
            }
            let snapshot = {
                let w = state.world.lock().await;
                w.run(&applied.run_id).cloned()
            };
            if let Some(run) = snapshot
                && let Err(e) = state.store.save_run(&run).await
            {
                tracing::warn!(error = %e, "could not persist a projected run");
            }
            let _ = state.tx.send(crate::core::Frame::Event(Box::new(applied)));
            through = seq;
            folded += 1;
        }
        state.store.set_projected_through(through).await?;
        if n < BATCH {
            break;
        }
    }
    Ok(folded)
}

/// Reads the roster once, before the API is reachable, so the first `ls`
/// after a cold start does not show an empty board.
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

    // The roster's `idle` covers both "waiting on a person" and "waiting on a
    // background job it started"; the process table tells them apart. One
    // `ps` covers every session, and it is only asked for non-busy sessions
    // with no hooks: a hooked session reports its background tasks on `Stop`,
    // which outranks a guess from `ps`.
    let hookless: std::collections::HashSet<String> = {
        let w = state.world.lock().await;
        rows.iter()
            .filter_map(|r| r.run_key())
            .filter(|k| {
                w.run(&RunId::new(k.clone()))
                    .is_none_or(|run| run.last_hook_at.is_none())
            })
            .collect()
    };
    let jobs = {
        let idle_pids: Vec<u32> = rows
            .iter()
            .filter(|r| r.status.as_deref() != Some("busy"))
            .filter(|r| r.run_key().is_some_and(|k| hookless.contains(&k)))
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
        // Every live session belongs on the board; the mode records who
        // supervises the process, which decides whose word counts.
        let mode = if row.is_background() {
            RunMode::Background
        } else {
            RunMode::Observed
        };
        let mut event = row.to_event();
        if let crate::core::Event::RosterSeen {
            jobs: ref mut jobs_field,
            ..
        } = event
        {
            // `None` unless this row was checked: the reducer reads a checked
            // zero as "the job finished", never as "nobody asked".
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

/// Ends asks whose project-set deadline has passed.
///
/// The ending records an authority (`timer`), the duration and the file it
/// came from; with no configured duration nothing ends. It is a query over
/// stored rows rather than a timer per ask, so it holds across restarts.
pub async fn expiry_sweeper(state: Shared) {
    loop {
        // A minute of slack is fine; waking per deadline would mean a timer
        // per waiting ask.
        tokio::time::sleep(Duration::from_secs(60)).await;

        // A held permission whose hook is gone ends too, so the inbox stops
        // offering a question nobody is waiting on.
        crate::record::end_orphaned_holds(&state.store).await;

        let now = jiff::Timestamp::now();
        let overdue: Vec<crate::core::ask::Ask> = match state.store.open_asks().await {
            Ok(a) => a
                .into_iter()
                .filter(|a| a.is_overdue(now, state.started_at))
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "could not read the open asks");
                continue;
            }
        };

        for mut ask in overdue {
            let crate::core::ask::Deadline::After(after) = ask.deadline else {
                continue;
            };
            // The config file's path, not just its name, so the reader can go
            // and change the setting that did this.
            let set_by = deadline_source(ask.project.as_ref());
            let ended = crate::core::ask::Ended::Timer { after, set_by };
            ask.end(ended.clone(), now);
            // Only onto a still-open row: an answer that landed since the read
            // above wins.
            match state.store.close_ask(&ask).await {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => {
                    tracing::error!(error = %e, ask = %ask.id, "could not close an expired ask");
                    continue;
                }
            }

            // Tell the agent, record it, and show the person — all three.
            crate::driven::expire(&state, &ask, &ended).await;
        }
    }
}

/// Marks runs that went quiet and keeps desktop notifications in sync.
///
/// A stall is the absence of evidence, so only the clock can find it; and the
/// inbox changes for reasons no event announces, so notifications are driven
/// from here too.
pub async fn stall_sweeper(state: Shared, notify_enabled: bool) {
    let mut notifier = crate::notify::Notifier::new(notify_enabled);
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;

        // One clock for the whole pass.
        let now = jiff::Timestamp::now();
        let newly_stalled: Vec<(RunId, i64)> = {
            let w = state.world.lock().await;
            let cfg = w.attention;
            w.runs()
                .filter(|r| {
                    // The threshold is per project (`[policy] stall_timeout`,
                    // resolved from the run's worktree), else the host default.
                    let dir = r.worktree.as_deref().unwrap_or(&r.cwd);
                    let limit = state.policy.stall_seconds(dir).unwrap_or(cfg.stall_seconds);
                    // `facts::stalled` is the reducer; `stall_noticed` only
                    // dedups the notification.
                    crate::core::reduce::facts::stalled(r, now, limit) && !r.stall_noticed
                })
                .map(|r| (r.id.clone(), crate::core::reduce::facts::idle_for(r, now)))
                .collect()
        };

        for (id, idle_seconds) in newly_stalled {
            state
                .ingest(
                    id,
                    Source::Host,
                    Event::Stalled { idle_seconds },
                    None,
                    RunMode::Observed,
                )
                .await;
        }

        // The same inbox the person sees; a notifier reading a different list
        // would announce things the inbox does not show.
        let inbox = state.current_inbox().await;
        notifier.sync(&inbox);

        // Record every open item once and close as `elsewhere` whatever stopped
        // asking untouched. The API writes `acted`/`dismissed` first and the
        // `resolved_at IS NULL` guard keeps this pass from overwriting it.
        // Errors are logged, never fatal: this is only a measurement.
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

/// Refreshes the pull requests Devplane opened, since checks finish long after
/// the session that wrote the code is gone.
///
/// Polled, not pushed: a webhook needs a public address and this is local.
pub async fn pull_requests(state: Shared) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        let watched: Vec<(crate::core::ChangeId, std::path::PathBuf, String)> = {
            let changes = state.changes.lock().await;
            changes
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
                let mut changes = state.changes.lock().await;
                match changes.get_mut(&id) {
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
                tracing::info!(change = %id, %status, "pull request changed");
                // Not `.ok()`: this write backs a `ci_red` item, and dropping
                // it leaves the board ahead of the store.
                crate::change::persist(&state, &w).await;
                state.notify_changed();
            }
        }
    }
}

/// How often every project's forge is read (`gh` is two spawns per project).
const FORGE_EVERY: Duration = Duration::from_secs(300);
/// Per-project cap on issues and pull requests; past it the board shows a count.
const FORGE_LIMIT: u32 = 100;

/// Reads open issues and pull requests for every registered project, and which
/// are waiting on the `gh` user. Read-only.
///
/// A project without a GitHub remote is skipped for a while; a `gh` that is not
/// logged in stops the pass and is reported in `doctor`.
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

    // Whose forge; asked once.
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

    // Review requests for this person, in one search: whether a team request
    // reaches them is known only to the server. A failure costs this signal
    // only.
    let asked_of_me = match crate::github::review_requested_of_me(&std::env::temp_dir()).await {
        Ok(set) => set,
        Err(e) => {
            tracing::info!(error = %e, "forge: could not search for review requests");
            Default::default()
        }
    };

    // A ruled-out project is retried after an hour, since people add remotes.
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
                                // Server-resolved: team membership is not in the row.
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
                    // Not a GitHub project; stop asking.
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

/// Prunes old events and old terminal runs, once per host start.
///
/// Runs are kept longer than events: losing one would hide a resumable
/// session. A startup pass rather than a timer, because hosts restart often.
pub async fn retention(state: &Shared, keep_event_days: i64, keep_run_days: i64) {
    match state.store.prune_events(keep_event_days).await {
        Ok(n) if n > 0 => tracing::info!(events = n, "pruned old events"),
        Err(e) => tracing::warn!(error = %e, "pruning events failed"),
        _ => {}
    }
    let dropped = {
        let mut w = state.world.lock().await;
        w.prune(jiff::Timestamp::now(), keep_run_days * 86_400)
    };
    if dropped > 0 {
        tracing::info!(runs = dropped, "dropped finished runs from the board");
    }
    // From the store too, or the next start restores them.
    match state.store.prune_runs(keep_run_days).await {
        Ok(n) if n > 0 => tracing::info!(runs = n, "pruned finished runs"),
        Err(e) => tracing::warn!(error = %e, "pruning runs failed"),
        _ => {}
    }
}

/// Whether a process is still alive; used by startup reconciliation.
///
/// On non-Unix platforms we cannot tell, so the answer is "alive" rather than
/// declaring a run lost on a guess.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Zero is not a pid: `kill(0, …)` addresses the caller's own process
        // group and would always answer yes.
        if pid == 0 {
            return false;
        }
        // SAFETY: signal 0 performs no action; it only reports whether the
        // pid exists. `EPERM` means it exists but is not ours; only `ESRCH`
        // means gone.
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

/// Which file set the deadline that ended an ask: the project's config path,
/// so the reader knows where to change it, or the bare filename with no
/// project.
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
/// * named by the vendor's roster: alive; the roster outranks everything;
/// * driven by Devplane: not alive. Its ACP pipes died with the previous host
///   and nothing can reattach, so it would otherwise read `working` for ever;
/// * otherwise: its pid if it reported one, else alive, since an interactive
///   session reports no pid and its own hooks correct the record.
///
/// `roster` is `None` when it could not be read, which is not the same as empty.
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

/// Reconciles restored runs against the machine at startup: a run the store
/// thinks is live, whose process is gone and which the roster does not list,
/// is lost.
pub async fn reconcile_at_startup(state: &Shared) {
    // An unreadable roster is `None` (no evidence), not an empty list, or every
    // restored run is marked lost at once. No `claude` binary is not a reason
    // to skip reconciliation.
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

    // Agents a killed host left behind: a driven, still-live run whose process
    // is alive, leads its group and still looks like its agent. Nothing can
    // reach it (its stdio was the dead host's pipes). Read before
    // reconciliation, which clears the pid and worktree this needs.
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
                "agents started by a previous host are still running and cannot be reached"
            );
            *state.leaked_agents.lock().await = leaked;
        }
    }

    // Questions a driven run held when the host was killed, captured before
    // reconciliation clears `blocked_on`. Each gets a row with authority
    // `nobody`. A cleanly stopped host records `interrupted` instead and the
    // ask stays answerable; such a run is no longer live here.
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
                .because("the host did not stop cleanly, and the question went with it")
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

    // A change left mid-setup: the install died with the previous host and
    // the agent never started, so it reads as isolated with no setup report.
    let mid_setup: Vec<crate::core::Change> = {
        let mut changes = state.changes.lock().await;
        changes
            .values_mut()
            .filter(|c| matches!(c.waiting, Some(crate::core::Waiting::Setup { .. })))
            .map(|c| {
                c.waiting = None;
                c.updated_at = jiff::Timestamp::now();
                c.clone()
            })
            .collect()
    };
    for c in &mid_setup {
        tracing::info!(change = %c.id.as_str(), "the setup this change was waiting on died with the previous host");
        crate::change::persist(state, c).await;
    }
}

/// Probes the installed permission gate at startup and on every settings
/// change, and records whether it answered.
///
/// A broken gate and a quiet machine look the same in the event log; this
/// puts the difference in the inbox.
pub async fn gate_watch(state: Shared) {
    // No timer: only a `ConfigChange` hook can break it between probes.
    probe_gate_once(&state).await;
    let mut rx = state.tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(crate::core::Frame::Event(env)) => {
                if matches!(env.event, Event::ConfigChanged { .. }) {
                    probe_gate_once(&state).await;
                }
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

async fn probe_gate_once(state: &Shared) {
    let settings = crate::observe::connect::settings_path()
        .ok()
        .and_then(|p| crate::observe::connect::read_settings(&p).ok());
    let Some(settings) = settings else {
        return;
    };
    // Spawns a process and reads a file: keep it off the reactor.
    let probe =
        tokio::task::spawn_blocking(move || crate::observe::connect::probe_gate(&settings)).await;
    let Ok(probe) = probe else {
        return;
    };
    // Not installed is not broken; other surfaces already say to `connect`.
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

/// How often the tree of every open change is read (one `git status` each).
const TREE_EVERY: Duration = Duration::from_secs(30);

/// Reads the tree of every open change with a worktree, so *verified* follows
/// the tree rather than the last time an agent stopped.
///
/// A run ending or a gate finishing refresh it at once; this catches hand
/// edits no event announces. It writes only when the stamp changed.
pub async fn tree_watch(state: Shared) {
    loop {
        tokio::time::sleep(TREE_EVERY).await;
        tree_tick(&state).await;
    }
}

/// One pass of [`tree_watch`]: every open change's tree, read now.
pub async fn tree_tick(state: &Shared) {
    for id in crate::change::with_worktrees(state).await {
        crate::change::refresh_tree(state, &id).await;
    }
}

#[cfg(test)]
mod tests {

    /// `EPERM` means the process exists but is not ours. pid 1 always exists and
    /// is never signalable by a normal user.
    #[cfg(unix)]
    #[test]
    fn a_process_we_may_not_signal_is_still_alive() {
        assert!(
            process_alive(1),
            "pid 1 always exists; EPERM was being read as 'no such process'"
        );
        // Our own pid is the control.
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(0), "0 addresses a group, never a process");
    }

    /// The timer row names the project's own config file.
    #[test]
    fn the_deadline_row_names_the_project_s_own_file() {
        let p = crate::core::ProjectId::from_path(std::path::Path::new("/repos/saas"));
        let named = deadline_source(Some(&p));
        assert_eq!(named, "/repos/saas/devplane.toml");
        assert!(
            named.contains("/repos/saas"),
            "a person with six projects has to be told which one: {named}"
        );

        // Two projects must not produce the same answer.
        let q = crate::core::ProjectId::from_path(std::path::Path::new("/repos/mobile"));
        assert_ne!(deadline_source(Some(&p)), deadline_source(Some(&q)));

        // No project: the bare filename.
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

    /// A driven run never survives a restart: nothing drives it and no hook will
    /// correct its record.
    #[test]
    fn a_driven_run_is_never_still_running_after_a_restart() {
        assert!(
            !still_running(&run(RunMode::Driven), None),
            "its ACP pipes died with the host that held them"
        );
        // Not even with an empty roster or a live pid.
        let mut with_pid = run(RunMode::Driven);
        with_pid.pid = Some(std::process::id());
        assert!(!still_running(&with_pid, Some(&[])));
    }

    /// The vendor's roster naming the session outranks the rule.
    #[test]
    fn the_providers_roster_outranks_the_rule() {
        assert!(still_running(
            &run(RunMode::Driven),
            Some(&[run(RunMode::Driven).id.to_string()])
        ));
    }

    /// An observed session without a pid gets the benefit of the doubt: its own
    /// hooks correct the record, and calling it lost would fill the inbox with ghosts.
    #[test]
    fn an_observed_run_without_a_pid_is_given_the_benefit_of_the_doubt() {
        assert!(still_running(&run(RunMode::Observed), None));
        assert!(still_running(&run(RunMode::Observed), Some(&[])));
    }

    /// An observed run that reported a pid is checked against the machine.
    #[test]
    fn an_observed_run_with_a_dead_pid_is_not_still_running() {
        let mut r = run(RunMode::Observed);
        // Zero is not a process id.
        r.pid = Some(0);
        assert!(!still_running(&r, Some(&[])));
        r.pid = Some(std::process::id());
        assert!(still_running(&r, Some(&[])), "this test's own process");
    }
}

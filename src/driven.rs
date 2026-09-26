//! Runs Devplane owns, over the Agent Client Protocol.
//!
//! An observed session is one Devplane watches; a driven one is one it can
//! answer. Both share the board, the inbox and the policy engine: a permission
//! request is matched on the tool kind and arguments the agent declared, never
//! its title (see [`crate::acp::ToolRequest::policy_subject`]).

use crate::acp::{AcpEvent, AgentSpec, Session};
use crate::core::Verdict;
use crate::core::event::{ApiUsage, Choice, Event, Source, WaitingFor};
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::host::Shared;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// Starts an agent for a change, **without prompting it**.
///
/// The caller must register the run in `changes` and `change_of_run` before
/// prompting; otherwise a fast turn end reaches `change::on_turn_ended` before
/// the run is known and the change's gates never run.
pub async fn dispatch_for_change(
    state: &Shared,
    spec: &AgentSpec,
    cwd: PathBuf,
    change: &crate::core::ChangeId,
    sent: Vec<crate::spec::SentTask>,
) -> Result<RunId> {
    start(state, spec, cwd, None, Some(change), sent).await
}

/// Starts an agent on an ad-hoc prompt, with no change behind it; safe to
/// prompt inline.
pub async fn dispatch(
    state: &Shared,
    spec: &AgentSpec,
    cwd: PathBuf,
    prompt: Option<String>,
    sent: Vec<crate::spec::SentTask>,
) -> Result<RunId> {
    start(state, spec, cwd, prompt, None, sent).await
}

/// Starts an agent and registers it as a run. Every start comes through here,
/// so the trust and capacity checks cannot be skipped.
async fn start(
    state: &Shared,
    spec: &AgentSpec,
    cwd: PathBuf,
    prompt: Option<String>,
    for_change: Option<&crate::core::ChangeId>,
    // Recorded on the run before any prompt reaches the agent.
    sent: Vec<crate::spec::SentTask>,
) -> Result<RunId> {
    if !cwd.is_dir() {
        anyhow::bail!("{} is not a directory", cwd.display());
    }
    let cwd = cwd
        .canonicalize()
        .with_context(|| format!("resolving {}", cwd.display()))?;
    require_trust(state, &cwd).await?;
    require_capacity(state, &cwd, for_change).await?;
    // Resolved before spawning, so an unfillable placeholder refuses by name.
    let prompt = match prompt {
        Some(text) => Some(
            resolve_prompt(state, &cwd, &text, &sent)
                .await
                .map_err(cannot_send)?,
        ),
        None => None,
    };
    let keep_transcript = transcripts_wanted(&cwd);
    // Minted before the spawn (the agent is told it, see `agent_env`) so the
    // board has a row immediately. The group-leading children are read either
    // side of the spawn to record the agent's pid, so a killed host leaves
    // enough behind to find it (`observe::procs`); if ambiguous, nothing is
    // recorded.
    let run_id = RunId::new(format!("acp-{}", uuid::Uuid::now_v7().simple()));
    // Held until the session is registered, so a stop arriving meanwhile
    // waits for something to stop rather than finding nothing.
    let lock = lifecycle(&run_id);
    let owned = lock.lock().await;
    let before = crate::observe::procs::own_group_leading_children();
    let env = agent_env(state, for_change, &run_id).await;
    let (session, events) =
        crate::acp::spawn(spec, cwd.clone(), &env, mcp_offer(&run_id, for_change))
            .await
            .with_context(|| format!("starting {}", spec.id))?;
    let agent_pid = crate::observe::procs::one_new_child(
        &before,
        &crate::observe::procs::own_group_leading_children(),
    );

    state
        .ingest_as(
            run_id.clone(),
            Source::Host,
            Event::SessionStarted {
                cwd: cwd.clone(),
                source: Some("dispatch".into()),
                model: None,
                entrypoint: Some(format!("acp:{}", spec.id)),
                // Devplane set the environment and never sets the vendor's
                // question timer; a driven run's questions are durable asks.
                question_clock: None,
                clock_read: true,
            },
            crate::core::RunHint {
                cwd: Some(cwd.clone()),
                mode: RunMode::Driven,
                agent: spec.id.clone(),
                agent_command: Some(spec.command.clone()),
            },
        )
        .await;

    if let Some(pid) = agent_pid {
        state
            .ingest_as(
                run_id.clone(),
                Source::Host,
                Event::AgentProcessSpawned { pid },
                crate::core::RunHint {
                    cwd: Some(cwd.clone()),
                    mode: RunMode::Driven,
                    agent: spec.id.clone(),
                    agent_command: Some(spec.command.clone()),
                },
            )
            .await;
    }
    // Recorded only when the dispatch named tasks; never inferred.
    if !sent.is_empty() {
        state
            .ingest(
                run_id.clone(),
                Source::Host,
                Event::TasksSent { tasks: sent },
                Some(cwd.clone()),
                RunMode::Driven,
            )
            .await;
    }

    register(
        state,
        &run_id,
        &cwd,
        session.clone(),
        events,
        keep_transcript,
    )
    .await;
    drop(owned);

    if let Some(text) = prompt {
        say(state, &run_id, &session, text, keep_transcript).await?;
    }
    Ok(run_id)
}

/// The environment an agent Devplane starts is given: its run id in
/// [`RUN_ENV`], so `devplane report` reads an origin the host set rather than
/// one the model typed, plus its change's build-cache variables.
async fn agent_env(
    state: &Shared,
    for_change: Option<&crate::core::ChangeId>,
    run: &RunId,
) -> Vec<(String, PathBuf)> {
    let mut env = cache_env(state, for_change).await;
    env.push((RUN_ENV.to_string(), PathBuf::from(run.as_str())));
    env
}

/// The variable every agent Devplane starts carries its run id in.
pub const RUN_ENV: &str = "DEVPLANE_RUN";

/// The variable Devplane's MCP server reads the change it serves from.
pub const CHANGE_ENV: &str = "DEVPLANE_CHANGE";

/// Devplane's own read-only MCP server, as a stdio server the agent launches:
/// this executable's `mcp`, told the run and the change it belongs to. Every
/// ACP agent must accept stdio servers, so it is always offered; the agent's
/// own configured servers stay as they are.
fn mcp_offer(run: &RunId, change: Option<&crate::core::ChangeId>) -> Option<crate::acp::McpOffer> {
    let command = std::env::current_exe().ok()?;
    let mut env = vec![(RUN_ENV.to_string(), run.to_string())];
    if let Some(c) = change {
        env.push((CHANGE_ENV.to_string(), c.to_string()));
    }
    Some(crate::acp::McpOffer {
        name: "devplane".into(),
        command,
        args: vec!["mcp".into()],
        env,
    })
}

/// The lock every start, resume, stop and retire of one run takes, so exactly
/// one agent process is ever owned for it: two resumes, or a stop racing a
/// resume, run one after the other rather than interleaving.
fn lifecycle(run: &RunId) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    static LOCKS: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<RunId, std::sync::Arc<tokio::sync::Mutex<()>>>>,
    > = std::sync::LazyLock::new(Default::default);
    let mut locks = LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    // Entries nobody holds or waits on are dropped, so the map stays small.
    locks.retain(|_, l| std::sync::Arc::strong_count(l) > 1);
    locks.entry(run.clone()).or_default().clone()
}

/// Where a run's agent stands, read under its [`lifecycle`] lock.
enum Seat {
    /// No agent: one may be started.
    Vacant,
    /// A live agent that is not stopping.
    Live,
}

/// Waits out an agent that is stopping or whose pump is still writing its
/// ending, so the next one starts only after the last is gone.
async fn settle(state: &Shared, run: &RunId) -> Result<Seat> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let seat = state.sessions.lock().await.get(run).cloned();
        match seat {
            None => return Ok(Seat::Vacant),
            Some(s) if s.is_live() && !s.is_stopping() => return Ok(Seat::Live),
            Some(_) if std::time::Instant::now() >= deadline => {
                bail!("the run's previous agent has not gone yet; try again in a moment")
            }
            Some(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
        }
    }
}

/// The build-cache environment a change's project declared, so the agent
/// builds into the same cache as the setup command and gates. Read from the
/// governing checkout, never the worktree; empty without a change.
async fn cache_env(
    state: &Shared,
    for_change: Option<&crate::core::ChangeId>,
) -> Vec<(String, PathBuf)> {
    let Some(id) = for_change else {
        return Vec::new();
    };
    match crate::change::governing_config(state, id).await {
        Ok((root, config)) => crate::core::caches::env_for(&root, &config.workspace.share),
        Err(_) => Vec::new(),
    }
}

/// Continues a run whose agent is gone (e.g. after a host restart), against
/// the same agent-side session.
///
/// Never automatic: resuming spends money and runs an agent, so the inbox
/// offers it and a person (or `devplane change resume`) takes it.
pub async fn resume(state: &Shared, run: &RunId) -> Result<RunId> {
    resume_run(state, run, false).await?;
    Ok(run.clone())
}

/// [`resume`], serialised with every other start and stop of the run. With
/// `join`, an agent already live is used rather than refused; returns whether
/// a new one was started.
async fn resume_run(state: &Shared, run: &RunId, join: bool) -> Result<bool> {
    let lock = lifecycle(run);
    let _owned = lock.lock().await;
    let (agent_id, agent_command, agent_session, cwd, live) = {
        let world = state.world.lock().await;
        let r = world.run(run).context("no such run")?;
        (
            r.agent.clone(),
            r.agent_command.clone(),
            r.agent_session.clone(),
            r.working_dir().clone(),
            r.mode,
        )
    };
    if live != RunMode::Driven {
        bail!("that is a session Devplane watches, not one it drives");
    }
    match settle(state, run).await? {
        Seat::Live if join => return Ok(false),
        Seat::Live => bail!("that run already has an agent; there is nothing to resume"),
        Seat::Vacant => {}
    }
    let agent_session = agent_session.context(
        "that run never got as far as the agent naming its session, so there is nothing to \
         resume — start the change again instead",
    )?;

    // A resume goes through the same trust and capacity checks as a start.
    require_trust(state, &cwd).await?;
    let change = state.change_of_run.lock().await.get(run).cloned();
    require_capacity(state, &cwd, change.as_ref()).await?;

    // By id first, so a registry agent gets today's pinned version; otherwise
    // the recorded command, for an agent started by path (id `custom`).
    let spec = crate::acp::resolve(&agent_id, &state.agents)
        .or_else(|| {
            agent_command
                .as_deref()
                .map(|c| crate::acp::AgentSpec::new(&agent_id, &agent_id, c))
        })
        .with_context(|| {
            format!("`{agent_id}` is not an agent this machine knows how to start any more")
        })?;
    let keep_transcript = transcripts_wanted(&cwd);

    let env = agent_env(state, change.as_ref(), run).await;
    let (session, events) = crate::acp::resume(
        &spec,
        cwd.clone(),
        agent_session.clone(),
        &env,
        mcp_offer(run, change.as_ref()),
    )
    .await
    .with_context(|| format!("resuming {}", spec.id))?;

    register(state, run, &cwd, session, events, keep_transcript).await;

    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "run:resume",
                agent_session,
                "resumed",
            )
            .because(format!(
                "continued the `{agent_id}` session after a restart"
            ))
            .for_run(run),
        )
        .await;
    Ok(true)
}

/// Sends a prompt and records it as the transcript's opening line.
async fn say(
    state: &Shared,
    run: &RunId,
    session: &crate::acp::Session,
    text: String,
    keep_transcript: bool,
) -> Result<()> {
    // Recorded before it is sent, so a transcript starts with the ask.
    if keep_transcript {
        state
            .ingest_message(crate::core::Message::new(
                run.clone(),
                crate::core::Role::User,
                &text,
            ))
            .await;
    }
    session.prompt(text).await
}

/// Registers a live session and starts its pump. Shared by start and resume,
/// so both reach the board by the same path.
async fn register(
    state: &Shared,
    run_id: &RunId,
    cwd: &Path,
    session: crate::acp::Session,
    mut events: tokio::sync::mpsc::Receiver<crate::acp::AcpEvent>,
    keep_transcript: bool,
) {
    state
        .sessions
        .lock()
        .await
        .insert(run_id.clone(), session.clone());

    // One pump per run, translating the protocol into the board's vocabulary.
    let pump_state = state.clone();
    let pump_run = run_id.clone();
    let pump_cwd = cwd.to_path_buf();
    let pump_session = session;
    tokio::spawn(async move {
        let mut pump = Pump {
            keep_transcript,
            ..Pump::default()
        };
        while let Some(event) = events.recv().await {
            handle(
                &pump_state,
                &pump_run,
                &pump_cwd,
                &pump_session,
                &mut pump,
                event,
            )
            .await;
        }
        // Fallback for an agent that died without sending `Ended`. Only fires
        // if the run is still live, so it never overwrites an ending already
        // recorded (e.g. `interrupted` with `completed`).
        let still_live = {
            let w = pump_state.world.lock().await;
            w.run(&pump_run).is_some_and(|r| r.state.is_live())
        };
        if still_live {
            let reason = pump_state
                .tearing_down
                .load(std::sync::atomic::Ordering::SeqCst)
                .then(|| "interrupted".to_string());
            pump_state
                .ingest(
                    pump_run.clone(),
                    Source::Host,
                    Event::SessionEnded { reason },
                    Some(pump_cwd),
                    RunMode::Driven,
                )
                .await;
        }
        // Last, so an empty `sessions` map means every pump finished writing;
        // `shutdown()` waits on that. Only its own entry: a newer session for
        // the run is somebody else's to remove.
        let mut sessions = pump_state.sessions.lock().await;
        if sessions
            .get(&pump_run)
            .is_some_and(|s| s.same_as(&pump_session))
        {
            sessions.remove(&pump_run);
        }
    });
}

/// Fills a prompt's placeholders from what this host knows about `cwd`'s
/// project, or names those it could not fill.
///
/// The one place a prompt is resolved, so a preflight shows what is sent.
/// `sent` tasks go where the template names `{tasks}`, or are appended under
/// *Tasks:*.
pub async fn resolve_prompt(
    state: &Shared,
    cwd: &Path,
    text: &str,
    sent: &[crate::spec::SentTask],
) -> Result<String, Vec<crate::core::context::Unresolved>> {
    use crate::core::context::{Field, Value};
    let root = crate::repo::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    let project = crate::core::ProjectId::from_path(&root);
    let mut ctx = context_for(state, &project, &root).await;
    let block = crate::spec::tasks_block(sent);
    if !sent.is_empty() {
        ctx = ctx.with(
            Field::Tasks,
            Value::new(block.clone(), jiff::Timestamp::now()),
        );
    }
    let out = crate::core::context::resolve(text, &ctx)?;
    let out = if sent.is_empty() || crate::core::context::names(text, Field::Tasks) {
        out
    } else {
        format!("{}\n\nTasks:\n{block}\n", out.trim_end())
    };
    Ok(crate::core::context::with_stop_and_say(&out))
}

/// A prompt refused before anything started: the placeholders it names that
/// nothing could fill. Typed, so a route can answer it as the caller's error.
#[derive(Debug)]
pub struct CannotSend(pub Vec<crate::core::context::Unresolved>);

impl std::fmt::Display for CannotSend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the prompt cannot be sent: {}",
            self.0
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}

impl std::error::Error for CannotSend {}

/// The refusal a caller hands back when a prompt cannot be sent.
pub(crate) fn cannot_send(problems: Vec<crate::core::context::Unresolved>) -> anyhow::Error {
    anyhow::Error::new(CannotSend(problems))
}

/// What one project can answer, read off disk and the world (`core::context`
/// only resolves). A member with nothing behind it is absent, and the resolver
/// refuses on it rather than filling in an empty string.
pub async fn context_for(
    state: &Shared,
    project: &crate::core::ProjectId,
    root: &Path,
) -> crate::core::context::Context {
    use crate::core::context::{Context, Field, Value};
    let now = jiff::Timestamp::now();
    // A project-wide prompt belongs to no change, so it has no answered reports.
    let mut ctx = Context::default().with(
        Field::Reports,
        Value::new(crate::core::context::NO_REPORTS, now),
    );

    {
        let w = state.world.lock().await;
        if let Some(p) = w.project(project) {
            ctx = ctx.with(Field::Project, Value::new(p.name.clone(), now));
        }
    }

    if let Ok(st) = crate::git::status(root).await {
        // Absent when the repository has no branch yet.
        if let Some(b) = st.branch.clone() {
            ctx = ctx.with(Field::Branch, Value::new(b, now));
        }
        // A sentence, because the prompt tells an agent a fact.
        ctx = ctx.with(
            Field::Dirty,
            Value::new(
                match st.changed_files + st.untracked_files > 0 {
                    true => "the worktree has uncommitted changes",
                    false => "the worktree is clean",
                },
                now,
            ),
        );
    }
    // The declared base branch where there is one, as merge checks read it.
    let base = match crate::core::ProjectConfig::load(root)
        .ok()
        .and_then(|c| c.project.base_branch)
    {
        Some(b) => b,
        None => crate::git::base_branch(root).await,
    };
    ctx = ctx.with(Field::BaseBranch, Value::new(base, now));
    if let Ok(cfg) = crate::core::ProjectConfig::load(root) {
        // The same plan reader the surfaces use, so the counts agree.
        let changes = state.changes.lock().await;
        let in_flight = changes
            .values()
            .filter(|w| w.project_id == *project && !w.is_settled())
            .find_map(|w| w.spec.as_deref());
        if let Some(spec) = in_flight {
            let plan = crate::spec::Plan::read(root, spec, &cfg.spec.open_questions);
            ctx = ctx.with(Field::PlanPath, Value::new(plan.path.clone(), now));
            if let Some(p) = plan.progress {
                ctx = ctx.with(Field::PlanOpen, Value::new(p.open().to_string(), now));
            }
            if plan.open_questions > 0 {
                ctx = ctx.with(
                    Field::PlanQuestions,
                    Value::new(plan.open_questions.to_string(), now),
                );
            }
        }
    }

    // The last gate failures, as the stored report reduced them.
    {
        let changes = state.changes.lock().await;
        if let Some((failures, at)) = changes
            .values()
            .filter(|w| w.project_id == *project)
            .filter_map(|w| w.check_report().map(|g| (g, w.updated_at)))
            .filter(|(g, _)| !g.passed())
            .max_by_key(|(_, at)| *at)
            .map(|(g, at)| {
                let f: Vec<String> = g
                    .commands
                    .iter()
                    .flat_map(|c| c.failures.iter().cloned())
                    .collect();
                (f, at)
            })
            && !failures.is_empty()
        {
            ctx = ctx.with(Field::GateFailures, Value::new(failures.join("\n"), at));
        }
    }

    if let Ok(rows) = state.store.decisions(None, 1).await
        && let Some(d) = rows.first()
    {
        ctx = ctx.with(
            Field::LastDecision,
            Value::new(format!("{} {}", d.outcome, d.subject), d.at),
        );
        ctx = ctx.with(Field::LastAuthority, Value::new(d.authority.as_str(), d.at));
    }

    // A question an agent asked that nobody answered.
    {
        let w = state.world.lock().await;
        if let Some(q) = w
            .runs()
            .filter(|r| r.project_id.as_ref() == Some(project))
            .flat_map(|r| r.abandoned_questions.iter())
            .max_by_key(|q| q.abandoned_at)
        {
            ctx = ctx.with(
                Field::UnansweredQuestion,
                Value::new(q.question.clone(), q.abandoned_at),
            );
        }
    }

    ctx
}

/// Whether this repository keeps what its agents say. Read once at dispatch,
/// so a setting changed mid-turn cannot leave half a conversation on disk.
fn transcripts_wanted(cwd: &Path) -> bool {
    let root = crate::repo::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    crate::core::ProjectConfig::load(&root)
        .map(|c| c.transcripts.keep)
        // An unparseable file is reported elsewhere; default to keeping.
        .unwrap_or(true)
}

/// Refuses unless somebody has trusted this directory's project. A headless
/// agent runs the repository's own hooks and MCP servers with no dialog, so
/// trust is explicit; a worktree inherits its repository's trust.
pub async fn require_trust(state: &Shared, cwd: &Path) -> Result<()> {
    let root = crate::repo::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    let id = crate::core::ProjectId::from_path(&root);

    let trusted = {
        let w = state.world.lock().await;
        w.project(&id).map(|p| p.trusted).unwrap_or(false)
    };
    if trusted {
        return Ok(());
    }
    anyhow::bail!(
        "{} is not trusted yet. Starting an agent there runs that repository's own hooks \
         and MCP servers without asking. Run `devplane trust {}` if you meant to.",
        root.display(),
        root.display()
    )
}

/// Refuses when a project already has as many agents running as its
/// `max_parallel_runs` allows.
async fn require_capacity(
    state: &Shared,
    cwd: &Path,
    for_change: Option<&crate::core::ChangeId>,
) -> Result<()> {
    let root = crate::repo::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());

    let Ok(config) = crate::core::ProjectConfig::load(&root) else {
        return Ok(());
    };
    let Some(limit) = config.policy.max_parallel_runs else {
        return Ok(());
    };

    let id = crate::core::ProjectId::from_path(&root);
    let change_of_run = state.change_of_run.lock().await.clone();
    let live = {
        let w = state.world.lock().await;
        // Counts distinct changes among driven/background runs: sessions the
        // user started are not counted, and one change's runs share a slot.
        let mut seen = std::collections::HashSet::new();
        w.runs()
            .filter(|r| {
                matches!(
                    r.mode,
                    crate::core::RunMode::Driven | crate::core::RunMode::Background
                ) && r.project_id.as_ref() == Some(&id)
                    && r.state.is_live()
            })
            .filter(|r| match change_of_run.get(&r.id) {
                // The change asking already holds its slot.
                Some(change) if Some(change) == for_change => false,
                Some(change) => seen.insert(change.to_string()),
                // A bare `dispatch` is its own unit.
                None => seen.insert(r.id.to_string()),
            })
            .count()
    };
    if live >= limit {
        anyhow::bail!(
            "{} already has {live} change(s) with an agent in them and allows \
             {limit}. Finish one, or raise max_parallel_runs in devplane.toml.",
            root.display()
        );
    }
    Ok(())
}

/// Sends a prompt to a driven run.
///
/// Reports its change filed that were answered since it last heard are
/// appended (unless the prompt carries them via `{reports}`) and marked told
/// once the prompt has gone, so each verdict arrives once.
pub async fn prompt(state: &Shared, run: &RunId, text: String) -> Result<()> {
    send_prompt(state, run, text, true).await
}

/// [`prompt`], superseding what the agent waits on only when `supersede`: an
/// answer to an old ask, sent as a message, must not withdraw a permission
/// the agent asked for since. It queues behind the current turn instead.
async fn send_prompt(state: &Shared, run: &RunId, text: String, supersede: bool) -> Result<()> {
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Devplane drives")?;
    // A follow-up while the agent waits on an answer supersedes the wait: the
    // requests are answered *cancelled*, as nobody's decision, so the turn can
    // end and the prompt reach the agent.
    let waiting = match supersede {
        true => session.waiting().await,
        false => Vec::new(),
    };
    for request_id in waiting {
        let answered = match session.decide(&request_id, None).await {
            Ok(()) => Ok(()),
            Err(_) => session.answer(&request_id, None).await,
        };
        if answered.is_ok() {
            end_unanswered(
                state,
                run,
                &request_id,
                "cancelled",
                "a follow-up prompt was sent before anybody answered",
            )
            .await;
        }
    }
    let change = state.change_of_run.lock().await.get(run).cloned();
    let owed = match &change {
        Some(c) => crate::reports::owed(state, c).await,
        None => Vec::new(),
    };
    let fresh: Vec<&String> = owed
        .iter()
        .map(|(_, p)| p)
        .filter(|p| !text.contains(p.as_str()))
        .collect();
    let text = match fresh.is_empty() {
        true => text,
        false => format!(
            "{}\n\nWhat became of reports this change filed about other projects:\n\n{}\n",
            text.trim_end(),
            fresh
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join("\n\n")
        ),
    };
    state
        .ingest(
            run.clone(),
            Source::Host,
            Event::PromptSubmitted {
                chars: text.chars().count(),
            },
            None,
            RunMode::Driven,
        )
        .await;
    // Kept in the transcript, including handed-back gate failures. The
    // repository's setting is read per turn rather than stored on the run.
    let dir = {
        let w = state.world.lock().await;
        w.run(run).map(|r| r.working_dir().clone())
    };
    if dir.as_deref().map(transcripts_wanted).unwrap_or(true) {
        state
            .ingest_message(crate::core::Message::new(
                run.clone(),
                crate::core::Role::User,
                &text,
            ))
            .await;
    }
    session.prompt(text).await?;
    if let Some(c) = &change
        && !owed.is_empty()
    {
        let ids: Vec<crate::core::ReportId> = owed.into_iter().map(|(id, _)| id).collect();
        crate::reports::told(state, c, &ids).await;
    }
    Ok(())
}

/// What a caller asked for when answering a permission request.
///
/// `Allow`/`Deny` are resolved against the options the agent offered, because
/// ACP option ids belong to the agent (`allow`, `proceed_once`, ...).
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Allow,
    Deny,
    /// An exact option id, for a caller that read them off the inbox item.
    Option(String),
}

impl Decision {
    /// Reads `{ "decision": "allow" }`, or `{ "option_id": "…" }` for an exact
    /// choice. Nothing at all is a refusal.
    pub fn parse(decision: Option<&str>, option_id: Option<String>) -> Self {
        if let Some(id) = option_id {
            // The ids Devplane publishes for a held permission mean what they
            // say, rather than going through `Option`'s unresolved label.
            return match id.as_str() {
                "allow" => Decision::Allow,
                "deny" => Decision::Deny,
                _ => Decision::Option(id),
            };
        }
        match decision {
            Some("allow") => Decision::Allow,
            _ => Decision::Deny,
        }
    }

    /// The option id, where this decision is one.
    fn option_id(&self) -> Option<&str> {
        match self {
            Decision::Option(id) => Some(id.as_str()),
            _ => None,
        }
    }

    /// What this decision is called in the record. `Option` returns `None`:
    /// what an agent's option means is resolved from the offered list, never
    /// assumed to be `allow`.
    fn label(&self) -> Option<&'static str> {
        match self {
            Decision::Deny => Some("deny"),
            Decision::Allow => Some("allow"),
            Decision::Option(_) => None,
        }
    }
}

/// Whether an offered option is a standing grant or refusal (`allow_always`,
/// `reject_always`). A standing grant approves later calls inside the agent
/// with no request reaching Devplane, so the record must say so.
fn standing(kind: Option<&str>) -> bool {
    matches!(kind, Some("allow_always") | Some("reject_always"))
}

/// Delivers a person's answer to a question down a live connection. Reached
/// only through [`answer_ask`], so the answer is recorded first.
///
/// The form is the one recorded on the ask when the question arrived, and an
/// answer naming anything it did not offer was refused before the ask closed.
async fn answer_question(
    state: &Shared,
    ask: &crate::core::ask::Ask,
    chosen: &[(String, crate::core::question::Chosen)],
) -> Result<()> {
    let (run, request_id) = (&ask.run, ask.request_id.as_str());
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Devplane drives")?;
    // This ask's own form, never the run's current block: under parallel
    // questions that is another request's.
    let content = question_content(form_of(ask), chosen)?;
    session.answer(request_id, Some(content.clone())).await?;

    // The one ledger row whose authority is known rather than inferred; the
    // reason carries what was chosen.
    let chose = content
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "agent:question",
                request_id.to_string(),
                "answered",
            )
            .because(format!("a person answered: {chose}"))
            .for_run(run),
        )
        .await;
    // The answer clears the block; a turn that ends without a tool call would
    // otherwise leave the run waiting on an answered question.
    state
        .ingest(
            run.clone(),
            Source::Host,
            Event::QuestionAnswered {
                action: "accept".into(),
            },
            None,
            RunMode::Driven,
        )
        .await;
    Ok(())
}

// ── Answering by token ──────────────────────────────────────────────────────
//
// Everything below addresses an ask by its own opaque id, not by the session
// holding it, so an answer survives the session being gone (a restart).

/// What a person chose, as it crosses from a surface into the record.
#[derive(Debug, Clone)]
pub enum Answer {
    /// A permission: allow, deny, or an exact option the agent offered.
    Permission(Decision),
    /// A question: one option per field, or the person's own words.
    Question(Vec<(String, crate::core::question::Chosen)>),
}

/// One question's answer, as a surface posts it: an option the agent
/// offered, or the person's own words, under the field it was asked as.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldAnswer {
    pub field: String,
    #[serde(default)]
    pub option: Option<String>,
    #[serde(default)]
    pub custom: Option<String>,
}

impl FieldAnswer {
    fn chosen(&self) -> Result<crate::core::question::Chosen, String> {
        match (&self.custom, &self.option) {
            (Some(t), _) if !t.trim().is_empty() => {
                Ok(crate::core::question::Chosen::Custom(t.clone()))
            }
            (_, Some(o)) => Ok(crate::core::question::Chosen::Option(o.clone())),
            _ => Err(format!(
                "say what the answer to `{}` is: an option the agent offered, or your own words",
                self.field
            )),
        }
    }
}

/// What a surface said, turned into an [`Answer`] the row can take — or the
/// sentence saying why it cannot.
///
/// The one parser for every surface (API and host-less `devplane answer`): a
/// permission with nothing said is refused, and an option must be one the
/// request offered.
pub fn parse_answer(
    ask: &crate::core::ask::Ask,
    decision: Option<&str>,
    option: Option<String>,
    custom: Option<String>,
    field: Option<String>,
    answers: &[FieldAnswer],
) -> Result<Answer, String> {
    match ask.kind {
        crate::core::ask::Kind::Permission => {
            // Nothing said is malformed, not a deny: `Decision::parse` fails
            // closed, which is right for a gate and wrong for a person.
            if decision.is_none() && option.is_none() {
                return Err(
                    "say what the answer is: `decision` of \"allow\" or \"deny\", or \
                            `option` naming one the agent offered"
                        .into(),
                );
            }
            // One answer, said once. An option says the whole answer, so a
            // `decision` beside it is a contradiction to refuse rather than a
            // tie to break — breaking it let `--deny --option allow_always` allow.
            if decision.is_some() && option.is_some() {
                return Err("say the answer once: a `decision` or an `option`, not both".into());
            }
            // A field names one question among several; a permission has none,
            // and accepting one let an allow slip past the self-answer check.
            if field.is_some() {
                return Err("a permission has no fields; `field` is for a question".into());
            }
            if let Some(id) = option.as_deref() {
                let offered: Vec<String> = offered_options(&ask.payload)
                    .into_iter()
                    .filter_map(|o| o.id)
                    .collect();
                if !offered.is_empty() && !offered.iter().any(|o| o == id) {
                    return Err(format!(
                        "`{id}` is not one of the answers this request offers ({}). An answer has \
                         to name something that was asked.",
                        offered.join(", ")
                    ));
                }
            }
            Ok(Answer::Permission(Decision::parse(decision, option)))
        }
        crate::core::ask::Kind::Question => {
            // A form answers every field; a single question is a one-field form.
            let mut chosen: Vec<(String, crate::core::question::Chosen)> = Vec::new();
            for a in answers {
                chosen.push((a.field.clone(), a.chosen()?));
            }
            if chosen.is_empty() {
                let field = field.unwrap_or_else(|| "question_0".into());
                let one = FieldAnswer {
                    field,
                    option,
                    custom,
                };
                chosen.push((one.field.clone(), one.chosen()?));
            }
            Ok(Answer::Question(chosen))
        }
    }
}

/// Records a person's answer with no host to deliver it.
///
/// For a held permission the row is the delivery (the hook polls it). Anything
/// else is recorded as the person's and marked undelivered; a host can deliver
/// it into a resumed session later.
pub async fn answer_without_host(
    store: &crate::store::Store,
    ask_id: &str,
    answer: Answer,
    from: &str,
) -> Result<crate::core::ask::Ask> {
    use crate::core::ask::Delivery;

    // The answerer's own process: refuse an allow from inside an agent session.
    if allows(&answer)
        && let Some(var) = crate::hook::agent_session()
    {
        bail!("{}", crate::hook::refuse_self_answer(var));
    }
    // Close holds whose hook is gone, so answering one says so.
    crate::record::end_orphaned_holds(store).await;
    let mut ask = store
        .ask(ask_id)
        .await?
        .with_context(|| format!("no ask with id `{ask_id}`"))?;
    let recorded = recorded_answer(&answer);
    if let Err(refused) = ask.answer(recorded.clone(), from, jiff::Timestamp::now()) {
        anyhow::bail!("{}", refused.says());
    }
    // A held permission: the hook is waiting on this very row, and it stamps
    // the delivery once it has read the answer. Until then nothing has
    // reached the agent, and the row says so.
    let delivery = (!ask.request_id.is_empty()).then(|| Delivery::Undeliverable {
        because: "no host is running to deliver it; a host delivers it when the run is resumed"
            .into(),
    });
    ask.delivery = delivery.clone();
    // Only onto an open row; a surface that answered in between wins.
    if !store.close_ask(&ask).await? {
        bail!("{}", lost_the_race(store, ask_id, recorded, from).await);
    }
    store
        .append_decision(
            &crate::core::Decision::new(
                crate::core::Authority::Person,
                match ask.kind {
                    crate::core::ask::Kind::Permission => "agent:tool.use",
                    crate::core::ask::Kind::Question => "agent:question",
                },
                ask.message.clone(),
                "answered",
            )
            .because(format!(
                "answered from the {from} — {}",
                delivery.as_ref().map_or_else(
                    || "left for the waiting hook to read".to_string(),
                    |d| d.says()
                )
            ))
            .for_run(&ask.run),
        )
        .await?;
    Ok(ask)
}

/// Whether an answer could let a call through: any permission answer but a
/// plain refusal. An option is counted as allowing, because what an agent's
/// own option means is the agent's to say.
fn allows(answer: &Answer) -> bool {
    matches!(answer, Answer::Permission(d) if !matches!(d, Decision::Deny))
}

/// Why an answer that lost a compare-and-set was not recorded: the row as the
/// winner left it, in the sentence [`Ask::answer`](crate::core::ask::Ask::answer)
/// uses for a second answer.
async fn lost_the_race(
    store: &crate::store::Store,
    ask_id: &str,
    recorded: serde_json::Value,
    from: &str,
) -> String {
    match store.ask(ask_id).await {
        Ok(Some(mut now)) => match now.answer(recorded, from, jiff::Timestamp::now()) {
            Err(refused) => refused.says(),
            Ok(()) => "the question changed while it was being answered; try again".into(),
        },
        _ => format!("no ask with id `{ask_id}`"),
    }
}

/// What the person chose, in the shape the record keeps: `allow`/`deny` where
/// the caller said one, the option's own id otherwise, so the held-permission
/// path reads a value it published.
fn recorded_answer(answer: &Answer) -> serde_json::Value {
    match answer {
        Answer::Permission(d) => match d.label() {
            Some(l) => serde_json::json!({ "permission": l }),
            None => serde_json::json!({ "permission": d.option_id() }),
        },
        Answer::Question(choices) => serde_json::json!({
            "question": choices
                .iter()
                .map(|(f, c)| match c {
                    crate::core::question::Chosen::Option(v) =>
                        serde_json::json!({ "field": f, "option": v }),
                    crate::core::question::Chosen::Custom(t) =>
                        serde_json::json!({ "field": f, "custom": t }),
                })
                .collect::<Vec<_>>()
        }),
    }
}

/// Records a person's answer to an ask, then tries to deliver it.
///
/// Recorded first, so a crash replays as answered and racing surfaces are
/// harmless: the first wins, the second is told who did. Delivery is reported
/// separately ([`Delivery`](crate::core::ask::Delivery)): delivered live,
/// into a resumed session, or never.
pub async fn answer_ask(
    state: &Shared,
    ask_id: &str,
    answer: Answer,
    from: &str,
) -> Result<crate::core::ask::Ask> {
    use crate::core::ask::Ended;

    // Close holds whose hook is gone, so answering one says so.
    crate::record::end_orphaned_holds(&state.store).await;
    let mut ask = state
        .store
        .ask(ask_id)
        .await?
        .with_context(|| format!("no ask with id `{ask_id}`"))?;

    // Composed first, so a second answer is refused fully understood.
    let recorded = recorded_answer(&answer);
    // Refused before anything is written, never after the row is closed.
    if ask.answer.is_none() {
        check_answer(&ask, &answer)?;
    }

    if let Err(refused) = ask.answer(recorded.clone(), from, jiff::Timestamp::now()) {
        // A second answer: told what the first one chose.
        anyhow::bail!("{}", refused.says());
    }
    // Durable before the effect, and only onto an open row; nothing after this
    // may lose what the person said.
    if !state.store.close_ask(&ask).await? {
        bail!(
            "{}",
            lost_the_race(&state.store, ask_id, recorded, from).await
        );
    }

    let delivery = deliver(state, &ask, answer).await;
    ask.delivery = Some(delivery.clone());
    state
        .store
        .set_ask_delivery(ask.id.as_str(), &delivery)
        .await?;

    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                match ask.kind {
                    crate::core::ask::Kind::Permission => "agent:tool.use",
                    crate::core::ask::Kind::Question => "agent:question",
                },
                ask.message.clone(),
                "answered",
            )
            .because(format!("answered from the {from} — {}", delivery.says()))
            .for_run(&ask.run),
        )
        .await;

    // The row already records `person`; this only catches the log up.
    debug_assert!(matches!(ask.ended, Some(Ended::Person)));
    Ok(ask)
}

/// Gets the person's answer to the agent: down the live connection, into a
/// resumed session, or not at all — in which case the row says nobody got it.
async fn deliver(
    state: &Shared,
    ask: &crate::core::ask::Ask,
    answer: Answer,
) -> crate::core::ask::Delivery {
    use crate::core::ask::Delivery;

    // A held permission has no protocol request: the hook polls this row, so
    // the row is the delivery.
    if ask.request_id.is_empty() {
        return Delivery::Live;
    }
    let session = state
        .sessions
        .lock()
        .await
        .get(&ask.run)
        .cloned()
        .filter(|s| s.is_live() && !s.is_stopping());
    // A live session that is not waiting on this request (one resumed after a
    // restart) gets the answer as a message, like any resumed session.
    let waiting = match &session {
        Some(s) => s.waiting().await.contains(&ask.request_id),
        None => false,
    };
    if session.is_some() && !waiting {
        let text = resumed_answer_text(ask);
        return match send_prompt(state, &ask.run, text, false).await {
            Ok(()) => Delivery::Resumed,
            Err(e) => Delivery::Undeliverable {
                because: e.to_string(),
            },
        };
    }
    if session.is_some() {
        let sent = match answer {
            Answer::Permission(d) => decide(state, ask, d).await,
            Answer::Question(c) => answer_question(state, ask, &c).await,
        };
        return match sent {
            Ok(()) => Delivery::Live,
            Err(e) => Delivery::Undeliverable {
                because: e.to_string(),
            },
        };
    }

    // The agent is gone but its session is on the vendor's disk: resume it and
    // send the answer as a new message (the asking turn died with the process).
    // Not guarded: after `kill -9` the old agent is stuck on a dead pipe and
    // can never receive another prompt.
    // Joined rather than refused when another answer resumed it first.
    match resume_run(state, &ask.run, true).await {
        Err(e) => Delivery::Undeliverable {
            because: format!("the agent could not be resumed: {e}"),
        },
        Ok(_) => {
            let text = resumed_answer_text(ask);
            match prompt(state, &ask.run, text).await {
                Ok(()) => Delivery::Resumed,
                Err(e) => Delivery::Undeliverable {
                    because: e.to_string(),
                },
            }
        }
    }
}

/// What a resumed agent is told: one framing sentence from Devplane and the
/// person's own answer, never words composed on their behalf.
fn resumed_answer_text(ask: &crate::core::ask::Ask) -> String {
    let chosen = ask
        .answer
        .as_ref()
        .map(|a| match a.get("permission").and_then(|p| p.as_str()) {
            Some(p) => p.to_string(),
            None => a
                .get("question")
                .and_then(|q| q.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|i| {
                            i.get("option")
                                .or_else(|| i.get("custom"))
                                .and_then(|v| v.as_str())
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default(),
        })
        .unwrap_or_default();
    let when = ask
        .answered_at
        .map(|t| t.to_string())
        .unwrap_or_else(|| "just now".to_string());
    format!(
        "Answering the question you asked before this session was interrupted.\n\n\
         You asked: {}\n\
         The answer, given at {when}: {chosen}",
        ask.message
    )
}

/// The option a permission answer picks among what *this* request offered.
/// An id the agent never offered is refused, never recorded as an allow; no
/// id is the protocol's cancelled, the safe end.
fn pick_option(offered: &[Choice], want: &Decision) -> Result<Option<String>> {
    let choose = |want_allow: bool| -> Option<String> {
        let exact = if want_allow {
            "allow_once"
        } else {
            "reject_once"
        };
        offered
            .iter()
            .find(|o| o.kind.as_deref() == Some(exact))
            .or_else(|| {
                offered.iter().find(|o| {
                    if want_allow {
                        o.is_allow()
                    } else {
                        o.is_reject()
                    }
                })
            })
            .and_then(|o| o.id.clone())
    };
    Ok(match want {
        Decision::Option(id) => {
            if !offered.is_empty() && !offered.iter().any(|o| o.id.as_deref() == Some(id.as_str()))
            {
                let names: Vec<&str> = offered.iter().filter_map(|o| o.id.as_deref()).collect();
                anyhow::bail!(
                    "`{id}` is not one of the answers this request offers ({}). \
                     An answer has to name something that was asked.",
                    match names.is_empty() {
                        true => "none have ids".to_string(),
                        false => names.join(", "),
                    }
                );
            }
            Some(id.clone())
        }
        Decision::Allow => match choose(true) {
            Some(id) => Some(id),
            None => anyhow::bail!("that request offers nothing that means allow"),
        },
        Decision::Deny => choose(false),
    })
}

/// The form a question ask carried, as recorded when it arrived.
fn form_of(ask: &crate::core::ask::Ask) -> Vec<crate::core::question::Question> {
    ask.payload
        .get("form")
        .cloned()
        .and_then(|f| serde_json::from_value(f).ok())
        .unwrap_or_default()
}

/// The `content` an answer to `form` sends, refusing any answer that names a
/// field, an option or a free-text slot the agent did not ask for — so a
/// person is never told their answer went when part of it was dropped.
fn question_content(
    form: Vec<crate::core::question::Question>,
    chosen: &[(String, crate::core::question::Chosen)],
) -> Result<serde_json::Value> {
    use crate::core::question::Chosen;
    if form.is_empty() {
        anyhow::bail!("no question is waiting on that run");
    }
    for (field, c) in chosen {
        let Some(q) = form.iter().find(|q| &q.field == field) else {
            anyhow::bail!("`{field}` is not a question the agent asked");
        };
        match c {
            Chosen::Option(v) if !q.options.iter().any(|o| &o.value == v) => {
                anyhow::bail!("`{v}` is not one of the answers offered for `{field}`")
            }
            Chosen::Custom(_) if q.custom_field.is_none() => {
                anyhow::bail!("`{field}` takes one of its options, not free text")
            }
            _ => {}
        }
    }
    let ask = crate::core::question::Ask {
        session_id: String::new(),
        tool_call_id: None,
        message: String::new(),
        questions: form,
    };
    let content = ask.content(chosen);
    if content.as_object().is_none_or(|o| o.is_empty()) {
        anyhow::bail!("none of those answers matches what the agent asked");
    }
    Ok(content)
}

/// Whether `answer` can be delivered to what `ask` asked, checked before the
/// ask is closed: an answer refused after the close would leave the row
/// answered and the agent waiting for ever.
fn check_answer(ask: &crate::core::ask::Ask, answer: &Answer) -> Result<()> {
    match answer {
        // A held permission is read by the hook, which offers no options.
        Answer::Permission(_) if ask.request_id.is_empty() => Ok(()),
        Answer::Permission(d) => pick_option(&offered_options(&ask.payload), d).map(|_| ()),
        Answer::Question(c) => question_content(form_of(ask), c).map(|_| ()),
    }
}

/// Delivers a permission decision down a live connection. Reached only through
/// [`answer_ask`].
async fn decide(state: &Shared, ask: &crate::core::ask::Ask, want: Decision) -> Result<()> {
    let run = &ask.run;
    let request_id = ask.request_id.as_str();
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Devplane drives")?;

    // The options *this* request offered, as recorded when it arrived —
    // never the run's current block, which under parallel calls is another
    // request's.
    let offered = offered_options(&ask.payload);
    let option = pick_option(&offered, &want)?;
    // The chosen option's kind; a caller may have passed an exact id.
    let chosen_kind = option.as_ref().and_then(|id| {
        offered
            .iter()
            .find(|o| o.id.as_deref() == Some(id.as_str()))
            .and_then(|o| o.kind.clone())
    });
    let is_standing = standing(chosen_kind.as_deref());
    // The option's own `kind` decides what an `Option` meant; `allow` is never
    // the fallback.
    let decision = match (is_standing, want.label(), chosen_kind.as_deref()) {
        (true, Some("deny"), _) | (true, _, Some("reject_always")) => "reject_always",
        (true, _, _) => "allow_always",
        (false, Some(l), _) => l,
        (false, None, Some(k)) if k.starts_with("reject") => "deny",
        (false, None, Some(_)) => "allow",
        // An option with no kind: the record names it and claims nothing.
        (false, None, None) => "chosen",
    };
    session.decide(request_id, option).await?;

    // What the person read when they answered: this request's own title.
    let subject = match ask.message.is_empty() {
        true => request_id.to_string(),
        false => ask.message.clone(),
    };
    let mut record = crate::core::Decision::new(
        crate::core::Authority::Person,
        "agent:tool.use",
        subject.clone(),
        decision,
    )
    .for_run(run);
    if is_standing {
        // A standing grant's later effects happen inside the agent, unseen,
        // so the log says so explicitly.
        record = record.because(
            "a standing choice made by a person: the agent applies it to later matching calls itself, and Devplane sees no request for those",
        );
    }
    state.record(record).await;

    // The decision takes the item out of the inbox now, not on the agent's
    // next move (which after a "no" may never come).
    state
        .ingest(
            run.clone(),
            Source::Host,
            Event::PermissionDecided {
                tool: subject,
                decision: decision.into(),
                by: "human".into(),
                reason: None,
                context: None,
                call_id: None,
            },
            None,
            RunMode::Driven,
        )
        .await;
    reblock(state, run).await;
    Ok(())
}

/// Puts the next request the agent is still waiting on back in front of the
/// person, after an ending cleared the run's block: parallel calls park
/// several requests, and the run shows one at a time.
async fn reblock(state: &Shared, run: &RunId) {
    let Some(session) = state.sessions.lock().await.get(run).cloned() else {
        return;
    };
    let blocked = {
        let w = state.world.lock().await;
        w.run(run).is_some_and(|r| r.blocked_on.is_some())
    };
    if blocked {
        return;
    }
    let waiting = session.waiting().await;
    let Ok(open) = state.store.open_asks().await else {
        return;
    };
    let Some(next) = open
        .into_iter()
        .filter(|a| &a.run == run && waiting.contains(&a.request_id))
        .max_by_key(|a| a.asked_at)
    else {
        return;
    };
    let options = offered_options(&next.payload);
    let event = match next.kind {
        crate::core::ask::Kind::Permission => Event::Blocked {
            waiting_for: WaitingFor::Permission,
            message: Some(next.message.clone()),
            request_id: Some(next.request_id.clone()),
            ask: Some(next.id.0.clone()),
            options,
            call: next
                .payload
                .get("call")
                .cloned()
                .and_then(|c| serde_json::from_value(c).ok()),
            context: None,
        },
        crate::core::ask::Kind::Question => Event::QuestionAsked {
            question: next.message.clone(),
            options,
            ask: Some(next.id.0.clone()),
            request_id: Some(next.request_id.clone()),
            form: next.payload.get("form").cloned().filter(|f| !f.is_null()),
        },
    };
    state
        .ingest(run.clone(), Source::Host, event, None, RunMode::Driven)
        .await;
}

/// Ends one ask the agent is no longer waiting on — withdrawn by the agent,
/// or superseded by a follow-up prompt — as nobody's answer, and takes it out
/// of the inbox. `None` when no open ask carries that request.
async fn end_unanswered(
    state: &Shared,
    run: &RunId,
    request_id: &str,
    outcome: &str,
    because: &str,
) {
    let open = match state.store.open_asks().await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "could not read the open asks");
            return;
        }
    };
    let Some(mut ask) = open
        .into_iter()
        .find(|a| &a.run == run && a.request_id == request_id)
    else {
        return;
    };
    ask.end(
        crate::core::ask::Ended::Nobody {
            because: because.to_string(),
        },
        jiff::Timestamp::now(),
    );
    match state.store.close_ask(&ask).await {
        // A person answered in between: theirs stands.
        Ok(false) => return,
        Ok(true) => {}
        Err(e) => {
            tracing::error!(error = %e, ask = %ask.id, "could not close an ask");
            return;
        }
    }
    let (action, event) = match ask.kind {
        crate::core::ask::Kind::Permission => (
            "agent:tool.use",
            Event::PermissionDecided {
                tool: ask.message.clone(),
                decision: outcome.to_string(),
                by: "nobody".into(),
                reason: Some(because.to_string()),
                context: None,
                call_id: None,
            },
        ),
        crate::core::ask::Kind::Question => (
            "agent:question",
            Event::QuestionEnded {
                request_id: request_id.to_string(),
            },
        ),
    };
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Nobody,
                action,
                ask.message.clone(),
                outcome,
            )
            .because(because)
            .for_run(run),
        )
        .await;
    // Only the run's own block is cleared; a parallel request stays shown.
    let shown = {
        let w = state.world.lock().await;
        w.run(run)
            .and_then(|r| r.blocked_on.as_ref())
            .and_then(|b| b.request_id.clone())
    };
    if shown.as_deref() == Some(request_id) {
        state
            .ingest(run.clone(), Source::Host, event, None, RunMode::Driven)
            .await;
        reblock(state, run).await;
    }
}

/// Whether Devplane still has a session it can prompt for this run, asked
/// before offering anything that needs one.
pub async fn is_live(state: &Shared, run: &RunId) -> bool {
    state
        .sessions
        .lock()
        .await
        .get(run)
        .map(|s| s.is_live() && !s.is_stopping())
        .unwrap_or(false)
}

/// Stops a driven run because a person asked. The session is flagged so the
/// pump ends the run as `stopped` and closes its asks under the person's name.
pub async fn stop(state: &Shared, run: &RunId) -> Result<()> {
    let lock = lifecycle(run);
    let _owned = lock.lock().await;
    // Left in the map: the pump removes it once the agent has gone, so a
    // resume after this waits for that rather than starting a second agent.
    let session = state.sessions.lock().await.get(run).cloned();
    let Some(s) = session else {
        anyhow::bail!("that run is not one Devplane drives");
    };
    if s.is_stopping() {
        return Ok(());
    }
    // No turn end will record what the specification said, so do it here.
    crate::change::observe_spec(state, run).await;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "run:stop",
                run.to_string(),
                "stopped",
            )
            .because("a person stopped the run")
            .for_run(run),
        )
        .await;
    s.stop_because(STOPPED_BY_PERSON);
    Ok(())
}

/// Ends a driven run the change has moved past. Nobody decided anything: the
/// step finished and the chain no longer needs its agent.
pub async fn retire(state: &Shared, run: &RunId) -> Result<()> {
    let lock = lifecycle(run);
    let _owned = lock.lock().await;
    let session = state.sessions.lock().await.get(run).cloned();
    match session {
        Some(s) => {
            s.stop();
            Ok(())
        }
        None => anyhow::bail!("that run is not one Devplane drives"),
    }
}

/// What [`stop`] tells the session, and what the pump reads back to end the
/// run as a person's doing.
const STOPPED_BY_PERSON: &str = "stopped";

/// What to call a tool call on the board: the policy vocabulary (`Bash`,
/// `Edit`, ...) where the call maps into it, so the board and the audit log
/// agree; the agent's title otherwise.
fn tool_label(call: &crate::acp::ToolRequest) -> String {
    match call.policy_subject() {
        Some((tool, _)) => tool.to_string(),
        None => crate::core::text::clip(&call.title, 60),
    }
}

/// What this pump is holding between chunks: the running cost it has already
/// counted, and the fragment of transcript it is still joining together.
#[derive(Debug, Default)]
struct Pump {
    cost: f64,
    transcript: crate::core::Coalescer,
    /// Tool calls in flight by protocol id, with the label their row opened
    /// under; updates need not repeat the title.
    open_calls: std::collections::HashMap<String, String>,
    /// Whether this repository keeps transcripts, fixed when the agent starts.
    keep_transcript: bool,
    /// The diffs each call's content reported last, by call id: content is
    /// replaced whole on every update, so the last set is the call's own.
    /// Written to the log when the call ends; always empty when the
    /// repository keeps no transcript.
    diffs: std::collections::HashMap<String, Vec<crate::acp::ReportedDiff>>,
}

/// Writes the diffs one call reported to the run's log, each as the agent's
/// own claim, clipped to [`crate::acp::DIFF_KEPT_BYTES`] per side.
async fn keep_diffs(state: &Shared, run: &RunId, cwd: &Path, call_id: &str, pump: &mut Pump) {
    let Some(diffs) = pump.diffs.remove(call_id) else {
        return;
    };
    for d in diffs {
        let mut omitted: u64 = 0;
        let mut clip = |text: String| -> String {
            if text.len() <= crate::acp::DIFF_KEPT_BYTES {
                return text;
            }
            let mut end = crate::acp::DIFF_KEPT_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            omitted += (text.len() - end) as u64;
            text[..end].to_string()
        };
        let old_text = d.old_text.map(&mut clip);
        let new_text = clip(d.new_text);
        state
            .ingest(
                run.clone(),
                Source::Host,
                Event::EditReported {
                    call_id: call_id.to_string(),
                    path: d.path,
                    old_text,
                    new_text,
                    omitted_bytes: omitted,
                },
                Some(cwd.to_path_buf()),
                RunMode::Driven,
            )
            .await;
    }
}

/// Every call's diffs not yet written, for a turn or session that ended
/// before the call reported an ending.
async fn keep_all_diffs(state: &Shared, run: &RunId, cwd: &Path, pump: &mut Pump) {
    let calls: Vec<String> = pump.diffs.keys().cloned().collect();
    for call in calls {
        keep_diffs(state, run, cwd, &call, pump).await;
    }
}

/// The option to answer a permission request with: the exact kind if offered,
/// otherwise any of the same family. Never a literal id; ids are the agent's.
fn pick(
    options: &[crate::acp::PermissionOption],
    exact: &str,
    family: &str,
) -> Option<crate::acp::PermissionOption> {
    options
        .iter()
        .find(|o| o.kind == exact)
        .or_else(|| options.iter().find(|o| o.kind.starts_with(family)))
        .cloned()
}

/// Ends the fragment being joined and records it; called at every sentence
/// boundary (tool call, turn end, session end).
async fn flush_transcript(state: &Shared, run: &RunId, pump: &mut Pump) {
    let fragment = pump.transcript.take().filter(|_| pump.keep_transcript);
    if let Some((role, text)) = fragment {
        state
            .ingest_message(crate::core::Message::new(run.clone(), role, text))
            .await;
    }
}

/// Records one fragment, if this repository keeps them.
async fn keep(state: &Shared, run: &RunId, pump: &mut Pump, role: crate::core::Role, chunk: &str) {
    let ready = match pump.keep_transcript {
        true => pump.transcript.push(role, chunk),
        false => None,
    };
    if let Some((role, text)) = ready {
        state
            .ingest_message(crate::core::Message::new(run.clone(), role, text))
            .await;
    }
}

/// Writes down what an agent just asked, before anything else happens.
///
/// The ask row is the only thing that cannot be re-derived (run state and the
/// inbox are projections), so it is written first and a failure is loud. The
/// deadline comes from the project's `devplane.toml`, else
/// [`Never`](crate::core::ask::Deadline::Never).
async fn persist_ask(
    state: &Shared,
    run: &RunId,
    kind: crate::core::ask::Kind,
    request_id: &str,
    message: &str,
    payload: serde_json::Value,
) -> Option<crate::core::AskId> {
    let (cwd, project) = {
        let w = state.world.lock().await;
        let r = w.run(run)?;
        (r.working_dir().clone(), r.project_id.clone())
    };
    let deadline = deadline_for(&cwd);
    let ask = crate::core::ask::Ask::new(
        crate::core::AskId::new(uuid::Uuid::now_v7().simple().to_string()),
        run.clone(),
        crate::core::ask::Asked {
            kind,
            request_id: request_id.to_string(),
            message: message.to_string(),
            payload,
            at: jiff::Timestamp::now(),
            deadline,
        },
    )
    .in_project(project);

    if let Err(e) = state.store.save_ask(&ask).await {
        // An unrecorded question is lost on restart: say so now.
        tracing::error!(error = %e, run = %run, "could not write down what the agent asked");
        // Recorded where `doctor` reads it.
        let _ = state
            .store
            .record_channel("asks", 0, Some(&e.to_string()))
            .await;
        return None;
    }
    Some(ask.id)
}

/// Ends one ask on the project's own clock: the agent is told, the row is
/// written, and the person is shown. A clock ending a question is a decision
/// taken on someone's behalf, so it must be visible, not just logged.
pub async fn expire(state: &Shared, ask: &crate::core::ask::Ask, ended: &crate::core::ask::Ended) {
    // Tell the blocked agent *no*, with its own reject option. `None` is the
    // protocol's *cancelled*, which the Claude adapter treats as an aborted
    // turn, and is recorded as that.
    let reject = reject_option(&ask.payload);
    let permission_outcome = match reject {
        Some(_) => "deny",
        None => "cancelled",
    };
    if let Some(session) = state.sessions.lock().await.get(&ask.run).cloned() {
        match ask.kind {
            crate::core::ask::Kind::Permission => {
                let _ = session.decide(&ask.request_id, reject.clone()).await;
            }
            crate::core::ask::Kind::Question => {
                // `None` is *cancel*, never *decline*: the adapter reads a
                // decline as "answered with nothing" and the agent proceeds.
                let _ = session.answer(&ask.request_id, None).await;
            }
        }
    }

    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Timer,
                match ask.kind {
                    crate::core::ask::Kind::Permission => "agent:tool.use",
                    crate::core::ask::Kind::Question => "agent:question",
                },
                ask.message.clone(),
                match ask.kind {
                    crate::core::ask::Kind::Permission => permission_outcome,
                    crate::core::ask::Kind::Question => "unanswered",
                },
            )
            .because(ended.says())
            .for_run(&ask.run),
        )
        .await;

    // `Source::Host` names the process that observed this, not whose clock it
    // was; the authority (the project's timer) is on the decision row above.
    let event = match ask.kind {
        crate::core::ask::Kind::Permission => Event::PermissionDecided {
            tool: ask.message.clone(),
            decision: permission_outcome.into(),
            by: "timer".into(),
            reason: None,
            context: None,
            call_id: None,
        },
        crate::core::ask::Kind::Question => Event::QuestionEnded {
            request_id: ask.request_id.clone(),
        },
    };
    state
        .ingest(
            ask.run.clone(),
            crate::core::Source::Host,
            event,
            None,
            crate::core::RunMode::Driven,
        )
        .await;
}

/// The options an ask's payload carries, as Devplane wrote them there: one
/// parse for every reader.
fn offered_options(payload: &serde_json::Value) -> Vec<Choice> {
    payload
        .get("options")
        .cloned()
        .and_then(|o| serde_json::from_value(o).ok())
        .unwrap_or_default()
}

/// The option a recorded permission request offered for refusing, by id: the
/// exact `reject_once` where the agent has it, any reject otherwise.
fn reject_option(payload: &serde_json::Value) -> Option<String> {
    let offered = offered_options(payload);
    offered
        .iter()
        .find(|o| o.kind.as_deref() == Some("reject_once"))
        .or_else(|| offered.iter().find(|o| o.is_reject()))
        .and_then(|o| o.id.clone())
}

/// Closes every ask still open on one run, with an authority on each. Never
/// overwrites an ending (`Ask::end` enforces it), so a person who just answered
/// keeps the credit.
async fn end_open_asks(state: &Shared, run: &RunId, ended: crate::core::ask::Ended) {
    let open = match state.store.open_asks().await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "could not read the open asks");
            return;
        }
    };
    let now = jiff::Timestamp::now();
    for mut ask in open.into_iter().filter(|a| &a.run == run) {
        ask.end(ended.clone(), now);
        // Only onto a row still open.
        if let Err(e) = state.store.close_ask(&ask).await {
            tracing::error!(error = %e, ask = %ask.id, "could not close an ask");
        }
    }
}

/// The deadline this repository sets for an unanswered ask. Absent or
/// unparseable means it waits: a typo must never end a question early
/// (`check` reports it).
fn deadline_for(cwd: &Path) -> crate::core::ask::Deadline {
    let root = crate::repo::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    crate::core::ProjectConfig::load(&root)
        .ok()
        .and_then(|c| c.questions.deadline())
        .unwrap_or(crate::core::ask::Deadline::Never)
}

/// The command that started this run, recorded at spawn; the stable key for
/// which agent this is.
async fn agent_command(state: &Shared, run: &RunId) -> Option<String> {
    state.world.lock().await.run(run)?.agent_command.clone()
}

/// Translates one protocol event, applying policy to permission requests.
async fn handle(
    state: &Shared,
    run: &RunId,
    cwd: &Path,
    session: &Session,
    pump: &mut Pump,
    event: AcpEvent,
) {
    let ingest = |e: Event| {
        let state = state.clone();
        let run = run.clone();
        let cwd = cwd.to_path_buf();
        async move {
            state
                .ingest(run, Source::Host, e, Some(cwd), RunMode::Driven)
                .await;
        }
    };

    match event {
        // The cross-vendor source for which sessions decide without you.
        AcpEvent::ModeChanged { mode } => {
            // Seen at session creation, so it amends the capability record.
            if let Some(cmd) = agent_command(state, run).await
                && let Err(e) = state.store.note_agent_declares_modes(&cmd).await
            {
                tracing::debug!(error = %e, "could not note that this agent declares modes");
            }
            ingest(Event::AgentModeSeen { mode }).await;
        }

        // A roster only fills gaps: `SessionInfo` carries no state.
        AcpEvent::SessionsListed { sessions } => {
            for listed in sessions {
                ingest(Event::SessionListed {
                    agent_session: listed.agent_session,
                    title: listed.title,
                })
                .await;
            }
        }

        AcpEvent::Capabilities {
            agent_name,
            resume,
            load_session,
            list_sessions,
            needs_auth,
        } => {
            // Keyed on the command recorded at spawn.
            if let Some(command) = agent_command(state, run).await {
                let record = crate::core::AgentCapabilityRecord {
                    command,
                    agent_name,
                    resume,
                    load_session,
                    list_sessions,
                    // Amended by `ModeChanged`, which is a later observation.
                    declares_modes: false,
                    needs_auth,
                    measured_at: jiff::Timestamp::now(),
                };
                if let Err(e) = state.store.save_agent_capabilities(&record).await {
                    tracing::debug!(error = %e, "could not record what this agent supports");
                }
            }
        }

        AcpEvent::Ready {
            agent_name,
            session_id,
        } => {
            ingest(Event::SessionStarted {
                cwd: cwd.to_path_buf(),
                source: Some("acp".into()),
                model: agent_name,
                entrypoint: None,
                question_clock: None,
                clock_read: true,
            })
            .await;
            // The id the agent answers `session/resume` on.
            ingest(Event::AgentSessionOpened {
                agent_session: session_id,
            })
            .await;
        }

        // A driven run's transcript, joined into fragments and kept out of the
        // event log (run state is a reduction over it).
        AcpEvent::Text(chunk) | AcpEvent::Thought(chunk) if chunk.is_empty() => {
            let _ = chunk;
        }
        AcpEvent::Text(chunk) => keep(state, run, pump, crate::core::Role::Agent, &chunk).await,
        AcpEvent::Thought(chunk) => {
            keep(state, run, pump, crate::core::Role::Thought, &chunk).await
        }

        // Structured and low-volume, so it belongs on the board.
        AcpEvent::Plan(steps) => {
            flush_transcript(state, run, pump).await;
            ingest(Event::PlanUpdated {
                steps: steps
                    .into_iter()
                    .map(|s| crate::core::PlanStep {
                        content: s.content,
                        status: s.status,
                    })
                    .collect(),
            })
            .await;
        }

        // A tool call ends the sentence before it.
        AcpEvent::Tool { call, status } => {
            flush_transcript(state, run, pump).await;
            // The protocol upserts by id, and updates need not repeat the
            // title: the row opens once under the first label and closes on id.
            let terminal = matches!(
                status.as_deref(),
                Some("completed" | "failed" | "cancelled")
            );
            let known = pump.open_calls.contains_key(&call.id);
            if !known {
                // Labelled for people; the arguments are what rules matched.
                let label = tool_label(&call);
                pump.open_calls.insert(call.id.clone(), label.clone());
                ingest(Event::ToolStarted {
                    tool: label,
                    input: call.raw_input.clone().unwrap_or(serde_json::Value::Null),
                    // The protocol does not say where a server came from.
                    server_source: None,
                    agent_id: None,
                    call_id: Some(call.id.clone()),
                })
                .await;
            }
            if terminal {
                keep_diffs(state, run, cwd, &call.id, pump).await;
            }
            if terminal && let Some(label) = pump.open_calls.remove(&call.id) {
                ingest(Event::ToolFinished {
                    tool: label,
                    ok: status.as_deref() == Some("completed"),
                    duration_ms: None,
                    call_id: Some(call.id),
                })
                .await;
            }
        }

        AcpEvent::PermissionRequested {
            request_id,
            call,
            options,
        } => {
            let title = call.title.clone();
            // The same rules and vocabulary as a hook. A call that cannot be
            // classified is left to a person.
            let verdict = match call.policy_subject() {
                Some((tool, input)) => state.policy.restrictive(cwd, tool, &input),
                None => Verdict::Undecided,
            };

            match verdict {
                Verdict::Deny { rule } => {
                    let _ = session
                        .decide(
                            &request_id,
                            pick(&options, "reject_once", "reject").map(|o| o.id),
                        )
                        .await;
                    state
                        .record(
                            crate::core::Decision::new(
                                crate::core::Authority::Rule,
                                "agent:tool.use",
                                title.clone(),
                                "deny",
                            )
                            .because(&rule)
                            .for_run(run),
                        )
                        .await;
                    ingest(Event::PermissionDecided {
                        tool: title,
                        decision: "deny".into(),
                        by: format!("policy:{rule}"),
                        reason: None,
                        context: None,
                        call_id: None,
                    })
                    .await;
                    return;
                }
                // Ask, Unresolved and Undecided all go to a person: a driven
                // permission request already is one.
                Verdict::Ask { .. } | Verdict::Unresolved { .. } | Verdict::Undecided => {}
            }

            // A person decides, from the inbox, naming one of these option ids.
            let choices: Vec<Choice> = options
                .into_iter()
                .map(|o| Choice {
                    id: Some(o.id),
                    label: o.label,
                    kind: Some(o.kind),
                })
                .collect();
            // This request's own call, so the inbox shows what *it* asks and
            // not whichever call started last.
            let asked_call = crate::core::event::ToolCallRef {
                tool: tool_label(&call),
                input: call.raw_input.clone().unwrap_or(serde_json::Value::Null),
            };
            // The ask row first; everything after it is a rebuildable projection.
            let token = persist_ask(
                state,
                run,
                crate::core::ask::Kind::Permission,
                &request_id,
                &title,
                serde_json::json!({ "options": choices, "call": asked_call }),
            )
            .await;
            let Some(token) = token else {
                // Unrecorded, nothing could ever address it: refused now
                // rather than left to hang the agent.
                let _ = session.decide(&request_id, None).await;
                return;
            };
            ingest(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: Some(title),
                request_id: Some(request_id),
                ask: Some(token.0),
                options: choices,
                call: Some(asked_call),
                context: None,
            })
            .await;
        }

        AcpEvent::Usage {
            cost_usd,
            context_tokens,
            context_window,
        } => {
            // The protocol reports session running totals; the reducer adds
            // per-request figures, so only the delta is ingested.
            let (cost_delta, tokens) = {
                let cost = cost_usd.unwrap_or(0.0);
                let delta = (cost - pump.cost).max(0.0);
                pump.cost = pump.cost.max(cost);
                (delta, context_tokens.unwrap_or(0))
            };
            // `used` is the window's level, not a per-request count, so it is
            // reported as a level rather than summed as `input_tokens`.
            ingest(Event::ApiRequest {
                usage: ApiUsage {
                    model: None,
                    cost_usd: cost_delta,
                    context_level: Some(tokens),
                    ..Default::default()
                },
            })
            .await;
            if let Some(window) = context_window {
                state.set_context_window(run, window).await;
            }
        }

        AcpEvent::TurnEnded { stop_reason } => {
            flush_transcript(state, run, pump).await;
            keep_all_diffs(state, run, cwd, pump).await;
            match stop_reason.as_str() {
                "refusal" => {
                    ingest(Event::TurnFailed {
                        message: "the agent refused to continue".into(),
                    })
                    .await
                }
                _ => ingest(Event::TurnEnded).await,
            }
            // Check the change's claim off this task, so a slow gate does not
            // queue the agent's next events.
            let state = state.clone();
            let run = run.clone();
            tokio::spawn(async move { crate::change::on_turn_ended(&state, &run).await });
        }

        // Not a permission: no rule consults it; it goes to the person.
        AcpEvent::QuestionAsked { request_id, ask } => {
            let options = ask
                .questions
                .first()
                .map(|q| {
                    q.options
                        .iter()
                        .map(|o| crate::core::event::Choice {
                            id: Some(o.value.clone()),
                            label: o.label.clone(),
                            kind: None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let form = serde_json::to_value(&ask.questions).ok();
            let token = persist_ask(
                state,
                run,
                crate::core::ask::Kind::Question,
                &request_id,
                &ask.message,
                serde_json::json!({ "form": form, "options": options }),
            )
            .await;
            ingest(Event::QuestionAsked {
                question: ask.message.clone(),
                options,
                ask: token.map(|a| a.0),
                request_id: Some(request_id),
                form,
            })
            .await;
        }

        // A form Devplane cannot render: the agent was told no, and the person
        // is told too.
        AcpEvent::QuestionUnrenderable { what } => {
            state
                .record(
                    crate::core::Decision::new(
                        crate::core::Authority::Devplane,
                        "agent:question",
                        run.to_string(),
                        "unrenderable",
                    )
                    .because("the agent asked in a form Devplane cannot show")
                    .for_run(run),
                )
                .await;
            ingest(Event::TurnFailed {
                message: format!(
                    "the agent asked something Devplane cannot show, so it was cancelled: {what}"
                ),
            })
            .await;
        }

        // Cancelled or ended under it: the inbox stops offering an answer.
        AcpEvent::QuestionCancelled { request_id } => {
            ingest(Event::QuestionEnded { request_id }).await;
        }

        // The agent withdrew what it asked: nobody answered, and the inbox
        // stops offering it.
        AcpEvent::PermissionWithdrawn { request_id }
        | AcpEvent::QuestionWithdrawn { request_id } => {
            end_unanswered(
                state,
                run,
                &request_id,
                "withdrawn",
                "the agent withdrew the request before anybody answered",
            )
            .await;
        }

        // A reported diff is file content the agent said: transcript content,
        // so a repository that keeps no transcript keeps none of it.
        AcpEvent::Diffs { call_id, diffs } if pump.keep_transcript => {
            pump.diffs.insert(call_id, diffs);
        }
        AcpEvent::Diffs { .. } => {}

        AcpEvent::Closed { sent } => {
            tracing::info!(run = %run, closed = sent, "the agent's session ended");
        }

        AcpEvent::Ended { error } => {
            flush_transcript(state, run, pump).await;
            keep_all_diffs(state, run, cwd, pump).await;
            // A waiting question must not end under `completed`. If the host
            // is tearing down, the vendor session can be resumed, so the ask
            // stays open and answerable after the restart; otherwise its asks
            // are closed first so the ending cannot overwrite them.
            let tearing_down = state.tearing_down.load(std::sync::atomic::Ordering::SeqCst);
            let by_person = session.stopped_because().as_deref() == Some(STOPPED_BY_PERSON);
            if !tearing_down {
                end_open_asks(
                    state,
                    run,
                    match by_person {
                        true => crate::core::ask::Ended::Stopped,
                        false => crate::core::ask::Ended::Nobody {
                            because: "the run ended before anybody answered".into(),
                        },
                    },
                )
                .await;
            }
            let unanswered = {
                let w = state.world.lock().await;
                w.run(run)
                    .and_then(|r| r.blocked_on.as_ref())
                    .filter(|b| b.waiting_for == crate::core::WaitingFor::Question)
                    .and_then(|b| b.request_id.clone())
            };
            if let Some(request_id) = unanswered.filter(|_| !tearing_down) {
                // Nobody decided, unless a person stopped the run under it.
                let (authority, because) = match by_person {
                    true => (
                        crate::core::Authority::Person,
                        "a person stopped the run without answering",
                    ),
                    false => (
                        crate::core::Authority::Nobody,
                        "the run ended before anybody answered",
                    ),
                };
                state
                    .record(
                        crate::core::Decision::new(
                            authority,
                            "agent:question",
                            request_id.clone(),
                            "unanswered",
                        )
                        .because(because)
                        .for_run(run),
                    )
                    .await;
                ingest(Event::QuestionEnded { request_id }).await;
            }
            match error {
                Some(e) => ingest(Event::TurnFailed { message: e }).await,
                // A host stopping is not the agent finishing: `interrupted`,
                // never `completed`.
                None if tearing_down => {
                    ingest(Event::SessionEnded {
                        reason: Some("interrupted".into()),
                    })
                    .await
                }
                None if by_person => {
                    ingest(Event::SessionEnded {
                        reason: Some(STOPPED_BY_PERSON.into()),
                    })
                    .await
                }
                None => ingest(Event::SessionEnded { reason: None }).await,
            }
        }
    }
}

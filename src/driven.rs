//! Runs Devplane owns, over the Agent Client Protocol.
//!
//! An observed session is something Devplane watches; a driven one is
//! something it can answer. The difference on the board is one word, which is
//! the point — the same run, the same inbox, the same project grouping,
//! whether the session was started here or in somebody's terminal.
//!
//! The policy engine is shared deliberately: "what may an agent do here" is a
//! property of the project, not of how the agent was launched. Sharing it means
//! speaking its vocabulary rather than approximating it, so a permission
//! request is matched on the tool kind the agent declared and the arguments it
//! chose — never on its human-readable title. See
//! [`crate::acp::ToolRequest::policy_subject`].

use crate::acp::{AcpEvent, AgentSpec, Session};
use crate::core::Verdict;
use crate::core::event::{ApiUsage, Choice, Event, Source, WaitingFor};
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::daemon::Shared;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// Starts an agent for a piece of work, **without prompting it**.
///
/// The separation is the point, and it is load-bearing. A prompt sent from
/// inside `dispatch` goes out before the caller has put this run into
/// `works` and `work_of_run`; an agent that answers quickly then ends its turn
/// before those maps can answer for it, `work::on_turn_ended` looks the run up,
/// finds nothing, and returns — leaving the work in `Implement` for ever with
/// its gates never run. That is the one failure the verified-done loop exists
/// to prevent, and it was reproducible by widening the window to 1.5s.
///
/// So a work-bound caller gets a run it must register first and prompt second,
/// and there is no parameter here with which to do it in the wrong order.
pub async fn dispatch_for_work(
    state: &Shared,
    spec: &AgentSpec,
    cwd: PathBuf,
    work: &crate::core::WorkId,
) -> Result<RunId> {
    start(state, spec, cwd, None, Some(work)).await
}

/// Starts an agent on an ad-hoc prompt, with no piece of work behind it.
///
/// Safe to prompt inline: there is no Work row whose absence a turn end could
/// fall through.
pub async fn dispatch(
    state: &Shared,
    spec: &AgentSpec,
    cwd: PathBuf,
    prompt: Option<String>,
) -> Result<RunId> {
    start(state, spec, cwd, prompt, None).await
}

/// Starts an agent and registers it as a run.
///
/// Every path that starts an agent comes through here, which is where the trust
/// gate belongs. Putting it only in `work::start` left `dispatch` as a way
/// around it — a check that one caller can skip is not a check.
async fn start(
    state: &Shared,
    spec: &AgentSpec,
    cwd: PathBuf,
    prompt: Option<String>,
    for_work: Option<&crate::core::WorkId>,
) -> Result<RunId> {
    if !cwd.is_dir() {
        anyhow::bail!("{} is not a directory", cwd.display());
    }
    let cwd = cwd
        .canonicalize()
        .with_context(|| format!("resolving {}", cwd.display()))?;
    require_trust(state, &cwd).await?;
    require_capacity(state, &cwd, for_work).await?;
    let keep_transcript = transcripts_wanted(&cwd);
    // Read this daemon's own group-leading children either side of the spawn.
    // The one that appears is the agent, and it is recorded so that a daemon
    // **killed** rather than stopped leaves enough behind to find what it
    // abandoned (`observe::procs`). Purely additive: if the reading fails, or
    // two dispatches race closely enough to be ambiguous, nothing is recorded
    // and everything else works exactly as before.
    let before = crate::observe::procs::own_group_leading_children();
    let (session, events) = crate::acp::spawn(spec, cwd.clone())
        .await
        .with_context(|| format!("starting {}", spec.id))?;
    let agent_pid = crate::observe::procs::one_new_child(
        &before,
        &crate::observe::procs::own_group_leading_children(),
    );

    // The run id is minted here rather than taken from the agent: the board
    // needs a row the moment the process starts, and the agent's own session id
    // only arrives after its handshake.
    let run_id = RunId::new(format!("acp-{}", uuid::Uuid::now_v7().simple()));

    state
        .ingest_as(
            run_id.clone(),
            Source::Daemon,
            Event::SessionStarted {
                cwd: cwd.clone(),
                source: Some("dispatch".into()),
                model: None,
                entrypoint: Some(format!("acp:{}", spec.id)),
                // **Read, and there is nothing to find.** Devplane spawned this
                // agent, so it knows the environment it was given, and a test
                // asserts nothing in this tree ever sets the variable. A driven
                // run's questions are durable `asks` rows that wait; the
                // vendor's own dialog — which is what that clock closes — is not
                // what asks here.
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
                Source::Daemon,
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

    register(
        state,
        &run_id,
        &cwd,
        session.clone(),
        events,
        keep_transcript,
    )
    .await;

    if let Some(text) = prompt {
        say(state, &run_id, &session, text, keep_transcript).await?;
    }
    Ok(run_id)
}

/// Continues a run whose agent is gone, against the same agent-side session.
///
/// The case this exists for is a daemon restart. The Work row, the branch and
/// the worktree all survive one; the agent process does not, and until now the
/// only honest thing the inbox could say was that the work was interrupted.
///
/// Deliberately not automatic on startup. Resuming spends money and runs an
/// agent in a repository, and doing either because a machine rebooted is a
/// decision nobody made — so the inbox offers it and a person (or `devplane
/// work resume`) takes it.
pub async fn resume(state: &Shared, run: &RunId) -> Result<RunId> {
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
    if state.sessions.lock().await.contains_key(run) {
        bail!("that run already has an agent; there is nothing to resume");
    }
    let agent_session = agent_session.context(
        "that run never got as far as the agent naming its session, so there is nothing to \
         resume — start the work again instead",
    )?;

    // The same gates a first dispatch goes through. A restart is not a reason
    // to skip the trust check or the project's parallelism limit.
    require_trust(state, &cwd).await?;
    let work = state.work_of_run.lock().await.get(run).cloned();
    require_capacity(state, &cwd, work.as_ref()).await?;

    // The id first, so a registry agent picks up whatever version is pinned
    // today; the recorded command second, which is the only thing that can
    // start an agent dispatched by path — its id is the placeholder `custom`
    // and names nothing.
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

    let (session, events) = crate::acp::resume(&spec, cwd.clone(), agent_session.clone())
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
    Ok(run.clone())
}

/// Sends a prompt and records it as the transcript's opening line.
async fn say(
    state: &Shared,
    run: &RunId,
    session: &crate::acp::Session,
    text: String,
    keep_transcript: bool,
) -> Result<()> {
    // Recorded before it is sent, so a transcript that is read back starts
    // with the ask rather than with the answer to something invisible.
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

/// Registers a live session and starts the one pump that belongs to it.
///
/// Shared by the first dispatch and by a resume, because a resumed run has to
/// reach the board through exactly the same path — a second copy of this is a
/// second place for the two to drift.
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
        let pump = tokio::sync::Mutex::new(Pump {
            keep_transcript,
            ..Pump::default()
        });
        while let Some(event) = events.recv().await {
            handle(
                &pump_state,
                &pump_run,
                &pump_cwd,
                &pump_session,
                &pump,
                event,
            )
            .await;
        }
        // **A fallback, not a second ending.** An agent that dies without
        // saying so closes the channel and never sends `Ended`, and this is the
        // only thing that records the stop. But `Ended` normally *does* arrive,
        // and this block then ingested a second `SessionEnded` over the top of
        // it — which, before the `interrupted` arm existed, silently promoted a
        // just-recorded interruption back to `completed`.
        //
        // So it fires only where there is still something live to end, and it
        // ends it the way the teardown would.
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
                    Source::Daemon,
                    Event::SessionEnded { reason },
                    Some(pump_cwd),
                    RunMode::Driven,
                )
                .await;
        }
        // **Last, so that an empty `sessions` map means every pump has
        // finished writing.** This used to be the first thing the pump did on
        // the way out, which made the map say *drained* while the final
        // `SessionEnded` was still in flight — and `shutdown()` reads this map
        // to decide when it is safe to return.
        pump_state.sessions.lock().await.remove(&pump_run);
    });
}

/// Whether this repository keeps what its agents say.
///
/// Read once at dispatch rather than per chunk: a policy question asked on
/// every syllable is a file read on every syllable, and a setting that changes
/// halfway through a turn would leave half a conversation on disk.
fn transcripts_wanted(cwd: &Path) -> bool {
    let root = crate::core::project::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    crate::core::ProjectConfig::load(&root)
        .map(|c| c.transcripts.keep)
        // A file that will not parse has already been refused elsewhere; the
        // safe reading of "we could not tell" is the default, not silence.
        .unwrap_or(true)
}

/// Refuses unless somebody has said this directory is theirs.
///
/// A headless agent runs the repository's own hooks and MCP servers with no
/// dialog of its own, so the decision has to be deliberate and once. A worktree
/// inherits the trust of the repository that owns it: trusting a project and
/// then being asked again for each of its checkouts would teach people to say
/// yes without reading.
pub async fn require_trust(state: &Shared, cwd: &Path) -> Result<()> {
    let root = crate::core::project::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
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

/// Refuses when a project already has as many agents running as it allows.
///
/// `max_parallel_runs` exists because five agents in one repository is rarely
/// five times the work: they collide on the same files, they multiply the
/// permission prompts, and each one costs money whether or not it was a good
/// idea. The limit is the project's own, because only it knows how much
/// parallelism its tests and its ports can take.
async fn require_capacity(
    state: &Shared,
    cwd: &Path,
    for_work: Option<&crate::core::WorkId>,
) -> Result<()> {
    let root = crate::core::project::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());

    let Ok(config) = crate::core::ProjectConfig::load(&root) else {
        return Ok(());
    };
    let Some(limit) = config.policy.max_parallel_runs else {
        return Ok(());
    };

    let id = crate::core::ProjectId::from_path(&root);
    let work_of_run = state.work_of_run.lock().await.clone();
    let live = {
        let w = state.world.lock().await;
        // Distinct *pieces of work*, not runs.
        //
        // Two things the limit must not count. Sessions the user started
        // themselves: three editor tabs would refuse every dispatch on a
        // project set to two. And the steps of one chain, which share a worktree
        // and run one after another — they are not the colliding agents the
        // limit exists to prevent, and counting them refuses step three of a
        // three-step pipeline under a limit of two.
        let mut seen = std::collections::HashSet::new();
        w.runs()
            .filter(|r| {
                matches!(
                    r.mode,
                    crate::core::RunMode::Driven | crate::core::RunMode::Background
                ) && r.project_id.as_ref() == Some(&id)
                    && r.state.is_live()
            })
            .filter(|r| match work_of_run.get(&r.id) {
                // The work asking for this step already holds its slot — the
                // step it is leaving is part of the same unit, not a rival for
                // it.
                Some(work) if Some(work) == for_work => false,
                Some(work) => seen.insert(work.to_string()),
                // A bare `dispatch` is its own unit.
                None => seen.insert(r.id.to_string()),
            })
            .count()
    };
    if live >= limit {
        anyhow::bail!(
            "{} already has {live} piece(s) of work with an agent in them and allows \
             {limit}. Finish one, or raise max_parallel_runs in devplane.toml.",
            root.display()
        );
    }
    Ok(())
}

/// Sends a prompt to a driven run.
pub async fn prompt(state: &Shared, run: &RunId, text: String) -> Result<()> {
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Devplane drives")?;
    state
        .ingest(
            run.clone(),
            Source::Daemon,
            Event::PromptSubmitted {
                chars: text.chars().count(),
            },
            None,
            RunMode::Driven,
        )
        .await;
    // Including a gate's failures handed back: "why did it try that" is
    // answered by what it was told, and that was the one thing not written down.
    //
    // The repository's setting is read once per turn here rather than carried
    // on the run: a turn is a rare event, and a flag on the domain type would
    // be a storage policy living in the state machine.
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
    session.prompt(text).await
}

/// What a caller asked for when answering a permission request.
///
/// `Allow`/`Deny` are resolved against the options the agent actually offered,
/// because ACP option ids belong to the agent: one names its option `allow`,
/// another `proceed_once`. A caller that had to know the id would be a caller
/// that guesses, and a wrong guess is a rejected answer and a wedged session.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Allow,
    Deny,
    /// An exact option id, for a caller that read them off the inbox item.
    Option(String),
}

impl Decision {
    /// Reads `{ "decision": "allow" }`, or `{ "option_id": "…" }` for an exact
    /// choice. Nothing at all is a refusal — which is also what an unanswered
    /// request becomes when it times out.
    pub fn parse(decision: Option<&str>, option_id: Option<String>) -> Self {
        if let Some(id) = option_id {
            return Decision::Option(id);
        }
        match decision {
            Some("allow") => Decision::Allow,
            _ => Decision::Deny,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Decision::Deny => "deny",
            _ => "allow",
        }
    }
}

/// Whether an option the agent offered is a **standing** grant or refusal
/// rather than a one-off — `allow_always` and `reject_always` in the protocol.
///
/// This matters more than it looks. A standing grant lives inside the *agent's*
/// session: every later call it covers is approved without a request ever
/// reaching Devplane again. Recording it as a plain "allow" would make the
/// decision log answer "why did that command run without anybody being asked?"
/// with silence — which is the one question this product exists to answer. So
/// the outcome says which it was, and the reason says what it means.
fn standing(kind: Option<&str>) -> bool {
    matches!(kind, Some("allow_always") | Some("reject_always"))
}

/// Answers an outstanding permission request.
/// Delivers a person's answer to a question a driven run asked.
///
/// The form the agent sent is re-read from the run rather than trusted from the
/// caller: the content that goes back must match the schema the agent
/// published, and the only copy of that schema this process can vouch for is
/// the one it recorded when the question arrived. A caller naming a field or an
/// option the agent never offered gets it dropped, not forwarded.
/// Delivers an answer down a live connection. Reached only through
/// [`answer_ask`], because an answer that has not been written down first is one
/// a crash can lose.
async fn answer_question(
    state: &Shared,
    run: &RunId,
    request_id: &str,
    chosen: &[(String, crate::core::question::Chosen)],
) -> Result<()> {
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Devplane drives")?;

    let form: Vec<crate::core::question::Question> = {
        let w = state.world.lock().await;
        w.run(run)
            .and_then(|r| r.blocked_on.as_ref())
            .and_then(|b| b.form.clone())
            .and_then(|f| serde_json::from_value(f).ok())
            .unwrap_or_default()
    };
    if form.is_empty() {
        anyhow::bail!("no question is waiting on that run");
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
    session.answer(request_id, Some(content.clone())).await?;

    // **Who answered, recorded at the moment it happens.** This is the
    // one row in the ledger whose authority is *known* rather than inferred:
    // everything else Devplane can say about who decided something is derived
    // from what did or did not arrive, and this is a person having typed. The
    // reason carries what they chose, because "somebody answered" without the
    // answer is a row nobody can check.
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
    Ok(())
}

// ── Answering by token ──────────────────────────────────────────────────────
//
// Everything below addresses an ask by its own opaque id rather than by the
// session that happens to be holding it. That is the fourth of the five
// durable-execution properties and the one that decides whether the other four
// are worth anything: an answer addressed to a session is undeliverable the
// moment the session is gone, which is the state every restart leaves behind.

/// What a person chose, as it crosses from a surface into the record.
#[derive(Debug, Clone)]
pub enum Answer {
    /// A permission: allow, deny, or an exact option the agent offered.
    Permission(Decision),
    /// A question: one option per field, or the person's own words.
    Question(Vec<(String, crate::core::question::Chosen)>),
}

/// Records a person's answer to an ask, then tries to deliver it.
///
/// **In that order, and the order is the feature.** A crash between *answered*
/// and *delivered* then replays as answered rather than as ask-again, and two
/// surfaces racing each other is harmless: the first wins, the second is told
/// who did, and the agent hears one answer.
///
/// Delivery is reported separately ([`Delivery`](crate::core::ask::Delivery)),
/// because *recorded and delivered later into a resumed session* and *recorded
/// and never delivered* are different outcomes and only one may be claimed.
pub async fn answer_ask(
    state: &Shared,
    ask_id: &str,
    answer: Answer,
    from: &str,
) -> Result<crate::core::ask::Ask> {
    use crate::core::ask::Ended;

    let mut ask = state
        .store
        .ask(ask_id)
        .await?
        .with_context(|| format!("no ask with id `{ask_id}`"))?;

    // What the person chose, in the shape the record keeps. Composed before the
    // idempotency check so that a second answer is refused having been fully
    // understood rather than half-parsed.
    let recorded = match &answer {
        Answer::Permission(d) => serde_json::json!({ "permission": d.label() }),
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
    };

    if let Err(refused) = ask.answer(recorded, from, jiff::Timestamp::now()) {
        // Not an error the caller made. The guarantee held, and the second
        // surface is told what the first one chose rather than being handed a
        // failure it cannot act on.
        anyhow::bail!("{}", refused.says());
    }
    // **Durable before the effect.** Everything after this line may fail; none
    // of it may lose what the person said.
    state.store.save_ask(&ask).await?;

    let delivery = deliver(state, &ask, answer).await;
    ask.delivery = Some(delivery.clone());
    state.store.save_ask(&ask).await?;

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

    // The ending is already `person`; this is only the log catching up with a
    // row that was written first.
    debug_assert!(matches!(ask.ended, Some(Ended::Person)));
    Ok(ask)
}

/// Gets the person's answer to the agent, by whatever route is still open.
///
/// Three outcomes and each is named rather than collapsed into success: the
/// connection that asked is still there; the agent is gone but its session can
/// be resumed; or neither, in which case the answer stays recorded and the row
/// says plainly that nobody received it.
async fn deliver(
    state: &Shared,
    ask: &crate::core::ask::Ask,
    answer: Answer,
) -> crate::core::ask::Delivery {
    use crate::core::ask::Delivery;

    let live = state.sessions.lock().await.contains_key(&ask.run);
    if live {
        let sent = match answer {
            Answer::Permission(d) => decide(state, &ask.run, &ask.request_id, d).await,
            Answer::Question(c) => answer_question(state, &ask.run, &ask.request_id, &c).await,
        };
        return match sent {
            Ok(()) => Delivery::Live,
            Err(e) => Delivery::Undeliverable {
                because: e.to_string(),
            },
        };
    }

    // The agent that asked is gone — a restart, a crash, a `devplane stop` —
    // but its session is on the vendor's disk and the protocol can continue it.
    //
    // **A new message, not a reply**, because the turn that asked ended with the
    // process. Every surface says so, and so does the text handed over.
    //
    // One case is deliberately not guarded, having been looked at: a `kill -9`
    // leaves the agent running and blocked on a dead pipe, so resuming puts a
    // second agent on that session. The leaked one can never receive another
    // prompt, and refusing delivery on a worktree match would trade a real
    // answer for a hypothetical conflict.
    match resume(state, &ask.run).await {
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

/// What a resumed agent is told, which is the person's own answer and a sentence
/// saying when it was given.
///
/// **Devplane writes exactly one sentence of its own here** — the frame — and
/// never a word of the answer. Composing an answer is the one thing this product
/// may not do, and the line between *carrying somebody's words* and *speaking
/// for them* is the whole of it.
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

/// Delivers a permission decision down a live connection. Reached only through
/// [`answer_ask`], for the same reason.
async fn decide(state: &Shared, run: &RunId, request_id: &str, want: Decision) -> Result<()> {
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Devplane drives")?;

    // The options the agent offered, as recorded when the request arrived.
    let offered = {
        let w = state.world.lock().await;
        w.run(run)
            .and_then(|r| r.blocked_on.as_ref())
            .map(|b| b.options.clone())
            .unwrap_or_default()
    };
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

    let option = match &want {
        Decision::Option(id) => Some(id.clone()),
        Decision::Allow => match choose(true) {
            Some(id) => Some(id),
            // The agent offered no way to say yes. Refusing is the only honest
            // answer; inventing an id would be rejected by the agent anyway.
            None => anyhow::bail!("that request offers nothing that means allow"),
        },
        // A refusal needs no id: the protocol treats an absent choice as
        // cancelled, which is the safe end of the range.
        Decision::Deny => choose(false),
    };
    // What the chosen option actually was, which is not always what was asked
    // for: a caller may pass an exact id read off the inbox item.
    let chosen_kind = option.as_ref().and_then(|id| {
        offered
            .iter()
            .find(|o| o.id.as_deref() == Some(id.as_str()))
            .and_then(|o| o.kind.clone())
    });
    let is_standing = standing(chosen_kind.as_deref());
    let decision = if is_standing {
        match want {
            Decision::Deny => "reject_always",
            _ => "allow_always",
        }
    } else {
        want.label()
    };
    session.decide(request_id, option).await?;

    let subject = {
        let w = state.world.lock().await;
        w.run(run)
            .and_then(|r| r.blocked_on.as_ref())
            .and_then(|b| b.message.clone())
            .unwrap_or_else(|| request_id.to_string())
    };
    let mut record = crate::core::Decision::new(
        crate::core::Authority::Person,
        "agent:tool.use",
        subject,
        decision,
    )
    .for_run(run);
    if is_standing {
        // The log has to say this out loud, because it is the one decision
        // whose consequences Devplane will not see. Everything this grant
        // covers from here on is approved inside the agent, and no later row
        // will appear to explain it.
        record = record.because(
            "a standing choice made by a person: the agent applies it to later matching calls itself, and Devplane sees no request for those",
        );
    }
    state.record(record).await;

    // Recording the decision is what takes the item out of the inbox. Waiting
    // for the agent's next move instead would leave an answered question on
    // screen until it happened to do something — and if the answer was "no",
    // that might be never.
    state
        .ingest(
            run.clone(),
            Source::Daemon,
            Event::PermissionDecided {
                tool: String::new(),
                decision: decision.into(),
                by: "human".into(),
            },
            None,
            RunMode::Driven,
        )
        .await;
    Ok(())
}

/// Whether Devplane still has a session it can prompt for this run.
///
/// Asked before offering anything that needs one, because an inbox button that
/// cannot do what it says is the one failure a control plane cannot afford.
pub async fn is_live(state: &Shared, run: &RunId) -> bool {
    state
        .sessions
        .lock()
        .await
        .get(run)
        .map(|s| s.is_live())
        .unwrap_or(false)
}

/// Stops a driven run.
pub async fn stop(state: &Shared, run: &RunId) -> Result<()> {
    let session = state.sessions.lock().await.remove(run);
    match session {
        Some(s) => {
            s.stop();
            Ok(())
        }
        None => anyhow::bail!("that run is not one Devplane drives"),
    }
}

/// What to call a tool call on the board.
///
/// The policy vocabulary where the call can be put in it — `Bash`, `Read`,
/// `Edit`, `WebFetch` — because that is the word the rule that governs it is
/// written in, and a board that names the call one thing while the audit log
/// names it another is two boards. The agent's own title otherwise.
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
    /// Whether this repository wants what its agents say written down. Decided
    /// once, when the agent starts, so that editing `devplane.toml` mid-turn
    /// cannot half-record a conversation.
    keep_transcript: bool,
}

/// The option to answer a permission request with: the exact kind if the agent
/// offers it, otherwise any option of the same family.
///
/// Never a literal id. ACP option ids belong to the agent — one calls it
/// `allow`, another `proceed_once` — so a hard-coded string works against a
/// fixture and against nothing else.
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

/// Translates one protocol event, applying policy to permission requests.
/// Ends the fragment being joined and records it.
///
/// Called whenever the sentence is over: a tool call, the turn ending, the
/// session ending. Without it the last thing an agent said would sit in memory
/// until it happened to say something else.
async fn flush_transcript(state: &Shared, run: &RunId, pump: &tokio::sync::Mutex<Pump>) {
    let fragment = {
        let mut p = pump.lock().await;
        p.transcript.take().filter(|_| p.keep_transcript)
    };
    if let Some((role, text)) = fragment {
        state
            .ingest_message(crate::core::Message::new(run.clone(), role, text))
            .await;
    }
}

/// Records one fragment, if this repository keeps them.
async fn keep(
    state: &Shared,
    run: &RunId,
    pump: &tokio::sync::Mutex<Pump>,
    role: crate::core::Role,
    chunk: &str,
) {
    let ready = {
        let mut p = pump.lock().await;
        match p.keep_transcript {
            true => p.transcript.push(role, chunk),
            false => None,
        }
    };
    if let Some((role, text)) = ready {
        state
            .ingest_message(crate::core::Message::new(run.clone(), role, text))
            .await;
    }
}

/// Writes down what an agent just asked, before anything else happens.
///
/// **The row is the question; everything else on this path is a projection.**
/// Run state can be replayed from the event log and the inbox is derived from
/// run state, but nothing anywhere can re-derive *what somebody was asked* —
/// so this is written first and a failure to write it is loud rather than
/// swallowed: a write a promise rests on may not be answered with `.ok()`, and
/// this is the promise the product is named for.
///
/// The deadline comes from the project's own `devplane.toml` and is
/// [`Never`](crate::core::ask::Deadline::Never) unless it says otherwise.
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
        // A question that could not be written down is a question that will be
        // lost on the next restart, and the person needs to know that *now*
        // rather than discovering an empty inbox later.
        tracing::error!(error = %e, run = %run, "could not write down what the agent asked");
        // Where `doctor` reads it: a recorded diagnostic is a displayed one,
        // and a failure written nowhere is the same as no failure handling at
        // all.
        let _ = state
            .store
            .record_channel("asks", 0, Some(&e.to_string()))
            .await;
        return None;
    }
    Some(ask.id)
}

/// Ends one ask on the project's own clock: the agent is told, the row is
/// written, and the person is shown.
///
/// **All three, because the version that did only the middle one is what this
/// replaced.** A permission nobody answered used to be refused after ten
/// minutes by a constant in this crate; the decision row was written and no
/// surface mentioned it, so a call refused in somebody's name was discoverable
/// only by running `devplane audit` and knowing to look. A clock that ends a
/// question is a decision taken on your behalf, which is the one class of event
/// this product exists to put in front of you.
pub(crate) async fn expire(
    state: &Shared,
    ask: &crate::core::ask::Ask,
    ended: &crate::core::ask::Ended,
) {
    // The agent first. It is blocked on an answer that is never coming, and
    // telling it *no* is the only honest thing left to say — the same direction
    // the old timeout chose, now with a name on it.
    if let Some(session) = state.sessions.lock().await.get(&ask.run).cloned() {
        match ask.kind {
            crate::core::ask::Kind::Permission => {
                let _ = session.decide(&ask.request_id, None).await;
            }
            crate::core::ask::Kind::Question => {
                // `None` is *cancel*, never *decline*: the adapter folds a
                // decline into "answered, with no answers", which is the agent
                // proceeding on nothing — the exact failure this feature
                // exists to prevent, reached through the politer word.
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
                    crate::core::ask::Kind::Permission => "deny",
                    crate::core::ask::Kind::Question => "unanswered",
                },
            )
            .because(ended.says())
            .for_run(&ask.run),
        )
        .await;

    // `Source::Daemon` says **which process observed this**, not whose clock it
    // was: the sweep produced the event, rather than the agent reporting it, and
    // a surface reading the source has to be able to tell those apart.
    //
    // The comment here used to say *because this is Devplane's own clock* — the
    // reasoning of the hard-coded ten-minute refusal that was deleted with the
    // clock itself. Devplane has no clock. The authority for this ending is the
    // **project's** timer, it is on the decision row above with the duration and
    // the file that set it, and it is deliberately not derivable from the source.
    let event = match ask.kind {
        crate::core::ask::Kind::Permission => Event::PermissionDecided {
            tool: String::new(),
            decision: "deny".into(),
            by: "timer".into(),
        },
        crate::core::ask::Kind::Question => Event::QuestionEnded {
            request_id: ask.request_id.clone(),
        },
    };
    state
        .ingest(
            ask.run.clone(),
            crate::core::Source::Daemon,
            event,
            None,
            crate::core::RunMode::Driven,
        )
        .await;
}

/// Closes every ask still open on one run, with an authority on each.
///
/// Never overwrites an ending, so a person who answered a moment before the run
/// died keeps the credit for it — `Ask::end` enforces that and this is only the
/// caller of it.
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
        if let Err(e) = state.store.save_ask(&ask).await {
            tracing::error!(error = %e, ask = %ask.id, "could not close an ask");
        }
    }
}

/// The deadline this repository sets for an unanswered ask.
///
/// **Absent means it waits**, which is both the default and the only honest
/// reading of an absent setting. A value that will not parse is treated as
/// `never` too — the same direction, and the file's own `check` command is
/// where a typo is reported, because ending somebody's question early on the
/// strength of a misspelling is the one outcome this must not produce.
fn deadline_for(cwd: &Path) -> crate::core::ask::Deadline {
    let root = crate::core::project::governing_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    crate::core::ProjectConfig::load(&root)
        .ok()
        .and_then(|c| c.questions.deadline())
        .unwrap_or(crate::core::ask::Deadline::Never)
}

/// What was run for this run, where it was recorded.
///
/// Recorded at spawn so that a daemon **killed** rather than stopped leaves
/// enough behind to find the agents it abandoned; reused here because it is
/// also the only stable key for *which agent this is*.
async fn agent_command(state: &Shared, run: &RunId) -> Option<String> {
    state.world.lock().await.run(run)?.agent_command.clone()
}

async fn handle(
    state: &Shared,
    run: &RunId,
    cwd: &Path,
    session: &Session,
    pump: &tokio::sync::Mutex<Pump>,
    event: AcpEvent,
) {
    let ingest = |e: Event| {
        let state = state.clone();
        let run = run.clone();
        let cwd = cwd.to_path_buf();
        async move {
            state
                .ingest(run, Source::Daemon, e, Some(cwd), RunMode::Driven)
                .await;
        }
    };

    match event {
        // **The cross-vendor half of *which sessions decide without you*.**
        // `devplane modes` reads Claude Code's settings files; this is the same
        // question answered by any agent that speaks the protocol.
        AcpEvent::ModeChanged { mode } => {
            // Observed at session creation rather than at `initialize`, so it
            // amends the capability record rather than being part of it.
            if let Some(cmd) = agent_command(state, run).await
                && let Err(e) = state.store.note_agent_declares_modes(&cmd).await
            {
                tracing::debug!(error = %e, "could not note that this agent declares modes");
            }
            ingest(Event::AgentModeSeen { mode }).await;
        }

        AcpEvent::Capabilities {
            agent_name,
            resume,
            load_session,
            list_sessions,
            needs_auth,
        } => {
            // **Keyed on what was actually run.** The session does not know how
            // it was started; the run does, because the command is recorded at
            // spawn so a killed daemon leaves enough behind to find what it
            // abandoned.
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
            // The id the agent will answer `session/resume` on. It was being
            // dropped here, which is why a daemon restart could only ever offer
            // to start the work again from nothing.
            ingest(Event::AgentSessionOpened {
                agent_session: session_id,
            })
            .await;
        }

        // A driven run has no window of its own, so this *is* its transcript.
        // The chunks are joined into fragments rather than stored one row per
        // syllable, and they never enter the event log: run state is a pure
        // reduction over that log, and a sentence reduces to nothing.
        AcpEvent::Text(chunk) | AcpEvent::Thought(chunk) if chunk.is_empty() => {
            let _ = chunk;
        }
        AcpEvent::Text(chunk) => keep(state, run, pump, crate::core::Role::Agent, &chunk).await,
        AcpEvent::Thought(chunk) => {
            keep(state, run, pump, crate::core::Role::Thought, &chunk).await
        }

        // What the agent intends, and how far it has got. Structured and
        // low-volume, so unlike the prose it belongs on the board itself.
        AcpEvent::Plan(steps) => {
            flush_transcript(state, run, pump).await;
            state
                .set_plan(
                    run,
                    steps
                        .into_iter()
                        .map(|s| crate::core::PlanStep {
                            content: s.content,
                            status: s.status,
                        })
                        .collect(),
                )
                .await;
        }

        // A tool call ends the sentence before it.
        AcpEvent::Tool { call, status } => {
            flush_transcript(state, run, pump).await;
            // The title is what a person reads; the arguments are what the
            // board summarises and what a rule was matched against, so they go
            // on the event rather than a `null` that made every driven tool
            // call look like a bare tool name.
            let label = tool_label(&call);
            let input = call.raw_input.clone().unwrap_or(serde_json::Value::Null);
            match status.as_deref() {
                Some("completed") => {
                    ingest(Event::ToolFinished {
                        tool: label,
                        ok: true,
                        duration_ms: None,
                    })
                    .await
                }
                Some("failed") => {
                    ingest(Event::ToolFinished {
                        tool: label,
                        ok: false,
                        duration_ms: None,
                    })
                    .await
                }
                // A driven agent speaks the protocol, which has no field for
                // where a server's definition came from. Absent, which reads
                // differently from a source nobody understood.
                _ => {
                    ingest(Event::ToolStarted {
                        tool: label,
                        input,
                        server_source: None,
                    })
                    .await
                }
            }
        }

        AcpEvent::PermissionRequested {
            request_id,
            call,
            options,
        } => {
            let title = call.title.clone();
            // The same rules that answer a hook answer this, in the same
            // vocabulary: the kind the agent declared, and the arguments it
            // actually passed. A call Devplane cannot classify is one no rule
            // can honestly be said to cover, so nothing auto-decides it and a
            // person is asked — which is the direction that is safe to be
            // wrong in.
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
                    })
                    .await;
                    return;
                }
                // The project asked to be asked. Same outcome as no rule at
                // all — a person decides — and the rule is named in the log.
                // `Unresolved` lands here too: a driven run's permission
                // request is already on its way to a person, so the escalation
                // has nowhere to go and the value is the recorded reason.
                Verdict::Ask { .. } | Verdict::Unresolved { .. } | Verdict::Undecided => {}
            }

            // Nobody's rule covers it, so a human decides. Unlike an observed
            // session, this one can actually be answered from the inbox — and
            // the answer has to name one of *these* ids, which is why they are
            // carried through rather than guessed at the far end.
            let choices: Vec<Choice> = options
                .into_iter()
                .map(|o| Choice {
                    id: Some(o.id),
                    label: o.label,
                    kind: Some(o.kind),
                })
                .collect();
            // **The row first, and the event second.** Everything below this
            // line is a projection that can be rebuilt; the ask cannot. It is
            // the one thing on this path nothing else on the machine knows.
            let token = persist_ask(
                state,
                run,
                crate::core::ask::Kind::Permission,
                &request_id,
                &title,
                serde_json::json!({ "options": choices }),
            )
            .await;
            ingest(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: Some(title),
                request_id: Some(request_id),
                ask: token.map(|t| t.0),
                options: choices,
                call: None,
            })
            .await;
        }

        AcpEvent::Usage {
            cost_usd,
            context_tokens,
            context_window,
        } => {
            // The protocol reports running totals for the session; the reducer
            // adds up per-request figures. Only the delta belongs here, so the
            // pump remembers what it last saw — without which a driven run
            // showed `$0.00` for ever, which is the one number people check.
            let (cost_delta, tokens) = {
                let mut seen = pump.lock().await;
                let cost = cost_usd.unwrap_or(0.0);
                let delta = (cost - seen.cost).max(0.0);
                seen.cost = seen.cost.max(cost);
                (delta, context_tokens.unwrap_or(0))
            };
            // `used` is the *level* of the context window, not a per-request
            // count, so it is reported as one. Putting it in `input_tokens`
            // made `RunTotals::apply` add successive window levels together,
            // and a driven run's token total grew roughly with the square of
            // its length — a number on the board that was not wrong about a
            // detail but meant nothing at all.
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
            match stop_reason.as_str() {
                "refusal" => {
                    ingest(Event::TurnFailed {
                        message: "the agent refused to continue".into(),
                    })
                    .await
                }
                _ => ingest(Event::TurnEnded).await,
            }
            // The agent has stopped. If this run is doing a piece of work, that
            // is the moment its claim gets checked.
            crate::work::on_turn_ended(state, run).await;
        }

        // The agent asked the person something. Not a permission: no rule can
        // answer it, so nothing here consults one — it goes straight to the
        // person, and waits.
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

        // Devplane said it could render a form and then could not. The agent has
        // already been told no; the person is told too, because an agent that
        // asked and gave up with nobody knowing is this feature's whole subject.
        AcpEvent::QuestionUnrenderable { what } => {
            state
                .record(
                    crate::core::Decision::new(
                        crate::core::Authority::Daemon,
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

        // Nobody answered and the call was cancelled, or the run ended under it.
        // Recorded because the inbox has to stop offering an answer that can no
        // longer be delivered.
        AcpEvent::QuestionCancelled { request_id } => {
            ingest(Event::QuestionEnded { request_id }).await;
        }

        AcpEvent::Ended { error } => {
            flush_transcript(state, run, pump).await;
            // **A question dies with its run, and must not be swallowed by the
            // word `completed`.** Measured 2026-09-19: stopping the daemon
            // killed the agent under a waiting question, and the run came back
            // reading `completed` with an empty inbox — which is the failure
            // this feature exists to prevent, wearing the one word the whole
            // product distrusts. Recorded first, so the ending cannot overwrite
            // it.
            // **Who killed this agent decides whether its question is still
            // owed an answer**, and both cases arrive here as the same closed
            // connection. A turn that ended on its own leaves a question
            // nobody can answer any more. A daemon that was stopped leaves one
            // that is perfectly answerable: the vendor's session is on disk,
            // the protocol continues it, and the person's answer can still be
            // delivered into it — so the ask stays **open** and the inbox goes
            // on offering it after the restart.
            let tearing_down = state.tearing_down.load(std::sync::atomic::Ordering::SeqCst);
            if !tearing_down {
                end_open_asks(
                    state,
                    run,
                    crate::core::ask::Ended::Nobody {
                        because: "the run ended before anybody answered".into(),
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
                state
                    .record(
                        crate::core::Decision::new(
                            // **Nobody decided.** This is the row the whole
                            // product exists to be able to write, and it said
                            // `daemon` — Devplane taking the blame for a
                            // question it faithfully delivered and nobody
                            // answered.
                            crate::core::Authority::Nobody,
                            "agent:question",
                            request_id.clone(),
                            "unanswered",
                        )
                        .because("the run ended before anybody answered")
                        .for_run(run),
                    )
                    .await;
                ingest(Event::QuestionEnded { request_id }).await;
            }
            match error {
                Some(e) => ingest(Event::TurnFailed { message: e }).await,
                // **The daemon stopping is not the agent finishing**, and the
                // same closed connection arrives here for both. Measured
                // 2026-09-20: a run that was `Working` when `devplane` was
                // stopped came back from the store reading `completed` — this
                // line, via the reducer's catch-all — which is the sentence
                // three paragraphs above saying the feature exists to prevent
                // exactly that, defeated by the code underneath it.
                None if tearing_down => {
                    ingest(Event::SessionEnded {
                        reason: Some("interrupted".into()),
                    })
                    .await
                }
                None => ingest(Event::SessionEnded { reason: None }).await,
            }
        }
    }
}

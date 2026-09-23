//! A driven session: an ACP agent Devplane owns.
//!
//! The protocol's client API is shaped around one connection that lives for as
//! long as the conversation, so a driven run is an actor: a task that owns the
//! connection, takes commands on a channel, and emits events on another. The
//! daemon never touches the connection, which is what keeps the reducer free of
//! protocol types and the protocol free of daemon locks.
//!
//! The part worth reading twice is the permission handler. It runs inside the
//! connection task, and it has to block that task until a human — or a policy —
//! answers, because the agent is waiting on the response. Every request is
//! therefore registered with a one-shot channel and a deadline, so a question
//! nobody answers times out into a refusal rather than wedging the session.

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::StopReason;
use agent_client_protocol::schema::v1::{
    CancelNotification, ClientCapabilities, ContentBlock, CreateElicitationRequest,
    CreateElicitationResponse, ElicitationAction, ElicitationCapabilities,
    ElicitationFormCapabilities, InitializeRequest, NewSessionRequest, PromptRequest,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    ResumeSessionRequest, SelectedPermissionOutcome, SessionNotification, SessionUpdate,
    TextContent,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectionTo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify, mpsc, oneshot};

use super::agent::AgentSpec;

// A `PERMISSION_TIMEOUT` of six hundred seconds lived here until 2026-09-20,
// and deleting it is the point rather than a tidy-up.
//
// It refused a permission nobody had answered after ten minutes, in the
// person's name, on a number nobody chose, and no surface said it had happened
// — the decision row was the only trace and `devplane audit` was the only way
// to find it. That is the trade this product indicts four vendors for, shipped
// here; and the vendor it mirrors most closely is stricter than this was, since
// Claude Code's own question timer is off by default and "permission prompts,
// including plan approval, never auto-resolve on idle".
//
// What replaces it is a deadline a **project** sets, in its own `devplane.toml`,
// defaulting to `never`, swept by the daemon against a durable row, and ending
// with an authority that names the duration and the file it came from.
// `core::ask` holds that; this connection now waits.

/// How long a cancelled turn has to acknowledge before the connection — and
/// with it the agent's process group — is torn down anyway.
const CANCEL_GRACE: Duration = Duration::from_secs(5);

/// What the daemon can ask a driven session to do.
///
/// Permission answers are deliberately absent: the connection task is blocked
/// inside the handler waiting for one, so a command it could only read *after*
/// the handler returned would deadlock the pair. [`Session::decide`] answers the
/// waiting task directly instead.
#[derive(Debug)]
pub enum Command {
    /// Send a prompt and run the turn to completion.
    Prompt(String),
}

/// What a driven session reports.
///
/// Deliberately its own type rather than the domain's: the protocol's
/// vocabulary is richer than the board needs, and mapping at the boundary keeps
/// the reducer from growing a dependency on a protocol version.
#[derive(Debug, Clone, PartialEq)]
pub enum AcpEvent {
    /// The agent is up and the session exists.
    Ready {
        session_id: String,
        agent_name: Option<String>,
    },
    /// A chunk of the agent's answer.
    Text(String),
    /// A chunk of the agent's reasoning.
    Thought(String),
    /// The agent's plan for the turn: what it intends to do, and how far it has
    /// got. Low-volume and structured, which is the opposite of the transcript
    /// and exactly what a supervisor wants — "what is it about to do" answered
    /// without reading a word of prose.
    Plan(Vec<PlanStep>),
    /// A tool call started or changed.
    Tool {
        call: ToolRequest,
        /// `pending`, `in_progress`, `completed`, `failed`. Absent when the
        /// update carried no status — content streaming in, usually — and an
        /// update with nothing to say is not reported at all.
        status: Option<String>,
    },
    /// The agent wants permission. Answer it with [`Session::decide`].
    PermissionRequested {
        request_id: String,
        call: ToolRequest,
        options: Vec<PermissionOption>,
    },
    /// The agent asked the person a question. Answer it with
    /// [`Session::answer`].
    ///
    /// Not a permission: no rule can answer *which of these do you want*, and
    /// it arrives on a different protocol method.
    QuestionAsked {
        request_id: String,
        ask: crate::core::question::Ask,
    },
    /// What the agent advertised at `initialize`, measured rather than assumed.
    ///
    /// **Carries no command**, because the session does not know how it was
    /// started — the caller has the spec and keys the record with it.
    ///
    /// Emitted once per connection, after the handshake and before anything is
    /// asked of the agent. Every field is a runtime fact about *that* agent at
    /// *that* version, which is the whole reason it is recorded with a date
    /// rather than written into a table by hand.
    Capabilities {
        agent_name: Option<String>,
        resume: bool,
        load_session: bool,
        list_sessions: bool,
        needs_auth: bool,
    },
    /// What the agent says its own sessions are.
    ///
    /// **Only where the agent advertises `session/list`**, and only as a
    /// roster: v1's `SessionInfo` carries no state field, so this can fill a
    /// gap — a title where Devplane has none — and can never say that a session
    /// is waiting for somebody. Asked once, after the handshake, because it is
    /// a catalogue rather than a signal and polling it would be the second
    /// roster on a machine that already has one.
    SessionsListed {
        sessions: Vec<crate::core::run::ListedSession>,
    },
    /// The agent says which mode it is now in.
    ///
    /// **The agent's own string, and never one of the vendor's four.** An ACP
    /// session mode is an identifier the agent declares; there is no
    /// cross-vendor vocabulary for it, and nothing in the specification maps it
    /// onto Claude Code's `default`/`acceptEdits`/`plan`/`bypassPermissions`.
    /// Presenting it as one of those would assert an equivalence no
    /// specification supports — in the one product whose argument is that an
    /// authority is recorded rather than inferred.
    ModeChanged { mode: String },
    /// The agent asked something over the question channel that Devplane could
    /// not render, so it was cancelled.
    ///
    /// Reported rather than swallowed: Devplane told the agent it could show a
    /// form, and a case where it cannot is a gap in this product, not a
    /// non-event. The alternative is an agent that asked and gave up with
    /// nobody ever knowing it asked.
    QuestionUnrenderable { what: String },
    /// A question was cancelled without being answered — the session ended, or
    /// somebody stopped the run. Reported so the inbox stops offering an answer
    /// that can no longer be delivered.
    ///
    /// **There was a `PermissionExpired` beside this one and it is gone.** It
    /// was emitted by the six-hundred-second `PERMISSION_TIMEOUT` deleted above,
    /// and deleting the clock left the variant declared, matched in `driven.rs`,
    /// and constructed by nothing — a handler that could never run, writing a
    /// decision row reading *"nobody answered within ten minutes"* about a rule
    /// this product no longer has. An expiry now comes from the project's own
    /// deadline and is swept against the durable `asks` row, which is a
    /// different path entirely.
    QuestionCancelled { request_id: String },
    /// Usage the agent reported for the session. Unlike the Claude-specific
    /// channels, the protocol reports the window size too, so the context
    /// gauge needs no table of model names.
    Usage {
        cost_usd: Option<f64>,
        context_tokens: Option<u64>,
        context_window: Option<u64>,
    },
    /// The turn finished.
    TurnEnded { stop_reason: String },
    /// The session or its process ended.
    Ended { error: Option<String> },
}

impl AcpEvent {
    /// Whether this is a fragment of the conversation itself, rather than a
    /// fact about where the session got to.
    ///
    /// Used to suppress a `session/load` replay: the prose is already written
    /// down, while a plan, a usage total or a tool call is state rather than
    /// speech and is idempotent to see again.
    pub fn is_conversation(&self) -> bool {
        matches!(self, AcpEvent::Text(_) | AcpEvent::Thought(_))
    }
}

/// One entry of an agent's plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    pub content: String,
    /// `pending`, `in_progress`, `completed`.
    pub status: String,
}

/// What an agent is asking to do, in the protocol's own terms.
///
/// The title is prose for a human. Everything else is what a *rule* needs: the
/// kind says whether this reads, edits, executes or fetches; `raw_input` is the
/// arguments the agent actually chose; `locations` are the absolute paths it
/// named. Carrying the title alone would mean evaluating every tool call as a
/// shell command whose text is prose — under which `Bash(rm -rf *)` protects a
/// driven run only by coincidence and `Read(.env)` protects nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolRequest {
    /// The protocol's id for this call. The identity a start and a finish are
    /// correlated on; a title is not one, and an update may not repeat it.
    pub id: String,
    /// Human-readable, and the only field that is always populated.
    pub title: String,
    /// `read`, `edit`, `delete`, `move`, `search`, `execute`, `think`, `fetch`,
    /// `switch_mode`, `other` — the protocol's own spelling.
    pub kind: Option<String>,
    /// The arguments the agent passed, where it sends them.
    pub raw_input: Option<serde_json::Value>,
    /// Absolute paths the call names.
    pub locations: Vec<std::path::PathBuf>,
}

impl ToolRequest {
    /// This call in the vocabulary the permission policy speaks, or `None`.
    ///
    /// The protocol sends no tool *name* — that field is unstable — so the
    /// classification comes from the kind the agent declared and, failing that,
    /// from the shape of its arguments. Both are facts about the call; the
    /// title is prose and is never matched against.
    ///
    /// `None` means the call cannot be classified, and no rule can honestly be
    /// said to cover it: the policy is skipped and a person is asked.
    pub fn policy_subject(&self) -> Option<(&'static str, serde_json::Value)> {
        let field = |k: &str| {
            self.raw_input
                .as_ref()
                .and_then(|v| v.get(k))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let path = || {
            self.locations
                .first()
                .map(|p| p.to_string_lossy().to_string())
                .or_else(|| field("file_path"))
                .or_else(|| field("notebook_path"))
                .or_else(|| field("path"))
        };

        let by_kind = match self.kind.as_deref() {
            Some("execute") => Some("Bash"),
            Some("read") | Some("search") => Some("Read"),
            Some("edit") | Some("delete") | Some("move") => Some("Edit"),
            Some("fetch") => Some("WebFetch"),
            // `think` and `switch_mode` touch nothing outside the session, and
            // `other` is the protocol's own "I am not saying".
            _ => None,
        };
        // An agent that declares no kind still passes arguments, and a call
        // carrying a `command` is a shell command whatever it is labelled.
        let tool = by_kind.or_else(|| match () {
            _ if field("command").is_some() => Some("Bash"),
            _ if field("url").is_some() => Some("WebFetch"),
            _ => None,
        })?;

        let input = match tool {
            "Bash" => serde_json::json!({ "command": field("command")? }),
            "WebFetch" => serde_json::json!({ "url": field("url")? }),
            _ => serde_json::json!({ "file_path": path()? }),
        };
        Some((tool, input))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
    /// `allow_once`, `allow_always`, `reject_once`, `reject_always`.
    pub kind: String,
}

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Option<String>>>>>;

/// Waiting questions. Separate from [`Pending`] because an answer is not an
/// option id: it is a map of fields to what the person chose or typed, and
/// squeezing it through the permission channel would mean encoding one as the
/// other and decoding it wrong exactly once.
type Asked = Arc<Mutex<HashMap<String, oneshot::Sender<Option<serde_json::Value>>>>>;

/// A handle on a running agent.
#[derive(Debug, Clone)]
pub struct Session {
    commands: mpsc::Sender<Command>,
    pending: Pending,
    asked: Asked,
    /// Raised by [`Session::stop`]. Separate from the command channel because
    /// stopping has to reach a session that is *busy*, and the command loop is
    /// not reading while a turn is in flight.
    stop: Arc<Notify>,
}

impl Session {
    /// Sends a prompt. Returns once the command is queued, not once the turn
    /// is done — the turn's progress arrives as events.
    pub async fn prompt(&self, text: impl Into<String>) -> anyhow::Result<()> {
        self.commands
            .send(Command::Prompt(text.into()))
            .await
            .map_err(|_| anyhow::anyhow!("the session has ended"))
    }

    /// Answers a permission request. `None` refuses it.
    pub async fn decide(&self, request_id: &str, option_id: Option<String>) -> anyhow::Result<()> {
        // Answer the waiting task directly rather than through the command
        // channel: the connection task is blocked inside the handler, so a
        // command it can only read after the handler returns would deadlock.
        let sender = self.pending.lock().await.remove(request_id);
        match sender {
            Some(tx) => {
                let _ = tx.send(option_id);
                Ok(())
            }
            None => anyhow::bail!("no permission request {request_id} is waiting"),
        }
    }

    /// Answers a question the agent asked.
    ///
    /// `None` cancels it. **There is deliberately no way to *decline*:** the
    /// adapter turns a decline into *answered, with no answers*, so the agent
    /// proceeds having asked and heard nothing — which is the failure this
    /// whole feature exists to prevent, reachable through a button somebody
    /// would have called "dismiss".
    pub async fn answer(
        &self,
        request_id: &str,
        content: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let sender = self.asked.lock().await.remove(request_id);
        match sender {
            Some(tx) => {
                let _ = tx.send(content);
                Ok(())
            }
            None => anyhow::bail!("no question {request_id} is waiting"),
        }
    }

    /// Asks the session to end.
    ///
    /// Effective mid-turn: the connection sends ACP `session/cancel`, which the
    /// protocol requires an agent to answer with `stop_reason: cancelled`, and
    /// then tears the connection down — which is what kills the agent's process
    /// group. A session that could only be stopped between turns would leave a
    /// model running, and spending, for as long as it felt like.
    pub fn stop(&self) {
        self.stop.notify_waiters();
        // A session that has not started a turn yet is parked on the command
        // channel rather than on the notify, so wake that too.
        self.stop.notify_one();
    }

    /// Whether the session is still accepting commands.
    pub fn is_live(&self) -> bool {
        !self.commands.is_closed()
    }
}

/// Starts an agent and returns a handle plus its event stream.
///
/// The agent is spawned by the protocol crate, as the leader of its own process
/// group, with a guard that kills that group when the connection is torn down.
/// Devplane never signals an agent itself; its part is to tear the connection
/// down, which [`Session::stop`] does and [`crate::daemon::AppState::shutdown`]
/// does for all of them at once.
///
/// That distinction is load-bearing. The process does **not** die simply
/// because this one exits: a daemon that returns from `main` never drops those
/// tasks, so the guard never runs and every agent it started re-parents to init
/// and keeps spending.
pub async fn spawn(
    spec: &AgentSpec,
    cwd: PathBuf,
) -> anyhow::Result<(Session, mpsc::Receiver<AcpEvent>)> {
    connect(spec, cwd, None).await
}

/// Starts the agent and continues an existing conversation instead of opening
/// a new one.
///
/// The difference is the whole reason `agent_session` is recorded. A pipeline
/// interrupted by a daemon restart has a worktree with half-finished work in
/// it; starting a *new* session there would pay a second time to rediscover
/// what the first one already knew, and would do it without the context that
/// produced the code it is looking at.
///
/// **Two ways to continue a session, advertised separately.**
/// `session/resume` continues without replaying history;
/// `session/load` (`agentCapabilities.loadSession`) continues with it. Resume
/// is preferred where offered — Devplane already holds the transcript, so a
/// replay only duplicates it. GitHub Copilot offers load and not resume.
///
/// An agent with neither is refused rather than downgraded to a new session:
/// silently opening a fresh conversation and calling it a resume is the kind of
/// lie this product exists to not tell.
pub async fn resume(
    spec: &AgentSpec,
    cwd: PathBuf,
    agent_session: String,
) -> anyhow::Result<(Session, mpsc::Receiver<AcpEvent>)> {
    connect(spec, cwd, Some(agent_session)).await
}

async fn connect(
    spec: &AgentSpec,
    cwd: PathBuf,
    resuming: Option<String>,
) -> anyhow::Result<(Session, mpsc::Receiver<AcpEvent>)> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(32);
    let (ev_tx, ev_rx) = mpsc::channel::<AcpEvent>(1024);
    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    let asked: Asked = Arc::new(Mutex::new(HashMap::new()));
    let stop: Arc<Notify> = Arc::new(Notify::new());

    let agent = AcpAgent::from_str(&spec.command)
        .map_err(|e| anyhow::anyhow!("cannot start `{}`: {e}", spec.command))?;

    let session = Session {
        commands: cmd_tx,
        pending: pending.clone(),
        asked: asked.clone(),
        stop: stop.clone(),
    };

    let ev_for_notify = ev_tx.clone();
    let ev_for_perm = ev_tx.clone();
    let pending_for_perm = pending.clone();
    let ev_for_ask = ev_tx.clone();
    let asked_for_ask = asked.clone();
    // `session/load` replays the conversation, as ordinary notifications
    // arriving before the response. Devplane already holds that transcript, so
    // taking the replay too would write every sentence down twice — the same
    // reasoning that drops `user_message_chunk`.
    let replaying = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let replaying_for_notify = replaying.clone();

    tokio::spawn(async move {
        let result = agent_client_protocol::Client
            .builder()
            .name("devplane")
            .on_receive_notification(
                async move |notification: SessionNotification, _cx| {
                    let replaying = replaying_for_notify.load(std::sync::atomic::Ordering::Relaxed);
                    for event in map_update(notification.update) {
                        // Only the conversation is suppressed. A plan, a usage
                        // update or a tool call replayed here still says
                        // something true about where the session got to, and
                        // none of them is written down twice.
                        if replaying && event.is_conversation() {
                            continue;
                        }
                        let _ = ev_for_notify.send(event).await;
                    }
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: RequestPermissionRequest, responder, cx| {
                    // Waiting happens off the handler. A human may take ten
                    // minutes to answer, and a handler that blocks stops the
                    // connection reading anything else for that long — which
                    // includes the reply it is waiting for.
                    let events = ev_for_perm.clone();
                    let pending = pending_for_perm.clone();
                    cx.spawn(async move {
                        let decision = ask(&events, &pending, &request).await;
                        let outcome = match decision {
                            Some(id) => RequestPermissionOutcome::Selected(
                                SelectedPermissionOutcome::new(id),
                            ),
                            None => RequestPermissionOutcome::Cancelled,
                        };
                        responder.respond(RequestPermissionResponse::new(outcome))
                    })
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |request: CreateElicitationRequest, responder, cx| {
                    let events = ev_for_ask.clone();
                    let asked = asked_for_ask.clone();
                    cx.spawn(async move {
                        let outcome = ask_person(&events, &asked, &request).await;
                        responder.respond(CreateElicitationResponse::new(outcome))
                    })
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, |connection: ConnectionTo<Agent>| async move {
                run(
                    connection,
                    cwd,
                    resuming,
                    cmd_rx,
                    ev_tx.clone(),
                    stop,
                    replaying,
                )
                .await
            })
            .await;

        if let Err(e) = result {
            tracing::warn!(error = %e, "acp session ended with an error");
        }
    });

    Ok((session, ev_rx))
}

/// What an agent said about signing in, as a sentence for a person.
///
/// `auth/login` is not implemented: a login belongs in the agent's own
/// terminal, where its device codes and browser redirects already work. So the
/// useful thing to do with `authMethods` is repeat it when it matters.
fn describe_auth(methods: &[agent_client_protocol::schema::v1::AuthMethod]) -> Option<String> {
    if methods.is_empty() {
        return None;
    }
    let listed: Vec<String> = methods
        .iter()
        .map(|m| match m.description() {
            Some(d) => format!("  • {} — {d}", m.name()),
            None => format!("  • {}", m.name()),
        })
        .collect();
    Some(format!(
        "This agent may need signing in first. It offers:\n{}",
        listed.join("\n")
    ))
}

/// The body of the connection: initialise, open a session, then serve commands
/// until the daemon stops asking.
async fn run(
    connection: ConnectionTo<Agent>,
    cwd: PathBuf,
    resuming: Option<String>,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<AcpEvent>,
    stop: Arc<Notify>,
    // Set while `session/load` is replaying a conversation, so the replay is
    // not written down a second time.
    replaying: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> agent_client_protocol::Result<()> {
    // **Declaring this is what makes an agent able to ask a question at all.**
    // The Claude adapter offers its `AskUserQuestion` tool only to a client that
    // says it can render a form elicitation, and Devplane declared no
    // capabilities at all until 2026-09-19 — so the tool was withheld, the agent
    // asked in prose, and the run went idle with nothing raised. Measured, not
    // read: the adapter's own source gates on `clientCapabilities.elicitation.form`.
    let init = connection
        .send_request(
            InitializeRequest::new(ProtocolVersion::V1).client_capabilities(
                ClientCapabilities::new().elicitation(
                    ElicitationCapabilities::new().form(ElicitationFormCapabilities::new()),
                ),
            ),
        )
        .block_task()
        .await?;

    // **Measured before anything is asked of the agent.** What it supports is
    // advertised here and nowhere else, and a record written after a session
    // exists would miss an agent whose session creation fails.
    let _ = events
        .send(AcpEvent::Capabilities {
            agent_name: init.agent_info.as_ref().map(|i| i.name.clone()),
            resume: init
                .agent_capabilities
                .session_capabilities
                .resume
                .is_some(),
            load_session: init.agent_capabilities.load_session,
            list_sessions: init.agent_capabilities.session_capabilities.list.is_some(),
            needs_auth: !init.auth_methods.is_empty(),
        })
        .await;

    // **The roster, where the agent has one.** Asked once, after the handshake
    // and before anything is asked of the agent, under the rule the capability
    // record already states: a runtime fact about *that* agent at *that*
    // version, never a property of this product.
    //
    // Errors are dropped on purpose. A listing is enrichment — an agent that
    // advertises the method and then refuses it leaves the run exactly as
    // observation found it, which is the correct outcome and not a failure
    // worth ending a session over.
    if init.agent_capabilities.session_capabilities.list.is_some() {
        let listed = connection
            .send_request(agent_client_protocol::schema::v1::ListSessionsRequest::default())
            .block_task()
            .await;
        if let Ok(page) = listed {
            let sessions: Vec<crate::core::run::ListedSession> = page
                .sessions
                .into_iter()
                .map(|s| crate::core::run::ListedSession {
                    agent_session: s.session_id.to_string(),
                    title: s.title,
                })
                .collect();
            if !sessions.is_empty() {
                let _ = events.send(AcpEvent::SessionsListed { sessions }).await;
            }
        }
        // **One page, deliberately.** The response is cursor-paged, and the
        // question this answers — *does the agent know a name for the session
        // in front of me* — is answered by the page the session is on or not at
        // all. Walking every page of somebody's history to fill a title is a
        // cost with no reader.
    }

    let mut initial_mode: Option<String> = None;
    let session_id = match resuming {
        Some(previous) => {
            // Checked before it is attempted: an agent that supports neither
            // answers with an error, and the caller would then have to decide
            // whether the work was continued or silently restarted.
            let can_resume = init
                .agent_capabilities
                .session_capabilities
                .resume
                .is_some();
            let can_load = init.agent_capabilities.load_session;
            if !can_resume && !can_load {
                let _ = events
                    .send(AcpEvent::Ended {
                        error: Some(format!(
                            "{} supports neither session/resume nor session/load, so this \
                             conversation cannot be continued",
                            init.agent_info
                                .as_ref()
                                .map(|i| i.name.clone())
                                .unwrap_or_else(|| "this agent".into())
                        )),
                    })
                    .await;
                return Ok(());
            }
            // `resume` first where it exists: it continues without replaying,
            // and Devplane already holds the transcript.
            let sid = agent_client_protocol::schema::v1::SessionId::new(previous.clone());
            if !can_resume {
                replaying.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let resumed = if can_resume {
                connection
                    .send_request(ResumeSessionRequest::new(sid, cwd.as_path()))
                    .block_task()
                    .await
                    .map(|_| ())
            } else {
                connection
                    .send_request(agent_client_protocol::schema::v1::LoadSessionRequest::new(
                        sid,
                        cwd.as_path(),
                    ))
                    .block_task()
                    .await
                    .map(|_| ())
            };
            replaying.store(false, std::sync::atomic::Ordering::Relaxed);
            match resumed {
                Ok(()) => agent_client_protocol::schema::v1::SessionId::new(previous),
                Err(e) => {
                    // The session is gone from the agent's side — expired, or
                    // the agent keeps nothing across process restarts. Saying
                    // so is the useful answer; starting a fresh conversation
                    // under the same run would be a restart wearing a resume's
                    // name.
                    let _ = events
                        .send(AcpEvent::Ended {
                            error: Some(format!("that session could not be continued: {e}")),
                        })
                        .await;
                    return Ok(());
                }
            }
        }
        None => {
            match connection
                .send_request(NewSessionRequest::new(cwd))
                .block_task()
                .await
            {
                Ok(r) => {
                    // **The mode a session starts in, which no update reports.**
                    // `current_mode_update` fires when a mode *changes*, so an
                    // agent that starts in one mode and stays there would never
                    // be reported at all — and that is the case the surface
                    // most wants: a session nobody is supervising, sitting in
                    // the mode it began in.
                    initial_mode = r.modes.as_ref().map(|m| m.current_mode_id.to_string());
                    r.session_id
                }
                Err(e) => {
                    // An agent that needs signing in advertises `authMethods`
                    // at initialize, often with the literal command to run.
                    // Repeating it beats a raw protocol error, which makes a
                    // login look like a broken integration.
                    //
                    // The message says *may*: the presence of those methods
                    // means authentication exists here, not that this error was
                    // an authentication failure. Guessing that is inference.
                    let hint = describe_auth(&init.auth_methods);
                    let _ = events
                        .send(AcpEvent::Ended {
                            error: Some(match hint {
                                Some(h) => format!("could not start a session: {e}.\n{h}"),
                                None => format!("could not start a session: {e}"),
                            }),
                        })
                        .await;
                    return Ok(());
                }
            }
        }
    };

    let _ = events
        .send(AcpEvent::Ready {
            // `Display` renders the protocol value; `Debug` would wrap it in
            // the type name, and an id that round-trips as `SessionId("x")`
            // is an id the agent does not recognise.
            session_id: session_id.to_string(),
            agent_name: init.agent_info.as_ref().map(|i| i.name.clone()),
        })
        .await;

    // After `Ready`, because the run has to exist before anything can be
    // recorded against it.
    if let Some(mode) = initial_mode {
        let _ = events.send(AcpEvent::ModeChanged { mode }).await;
    }

    loop {
        let command = tokio::select! {
            // Biased so a stop that arrives with a queued prompt wins: the
            // caller asked for the session to end, not for one more turn.
            biased;
            _ = stop.notified() => break,
            c = commands.recv() => match c {
                Some(c) => c,
                None => break,
            },
        };

        match command {
            Command::Prompt(text) => {
                let turn = connection
                    .send_request(PromptRequest::new(
                        session_id.clone(),
                        vec![ContentBlock::Text(TextContent::new(text))],
                    ))
                    .block_task();
                tokio::pin!(turn);

                let response = tokio::select! {
                    r = &mut turn => r,
                    _ = stop.notified() => {
                        // The protocol requires the agent to answer a cancel
                        // with `stop_reason: cancelled`, so ask properly and
                        // give it a moment before tearing the connection down.
                        let _ = connection.send_notification(CancelNotification::new(
                            session_id.clone(),
                        ));
                        match tokio::time::timeout(CANCEL_GRACE, &mut turn).await {
                            Ok(r) => r,
                            Err(_) => {
                                tracing::debug!("agent did not acknowledge the cancel");
                                break;
                            }
                        }
                    }
                };

                match response {
                    Ok(r) => {
                        let cancelled = r.stop_reason == StopReason::Cancelled;
                        let _ = events
                            .send(AcpEvent::TurnEnded {
                                stop_reason: stop_reason_name(r.stop_reason).to_string(),
                            })
                            .await;
                        if cancelled {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = events
                            .send(AcpEvent::Ended {
                                error: Some(e.to_string()),
                            })
                            .await;
                        break;
                    }
                }
            }
        }
    }

    let _ = events.send(AcpEvent::Ended { error: None }).await;
    Ok(())
}

/// The wire name of a stop reason.
///
/// Spelled out rather than lowercased from `Debug`: `MaxTurnRequests` would
/// become `maxturnrequests`, and the daemon matches on these.
fn stop_reason_name(r: StopReason) -> &'static str {
    match r {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::MaxTurnRequests => "max_turn_requests",
        StopReason::Refusal => "refusal",
        StopReason::Cancelled => "cancelled",
        _ => "unknown",
    }
}

/// Publishes a permission request and waits for the answer.
async fn ask(
    events: &mpsc::Sender<AcpEvent>,
    pending: &Pending,
    request: &RequestPermissionRequest,
) -> Option<String> {
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let (tx, rx) = oneshot::channel();
    pending.lock().await.insert(request_id.clone(), tx);

    let options: Vec<PermissionOption> = request
        .options
        .iter()
        .map(|o| PermissionOption {
            id: o.option_id.to_string(),
            label: o.name.clone(),
            kind: option_kind_name(o.kind).to_string(),
        })
        .collect();

    let _ = events
        .send(AcpEvent::PermissionRequested {
            request_id: request_id.clone(),
            call: describe(request),
            options,
        })
        .await;

    // **It waits.** Not for ten minutes, not for a number in this file: until a
    // person answers, until the project's own deadline sweeps it, or until the
    // connection goes away. Those are the only three things that end it and
    // each of them has a name on it.
    let answer = rx.await;
    pending.lock().await.remove(&request_id);

    // An `Err` is the sender being dropped: the daemon is tearing this session
    // down, or a deadline sweep closed the ask and released the wait. Either
    // way the ending has already been recorded by whoever did it, with its own
    // authority — so nothing is invented here and nothing is announced twice.
    // The agent is told no, which is the only safe thing to say when the answer
    // is that there is no longer anybody to ask.
    answer.unwrap_or_default()
}

/// Publishes a question and waits — with no timeout — for the person.
///
/// **No timeout is the feature, and since 2026-09-20 it is the rule on both
/// channels rather than a distinction between them.** Neither a permission nor
/// a question has a safe default, and inventing one is the thing this product
/// refuses to do — so both wait, and the only clock that ever ends either is
/// one a project wrote down for itself. The wait happens off the handler, so a
/// question left overnight does not stop the connection reading anything else.
///
/// **Cancel, never decline.** `decline` is folded by the adapter into
/// *answered, with no answers*, so the agent proceeds as though it had asked and
/// been told nothing. `cancel` stops the call instead, which is the honest
/// outcome when there is no answer to give.
async fn ask_person(
    events: &mpsc::Sender<AcpEvent>,
    asked: &Asked,
    request: &CreateElicitationRequest,
) -> ElicitationAction {
    let Some(parsed) = serde_json::to_value(request)
        .ok()
        .as_ref()
        .and_then(crate::core::question::Ask::parse)
    else {
        // The same method carries elicitations from MCP servers, in shapes this
        // cannot render. Guessing at one would put words in somebody's mouth,
        // so it is cancelled — **and said out loud**.
        //
        // Cancelling silently is the exact failure this whole feature exists to
        // prevent: an agent asked a person something, a person was never told,
        // and the only trace was the agent giving up. Devplane declared it could
        // render a form; where it turns out it cannot, that is news.
        let _ = events
            .send(AcpEvent::QuestionUnrenderable {
                what: serde_json::to_value(request)
                    .ok()
                    .and_then(|v| {
                        v.get("message")
                            .and_then(|m| m.as_str())
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| "an agent asked something".to_string()),
            })
            .await;
        return ElicitationAction::Cancel;
    };

    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let (tx, rx) = oneshot::channel();
    asked.lock().await.insert(request_id.clone(), tx);

    let _ = events
        .send(AcpEvent::QuestionAsked {
            request_id: request_id.clone(),
            ask: parsed,
        })
        .await;

    let answered = rx.await;
    asked.lock().await.remove(&request_id);

    match answered {
        // The protocol's own content type rather than raw JSON: converting here
        // means a shape the schema would reject fails at the boundary, where it
        // can still be turned into a cancel, instead of on the wire.
        Ok(Some(content)) => match serde_json::from_value::<
            std::collections::BTreeMap<
                String,
                agent_client_protocol::schema::v1::ElicitationContentValue,
            >,
        >(content)
        {
            Ok(map) => ElicitationAction::Accept(
                agent_client_protocol::schema::v1::ElicitationAcceptAction::new().content(map),
            ),
            Err(_) => ElicitationAction::Cancel,
        },
        _ => {
            let _ = events
                .send(AcpEvent::QuestionCancelled {
                    request_id: request_id.clone(),
                })
                .await;
            ElicitationAction::Cancel
        }
    }
}

/// The wire name of a permission option kind, spelled out for the same reason
/// as [`stop_reason_name`]: `AllowOnce` lowercased is `allowonce`, and a policy
/// that matched on that would be matching on an accident.
fn option_kind_name(k: agent_client_protocol::schema::v1::PermissionOptionKind) -> &'static str {
    use agent_client_protocol::schema::v1::PermissionOptionKind as K;
    match k {
        K::AllowOnce => "allow_once",
        K::AllowAlways => "allow_always",
        K::RejectOnce => "reject_once",
        K::RejectAlways => "reject_always",
        _ => "unknown",
    }
}

/// What the agent is asking to do, in the protocol's own terms.
fn describe(request: &RequestPermissionRequest) -> ToolRequest {
    let f = &request.tool_call.fields;
    ToolRequest {
        id: request.tool_call.tool_call_id.to_string(),
        title: crate::core::text::clip(
            f.title
                .as_deref()
                .unwrap_or(&request.tool_call.tool_call_id.to_string()),
            200,
        ),
        kind: f.kind.as_ref().and_then(wire),
        raw_input: f.raw_input.clone(),
        locations: f
            .locations
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|l| l.path.clone())
            .collect(),
    }
}

/// Maps one protocol update to zero or more events.
fn map_update(update: SessionUpdate) -> Vec<AcpEvent> {
    match update {
        SessionUpdate::AgentMessageChunk(c) => {
            text_of(&c).map(AcpEvent::Text).into_iter().collect()
        }
        SessionUpdate::AgentThoughtChunk(c) => {
            text_of(&c).map(AcpEvent::Thought).into_iter().collect()
        }
        SessionUpdate::ToolCall(t) => vec![AcpEvent::Tool {
            call: ToolRequest {
                id: t.tool_call_id.to_string(),
                title: t.title.clone(),
                kind: wire(&t.kind),
                raw_input: t.raw_input.clone(),
                locations: t.locations.iter().map(|l| l.path.clone()).collect(),
            },
            status: wire(&t.status),
        }],
        // An update with no status is content streaming in — the tool's output
        // arriving a line at a time. Reporting it as a tool call would count one
        // per chunk, and blank the board's summary, because an update need not
        // repeat the title.
        SessionUpdate::ToolCallUpdate(t) if t.fields.status.is_none() => Vec::new(),
        SessionUpdate::ToolCallUpdate(t) => vec![AcpEvent::Tool {
            call: ToolRequest {
                id: t.tool_call_id.to_string(),
                title: t.fields.title.clone().unwrap_or_default(),
                kind: t.fields.kind.as_ref().and_then(wire),
                raw_input: t.fields.raw_input.clone(),
                locations: t
                    .fields
                    .locations
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(|l| l.path.clone())
                    .collect(),
            },
            status: t.fields.status.as_ref().and_then(wire),
        }],
        SessionUpdate::UsageUpdate(u) => vec![AcpEvent::Usage {
            // The protocol reports cost in whatever currency the agent bills
            // in. Only dollars are carried through, because the board adds the
            // figures up and a total that silently mixes currencies is worse
            // than one that admits it does not know.
            cost_usd: u
                .cost
                .as_ref()
                .filter(|c| c.currency.eq_ignore_ascii_case("usd"))
                .map(|c| c.amount),
            context_tokens: Some(u.used),
            context_window: Some(u.size).filter(|s| *s > 0),
        }],
        SessionUpdate::Plan(p) => vec![AcpEvent::Plan(
            p.entries
                .iter()
                .map(|e| PlanStep {
                    content: e.content.clone(),
                    status: wire(&e.status).unwrap_or_else(|| "pending".into()),
                })
                .collect(),
        )],
        // The user's own message, replayed when a session is resumed. Devplane
        // records what it sent at the moment it sends it, so taking this too
        // would write every prompt down twice.
        SessionUpdate::UserMessageChunk(_) => Vec::new(),
        // **The cross-vendor answer to *which sessions decide without you*.**
        // `devplane modes` reads one vendor's settings files; this is the same
        // question asked of any agent that speaks the protocol.
        SessionUpdate::CurrentModeUpdate(m) => vec![AcpEvent::ModeChanged {
            mode: m.current_mode_id.to_string(),
        }],
        // Command lists are real and nothing consumes them yet. An event the
        // product does nothing with is noise in the log.
        _ => Vec::new(),
    }
}

/// The wire name of a protocol enum, asked of serde rather than reconstructed.
///
/// A protocol value crossing a boundary is never derived from `Debug`.
/// Lowercasing it and re-inserting underscores agrees with the wire format for
/// `InProgress` and stops agreeing at the first variant spelled differently;
/// serde already holds the answer, because serde is what put it there.
fn wire<T: serde::Serialize>(value: &T) -> Option<String> {
    match serde_json::to_value(value).ok()? {
        serde_json::Value::String(s) => Some(s),
        _ => None,
    }
}

fn text_of(chunk: &agent_client_protocol::schema::v1::ContentChunk) -> Option<String> {
    match &chunk.content {
        ContentBlock::Text(t) => Some(t.text.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_decision_for_nothing_is_an_error_not_a_panic() {
        let (tx, _rx) = mpsc::channel(1);
        let s = Session {
            commands: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            asked: Arc::new(Mutex::new(HashMap::new())),
            stop: Arc::new(Notify::new()),
        };
        assert!(s.decide("nope", Some("x".into())).await.is_err());
    }

    #[tokio::test]
    async fn an_unanswered_permission_refuses_rather_than_hanging() {
        // The real timeout is ten minutes; the mechanism is what is tested.
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (tx, _rx) = oneshot::channel::<Option<String>>();
        pending.lock().await.insert("r1".into(), tx);

        let answer = tokio::time::timeout(Duration::from_millis(10), async {
            let (_tx2, rx2) = oneshot::channel::<Option<String>>();
            rx2.await
        })
        .await;
        assert!(answer.is_err(), "a timeout is a refusal, not a hang");
    }

    #[test]
    fn protocol_enums_use_their_wire_names_not_a_debug_string() {
        // The names on the far side of this boundary are matched on by string,
        // and serde is what put them on the wire — so serde is what is asked,
        // rather than a transformation of `Debug` that happens to agree.
        use agent_client_protocol::schema::v1::{ToolCallStatus, ToolKind};
        assert_eq!(
            wire(&ToolCallStatus::InProgress).as_deref(),
            Some("in_progress")
        );
        assert_eq!(
            wire(&ToolCallStatus::Completed).as_deref(),
            Some("completed")
        );
        assert_eq!(wire(&ToolKind::Execute).as_deref(), Some("execute"));
        assert_eq!(wire(&ToolKind::SwitchMode).as_deref(), Some("switch_mode"));
        assert_eq!(
            stop_reason_name(StopReason::MaxTurnRequests),
            "max_turn_requests"
        );
        assert_eq!(stop_reason_name(StopReason::EndTurn), "end_turn");
        assert_eq!(stop_reason_name(StopReason::Cancelled), "cancelled");
    }

    #[tokio::test]
    async fn a_stopped_session_reports_itself_as_dead() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        let s = Session {
            commands: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            asked: Arc::new(Mutex::new(HashMap::new())),
            stop: Arc::new(Notify::new()),
        };
        assert!(s.prompt("hello").await.is_err());
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    /// The one line that decides whether an agent can ask a question at all.
    ///
    /// Until 2026-09-19 Devplane sent no client capabilities, and the Claude
    /// adapter withheld its question tool for exactly that reason — the agent
    /// asked in prose instead and the run went idle with nothing raised. The
    /// adapter gates on `clientCapabilities.elicitation.form`, so this asserts
    /// the wire shape rather than the builder call: a rename upstream that kept
    /// the method and changed the JSON would pass a test written the other way
    /// and silently take the feature away again.
    #[test]
    fn the_initialize_request_says_it_can_render_a_question() {
        let init = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(
            ClientCapabilities::new().elicitation(
                ElicitationCapabilities::new().form(ElicitationFormCapabilities::new()),
            ),
        );
        let v = serde_json::to_value(&init).expect("serialises");
        assert_eq!(
            v["clientCapabilities"]["elicitation"]["form"],
            serde_json::json!({}),
            "the adapter tests `clientCapabilities?.elicitation?.form`"
        );
    }
}

#[cfg(test)]
mod answer_tests {
    use super::*;

    fn waiting() -> (Asked, oneshot::Receiver<Option<serde_json::Value>>) {
        let (tx, rx) = oneshot::channel();
        let mut m = HashMap::new();
        m.insert("q1".to_string(), tx);
        (Arc::new(Mutex::new(m)), rx)
    }

    /// Two tabs, or a phone and a laptop, answering at once.
    ///
    /// The winner is whoever takes the waiter out of the map; the loser is told
    /// there is no question waiting, which is a **normal outcome** rather than
    /// an error to show. The alternative — both answers reaching the agent — is
    /// the one bug this feature cannot afford.
    #[tokio::test]
    async fn an_answer_is_delivered_exactly_once() {
        let (asked, rx) = waiting();
        let s = Session {
            commands: mpsc::channel(1).0,
            pending: Arc::new(Mutex::new(HashMap::new())),
            asked: asked.clone(),
            stop: Arc::new(Notify::new()),
        };
        let first = s
            .answer("q1", Some(serde_json::json!({"question_0": "a"})))
            .await;
        let second = s
            .answer("q1", Some(serde_json::json!({"question_0": "b"})))
            .await;

        assert!(first.is_ok(), "the first answer is delivered");
        assert!(second.is_err(), "the second finds nothing waiting");
        assert_eq!(
            rx.await.unwrap(),
            Some(serde_json::json!({"question_0": "a"})),
            "the agent receives the first answer, once"
        );
    }

    /// There is deliberately no `decline`.
    ///
    /// The adapter folds a decline into *answered, with no answers*, so the
    /// agent proceeds having asked and heard nothing. A "dismiss" button would
    /// be that failure with a friendly name on it, so the only two things that
    /// can happen to a question are an answer and a cancel.
    #[tokio::test]
    async fn nothing_can_answer_a_question_with_nothing() {
        let (asked, rx) = waiting();
        let s = Session {
            commands: mpsc::channel(1).0,
            pending: Arc::new(Mutex::new(HashMap::new())),
            asked,
            stop: Arc::new(Notify::new()),
        };
        // `None` is the cancel path, and the waiter must read it as a cancel
        // rather than as an empty answer.
        s.answer("q1", None).await.expect("cancel is deliverable");
        assert_eq!(rx.await.unwrap(), None, "a cancel is not an empty answer");
    }
}

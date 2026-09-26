//! A driven session: an ACP agent Devplane owns, run as an actor — a task
//! that owns the connection, takes commands on one channel and emits events on
//! another, so the reducer never sees protocol types. The permission handler
//! parks each request on a one-shot channel and waits off the connection task
//! until a person answers, the project's deadline sweeps it, or the session
//! stops (which answers every parked request with *cancelled* first).

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::StopReason;
use agent_client_protocol::schema::v1::{
    CancelNotification, ClientCapabilities, CloseSessionRequest, ContentBlock,
    CreateElicitationRequest, CreateElicitationResponse, ElicitationAction,
    ElicitationCapabilities, ElicitationFormCapabilities, EnvVariable, InitializeRequest,
    McpServer, McpServerStdio, NewSessionRequest, PromptRequest, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, ResumeSessionRequest,
    SelectedPermissionOutcome, SessionNotification, SessionUpdate, TextContent, ToolCallContent,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectionTo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify, mpsc, oneshot};

use super::agent::AgentSpec;

// There is no permission timeout here: the only clock that ends a wait is a
// deadline the project sets in its own `devplane.toml` (default `never`),
// swept by the host in `core::ask`.

/// How long a cancelled turn has to acknowledge before the connection — and
/// with it the agent's process group — is torn down anyway.
const CANCEL_GRACE: Duration = Duration::from_secs(5);

/// How long an agent that advertises `session/close` has to answer one before
/// the connection is torn down anyway.
const CLOSE_GRACE: Duration = Duration::from_secs(2);

/// The most of one side of a reported diff that is kept; the rest is counted.
pub const DIFF_KEPT_BYTES: usize = 64 * 1024;

/// A diff an agent reported in a tool call's content: its own claim about an
/// edit, never evidence — the worktree is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedDiff {
    pub path: PathBuf,
    /// `None` for a file the agent says it created.
    pub old_text: Option<String>,
    pub new_text: String,
}

/// Devplane's own read-only MCP server, offered to the agent on
/// `session/new` and `session/resume` as a stdio server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOffer {
    pub name: String,
    pub command: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl McpOffer {
    fn wire(&self) -> McpServer {
        McpServer::Stdio(
            McpServerStdio::new(self.name.clone(), self.command.clone())
                .args(self.args.clone())
                .env(
                    self.env
                        .iter()
                        .map(|(k, v)| EnvVariable::new(k.clone(), v.clone()))
                        .collect(),
                ),
        )
    }
}

/// What a driven session reports.
///
/// Its own type rather than the domain's, so the reducer does not depend on a
/// protocol version.
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
    /// The agent's plan for the turn: what it intends to do, and how far it
    /// has got.
    Plan(Vec<PlanStep>),
    /// A tool call started or changed.
    Tool {
        call: ToolRequest,
        /// `pending`, `in_progress`, `completed`, `failed`. Absent when the
        /// update carried no status.
        status: Option<String>,
    },
    /// The agent withdrew a permission request before anybody answered it
    /// (`$/cancel_request`): nothing waits on an answer any more.
    PermissionWithdrawn { request_id: String },
    /// The agent withdrew a question before anybody answered it.
    QuestionWithdrawn { request_id: String },
    /// The diffs a tool call's content carries, whole: the protocol replaces a
    /// call's content on every update, so the latest set is the complete one.
    Diffs {
        call_id: String,
        diffs: Vec<ReportedDiff>,
    },
    /// Whether the session was closed over the protocol (`session/close`)
    /// before the connection went: `false` when the agent does not offer it
    /// or did not answer in time.
    Closed { sent: bool },
    /// The agent wants permission. Answer it with [`Session::decide`].
    PermissionRequested {
        request_id: String,
        call: ToolRequest,
        options: Vec<PermissionOption>,
    },
    /// The agent asked the person a question. Answer it with
    /// [`Session::answer`].
    ///
    /// Not a permission: no rule can answer it, and it arrives on a different
    /// protocol method.
    QuestionAsked {
        request_id: String,
        ask: crate::core::question::Ask,
    },
    /// What the agent advertised at `initialize`, emitted once per connection
    /// after the handshake. Carries no command: the caller keys the record
    /// with the spec it launched.
    Capabilities {
        agent_name: Option<String>,
        resume: bool,
        load_session: bool,
        list_sessions: bool,
        needs_auth: bool,
    },
    /// What the agent says its own sessions are, where it advertises
    /// `session/list`. Asked once; a roster that can fill a missing title but
    /// never says a session is waiting.
    SessionsListed {
        sessions: Vec<crate::core::run::ListedSession>,
    },
    /// The agent says which mode it is now in: the agent's own identifier,
    /// never mapped onto Claude Code's modes, since no specification defines
    /// that equivalence.
    ModeChanged { mode: String },
    /// The agent asked something over the question channel that Devplane could
    /// not render, so it was cancelled — reported, because the agent was told
    /// a form could be shown.
    QuestionUnrenderable { what: String },
    /// A question was cancelled without being answered — the session ended, or
    /// somebody stopped the run — so the inbox stops offering an answer.
    QuestionCancelled { request_id: String },
    /// Usage the agent reported for the session, window size included.
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
    /// fact about where the session got to. Used to suppress a `session/load`
    /// replay; state events are idempotent to see again.
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

/// What an agent is asking to do, in the protocol's own terms. The title is
/// prose for a human; the kind, `raw_input` and `locations` are what a rule
/// evaluates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolRequest {
    /// The protocol's id for this call, which a start and a finish are
    /// correlated on.
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
    /// This call in the policy's vocabulary. The protocol sends no stable tool
    /// name, so this comes from the declared kind or, failing that, the shape
    /// of the arguments — never the title. `None` means no rule covers it: the
    /// policy is skipped and a person is asked.
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

/// Waiting questions. Separate from [`Pending`] because an answer is a map of
/// fields to what the person chose or typed, not an option id.
type Asked = Arc<Mutex<HashMap<String, oneshot::Sender<Option<serde_json::Value>>>>>;

/// A handle on a running agent. Prompts go down a channel; answers reach the
/// waiting handler directly, because the connection task is parked inside it
/// and a queued command would deadlock the pair.
#[derive(Debug, Clone)]
pub struct Session {
    prompts: mpsc::Sender<String>,
    pending: Pending,
    asked: Asked,
    /// Raised by [`Session::stop`]. Separate from the prompt channel, which
    /// is not read while a turn is in flight.
    stop: Arc<Notify>,
    /// Why the caller stopped it, read back at the end to tell a person's stop
    /// from the host's own.
    stopped_because: Arc<std::sync::Mutex<Option<String>>>,
    /// Set by [`Session::stop`], so a session on its way out is never handed
    /// another prompt or mistaken for one that can take an answer.
    stopping: Arc<std::sync::atomic::AtomicBool>,
}

impl Session {
    fn with_channels(
        prompts: mpsc::Sender<String>,
        pending: Pending,
        asked: Asked,
        stop: Arc<Notify>,
    ) -> Self {
        Self {
            prompts,
            pending,
            asked,
            stop,
            stopped_because: Arc::new(std::sync::Mutex::new(None)),
            stopping: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Sends a prompt. Returns once the command is queued, not once the turn
    /// is done — the turn's progress arrives as events.
    pub async fn prompt(&self, text: impl Into<String>) -> anyhow::Result<()> {
        if self.is_stopping() {
            anyhow::bail!("the session is stopping");
        }
        self.prompts
            .send(text.into())
            .await
            .map_err(|_| anyhow::anyhow!("the session has ended"))
    }

    /// Answers a permission request. `None` refuses it.
    pub async fn decide(&self, request_id: &str, option_id: Option<String>) -> anyhow::Result<()> {
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
    /// `None` cancels it. There is no way to *decline*: the adapter turns a
    /// decline into *answered, with no answers*, and the agent proceeds on
    /// nothing.
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
    /// Effective mid-turn: sends ACP `session/cancel`, then tears the connection
    /// down, which kills the agent's process group.
    pub fn stop(&self) {
        self.stopping
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.stop.notify_waiters();
        // A session that has not started a turn yet is parked on the prompt
        // channel rather than on the notify, so wake that too.
        self.stop.notify_one();
    }

    /// [`Session::stop`], with the caller's reason kept for whoever reads the
    /// ending.
    pub fn stop_because(&self, why: &str) {
        *self.stopped_because.lock().expect("stop reason") = Some(why.to_string());
        self.stop();
    }

    /// The reason given to [`Session::stop_because`], if one was.
    pub fn stopped_because(&self) -> Option<String> {
        self.stopped_because.lock().expect("stop reason").clone()
    }

    /// Whether the connection is still up. A stopped session stays live until
    /// its agent has gone; see [`Session::is_stopping`].
    pub fn is_live(&self) -> bool {
        !self.prompts.is_closed()
    }

    /// Whether [`Session::stop`] has been called.
    pub fn is_stopping(&self) -> bool {
        self.stopping.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Whether two handles are the same connection, so a pump removes only
    /// its own session from a map a newer one may already occupy.
    pub fn same_as(&self, other: &Session) -> bool {
        Arc::ptr_eq(&self.stop, &other.stop)
    }

    /// The permission requests and questions the agent is waiting on an
    /// answer to, by request id.
    pub async fn waiting(&self) -> Vec<String> {
        let mut out: Vec<String> = self.pending.lock().await.keys().cloned().collect();
        out.extend(self.asked.lock().await.keys().cloned());
        out
    }
}

/// Starts an agent and returns a handle plus its event stream.
///
/// The protocol crate spawns it as the leader of its own process group, killed
/// when the connection is torn down ([`Session::stop`],
/// [`crate::host::AppState::shutdown`]). Returning from `main` does not drop
/// those tasks, so shutdown must tear connections down explicitly.
///
/// `env` (the project's declared build environment) is set over the host's.
pub async fn spawn(
    spec: &AgentSpec,
    cwd: PathBuf,
    env: &[(String, PathBuf)],
    mcp: Option<McpOffer>,
) -> anyhow::Result<(Session, mpsc::Receiver<AcpEvent>)> {
    connect(spec, cwd, None, env, mcp).await
}

/// Starts the agent and continues an existing conversation, so a change
/// interrupted by a restart keeps its context. Prefers `session/resume` (no
/// replay) over `session/load` (`agentCapabilities.loadSession`); an agent
/// with neither is refused rather than silently given a fresh session.
pub async fn resume(
    spec: &AgentSpec,
    cwd: PathBuf,
    agent_session: String,
    env: &[(String, PathBuf)],
    mcp: Option<McpOffer>,
) -> anyhow::Result<(Session, mpsc::Receiver<AcpEvent>)> {
    connect(spec, cwd, Some(agent_session), env, mcp).await
}

async fn connect(
    spec: &AgentSpec,
    cwd: PathBuf,
    resuming: Option<String>,
    env: &[(String, PathBuf)],
    mcp: Option<McpOffer>,
) -> anyhow::Result<(Session, mpsc::Receiver<AcpEvent>)> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<String>(32);
    let (ev_tx, ev_rx) = mpsc::channel::<AcpEvent>(1024);
    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
    let asked: Asked = Arc::new(Mutex::new(HashMap::new()));
    let stop: Arc<Notify> = Arc::new(Notify::new());

    let agent = AcpAgent::from_str(&spec.command)
        .map_err(|e| anyhow::anyhow!("cannot start `{}`: {e}", spec.command))?;
    // The protocol crate spawns the process, so the variables go through its
    // configuration.
    let config = agent.into_config().envs(
        env.iter()
            .map(|(k, v)| (k.clone(), v.to_string_lossy().into_owned())),
    );
    let agent = AcpAgent::new(in_directory(config, &cwd));

    let session = Session::with_channels(cmd_tx, pending.clone(), asked.clone(), stop.clone());
    let pending_for_run = pending.clone();
    let asked_for_run = asked.clone();

    let ev_for_notify = ev_tx.clone();
    let ev_for_perm = ev_tx.clone();
    let pending_for_perm = pending.clone();
    let ev_for_ask = ev_tx.clone();
    let asked_for_ask = asked.clone();
    let ev_for_failure = ev_tx.clone();
    // Set once the connection is up: from then on the run itself sends the
    // one `Ended`, so a failure only reports an agent that never started.
    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started_for_run = started.clone();
    // `session/load` replays the conversation before its response; the
    // transcript is already held, so the replayed prose is dropped.
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
                        // Only the conversation is suppressed; state
                        // updates still say something true.
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
                    // Waiting happens off the handler, which would
                    // otherwise stop the connection reading the very reply it
                    // waits for.
                    let events = ev_for_perm.clone();
                    let pending = pending_for_perm.clone();
                    // The agent may withdraw the request (`$/cancel_request`);
                    // the ask then ends as the agent's act, not anybody's answer.
                    let withdrawn = responder.cancellation();
                    cx.spawn(async move {
                        let request_id = uuid::Uuid::new_v4().simple().to_string();
                        let decision = tokio::select! {
                            d = ask(&events, &pending, &request, &request_id) => d,
                            () = withdrawn.cancelled() => {
                                // Only a request still waiting is withdrawn;
                                // one already answered was not.
                                if pending.lock().await.remove(&request_id).is_some() {
                                    let _ = events
                                        .send(AcpEvent::PermissionWithdrawn { request_id })
                                        .await;
                                }
                                return responder.respond_with_error(
                                    agent_client_protocol::Error::request_cancelled(),
                                );
                            }
                        };
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
                    let withdrawn = responder.cancellation();
                    cx.spawn(async move {
                        let request_id = uuid::Uuid::new_v4().simple().to_string();
                        let outcome = tokio::select! {
                            o = ask_person(&events, &asked, &request, &request_id) => o,
                            () = withdrawn.cancelled() => {
                                if asked.lock().await.remove(&request_id).is_some() {
                                    let _ = events
                                        .send(AcpEvent::QuestionWithdrawn { request_id })
                                        .await;
                                }
                                return responder.respond_with_error(
                                    agent_client_protocol::Error::request_cancelled(),
                                );
                            }
                        };
                        responder.respond(CreateElicitationResponse::new(outcome))
                    })
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, |connection: ConnectionTo<Agent>| async move {
                started_for_run.store(true, std::sync::atomic::Ordering::SeqCst);
                run(
                    connection,
                    cwd,
                    resuming,
                    cmd_rx,
                    ev_tx.clone(),
                    stop,
                    replaying,
                    Parked {
                        pending: pending_for_run,
                        asked: asked_for_run,
                    },
                    mcp.map(|m| vec![m.wire()]).unwrap_or_default(),
                )
                .await
            })
            .await;

        // An agent that could not start ends the run with the reason — never
        // a silent end that reads as finished.
        if let Err(e) = result {
            tracing::warn!(error = %e, "acp session ended with an error");
            if !started.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = ev_for_failure
                    .send(AcpEvent::Ended {
                        error: Some(format!("the agent could not be started: {e}")),
                    })
                    .await;
            }
        }
    });

    Ok((session, ev_rx))
}

/// The launch configuration, run from `dir`. The protocol crate spawns the
/// agent in this process's directory and takes none, and an adapter resolves
/// its own configuration, `npx` packages and `.npmrc` from where it starts —
/// so on Unix the command is started through `sh`, which changes directory
/// and `exec`s it: the same process, the same command line.
fn in_directory(
    config: agent_client_protocol::AcpAgentConfig,
    dir: &std::path::Path,
) -> agent_client_protocol::AcpAgentConfig {
    if cfg!(not(unix)) {
        return config;
    }
    agent_client_protocol::AcpAgentConfig::new("/bin/sh")
        .args(["-c", "cd \"$0\" && exec \"$@\""])
        .arg(dir.to_string_lossy().into_owned())
        .arg(config.command().to_string_lossy().into_owned())
        .args(config.arguments().iter().cloned())
        .envs(config.environment().clone())
}

/// What an agent said about signing in, as a sentence for a person.
///
///
/// `auth/login` is not implemented: a login belongs in the agent's own
/// terminal, so this repeats the advertised `authMethods`.
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

/// The requests an agent is waiting on an answer to.
struct Parked {
    pending: Pending,
    asked: Asked,
}

impl Parked {
    /// Answers everything still waiting with *cancelled*. The protocol
    /// requires it before a cancel: a blocked agent cannot acknowledge one.
    async fn cancel_all(&self) {
        for (_, tx) in self.pending.lock().await.drain() {
            let _ = tx.send(None);
        }
        for (_, tx) in self.asked.lock().await.drain() {
            let _ = tx.send(None);
        }
    }
}

/// What setting up a session produced, when it produced one.
struct Opened {
    session_id: agent_client_protocol::schema::v1::SessionId,
    agent_name: Option<String>,
    initial_mode: Option<String>,
    can_close: bool,
}

/// The body of the connection: initialise, open a session, then serve prompts
/// until the host stops asking.
#[allow(clippy::too_many_arguments)]
async fn run(
    connection: ConnectionTo<Agent>,
    cwd: PathBuf,
    resuming: Option<String>,
    mut prompts: mpsc::Receiver<String>,
    events: mpsc::Sender<AcpEvent>,
    stop: Arc<Notify>,
    // Set while `session/load` replays, so the replay is not recorded twice.
    replaying: std::sync::Arc<std::sync::atomic::AtomicBool>,
    parked: Parked,
    mcp_servers: Vec<McpServer>,
) -> agent_client_protocol::Result<()> {
    // A stop during set-up — an `npx` install, a sign-in prompt, a long
    // replay, a binary that never speaks ACP — ends it here: returning tears
    // the connection, and with it the agent's process group, down.
    let opened = tokio::select! {
        biased;
        _ = stop.notified() => {
            let _ = events.send(AcpEvent::Ended { error: None }).await;
            return Ok(());
        }
        o = open(&connection, cwd, resuming, &events, &replaying, mcp_servers) => o?,
    };
    let Some(Opened {
        session_id,
        agent_name,
        initial_mode,
        can_close,
    }) = opened
    else {
        return Ok(());
    };

    let _ = events
        .send(AcpEvent::Ready {
            // `Display`, not `Debug`, which would wrap it as
            // `SessionId("x")`.
            session_id: session_id.to_string(),
            agent_name,
        })
        .await;

    // After `Ready`, so the run exists to record against.
    if let Some(mode) = initial_mode {
        let _ = events.send(AcpEvent::ModeChanged { mode }).await;
    }

    let mut ended: Option<AcpEvent> = None;
    loop {
        let text = tokio::select! {
            // Biased so a stop wins over a queued prompt.
            biased;
            _ = stop.notified() => break,
            p = prompts.recv() => match p {
                Some(p) => p,
                None => break,
            },
        };

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
                // Pending requests first, then the cancel: an agent blocked
                // on a permission cannot answer `stop_reason: cancelled`.
                parked.cancel_all().await;
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
                ended = Some(AcpEvent::Ended {
                    error: Some(e.to_string()),
                });
                break;
            }
        }
    }

    // Anything still parked is answered before the session goes, so the
    // agent is never left holding a request.
    parked.cancel_all().await;
    // Closed by the protocol where the agent offers it, then torn down as
    // before: a close frees what the agent holds for the session.
    let sent = can_close
        && ended.is_none()
        && matches!(
            tokio::time::timeout(
                CLOSE_GRACE,
                connection
                    .send_request(CloseSessionRequest::new(session_id.clone()))
                    .block_task(),
            )
            .await,
            Ok(Ok(_))
        );
    let _ = events.send(AcpEvent::Closed { sent }).await;
    let _ = events
        .send(ended.unwrap_or(AcpEvent::Ended { error: None }))
        .await;
    Ok(())
}

/// Initialises the connection and opens (or continues) the session. `None`
/// means it could not, and the ending has already been reported.
async fn open(
    connection: &ConnectionTo<Agent>,
    cwd: PathBuf,
    resuming: Option<String>,
    events: &mpsc::Sender<AcpEvent>,
    replaying: &std::sync::atomic::AtomicBool,
    mcp_servers: Vec<McpServer>,
) -> agent_client_protocol::Result<Option<Opened>> {
    // Declaring form elicitation is what lets an agent ask a question: the
    // Claude adapter offers `AskUserQuestion` only to a client with
    // `clientCapabilities.elicitation.form`. No `fs` and no `terminal`: the
    // agent works in its own worktree with its own tools.
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

    // Recorded before anything is asked, so an agent whose session creation
    // fails is still recorded.
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

    // The roster, where the agent has one: asked once, after the handshake.
    // Errors are dropped — a listing is enrichment, not worth ending a session.
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
        // One page: the session in front of us is on it or it is not.
    }

    let can_close = init.agent_capabilities.session_capabilities.close.is_some();
    let agent_name = init.agent_info.as_ref().map(|i| i.name.clone());
    let mut initial_mode: Option<String> = None;
    let session_id = match resuming {
        Some(previous) => {
            // Checked up front, so a missing capability is a refusal rather
            // than an ambiguous error.
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
                            agent_name.as_deref().unwrap_or("this agent")
                        )),
                    })
                    .await;
                return Ok(None);
            }
            // `resume` first where it exists: it does not replay.
            let sid = agent_client_protocol::schema::v1::SessionId::new(previous.clone());
            if !can_resume {
                replaying.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let resumed = if can_resume {
                connection
                    .send_request(
                        ResumeSessionRequest::new(sid, cwd.as_path()).mcp_servers(mcp_servers),
                    )
                    .block_task()
                    .await
                    .map(|_| ())
            } else {
                connection
                    .send_request(
                        agent_client_protocol::schema::v1::LoadSessionRequest::new(
                            sid,
                            cwd.as_path(),
                        )
                        .mcp_servers(mcp_servers),
                    )
                    .block_task()
                    .await
                    .map(|_| ())
            };
            replaying.store(false, std::sync::atomic::Ordering::Relaxed);
            match resumed {
                Ok(()) => agent_client_protocol::schema::v1::SessionId::new(previous),
                Err(e) => {
                    // The session is gone from the agent's side. Say so,
                    // rather than restarting under a resume's name.
                    let _ = events
                        .send(AcpEvent::Ended {
                            error: Some(format!("that session could not be continued: {e}")),
                        })
                        .await;
                    return Ok(None);
                }
            }
        }
        None => {
            match connection
                .send_request(NewSessionRequest::new(cwd).mcp_servers(mcp_servers))
                .block_task()
                .await
            {
                Ok(r) => {
                    // The mode a session starts in: `current_mode_update`
                    // fires only on a change, so the initial mode is reported
                    // here.
                    initial_mode = r.modes.as_ref().map(|m| m.current_mode_id.to_string());
                    r.session_id
                }
                Err(e) => {
                    // An agent that needs signing in advertises `authMethods`,
                    // often with the command to run. The message says *may*:
                    // the methods existing does not make this an auth failure.
                    let hint = describe_auth(&init.auth_methods);
                    let _ = events
                        .send(AcpEvent::Ended {
                            error: Some(match hint {
                                Some(h) => format!("could not start a session: {e}.\n{h}"),
                                None => format!("could not start a session: {e}"),
                            }),
                        })
                        .await;
                    return Ok(None);
                }
            }
        }
    };
    Ok(Some(Opened {
        session_id,
        agent_name,
        initial_mode,
        can_close,
    }))
}

/// The wire name of a stop reason.
/// Spelled out rather than lowercased from `Debug`: the host matches on these.
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
    request_id: &str,
) -> Option<String> {
    let request_id = request_id.to_string();
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

    // A request that carries the proposed edit: kept like any call's diff.
    let diffs = diffs_of(request.tool_call.fields.content.as_deref());
    if !diffs.is_empty() {
        let _ = events
            .send(AcpEvent::Diffs {
                call_id: request.tool_call.tool_call_id.to_string(),
                diffs,
            })
            .await;
    }
    let _ = events
        .send(AcpEvent::PermissionRequested {
            request_id: request_id.clone(),
            call: describe(request),
            options,
        })
        .await;

    // It waits — until a person answers, the project's deadline sweeps it, or
    // the connection goes away.
    let answer = rx.await;
    pending.lock().await.remove(&request_id);

    // An `Err` is the sender dropped: teardown or a deadline sweep, which has
    // already recorded the ending with its own authority. The agent is told no.
    answer.unwrap_or_default()
}

/// Publishes a question and waits — with no timeout — for the person. The
/// wait is off the handler, so the connection keeps reading.
///
/// Cancel, never decline: the adapter folds `decline` into *answered, with no
/// answers*, whereas `cancel` stops the call.
async fn ask_person(
    events: &mpsc::Sender<AcpEvent>,
    asked: &Asked,
    request: &CreateElicitationRequest,
    request_id: &str,
) -> ElicitationAction {
    let Some(parsed) = serde_json::to_value(request)
        .ok()
        .as_ref()
        .and_then(crate::core::question::Ask::parse)
    else {
        // The same method carries MCP-server elicitations in shapes this
        // cannot render. They are cancelled, and reported rather than silently
        // dropped.
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

    let request_id = request_id.to_string();
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
        // The protocol's content type rather than raw JSON, so a shape the
        // schema rejects fails here, where it can still become a cancel.
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
/// as [`stop_reason_name`].
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
        SessionUpdate::ToolCall(t) => {
            let diffs = diffs_of(Some(&t.content));
            let mut out = Vec::new();
            if !diffs.is_empty() {
                out.push(AcpEvent::Diffs {
                    call_id: t.tool_call_id.to_string(),
                    diffs,
                });
            }
            out.push(AcpEvent::Tool {
                call: ToolRequest {
                    id: t.tool_call_id.to_string(),
                    title: t.title.clone(),
                    kind: wire(&t.kind),
                    raw_input: t.raw_input.clone(),
                    locations: t.locations.iter().map(|l| l.path.clone()).collect(),
                },
                status: wire(&t.status),
            });
            out
        }
        // Content replaces the call's whole collection, so the diffs it
        // carries are the call's complete set so far. Reported before the
        // status, so a finishing update's diffs are kept with it.
        SessionUpdate::ToolCallUpdate(t) => {
            let mut out = Vec::new();
            if let Some(content) = t.fields.content.as_deref() {
                let diffs = diffs_of(Some(content));
                if !diffs.is_empty() {
                    out.push(AcpEvent::Diffs {
                        call_id: t.tool_call_id.to_string(),
                        diffs,
                    });
                }
            }
            // No status means content streaming in; reporting it would count
            // one call per chunk and blank the title.
            if t.fields.status.is_some() {
                out.push(tool_update(t));
            }
            out
        }
        SessionUpdate::UsageUpdate(u) => vec![AcpEvent::Usage {
            // Only dollars are carried through: the board sums the figures,
            // and must not mix currencies.
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
        // The user's own message, replayed on resume — already recorded when
        // sent.
        SessionUpdate::UserMessageChunk(_) => Vec::new(),
        // The cross-vendor answer to *which sessions decide without you*.
        SessionUpdate::CurrentModeUpdate(m) => vec![AcpEvent::ModeChanged {
            mode: m.current_mode_id.to_string(),
        }],
        // Command lists are not consumed yet.
        _ => Vec::new(),
    }
}

/// A status-carrying `tool_call_update`, as a [`AcpEvent::Tool`].
fn tool_update(t: agent_client_protocol::schema::v1::ToolCallUpdate) -> AcpEvent {
    AcpEvent::Tool {
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
    }
}

/// The diffs in a tool call's content, in order. Terminal and plain content
/// are not edits and are left out.
fn diffs_of(content: Option<&[ToolCallContent]>) -> Vec<ReportedDiff> {
    content
        .unwrap_or_default()
        .iter()
        .filter_map(|c| match c {
            ToolCallContent::Diff(d) => Some(ReportedDiff {
                path: d.path.clone(),
                old_text: d.old_text.clone(),
                new_text: d.new_text.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// The wire name of a protocol enum, asked of serde rather than reconstructed.
///
/// Never derived from `Debug`: serde is what put it on the wire.
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
        let s = Session::with_channels(
            tx,
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Notify::new()),
        );
        assert!(s.decide("nope", Some("x".into())).await.is_err());
    }

    #[test]
    fn protocol_enums_use_their_wire_names_not_a_debug_string() {
        // Matched on by string downstream, so serde is asked.
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
        let s = Session::with_channels(
            tx,
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(Notify::new()),
        );
        assert!(s.prompt("hello").await.is_err());
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    /// The one line that decides whether an agent can ask a question at all.
    /// The Claude adapter gates on `clientCapabilities.elicitation.form`, so
    /// this asserts the wire shape rather than the builder call.
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

    /// Two tabs answering at once: whoever takes the waiter wins; the loser is
    /// told no question is waiting — a normal outcome. Both answers reaching
    /// the agent is the bug this prevents.
    #[tokio::test]
    async fn an_answer_is_delivered_exactly_once() {
        let (asked, rx) = waiting();
        let s = Session::with_channels(
            mpsc::channel(1).0,
            Arc::new(Mutex::new(HashMap::new())),
            asked.clone(),
            Arc::new(Notify::new()),
        );
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

    /// There is no `decline`: the adapter folds it into *answered, with no
    /// answers*, so a question can only be answered or cancelled.
    #[tokio::test]
    async fn nothing_can_answer_a_question_with_nothing() {
        let (asked, rx) = waiting();
        let s = Session::with_channels(
            mpsc::channel(1).0,
            Arc::new(Mutex::new(HashMap::new())),
            asked,
            Arc::new(Notify::new()),
        );
        // `None` is the cancel path, and the waiter must read it as a cancel
        // rather than as an empty answer.
        s.answer("q1", None).await.expect("cancel is deliverable");
        assert_eq!(rx.await.unwrap(), None, "a cancel is not an empty answer");
    }
}

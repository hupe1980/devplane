//! A driven session: an ACP agent Vibeplane owns.
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
use agent_client_protocol::schema::v1::{
    ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionNotification, SessionUpdate, TextContent,
};
use agent_client_protocol::{AcpAgent, Agent, ConnectionTo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::agent::AgentSpec;

/// How long a permission request waits for an answer before it is refused.
///
/// A session blocked on a question nobody is going to answer is worse than one
/// that gave up: the refusal is visible, recoverable and honest.
const PERMISSION_TIMEOUT: Duration = Duration::from_secs(600);

/// What the daemon can ask a driven session to do.
#[derive(Debug)]
pub enum Command {
    /// Send a prompt and run the turn to completion.
    Prompt(String),
    /// Answer an outstanding permission request.
    Decide {
        request_id: String,
        option_id: Option<String>,
    },
    /// Stop the session and let the process exit.
    Stop,
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
    /// A tool call started or changed.
    Tool {
        title: String,
        kind: Option<String>,
        status: Option<String>,
    },
    /// The agent wants permission. Answer it with [`Session::decide`].
    PermissionRequested {
        request_id: String,
        title: String,
        options: Vec<PermissionOption>,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
    /// `allow_once`, `allow_always`, `reject_once`, `reject_always`.
    pub kind: String,
}

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Option<String>>>>>;

/// A handle on a running agent.
#[derive(Debug, Clone)]
pub struct Session {
    commands: mpsc::Sender<Command>,
    pending: Pending,
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

    pub async fn stop(&self) {
        let _ = self.commands.send(Command::Stop).await;
    }

    /// Whether the session is still accepting commands.
    pub fn is_live(&self) -> bool {
        !self.commands.is_closed()
    }
}

/// Starts an agent and returns a handle plus its event stream.
///
/// The agent process is a child of this one and dies with it. That is the right
/// default for a run Vibeplane owns: an orphaned agent still spending money is
/// the failure nobody notices.
pub async fn spawn(
    spec: &AgentSpec,
    cwd: PathBuf,
) -> anyhow::Result<(Session, mpsc::Receiver<AcpEvent>)> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(32);
    let (ev_tx, ev_rx) = mpsc::channel::<AcpEvent>(1024);
    let pending: Pending = Arc::new(Mutex::new(HashMap::new()));

    let agent = AcpAgent::from_str(&spec.command)
        .map_err(|e| anyhow::anyhow!("cannot start `{}`: {e}", spec.command))?;

    let session = Session {
        commands: cmd_tx,
        pending: pending.clone(),
    };

    let ev_for_notify = ev_tx.clone();
    let ev_for_perm = ev_tx.clone();
    let pending_for_perm = pending.clone();

    tokio::spawn(async move {
        let result = agent_client_protocol::Client
            .builder()
            .name("vibeplane")
            .on_receive_notification(
                async move |notification: SessionNotification, _cx| {
                    for event in map_update(notification.update) {
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
            .connect_with(agent, |connection: ConnectionTo<Agent>| async move {
                run(connection, cwd, cmd_rx, ev_tx.clone()).await
            })
            .await;

        if let Err(e) = result {
            tracing::warn!(error = %e, "acp session ended with an error");
        }
    });

    Ok((session, ev_rx))
}

/// The body of the connection: initialise, open a session, then serve commands
/// until the daemon stops asking.
async fn run(
    connection: ConnectionTo<Agent>,
    cwd: PathBuf,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<AcpEvent>,
) -> agent_client_protocol::Result<()> {
    let init = connection
        .send_request(InitializeRequest::new(ProtocolVersion::V1))
        .block_task()
        .await?;

    let new_session = connection
        .send_request(NewSessionRequest::new(cwd))
        .block_task()
        .await?;
    let session_id = new_session.session_id;

    let _ = events
        .send(AcpEvent::Ready {
            // `Display` renders the protocol value; `Debug` would wrap it in
            // the type name, and an id that round-trips as `SessionId("x")`
            // is an id the agent does not recognise.
            session_id: session_id.to_string(),
            agent_name: init.agent_info.as_ref().map(|i| i.name.clone()),
        })
        .await;

    while let Some(command) = commands.recv().await {
        match command {
            Command::Prompt(text) => {
                let response = connection
                    .send_request(PromptRequest::new(
                        session_id.clone(),
                        vec![ContentBlock::Text(TextContent::new(text))],
                    ))
                    .block_task()
                    .await;
                match response {
                    Ok(r) => {
                        let _ = events
                            .send(AcpEvent::TurnEnded {
                                stop_reason: format!("{:?}", r.stop_reason).to_lowercase(),
                            })
                            .await;
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
            // Answered directly by `Session::decide`; a command arriving here
            // means nothing was waiting for it.
            Command::Decide { .. } => {}
            Command::Stop => break,
        }
    }

    let _ = events.send(AcpEvent::Ended { error: None }).await;
    Ok(())
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
            kind: format!("{:?}", o.kind).to_lowercase(),
        })
        .collect();

    let _ = events
        .send(AcpEvent::PermissionRequested {
            request_id: request_id.clone(),
            title: describe(request),
            options,
        })
        .await;

    let answer = tokio::time::timeout(PERMISSION_TIMEOUT, rx).await;
    pending.lock().await.remove(&request_id);

    match answer {
        Ok(Ok(choice)) => choice,
        // Nobody answered, or the daemon went away. Refusing is the safe end:
        // the agent learns the answer is no and can say so, instead of the
        // session hanging until someone notices.
        _ => None,
    }
}

/// What the agent is asking to do, in the words it used.
fn describe(request: &RequestPermissionRequest) -> String {
    request
        .tool_call
        .fields
        .title
        .clone()
        .unwrap_or_else(|| request.tool_call.tool_call_id.to_string())
        .chars()
        .take(200)
        .collect()
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
            title: t.title.clone(),
            kind: Some(format!("{:?}", t.kind).to_lowercase()),
            status: Some(format!("{:?}", t.status).to_lowercase()),
        }],
        SessionUpdate::ToolCallUpdate(t) => vec![AcpEvent::Tool {
            title: t.fields.title.clone().unwrap_or_default(),
            kind: t.fields.kind.map(|k| format!("{k:?}").to_lowercase()),
            status: t.fields.status.map(|s| format!("{s:?}").to_lowercase()),
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
        // Plans, mode changes and command lists are real, and belong on the
        // Work view rather than the board. Dropping them here is deliberate:
        // an event the product does nothing with is noise in the log.
        _ => Vec::new(),
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

    #[tokio::test]
    async fn a_stopped_session_reports_itself_as_dead() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        let s = Session {
            commands: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
        };
        assert!(s.prompt("hello").await.is_err());
    }
}

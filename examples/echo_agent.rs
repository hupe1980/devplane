//! A fake ACP agent, for testing the client.
//!
//! Vibeplane's job is to drive other people's agents, and every interesting bug
//! lives at that seam. Testing it against a real agent would mean a network, a
//! subscription and a bill on every `cargo test`, so the suite drives this one
//! instead: it speaks the protocol, streams text, calls a tool, and asks for
//! permission — the whole shape of a turn, deterministically and offline.
//!
//! The prompt text selects the behaviour, so one fixture covers every case:
//!
//! | Prompt contains | What the agent does                       |
//! |-----------------|-------------------------------------------|
//! | `permission`    | asks for permission before finishing      |
//! | `tool`          | reports a tool call                       |
//! | `fail`          | ends the turn with a refusal              |
//! | `criticise`     | writes the findings file it was asked to  |
//! | `expensive`     | reports a cumulative cost of $100         |
//!
//! Every prompt it hears is also appended to `.vibeplane/heard.log` in the
//! session's working directory, so a test can assert what an agent was told —
//! which the event log cannot answer, because prompt text is never stored.
//! | anything else   | streams the prompt back and ends the turn |
//!
//! An ACP agent owns stdout for JSON-RPC, so anything diagnostic goes to stderr.

use agent_client_protocol::schema::v1::{
    AgentCapabilities, ContentBlock, ContentChunk, Cost, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    PermissionOption, PermissionOptionId, PermissionOptionKind, PromptRequest, PromptResponse,
    RequestPermissionRequest, ResumeSessionRequest, ResumeSessionResponse, SessionCapabilities,
    SessionId, SessionNotification, SessionResumeCapabilities, SessionUpdate, StopReason,
    TextContent, ToolCall, ToolCallId, ToolCallUpdate, ToolCallUpdateFields, UsageUpdate,
};
use agent_client_protocol::{Agent, Client, Result, Stdio};

/// The working directory the client gave this session.
static CWD: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Where this fixture remembers a session, and how many turns it has heard.
///
/// **On disk, deliberately.** A real resumable agent persists its conversations
/// — that is what lets `session/resume` survive the agent process exiting — and
/// an in-memory map would make the fixture unable to model the only case resume
/// exists for: the daemon restarted, so every agent it had started is gone.
/// The turn count is the cheap fact a test can check: a resumed session
/// continues its own count, a fresh one says `turn 1`.
fn session_file(session: &str) -> Option<std::path::PathBuf> {
    let root = CWD.lock().expect("cwd").clone()?;
    Some(root.join(".vibeplane").join(format!("session-{session}")))
}

fn turns_for(session: &str) -> u32 {
    let Some(path) = session_file(session) else {
        return 1;
    };
    let n = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0)
        + 1;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, n.to_string()).ok();
    n
}

fn knows(session: &str) -> bool {
    session_file(session).is_some_and(|p| p.exists())
}

/// The next unused session name in this working directory.
fn mint_session() -> String {
    for n in 1..10_000 {
        let id = format!("echo-{n}");
        if !knows(&id) {
            return id;
        }
    }
    "echo-overflow".into()
}

fn open_session(session: &str) {
    if let Some(path) = session_file(session) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if !path.exists() {
            std::fs::write(&path, "0").ok();
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    Agent
        .builder()
        .name("echo-agent")
        .on_receive_request(
            async move |init: InitializeRequest, responder, _cx| {
                // **Two ways to continue a session, advertised separately.**
                // `session/resume` continues without replaying; `session/load`
                // continues *with* a replay, and an agent may have either.
                // GitHub Copilot advertises `loadSession` and not `resume`, and
                // a client that checks only for `resume` refuses to continue a
                // conversation that agent is willing to continue.
                //
                // `VIBEPLANE_ECHO_NO_RESUME=1` makes this fixture that agent,
                // so the fallback is exercised rather than described.
                let load_only = std::env::var_os("VIBEPLANE_ECHO_NO_RESUME").is_some();
                // `VIBEPLANE_ECHO_NEEDS_AUTH=1` makes the fixture an agent that
                // has to be signed into, so the client's handling of that can
                // be tested without one that really does.
                let auth = if std::env::var_os("VIBEPLANE_ECHO_NEEDS_AUTH").is_some() {
                    vec![agent_client_protocol::schema::v1::AuthMethod::Agent(
                        agent_client_protocol::schema::v1::AuthMethodAgent::new(
                            agent_client_protocol::schema::v1::AuthMethodId::new("echo-login"),
                            "Log in with Echo",
                        )
                        .description(Some("Run `echo login` in the terminal".to_string())),
                    )]
                } else {
                    vec![]
                };
                let caps = if load_only {
                    AgentCapabilities::new().load_session(true)
                } else {
                    AgentCapabilities::new()
                        .load_session(true)
                        .session_capabilities(
                            SessionCapabilities::new().resume(SessionResumeCapabilities::new()),
                        )
                };
                responder.respond(
                    InitializeResponse::new(init.protocol_version)
                        .agent_capabilities(caps)
                        .auth_methods(auth),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: NewSessionRequest, responder, _cx| {
                // An ACP agent is told its working directory in the protocol,
                // not by being spawned in one — the client's own process cwd is
                // none of its business. Remembering it here is what makes the
                // fixture behave like a real agent.
                *CWD.lock().expect("cwd") = Some(req.cwd.clone());
                // A *new* id every time, so a test can tell a resumed session
                // from one that was quietly started again. With a fixed id the
                // two are indistinguishable, and a test that cannot tell them
                // apart cannot check the thing resume exists for.
                if std::env::var_os("VIBEPLANE_ECHO_NEEDS_AUTH").is_some() {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::invalid_request()
                            .data("authentication required"),
                    );
                }
                let id = mint_session();
                open_session(&id);
                responder.respond(NewSessionResponse::new(SessionId::new(id)))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: ResumeSessionRequest, responder, _cx| {
                // A session this process has never opened cannot be resumed,
                // and saying so is the point: the client must surface that
                // rather than opening a new conversation under the same run.
                // The directory first: it is where this fixture keeps its
                // sessions, so it has to be known before asking whether the
                // session is one of them.
                *CWD.lock().expect("cwd") = Some(req.cwd.clone());
                let id = req.session_id.to_string();
                if !knows(&id) {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::invalid_params()
                            .data(format!("no session {id}")),
                    );
                }
                responder.respond(ResumeSessionResponse::new())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            // `session/load`, the other way to continue. Same bookkeeping as
            // resume; the difference a client cares about is that a real agent
            // replays the conversation here, which is why the client suppresses
            // the replay it already has written down.
            async move |req: LoadSessionRequest, responder, _cx| {
                *CWD.lock().expect("cwd") = Some(req.cwd.clone());
                let id = req.session_id.to_string();
                if !knows(&id) {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::invalid_params()
                            .data(format!("no session {id}")),
                    );
                }
                responder.respond(LoadSessionResponse::new())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: PromptRequest, responder, connection| {
                // The work runs off the handler. An ACP handler that awaits a
                // round trip to its counterpart blocks the pump that would read
                // the reply, which deadlocks the pair — the permission request
                // below is exactly that shape.
                let worker = connection.clone();
                connection.spawn(async move { serve_prompt(req, responder, worker).await })
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_to(Stdio::new())
        .await
}

/// The path a pipeline told this agent to write its findings to.
///
/// The instruction is `write what you found to \`<path>\``, appended by
/// Vibeplane rather than by the project's template, so the fixture can take it
/// literally.
fn findings_path(text: &str) -> Option<String> {
    let rest = text.split_once("write what you found to `")?.1;
    let path = rest.split_once('`')?.0;
    (!path.is_empty()).then(|| path.to_string())
}

async fn serve_prompt(
    req: PromptRequest,
    responder: agent_client_protocol::Responder<PromptResponse>,
    connection: agent_client_protocol::ConnectionTo<Client>,
) -> Result<()> {
    {
        let text = req
            .prompt
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");

        let say = |t: String| {
            SessionNotification::new(
                req.session_id.clone(),
                SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new(t),
                ))),
            )
        };

        // The turn number is what proves a resume continued the conversation
        // rather than starting a new one: a fresh session says `turn 1`.
        let turn = turns_for(&req.session_id.to_string());
        connection.send_notification(say(format!("echo: {text} [turn {turn}]")))?;

        // Vibeplane never stores prompt text — telemetry is redacted and stays
        // that way — so a test cannot ask the event log what an agent was told.
        // This fixture writes it down instead, in the session's own directory,
        // which is the only way to prove that what one step found reaches the
        // step that has to act on it.
        if let Some(root) = CWD.lock().expect("cwd").clone() {
            let log = root.join(".vibeplane/heard.log");
            if let Some(parent) = log.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut prior = std::fs::read_to_string(&log).unwrap_or_default();
            prior.push_str(&text);
            prior.push_str("\n---\n");
            std::fs::write(&log, prior).ok();
        }

        // ACP reports usage as a running total for the session, and the cost
        // field is optional — an agent that never sends one can never be
        // stopped by a budget, which is exactly why the fixture sends one.
        if text.contains("expensive") {
            connection.send_notification(SessionNotification::new(
                req.session_id.clone(),
                SessionUpdate::UsageUpdate(
                    UsageUpdate::new(180_000, 200_000).cost(Cost::new(100.0, "USD".to_string())),
                ),
            ))?;
        }

        if text.contains("tool") {
            connection.send_notification(SessionNotification::new(
                req.session_id.clone(),
                SessionUpdate::ToolCall(ToolCall::new(
                    ToolCallId::new("t1"),
                    "cargo test --workspace".to_string(),
                )),
            ))?;
        }

        // A reviewing step in a pipeline is told, in its prompt, where to
        // write what it found. Playing that role honestly — reading the path
        // out of the instruction rather than having it hard-coded — is what
        // makes the fixture a test of the mechanism and not of itself.
        if text.contains("criticise")
            && let Some(path) = findings_path(&text)
        {
            // Only ever under the directory the client named. A default of
            // "wherever this process happens to be" wrote test litter into the
            // source tree, which is exactly the bug a real agent would have.
            let Some(root) = CWD.lock().expect("cwd").clone() else {
                return Ok(());
            };
            let file = root.join(&path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(&file, "the error path is not covered by a test\n").ok();
            connection.send_notification(say(format!("wrote findings to {}", file.display())))?;
        }

        if text.contains("permission") {
            // The client must answer this before the turn can end,
            // which is exactly the blocking path worth testing.
            let answer = connection
                .send_request(RequestPermissionRequest::new(
                    req.session_id.clone(),
                    ToolCallUpdate::new(
                        ToolCallId::new("t2"),
                        ToolCallUpdateFields::new().title("rm -rf node_modules"),
                    ),
                    vec![
                        PermissionOption::new(
                            PermissionOptionId::new("allow"),
                            "Allow once".to_string(),
                            PermissionOptionKind::AllowOnce,
                        ),
                        PermissionOption::new(
                            PermissionOptionId::new("deny"),
                            "Deny".to_string(),
                            PermissionOptionKind::RejectOnce,
                        ),
                    ],
                ))
                .block_task()
                .await?;
            connection
                .send_notification(say(format!("permission outcome: {:?}", answer.outcome)))?;
        }

        let stop = if text.contains("fail") {
            StopReason::Refusal
        } else {
            StopReason::EndTurn
        };
        responder.respond(PromptResponse::new(stop))
    }
}

//! A fake ACP agent, for testing the client.
//!
//! Devplane's job is to drive other people's agents, and every interesting bug
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
//! | `question`      | asks the **person** a question and waits for the answer |
//! | `question2`     | asks two questions in one form            |
//! | `unrenderable`  | sends an elicitation no client can render as a question |
//! | `tool`          | reports a tool call                       |
//! | `fail`          | ends the turn with a refusal              |
//! | `criticise`     | writes the findings file it was asked to  |
//! | `expensive`     | reports a cumulative cost of $100         |
//! | `slow`          | works until cancelled, then stops properly |
//! | anything else   | streams the prompt back and ends the turn |
//!
//! Every prompt it hears is also appended to `.devplane/heard.log` in the
//! session's working directory, so a test can assert what an agent was told —
//! which the event log cannot answer, because prompt text is never stored.
//!
//! **The question shapes are a capture, not an invention.** A real agent's
//! question does not arrive as a permission request: the Claude adapter renders
//! its `AskUserQuestion` tool as an `elicitation/create` **form**, and only when
//! the client declares `elicitation.form`. The schema below — `question_0` with
//! a `oneOf`, each branch carrying `const`, `title` and `description`, plus a
//! `question_0_custom` free-text field marked with `_askUserQuestionCustomAnswer`
//! — is what `claude-agent-acp@0.76` put on the wire on 2026-09-19, copied
//! field for field. Inventing a plausible shape here would make every test
//! below a statement about this file rather than about the product.
//!
//! **And it only asks when the client says it can render one**, which is the
//! behaviour that cost a day to find: before Devplane declared the capability,
//! the tool was withheld and the agent asked in prose with nobody told.
//!
//! An ACP agent owns stdout for JSON-RPC, so anything diagnostic goes to stderr.

use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, ContentBlock, ContentChunk, Cost, InitializeRequest,
    InitializeResponse, LoadSessionRequest, LoadSessionResponse, NewSessionRequest,
    NewSessionResponse, PermissionOption, PermissionOptionId, PermissionOptionKind, PromptRequest,
    PromptResponse, RequestPermissionRequest, ResumeSessionRequest, ResumeSessionResponse,
    SessionCapabilities, SessionId, SessionMode, SessionModeId, SessionModeState,
    SessionNotification, SessionResumeCapabilities, SessionUpdate, StopReason, TextContent,
    ToolCall, ToolCallId, ToolCallUpdate, ToolCallUpdateFields, UsageUpdate,
};
use agent_client_protocol::{Agent, Client, Result, Stdio};

/// Whether the client said it can render a form elicitation.
///
/// **The whole question path hangs off this.** A real agent withholds its
/// question tool from a client that did not declare `elicitation.form`, and
/// asks in prose instead — which looks, from the outside, exactly like an agent
/// that had nothing to ask.
static CAN_RENDER_FORMS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// An `elicitation/create` in the shape the Claude adapter emits.
fn create_elicitation(
    session: SessionId,
    message: &str,
    schema: serde_json::Value,
) -> agent_client_protocol::schema::v1::CreateElicitationRequest {
    use agent_client_protocol::schema::v1;
    v1::CreateElicitationRequest::new(
        v1::ElicitationFormMode::new(
            v1::ElicitationSessionScope::new(session.to_string()),
            serde_json::from_value(schema).expect("the fixture's own schema parses"),
        ),
        message,
    )
}

/// One question's schema branch, in the shape the Claude adapter emits.
fn one_of(title: &str, options: &[(&str, &str)]) -> serde_json::Value {
    serde_json::json!({
        "type": "string", "title": title,
        "oneOf": options.iter().map(|(v, d)| serde_json::json!(
            { "const": v, "title": v, "description": d })).collect::<Vec<_>>(),
    })
}

/// The working directory the client gave this session.
static CWD: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Sessions the client has asked to stop.
///
/// **`session/cancel` is a notification, not JSON-RPC cancellation**, so the
/// library's `Responder::cancellation` never fires for it and an agent has to
/// track it itself — which is the whole reason this fixture models it. The
/// client sends the notification, waits a grace period for the turn to end with
/// `stop_reason: cancelled`, and tears the connection down if it does not. Until
/// this existed nothing exercised the acknowledging half: every fixture turn
/// finished instantly, so the client's cancel path could only ever be measured
/// by its timeout.
static CANCELLED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn cancel(session: String) {
    CANCELLED.lock().expect("cancelled").push(session);
}

fn is_cancelled(session: &str) -> bool {
    CANCELLED
        .lock()
        .expect("cancelled")
        .iter()
        .any(|s| s == session)
}

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
    Some(root.join(".devplane").join(format!("session-{session}")))
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
                // **Remember whether this client can be asked a question.** A
                // real agent gates its question tool on exactly this, and a
                // client that declares nothing gets prose instead — the failure
                // this fixture exists to be able to reproduce offline.
                // `DEVPLANE_ECHO_NO_FORMS=1` makes the fixture an agent whose
                // question tool is withheld whatever the client declares — the
                // state every agent was in before Devplane declared the
                // capability, and the one a real one cannot be put back into.
                CAN_RENDER_FORMS.store(
                    std::env::var_os("DEVPLANE_ECHO_NO_FORMS").is_none()
                        && init
                            .client_capabilities
                            .elicitation
                            .as_ref()
                            .and_then(|e| e.form.as_ref())
                            .is_some(),
                    std::sync::atomic::Ordering::SeqCst,
                );
                // **Two ways to continue a session, advertised separately.**
                // `session/resume` continues without replaying; `session/load`
                // continues *with* a replay, and an agent may have either.
                // GitHub Copilot advertises `loadSession` and not `resume`, and
                // a client that checks only for `resume` refuses to continue a
                // conversation that agent is willing to continue.
                //
                // `DEVPLANE_ECHO_NO_RESUME=1` makes this fixture that agent,
                // so the fallback is exercised rather than described.
                let load_only = std::env::var_os("DEVPLANE_ECHO_NO_RESUME").is_some();
                // `DEVPLANE_ECHO_NEEDS_AUTH=1` makes the fixture an agent that
                // has to be signed into, so the client's handling of that can
                // be tested without one that really does.
                let auth = if std::env::var_os("DEVPLANE_ECHO_NEEDS_AUTH").is_some() {
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
                if std::env::var_os("DEVPLANE_ECHO_NEEDS_AUTH").is_some() {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::invalid_request()
                            .data("authentication required"),
                    );
                }
                // A *new* id every time, so a test can tell a resumed session
                // from one that was quietly started again. With a fixed id the
                // two are indistinguishable, and a test that cannot tell them
                // apart cannot check the thing resume exists for.
                let id = mint_session();
                open_session(&id);
                // `DEVPLANE_ECHO_MODE=<id>` makes the fixture an agent that
                // declares a session mode, so the cross-vendor half of *which
                // sessions decide without you* can be exercised against
                // something rather than described. The id is arbitrary on
                // purpose: an ACP mode is a string the agent chooses, and a
                // fixture that only ever said `default` would quietly suggest
                // the vendor's vocabulary is the protocol's.
                let resp = NewSessionResponse::new(SessionId::new(id));
                let resp = match std::env::var("DEVPLANE_ECHO_MODE") {
                    Ok(m) if !m.is_empty() => resp.modes(SessionModeState::new(
                        SessionModeId::new(m.clone()),
                        vec![SessionMode::new(SessionModeId::new(m), "Echo's own mode")],
                    )),
                    _ => resp,
                };
                responder.respond(resp)
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
        .on_receive_notification(
            async move |note: CancelNotification, _cx| {
                // Fire-and-forget by design: the client is not waiting on a
                // reply here, it is waiting for the *turn* to end properly.
                cancel(note.session_id.to_string());
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
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
/// Devplane rather than by the project's template, so the fixture can take it
/// literally.
fn findings_path(text: &str) -> Option<String> {
    let rest = text.split_once("write what you found to `")?.1;
    let path = rest.split_once('`')?.0;
    (!path.is_empty()).then(|| path.to_string())
}

/// Answers one `session/prompt`, whatever happens while producing the answer.
///
/// **Every path through this function replies to the client**, and that is the
/// whole reason it is split in two. The protocol library is explicit that
/// dropping a responder for an individual request *"does not automatically send
/// a reply"*, so an early exit leaves the client waiting for a `PromptResponse`
/// that never comes — the same deadlock the handler above spawns a task to
/// avoid, arrived at from the other side.
///
/// The trap is that an early exit does not have to look like one. The turn body
/// sends six notifications and awaits a permission round trip, and **every `?`
/// among them is a `return`**: a failed `send_notification` is harmless because
/// nobody is left to wait, but the permission request can fail while the client
/// is very much alive. So the body hands back a `StopReason` or an error, and
/// the reply happens here, once.
async fn serve_prompt(
    req: PromptRequest,
    responder: agent_client_protocol::Responder<PromptResponse>,
    connection: agent_client_protocol::ConnectionTo<Client>,
) -> Result<()> {
    match take_turn(&req, &connection).await {
        Ok(stop) => responder.respond(PromptResponse::new(stop)),
        Err(e) => responder.respond_with_error(e),
    }
}

/// The turn itself: notifications, an optional permission round trip, and how
/// it ended. Free to use `?`, because its caller always replies.
async fn take_turn(
    req: &PromptRequest,
    connection: &agent_client_protocol::ConnectionTo<Client>,
) -> Result<StopReason> {
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

    // Devplane never stores prompt text — telemetry is redacted and stays
    // that way — so a test cannot ask the event log what an agent was told.
    // This fixture writes it down instead, in the session's own directory,
    // which is the only way to prove that what one step found reaches the
    // step that has to act on it.
    if let Some(root) = CWD.lock().expect("cwd").clone() {
        let log = root.join(".devplane/heard.log");
        if let Some(parent) = log.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        // Appended rather than read-modify-written: two sessions sharing
        // one working directory would otherwise each write back a copy of
        // what they read, and the later write would drop the other's turn.
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
        {
            write!(f, "{text}\n---\n").ok();
        }
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

    // A reviewing step in a pipeline is told, in its prompt, where to write what
    // it found. Playing that role honestly — reading the path out of the
    // instruction rather than having it hard-coded — is what makes the fixture a
    // test of the mechanism and not of itself.
    //
    // Only ever under the directory the client named. A default of "wherever
    // this process happens to be" wrote test litter into the source tree, which
    // is exactly the bug a real agent would have.
    if text.contains("criticise")
        && let Some(path) = findings_path(&text)
        && let Some(root) = CWD.lock().expect("cwd").clone()
    {
        let file = root.join(&path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&file, "the error path is not covered by a test\n").ok();
        connection.send_notification(say(format!("wrote findings to {}", file.display())))?;
    }

    // A turn long enough to be interrupted. Everything else here finishes in
    // microseconds, which makes the client's cancel handshake unobservable: the
    // turn is always over before the notification arrives.
    if text.contains("slow") {
        let id = req.session_id.to_string();
        // Comfortably longer than the client's cancel grace period, which is
        // five seconds: if this ceiling were the shorter of the two, the turn
        // would end on its own while the client was still waiting and the test
        // would pass without the cancel ever being acknowledged. Bounded all
        // the same, so a fixture nobody cancels cannot hang a suite.
        const CEILING: usize = 30 * 100;
        for _ in 0..CEILING {
            if is_cancelled(&id) {
                // The protocol's half of the bargain. A turn that just stops
                // leaves the client waiting out its grace period and then
                // tearing the connection down — which is what "the agent did
                // not acknowledge the cancel" means on the other side.
                return Ok(StopReason::Cancelled);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    if text.contains("permission") {
        // The client must answer this before the turn can end, which is exactly
        // the blocking path worth testing — and the one `?` here that can fail
        // with the client still listening.
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
        connection.send_notification(say(format!("permission outcome: {:?}", answer.outcome)))?;
    }

    if text.contains("question") || text.contains("unrenderable") {
        // Only if the client said it can show one. A client that declared no
        // capability gets prose — the real failure this fixture exists to
        // reproduce, and the reason `asks_nothing_when_the_client_cannot_render`
        // is a test rather than a comment.
        if !CAN_RENDER_FORMS.load(std::sync::atomic::Ordering::SeqCst) {
            connection.send_notification(say(
                "`AskUserQuestion` isn't available in this session — asking in plain text instead."
                    .to_string(),
            ))?;
        } else {
            let schema = if text.contains("unrenderable") {
                // A form with no options: legitimate for an MCP server, and not
                // a question this client can present.
                serde_json::json!({ "type": "object",
                    "properties": { "name": { "type": "string", "title": "Your name" } } })
            } else if text.contains("question2") {
                serde_json::json!({ "type": "object", "properties": {
                    "question_0": one_of("Language", &[("Rust", "Fast."), ("Go", "Simple.")]),
                    "question_1": one_of("Runtime", &[("Tokio", "Big."), ("smol", "Small.")]) } })
            } else {
                serde_json::json!({ "type": "object", "properties": {
                    "question_0": one_of("/v1/login",
                        &[("Keep it", "Retain the legacy route as-is."),
                          ("Drop it", "Remove the legacy route.")]),
                    "question_0_custom": { "type": "string", "title": "Other",
                        "description": "Type your own answer instead of choosing an option above (optional).",
                        "_meta": { "_askUserQuestionCustomAnswer":
                            { "questionId": "question_0", "isCustomAnswer": true } } } } })
            };
            let answer = connection
                .send_request(create_elicitation(
                    req.session_id.clone(),
                    "Should the legacy /v1/login route be kept or dropped?",
                    schema,
                ))
                .block_task()
                .await?;
            connection.send_notification(say(format!("answer: {:?}", answer.action)))?;
        }
    }

    Ok(if text.contains("fail") {
        StopReason::Refusal
    } else {
        StopReason::EndTurn
    })
}

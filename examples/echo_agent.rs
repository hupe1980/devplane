//! A fake ACP agent for testing the client offline and deterministically.
//! Build with `cargo build --example echo_agent`.
//!
//! The prompt text selects the behaviour:
//!
//! | Prompt contains | What the agent does                       |
//! |-----------------|-------------------------------------------|
//! | `permission`    | asks for permission before finishing      |
//! | `question`      | asks the person a question and waits for the answer |
//! | `question2`     | asks two questions in one form            |
//! | `unrenderable`  | sends an elicitation no client can render as a question |
//! | `tool`          | reports a tool call: pending, running, completed |
//! | `plan`          | reports a two-step plan                   |
//! | `fail`          | ends the turn with a refusal              |
//! | `expensive`     | reports a cumulative cost of $100         |
//! | `slow`          | runs until cancelled, then stops properly |
//! | `slowish`       | as `slow`, for three seconds, then ends the turn itself |
//! | `write-file <path>` | writes `<path>` under its directory and reports it as an `Edit` call |
//! | `twin-asks`     | asks two permissions at once, each with its own options |
//! | `withdraw`      | asks a permission, then withdraws it (`$/cancel_request`) |
//! | `diff-edit`     | reports an edit call whose diff arrives whole in a status-less update |
//! | anything else   | streams the prompt back and ends the turn |
//!
//! Every prompt is appended to `.devplane/heard.log` in the session's working
//! directory (prompt text is never stored in the event log), and `DEVPLANE_RUN`
//! is written to `.devplane/run.id`. Opening a session writes the agent's pid
//! to `.devplane/pids/<pid>` and the offered MCP servers to
//! `.devplane/mcp.json`; `session/close` appends to `.devplane/closed.log`.
//! `DEVPLANE_ECHO_PIDS=<dir>` writes the pid there at start, and
//! `DEVPLANE_ECHO_SLOW_INIT=1` answers `initialize` only after 30 seconds.
//!
//! The question schema copies what `claude-agent-acp` sends: an
//! `elicitation/create` form, only when the client declares `elicitation.form`.
//! Stdout is JSON-RPC; diagnostics go to stderr.

use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, CloseSessionRequest, CloseSessionResponse, ContentBlock,
    ContentChunk, Cost, Diff, InitializeRequest, InitializeResponse, LoadSessionRequest,
    LoadSessionResponse, NewSessionRequest, NewSessionResponse, PermissionOption,
    PermissionOptionId, PermissionOptionKind, Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus,
    PromptRequest, PromptResponse, RequestPermissionRequest, ResumeSessionRequest,
    ResumeSessionResponse, SessionCapabilities, SessionCloseCapabilities, SessionId, SessionMode,
    SessionModeId, SessionModeState, SessionNotification, SessionResumeCapabilities, SessionUpdate,
    StopReason, TextContent, ToolCall, ToolCallContent, ToolCallId, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind, UsageUpdate,
};
use agent_client_protocol::{Agent, Client, Result, Stdio};

/// Whether the client declared `elicitation.form`; without it a real agent
/// asks in prose instead.
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

/// Sessions the client has asked to stop. `session/cancel` is a notification,
/// not JSON-RPC cancellation, so the agent must track it itself.
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

/// Where a session's turn count is kept. On disk, so resume survives the agent
/// process exiting, as with a real agent.
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

/// Leaves what a test reads to count this agent's processes and see what
/// it was offered.
fn note_session(mcp: &[agent_client_protocol::schema::v1::McpServer]) {
    let Some(root) = CWD.lock().expect("cwd").clone() else {
        return;
    };
    let pids = root.join(".devplane/pids");
    std::fs::create_dir_all(&pids).ok();
    std::fs::write(pids.join(std::process::id().to_string()), "").ok();
    std::fs::write(
        root.join(".devplane/mcp.json"),
        serde_json::to_string(mcp).unwrap_or_default(),
    )
    .ok();
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
    if let Some(dir) = std::env::var_os("DEVPLANE_ECHO_PIDS") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join(std::process::id().to_string()), "").ok();
    }
    Agent
        .builder()
        .name("echo-agent")
        .on_receive_request(
            async move |init: InitializeRequest, responder, _cx| {
                if std::env::var_os("DEVPLANE_ECHO_SLOW_INIT").is_some() {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
                // `DEVPLANE_ECHO_NO_FORMS=1` withholds the question tool
                // whatever the client declares.
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
                // `DEVPLANE_ECHO_NO_RESUME=1` advertises only `loadSession`
                // (as GitHub Copilot does), exercising the client's fallback.
                let load_only = std::env::var_os("DEVPLANE_ECHO_NO_RESUME").is_some();
                // `DEVPLANE_ECHO_NEEDS_AUTH=1`: an agent that requires sign-in.
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
                            SessionCapabilities::new()
                                .resume(SessionResumeCapabilities::new())
                                .close(SessionCloseCapabilities::new()),
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
                // ACP gives the working directory in the protocol, not the
                // process cwd.
                *CWD.lock().expect("cwd") = Some(req.cwd.clone());
                if std::env::var_os("DEVPLANE_ECHO_NEEDS_AUTH").is_some() {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::invalid_request()
                            .data("authentication required"),
                    );
                }
                // A new id every time, so a resumed session is distinguishable
                // from a restarted one.
                let id = mint_session();
                open_session(&id);
                note_session(&req.mcp_servers);
                // `DEVPLANE_ECHO_MODE=<id>` declares a session mode. The id is
                // arbitrary: an ACP mode is whatever string the agent chooses.
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
                // An unknown session is an error the client must surface. Set
                // the directory first: sessions are stored there.
                *CWD.lock().expect("cwd") = Some(req.cwd.clone());
                let id = req.session_id.to_string();
                if !knows(&id) {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::invalid_params()
                            .data(format!("no session {id}")),
                    );
                }
                note_session(&req.mcp_servers);
                responder.respond(ResumeSessionResponse::new())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            // `session/load`: like resume, but a real agent replays history.
            async move |req: LoadSessionRequest, responder, _cx| {
                *CWD.lock().expect("cwd") = Some(req.cwd.clone());
                let id = req.session_id.to_string();
                if !knows(&id) {
                    return responder.respond_with_error(
                        agent_client_protocol::Error::invalid_params()
                            .data(format!("no session {id}")),
                    );
                }
                note_session(&req.mcp_servers);
                responder.respond(LoadSessionResponse::new())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |req: CloseSessionRequest, responder, _cx| {
                if let Some(root) = CWD.lock().expect("cwd").clone() {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(root.join(".devplane/closed.log"))
                    {
                        writeln!(f, "{}", req.session_id).ok();
                    }
                }
                cancel(req.session_id.to_string());
                responder.respond(CloseSessionResponse::new())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            async move |note: CancelNotification, _cx| {
                // The client waits for the turn to end, not for a reply.
                cancel(note.session_id.to_string());
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |req: PromptRequest, responder, connection| {
                // Off the handler: awaiting a round trip (the permission
                // request) inside it would block the pump and deadlock.
                let worker = connection.clone();
                connection.spawn(async move { serve_prompt(req, responder, worker).await })
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_to(Stdio::new())
        .await
}

/// Answers one `session/prompt` on every path, including errors: a dropped
/// responder sends no reply, so an early `?` would leave the client waiting.
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

    // A fresh session says `turn 1`; a resumed one continues its count.
    let turn = turns_for(&req.session_id.to_string());
    connection.send_notification(say(format!("echo: {text} [turn {turn}]")))?;

    // Prompt text is never stored by Devplane, so record it here for tests.
    if let Some(root) = CWD.lock().expect("cwd").clone() {
        let log = root.join(".devplane/heard.log");
        if let Some(parent) = log.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        // Append, so sessions sharing a directory don't drop each other's turns.
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
        {
            write!(f, "{text}\n---\n").ok();
        }
        // The run id as the agent's shell would see it.
        if let Some(run) = std::env::var_os("DEVPLANE_RUN") {
            std::fs::write(
                root.join(".devplane/run.id"),
                run.to_string_lossy().as_bytes(),
            )
            .ok();
        }
    }

    // ACP usage is a running session total; cost is optional.
    if text.contains("expensive") {
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::UsageUpdate(
                UsageUpdate::new(180_000, 200_000).cost(Cost::new(100.0, "USD".to_string())),
            ),
        ))?;
    }

    if text.contains("tool") {
        // Pending, then updates under the same id without repeating the title.
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::ToolCall(ToolCall::new(
                ToolCallId::new("t1"),
                "cargo test --workspace".to_string(),
            )),
        ))?;
        for status in [ToolCallStatus::InProgress, ToolCallStatus::Completed] {
            connection.send_notification(SessionNotification::new(
                req.session_id.clone(),
                SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                    ToolCallId::new("t1"),
                    ToolCallUpdateFields::new().status(status),
                )),
            ))?;
        }
    }

    if text.contains("plan") {
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::Plan(Plan::new(vec![
                PlanEntry::new(
                    "read the failing test",
                    PlanEntryPriority::High,
                    PlanEntryStatus::Completed,
                ),
                PlanEntry::new(
                    "fix the cause",
                    PlanEntryPriority::High,
                    PlanEntryStatus::InProgress,
                ),
            ])),
        ))?;
    }

    // Writes the file for real and reports an `Edit` call. `write-file`, not
    // `write`, because prompts use that word.
    if let Some(rest) = text.split_once("write-file ").map(|(_, r)| r)
        && let Some(path) = rest.split_whitespace().next()
        && let Some(root) = CWD.lock().expect("cwd").clone()
    {
        let file = root.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&file, "// written by the echo agent\n").ok();
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::ToolCall(
                ToolCall::new(ToolCallId::new("w1"), format!("Edit {path}"))
                    .kind(ToolKind::Edit)
                    .raw_input(serde_json::json!({ "file_path": file.to_string_lossy() })),
            ),
        ))?;
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new("w1"),
                ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
            )),
        ))?;
    }

    // Two calls asking at once, each with its own options.
    if text.contains("twin-asks") {
        let ask = |call: &str, title: &str| {
            connection
                .send_request(RequestPermissionRequest::new(
                    req.session_id.clone(),
                    ToolCallUpdate::new(
                        ToolCallId::new(call.to_string()),
                        ToolCallUpdateFields::new().title(title.to_string()),
                    ),
                    vec![
                        PermissionOption::new(
                            PermissionOptionId::new(format!("allow-{call}")),
                            "Allow once".to_string(),
                            PermissionOptionKind::AllowOnce,
                        ),
                        PermissionOption::new(
                            PermissionOptionId::new(format!("deny-{call}")),
                            "Deny".to_string(),
                            PermissionOptionKind::RejectOnce,
                        ),
                    ],
                ))
                .block_task()
        };
        let (a, b) = tokio::join!(ask("pa", "Read a.txt"), ask("pb", "rm -rf b"));
        connection.send_notification(say(format!("pa outcome: {:?}", a?.outcome)))?;
        connection.send_notification(say(format!("pb outcome: {:?}", b?.outcome)))?;
    }

    // Asks, then thinks better of it: dropping the request sends
    // `$/cancel_request`.
    if text.contains("withdraw") {
        let asked = connection
            .send_request(RequestPermissionRequest::new(
                req.session_id.clone(),
                ToolCallUpdate::new(
                    ToolCallId::new("wd"),
                    ToolCallUpdateFields::new().title("git push --force"),
                ),
                vec![PermissionOption::new(
                    PermissionOptionId::new("allow"),
                    "Allow once".to_string(),
                    PermissionOptionKind::AllowOnce,
                )],
            ))
            .block_task();
        let wait = std::env::var("DEVPLANE_ECHO_WITHDRAW_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1500);
        match tokio::time::timeout(std::time::Duration::from_millis(wait), asked).await {
            Ok(r) => connection.send_notification(say(format!("answered: {:?}", r?.outcome)))?,
            Err(_) => connection.send_notification(say("withdrew the request".into()))?,
        }
    }

    // An edit whose diff arrives in pieces: a partial one with the call, then
    // the whole one in a status-less update, then the ending.
    if text.contains("diff-edit")
        && let Some(root) = CWD.lock().expect("cwd").clone()
    {
        let file = root.join("a.txt");
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::ToolCall(
                ToolCall::new(ToolCallId::new("d1"), "Edit a.txt".to_string())
                    .kind(ToolKind::Edit)
                    .raw_input(serde_json::json!({ "file_path": file.to_string_lossy() }))
                    .content(vec![ToolCallContent::Diff(
                        Diff::new(file.clone(), "tw").old_text("one\n".to_string()),
                    )]),
            ),
        ))?;
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new("d1"),
                ToolCallUpdateFields::new().content(vec![ToolCallContent::Diff(
                    Diff::new(file.clone(), "two\n").old_text("one\n".to_string()),
                )]),
            )),
        ))?;
        connection.send_notification(SessionNotification::new(
            req.session_id.clone(),
            SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new("d1"),
                ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
            )),
        ))?;
    }

    // A turn long enough to be cancelled.
    if text.contains("slow") {
        let id = req.session_id.to_string();
        // 30 s outlasts the client's 5 s cancel grace yet cannot hang a suite;
        // `slowish` ends itself after 3 s, enough for a mid-turn message.
        let ceiling: usize = if text.contains("slowish") {
            3 * 100
        } else {
            30 * 100
        };
        for _ in 0..ceiling {
            if is_cancelled(&id) {
                // Acknowledge the cancel, or the client waits out its grace.
                return Ok(StopReason::Cancelled);
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    if text.contains("permission") {
        // Blocks the turn until the client answers.
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
        // A cancelling client answers `cancelled`; stop and say so.
        if is_cancelled(&req.session_id.to_string()) {
            return Ok(StopReason::Cancelled);
        }
    }

    if text.contains("question") || text.contains("unrenderable") {
        // A client that cannot render forms gets prose, as from a real agent.
        if !CAN_RENDER_FORMS.load(std::sync::atomic::Ordering::SeqCst) {
            connection.send_notification(say(
                "`AskUserQuestion` isn't available in this session — asking in plain text instead."
                    .to_string(),
            ))?;
        } else {
            let schema = if text.contains("unrenderable") {
                // A valid form with no options: not presentable as a question.
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

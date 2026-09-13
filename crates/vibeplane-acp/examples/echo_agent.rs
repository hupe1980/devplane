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
//!
//! Every prompt it hears is also appended to `.vibeplane/heard.log` in the
//! session's working directory, so a test can assert what an agent was told —
//! which the event log cannot answer, because prompt text is never stored.
//! | anything else   | streams the prompt back and ends the turn |
//!
//! An ACP agent owns stdout for JSON-RPC, so anything diagnostic goes to stderr.

use agent_client_protocol::schema::v1::{
    AgentCapabilities, ContentBlock, ContentChunk, InitializeRequest, InitializeResponse,
    NewSessionRequest, NewSessionResponse, PermissionOption, PermissionOptionId,
    PermissionOptionKind, PromptRequest, PromptResponse, RequestPermissionRequest, SessionId,
    SessionNotification, SessionUpdate, StopReason, TextContent, ToolCall, ToolCallId,
    ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol::{Agent, Client, Result, Stdio};

/// The working directory the client gave this session.
static CWD: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

#[tokio::main]
async fn main() -> Result<()> {
    Agent
        .builder()
        .name("echo-agent")
        .on_receive_request(
            async move |init: InitializeRequest, responder, _cx| {
                responder.respond(
                    InitializeResponse::new(init.protocol_version)
                        .agent_capabilities(AgentCapabilities::new()),
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
                responder.respond(NewSessionResponse::new(SessionId::new("echo-1")))
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

        connection.send_notification(say(format!("echo: {text}")))?;

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

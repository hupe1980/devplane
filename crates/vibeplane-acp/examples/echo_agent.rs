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
            async move |_req: NewSessionRequest, responder, _cx| {
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

        if text.contains("tool") {
            connection.send_notification(SessionNotification::new(
                req.session_id.clone(),
                SessionUpdate::ToolCall(ToolCall::new(
                    ToolCallId::new("t1"),
                    "cargo test --workspace".to_string(),
                )),
            ))?;
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

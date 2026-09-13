//! Conformance: the client, driven against a real agent process.
//!
//! The agent is `examples/echo_agent`, which speaks the protocol over stdio the
//! same way `claude-agent-acp` and `opencode acp` do. Using a fixture rather
//! than a vendor's agent is what makes this suite runnable on every commit: no
//! network, no subscription, no bill, and a failure means the client broke
//! rather than someone's API being slow.
//!
//! What the real agents add — and what this cannot prove — is their own
//! behaviour. That is what a pinned version and a separate, opt-in suite
//! against the vendor binaries are for.

use std::path::PathBuf;
use std::time::Duration;
use vibeplane_acp::{AcpEvent, AgentSpec};

/// The fixture agent, built by `cargo test` as a sibling of the test binary.
fn echo_agent() -> AgentSpec {
    let exe = std::env::current_exe().expect("test binary path");
    // target/debug/deps/conformance-<hash> → target/debug/examples/echo_agent
    let dir = exe
        .parent()
        .and_then(|p| p.parent())
        .expect("target/debug")
        .join("examples");
    let bin = dir.join("echo_agent");
    assert!(
        bin.exists(),
        "build the fixture first: cargo build -p vibeplane-acp --examples ({} missing)",
        bin.display()
    );
    AgentSpec::new("echo", "Echo", bin.to_str().unwrap())
}

async fn collect(
    rx: &mut tokio::sync::mpsc::Receiver<AcpEvent>,
    stop: impl Fn(&AcpEvent) -> bool,
) -> Vec<AcpEvent> {
    let mut out = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while let Ok(Some(ev)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        let done = stop(&ev);
        out.push(ev);
        if done {
            break;
        }
    }
    out
}

#[tokio::test]
async fn a_prompt_runs_a_turn_and_streams_the_answer() {
    let (session, mut events) = vibeplane_acp::spawn(&echo_agent(), PathBuf::from("."))
        .await
        .expect("spawning the agent");

    let ready = collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    assert!(
        ready.iter().any(|e| matches!(e, AcpEvent::Ready { .. })),
        "the agent must initialise and open a session: {ready:?}"
    );

    session.prompt("hello there").await.unwrap();
    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;

    assert!(
        turn.iter()
            .any(|e| matches!(e, AcpEvent::Text(t) if t.contains("hello there"))),
        "the answer must reach the client: {turn:?}"
    );
    match turn.last() {
        Some(AcpEvent::TurnEnded { stop_reason }) => assert_eq!(stop_reason, "endturn"),
        other => panic!("the turn must end: {other:?}"),
    }
    session.stop().await;
}

#[tokio::test]
async fn a_tool_call_is_reported() {
    let (session, mut events) = vibeplane_acp::spawn(&echo_agent(), PathBuf::from("."))
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;

    session.prompt("run a tool please").await.unwrap();
    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;

    assert!(
        turn.iter()
            .any(|e| matches!(e, AcpEvent::Tool { title, .. } if title.contains("cargo test"))),
        "tool calls are what the board summarises: {turn:?}"
    );
    session.stop().await;
}

#[tokio::test]
async fn a_permission_request_blocks_until_it_is_answered() {
    // This is the path the whole product turns on: the agent is stopped, the
    // human decides, and the turn continues.
    let (session, mut events) = vibeplane_acp::spawn(&echo_agent(), PathBuf::from("."))
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;

    session.prompt("this needs permission").await.unwrap();
    let until_ask = collect(&mut events, |e| {
        matches!(e, AcpEvent::PermissionRequested { .. })
    })
    .await;

    let (request_id, options) = match until_ask.last() {
        Some(AcpEvent::PermissionRequested {
            request_id,
            options,
            ..
        }) => (request_id.clone(), options.clone()),
        other => panic!("expected a permission request, got {other:?}"),
    };
    assert_eq!(options.len(), 2, "both choices reach the human");
    assert!(options.iter().any(|o| o.kind.contains("allow")));

    // The turn cannot have ended while the agent waits.
    assert!(
        !until_ask
            .iter()
            .any(|e| matches!(e, AcpEvent::TurnEnded { .. })),
        "the turn must still be open while permission is outstanding"
    );

    let allow = options.iter().find(|o| o.kind.contains("allow")).unwrap();
    session
        .decide(&request_id, Some(allow.id.clone()))
        .await
        .expect("answering an outstanding request");

    let rest = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    assert!(
        rest.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. })),
        "answering lets the turn finish: {rest:?}"
    );
    session.stop().await;
}

#[tokio::test]
async fn refusing_a_permission_also_lets_the_turn_finish() {
    let (session, mut events) = vibeplane_acp::spawn(&echo_agent(), PathBuf::from("."))
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("permission please").await.unwrap();

    let asked = collect(&mut events, |e| {
        matches!(e, AcpEvent::PermissionRequested { .. })
    })
    .await;
    let request_id = match asked.last() {
        Some(AcpEvent::PermissionRequested { request_id, .. }) => request_id.clone(),
        other => panic!("{other:?}"),
    };

    // `None` is a refusal, which is also what an unanswered request becomes
    // when its deadline passes. A session must never be left wedged.
    session.decide(&request_id, None).await.unwrap();
    let rest = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    assert!(rest.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. })));
    session.stop().await;
}

#[tokio::test]
async fn a_refusal_is_reported_as_the_stop_reason() {
    let (session, mut events) = vibeplane_acp::spawn(&echo_agent(), PathBuf::from("."))
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("please fail").await.unwrap();

    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    match turn.last() {
        Some(AcpEvent::TurnEnded { stop_reason }) => assert_eq!(stop_reason, "refusal"),
        other => panic!("{other:?}"),
    }
    session.stop().await;
}

#[tokio::test]
async fn several_turns_run_on_one_session() {
    // A driven run is a conversation, not a one-shot: the session has to
    // survive a completed turn and take the next prompt.
    let (session, mut events) = vibeplane_acp::spawn(&echo_agent(), PathBuf::from("."))
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;

    for n in 1..=3 {
        session.prompt(format!("turn {n}")).await.unwrap();
        let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
        assert!(
            turn.iter()
                .any(|e| matches!(e, AcpEvent::Text(t) if t.contains(&format!("turn {n}")))),
            "turn {n} did not answer: {turn:?}"
        );
    }
    session.stop().await;
}

#[tokio::test]
async fn a_missing_agent_fails_loudly_rather_than_hanging() {
    let spec = AgentSpec::new("nope", "nope", "/definitely/not/an/agent --acp");
    match vibeplane_acp::spawn(&spec, PathBuf::from(".")).await {
        Err(e) => assert!(!e.to_string().is_empty()),
        Ok((session, mut events)) => {
            // Some transports only fail once the process is reaped, so an
            // immediate error is not required — ending promptly is.
            let seen = tokio::time::timeout(
                Duration::from_secs(10),
                collect(&mut events, |e| matches!(e, AcpEvent::Ended { .. })),
            )
            .await;
            assert!(
                seen.is_ok(),
                "a session that cannot start must end, not hang"
            );
            let _ = session;
        }
    }
}

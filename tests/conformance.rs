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

mod common;

use std::path::PathBuf;
use std::time::Duration;
use vibeplane::acp::{AcpEvent, AgentSpec};

/// The repository root, which is also the crate root.
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The fixture agent, built by `cargo test` as a sibling of the test binary.
/// A working directory per test, so the fixture's on-disk session files land
/// in a temporary place rather than in the repository the tests run from —
/// which is where 847 of them had accumulated.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-conformance-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

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
        "build the fixture first: cargo build --examples ({} missing)",
        bin.display()
    );

    // **And that it is the fixture in this working tree.** `cargo test` does
    // not rebuild examples, so editing the agent and running the suite tests
    // the *previous* agent — silently, and in the direction that passes. This
    // was not hypothetical: a test written to exercise `session/load` went
    // green against a fixture that did not implement it yet, and only failed
    // once the example was rebuilt by hand.
    let src = repo_root().join("examples/echo_agent.rs");
    if let (Ok(b), Ok(s)) = (
        std::fs::metadata(&bin).and_then(|m| m.modified()),
        std::fs::metadata(&src).and_then(|m| m.modified()),
    ) && s > b
    {
        panic!(
            "the fixture agent is older than its source: run `cargo build --examples`.\n\
             Testing a stale agent passes for the wrong reason, which is worse than failing."
        );
    }
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
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = vibeplane::acp::spawn(&echo_agent(), scratch())
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
        Some(AcpEvent::TurnEnded { stop_reason }) => assert_eq!(stop_reason, "end_turn"),
        other => panic!("the turn must end: {other:?}"),
    }
    session.stop();
}

#[tokio::test]
async fn a_tool_call_is_reported() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = vibeplane::acp::spawn(&echo_agent(), scratch())
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;

    session.prompt("run a tool please").await.unwrap();
    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;

    assert!(
        turn.iter()
            .any(|e| matches!(e, AcpEvent::Tool { call, .. } if call.title.contains("cargo test"))),
        "tool calls are what the board summarises: {turn:?}"
    );
    session.stop();
}

#[tokio::test]
async fn a_permission_request_blocks_until_it_is_answered() {
    let _serial = common::one_agent_at_a_time();
    // This is the path the whole product turns on: the agent is stopped, the
    // human decides, and the turn continues.
    let (session, mut events) = vibeplane::acp::spawn(&echo_agent(), scratch())
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
    session.stop();
}

#[tokio::test]
async fn refusing_a_permission_also_lets_the_turn_finish() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = vibeplane::acp::spawn(&echo_agent(), scratch())
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
    session.stop();
}

#[tokio::test]
async fn a_refusal_is_reported_as_the_stop_reason() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = vibeplane::acp::spawn(&echo_agent(), scratch())
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("please fail").await.unwrap();

    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    match turn.last() {
        Some(AcpEvent::TurnEnded { stop_reason }) => assert_eq!(stop_reason, "refusal"),
        other => panic!("{other:?}"),
    }
    session.stop();
}

#[tokio::test]
async fn several_turns_run_on_one_session() {
    let _serial = common::one_agent_at_a_time();
    // A driven run is a conversation, not a one-shot: the session has to
    // survive a completed turn and take the next prompt.
    let (session, mut events) = vibeplane::acp::spawn(&echo_agent(), scratch())
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
    session.stop();
}

/// An agent that needs signing in says so, in its own words.
///
/// The agent already answers `initialize` with `authMethods` — a name, and
/// often the literal command to run; Copilot's says *"Run `copilot login` in
/// the terminal"*. Handing back the raw protocol error instead makes a first
/// run look like a broken integration when it is a login, which is the worst
/// five minutes a new user can have.
#[tokio::test]
async fn an_agent_that_needs_a_login_says_how() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    spec.command = format!("env VIBEPLANE_ECHO_NEEDS_AUTH=1 {}", spec.command);

    let (session, mut rx) = vibeplane::acp::spawn(&spec, std::env::temp_dir())
        .await
        .expect("the fixture starts");
    let events = collect(&mut rx, |e| {
        matches!(e, AcpEvent::Ready { .. } | AcpEvent::Ended { .. })
    })
    .await;
    match events.last() {
        Some(AcpEvent::Ended { error: Some(e) }) => {
            assert!(
                e.contains("signing in"),
                "the failure should name the cause: {e}"
            );
            assert!(
                e.contains("Run `echo login` in the terminal"),
                "and repeat the agent's own instruction: {e}"
            );
        }
        other => panic!("expected a session that could not start, got {other:?}"),
    }
    session.stop();
}

/// An agent that offers `session/load` and not `session/resume` can still be
/// continued.
///
/// The protocol has two ways of carrying a conversation across a restart and
/// they are advertised separately: `session/resume` continues without replaying
/// history, `session/load` continues with it. This client checked only for
/// `resume`, so an agent with the other one was told its conversation "cannot
/// be continued" — a refusal, on the feature whose whole promise is that a
/// restart does not lose the work.
///
/// Not hypothetical: GitHub Copilot's ACP server advertises exactly this
/// combination, which is how it was found. The fixture is asked to imitate it.
#[tokio::test]
async fn a_session_can_be_continued_by_load_where_resume_is_not_offered() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    // The fixture reads this and drops `resume` from what it advertises.
    spec.command = format!("env VIBEPLANE_ECHO_NO_RESUME=1 {}", spec.command);
    let cwd = std::env::temp_dir();

    let (session, mut rx) = vibeplane::acp::spawn(&spec, cwd.clone())
        .await
        .expect("the fixture starts");
    let ready = collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
    let id = ready
        .iter()
        .find_map(|e| match e {
            AcpEvent::Ready { session_id, .. } => Some(session_id.clone()),
            _ => None,
        })
        .expect("the agent names its session");
    session.stop();

    // Same id, a second connection: continued rather than started again.
    let (again, mut rx2) = vibeplane::acp::resume(&spec, cwd, id.clone())
        .await
        .expect("the fixture starts again");
    let ready2 = collect(&mut rx2, |e| {
        matches!(e, AcpEvent::Ready { .. } | AcpEvent::Ended { .. })
    })
    .await;
    match ready2.last() {
        Some(AcpEvent::Ready { session_id, .. }) => assert_eq!(
            session_id, &id,
            "a continued session keeps its id; a new one would have a new one"
        ),
        other => panic!("expected the session to be continued, got {other:?}"),
    }
    again.stop();
}

#[tokio::test]
async fn a_missing_agent_fails_loudly_rather_than_hanging() {
    let _serial = common::one_agent_at_a_time();
    let spec = AgentSpec::new("nope", "nope", "/definitely/not/an/agent --acp");
    match vibeplane::acp::spawn(&spec, scratch()).await {
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

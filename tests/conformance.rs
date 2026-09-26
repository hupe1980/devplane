//! Conformance: the client driven against a real agent process,
//! `examples/echo_agent`, which speaks ACP over stdio like the vendor agents.
//! Offline and free, so a failure means the client broke.

mod common;

use devplane::acp::{AcpEvent, AgentSpec};
use std::path::PathBuf;
use std::time::Duration;

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A working directory per test, so the fixture's session files stay out of
/// the repository.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-conformance-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// The fixture agent, next to the test binary, refused if older than its source.
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

    // `cargo test` does not rebuild examples, so a stale agent would silently
    // test the previous behaviour.
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;

    session.prompt("run a tool please").await.unwrap();
    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;

    // Three updates on one id, in order; only the first carries the title.
    let seen: Vec<(String, Option<String>, String)> = turn
        .iter()
        .filter_map(|e| match e {
            AcpEvent::Tool { call, status } => {
                Some((call.id.clone(), status.clone(), call.title.clone()))
            }
            _ => None,
        })
        .collect();
    let statuses: Vec<Option<&str>> = seen.iter().map(|(_, s, _)| s.as_deref()).collect();
    assert_eq!(
        statuses,
        [Some("pending"), Some("in_progress"), Some("completed")],
        "{turn:?}"
    );
    assert!(seen.iter().all(|(id, _, _)| id == "t1"), "{seen:?}");
    assert!(seen[0].2.contains("cargo test"), "{seen:?}");
    session.stop();
}

/// Stopping a run blocked on a permission answers the request with
/// `cancelled` first, so the agent can acknowledge the cancel instead of
/// waiting out the grace period into a kill.
#[tokio::test]
async fn a_stop_answers_a_pending_permission_before_cancelling_the_turn() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("this needs permission").await.unwrap();
    collect(&mut events, |e| {
        matches!(e, AcpEvent::PermissionRequested { .. })
    })
    .await;

    let asked_at = std::time::Instant::now();
    session.stop();
    let rest = collect(&mut events, |e| {
        matches!(e, AcpEvent::TurnEnded { .. } | AcpEvent::Ended { .. })
    })
    .await;
    assert!(
        matches!(
            rest.last(),
            Some(AcpEvent::TurnEnded { stop_reason }) if stop_reason == "cancelled"
        ),
        "the agent never got to acknowledge the cancel: {rest:?}"
    );
    assert!(
        rest.iter()
            .any(|e| matches!(e, AcpEvent::Text(t) if t.contains("Cancelled"))),
        "the pending request was not answered with `cancelled`: {rest:?}"
    );
    assert!(
        asked_at.elapsed() < Duration::from_secs(4),
        "the stop waited out the grace period rather than being acknowledged"
    );
}

#[tokio::test]
async fn a_permission_request_blocks_until_it_is_answered() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
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

    // `None` is a refusal, as for an unanswered request past its deadline.
    session.decide(&request_id, None).await.unwrap();
    let rest = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    assert!(rest.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. })));
    session.stop();
}

#[tokio::test]
async fn a_refusal_is_reported_as_the_stop_reason() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
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

/// An agent that needs sign-in fails with a message naming the cause and
/// repeating the agent's own `authMethods` instruction.
#[tokio::test]
async fn an_agent_that_needs_a_login_says_how() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_NEEDS_AUTH=1 {}", spec.command);

    let (session, mut rx) = devplane::acp::spawn(&spec, std::env::temp_dir(), &[], None)
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

/// An agent that offers `session/load` but not `session/resume` (as GitHub
/// Copilot does) can still be continued, keeping its session id.
#[tokio::test]
async fn a_session_can_be_continued_by_load_where_resume_is_not_offered() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_NO_RESUME=1 {}", spec.command);
    let cwd = std::env::temp_dir();

    let (session, mut rx) = devplane::acp::spawn(&spec, cwd.clone(), &[], None)
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
    let (again, mut rx2) = devplane::acp::resume(&spec, cwd, id.clone(), &[], None)
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
    match devplane::acp::spawn(&spec, scratch(), &[], None).await {
        Err(e) => assert!(!e.to_string().is_empty()),
        Ok((session, mut events)) => {
            // Some transports fail only once the process is reaped.
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

/// A cancelled turn ends with the agent acknowledging it
/// (`stop_reason: cancelled`), not with the client's timeout.
#[tokio::test]
async fn a_cancelled_turn_is_acknowledged_rather_than_timed_out() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    // `slow` runs until cancelled.
    session.prompt("please work slow").await.unwrap();

    // Long enough for the turn to be in flight, well short of its own ceiling.
    tokio::time::sleep(Duration::from_millis(300)).await;
    session.stop();

    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    match turn
        .iter()
        .rev()
        .find(|e| matches!(e, AcpEvent::TurnEnded { .. }))
    {
        Some(AcpEvent::TurnEnded { stop_reason }) => assert_eq!(
            stop_reason, "cancelled",
            "the agent acknowledged the cancel, so this is not a timeout"
        ),
        other => panic!("{other:?}"),
    }
}

// ── Questions ───────────────────────────────────────────────────────────────
// A question arrives as an `elicitation/create` form, only to a client that
// declared `elicitation.form`; the fixture's shapes copy `claude-agent-acp`.

/// A question is held open until answered, and the agent receives the answer.
#[tokio::test]
async fn a_question_reaches_a_person_and_the_answer_reaches_the_agent() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;

    session.prompt("ask me a question").await.unwrap();
    let until_ask = collect(&mut events, |e| matches!(e, AcpEvent::QuestionAsked { .. })).await;

    let (request_id, ask) = match until_ask.last() {
        Some(AcpEvent::QuestionAsked { request_id, ask }) => (request_id.clone(), ask.clone()),
        other => panic!("expected a question, got {other:?}"),
    };

    // Each option carries its reason, and there is a free-text field.
    assert_eq!(ask.questions.len(), 1);
    let q = &ask.questions[0];
    assert_eq!(q.options.len(), 2);
    assert_eq!(
        q.options[0].detail.as_deref(),
        Some("Retain the legacy route as-is."),
        "an option's own reason is what makes it a choice rather than a label"
    );
    assert_eq!(q.custom_field.as_deref(), Some("question_0_custom"));

    // The turn cannot have ended while the agent waits.
    assert!(
        !until_ask
            .iter()
            .any(|e| matches!(e, AcpEvent::TurnEnded { .. })),
        "the turn must still be open while a question is outstanding"
    );

    let content = ask.content(&[(
        q.field.clone(),
        devplane::core::question::Chosen::Option("Drop it".into()),
    )]);
    session
        .answer(&request_id, Some(content))
        .await
        .expect("answering an outstanding question");

    let rest = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    assert!(
        rest.iter()
            .any(|e| matches!(e, AcpEvent::Text(t) if t.contains("Accept"))),
        "the agent must see the answer it was waiting for: {rest:?}"
    );
    session.stop();
}

/// A typed answer is sent under the custom field, not the selection.
#[tokio::test]
async fn a_typed_answer_reaches_the_agent_under_the_custom_field() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("ask me a question").await.unwrap();
    let until_ask = collect(&mut events, |e| matches!(e, AcpEvent::QuestionAsked { .. })).await;
    let (request_id, ask) = match until_ask.last() {
        Some(AcpEvent::QuestionAsked { request_id, ask }) => (request_id.clone(), ask.clone()),
        other => panic!("expected a question, got {other:?}"),
    };

    let content = ask.content(&[(
        ask.questions[0].field.clone(),
        devplane::core::question::Chosen::Custom("neither — keep it behind a flag".into()),
    )]);
    assert_eq!(
        content["question_0_custom"], "neither — keep it behind a flag",
        "a typed answer goes back under the custom field, not the selection"
    );
    session.answer(&request_id, Some(content)).await.unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    session.stop();
}

/// Several questions in one form keep their order and their own fields.
#[tokio::test]
async fn a_form_with_two_questions_keeps_them_in_order() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("question2 please").await.unwrap();
    let until_ask = collect(&mut events, |e| matches!(e, AcpEvent::QuestionAsked { .. })).await;
    let (request_id, ask) = match until_ask.last() {
        Some(AcpEvent::QuestionAsked { request_id, ask }) => (request_id.clone(), ask.clone()),
        other => panic!("expected a question, got {other:?}"),
    };
    let fields: Vec<&str> = ask.questions.iter().map(|q| q.field.as_str()).collect();
    assert_eq!(
        fields,
        ["question_0", "question_1"],
        "the order the agent asked in is the order they are answered in"
    );
    let content = ask.content(&[
        (
            "question_0".into(),
            devplane::core::question::Chosen::Option("Rust".into()),
        ),
        (
            "question_1".into(),
            devplane::core::question::Chosen::Option("smol".into()),
        ),
    ]);
    assert_eq!(content["question_0"], "Rust");
    assert_eq!(content["question_1"], "smol");
    session.answer(&request_id, Some(content)).await.unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    session.stop();
}

/// An elicitation that is not a question with options (e.g. an MCP form) is
/// reported as unrenderable, not raised as a question or dropped silently.
#[tokio::test]
async fn an_elicitation_that_cannot_be_rendered_is_reported_not_swallowed() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("send an unrenderable form").await.unwrap();
    let seen = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;

    assert!(
        seen.iter()
            .any(|e| matches!(e, AcpEvent::QuestionUnrenderable { .. })),
        "a form this client cannot show has to be reported: {seen:?}"
    );
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AcpEvent::QuestionAsked { .. })),
        "and it must not be raised as a question with no options"
    );
    session.stop();
}

/// An unanswered question ends as cancelled, never as an empty answer (the
/// adapter would treat a decline as "answered with nothing").
#[tokio::test]
async fn a_question_nobody_answers_is_cancelled_rather_than_answered_with_nothing() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("ask me a question").await.unwrap();
    let until_ask = collect(&mut events, |e| matches!(e, AcpEvent::QuestionAsked { .. })).await;
    let request_id = match until_ask.last() {
        Some(AcpEvent::QuestionAsked { request_id, .. }) => request_id.clone(),
        other => panic!("expected a question, got {other:?}"),
    };

    // `None` cancels; there is no way to express "answered with nothing".
    session
        .answer(&request_id, None)
        .await
        .expect("cancelling an outstanding question");

    let rest = collect(&mut events, |e| {
        matches!(e, AcpEvent::TurnEnded { .. } | AcpEvent::Ended { .. })
    })
    .await;
    assert!(
        rest.iter()
            .any(|e| matches!(e, AcpEvent::QuestionCancelled { .. })),
        "a question that got no answer has to say so, or the row keeps offering \
         an answer nothing can deliver: {rest:?}"
    );
    assert!(
        rest.iter()
            .any(|e| matches!(e, AcpEvent::Text(t) if t.contains("Cancel"))),
        "and the agent must be told it was cancelled, not handed an empty answer: {rest:?}"
    );
    session.stop();
}

/// Two answers to one question on a live session produce one delivery and one
/// ordinary refusal (e.g. two open tabs).
#[tokio::test]
async fn two_answers_to_one_question_deliver_once() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("ask me a question").await.unwrap();
    let until_ask = collect(&mut events, |e| matches!(e, AcpEvent::QuestionAsked { .. })).await;
    let (request_id, ask) = match until_ask.last() {
        Some(AcpEvent::QuestionAsked { request_id, ask }) => (request_id.clone(), ask.clone()),
        other => panic!("expected a question, got {other:?}"),
    };

    let pick = |v: &str| {
        ask.content(&[(
            ask.questions[0].field.clone(),
            devplane::core::question::Chosen::Option(v.into()),
        )])
    };
    let first = session.answer(&request_id, Some(pick("Drop it"))).await;
    let second = session.answer(&request_id, Some(pick("Keep it"))).await;

    assert!(first.is_ok(), "the first answer is delivered");
    assert!(
        second.is_err(),
        "the second finds nothing waiting — which is the guard against an agent \
         receiving two answers to one question"
    );

    let rest = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    let accepted = rest
        .iter()
        .filter(|e| matches!(e, AcpEvent::Text(t) if t.contains("Accept")))
        .count();
    assert_eq!(
        accepted, 1,
        "exactly one answer reached the agent: {rest:?}"
    );
    session.stop();
}

/// Without form support the agent asks in prose and no question is raised;
/// pins why the client must declare `elicitation.form`.
#[tokio::test]
async fn an_agent_asks_in_prose_when_the_client_cannot_render_a_question() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_NO_FORMS=1 {}", spec.command);

    let (session, mut events) = devplane::acp::spawn(&spec, scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("ask me a question").await.unwrap();
    let seen = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;

    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, AcpEvent::QuestionAsked { .. })),
        "no question can be raised when the agent was never able to ask one"
    );
    assert!(
        seen.iter()
            .any(|e| matches!(e, AcpEvent::Text(t) if t.contains("isn't available"))),
        "the agent falls back to prose, and that is the failure worth seeing: {seen:?}"
    );
    session.stop();
}

/// An agent's initial ACP session mode is reported without any mode change,
/// verbatim as the agent's own string, never mapped onto a vendor's modes.
#[tokio::test]
async fn an_agent_declares_its_own_session_mode_and_it_is_not_a_vendor_s() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_MODE=echo-supervised {}", spec.command);
    let cwd = std::env::temp_dir();

    let (session, mut rx) = devplane::acp::spawn(&spec, cwd, &[], None)
        .await
        .expect("the fixture starts");
    let seen = collect(&mut rx, |e| matches!(e, AcpEvent::ModeChanged { .. })).await;
    session.stop();

    let mode = seen
        .iter()
        .find_map(|e| match e {
            AcpEvent::ModeChanged { mode } => Some(mode.clone()),
            _ => None,
        })
        .expect("an agent that declares a mode is heard, without changing it");
    assert_eq!(
        mode, "echo-supervised",
        "the agent's own spelling reaches the client unaltered"
    );
    for vendor in [
        "default",
        "acceptEdits",
        "plan",
        "bypassPermissions",
        "auto",
    ] {
        assert_ne!(
            mode, vendor,
            "an ACP mode was translated into a vendor's mode; no specification supports that"
        );
    }
}

/// Capabilities are read from each agent's `initialize` handshake, and two
/// differently-configured agents produce different records.
#[tokio::test]
async fn what_an_agent_supports_is_read_from_its_own_handshake() {
    let _serial = common::one_agent_at_a_time();

    let caps = |extra: &str| {
        let mut spec = echo_agent();
        if !extra.is_empty() {
            spec.command = format!("env {} {}", extra, spec.command);
        }
        spec
    };

    // The default fixture: both ways to continue a session.
    let (session, mut rx) = devplane::acp::spawn(&caps(""), std::env::temp_dir(), &[], None)
        .await
        .expect("the fixture starts");
    let seen = collect(&mut rx, |e| matches!(e, AcpEvent::Capabilities { .. })).await;
    session.stop();
    let full = seen
        .iter()
        .find_map(|e| match e {
            AcpEvent::Capabilities {
                resume,
                load_session,
                needs_auth,
                ..
            } => Some((*resume, *load_session, *needs_auth)),
            _ => None,
        })
        .expect("capabilities are reported once, from the handshake");
    assert_eq!(
        full,
        (true, true, false),
        "this fixture advertises resume and load, and needs no sign-in"
    );

    // Load-only and needing sign-in.
    let (session, mut rx) = devplane::acp::spawn(
        &caps("DEVPLANE_ECHO_NO_RESUME=1 DEVPLANE_ECHO_NEEDS_AUTH=1"),
        std::env::temp_dir(),
        &[],
        None,
    )
    .await
    .expect("the fixture starts again");
    let seen = collect(&mut rx, |e| matches!(e, AcpEvent::Capabilities { .. })).await;
    session.stop();
    let narrow = seen
        .iter()
        .find_map(|e| match e {
            AcpEvent::Capabilities {
                resume,
                load_session,
                needs_auth,
                ..
            } => Some((*resume, *load_session, *needs_auth)),
            _ => None,
        })
        .expect("capabilities are reported for this shape too");
    assert_eq!(
        narrow,
        (false, true, true),
        "an agent with load-only and a sign-in must not read like one with everything"
    );
    assert_ne!(full, narrow, "two different agents produced one record");
}

/// Whether a process is still running.
fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks for the process.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// The pids the fixture wrote into `dir`, once there is at least one.
async fn pids_in(dir: &std::path::Path) -> Vec<i32> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let pids: Vec<i32> = std::fs::read_dir(dir)
            .map(|d| {
                d.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        if !pids.is_empty() || tokio::time::Instant::now() >= deadline {
            return pids;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Every event until the session's channel closes.
async fn drain(rx: &mut tokio::sync::mpsc::Receiver<AcpEvent>) -> Vec<AcpEvent> {
    let mut out = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while let Ok(Some(ev)) = tokio::time::timeout_at(deadline, rx.recv()).await {
        out.push(ev);
    }
    out
}

/// A stop while the agent is still in `initialize` ends the connection and
/// the process, rather than waiting for a handshake that may never come.
#[tokio::test]
async fn a_stop_during_setup_tears_the_agent_down() {
    let _serial = common::one_agent_at_a_time();
    let pids = scratch().join("pids");
    let (session, mut rx) = devplane::acp::spawn(
        &echo_agent(),
        scratch(),
        &[
            ("DEVPLANE_ECHO_PIDS".into(), pids.clone()),
            ("DEVPLANE_ECHO_SLOW_INIT".into(), PathBuf::from("1")),
        ],
        None,
    )
    .await
    .expect("spawning the agent");
    let pid = *pids_in(&pids).await.first().expect("the agent started");
    session.stop();
    let started = std::time::Instant::now();
    let seen = drain(&mut rx).await;
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the stop waited for the handshake"
    );
    assert!(
        seen.iter()
            .any(|e| matches!(e, AcpEvent::Ended { error: None })),
        "{seen:?}"
    );
    assert!(
        !seen.iter().any(|e| matches!(e, AcpEvent::Ready { .. })),
        "{seen:?}"
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while alive(pid) && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!alive(pid), "the agent outlived its stopped session");
}

/// An agent that offers `session/close` is told the session is closed before
/// the connection goes.
#[tokio::test]
async fn a_stop_closes_the_session_when_the_agent_offers_it() {
    let _serial = common::one_agent_at_a_time();
    let dir = scratch();
    let (session, mut rx) = devplane::acp::spawn(&echo_agent(), dir.clone(), &[], None)
        .await
        .unwrap();
    let ready = collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
    let id = ready
        .iter()
        .find_map(|e| match e {
            AcpEvent::Ready { session_id, .. } => Some(session_id.clone()),
            _ => None,
        })
        .expect("ready");
    session.stop();
    let seen = drain(&mut rx).await;
    assert!(
        seen.iter()
            .any(|e| matches!(e, AcpEvent::Closed { sent: true })),
        "{seen:?}"
    );
    let closed = std::fs::read_to_string(dir.join(".devplane/closed.log")).unwrap_or_default();
    assert_eq!(closed.trim(), id, "the agent never heard `session/close`");
    let ended = seen
        .iter()
        .filter(|e| matches!(e, AcpEvent::Ended { .. }))
        .count();
    assert_eq!(ended, 1, "the ending is reported once: {seen:?}");
}

/// An agent withdrawing its own permission request (`$/cancel_request`) is
/// reported as that, and nothing is left waiting on an answer.
#[tokio::test]
async fn a_withdrawn_permission_is_reported_and_forgotten() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut rx) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("withdraw it").await.unwrap();
    let turn = collect(&mut rx, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    let asked = turn
        .iter()
        .find_map(|e| match e {
            AcpEvent::PermissionRequested { request_id, .. } => Some(request_id.clone()),
            _ => None,
        })
        .expect("the agent asked");
    assert!(
        turn.iter().any(
            |e| matches!(e, AcpEvent::PermissionWithdrawn { request_id } if *request_id == asked)
        ),
        "{turn:?}"
    );
    assert!(
        turn.iter()
            .any(|e| matches!(e, AcpEvent::Text(t) if t.contains("withdrew the request"))),
        "{turn:?}"
    );
    assert!(session.waiting().await.is_empty(), "still parked");
    assert!(session.decide(&asked, None).await.is_err());
    session.stop();
}

/// A diff in a tool call's content is kept, and the whole one wins over the
/// fragment that came first.
#[tokio::test]
async fn a_tool_calls_diff_arrives_whole_before_the_call_ends() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut rx) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("diff-edit please").await.unwrap();
    let turn = collect(&mut rx, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    let diffs: Vec<(usize, &devplane::acp::ReportedDiff)> = turn
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            AcpEvent::Diffs { call_id, diffs } if call_id == "d1" => Some((i, &diffs[0])),
            _ => None,
        })
        .collect();
    assert_eq!(diffs.len(), 2, "{turn:?}");
    let (at, last) = diffs[1];
    assert_eq!(last.new_text, "two\n");
    assert_eq!(last.old_text.as_deref(), Some("one\n"));
    let completed = turn
        .iter()
        .position(|e| {
            matches!(e, AcpEvent::Tool { call, status } if call.id == "d1"
                && status.as_deref() == Some("completed"))
        })
        .expect("the call completed");
    assert!(at < completed, "the diff came after the ending");
    session.stop();
}

/// Devplane's MCP server is offered on `session/new` and again on
/// `session/resume`, as a stdio server with its own environment.
#[tokio::test]
async fn devplanes_mcp_server_is_offered_on_new_and_on_resume() {
    let _serial = common::one_agent_at_a_time();
    let dir = scratch();
    let offer = devplane::acp::McpOffer {
        name: "devplane".into(),
        command: PathBuf::from("/usr/local/bin/devplane"),
        args: vec!["mcp".into()],
        env: vec![("DEVPLANE_CHANGE".into(), "chg-1".into())],
    };
    let offered = |dir: &std::path::Path| -> serde_json::Value {
        serde_json::from_str(
            &std::fs::read_to_string(dir.join(".devplane/mcp.json")).unwrap_or_default(),
        )
        .unwrap_or_default()
    };

    let (session, mut rx) =
        devplane::acp::spawn(&echo_agent(), dir.clone(), &[], Some(offer.clone()))
            .await
            .unwrap();
    let ready = collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
    let id = ready
        .iter()
        .find_map(|e| match e {
            AcpEvent::Ready { session_id, .. } => Some(session_id.clone()),
            _ => None,
        })
        .unwrap();
    let v = offered(&dir);
    assert_eq!(v[0]["name"], "devplane", "{v}");
    assert_eq!(v[0]["command"], "/usr/local/bin/devplane", "{v}");
    assert_eq!(v[0]["args"], serde_json::json!(["mcp"]), "{v}");
    assert_eq!(v[0]["env"][0]["name"], "DEVPLANE_CHANGE", "{v}");
    session.stop();
    drain(&mut rx).await;

    std::fs::remove_file(dir.join(".devplane/mcp.json")).ok();
    let (again, mut rx) = devplane::acp::resume(&echo_agent(), dir.clone(), id, &[], Some(offer))
        .await
        .unwrap();
    collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
    assert_eq!(offered(&dir)[0]["args"], serde_json::json!(["mcp"]));
    again.stop();
}

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

use devplane::acp::{AcpEvent, AgentSpec};
use std::path::PathBuf;
use std::time::Duration;

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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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
    spec.command = format!("env DEVPLANE_ECHO_NEEDS_AUTH=1 {}", spec.command);

    let (session, mut rx) = devplane::acp::spawn(&spec, std::env::temp_dir())
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
    spec.command = format!("env DEVPLANE_ECHO_NO_RESUME=1 {}", spec.command);
    let cwd = std::env::temp_dir();

    let (session, mut rx) = devplane::acp::spawn(&spec, cwd.clone())
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
    let (again, mut rx2) = devplane::acp::resume(&spec, cwd, id.clone())
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
    match devplane::acp::spawn(&spec, scratch()).await {
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

/// A cancelled turn ends with the agent saying so, not with a timeout.
///
/// **The half of `session/cancel` nothing measured.** The client sends the
/// notification, waits a grace period for the turn to end with
/// `stop_reason: cancelled`, and tears the connection down if it does not — and
/// until the fixture could be interrupted, every turn finished in microseconds,
/// so only the *timeout* branch was ever reachable. A handshake whose success
/// path is untested is a handshake that can rot into its failure path silently.
#[tokio::test]
async fn a_cancelled_turn_is_acknowledged_rather_than_timed_out() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    // `slow` keeps the fixture working until it is told to stop.
    session.prompt("please work slow").await.unwrap();

    // Long enough that the turn is certainly in flight, short enough that the
    // fixture's own six-second ceiling cannot be what ends it.
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
//
// A question is not a permission and does not arrive like one. It comes over
// `elicitation/create` as a form, and only to a client that declared it can
// render one — which is the behaviour that cost a day to find, because an agent
// with no way to ask a *structured* question asks in prose and stops, and a run
// sitting on an unanswered question looks exactly like a run that finished.
//
// These run against the fixture, so every one of them is free and offline. The
// shapes the fixture sends were captured from `claude-agent-acp@0.76` on
// 2026-09-19 rather than invented, which is what stops them being a test of the
// fixture's imagination.

/// The round trip: asked, held, answered, and the agent continues.
#[tokio::test]
async fn a_question_reaches_a_person_and_the_answer_reaches_the_agent() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;

    session.prompt("ask me a question").await.unwrap();
    let until_ask = collect(&mut events, |e| matches!(e, AcpEvent::QuestionAsked { .. })).await;

    let (request_id, ask) = match until_ask.last() {
        Some(AcpEvent::QuestionAsked { request_id, ask }) => (request_id.clone(), ask.clone()),
        other => panic!("expected a question, got {other:?}"),
    };

    // Everything the person needs, including the two things a hook-shaped model
    // had no room for: each option's own reason, and a box for an answer the
    // agent did not offer.
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

/// A person's own words, for a question whose options did not fit.
#[tokio::test]
async fn a_typed_answer_reaches_the_agent_under_the_custom_field() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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

/// Several questions in one form, each answerable under its own field.
#[tokio::test]
async fn a_form_with_two_questions_keeps_them_in_order() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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

/// An elicitation this client cannot present is cancelled **and said out loud**.
///
/// The same protocol method carries forms from MCP servers, in shapes that are
/// not questions with options. Guessing at one would put words in somebody's
/// mouth; cancelling it silently would be the failure this whole feature is
/// about, committed inside it.
#[tokio::test]
async fn an_elicitation_that_cannot_be_rendered_is_reported_not_swallowed() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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

/// A question that nobody answers ends as **cancelled**, never as an empty answer.
///
/// The distinction is the feature. The adapter folds a *decline* into
/// "answered, with no answers" — the agent proceeds having asked and heard
/// nothing, which is exactly what this exists to prevent — so the only refusal
/// Devplane may send is a cancel. This drives the whole path rather than
/// asserting it over source: the question is raised, the run is stopped under
/// it, and the event that comes back says which of the two happened.
#[tokio::test]
async fn a_question_nobody_answers_is_cancelled_rather_than_answered_with_nothing() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("ask me a question").await.unwrap();
    let until_ask = collect(&mut events, |e| matches!(e, AcpEvent::QuestionAsked { .. })).await;
    let request_id = match until_ask.last() {
        Some(AcpEvent::QuestionAsked { request_id, .. }) => request_id.clone(),
        other => panic!("expected a question, got {other:?}"),
    };

    // Nobody answers; the run goes away under it. `None` is the cancel path —
    // there is deliberately no way to express "answered with nothing".
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

/// An answer is delivered exactly once, driven end to end.
///
/// The unit test proves the waiter map hands out one answer. This proves the
/// property that matters: two answers against a **live** session produce one
/// delivery and one refusal, and the refusal is an ordinary outcome rather than
/// a failure — two tabs, or a phone and a laptop, is the expected case.
#[tokio::test]
async fn two_answers_to_one_question_deliver_once() {
    let _serial = common::one_agent_at_a_time();
    let (session, mut events) = devplane::acp::spawn(&echo_agent(), scratch())
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

/// An agent asking a client that cannot render a form gets **prose**, and the
/// run goes quiet with nothing raised.
///
/// This is the shape of the defect that cost a day: before Devplane declared
/// `elicitation.form`, the tool was withheld, the agent asked in plain text, the
/// run went idle and the inbox said *"Nothing needs you"* about a session
/// waiting on a person. It is pinned here so the capability cannot be dropped
/// without something failing — and so the case can be exercised at all, which a
/// live agent cannot be made to do on demand.
#[tokio::test]
async fn an_agent_asks_in_prose_when_the_client_cannot_render_a_question() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    // The fixture mirrors the real gate: no declared capability, no question.
    spec.command = format!("env DEVPLANE_ECHO_NO_FORMS=1 {}", spec.command);

    let (session, mut events) = devplane::acp::spawn(&spec, scratch()).await.unwrap();
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

/// **The cross-vendor half of *which sessions decide without you*.**
///
/// `devplane modes` answers that question by reading one vendor's settings
/// files. Any agent that speaks the protocol can answer it directly — ACP
/// carries a session mode — and the update was being dropped at the protocol
/// boundary under a comment saying *"nothing consumes them yet"*.
///
/// Two properties, and the second is the one the design turns on.
///
/// **A mode is reported even when it never changes.** `current_mode_update`
/// fires on a *change*, so an agent that starts in a mode and stays there would
/// never be heard from — which is exactly the session worth knowing about. The
/// initial mode comes from the session response instead.
///
/// **And it is the agent's own string.** There is no cross-vendor vocabulary for
/// an ACP mode and nothing maps it onto a vendor's documented four, so the
/// fixture declares something deliberately unlike any of them: a client that
/// quietly normalised it into `default` would pass a test written with a
/// familiar word and fail here.
#[tokio::test]
async fn an_agent_declares_its_own_session_mode_and_it_is_not_a_vendor_s() {
    let _serial = common::one_agent_at_a_time();
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_MODE=echo-supervised {}", spec.command);
    let cwd = std::env::temp_dir();

    let (session, mut rx) = devplane::acp::spawn(&spec, cwd)
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
    // The negative half, and the reason the fixture's mode is spelled oddly:
    // nothing may have mapped it onto a vendor's vocabulary on the way.
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

/// **What an agent supports is measured, not listed.**
///
/// Every session capability is advertised per agent at `initialize`, so it is a
/// runtime fact about that agent at that version — never something this product
/// can assert from a registry entry. The notes have carried *across vendors* as
/// a design property for passes; this is the thing that makes it checkable, and
/// it is allowed to come back disappointing.
///
/// The fixture is asked to be two different agents, because a record that cannot
/// tell them apart is a record of nothing.
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
    let (session, mut rx) = devplane::acp::spawn(&caps(""), std::env::temp_dir())
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

    // The same fixture asked to be GitHub Copilot's shape: `load` and no
    // `resume`. A record that reported the two identically would be worthless
    // for the one question it exists to answer.
    let (session, mut rx) = devplane::acp::spawn(
        &caps("DEVPLANE_ECHO_NO_RESUME=1 DEVPLANE_ECHO_NEEDS_AUTH=1"),
        std::env::temp_dir(),
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

//! Conformance: the client driven against a real agent process,
//! `examples/echo_agent`, which speaks ACP over stdio like the vendor agents.
//! Offline and free, so a failure means the client broke.

mod common;

use devplane::acp::{AcpEvent, AgentSpec};
use std::path::PathBuf;
use std::time::Duration;

/// A working directory per test, so the fixture's session files stay out of
/// the repository.
fn scratch() -> PathBuf {
    common::scratch("conformance")
}

/// The fixture agent; this suite is about it, so its absence fails rather than skips.
fn echo_agent() -> AgentSpec {
    common::echo_spec().expect("build the fixture first: cargo build --examples")
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
            AcpEvent::Tool { report } => Some((
                report.id.clone(),
                report.status.clone(),
                report.title.clone().unwrap_or_default(),
            )),
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

/// An agent that needs signing in is shown by its own sign-in methods, by
/// the agent's names and kinds, and the session ends saying why; the client
/// declared it can run a terminal login, so the agent offered one.
#[tokio::test]
async fn sign_in_methods_are_the_agents_own() {
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_NEEDS_AUTH=1 {}", spec.command);

    let (session, mut rx) = devplane::acp::spawn(&spec, scratch(), &[], None)
        .await
        .expect("the fixture starts");
    let events = collect(&mut rx, |e| {
        matches!(e, AcpEvent::Ready { .. } | AcpEvent::Ended { .. })
    })
    .await;
    session.stop();
    let methods = events
        .iter()
        .find_map(|e| match e {
            AcpEvent::SignInRequired { methods } => Some(methods.clone()),
            _ => None,
        })
        .expect("the agent's sign-in methods are reported");
    let shown: Vec<(&str, &str, &str)> = methods
        .iter()
        .map(|m| (m.id.as_str(), m.name.as_str(), m.kind.as_str()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("echo-login", "Log in with Echo", "agent"),
            ("echo-terminal", "Echo's own login", "terminal"),
        ],
        "by the agent's own names and kinds; terminal offered because the client declared it"
    );
    assert_eq!(methods[1].args, vec!["--login".to_string()]);
    match events.last() {
        Some(AcpEvent::Ended { error: Some(e) }) => {
            assert!(e.contains("signing in"), "the ending names the cause: {e}")
        }
        other => panic!("expected a session that could not start, got {other:?}"),
    }

    // The person chose the agent-handled method: `authenticate` first, then
    // the session starts.
    let dir = scratch();
    let (session, mut rx) = devplane::acp::session::spawn_with(
        &spec,
        dir.clone(),
        &[],
        None,
        devplane::acp::session::Launch {
            authenticate: Some("echo-login".into()),
            ..Default::default()
        },
    )
    .await
    .expect("the fixture starts");
    let events = collect(&mut rx, |e| {
        matches!(e, AcpEvent::Ready { .. } | AcpEvent::Ended { .. })
    })
    .await;
    session.stop();
    assert!(
        matches!(events.last(), Some(AcpEvent::Ready { .. })),
        "signed in, the session starts: {events:?}"
    );
    let heard = std::fs::read_to_string(dir.join(".devplane/heard.log")).unwrap_or_default();
    assert!(heard.contains("authenticate echo-login"), "{heard}");
}

/// Every page of the agent's session list is read, up to the limit, and a
/// list longer than that says more sessions exist than were read.
#[tokio::test]
async fn lists_every_page() {
    for (pages, want, truncated) in [
        (3, 3, false),
        (25, devplane::acp::session::LIST_PAGES, true),
    ] {
        let mut spec = echo_agent();
        spec.command = format!("env DEVPLANE_ECHO_PAGES={pages} {}", spec.command);
        let (session, mut rx) = devplane::acp::spawn(&spec, scratch(), &[], None)
            .await
            .expect("the fixture starts");
        let events = collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
        session.stop();
        let listed = events
            .iter()
            .find_map(|e| match e {
                AcpEvent::SessionsListed {
                    sessions,
                    truncated,
                } => Some((sessions.len(), *truncated)),
                _ => None,
            })
            .expect("the list was read");
        assert_eq!(listed, (want, truncated), "{pages} pages");
    }
}

/// An agent answering another protocol version is not driven: the run ends
/// naming both versions, and no session is created.
#[tokio::test]
async fn another_protocol_version_is_refused() {
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_VERSION=2 {}", spec.command);
    let (session, mut rx) = devplane::acp::spawn(&spec, scratch(), &[], None)
        .await
        .expect("the fixture starts");
    let events = collect(&mut rx, |e| {
        matches!(e, AcpEvent::Ready { .. } | AcpEvent::Ended { .. })
    })
    .await;
    session.stop();
    match events.last() {
        Some(AcpEvent::Ended { error: Some(e) }) => assert!(
            e.contains("protocol version 2") && e.contains("version 1"),
            "{e}"
        ),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A folder outside the working directory is declared to an agent that takes
/// additional directories; an agent that does not is never prompted.
#[tokio::test]
async fn declares_the_spec_folder() {
    let outside = scratch();
    let launch = || devplane::acp::session::Launch {
        additional_directories: vec![outside.clone()],
        ..Default::default()
    };

    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_ADDDIRS=1 {}", spec.command);
    let dir = scratch();
    let (session, mut rx) =
        devplane::acp::session::spawn_with(&spec, dir.clone(), &[], None, launch())
            .await
            .expect("the fixture starts");
    let events = collect(&mut rx, |e| {
        matches!(e, AcpEvent::Ready { .. } | AcpEvent::Ended { .. })
    })
    .await;
    session.stop();
    assert!(
        matches!(events.last(), Some(AcpEvent::Ready { .. })),
        "{events:?}"
    );
    let declared: Vec<PathBuf> = serde_json::from_str(
        &std::fs::read_to_string(dir.join(".devplane/dirs.json")).unwrap_or_default(),
    )
    .unwrap_or_default();
    assert_eq!(declared, vec![outside.clone()]);

    // Without the capability: ended before any prompt, saying why.
    let dir = scratch();
    let (session, mut rx) =
        devplane::acp::session::spawn_with(&echo_agent(), dir.clone(), &[], None, launch())
            .await
            .expect("the fixture starts");
    session.prompt("hello").await.ok();
    let events = collect(&mut rx, |e| {
        matches!(e, AcpEvent::Ready { .. } | AcpEvent::Ended { .. })
    })
    .await;
    session.stop();
    match events.last() {
        Some(AcpEvent::Ended { error: Some(e) }) => {
            assert!(e.contains("cannot be told about"), "{e}")
        }
        other => panic!("expected the session to end, got {other:?}"),
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !dir.join(".devplane/heard.log").exists(),
        "no prompt reached an agent that cannot see its specification"
    );
}

/// A notice the agent addresses to the person arrives as an event, in its
/// own words.
#[tokio::test]
async fn a_notice_is_reported_not_dropped() {
    let (session, mut rx) = devplane::acp::spawn(&echo_agent(), scratch(), &[], None)
        .await
        .expect("the fixture starts");
    session.prompt("notice").await.unwrap();
    let events = collect(&mut rx, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    session.stop();
    assert!(
        events.iter().any(|e| matches!(e,
            AcpEvent::Notice { severity, title, description }
                if severity == "warning"
                    && title == "Rate limit close"
                    && description.as_deref() == Some("80% of this hour's requests are used"))),
        "{events:?}"
    );
}

/// A stop closes the session over the protocol where the agent offers it,
/// and says it did not where it does not.
#[tokio::test]
async fn closes_before_ending() {
    for (extra, want) in [("", true), ("DEVPLANE_ECHO_NO_CLOSE=1", false)] {
        let mut spec = echo_agent();
        if !extra.is_empty() {
            spec.command = format!("env {extra} {}", spec.command);
        }
        let dir = scratch();
        let (session, mut rx) = devplane::acp::spawn(&spec, dir.clone(), &[], None)
            .await
            .expect("the fixture starts");
        collect(&mut rx, |e| matches!(e, AcpEvent::Ready { .. })).await;
        session.stop();
        let events = collect(&mut rx, |e| matches!(e, AcpEvent::Ended { .. })).await;
        let sent = events.iter().find_map(|e| match e {
            AcpEvent::Closed { sent } => Some(*sent),
            _ => None,
        });
        assert_eq!(sent, Some(want), "{extra}: {events:?}");
        assert_eq!(dir.join(".devplane/closed.log").exists(), want, "{extra}");
    }
}

/// An agent that offers `session/load` but not `session/resume` (as GitHub
/// Copilot does) can still be continued, keeping its session id.
#[tokio::test]
async fn a_session_can_be_continued_by_load_where_resume_is_not_offered() {
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
            matches!(e, AcpEvent::Tool { report } if report.id == "d1"
                && report.status.as_deref() == Some("completed"))
        })
        .expect("the call completed");
    assert!(at < completed, "the diff came after the ending");
    session.stop();
}

/// Devplane's MCP server is offered on `session/new` and again on
/// `session/resume`, as a stdio server with its own environment.
#[tokio::test]
async fn devplanes_mcp_server_is_offered_on_new_and_on_resume() {
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

// ── Sparse tool-call reports ─────────────────────────────────────────────────

/// An update that carries only the call's input is forwarded, and nothing the
/// agent did not send is filled in: no `""` title, no status.
#[tokio::test]
async fn a_sparse_update_is_forwarded_unfilled() {
    let mut spec = echo_agent();
    spec.command = format!("env DEVPLANE_ECHO_SPARSE=1 {}", spec.command);
    let (session, mut events) = devplane::acp::spawn(&spec, scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::Ready { .. })).await;
    session.prompt("run a tool please").await.unwrap();
    let until_ask = collect(&mut events, |e| {
        matches!(e, AcpEvent::PermissionRequested { .. })
    })
    .await;

    let reports: Vec<&devplane::acp::calls::ToolReport> = until_ask
        .iter()
        .filter_map(|e| match e {
            AcpEvent::Tool { report } => Some(report),
            _ => None,
        })
        .collect();
    assert_eq!(reports.len(), 2, "{until_ask:?}");
    assert_eq!(reports[0].title.as_deref(), Some("Run a command"));
    assert_eq!(reports[0].kind.as_deref(), Some("execute"));
    let later = reports[1];
    assert_eq!(
        later.raw_input,
        Some(serde_json::json!({"command": "rm -rf build"})),
        "the status-less update was dropped"
    );
    assert_eq!(
        (
            later.title.as_deref(),
            later.kind.as_deref(),
            later.status.as_deref()
        ),
        (None, None, None)
    );
    let (request_id, report) = match until_ask.last() {
        Some(AcpEvent::PermissionRequested {
            request_id, report, ..
        }) => (request_id.clone(), report.clone()),
        other => panic!("expected a permission request, got {other:?}"),
    };
    assert_eq!(
        report,
        devplane::acp::calls::ToolReport {
            id: "s1".into(),
            ..Default::default()
        },
        "nothing filled in"
    );
    session
        .decide(&request_id, Some("deny".into()))
        .await
        .unwrap();
    collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    session.stop();
}

// ── Modes ────────────────────────────────────────────────────────────────────

const CODEX_MODES: &str = "mode:read-only=Read-only,workspace-write=Workspace access,agent=Auto review,agent-full-access=Full access";

/// The fixture presenting codex-acp 2.1.1's recorded identity and mode option,
/// so the shipped profile applies — no test-only switch in the client.
fn codex_like(extra: &str) -> AgentSpec {
    let mut spec = echo_agent();
    spec.command = format!(
        "env DEVPLANE_ECHO_AGENT_INFO=@agentclientprotocol/codex-acp@2.1.1 \
         'DEVPLANE_ECHO_CONFIG_MODE={CODEX_MODES}' {extra} {}",
        spec.command
    );
    spec
}

/// The set-up's mode report, and what the agent was told: its heard.log after
/// one prompt.
async fn set_up(
    spec: &AgentSpec,
) -> (
    Option<devplane::acp::modes::Advertised>,
    devplane::acp::modes::ModePlan,
    Option<devplane::acp::modes::Outcome>,
    String,
    String,
) {
    let dir = scratch();
    let (session, mut events) = devplane::acp::spawn(spec, dir.clone(), &[], None)
        .await
        .unwrap();
    let opened = collect(&mut events, |e| {
        matches!(e, AcpEvent::ModeAdvertised { .. })
    })
    .await;
    let (advertised, plan, outcome) = match opened.last() {
        Some(AcpEvent::ModeAdvertised {
            advertised,
            plan,
            outcome,
            ..
        }) => (advertised.clone(), plan.clone(), outcome.clone()),
        other => panic!("no mode report after set-up: {other:?}"),
    };
    let session_id = opened
        .iter()
        .find_map(|e| match e {
            AcpEvent::Ready { session_id, .. } => Some(session_id.clone()),
            _ => None,
        })
        .expect("ready before the mode report");
    session.prompt("hello mode").await.unwrap();
    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    assert!(
        turn.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. })),
        "the first prompt never went: {turn:?}"
    );
    session.stop();
    let heard = std::fs::read_to_string(dir.join(".devplane/heard.log")).unwrap_or_default();
    (advertised, plan, outcome, heard, session_id)
}

fn set_lines(heard: &str) -> Vec<&str> {
    heard.lines().filter(|l| l.starts_with("set_")).collect()
}

#[tokio::test]
async fn mode_unconfigured_start_is_asked_for_ask_first_before_the_prompt() {
    let (_, plan, outcome, heard, _) = set_up(&codex_like("DEVPLANE_ECHO_START=agent")).await;
    assert_eq!(plan.request.as_deref(), Some("read-only"));
    assert_eq!(plan.why, devplane::core::run::Why::AskedForAskFirst);
    assert_eq!(
        outcome,
        Some(devplane::acp::modes::Outcome::Confirmed("read-only".into()))
    );
    assert_eq!(
        set_lines(&heard),
        ["set_config_option mode=read-only"],
        "{heard}"
    );
    let set = heard.find("set_config_option").unwrap();
    let prompt = heard.find("hello mode").expect("prompted");
    assert!(
        set < prompt,
        "the prompt went before the mode was settled: {heard}"
    );
}

#[tokio::test]
async fn mode_config_option_is_used_over_modes() {
    // The fixture advertises the same values both ways.
    let (advertised, _, _, heard, _) = set_up(&codex_like("DEVPLANE_ECHO_START=agent")).await;
    assert_eq!(
        advertised.map(|a| a.via),
        Some(devplane::acp::modes::Via::Config("mode".into()))
    );
    assert!(!heard.contains("set_mode"), "{heard}");
}

#[tokio::test]
async fn mode_only_modes_uses_set_mode() {
    let mut spec = echo_agent();
    spec.command = format!(
        "env DEVPLANE_ECHO_AGENT_INFO=gemini-cli@0.62.0 \
         'DEVPLANE_ECHO_CONFIG_MODE=mode:default=Default,autoEdit=Auto Edit,yolo=YOLO' \
         DEVPLANE_ECHO_MODES_ONLY=1 {}",
        spec.command
    );
    spec.mode = Some("autoEdit".into());
    let (advertised, plan, outcome, heard, _) = set_up(&spec).await;
    assert_eq!(
        advertised.map(|a| a.via),
        Some(devplane::acp::modes::Via::Modes)
    );
    assert_eq!(plan.why, devplane::core::run::Why::YouChose);
    assert_eq!(
        outcome,
        Some(devplane::acp::modes::Outcome::Confirmed("autoEdit".into()))
    );
    assert_eq!(set_lines(&heard), ["set_mode autoEdit"], "{heard}");
}

#[tokio::test]
async fn mode_refusal_is_reported_and_the_prompt_still_goes() {
    let (_, _, outcome, heard, _) = set_up(&codex_like(
        "DEVPLANE_ECHO_START=agent DEVPLANE_ECHO_REFUSE_SET=1",
    ))
    .await;
    match outcome {
        Some(devplane::acp::modes::Outcome::Refused(reason)) => {
            assert!(
                reason.contains("locked"),
                "the agent's own message: {reason}"
            )
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(heard.contains("hello mode"), "{heard}");
}

/// The only test that waits the real ten seconds: the grace is not shortened
/// through a hidden switch.
#[tokio::test]
async fn mode_no_answer_after_the_grace() {
    let (_, _, outcome, heard, _) = set_up(&codex_like(
        "DEVPLANE_ECHO_START=agent DEVPLANE_ECHO_SLOW_SET=1",
    ))
    .await;
    assert_eq!(outcome, Some(devplane::acp::modes::Outcome::NoAnswer));
    assert!(
        heard.contains("hello mode"),
        "the prompt waited forever: {heard}"
    );
}

#[tokio::test]
async fn mode_already_ask_first_sends_nothing() {
    let (_, plan, outcome, heard, _) = set_up(&codex_like("DEVPLANE_ECHO_START=read-only")).await;
    assert_eq!(plan.why, devplane::core::run::Why::AsksAlready);
    assert_eq!(outcome, None);
    assert!(set_lines(&heard).is_empty(), "{heard}");
}

#[tokio::test]
async fn mode_none_advertised_sends_nothing() {
    let (advertised, plan, _, heard, _) = set_up(&echo_agent()).await;
    assert_eq!(advertised, None);
    assert_eq!(plan.why, devplane::core::run::Why::NoneStated);
    assert!(set_lines(&heard).is_empty(), "{heard}");
}

#[tokio::test]
async fn mode_ask_first_not_advertised_sends_nothing() {
    let mut spec = echo_agent();
    spec.command = format!(
        "env DEVPLANE_ECHO_AGENT_INFO=@agentclientprotocol/codex-acp@2.1.1 \
         'DEVPLANE_ECHO_CONFIG_MODE=mode:agent=Auto review,workspace-write=Workspace access' \
         DEVPLANE_ECHO_START=agent {}",
        spec.command
    );
    let (_, plan, _, heard, _) = set_up(&spec).await;
    assert_eq!(plan.why, devplane::core::run::Why::NoAskFirst);
    assert!(set_lines(&heard).is_empty(), "{heard}");
}

#[tokio::test]
async fn mode_on_resume_asks_again_when_back_at_the_start() {
    let dir = scratch();
    let spec = codex_like("DEVPLANE_ECHO_START=agent DEVPLANE_ECHO_RESUME_START=agent");
    let (session, mut events) = devplane::acp::spawn(&spec, dir.clone(), &[], None)
        .await
        .unwrap();
    let opened = collect(&mut events, |e| {
        matches!(e, AcpEvent::ModeAdvertised { .. })
    })
    .await;
    let id = opened
        .iter()
        .find_map(|e| match e {
            AcpEvent::Ready { session_id, .. } => Some(session_id.clone()),
            _ => None,
        })
        .unwrap();
    session.stop();
    collect(&mut events, |e| matches!(e, AcpEvent::Ended { .. })).await;

    let (again, mut events) = devplane::acp::resume(&spec, dir.clone(), id, &[], None)
        .await
        .unwrap();
    let reopened = collect(&mut events, |e| {
        matches!(e, AcpEvent::ModeAdvertised { .. })
    })
    .await;
    match reopened.last() {
        Some(AcpEvent::ModeAdvertised { plan, .. }) => {
            assert_eq!(plan.request.as_deref(), Some("read-only"), "{plan:?}")
        }
        other => panic!("no mode report on resume: {other:?}"),
    }
    again.stop();
    let heard = std::fs::read_to_string(dir.join(".devplane/heard.log")).unwrap_or_default();
    assert_eq!(set_lines(&heard).len(), 2, "{heard}");
}

#[tokio::test]
async fn mode_configured_start_is_kept() {
    let (_, plan, outcome, heard, _) =
        set_up(&codex_like("DEVPLANE_ECHO_START=agent-full-access")).await;
    assert_eq!(plan.why, devplane::core::run::Why::YouSetIt);
    assert_eq!(outcome, None);
    assert!(set_lines(&heard).is_empty(), "{heard}");
}

#[tokio::test]
async fn mode_preference_is_requested_once() {
    let mut spec = codex_like("DEVPLANE_ECHO_START=agent");
    spec.mode = Some("workspace-write".into());
    let (_, plan, _, heard, _) = set_up(&spec).await;
    assert_eq!(plan.why, devplane::core::run::Why::YouChose);
    assert_eq!(
        set_lines(&heard),
        ["set_config_option mode=workspace-write"],
        "{heard}"
    );
}

#[tokio::test]
async fn mode_preference_not_offered_requests_nothing() {
    let mut spec = codex_like("DEVPLANE_ECHO_START=agent");
    spec.mode = Some("turbo".into());
    let (_, plan, _, heard, _) = set_up(&spec).await;
    assert_eq!(plan.why, devplane::core::run::Why::ChoseUnoffered);
    assert!(set_lines(&heard).is_empty(), "{heard}");
}

#[tokio::test]
async fn mode_preference_already_current_requests_nothing() {
    let mut spec = codex_like("DEVPLANE_ECHO_START=agent");
    spec.mode = Some("agent".into());
    let (_, plan, _, heard, _) = set_up(&spec).await;
    assert_eq!(plan.why, devplane::core::run::Why::YouChose);
    assert!(set_lines(&heard).is_empty(), "{heard}");
}

/// A config-option update and a mode update about one change are one change.
#[tokio::test]
async fn mode_change_mid_session_is_reported_once() {
    let spec = codex_like(
        "DEVPLANE_ECHO_START=read-only DEVPLANE_ECHO_MODE_UPDATE_AFTER=agent-full-access",
    );
    let (session, mut events) = devplane::acp::spawn(&spec, scratch(), &[], None)
        .await
        .unwrap();
    collect(&mut events, |e| {
        matches!(e, AcpEvent::ModeAdvertised { .. })
    })
    .await;
    session.prompt("hello").await.unwrap();
    let turn = collect(&mut events, |e| matches!(e, AcpEvent::TurnEnded { .. })).await;
    let moved = turn
        .iter()
        .filter(|e| matches!(e, AcpEvent::ModeChanged { mode } if mode == "agent-full-access"))
        .count();
    assert_eq!(moved, 1, "{turn:?}");
    session.stop();
}

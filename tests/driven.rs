//! Driven runs end to end: what the host records about a real agent process
//! (`examples/echo_agent`), the rows a person reads afterwards.

mod common;

use devplane::core::{RunId, RunState};
use devplane::host::Shared;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{boot, echo_spec as echo_agent, post, trust};

fn scratch_repo(tag: &str) -> PathBuf {
    common::scratch_repo(tag, common::PASSING)
}

/// Polls the run until `want` holds, or says what it was doing instead.
async fn await_run(state: &Shared, run: &RunId, want: impl Fn(&devplane::core::Run) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut last = None;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let r = state.world.lock().await.run(run).cloned();
        if let Some(r) = r {
            if want(&r) {
                return;
            }
            last = Some(r);
        }
    }
    panic!(
        "the run never got there; it reads {:?} with {} tool rows, {} turns and blocked on {:?}",
        last.as_ref().map(|r| r.state.clone()),
        last.as_ref().map(|r| r.recent_tools.len()).unwrap_or(0),
        last.as_ref().map(|r| r.totals.turns).unwrap_or(0),
        last.as_ref()
            .and_then(|r| r.blocked_on.as_ref().map(|b| b.waiting_for.clone())),
    );
}

/// What the fixture was told, which the event log cannot answer.
fn heard(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".devplane/heard.log")).unwrap_or_default()
}

async fn wait_for_heard(dir: &Path) -> String {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        let text = heard(dir);
        if !text.is_empty() {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the agent was never prompted");
}

/// The text shown is the text sent: placeholders resolve against the project,
/// and an unfillable name refuses before anything starts.
#[tokio::test]
async fn a_placeholder_reaches_the_agent_resolved_or_the_dispatch_is_refused() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("resolve");
    trust(&c, &addr, &repo).await;
    let name = state
        .world
        .lock()
        .await
        .project(&devplane::core::ProjectId::from_path(&repo))
        .map(|p| p.name.clone())
        .expect("trusted, so registered");

    // A run with no change.
    devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("Hello from {project} on {branch}".into()),
        Vec::new(),
    )
    .await
    .expect("resolves");
    let text = wait_for_heard(&repo).await;
    assert!(
        text.contains(&format!("Hello from {name} on main")),
        "the agent was told the braces, not the facts: {text}"
    );
    assert!(!text.contains("{project}"), "{text}");
    for (_, s) in state.sessions.lock().await.drain() {
        s.stop();
    }

    // A change, through the route the interface and the CLI use.
    let work = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "say the project",
            "prompt": "Change in {project}: {dirty}",
            "agent": agent.command,
            "worktree": false,
        }),
    )
    .await;
    assert!(work["change_id"].is_string(), "{work}");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let text = heard(&repo);
        if text.contains(&format!("Change in {name}: the worktree")) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the change's prompt never arrived resolved: {text}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // The record holds what was sent, not the template.
    assert_eq!(
        work["change"]["prompt"].as_str().map(|p| p.contains('{')),
        Some(false),
        "the change row kept the braces: {}",
        work["change"]["prompt"]
    );

    // A name nothing can fill: refused, by name, and nothing started.
    let heard_before = heard(&repo);
    let refused = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("Fix {gate.failures} and {nope}".into()),
        Vec::new(),
    )
    .await
    .expect_err("nothing has failed and nope is not a field");
    let says = refused.to_string();
    assert!(says.contains("gate.failures"), "{says}");
    assert!(says.contains("nope"), "{says}");
    assert_eq!(heard(&repo), heard_before, "an agent was prompted anyway");
    let refused = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "agent": agent.command,
            "cwd": repo.to_string_lossy(),
            "title": "x",
            "prompt": "{nope}",
        }),
    )
    .await;
    assert!(
        refused["error"].as_str().unwrap_or("").contains("nope"),
        "{refused}"
    );
}

/// The preflight resolves per project as the start will, refusing by name and
/// writing nothing when a placeholder cannot be filled.
#[tokio::test]
async fn the_preflight_refuses_a_prompt_that_cannot_be_resolved_there() {
    let (addr, c, _state) = boot().await;
    let repo = scratch_repo("preflight");
    trust(&c, &addr, &repo).await;

    let v = post(
        &c,
        &addr,
        "/api/changes/preflight",
        serde_json::json!({
            "projects": [repo.to_string_lossy()],
            "agent": "claude",
            "title": "bump",
            "prompt": "Bump {project}",
        }),
    )
    .await;
    assert!(v["targets"][0]["refusal"].is_null(), "{v}");

    let v = post(
        &c,
        &addr,
        "/api/changes/preflight",
        serde_json::json!({
            "projects": [repo.to_string_lossy()],
            "agent": "claude",
            "title": "fix",
            "prompt": "Fix {gate.failures}",
        }),
    )
    .await;
    assert_eq!(v["targets"][0]["refusal"], "cannot_send", "{v}");
    assert!(
        v["targets"][0]["says"]
            .as_str()
            .unwrap_or("")
            .contains("gate.failures"),
        "{v}"
    );
    assert!(!repo.join(".claude/worktrees").exists());
}

/// A person stopping a run is recorded as theirs — not as the agent finishing.
#[tokio::test]
async fn a_person_stopping_a_run_is_recorded_as_a_stop_not_a_completion() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("stop");
    trust(&c, &addr, &repo).await;

    // Stopped under an open question, so the ask has an ending to get right.
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("ask me a question".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| {
        r.state == RunState::Waiting(devplane::core::WaitingFor::Question)
    })
    .await;

    devplane::driven::stop(&state, &run)
        .await
        .expect("a driven run stops");
    await_run(&state, &run, |r| !r.state.is_live()).await;

    let r = state.world.lock().await.run(&run).cloned().unwrap();
    assert_eq!(
        r.state,
        RunState::Stopped,
        "a stop read as the agent finishing"
    );

    let decisions = state.store.decisions(Some(run.as_str()), 20).await.unwrap();
    assert!(
        decisions
            .iter()
            .any(|d| d.action == "run:stop" && d.authority == devplane::core::Authority::Person),
        "nothing recorded who stopped it: {decisions:?}"
    );
    let asks = state.store.asks(10).await.unwrap();
    let ask = asks
        .iter()
        .find(|a| a.run == run)
        .expect("the question was written down");
    assert_eq!(
        ask.ended,
        Some(devplane::core::ask::Ended::Stopped),
        "the person's stop was recorded as nobody's, or as an answer"
    );
}

/// One tool call is one row, opened and closed once across its three updates.
#[tokio::test]
async fn a_tool_call_is_one_row_that_finishes() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("tool");
    trust(&c, &addr, &repo).await;

    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("run a tool please".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;

    let r = state.world.lock().await.run(&run).cloned().unwrap();
    assert_eq!(r.totals.tool_calls, 1, "counted per update, not per call");
    assert_eq!(r.recent_tools.len(), 1, "{:?}", r.recent_tools);
    assert_eq!(r.recent_tools[0].ok, Some(true), "the row never finished");
    assert!(
        r.recent_tools[0].tool.contains("cargo test"),
        "the row lost its title: {:?}",
        r.recent_tools[0]
    );
}

/// A reported plan is an event, so it survives a replay of the log.
#[tokio::test]
async fn a_plan_survives_a_restart_via_replay() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("plan");
    trust(&c, &addr, &repo).await;

    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("make a plan".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    assert_eq!(
        state.world.lock().await.run(&run).unwrap().plan.len(),
        2,
        "the plan reached the run"
    );

    // What a restart does: the world from the log alone.
    let events = state.store.events_for_run(&run, 1000).await.unwrap();
    let mut world = devplane::core::World::new();
    world.replay(events);
    let replayed = world.run(&run).expect("the run replays");
    assert_eq!(replayed.plan.len(), 2, "the plan was not in the log");
    assert_eq!(replayed.plan[1].content, "fix the cause");
    assert_eq!(replayed.plan[1].status, "in_progress");
}

/// An expired deadline sends the agent its own reject option, not `cancelled`,
/// which some adapters read as an aborted turn.
#[tokio::test]
async fn an_expired_permission_is_refused_with_the_agents_own_option() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("expire");
    trust(&c, &addr, &repo).await;

    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("this needs permission".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| {
        r.state == RunState::Waiting(devplane::core::WaitingFor::Permission)
    })
    .await;

    // What the sweeper does when the project's clock runs out.
    let mut ask = state
        .store
        .open_asks()
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.run == run)
        .expect("the permission was written down");
    let ended = devplane::core::ask::Ended::Timer {
        after: 1,
        set_by: "devplane.toml".into(),
    };
    ask.end(ended.clone(), jiff::Timestamp::now());
    state.store.save_ask(&ask).await.unwrap();
    devplane::driven::expire(&state, &ask, &ended).await;

    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let said: Vec<String> = state
        .store
        .messages_for_run(&run, 50)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.text)
        .collect();
    assert!(
        said.iter()
            .any(|t| t.contains("permission outcome: Selected")),
        "the agent was sent `cancelled` rather than its reject option: {said:?}"
    );
    let decisions = state.store.decisions(Some(run.as_str()), 20).await.unwrap();
    let timer = decisions
        .iter()
        .find(|d| d.authority == devplane::core::Authority::Timer)
        .expect("the timer's row");
    assert_eq!(timer.outcome, "deny");
}

/// Answering clears the block immediately, not at the agent's next move, which
/// never comes for a turn that ends in prose.
#[tokio::test]
async fn answering_a_question_clears_the_block() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("answer");
    trust(&c, &addr, &repo).await;

    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("ask me a question".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| {
        r.state == RunState::Waiting(devplane::core::WaitingFor::Question)
    })
    .await;
    let ask = state
        .store
        .open_asks()
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.run == run)
        .expect("the question was written down");

    // An answer naming what was not asked is refused before the row closes:
    // the question stays open, and the person can answer it properly.
    for (field, chosen) in [("question_0", "Nope"), ("question_9", "Drop it")] {
        let refused = devplane::driven::answer_ask(
            &state,
            ask.id.as_str(),
            devplane::driven::Answer::Question(vec![(
                field.into(),
                devplane::core::question::Chosen::Option(chosen.into()),
            )]),
            devplane::core::ask::AnswerPath::Cli,
        )
        .await
        .expect_err("an answer to nothing that was asked");
        assert!(refused.to_string().contains('`'), "{refused}");
        assert!(
            open_asks(&state, &run).await.iter().any(|a| a.id == ask.id),
            "a refused answer closed the question"
        );
    }

    devplane::driven::answer_ask(
        &state,
        ask.id.as_str(),
        devplane::driven::Answer::Question(vec![(
            "question_0".into(),
            devplane::core::question::Chosen::Option("Drop it".into()),
        )]),
        devplane::core::ask::AnswerPath::Cli,
    )
    .await
    .expect("delivered");

    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let r = state.world.lock().await.run(&run).cloned().unwrap();
    assert_ne!(
        r.state,
        RunState::Waiting(devplane::core::WaitingFor::Question),
        "answered, and still reading as waiting on the question"
    );
    assert!(r.blocked_on.is_none(), "{:?}", r.blocked_on);
}

/// The open asks of one run.
async fn open_asks(state: &Shared, run: &RunId) -> Vec<devplane::core::ask::Ask> {
    state
        .store
        .open_asks()
        .await
        .unwrap()
        .into_iter()
        .filter(|a| &a.run == run)
        .collect()
}

/// Waits until the run has `n` open asks.
async fn await_asks(state: &Shared, run: &RunId, n: usize) -> Vec<devplane::core::ask::Ask> {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let asks = open_asks(state, run).await;
        if asks.len() == n {
            return asks;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "expected {n} open asks, have {}",
            asks.len()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// What the agent said on this run, as kept in its transcript.
async fn said(state: &Shared, run: &RunId) -> Vec<String> {
    state
        .store
        .messages_for_run(run, 200)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.text)
        .collect()
}

async fn await_said(state: &Shared, run: &RunId, needle: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        if said(state, run).await.iter().any(|t| t.contains(needle)) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the agent never said `{needle}`: {:?}",
            said(state, run).await
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Two calls ask at once. Each answer reaches the call it answers, with that
/// call's own options and title — never the options of whichever asked last.
#[tokio::test]
async fn parallel_permissions_are_answered_against_their_own_request() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("twins");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("twin-asks".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let asks = await_asks(&state, &run, 2).await;
    let a = asks.iter().find(|a| a.message == "Read a.txt").expect("A");
    let b = asks.iter().find(|a| a.message == "rm -rf b").expect("B");
    let shown = |r: &devplane::core::Run| r.blocked_on.as_ref().and_then(|b| b.request_id.clone());

    // Whichever the run shows, answer the *other* one first.
    let (first, second) = {
        let w = state.world.lock().await;
        let r = w.run(&run).unwrap();
        if shown(r).as_deref() == Some(a.request_id.as_str()) {
            (b.clone(), a.clone())
        } else {
            (a.clone(), b.clone())
        }
    };
    devplane::driven::answer_ask(
        &state,
        first.id.as_str(),
        devplane::driven::Answer::Permission(devplane::driven::Decision::Allow),
        devplane::core::ask::AnswerPath::Cli,
    )
    .await
    .expect("delivered");
    {
        let w = state.world.lock().await;
        let r = w.run(&run).unwrap();
        assert_eq!(
            shown(r).as_deref(),
            Some(second.request_id.as_str()),
            "the request still waiting left the inbox"
        );
    }
    devplane::driven::answer_ask(
        &state,
        second.id.as_str(),
        devplane::driven::Answer::Permission(devplane::driven::Decision::Deny),
        devplane::core::ask::AnswerPath::Cli,
    )
    .await
    .expect("delivered");
    await_run(&state, &run, |r| r.totals.turns >= 1).await;

    let (first_call, second_call) = match first.message.as_str() {
        "Read a.txt" => ("pa", "pb"),
        _ => ("pb", "pa"),
    };
    let text = said(&state, &run).await.join("\n");
    assert!(
        text.contains(&format!("{first_call} outcome: Selected(SelectedPermissionOutcome {{ option_id: PermissionOptionId(\"allow-{first_call}\")")),
        "the first answer did not reach its own call with its own option: {text}"
    );
    assert!(
        text.contains(&format!(
            "option_id: PermissionOptionId(\"deny-{second_call}\")"
        )),
        "{text}"
    );
    let decisions = state.store.decisions(Some(run.as_str()), 20).await.unwrap();
    let person: Vec<(&str, &str)> = decisions
        .iter()
        .filter(|d| {
            d.action == "agent:tool.use" && d.authority == devplane::core::Authority::Person
        })
        .filter(|d| d.outcome == "allow" || d.outcome == "deny")
        .map(|d| (d.subject.as_str(), d.outcome.as_str()))
        .collect();
    assert!(
        person.contains(&(first.message.as_str(), "allow")),
        "the allow was recorded against another call: {person:?}"
    );
    assert!(
        person.contains(&(second.message.as_str(), "deny")),
        "{person:?}"
    );
}

/// An agent withdrawing its request ends the ask as nobody's, takes it out of
/// the inbox, and records no person deciding anything.
#[tokio::test]
async fn a_withdrawn_permission_ends_its_ask_as_nobodys() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("withdraw");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("withdraw".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    await_said(&state, &run, "withdrew the request").await;

    assert!(
        open_asks(&state, &run).await.is_empty(),
        "still in the inbox"
    );
    let ask = state
        .store
        .asks(10)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.run == run)
        .expect("written down");
    assert!(
        matches!(ask.ended, Some(devplane::core::ask::Ended::Nobody { .. })),
        "{:?}",
        ask.ended
    );
    assert!(ask.answer.is_none(), "a withdrawal is not an answer");
    let decisions = state.store.decisions(Some(run.as_str()), 20).await.unwrap();
    assert!(
        decisions
            .iter()
            .any(|d| d.outcome == "withdrawn" && d.authority == devplane::core::Authority::Nobody),
        "{decisions:?}"
    );
    assert!(
        !decisions
            .iter()
            .any(|d| d.action == "agent:tool.use"
                && d.authority == devplane::core::Authority::Person),
        "a person was credited with a withdrawal: {decisions:?}"
    );
    let r = state.world.lock().await.run(&run).cloned().unwrap();
    assert!(r.blocked_on.is_none(), "{:?}", r.blocked_on);
}

/// A follow-up prompt supersedes a pending request: it is answered
/// *cancelled*, ended as nobody's, and the prompt reaches the agent.
#[tokio::test]
async fn a_follow_up_prompt_cancels_what_was_still_waiting() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("followup");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("this needs permission".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_asks(&state, &run, 1).await;
    devplane::driven::prompt(&state, &run, "never mind, say hi".into())
        .await
        .expect("prompted");
    await_said(&state, &run, "permission outcome: Cancelled").await;
    await_run(&state, &run, |r| r.totals.turns >= 2).await;
    assert!(heard(&repo).contains("never mind, say hi"));
    assert!(open_asks(&state, &run).await.is_empty());
    let decisions = state.store.decisions(Some(run.as_str()), 20).await.unwrap();
    assert!(
        decisions
            .iter()
            .any(|d| d.outcome == "cancelled" && d.authority == devplane::core::Authority::Nobody),
        "{decisions:?}"
    );
}

/// Answering an ask the live session no longer waits on (one from before a
/// restart) reaches the agent as a message, and never withdraws the requests
/// it is waiting on now.
#[tokio::test]
async fn answering_a_stale_ask_leaves_the_live_requests_waiting() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("stale-ask");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("twin-asks".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let asks = await_asks(&state, &run, 2).await;
    let mut stale = asks[0].clone();
    stale.id = devplane::core::AskId::new("stale-from-before-a-restart");
    stale.request_id = "request-from-before-a-restart".into();
    state.store.save_ask(&stale).await.unwrap();

    let answered = devplane::driven::answer_ask(
        &state,
        stale.id.as_str(),
        devplane::driven::Answer::Permission(devplane::driven::Decision::Allow),
        devplane::core::ask::AnswerPath::Cli,
    )
    .await
    .expect("answered");
    assert!(
        matches!(
            answered.delivery,
            Some(devplane::core::ask::Delivery::Resumed)
        ),
        "{:?}",
        answered.delivery
    );
    let open = open_asks(&state, &run).await;
    assert_eq!(open.len(), 2, "a live request was withdrawn: {open:?}");
    let decisions = state.store.decisions(Some(run.as_str()), 20).await.unwrap();
    assert!(
        !decisions.iter().any(|d| d.outcome == "cancelled"),
        "{decisions:?}"
    );
}

/// The pids of the fixture processes that opened a session in `repo`, and
/// how many of them are still running.
fn alive_agents(repo: &Path) -> usize {
    std::fs::read_dir(repo.join(".devplane/pids"))
        .map(|d| {
            d.filter_map(|e| e.ok()?.file_name().to_str()?.parse::<i32>().ok())
                // SAFETY: signal 0 only checks for the process.
                .filter(|pid| unsafe { libc::kill(*pid, 0) } == 0)
                .count()
        })
        .unwrap_or(0)
}

/// Waits until the running agents match what the host owns: one live
/// session, or none.
async fn await_owned(state: &Shared, run: &RunId, repo: &Path) -> usize {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let owned = {
            let map = state.sessions.lock().await;
            map.get(run)
                .map(|s| usize::from(s.is_live() && !s.is_stopping()))
                .unwrap_or(0)
        };
        let settled = state
            .sessions
            .lock()
            .await
            .get(run)
            .is_none_or(|s| !s.is_stopping());
        let running = alive_agents(repo);
        if settled && running == owned {
            return owned;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{running} agent process(es) running while the host owns {owned}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Resumes racing each other, or a stop, leave exactly the one agent the
/// host owns — never a second, unowned process.
#[tokio::test]
async fn racing_resumes_and_stops_leave_exactly_the_owned_agent() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("race");
    trust(&c, &addr, &repo).await;
    let run =
        devplane::driven::dispatch(&state, &agent, repo.clone(), Some("hi".into()), Vec::new())
            .await
            .unwrap();
    await_run(&state, &run, |r| {
        r.totals.turns >= 1 && r.agent_session.is_some()
    })
    .await;

    // Two resumes right behind a stop, while the old agent is still going.
    devplane::driven::stop(&state, &run).await.unwrap();
    let (a, b) = tokio::join!(
        devplane::driven::resume(&state, &run),
        devplane::driven::resume(&state, &run)
    );
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "exactly one resume starts an agent: {a:?} {b:?}"
    );
    assert_eq!(await_owned(&state, &run, &repo).await, 1);

    // A stop and a resume at once: whatever order they land in, what runs is
    // what the host owns.
    let (_stopped, _resumed) = tokio::join!(
        devplane::driven::stop(&state, &run),
        devplane::driven::resume(&state, &run)
    );
    await_owned(&state, &run, &repo).await;

    if devplane::driven::is_live(&state, &run).await {
        devplane::driven::stop(&state, &run).await.unwrap();
    }
    assert_eq!(await_owned(&state, &run, &repo).await, 0);
}

/// The diff a call reported is on the run's log, keyed on the call, as the
/// agent's claim; and Devplane's MCP server was offered to the agent.
#[tokio::test]
async fn a_reported_diff_is_kept_on_the_log_and_our_tools_are_offered() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("diff");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("diff-edit".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let events = state.store.events_for_run(&run, 1000).await.unwrap();
    let reported: Vec<&devplane::core::Event> = events
        .iter()
        .map(|e| &e.event)
        .filter(|e| matches!(e, devplane::core::Event::EditReported { .. }))
        .collect();
    assert_eq!(reported.len(), 1, "one diff, kept once: {reported:?}");
    match reported[0] {
        devplane::core::Event::EditReported {
            call_id,
            new_text,
            old_text,
            omitted_bytes,
            ..
        } => {
            assert_eq!(call_id, "d1");
            assert_eq!(new_text, "two\n", "the fragment was kept, not the whole");
            assert_eq!(old_text.as_deref(), Some("one\n"));
            assert_eq!(*omitted_bytes, 0);
        }
        _ => unreachable!(),
    }

    let offered: Value = serde_json::from_str(
        &std::fs::read_to_string(repo.join(".devplane/mcp.json")).unwrap_or_default(),
    )
    .unwrap_or_default();
    assert_eq!(offered[0]["name"], "devplane", "{offered}");
    assert_eq!(offered[0]["args"], serde_json::json!(["mcp"]), "{offered}");
    assert!(
        offered[0]["env"].as_array().is_some_and(|e| e
            .iter()
            .any(|v| v["name"] == "DEVPLANE_RUN" && v["value"] == run.as_str())),
        "{offered}"
    );
}

/// A repository that keeps no transcript keeps no reported diff either: it is
/// file content the agent said.
#[tokio::test]
async fn a_reported_diff_is_not_kept_where_transcripts_are_not() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("diff-unkept");
    std::fs::write(
        repo.join("devplane.toml"),
        "[gates]\ncheck = [\"true\"]\n\n[agent]\nkeep_transcripts = false\n",
    )
    .unwrap();
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("diff-edit".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let events = state.store.events_for_run(&run, 1000).await.unwrap();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e.event, devplane::core::Event::EditReported { .. })),
        "a diff was kept although transcripts are not"
    );
}

// ── Calls merged field by field; modes stated ────────────────────────────────

/// The fixture with extra environment, as a launch spec.
fn echo_with(env: &str) -> Option<devplane::acp::AgentSpec> {
    let base = echo_agent()?;
    Some(devplane::acp::AgentSpec::new(
        "echo",
        "Echo",
        &format!("env {env} {}", base.command),
    ))
}

/// Every event this run logged, oldest first.
async fn logged(state: &Shared, run: &RunId) -> Vec<devplane::core::event::Event> {
    let mut all = state.store.events_for_run(run, 5000).await.unwrap();
    all.sort_by_key(|e| e.at);
    all.into_iter().map(|e| e.event).collect()
}

/// Waits until the log holds an event `want` accepts.
async fn await_logged(
    state: &Shared,
    run: &RunId,
    what: &str,
    want: impl Fn(&devplane::core::event::Event) -> bool,
) -> Vec<devplane::core::event::Event> {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let all = logged(state, run).await;
        if all.iter().any(&want) {
            return all;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "never logged {what}: {all:#?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn decided(events: &[devplane::core::event::Event]) -> Vec<(String, String, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            devplane::core::event::Event::PermissionDecided {
                tool, decision, by, ..
            } => Some((tool.clone(), decision.clone(), by.clone())),
            _ => None,
        })
        .collect()
}

fn started(events: &[devplane::core::event::Event]) -> Vec<(Option<String>, Value)> {
    events
        .iter()
        .filter_map(|e| match e {
            devplane::core::event::Event::ToolStarted { call_id, input, .. } => {
                Some((call_id.clone(), input.clone()))
            }
            _ => None,
        })
        .collect()
}

const DENY_RM: &str = "[policy]\ndeny = [\"Bash(rm -rf *)\"]\n";

/// A permission that names only the call's id is matched on the input an
/// earlier, status-less update carried, and the decision names the call.
#[tokio::test]
async fn a_permission_naming_only_the_id_is_matched_on_the_earlier_input() {
    let Some(agent) = echo_with("DEVPLANE_ECHO_SPARSE=1") else {
        return;
    };
    let (addr, c, state) = boot().await;
    let repo = common::scratch_repo("sparse-deny", DENY_RM);
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo,
        Some("run a tool please".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let events = await_logged(&state, &run, "a decision", |e| {
        matches!(e, devplane::core::event::Event::PermissionDecided { .. })
    })
    .await;
    let d = decided(&events);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(
        d[0].0, "Run a command",
        "the decision lost the call's title"
    );
    assert_eq!(d[0].1, "deny");
    assert!(d[0].2.starts_with("policy:"), "{d:?}");
    assert!(d.iter().all(|(t, _, _)| !t.is_empty()));
}

/// The call's ledger row opens with the input that arrived after its first
/// report.
#[tokio::test]
async fn an_input_sent_later_reaches_the_ledger_row() {
    let Some(agent) = echo_with("DEVPLANE_ECHO_SPARSE=1") else {
        return;
    };
    let (addr, c, state) = boot().await;
    let repo = common::scratch_repo("sparse-row", DENY_RM);
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo,
        Some("run a tool please".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let events = await_logged(&state, &run, "the call's row", |e| {
        matches!(e, devplane::core::event::Event::ToolStarted { .. })
    })
    .await;
    let rows = started(&events);
    assert_eq!(rows[0].0.as_deref(), Some("s1"));
    assert_eq!(rows[0].1["command"], "rm -rf build", "{rows:?}");
}

/// An update after the call finished merges into its record and never opens
/// a second row.
#[tokio::test]
async fn a_late_update_never_reopens_a_finished_row() {
    let Some(agent) = echo_with("DEVPLANE_ECHO_SPARSE=1") else {
        return;
    };
    let (addr, c, state) = boot().await;
    let repo = common::scratch_repo("sparse-late", DENY_RM);
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo,
        Some("run a tool please".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let events = logged(&state, &run).await;
    let rows: Vec<_> = started(&events)
        .into_iter()
        .filter(|(id, _)| id.as_deref() == Some("s1"))
        .collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let finished = events
        .iter()
        .filter(|e| matches!(e, devplane::core::event::Event::ToolFinished { call_id, .. } if call_id.as_deref() == Some("s1")))
        .count();
    assert_eq!(finished, 1, "the late completion closed a second time");
}

const CODEX: &str = "DEVPLANE_ECHO_AGENT_INFO=@agentclientprotocol/codex-acp@2.1.1 \
     'DEVPLANE_ECHO_CONFIG_MODE=mode:read-only=Read-only,workspace-write=Workspace access,agent=Auto review,agent-full-access=Full access'";

async fn statement(state: &Shared, run: &RunId) -> devplane::core::run::ModeStatement {
    await_run(state, run, |r| r.mode_statement.is_some()).await;
    state
        .world
        .lock()
        .await
        .run(run)
        .and_then(|r| r.mode_statement.clone())
        .unwrap()
}

/// A Codex session that opens in Auto review with no mode set is asked for
/// Read-only, the change says so, its calls reach the inbox, and asking wrote
/// no decision.
#[tokio::test]
async fn mode_statement_says_it_asked() {
    let Some(agent) = echo_with(&format!("{CODEX} DEVPLANE_ECHO_START=agent")) else {
        return;
    };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("mode-asked");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(&state, &agent, repo, None, Vec::new())
        .await
        .unwrap();
    let s = statement(&state, &run).await;
    assert_eq!(
        s.sentence(),
        "asked Echo to ask before acting — you set no mode"
    );
    assert_eq!(s.reading, devplane::core::run::Reading::Asks);
    assert_eq!(s.name.as_deref(), Some("Read-only"));
    let decisions = state.store.decisions(Some(run.as_str()), 20).await.unwrap();
    assert!(
        decisions.is_empty(),
        "asking for a mode decided something: {decisions:?}"
    );

    let session = state.sessions.lock().await.get(&run).cloned().unwrap();
    session.prompt("this needs permission").await.unwrap();
    await_run(&state, &run, |r| {
        r.state == RunState::Waiting(devplane::core::WaitingFor::Permission)
    })
    .await;
    // The board and the change read the same sentence.
    let view = devplane::view::RunView::of(
        state.world.lock().await.run(&run).unwrap(),
        None,
        jiff::Timestamp::now(),
    );
    assert_eq!(view.asks_a_person, Some(true));
    assert_eq!(
        view.mode_statement.map(|m| m.sentence),
        Some("asked Echo to ask before acting — you set no mode".to_string())
    );
}

/// A start the person configured is kept and named for what it decides.
#[tokio::test]
async fn mode_statement_names_a_deciding_mode_you_set() {
    let Some(agent) = echo_with(&format!("{CODEX} DEVPLANE_ECHO_START=agent-full-access")) else {
        return;
    };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("mode-set");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("hello".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let s = statement(&state, &run).await;
    assert_eq!(
        s.sentence(),
        "decides without you: Full access (everything) — you set it"
    );
    let heard = wait_for_heard(&repo).await;
    assert!(
        !heard.contains("set_"),
        "a configured mode was overridden: {heard}"
    );
    let view = devplane::view::RunView::of(
        state.world.lock().await.run(&run).unwrap(),
        None,
        jiff::Timestamp::now(),
    );
    assert_eq!(view.asks_a_person, Some(false));
}

/// A move mid-session moves the statement; the earlier one stays in the log,
/// and Devplane does not move the mode back.
#[tokio::test]
async fn mode_statement_follows_a_mid_session_change() {
    let Some(agent) = echo_with(&format!(
        "{CODEX} DEVPLANE_ECHO_START=agent DEVPLANE_ECHO_MODE_UPDATE_AFTER=agent-full-access"
    )) else {
        return;
    };
    let (addr, c, state) = boot().await;
    let repo = scratch_repo("mode-moved");
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        repo.clone(),
        Some("hello".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| {
        r.mode_statement
            .as_ref()
            .is_some_and(|s| s.why == devplane::core::run::Why::Moved)
    })
    .await;
    let s = statement(&state, &run).await;
    assert_eq!(
        s.sentence(),
        "Echo moved to Full access — decides without you: everything"
    );
    let stated: Vec<String> = logged(&state, &run)
        .await
        .into_iter()
        .filter_map(|e| match e {
            devplane::core::event::Event::ModeStated { statement } => Some(statement.sentence()),
            _ => None,
        })
        .collect();
    assert_eq!(stated.len(), 2, "{stated:?}");
    assert_eq!(
        stated[0],
        "asked Echo to ask before acting — you set no mode"
    );
    let heard = heard(&repo);
    assert_eq!(
        heard.matches("set_config_option").count(),
        1,
        "moved back: {heard}"
    );
}

// ── Each pinned adapter's replay ─────────────────────────────────────────────

/// Drives the host through one adapter's transcript, with a project rule
/// denying that transcript's call by its full input.
async fn replay(
    registry_id: &str,
    agent_id: &str,
) -> (Vec<devplane::core::event::Event>, Value, Shared, RunId) {
    let pin = devplane::acp::builtin()
        .into_iter()
        .find(|a| a.id == agent_id)
        .expect("a built-in agent");
    let (_, version) = pin.npm_pin().expect("an npm pin");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/acp")
        .join(format!("{registry_id}@{version}"));
    assert!(
        dir.join("session.jsonl").is_file(),
        "{agent_id} is pinned to {version} and has no recording at {}",
        dir.display()
    );
    let expect: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expect.json")).unwrap()).unwrap();
    // The rule names the call's full command; an adapter that sends none
    // still has a rule about what may run in force.
    let command = expect["command"]
        .as_str()
        .unwrap_or("touch devplane-replay-marker");
    let config = format!("[policy]\ndeny = [\"Bash({command}*)\"]\n");
    let agent = echo_with(&format!(
        "DEVPLANE_ECHO_REPLAY={}",
        dir.join("session.jsonl").display()
    ))
    .expect("build the fixture first");
    let (addr, c, state) = boot().await;
    let repo = common::scratch_repo(&format!("replay-{registry_id}"), &config);
    trust(&c, &addr, &repo).await;
    let run = devplane::driven::dispatch(&state, &agent, repo, Some("replay".into()), Vec::new())
        .await
        .unwrap();
    let events = await_logged(&state, &run, "a decision or an ask", |e| {
        matches!(
            e,
            devplane::core::event::Event::PermissionDecided { .. }
                | devplane::core::event::Event::Blocked { .. }
        )
    })
    .await;
    // Nothing that names a call is ever blank.
    for (title, _, _) in decided(&events) {
        assert!(!title.is_empty());
    }
    for (_, input) in started(&events) {
        assert_ne!(input, Value::String(String::new()));
    }
    (events, expect, state.0.clone(), run)
}

fn assert_denied_by_rule(events: &[devplane::core::event::Event], expect: &Value) {
    let d = decided(events);
    assert!(
        d.iter().any(|(tool, decision, by)| decision == "deny"
            && by.starts_with("policy:")
            && Some(tool.as_str()) == expect["title"].as_str()),
        "the rule never saw the call's full input, or the title was lost: {d:?} (want {expect})"
    );
}

#[tokio::test]
async fn replay_claude_acp_denies_the_full_input() {
    if echo_agent().is_none() {
        return;
    }
    let (events, expect, ..) = replay("claude-acp", "claude").await;
    assert_denied_by_rule(&events, &expect);
}

#[tokio::test]
async fn replay_codex_acp_denies_the_full_input() {
    if echo_agent().is_none() {
        return;
    }
    let (events, expect, state, run) = replay("codex-acp", "codex").await;
    assert_denied_by_rule(&events, &expect);
    // The transcript opens in Auto review with nothing configured: asked for
    // Read-only, from the adapter's own advertisement.
    let s = state
        .world
        .lock()
        .await
        .run(&run)
        .and_then(|r| r.mode_statement.clone());
    assert_eq!(
        s.map(|s| s.sentence()),
        Some("asked Echo to ask before acting — you set no mode".to_string())
    );
}

#[tokio::test]
async fn replay_github_copilot_cli_denies_the_full_input() {
    if echo_agent().is_none() {
        return;
    }
    let (events, expect, ..) = replay("github-copilot-cli", "copilot").await;
    assert_denied_by_rule(&events, &expect);
}

/// Gemini CLI sends no input for a call, so no rule can match its command:
/// with a rule about what may run in force, a person decides, the inbox says
/// why, and the title is never matched in the command's place.
#[tokio::test]
async fn replay_gemini_denies_the_full_input() {
    if echo_agent().is_none() {
        return;
    }
    let (events, expect, ..) = replay("gemini", "gemini").await;
    assert!(
        expect["command"].is_null(),
        "the recording now carries input: {expect}"
    );
    assert!(
        decided(&events).is_empty(),
        "a rule decided a call it could not see"
    );
    let blocked = events.iter().find_map(|e| match e {
        devplane::core::event::Event::Blocked { message, .. } => message.clone(),
        _ => None,
    });
    assert_eq!(
        blocked.as_deref(),
        Some("touch devplane-replay-marker — the agent did not send what it means to run")
    );
}

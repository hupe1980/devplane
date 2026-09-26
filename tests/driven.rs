//! Driven runs end to end: what the host records about a real agent process
//! (`examples/echo_agent`), the rows a person reads afterwards.

mod common;

use devplane::core::{Policy, RunId, RunState};
use devplane::host::{AppState, Shared};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A git repository with a `devplane.toml`, trusted by the host.
fn scratch_repo(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-driven-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("git");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(dir.join("devplane.toml"), "[gates]\ncheck = [\"true\"]\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

fn echo_agent() -> Option<devplane::acp::AgentSpec> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?.parent()?.join("examples").join("echo_agent");
    bin.exists()
        .then(|| devplane::acp::AgentSpec::new("echo", "Echo", &bin.to_string_lossy()))
}

/// A host for one test; its agents end with it.
struct Host(Shared, #[allow(dead_code)] common::AgentSerial);
impl std::ops::Deref for Host {
    type Target = Shared;
    fn deref(&self) -> &Shared {
        &self.0
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        for _ in 0..100 {
            if let Ok(mut map) = self.0.sessions.try_lock() {
                for (_, session) in map.drain() {
                    session.stop();
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

async fn boot() -> (std::net::SocketAddr, reqwest::Client, Host) {
    let serial = common::one_agent_at_a_time();
    let db = std::env::temp_dir().join(format!("vp-driven-{}.db", uuid::Uuid::new_v4().simple()));
    let state = AppState::new(
        db.clone(),
        "tok".into(),
        Policy::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new(), Host(state, serial))
}

async fn post(c: &reqwest::Client, addr: &std::net::SocketAddr, path: &str, body: Value) -> Value {
    c.post(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn trust(c: &reqwest::Client, addr: &std::net::SocketAddr, repo: &Path) {
    let res = post(
        c,
        addr,
        "/api/projects/trust",
        serde_json::json!({ "path": repo.to_string_lossy() }),
    )
    .await;
    assert_eq!(res["trusted"], true, "{res}");
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
            "test",
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
        "test",
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
        "test",
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
        "test",
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
        "test",
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
        "[gates]\ncheck = [\"true\"]\n\n[transcripts]\nkeep = false\n",
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

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

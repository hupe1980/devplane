//! What a driven agent's protocol stream adds to a change: the diff each call
//! reported, on the decision about that call; the session's close and the
//! agent's notices on the record; a change continued with a second agent; a
//! sign-in the person chose; and the change's own tasks read through
//! Devplane's MCP server.

mod common;

use devplane::core::{RunId, RunState};
use devplane::host::Shared;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{boot, echo_spec, post, trust};

fn repo(tag: &str) -> PathBuf {
    common::scratch_repo_with(tag, common::PASSING, &[(".gitignore", ".devplane/\n")])
}

fn echo_with(env: &str) -> Option<devplane::acp::AgentSpec> {
    let base = echo_spec()?;
    Some(devplane::acp::AgentSpec::new(
        "echo",
        "Echo",
        &format!("env {env} {}", base.command),
    ))
}

async fn await_run(state: &Shared, run: &RunId, want: impl Fn(&devplane::core::Run) -> bool) {
    common::until("the run to get there", Duration::from_secs(30), || async {
        state
            .world
            .lock()
            .await
            .run(run)
            .filter(|r| want(r))
            .map(|_| ())
    })
    .await
}

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

async fn next_ask(state: &Shared, run: &RunId) -> devplane::core::ask::Ask {
    common::until("an open ask", Duration::from_secs(30), || async {
        open_asks(state, run).await.into_iter().next()
    })
    .await
}

fn heard(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".devplane/heard.log")).unwrap_or_default()
}

// ── The edit on the approval that allowed it ────────────────────────────────

/// Ten edit calls, each asked for with its diff and each allowed by a person:
/// every decision row carries its own call's diff, labelled as the agent's
/// report, and the pending ask carried it before the answer.
#[tokio::test]
async fn the_diff_is_on_the_decision_that_allowed_it() {
    let Some(agent) = echo_spec() else { return };
    let (addr, c, state) = boot().await;
    let dir = repo("ten-edits");
    trust(&c, &addr, &dir).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        dir.clone(),
        Some("ten-edits".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    for n in 1..=10 {
        let ask = next_ask(&state, &run).await;
        assert_eq!(ask.payload["call_id"], format!("e{n}"), "{}", ask.payload);
        let diffs = ask.payload["diffs"].as_array().expect("the proposed diff");
        assert_eq!(diffs[0]["new_text"], format!("after {n}\n"));
        assert_eq!(diffs[0]["says"], "the agent's report of its edit");
        devplane::driven::answer_ask(
            &state,
            ask.id.as_str(),
            devplane::driven::Answer::Permission(devplane::driven::Decision::Allow),
            devplane::core::ask::AnswerPath::Cli,
        )
        .await
        .expect("delivered");
    }
    await_run(&state, &run, |r| r.totals.turns >= 1).await;

    let rows = devplane::view::decisions(&state.store, Some(run.as_str()), 100, false)
        .await
        .unwrap();
    let calls: Vec<_> = rows
        .iter()
        .filter(|r| r.decision.action == "agent:tool.use")
        .collect();
    let named: std::collections::BTreeSet<_> = calls
        .iter()
        .filter_map(|r| r.decision.call_id.clone())
        .collect();
    assert_eq!(named.len(), 10, "ten approvals, each naming its call");
    for row in calls {
        let call = row.decision.call_id.as_deref().expect("the call is named");
        let n = call.trim_start_matches('e');
        let edits = row
            .edits
            .as_ref()
            .expect("a driven call's row carries edits");
        assert_eq!(edits.len(), 1, "{call}: its own diff, and only that");
        assert!(edits[0].path.ends_with(format!("f{n}.txt")), "{call}");
        assert_eq!(
            edits[0].old_text.as_deref(),
            Some(format!("before {n}\n").as_str())
        );
        assert_eq!(edits[0].says, "the agent's report of its edit");
        assert_eq!(row.decision.authority.as_str(), "person");
    }
    // The run indexes what was reported; the bodies stay in the log.
    let w = state.world.lock().await;
    assert_eq!(w.run(&run).unwrap().edits_reported.len(), 10);
}

/// A project that keeps no transcripts keeps no diff: the ask says so and the
/// decision row says so, in a sentence distinct from *the agent sent none*.
#[tokio::test]
async fn no_diff_is_kept_where_transcripts_are_not() {
    let Some(agent) = echo_spec() else { return };
    let (addr, c, state) = boot().await;
    let dir = common::scratch_repo_with(
        "unkept",
        "[gates]\ncheck = [\"true\"]\n\n[agent]\nkeep_transcripts = false\n",
        &[(".gitignore", ".devplane/\n")],
    );
    trust(&c, &addr, &dir).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        dir.clone(),
        Some("diff-ask".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let ask = next_ask(&state, &run).await;
    assert!(ask.payload.get("diffs").is_none(), "{}", ask.payload);
    assert_eq!(
        ask.payload["no_diff"],
        "no diff was kept: this project keeps no transcripts"
    );
    devplane::driven::answer_ask(
        &state,
        ask.id.as_str(),
        devplane::driven::Answer::Permission(devplane::driven::Decision::Allow),
        devplane::core::ask::AnswerPath::Cli,
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let rows = devplane::view::decisions(&state.store, Some(run.as_str()), 100, false)
        .await
        .unwrap();
    let row = rows
        .iter()
        .find(|r| r.decision.call_id.as_deref() == Some("e1"))
        .expect("the decision names its call");
    assert_eq!(row.edits.as_deref().map(<[_]>::len), Some(0));
    assert_eq!(
        row.no_edit.as_deref(),
        Some("no diff was kept: this project keeps no transcripts")
    );
}

/// A request that carries no diff says so on the ask.
#[tokio::test]
async fn a_request_without_a_diff_says_so() {
    let Some(agent) = echo_spec() else { return };
    let (addr, c, state) = boot().await;
    let dir = repo("no-diff");
    trust(&c, &addr, &dir).await;
    let run = devplane::driven::dispatch(
        &state,
        &agent,
        dir.clone(),
        Some("permission".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let ask = next_ask(&state, &run).await;
    assert_eq!(ask.payload["call_id"], "t2");
    assert_eq!(
        ask.payload["no_diff"],
        "the agent sent no diff with this request"
    );
    devplane::driven::answer_ask(
        &state,
        ask.id.as_str(),
        devplane::driven::Answer::Permission(devplane::driven::Decision::Deny),
        devplane::core::ask::AnswerPath::Cli,
    )
    .await
    .unwrap();
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let rows = devplane::view::decisions(&state.store, Some(run.as_str()), 100, false)
        .await
        .unwrap();
    let row = rows
        .iter()
        .find(|r| r.decision.call_id.as_deref() == Some("t2"))
        .expect("the decision names its call");
    assert_eq!(
        row.no_edit.as_deref(),
        Some("the agent sent no diff for this call")
    );
}

// ── The session on the record ───────────────────────────────────────────────

/// A stop records whether the session was closed over the protocol, and a
/// notice the agent sent is kept on the run in its own words.
#[tokio::test]
async fn the_close_and_the_notices_are_on_the_record() {
    for (env, closed) in [
        ("DEVPLANE_ECHO_X=1", true),
        ("DEVPLANE_ECHO_NO_CLOSE=1", false),
    ] {
        let Some(agent) = echo_with(env) else { return };
        let (addr, c, state) = boot().await;
        let dir = repo("closed");
        trust(&c, &addr, &dir).await;
        let run = devplane::driven::dispatch(
            &state,
            &agent,
            dir.clone(),
            Some("notice".into()),
            Vec::new(),
        )
        .await
        .unwrap();
        await_run(&state, &run, |r| r.totals.turns >= 1).await;
        {
            let w = state.world.lock().await;
            let r = w.run(&run).unwrap();
            assert_eq!(r.notices.len(), 1, "{:?}", r.notices);
            assert_eq!(r.notices[0].title, "Rate limit close");
            assert_eq!(r.notices[0].severity, "warning");
        }
        devplane::driven::stop(&state, &run).await.unwrap();
        await_run(&state, &run, |r| r.session_closed.is_some()).await;
        let w = state.world.lock().await;
        assert_eq!(w.run(&run).unwrap().session_closed, Some(closed), "{env}");
    }
}

// ── A change continued with a second agent ──────────────────────────────────

/// Driven by agent A, then continued with agent B: both runs are on the
/// change, the switch is the person's, B works in the same directory, and B is
/// told only the person's words — nothing of A's conversation.
#[tokio::test]
async fn a_change_continues_with_a_second_agent() {
    let (Some(a), Some(b)) = (
        echo_with("DEVPLANE_ECHO_WHO=a"),
        echo_with("DEVPLANE_ECHO_WHO=b"),
    ) else {
        return;
    };
    let (addr, c, state) = boot().await;
    let dir = repo("switch");
    trust(&c, &addr, &dir).await;
    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": dir.to_string_lossy(),
            "title": "rate limit",
            "prompt": "first words for agent a",
            "agent": a.command,
            "worktree": false,
        }),
    )
    .await;
    let id = started["change_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{started}"))
        .to_string();
    let first = common::until("A's turn", Duration::from_secs(30), || {
        let state = state.clone();
        let id = id.clone();
        async move {
            let run = state
                .changes
                .lock()
                .await
                .get(&devplane::core::ChangeId::new(&id))?
                .runs
                .first()
                .cloned()?;
            let w = state.world.lock().await;
            (w.run(&run)?.totals.turns >= 1).then_some(run)
        }
    })
    .await;

    // While A is live, the switch is refused and starts nothing.
    let refused = c
        .post(format!("http://{addr}/api/changes/{id}/resume"))
        .bearer_auth("tok")
        .json(&serde_json::json!({ "agent": b.command, "prompt": "carry on" }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 409);
    let said: Value = refused.json().await.unwrap();
    assert!(
        said["error"]
            .as_str()
            .unwrap_or("")
            .contains("still running"),
        "{said}"
    );

    devplane::driven::stop(&state, &first).await.unwrap();
    await_run(&state, &first, |r| !r.state.is_live()).await;
    let before = heard(&dir);

    let switched = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/resume"),
        serde_json::json!({ "agent": b.command, "prompt": "carry on with the limiter" }),
    )
    .await;
    let second = switched["switched"]["run"]
        .as_str()
        .unwrap_or_else(|| panic!("{switched}"));
    assert_eq!(switched["switched"]["was"], a.command);
    let second = RunId::new(second);
    await_run(&state, &second, |r| r.totals.turns >= 1).await;

    let change = state
        .changes
        .lock()
        .await
        .get(&devplane::core::ChangeId::new(&id))
        .cloned()
        .unwrap();
    assert_eq!(
        change.runs,
        vec![first.clone(), second.clone()],
        "both runs stay"
    );
    {
        let w = state.world.lock().await;
        assert_eq!(
            w.run(&second).unwrap().agent_command.as_deref(),
            Some(b.command.as_str())
        );
        assert_eq!(w.run(&second).unwrap().cwd, w.run(&first).unwrap().cwd);
        assert!(
            w.run(&first).unwrap().agent_session.is_some(),
            "A's session stays its own record, resumable with A"
        );
    }
    let rows = devplane::view::decisions(&state.store, Some(&id), 100, false)
        .await
        .unwrap();
    let switch = rows
        .iter()
        .find(|r| r.decision.action == "change:agent.switch")
        .expect("the switch is recorded");
    assert_eq!(switch.decision.authority.as_str(), "person");
    assert!(
        switch.decision.subject.contains('→'),
        "{}",
        switch.decision.subject
    );

    // What B was told: the person's words and the standing sentence, nothing
    // of A's prompt or A's replies.
    let told = heard(&dir)[before.len()..].to_string();
    assert!(told.contains("carry on with the limiter"), "{told}");
    assert!(!told.contains("first words for agent a"), "{told}");
    assert!(!told.contains("echo:"), "{told}");
    let _ = RunState::Working;
}

// ── Sign-in ─────────────────────────────────────────────────────────────────

/// A change whose agent needs signing in shows the agent's methods; the
/// person choosing its agent-handled method starts the change again, and a
/// method the agent did not offer is refused. No credential appears anywhere.
#[tokio::test]
async fn a_change_starts_after_the_person_signs_in() {
    let Some(agent) = echo_with("DEVPLANE_ECHO_NEEDS_AUTH=1") else {
        return;
    };
    let (addr, c, state) = boot().await;
    let dir = repo("signin");
    trust(&c, &addr, &dir).await;
    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": dir.to_string_lossy(),
            "title": "needs sign-in",
            "prompt": "hello after sign-in",
            "agent": agent.command,
            "worktree": false,
        }),
    )
    .await;
    let id = started["change_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{started}"))
        .to_string();
    let first = state
        .changes
        .lock()
        .await
        .get(&devplane::core::ChangeId::new(&id))
        .and_then(|c| c.runs.first().cloned())
        .unwrap();
    await_run(&state, &first, |r| {
        r.sign_in.is_some() && !r.state.is_live()
    })
    .await;
    let methods = state
        .world
        .lock()
        .await
        .run(&first)
        .unwrap()
        .sign_in
        .clone()
        .unwrap();
    assert!(
        methods
            .iter()
            .any(|m| m.id == "echo-login" && m.kind == "agent")
    );
    assert!(
        methods
            .iter()
            .any(|m| m.id == "echo-terminal" && m.kind == "terminal")
    );

    let refused = c
        .post(format!("http://{addr}/api/changes/{id}/login"))
        .bearer_auth("tok")
        .json(&serde_json::json!({ "method": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 409);
    let terminal = c
        .post(format!("http://{addr}/api/changes/{id}/login"))
        .bearer_auth("tok")
        .json(&serde_json::json!({ "method": "echo-terminal" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        terminal.status(),
        409,
        "a terminal sign-in is the person's to run"
    );

    let signed = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/login"),
        serde_json::json!({ "method": "echo-login" }),
    )
    .await;
    let run = RunId::new(signed["run"].as_str().unwrap_or_else(|| panic!("{signed}")));
    await_run(&state, &run, |r| r.totals.turns >= 1).await;
    let told = heard(&dir);
    assert!(told.contains("authenticate echo-login"), "{told}");
    assert!(told.contains("hello after sign-in"), "{told}");
    let w = state.world.lock().await;
    assert!(w.run(&run).unwrap().sign_in.is_none());
    // The methods carry the agent's names and kinds, never a credential.
    let doc = serde_json::to_string(&methods).unwrap();
    for word in ["password", "token", "secret", "credential", "apiKey"] {
        assert!(!doc.contains(word), "{word} in {doc}");
    }
}

// ── The change's tasks through Devplane's MCP server ────────────────────────

/// A host in its own process and home, killed when dropped: the agent
/// launches `devplane mcp` from the host's executable, which finds the host
/// through that home.
struct Spawned {
    child: std::process::Child,
    addr: std::net::SocketAddr,
    token: String,
    home: PathBuf,
    c: reqwest::Client,
}

impl Drop for Spawned {
    fn drop(&mut self) {
        let _ = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("down")
            .env("DEVPLANE_HOME", &self.home)
            .output();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Spawned {
    async fn up(tag: &str) -> Self {
        let home = common::scratch(&format!("home-{tag}"));
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .args(["up", "--no-browser", "--port", "0"])
            .env("DEVPLANE_HOME", &home)
            .env("DEVPLANE_NOTIFY", "0")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("a host");
        let info: Value = common::until("the host's address", Duration::from_secs(15), || {
            let p = home.join("host.json");
            async move {
                std::fs::read_to_string(p)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok())
            }
        })
        .await;
        let port = info["port"].as_u64().expect("a port");
        let token = std::fs::read_to_string(home.join("token"))
            .expect("a token")
            .trim()
            .to_string();
        Self {
            child,
            addr: format!("127.0.0.1:{port}").parse().unwrap(),
            token,
            home,
            c: reqwest::Client::new(),
        }
    }

    async fn post(&self, path: &str, body: Value) -> Value {
        self.c
            .post(format!("http://{}{path}", self.addr))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

/// A driven agent asks Devplane's MCP server about its change without naming
/// it and receives that change's task lines; none of the task text was in the
/// prompt it was sent.
#[tokio::test]
#[cfg(unix)]
async fn a_driven_agent_reads_its_own_change_over_mcp() {
    let Some(agent) = common::echo_agent() else {
        return;
    };
    let host = Spawned::up("mcp-change").await;
    let tasks = "# Tasks\n\n- [ ] T001 Add the zebra-striped rate limiter\n- [ ] T002 Count the okapi requests\n";
    let dir = common::scratch_repo_with(
        "mcp-change",
        common::PASSING,
        &[(".gitignore", ".devplane/\n")],
    );
    std::fs::create_dir_all(dir.join("specs/001-limit")).unwrap();
    std::fs::write(
        dir.join("specs/001-limit/spec.md"),
        "# Limit\n\nRequests are limited.\n",
    )
    .unwrap();
    std::fs::write(dir.join("specs/001-limit/tasks.md"), tasks).unwrap();
    common::git(&dir, &["add", "-A"]);
    common::git(&dir, &["commit", "-qm", "spec"]);
    common::trust_from_a_terminal(&host.home, &dir);
    let v = host
        .post(
            "/api/changes",
            serde_json::json!({
                "agent": agent,
                "cwd": dir.to_string_lossy(),
                "title": "Limit requests",
                "prompt": "ask-mcp",
                "spec": "specs/001-limit",
                "worktree": false,
            }),
        )
        .await;
    let id = v["change"]["id"]
        .as_str()
        .or(v["change_id"].as_str())
        .unwrap_or_else(|| panic!("{v}"))
        .to_string();
    let answer = common::until("the agent's MCP answer", Duration::from_secs(60), || {
        let p = dir.join(".devplane/mcp-answer.txt");
        async move { std::fs::read_to_string(p).ok().filter(|t| !t.is_empty()) }
    })
    .await;
    assert!(!answer.starts_with("refused"), "{answer}");
    assert!(
        answer.contains(&id),
        "the answer is about its own change: {answer}"
    );
    assert!(
        answer.contains("zebra-striped rate limiter") && answer.contains("okapi requests"),
        "the change's tasks reached the agent: {answer}"
    );
    let prompt = heard(&dir);
    assert!(
        !prompt.contains("zebra"),
        "no task text was in the prompt: {prompt}"
    );
}

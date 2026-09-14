//! The verified-done loop, against a real repository.
//!
//! This is the product's central claim, so the test does the real thing: a git
//! repository with a committed `vibeplane.toml`, a gate command that genuinely
//! fails, a worktree, and an agent that claims success without earning it.

mod common;

use serde_json::Value;
use std::path::{Path, PathBuf};
use vibeplane::core::Policy;
use vibeplane::daemon::{AppState, Shared};

/// A repository whose gate fails until `fixed.txt` exists.
fn scratch_repo(tag: &str, on_fail: &str, rounds: u32) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-work-{tag}-{}-{}",
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
    std::fs::write(
        dir.join("vibeplane.toml"),
        format!(
            r#"
[gates]
check = ["sh ./check.sh"]
timeout = "30s"
on_fail = "{on_fail}"
max_feedback_rounds = {rounds}
"#
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("check.sh"),
        "#!/bin/sh\nif [ -f fixed.txt ]; then exit 0; fi\necho 'test auth::login ... FAILED'\nexit 1\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    // macOS puts the temp directory behind a symlink, and the daemon
    // canonicalises paths; comparing against the un-resolved one fails for
    // reasons that have nothing to do with the code under test.
    dir.canonicalize().unwrap()
}

/// A repository whose `vibeplane.toml` declares a pipeline.
fn pipeline_repo(tag: &str, pipeline: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-pipe-{tag}-{}-{}",
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
    std::fs::write(dir.join("vibeplane.toml"), pipeline).unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

fn echo_agent() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?.parent()?.join("examples").join("echo_agent");
    bin.exists().then(|| bin.to_string_lossy().to_string())
}

/// A daemon for one test, which takes its agents with it.
///
/// An abandoned ACP session is a real product bug — the agent's process group
/// outlives the thing that started it, which is what
/// `shutting_down_takes_the_agents_with_it` covers. In *this binary* it is also
/// what made the file flaky: `#[tokio::test]` gives every test its own runtime
/// while the process shares one async-io reactor, and an agent left holding
/// pipes on it starves the sessions a later test opens. Stopping on drop makes
/// a test that panics as clean as one that passes.
struct Daemon(Shared, #[allow(dead_code)] common::AgentSerial);
impl std::ops::Deref for Daemon {
    type Target = Shared;
    fn deref(&self) -> &Shared {
        &self.0
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        for _ in 0..100 {
            if let Ok(mut map) = self.0.sessions.try_lock() {
                for (_, session) in map.drain() {
                    session.stop();
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

async fn boot() -> (std::net::SocketAddr, reqwest::Client, Daemon) {
    let serial = common::one_agent_at_a_time();
    let db = std::env::temp_dir().join(format!("vp-work-{}.db", uuid::Uuid::new_v4().simple()));
    let state = AppState::new(
        db.clone(),
        "tok".into(),
        Policy::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();
    let app = vibeplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new(), Daemon(state, serial))
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

/// Waits for a work item to settle, rather than sleeping a fixed time.
///
/// When it gives up it says what the work was actually doing. A bare "never
/// reached ["review"]" sends whoever reads it to the logs; the phase, the runs,
/// the gates and the decision log are what actually identify a stall.
/// How long that wait is allowed to take.
///
/// Generous on purpose. Every one of these tests drives a *real* agent process
/// and runs the project's gates in a real repository, and `cargo test` runs the
/// test binaries in parallel — so on a loaded CI runner the honest answer is
/// often "slower than a laptop", and a budget tuned to a laptop turns that into
/// a red build that says nothing. It only costs time when something is already
/// broken: the wait returns the moment the phase arrives.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(90);
const POLL: std::time::Duration = std::time::Duration::from_millis(100);

async fn await_phase(c: &reqwest::Client, addr: &std::net::SocketAddr, want: &[&str]) -> Value {
    let mut last = Value::Null;
    let deadline = std::time::Instant::now() + SETTLE;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(POLL).await;
        let list: Value = c
            .get(format!("http://{addr}/api/work"))
            .bearer_auth("tok")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if let Some(w) = list.as_array().and_then(|a| a.first()) {
            last = w.clone();
            if want.contains(&w["phase"].as_str().unwrap_or("")) {
                return w.clone();
            }
        }
    }
    let audit: Value = c
        .get(format!("http://{addr}/api/decisions"))
        .bearer_auth("tok")
        .send()
        .await
        .map(|r| r.json())
        .unwrap()
        .await
        .unwrap_or(Value::Null);
    panic!(
        "work never reached {want:?}\n  phase:     {}\n  runs:      {}\n  pipeline:  {}\n           gates:     {}\n  decisions: {}",
        last["phase"],
        last["runs"],
        last["pipeline"],
        last["gates"]
            .as_array()
            .map(|g| g
                .iter()
                .map(|x| x["gate"].to_string())
                .collect::<Vec<_>>()
                .join(","))
            .unwrap_or_default(),
        audit
            .as_array()
            .map(|d| d
                .iter()
                .map(|x| format!("{}/{}", x["action"], x["outcome"]))
                .collect::<Vec<_>>()
                .join(" "))
            .unwrap_or_default(),
    );
}

#[tokio::test]
async fn an_untrusted_project_will_not_host_an_agent() {
    // A headless agent runs the repository's own hooks and MCP servers without
    // asking. Somebody has to have decided that this directory is theirs.
    let repo = scratch_repo("trust", "feedback", 1);
    let (addr, c, _state) = boot().await;

    let res = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({ "cwd": repo.to_string_lossy(), "title": "do a thing" }),
    )
    .await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("not trusted"), "got {err:?}");
    assert!(err.contains("vibeplane trust"), "and says how to fix it");

    // Nothing was created on the way to refusing.
    assert!(!repo.join(".claude/worktrees").exists());
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_claim_of_done_that_fails_the_gates_does_not_become_done() {
    // The product's central claim, end to end: the agent says it finished, the
    // project's own check disagrees, and the work is not marked done.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("verify", "feedback", 1);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "fix the flaky login test",
            "kind": "bug",
            "agent": agent,
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");

    let work = started["work"].clone();
    let worktree = PathBuf::from(work["worktree"].as_str().expect("an isolated checkout"));
    assert!(worktree.exists(), "the checkout is real");
    assert!(
        worktree.starts_with(repo.join(".claude/worktrees")),
        "and lives where Claude Code puts its own"
    );
    assert!(work["branch"].as_str().unwrap().starts_with("fix/"));

    let settled = await_phase(&c, &addr, &["failed", "review"]).await;
    assert_eq!(
        settled["phase"], "failed",
        "an unearned claim of done must not reach review"
    );

    // It tried, was told what was wrong, and tried again before giving up.
    assert_eq!(settled["feedback_rounds"], 1);
    let gates = settled["gates"].as_array().unwrap();
    assert_eq!(gates.len(), 2, "one attempt, one retry after feedback");
    assert!(
        gates[0]["commands"][0]["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap_or("").contains("auth::login")),
        "the failure the human sees is the one the runner printed"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn fixing_the_cause_lets_the_work_pass() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("pass", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "make it pass",
            "agent": agent,
        }),
    )
    .await;
    let id = started["work_id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(started["work"]["worktree"].as_str().unwrap());
    await_phase(&c, &addr, &["failed"]).await;

    // What the agent should have done.
    std::fs::write(worktree.join("fixed.txt"), "").unwrap();

    let verdict = post(&c, &addr, &format!("/api/work/{id}/verify"), Value::Null).await;
    assert_eq!(verdict["passed"], true, "{verdict}");
    let w = await_phase(&c, &addr, &["review"]).await;
    assert_eq!(
        w["phase"], "review",
        "green gates means a human should look"
    );

    // Finishing cleans up after itself and leaves git tidy.
    post(
        &c,
        &addr,
        &format!("/api/work/{id}/finish?remove_worktree=true&force=true"),
        Value::Null,
    )
    .await;
    assert!(!worktree.exists(), "the checkout is gone");
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_project_with_no_gates_asks_a_human_rather_than_claiming_success() {
    // No Definition of Done means nothing was verified, and the work must not
    // pretend otherwise.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("nogates", "feedback", 1);
    // Committed, not merely written: a worktree is a checkout of a branch, so
    // the definition of done that governs it is the one in the commit. That is
    // the right behaviour — a project's checks should not change because
    // somebody has an unsaved edit in another window.
    std::fs::write(repo.join("vibeplane.toml"), "[project]\nname = \"x\"\n").unwrap();
    for args in [vec!["add", "-A"], vec!["commit", "-qm", "drop the gates"]] {
        std::process::Command::new("git")
            .args(&args)
            .current_dir(&repo)
            .output()
            .unwrap();
    }
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "unverifiable",
            "agent": agent,
        }),
    )
    .await;

    let w = await_phase(&c, &addr, &["review", "failed", "done"]).await;
    assert_eq!(w["phase"], "review");
    assert!(
        w["gates"].as_array().unwrap().is_empty(),
        "and no gate report pretends one ran"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_broken_config_stops_the_work_rather_than_being_ignored() {
    // A typo in a deny rule that silently disappears is the worst way for a
    // policy to fail.
    let repo = scratch_repo("badconfig", "feedback", 1);
    // The main checkout's copy is what `work start` reads, before any worktree
    // exists — so this one does not need committing.
    std::fs::write(repo.join("vibeplane.toml"), "[gates]\nchekc = [\"true\"]\n").unwrap();
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let res = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({ "cwd": repo.to_string_lossy(), "title": "x" }),
    )
    .await;
    assert!(
        res["error"]
            .as_str()
            .unwrap_or("")
            .contains("vibeplane.toml"),
        "the error names the file: {res}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn dispatch_cannot_be_used_to_skip_the_trust_gate() {
    // A check one caller can skip is not a check. `work start` enforced it and
    // `dispatch` did not, so an agent could be started anywhere by choosing the
    // other command.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("bypass", "feedback", 1);
    let (addr, c, _state) = boot().await;

    let res = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({ "agent": agent, "cwd": repo.to_string_lossy() }),
    )
    .await;
    assert!(
        res["error"].as_str().unwrap_or("").contains("not trusted"),
        "dispatch must refuse an untrusted repository too: {res}"
    );

    // And works once the decision has been made.
    trust(&c, &addr, &repo).await;
    let ok = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({ "agent": agent, "cwd": repo.to_string_lossy() }),
    )
    .await;
    assert!(ok["run_id"].is_string(), "{ok}");
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn every_verdict_is_accountable_afterwards() {
    // The two questions the event log cannot answer: why did that command run
    // without anybody being asked, and why is there a pull request on this
    // branch. "Auto-approved" is not an answer; the rule that did it is.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("audit", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "fix the flaky login test",
            "agent": agent,
        }),
    )
    .await;
    await_phase(&c, &addr, &["failed", "review"]).await;

    let log: Value = c
        .get(format!("http://{addr}/api/decisions"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rows = log.as_array().expect("a decision log");
    let gate = rows
        .iter()
        .find(|d| d["action"] == "gate:run")
        .expect("the gate verdict is the decision that made this work fail");
    assert_eq!(gate["outcome"], "fail");
    assert_eq!(gate["actor"], "daemon");
    assert!(
        gate["subject"].as_str().unwrap().contains("check.sh"),
        "the log names the commands that decided: {gate}"
    );
    assert!(
        gate["reason"].as_str().unwrap().contains("failed"),
        "and why: {gate}"
    );
    assert!(gate["work_id"].is_string(), "attributed to the work");

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_spent_feedback_budget_actually_asks_somebody() {
    // The README's promise, and for a long time nobody kept it: "red means the
    // failures go back to that same session, bounded, and then you are asked".
    // The work reached `failed` and sat there. `work list` showed it; the inbox
    // did not, and the inbox is the thing people read.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("asked", "feedback", 1);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "fix the flaky login test",
            "agent": agent,
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let work_id = started["work_id"].as_str().unwrap().to_string();
    let settled = await_phase(&c, &addr, &["failed", "review"]).await;
    assert_eq!(settled["phase"], "failed");

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "gate_failed")
        .expect("a spent budget has to reach the person whose turn it now is");

    assert_eq!(item["level"], "high");
    assert_eq!(item["work_id"], work_id.as_str());
    assert!(
        item["detail"].as_str().unwrap().contains("auth::login"),
        "the person and the agent must see the same failures: {item}"
    );

    // And the offer of one more round is real: the session that wrote the code
    // is still there, so the button is not a button that lies.
    assert!(
        item["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "retry"),
        "{item}"
    );
    let retried = post(
        &c,
        &addr,
        &format!("/api/work/{work_id}/retry"),
        serde_json::json!({}),
    )
    .await;
    assert!(retried["error"].is_null(), "{retried}");

    // It lands back with the agent, counts against the budget, and is recorded
    // as a person's decision rather than the machine's.
    let after = await_phase(&c, &addr, &["failed", "review"]).await;
    assert_eq!(
        after["feedback_rounds"].as_u64().unwrap(),
        2,
        "a human round still counts, so the next automatic one respects the budget"
    );
    let log: Value = c
        .get(format!("http://{addr}/api/decisions"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let retry = log
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "work:retry")
        .expect("going past the project's bound is a decision somebody made");
    assert_eq!(retry["actor"], "human");
    assert!(
        retry["reason"]
            .as_str()
            .unwrap()
            .contains("past the project")
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_pipeline_does_not_count_against_itself() {
    // `max_parallel_runs = 2` and a three-step pipeline are both in the
    // README's own example. A step that left its agent alive would have the
    // limit refuse step three — a cap on *concurrent work* read as a cap on the
    // length of a chain.
    let Some(agent) = echo_agent() else { return };
    let repo = pipeline_repo(
        "cap",
        &format!(
            r#"
[policy]
max_parallel_runs = 1

[gates]
check = ["true"]

[pipelines.feature]
steps = [
  {{ role = "implement", agent = "{agent}", prompt = "write the thing", gate = "check" }},
  {{ role = "review",    agent = "{agent}", prompt = "read the thing" }},
  {{ human = "merge" }},
]
"#
        ),
    );
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
            "kind": "feature",
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");

    let held = await_phase(&c, &addr, &["human", "failed"]).await;
    assert_eq!(
        held["phase"], "human",
        "a chain of three steps must not be stopped by a limit of one concurrent run"
    );
    assert_eq!(held["runs"].as_array().unwrap().len(), 2);

    // And a finished step does not leave an agent behind: the whole chain is
    // one live session at a time, not one per step.
    assert!(
        state.sessions.lock().await.len() <= 1,
        "a pipeline left {} agents running",
        state.sessions.lock().await.len()
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_ceiling_stops_work_that_gets_expensive() {
    // The count-based bounds stop an agent arguing with a test suite for ever.
    // They do nothing about one turn going a long way on its own, which is the
    // other way an afternoon becomes expensive. The fixture reports a real
    // usage update, because a ceiling tested against invented numbers proves
    // only that the arithmetic works.
    let Some(agent) = echo_agent() else { return };
    let repo = pipeline_repo(
        "budget",
        &format!(
            r#"
[budget]
default_usd = 10

[gates]
check = ["true"]

[pipelines.feature]
steps = [
  {{ role = "implement", agent = "{agent}", prompt = "do something expensive", gate = "check" }},
  {{ role = "review",    agent = "{agent}", prompt = "read the thing" }},
  {{ human = "merge" }},
]
"#
        ),
    );
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
            "kind": "feature",
        }),
    )
    .await;

    let stopped = await_phase(&c, &addr, &["failed", "human", "review"]).await;
    assert_eq!(
        stopped["phase"], "failed",
        "$100 against a $10 ceiling must not reach the next step: {stopped}"
    );
    // Why it stopped is *recorded*, not inferred from the last gate report —
    // the checks were green here, so anything that guessed would call a passing
    // gate the failure.
    assert_eq!(stopped["stopped"]["reason"], "over_budget", "{stopped}");
    assert!(
        stopped["stopped"]["spent_usd"].as_f64().unwrap() >= 100.0,
        "{stopped}"
    );
    assert_eq!(
        stopped["runs"].as_array().unwrap().len(),
        1,
        "and the chain stopped rather than paying for the review step too"
    );

    // It asks about money, not about correctness — the checks were green.
    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "cost_spike")
        .expect("a ceiling nobody is told about is not a ceiling");
    assert!(item["title"].as_str().unwrap().contains("$"), "{item}");
    assert!(
        !inbox
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "gate_failed"),
        "the checks passed; blaming them would send somebody after the wrong thing"
    );

    let log: Value = c
        .get(format!("http://{addr}/api/decisions"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let stop = log
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "work:stop")
        .expect("stopping something costs a reason");
    assert!(stop["reason"].as_str().unwrap().contains("ceiling"));

    // Handing it back would buy one more turn and stop at the same place with
    // more spent, so the offer is not made and the command says what to change.
    assert_eq!(stopped["can_retry"], false);
    let refused = post(
        &c,
        &addr,
        &format!("/api/work/{}/retry", stopped["id"].as_str().unwrap()),
        serde_json::json!({}),
    )
    .await;
    assert!(
        refused["error"].as_str().unwrap_or("").contains("Raise it"),
        "{refused}"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_driven_run_keeps_the_conversation_it_is_the_only_window_for() {
    // A session somebody started in a terminal already has a window showing its
    // transcript, and Vibeplane's honest action for it is `focus`. A run
    // Vibeplane *drives* has no other window: without this, `dispatch` can say
    // that a tool ran and not one word about why.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("said", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({
            "agent": agent,
            "cwd": repo.to_string_lossy(),
            "prompt": "use a tool and tell me about it",
        }),
    )
    .await;
    let run = started["run_id"].as_str().expect("a run").to_string();

    // Wait for the turn rather than for a clock.
    let mut said: Vec<Value> = Vec::new();
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        said = c
            .get(format!("http://{addr}/api/runs/{run}/messages"))
            .bearer_auth("tok")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if said.len() >= 2 {
            break;
        }
    }

    // It starts with the ask. A transcript that begins with the answer is half
    // a conversation, and the prompt is the one thing Vibeplane always knows.
    assert_eq!(said[0]["role"], "user", "{said:?}");
    assert!(said[0]["text"].as_str().unwrap().contains("use a tool"));

    let answered = said
        .iter()
        .find(|m| m["role"] == "agent")
        .expect("what the agent said");
    assert!(
        answered["text"].as_str().unwrap().contains("echo:"),
        "{answered}"
    );

    // Chunks are joined, not stored one row per syllable.
    assert!(
        said.len() <= 4,
        "a short turn is not a database of syllables: {said:?}"
    );

    // And it stays out of the event log, which run state is a reduction over.
    let events: Value = c
        .get(format!("http://{addr}/api/runs/{run}/events"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        !events.as_array().unwrap().iter().any(|e| e["event"]["type"]
            .as_str()
            .unwrap_or("")
            .contains("message")),
        "a sentence changes no state and must not be in the log that state reduces over"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_repository_can_refuse_to_keep_what_its_agents_say() {
    // An agent's prose can quote something it read, and `[workspace] include`
    // deliberately copies `.env` into the worktree it works in. Keeping the
    // conversation is the default because a driven run has no other window, but
    // it stays the repository's decision.
    let Some(agent) = echo_agent() else { return };
    let repo = pipeline_repo("quiet", "[transcripts]\nkeep = false\n");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({
            "agent": agent,
            "cwd": repo.to_string_lossy(),
            "prompt": "say something quotable",
        }),
    )
    .await;
    let run = started["run_id"].as_str().expect("a run").to_string();
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;

    let said: Vec<Value> = c
        .get(format!("http://{addr}/api/runs/{run}/messages"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        said.is_empty(),
        "nothing may be written down, including the prompt: {said:?}"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn shutting_down_takes_the_agents_with_it() {
    // The daemon exits, its tasks are never dropped, the protocol connections
    // are never torn down — and every agent it started re-parents to init and
    // keeps running. A model with a subscription attached, spending, with
    // nothing left on the machine that knows it is there.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("reap", "feedback", 1);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({ "agent": agent, "cwd": repo.to_string_lossy() }),
    )
    .await;
    assert!(started["run_id"].is_string(), "{started}");
    assert_eq!(state.sessions.lock().await.len(), 1);

    state.shutdown().await;
    assert!(
        state.sessions.lock().await.is_empty(),
        "an agent survived the daemon that started it"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_config_that_cannot_work_is_refused_before_anything_is_spent() {
    // A `back_to` naming nothing, a gate nobody declared, a review loop with no
    // check behind it — each of which would otherwise surface as a chain that
    // fell over several minutes and one model call later.
    let repo = pipeline_repo(
        "invalid",
        r#"
[pipelines.feature]
steps = [
  { role = "implement", prompt = "implement" },
  { role = "review", prompt = "review", findings = { back_to = "implement" } },
]
"#,
    );
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let res = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
            "kind": "feature",
        }),
    )
    .await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("declares no `gate`"), "got {err:?}");
    // And nothing was created on the way to refusing.
    assert!(!repo.join(".claude/worktrees").exists());
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_worktree_inherits_the_trust_of_its_repository() {
    // Asking again for every checkout of a project somebody already trusted
    // teaches people to say yes without reading.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("inherit", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "in a worktree",
            "agent": agent,
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let worktree = started["work"]["worktree"].as_str().unwrap();

    let again = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({ "agent": agent, "cwd": worktree }),
    )
    .await;
    assert!(
        again["run_id"].is_string(),
        "the worktree is trusted too: {again}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn snoozing_work_actually_quietens_it() {
    // Every item Work produces offers `snooze`, and the only snooze endpoint
    // was the run's — so for the items that outlive their session, which is
    // most of them, the button posted to `/api/runs//snooze` and reached
    // nothing. The inbox is the product; an action on it that silently does
    // nothing is the one failure a control plane cannot afford.
    let repo = scratch_repo("snoozework", "escalate", 0);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let mut work = vibeplane::core::Work::new(
        vibeplane::core::ProjectId::from_path(&repo),
        vibeplane::core::WorkKind::Bug,
        "flaky login test".into(),
        "fix it".into(),
    );
    work.phase = vibeplane::core::Phase::Review;
    work.pull_request = Some(vibeplane::core::work::PullRequestRef {
        number: 7,
        url: "https://github.com/acme/app/pull/7".into(),
        status: "failing".into(),
        failing_checks: vec!["test".into()],
    });
    let id = work.id.clone();
    // No run has ever touched it, which is exactly the case the run route
    // cannot serve.
    assert!(work.current_run().is_none());
    state.works.lock().await.insert(id.clone(), work);

    let inbox_kinds = |c: reqwest::Client, addr: std::net::SocketAddr| async move {
        let v: Value = c
            .get(format!("http://{addr}/api/inbox"))
            .bearer_auth("tok")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        v.as_array()
            .unwrap()
            .iter()
            .map(|i| i["kind"].as_str().unwrap_or("").to_string())
            .collect::<Vec<_>>()
    };

    let before = inbox_kinds(c.clone(), addr).await;
    assert!(before.contains(&"ci_red".to_string()), "{before:?}");

    let snoozed = post(
        &c,
        &addr,
        &format!("/api/work/{}/snooze?minutes=60", id.as_str()),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(snoozed["ok"], true, "{snoozed}");

    let after = inbox_kinds(c.clone(), addr).await;
    assert!(
        !after.contains(&"ci_red".to_string()),
        "a snoozed piece of work must leave the queue: {after:?}"
    );

    // And it comes back, because snoozing is "not now", not "never".
    let woken = post(
        &c,
        &addr,
        &format!("/api/work/{}/snooze?minutes=0", id.as_str()),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(woken["ok"], true, "{woken}");
    let again = inbox_kinds(c, addr).await;
    assert!(again.contains(&"ci_red".to_string()), "{again:?}");

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_red_pull_request_reaches_the_inbox_after_the_agent_is_gone() {
    // The clearest reason Work is the durable unit: checks finish long after
    // the session that wrote the code has ended, and somebody still has to be
    // told the build went red.
    let repo = scratch_repo("prinbox", "escalate", 0);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    // A finished piece of work with a pull request, and no live session at all.
    let mut work = vibeplane::core::Work::new(
        vibeplane::core::ProjectId::from_path(&repo),
        vibeplane::core::WorkKind::Bug,
        "fix the flaky login test".into(),
        "fix it".into(),
    );
    work.phase = vibeplane::core::Phase::Review;
    work.pull_request = Some(vibeplane::core::work::PullRequestRef {
        number: 142,
        url: "https://github.com/acme/app/pull/142".into(),
        status: "failing".into(),
        failing_checks: vec!["test".into()],
    });
    let id = work.id.clone();
    state.works.lock().await.insert(id.clone(), work);

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "ci_red")
        .expect("a red pull request needs a human");
    assert!(item["title"].as_str().unwrap().contains("#142"));
    assert!(item["detail"].as_str().unwrap().contains("test"));
    assert_eq!(
        item["level"], "high",
        "red is the state where waiting is wrong"
    );
    assert!(
        item["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "open_pr"),
        "and the obvious action is to go and look at it"
    );
    assert!(
        item["url"].as_str().unwrap_or("").contains("/pull/142"),
        "an action that offers to open something must say what: {item}"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn an_approved_and_green_pull_request_asks_for_nothing() {
    // Nobody is being asked for anything, so it does not belong in a queue of
    // decisions. An inbox that lists finished work stops being read.
    let repo = scratch_repo("prquiet", "escalate", 0);
    let (addr, c, state) = boot().await;
    let mut work = vibeplane::core::Work::new(
        vibeplane::core::ProjectId::from_path(&repo),
        vibeplane::core::WorkKind::Quick,
        "done and dusted".into(),
        "x".into(),
    );
    work.pull_request = Some(vibeplane::core::work::PullRequestRef {
        number: 9,
        url: "u".into(),
        status: "ready_to_merge".into(),
        failing_checks: vec![],
    });
    state.works.lock().await.insert(work.id.clone(), work);

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        inbox.as_array().unwrap().is_empty(),
        "nothing to decide: {inbox}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_projects_own_rules_decide_its_agents() {
    // `[policy]` in vibeplane.toml was parsed and then ignored, which is worse
    // than not offering it: a rule someone wrote did nothing at all.
    let repo = scratch_repo("policy", "escalate", 0);
    std::fs::write(
        repo.join("vibeplane.toml"),
        "[policy]\nauto_allow = [\"Bash(echo *)\"]\nnever_auto = [\"Bash(rm -rf *)\"]\n",
    )
    .unwrap();
    let (addr, c, _state) = boot().await;

    let ask = |command: &str| {
        let body = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "s1",
            "cwd": repo.to_string_lossy(),
            "tool_name": "Bash",
            "tool_input": { "command": command }
        });
        let c = c.clone();
        async move {
            c.post(format!("http://{addr}/vibeplane/policy"))
                .bearer_auth("tok")
                .json(&body)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        }
    };

    let allowed: Value = serde_json::from_str(&ask("echo hello").await).unwrap();
    assert_eq!(
        allowed["hookSpecificOutput"]["decision"]["behavior"], "allow",
        "the project's own allow rule must answer"
    );

    let denied: Value = serde_json::from_str(&ask("rm -rf node_modules").await).unwrap();
    assert_eq!(denied["hookSpecificOutput"]["decision"]["behavior"], "deny");

    let neither = ask("curl example.com").await;
    assert_eq!(neither, "{}", "and anything else still reaches the human");

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_projects_parallelism_limit_is_enforced() {
    // Five agents in one repository is rarely five times the work: they collide
    // on the same files and each one costs money regardless.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("parallel", "escalate", 0);
    std::fs::write(
        repo.join("vibeplane.toml"),
        "[policy]\nmax_parallel_runs = 1\n",
    )
    .unwrap();
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let first = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({ "agent": agent, "cwd": repo.to_string_lossy() }),
    )
    .await;
    assert!(first["run_id"].is_string(), "{first}");

    let second = post(
        &c,
        &addr,
        "/api/dispatch",
        serde_json::json!({ "agent": agent, "cwd": repo.to_string_lossy() }),
    )
    .await;
    let err = second["error"].as_str().unwrap_or_default();
    assert!(err.contains("allows 1"), "{second}");
    assert!(
        err.contains("max_parallel_runs"),
        "and says how to change it"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn work_stranded_by_a_restart_says_so() {
    // The Work comes back from the store; the agent process does not. Nothing
    // will move it on its own, so it must not sit there looking busy for ever.
    let repo = scratch_repo("stranded", "escalate", 0);
    let (addr, c, state) = boot().await;

    let mut work = vibeplane::core::Work::new(
        vibeplane::core::ProjectId::from_path(&repo),
        vibeplane::core::WorkKind::Quick,
        "half-finished".into(),
        "x".into(),
    );
    work.phase = vibeplane::core::Phase::Implement;
    work.runs.push(vibeplane::core::RunId::new("gone"));
    state.works.lock().await.insert(work.id.clone(), work);

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "interrupted")
        .expect("interrupted work must be visible");
    assert!(item["title"].as_str().unwrap().contains("half-finished"));
    assert!(
        item["detail"].as_str().unwrap().contains("untouched"),
        "and says the work itself is safe"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_restart_does_not_lose_the_conversation_the_work_was_having() {
    // The largest functional gap this product had: "Work outlives the sessions
    // that do it" was true of the row and false of the agent. A daemon restart
    // took every agent process with it, and the only thing the inbox could
    // offer was to start the work again from nothing — paying a second time to
    // rediscover what the first session already knew, and doing it without the
    // context that produced the code in the worktree.
    //
    // What makes this checkable is the fixture's turn counter: it persists its
    // sessions, the way a real resumable agent does, so a resumed session says
    // `turn 2` and a session that was quietly started again says `turn 1`.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("resume", "escalate", 0);
    let db = std::env::temp_dir().join(format!("vp-resume-{}.db", uuid::Uuid::new_v4().simple()));

    let _serial = common::one_agent_at_a_time();

    let (work_id, agent_session) = {
        let state = AppState::new(
            db.clone(),
            "tok".into(),
            Policy::default(),
            db.parent().unwrap().to_path_buf(),
        )
        .await
        .unwrap();
        let app = vibeplane::api::router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        let c = reqwest::Client::new();
        trust(&c, &addr, &repo).await;

        let run = post(
            &c,
            &addr,
            "/api/dispatch",
            serde_json::json!({
                "agent": agent,
                "cwd": repo.to_string_lossy(),
                "prompt": "start the work",
            }),
        )
        .await;
        let run_id = vibeplane::core::RunId::new(run["run_id"].as_str().unwrap());

        // Wait for the handshake: the agent's own session id is what resume
        // needs, and it only exists once the agent has answered.
        let mut named = None;
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(s) = state
                .world
                .lock()
                .await
                .run(&run_id)
                .and_then(|r| r.agent_session.clone())
            {
                named = Some(s);
                break;
            }
        }
        let agent_session = named.expect("the agent named its session, and we recorded it");

        let mut work = vibeplane::core::Work::new(
            vibeplane::core::ProjectId::from_path(&repo),
            vibeplane::core::WorkKind::Quick,
            "half-finished".into(),
            "x".into(),
        );
        work.phase = vibeplane::core::Phase::Implement;
        work.runs.push(run_id.clone());
        let id = work.id.clone();
        state.store.save_work(&work).await.unwrap();
        state.works.lock().await.insert(id.clone(), work);

        // A crash: the store's last word is a run that was working.
        let mut snapshot = state
            .world
            .lock()
            .await
            .run(&run_id)
            .expect("the run exists")
            .clone();
        snapshot.state = vibeplane::core::RunState::Working;
        state.shutdown().await;
        state.store.save_run(&snapshot).await.unwrap();
        server.abort();
        (id, agent_session)
    };

    // A new daemon over the same database, with every agent process gone.
    let state = AppState::new(
        db.clone(),
        "tok".into(),
        Policy::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();
    let app = vibeplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    let c = reqwest::Client::new();
    assert!(
        state.drivable_runs().await.is_empty(),
        "no agent survived the restart"
    );
    // The id that makes continuing possible came back with the run.
    assert_eq!(
        state
            .world
            .lock()
            .await
            .runs()
            .find_map(|r| r.agent_session.clone()),
        Some(agent_session),
        "the agent's own session id has to survive the restart, or there is \
         nothing to resume against"
    );

    // The inbox offers it, rather than only describing the problem.
    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "interrupted")
        .expect("interrupted work is in the inbox")
        .clone();
    assert!(
        item["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "resume"),
        "an item that can be rescued must offer the rescue: {item}"
    );

    let resumed = post(
        &c,
        &addr,
        &format!("/api/work/{}/resume", work_id.as_str()),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(resumed["ok"], true, "{resumed}");

    // And it is the *same* conversation. A new session would say `turn 1`.
    let run = {
        let works = state.works.lock().await;
        works.get(&work_id).unwrap().current_run().cloned().unwrap()
    };
    post(
        &c,
        &addr,
        &format!("/api/runs/{}/prompt", run.as_str()),
        serde_json::json!({ "text": "carry on" }),
    )
    .await;

    let mut said = String::new();
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let msgs: Value = c
            .get(format!("http://{addr}/api/runs/{}/messages", run.as_str()))
            .bearer_auth("tok")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        said = msgs
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|m| m["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if said.contains("turn ") {
            break;
        }
    }
    assert!(
        said.contains("turn 2"),
        "the agent continued the session it already had; a silent restart would \
         say `turn 1`. It said: {said}"
    );

    // The resumed agent is a real process; this test started it, so this test
    // ends it. Leaving it would starve the reactor for the rest of the binary —
    // which is the whole reason these tests hold `ONE_RUNTIME_AT_A_TIME`.
    state.shutdown().await;
    server.abort();
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_restart_does_not_leave_a_run_looking_drivable() {
    // The case `work_stranded_by_a_restart_says_so` misses, and the one that
    // actually happens. A run row comes back from the store looking perfectly
    // alive — an ACP run has no pid, so reconciliation rightly leaves it alone —
    // while the process behind it is gone. Deciding "can this be driven" from
    // the run's *state* meant interrupted work never said so and sat on the
    // board looking busy for ever, and the inbox offered to hand failures back
    // to an agent that no longer existed.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("restart", "escalate", 0);
    let db = std::env::temp_dir().join(format!("vp-restart-{}.db", uuid::Uuid::new_v4().simple()));

    let _serial = common::one_agent_at_a_time();
    let work_id = {
        let state = AppState::new(
            db.clone(),
            "tok".into(),
            Policy::default(),
            db.parent().unwrap().to_path_buf(),
        )
        .await
        .unwrap();
        let app = vibeplane::api::router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        let c = reqwest::Client::new();
        trust(&c, &addr, &repo).await;

        // A real agent, with no prompt, so it stays exactly where it is rather
        // than racing a gate to the finish.
        let run = post(
            &c,
            &addr,
            "/api/dispatch",
            serde_json::json!({ "agent": agent, "cwd": repo.to_string_lossy() }),
        )
        .await;
        let run_id = vibeplane::core::RunId::new(run["run_id"].as_str().unwrap());

        // Work that is mid-flight in it.
        let mut work = vibeplane::core::Work::new(
            vibeplane::core::ProjectId::from_path(&repo),
            vibeplane::core::WorkKind::Quick,
            "half-finished".into(),
            "x".into(),
        );
        work.phase = vibeplane::core::Phase::Implement;
        work.runs.push(run_id.clone());
        let id = work.id.clone();
        state.store.save_work(&work).await.unwrap();
        state.works.lock().await.insert(id.clone(), work);

        // A crash, not a clean stop: the last thing the store saw is a run that
        // was *working*. A graceful shutdown would record the session ending and
        // the restored row would look finished — which is the case that was
        // already handled, and not the one that bites.
        let mut snapshot = state
            .world
            .lock()
            .await
            .run(&run_id)
            .expect("the run exists")
            .clone();
        snapshot.state = vibeplane::core::RunState::Working;
        state.shutdown().await;
        state.store.save_run(&snapshot).await.unwrap();
        server.abort();
        id.to_string()
    };

    // A new daemon over the same database: this is the restart.
    let state = AppState::new(
        db.clone(),
        "tok".into(),
        Policy::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();
    let app = vibeplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    let c = reqwest::Client::new();

    // The restored run really does look alive. That is the situation, not a
    // hypothetical, and it is why run state is the wrong thing to ask.
    assert!(
        state.world.lock().await.runs().any(|r| r.state.is_live()),
        "the crash left a live-looking run behind, which is the whole point"
    );
    assert!(
        state.drivable_runs().await.is_empty(),
        "and no session survived it"
    );

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = inbox.as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|i| i["kind"] == "interrupted" && i["work_id"] == work_id.as_str()),
        "work whose agent died with the daemon must say so rather than look busy: {inbox}"
    );
    assert!(
        !items.iter().any(|i| i["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "retry")),
        "and nothing may offer to talk to an agent that is gone: {inbox}"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_pipeline_runs_its_steps_in_order_and_stops_where_a_person_belongs() {
    // "Start an agent to implement; when it is done start another to check it"
    // is only worth having if the chain is written down. This is that chain,
    // end to end, including the step where the project said a human decides.
    let Some(agent) = echo_agent() else { return };
    let repo = pipeline_repo(
        "order",
        &format!(
            r#"
[pipelines.feature]
steps = [
  {{ role = "implement", agent = "{agent}", prompt = "write the thing" }},
  {{ role = "review",    agent = "{agent}", prompt = "read the thing" }},
  {{ human = "merge" }},
]
"#
        ),
    );
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
            "kind": "feature",
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");

    let held = await_phase(&c, &addr, &["human", "failed"]).await;
    assert_eq!(
        held["phase"], "human",
        "the chain must reach the human step, not fall over on the way"
    );
    assert_eq!(held["pipeline"]["name"], "feature");
    assert_eq!(
        held["pipeline"]["step"], 2,
        "implement and review are behind it"
    );
    assert_eq!(
        held["runs"].as_array().unwrap().len(),
        2,
        "one run per step"
    );

    // It is in the inbox as a decision, not as a failure.
    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "human_step")
        .expect("a human step belongs in the inbox");
    assert_eq!(item["level"], "normal", "an expected pause is not an alarm");
    assert!(
        item["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "approve")
    );
    assert_eq!(item["work_id"], held["id"]);

    // Releasing it finishes the chain.
    let released = post(
        &c,
        &addr,
        &format!("/api/work/{}/approve", held["id"].as_str().unwrap()),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(released["released"], "merge", "{released}");

    let done = await_phase(&c, &addr, &["review", "failed"]).await;
    assert_eq!(done["phase"], "review");

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_reviewer_that_finds_something_sends_the_work_back_and_the_loop_is_bounded() {
    // Agents checking agents is the point, and an unbounded loop between them
    // is the risk. The reviewer here never runs out of complaints, so the
    // only thing that can stop it is the declared bound.
    let Some(agent) = echo_agent() else { return };
    let repo = pipeline_repo(
        "loop",
        &format!(
            r#"
[gates]
check = ["true"]

[pipelines.feature]
steps = [
  {{ role = "implement", agent = "{agent}", prompt = "write the thing", gate = "check" }},
  {{ role = "review",    agent = "{agent}", prompt = "criticise the thing", findings = {{ back_to = "implement", max = 1 }} }},
  {{ human = "merge" }},
]
"#
        ),
    );
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
            "kind": "feature",
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");

    let settled = await_phase(&c, &addr, &["failed", "human", "review"]).await;
    assert_eq!(
        settled["phase"], "failed",
        "a review loop that never converges must end at a human, not run forever"
    );
    let entries = settled["pipeline"]["entries"].as_array().unwrap();
    assert_eq!(
        entries[0], 2,
        "implement ran once, then once more on the findings — and no further"
    );

    // **The reviewer is named as the reason, and what it found survives.**
    // This is the half that was wrong for the life of the module: the findings
    // file is read and deleted by the code that decides the loop is spent, so
    // that text is the only copy left. Dropping it left the inbox to guess from
    // `last_gate()` — which here is `check`, and which *passed* — so a person
    // was shown a green gate announced as the failure, with a Retry button that
    // handed the agent a passing summary.
    assert_eq!(
        settled["stopped"]["reason"], "review_exhausted",
        "{settled}"
    );
    assert_eq!(settled["stopped"]["step"], "review", "{settled}");
    // The board is handed the sentence rather than deriving it a second time:
    // its "hand back again" button used to be titled with the last gate's
    // summary, and here that gate passed. Named apart from the object it
    // summarises, because two fields of one name in a flattened struct is a
    // duplicate key and whichever a reader takes is luck.
    assert_eq!(
        settled["stopped_summary"], "review kept finding things",
        "{settled}"
    );
    assert_eq!(settled["stopped"]["back_to"], "implement", "{settled}");
    assert!(
        !settled["stopped"]["findings"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .is_empty(),
        "what the reviewer found has to outlive the file it was written to: {settled}"
    );
    // Every gate here passed, which is exactly why guessing the reason was
    // wrong: the judged verdict the board reads says so (D60 keeps that in one
    // place), and inferring from it would have announced `check` as the thing
    // that failed.
    assert_eq!(
        settled["gate"]["passed"], true,
        "the last gate passed, so nothing may report it as the failure: {settled}"
    );

    // And the inbox says a reviewer objected, not that a check failed.
    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let kinds: Vec<&str> = inbox
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["kind"].as_str())
        .collect();
    assert!(
        kinds.contains(&"review_exhausted"),
        "a reviewer disagreeing and a suite disagreeing are not the same errand: {inbox}"
    );
    assert!(
        !kinds.contains(&"gate_failed"),
        "no gate failed here: {inbox}"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn the_findings_reach_the_agent_that_has_to_act_on_them() {
    // Sending work back without saying why costs a full turn of rediscovery.
    let Some(agent) = echo_agent() else { return };
    let repo = pipeline_repo(
        "handback",
        &format!(
            r#"
[gates]
check = ["true"]

[pipelines.feature]
steps = [
  {{ role = "implement", agent = "{agent}", prompt = "write the thing", gate = "check" }},
  {{ role = "review",    agent = "{agent}", prompt = "criticise the thing", findings = {{ back_to = "implement", max = 1 }} }},
]
"#
        ),
    );
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    post(
        &c,
        &addr,
        "/api/work",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
            "kind": "feature",
        }),
    )
    .await;
    let settled = await_phase(&c, &addr, &["failed", "review"]).await;

    // implement, review, then implement again on what the review found.
    let runs = settled["runs"].as_array().unwrap();
    assert!(runs.len() >= 3, "the work went back for a second attempt");

    // Prompt text is never stored — telemetry is redacted and stays that way —
    // so the fixture writes down what it was told, and that is what proves the
    // reviewer's words reached the agent that has to act on them.
    let worktree = PathBuf::from(settled["worktree"].as_str().unwrap());
    let heard = std::fs::read_to_string(worktree.join(".vibeplane/heard.log")).unwrap_or_default();
    let turns: Vec<&str> = heard
        .split("\n---\n")
        .filter(|t| !t.trim().is_empty())
        .collect();
    assert!(turns.len() >= 3, "three prompts, one per step: {turns:?}");
    assert!(
        turns[2].contains("error path is not covered"),
        "the agent that has to fix it was never told what was wrong: {}",
        turns[2]
    );
    assert!(
        !turns[0].contains("error path is not covered"),
        "and it was not told before there was anything to tell"
    );

    std::fs::remove_dir_all(&repo).ok();
}

/// A standing grant is the one decision whose consequences Vibeplane never
/// sees, so it has to be the one the log is clearest about.
///
/// "Allow always" lives inside the *agent's* session: every later call it
/// covers is approved there, and no request for those ever reaches Vibeplane.
/// Recorded as a plain "allow" it would leave `vibeplane audit` answering
/// "why did that command run without anybody being asked?" with silence —
/// which is the question the decision log exists for
/// ([arXiv:2606.22504](https://arxiv.org/abs/2606.22504) calls this lingering
/// authority, and recommends exactly this: make the scope visible).
#[test]
fn a_standing_grant_is_recorded_as_one() {
    use vibeplane::core::{Actor, Decision};

    let once = Decision::new(Actor::Human, "agent:tool.use", "Bash: pnpm test", "allow");
    let always = Decision::new(
        Actor::Human,
        "agent:tool.use",
        "Bash: pnpm test",
        "allow_always",
    )
    .because("a standing choice made by a person");

    assert_ne!(
        once.outcome, always.outcome,
        "a one-off and a standing grant must not read the same in the log"
    );
    assert!(
        always.reason.is_some(),
        "and the standing one has to say what it means"
    );
}

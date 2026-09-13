//! The verified-done loop, against a real repository.
//!
//! This is the product's central claim, so the test does the real thing: a git
//! repository with a committed `vibeplane.toml`, a gate command that genuinely
//! fails, a worktree, and an agent that claims success without earning it.

use serde_json::Value;
use std::path::{Path, PathBuf};
use vibeplane::daemon::{AppState, Shared};
use vibeplane_core::Policy;

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

async fn boot() -> (std::net::SocketAddr, reqwest::Client, Shared) {
    let db = std::env::temp_dir().join(format!("vp-work-{}.db", uuid::Uuid::new_v4().simple()));
    let state = AppState::new(db, "tok".into(), Policy::default())
        .await
        .unwrap();
    let app = vibeplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new(), state)
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
async fn await_phase(c: &reqwest::Client, addr: &std::net::SocketAddr, want: &[&str]) -> Value {
    for _ in 0..200 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let list: Value = c
            .get(format!("http://{addr}/api/work"))
            .bearer_auth("tok")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if let Some(w) = list.as_array().and_then(|a| a.first())
            && want.contains(&w["phase"].as_str().unwrap_or(""))
        {
            return w.clone();
        }
    }
    panic!("work never reached {want:?}");
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
async fn a_red_pull_request_reaches_the_inbox_after_the_agent_is_gone() {
    // The clearest reason Work is the durable unit: checks finish long after
    // the session that wrote the code has ended, and somebody still has to be
    // told the build went red.
    let repo = scratch_repo("prinbox", "escalate", 0);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    // A finished piece of work with a pull request, and no live session at all.
    let mut work = vibeplane_domain::Work::new(
        vibeplane_domain::ProjectId::from_path(&repo),
        vibeplane_domain::WorkKind::Bug,
        "fix the flaky login test".into(),
        "fix it".into(),
    );
    work.phase = vibeplane_domain::Phase::Review;
    work.pull_request = Some(vibeplane_domain::work::PullRequestRef {
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
            .any(|a| a == "send_to_agent"),
        "and the obvious action is to hand it back"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn an_approved_and_green_pull_request_asks_for_nothing() {
    // Nobody is being asked for anything, so it does not belong in a queue of
    // decisions. An inbox that lists finished work stops being read.
    let repo = scratch_repo("prquiet", "escalate", 0);
    let (addr, c, state) = boot().await;
    let mut work = vibeplane_domain::Work::new(
        vibeplane_domain::ProjectId::from_path(&repo),
        vibeplane_domain::WorkKind::Quick,
        "done and dusted".into(),
        "x".into(),
    );
    work.pull_request = Some(vibeplane_domain::work::PullRequestRef {
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

    let mut work = vibeplane_domain::Work::new(
        vibeplane_domain::ProjectId::from_path(&repo),
        vibeplane_domain::WorkKind::Quick,
        "half-finished".into(),
        "x".into(),
    );
    work.phase = vibeplane_domain::Phase::Implement;
    work.runs.push(vibeplane_domain::RunId::new("gone"));
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

// ---------------------------------------------------------------------------
// Declared pipelines (D34)
// ---------------------------------------------------------------------------

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
    // is the risk (R16). The reviewer here never runs out of complaints, so the
    // only thing that can stop it is the declared bound.
    let Some(agent) = echo_agent() else { return };
    let repo = pipeline_repo(
        "loop",
        &format!(
            r#"
[pipelines.feature]
steps = [
  {{ role = "implement", agent = "{agent}", prompt = "write the thing" }},
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
[pipelines.feature]
steps = [
  {{ role = "implement", agent = "{agent}", prompt = "write the thing" }},
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

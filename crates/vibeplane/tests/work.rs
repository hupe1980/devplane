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

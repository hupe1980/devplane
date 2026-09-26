//! The verified-done loop against a real repository: a committed `devplane.toml`,
//! a gate that genuinely fails, a worktree, and an agent that claims success unearned.

mod common;
#[path = "common/sandbox.rs"]
mod sandbox;

use devplane::core::Policy;
use devplane::host::{AppState, Shared};
use serde_json::Value;
use std::path::{Path, PathBuf};

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
        dir.join("devplane.toml"),
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
    // macOS temp dirs sit behind a symlink the host canonicalises.
    dir.canonicalize().unwrap()
}

/// A repository with this `devplane.toml` committed.
fn configured_repo(tag: &str, config: &str) -> PathBuf {
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
    std::fs::write(dir.join("devplane.toml"), config).unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

/// The counts fixture (15 task lines, 11 ticked, one passing gate), copied into its own repository.
fn counts_repo(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-counts-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/spec/counts");
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), target).unwrap();
            }
        }
    }
    copy(&from, &dir);
    // Ignored, so the echo agent's log keeps the gate's tree clean.
    std::fs::write(dir.join(".gitignore"), ".devplane/\n").unwrap();
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
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

fn echo_agent() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?.parent()?.join("examples").join("echo_agent");
    bin.exists().then(|| bin.to_string_lossy().to_string())
}

/// A host for one test that stops its agents on drop, so a panicking test
/// cannot leave ACP sessions starving the shared async-io reactor.
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
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

async fn boot() -> (std::net::SocketAddr, reqwest::Client, Host) {
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

/// A driven run in `dir` with no change behind it. Returns the run id.
async fn run_in(state: &Shared, agent: &str, dir: &Path, prompt: Option<&str>) -> String {
    let spec = devplane::acp::resolve(agent, &state.agents).expect("the echo agent resolves");
    devplane::driven::dispatch(
        state,
        &spec,
        dir.to_path_buf(),
        prompt.map(str::to_string),
        Vec::new(),
    )
    .await
    .expect("the run starts")
    .to_string()
}

/// How long a change may take to settle. Generous because every test drives a
/// real agent and real gates, in parallel, on possibly loaded CI runners.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(90);
const POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// Where a change came to rest: `stopped`, `looking` (a person should look),
/// `finished` (accepted), or empty while still moving.
fn resting(w: &Value) -> &'static str {
    if w["completion"].is_object() {
        "finished"
    } else if w["stopped"].is_object() {
        "stopped"
    } else if w["waiting"]["on"] == "person" {
        "looking"
    } else {
        ""
    }
}

async fn await_rest(c: &reqwest::Client, addr: &std::net::SocketAddr, want: &[&str]) -> Value {
    let mut last = Value::Null;
    let deadline = std::time::Instant::now() + SETTLE;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(POLL).await;
        let list: Value = c
            .get(format!("http://{addr}/api/changes"))
            .bearer_auth("tok")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if let Some(w) = list.as_array().and_then(|a| a.first()) {
            last = w.clone();
            if want.contains(&resting(w)) {
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
        "change never reached {want:?}\n  state:     {} waiting {} stopped {}\n  runs:      {}\n  gates:     {}\n  decisions: {}",
        last["state"],
        last["waiting"],
        last["stopped"],
        last["runs"],
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
    // A headless agent runs the repo's hooks and MCP servers unasked, so trust is required.
    let repo = scratch_repo("trust", "feedback", 1);
    let (addr, c, _state) = boot().await;

    let res = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({ "cwd": repo.to_string_lossy(), "title": "do a thing" }),
    )
    .await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("not trusted"), "got {err:?}");
    assert!(err.contains("devplane trust"), "and says how to fix it");

    // Nothing was created on the way to refusing.
    assert!(!repo.join(".claude/worktrees").exists());
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_claim_of_done_that_fails_the_gates_does_not_become_done() {
    // The agent says it finished, the gate disagrees, and the change is not marked done.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("verify", "feedback", 1);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "fix the flaky login test",
            "agent": agent,
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");

    let work = started["change"].clone();
    let worktree = PathBuf::from(work["worktree"].as_str().expect("an isolated checkout"));
    assert!(worktree.exists(), "the checkout is real");
    assert!(
        worktree.starts_with(repo.join(".claude/worktrees")),
        "and lives where Claude Code puts its own"
    );
    assert!(work["branch"].as_str().unwrap().starts_with("change/"));

    let settled = await_rest(&c, &addr, &["stopped", "looking"]).await;
    assert_eq!(
        resting(&settled),
        "stopped",
        "an unearned claim of done must not reach review"
    );

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

    // The current verdict carries its own evidence and `passed` flag.
    let gate = &settled["gate"];
    let commands = gate["commands"]
        .as_array()
        .expect("the verdict carries the commands it was reached from");
    assert!(!commands.is_empty());
    assert!(
        commands
            .iter()
            .any(|c| c["outcome"]["code"].as_i64().is_some_and(|code| code != 0)),
        "a failed gate names the command that failed: {gate}"
    );
    assert!(
        commands
            .iter()
            .any(|c| !c["output_tail"].as_str().unwrap_or("").is_empty()),
        "and what it printed — the text the agent was handed, not a summary"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn fixing_the_cause_lets_the_change_pass() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("pass", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "make it pass",
            "agent": agent,
        }),
    )
    .await;
    let id = started["change_id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    await_rest(&c, &addr, &["stopped"]).await;

    std::fs::write(worktree.join("fixed.txt"), "").unwrap();

    let verdict = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(verdict["passed"], true, "{verdict}");
    let w = await_rest(&c, &addr, &["looking"]).await;
    assert_eq!(
        resting(&w),
        "looking",
        "green gates means a human should look"
    );

    // Finishing removes nothing; archiving removes the worktree and leaves git tidy.
    post(&c, &addr, &format!("/api/changes/{id}/finish"), Value::Null).await;
    assert!(worktree.exists(), "finishing does not remove the checkout");
    let archived = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive?discard_uncommitted=true"),
        Value::Null,
    )
    .await;
    assert_eq!(archived["ok"], true, "{archived}");
    assert!(!worktree.exists(), "the checkout is gone");
    std::fs::remove_dir_all(&repo).ok();
}

/// End to end through the real API: fix, finish, export the certificate, and
/// check the fields the running code fills in (commit, basis, gate).
#[tokio::test]
async fn a_finished_change_exports_a_certificate_a_stranger_could_check() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("cert", "feedback", 2);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "make the gate pass",
            "agent": agent,
        }),
    )
    .await;

    // Commit the fix: a pass over an uncommitted tree finishes as `gates_stale`.
    let w = await_rest(&c, &addr, &["stopped", "looking", "finished"]).await;
    let id = w["id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(w["worktree"].as_str().expect("an isolated checkout"));
    std::fs::write(worktree.join("fixed.txt"), "yes").unwrap();
    for args in [vec!["add", "-A"], vec!["commit", "-qm", "fix it"]] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(&worktree)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let verdict = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(verdict["passed"], true, "{verdict}");

    // Before finishing, the export does not claim to be a certificate.
    let pending: Value = c
        .get(format!("http://{addr}/api/changes/{id}/certificate"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        pending["finished"], false,
        "unfinished work claimed a completion"
    );
    assert!(
        pending["markdown"]
            .as_str()
            .unwrap()
            .contains("not finished"),
        "{}",
        pending["markdown"]
    );

    post(
        &c,
        &addr,
        &format!("/api/changes/{id}/finish"),
        serde_json::json!({}),
    )
    .await;

    let cert: Value = c
        .get(format!("http://{addr}/api/changes/{id}/certificate"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cert["finished"], true);
    let md = cert["markdown"].as_str().unwrap();
    let st = &cert["statement"];

    assert_eq!(st["_type"], "https://in-toto.io/Statement/v1");
    let sha = st["subject"][0]["digest"]["gitCommit"]
        .as_str()
        .expect("no commit was stamped by the code that actually runs");
    assert_eq!(sha.len(), 40, "not a full object id: {sha}");
    assert!(md.contains(sha), "the document and the statement disagree");

    // Committed, clean, and unchanged since the gate ran: the one passing basis.
    assert_eq!(
        st["predicate"]["completion"]["basis"], "gates_passed",
        "{}",
        st["predicate"]["completion"]
    );
    assert_eq!(st["predicate"]["evidence"]["passed"], true);
    assert!(md.contains("gates passed"), "{md}");
    let tree = st["predicate"]["evidence"]["commit"]["tree"]
        .as_str()
        .expect("no tree digest was stamped by the code that actually runs");
    assert_eq!(tree.len(), 40, "not a full object id: {tree}");
    assert_ne!(tree, sha);
    assert!(md.contains(tree), "the document does not carry the tree");

    let steps = st["predicate"]["verification"]["steps"].as_array().unwrap();
    assert_eq!(steps[0], format!("git checkout {sha}"));
    for cmd in st["predicate"]["evidence"]["commands"].as_array().unwrap() {
        assert!(
            cmd["outcome"]["outcome"].is_string(),
            "a command with no structured outcome: {cmd}"
        );
        assert!(cmd["output_digest"].as_str().is_some_and(|d| d.len() == 16));
    }

    assert!(md.contains("not evidence that the work is correct"), "{md}");
    let caveat = &st["predicate"]["verification"]["caveat"];
    assert!(
        caveat.is_string(),
        "a scratch repository has no remote, so the certificate owes the reader a caveat"
    );
    assert!(md.contains("no remote"), "{md}");

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_project_with_no_gates_asks_a_human_rather_than_claiming_success() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("nogates", "feedback", 1);
    // Committed: isolation refuses a checkout with uncommitted tracked changes.
    std::fs::write(repo.join("devplane.toml"), "[project]\nname = \"x\"\n").unwrap();
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
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "unverifiable",
            "agent": agent,
        }),
    )
    .await;

    let w = await_rest(&c, &addr, &["looking", "stopped", "finished"]).await;
    assert_eq!(resting(&w), "looking");
    assert!(
        w["gates"].as_array().unwrap().is_empty(),
        "and no gate report pretends one ran"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// The gates are declared by the person's checkout, not by the `devplane.toml`
/// on the agent's branch, though they run in the worktree.
#[tokio::test]
async fn the_branch_cannot_choose_its_own_gates() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("owngates", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "redefine done",
            "agent": agent,
        }),
    )
    .await;
    let id = started["change_id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    await_rest(&c, &addr, &["stopped"]).await;

    std::fs::write(
        worktree.join("devplane.toml"),
        "[gates]\ncheck = [\"true\"]\n",
    )
    .unwrap();
    for args in [vec!["add", "-A"], vec!["commit", "-qm", "my own gates"]] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(&worktree)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // On demand: the person's command runs, and still fails.
    let verdict = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(verdict["passed"], false, "{verdict}");
    assert_eq!(
        verdict["report"]["commands"][0]["command"], "sh ./check.sh",
        "the branch's gate ran instead of the person's: {verdict}"
    );

    // And on the loop's own path, the gates that run are still the person's.
    let retried = post(&c, &addr, &format!("/api/changes/{id}/retry"), Value::Null).await;
    assert!(retried["error"].is_null(), "{retried}");
    let settled = await_rest(&c, &addr, &["stopped", "looking"]).await;
    assert_eq!(resting(&settled), "stopped", "{settled}");
    let gates = settled["gates"].as_array().unwrap();
    assert!(gates.len() >= 3, "{settled}");
    for g in gates {
        for cmd in g["commands"].as_array().unwrap() {
            assert_eq!(
                cmd["command"], "sh ./check.sh",
                "a gate the branch declared ran: {g}"
            );
        }
    }
    std::fs::remove_dir_all(&repo).ok();
}

/// A worktree deleted by hand reports that no gate command started, and why,
/// rather than a broken gate.
#[tokio::test]
async fn a_worktree_removed_by_hand_is_recorded_as_nothing_ran() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("byhand", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "lose the checkout",
            "agent": agent,
        }),
    )
    .await;
    let id = started["change_id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    await_rest(&c, &addr, &["stopped"]).await;

    std::fs::remove_dir_all(&worktree).unwrap();

    let verdict = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert!(
        verdict["error"].is_null(),
        "an error, not a report: {verdict}"
    );
    assert_eq!(verdict["passed"], false);
    let cmd = &verdict["report"]["commands"][0];
    assert_eq!(cmd["command"], "sh ./check.sh");
    assert_eq!(
        cmd["outcome"]["outcome"], "never_started",
        "a gate that could not run reported as though it had: {verdict}"
    );
    assert!(
        cmd["outcome"]["reason"]
            .as_str()
            .unwrap_or("")
            .contains("is gone"),
        "{verdict}"
    );
    assert!(
        verdict["report"]["commit"].is_null(),
        "no tree was there to stamp"
    );

    let w = await_rest(&c, &addr, &["stopped"]).await;
    assert_eq!(w["stopped"]["reason"], "broken", "{w}");
    assert!(
        w["stopped"]["detail"]
            .as_str()
            .unwrap_or("")
            .contains("is gone"),
        "{w}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_broken_config_stops_the_change_rather_than_being_ignored() {
    // A typo in a deny rule must not silently disappear.
    let repo = scratch_repo("badconfig", "feedback", 1);
    // `change start` reads the main checkout's copy, so no commit is needed.
    std::fs::write(repo.join("devplane.toml"), "[gates]\nchekc = [\"true\"]\n").unwrap();
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let res = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({ "cwd": repo.to_string_lossy(), "title": "x" }),
    )
    .await;
    assert!(
        res["error"]
            .as_str()
            .unwrap_or("")
            .contains("devplane.toml"),
        "the error names the file: {res}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn an_agent_cannot_start_where_nobody_trusted_it() {
    // The bare run beneath a change is refused as well as the change.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("bypass", "feedback", 1);
    let (addr, c, state) = boot().await;

    let res = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({ "agent": agent, "cwd": repo.to_string_lossy(), "title": "x" }),
    )
    .await;
    assert!(
        res["error"].as_str().unwrap_or("").contains("not trusted"),
        "an untrusted repository is refused: {res}"
    );
    let spec = devplane::acp::resolve(&agent, &state.agents).unwrap();
    let bare = devplane::driven::dispatch(&state, &spec, repo.clone(), None, Vec::new()).await;
    assert!(
        bare.unwrap_err().to_string().contains("not trusted"),
        "and so is a run with no change behind it"
    );

    trust(&c, &addr, &repo).await;
    assert!(!run_in(&state, &agent, &repo, None).await.is_empty());
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn one_prompt_to_several_projects_starts_none_until_every_one_can_take_it() {
    // All-or-nothing: never one failure after some successes.
    let Some(agent) = echo_agent() else { return };
    let a = scratch_repo("many-a", "escalate", 0);
    let b = scratch_repo("many-b", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &a).await;

    let body = serde_json::json!({
        "projects": [a.to_string_lossy(), b.to_string_lossy(), "no-such-project"],
        "title": "bump the lockfile",
        "agent": agent,
    });
    let refused = post(&c, &addr, "/api/changes", body.clone()).await;
    let err = refused["error"].as_str().unwrap_or_default();
    assert!(err.contains("nothing was started"), "{refused}");
    assert!(
        err.contains("not trusted"),
        "the untrusted one is named: {err}"
    );
    assert!(
        err.contains("no-such-project"),
        "and so is the name that matches nothing: {err}"
    );
    assert!(
        !a.join(".claude/worktrees").exists(),
        "the trusted project was started anyway"
    );
    assert!(
        get(&c, &addr, "/api/changes")
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );

    let pre = post(&c, &addr, "/api/changes/preflight", body).await;
    assert_eq!(pre["refused"], 2, "{pre}");

    trust(&c, &addr, &b).await;
    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "projects": [a.to_string_lossy(), b.to_string_lossy()],
            "title": "bump the lockfile",
            "agent": agent,
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    assert_eq!(started["changes"].as_array().unwrap().len(), 2, "{started}");
    std::fs::remove_dir_all(&a).ok();
    std::fs::remove_dir_all(&b).ok();
}

#[tokio::test]
async fn every_verdict_is_accountable_afterwards() {
    // The log names the rule that auto-approved a command or opened a pull request.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("audit", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "fix the flaky login test",
            "agent": agent,
        }),
    )
    .await;
    await_rest(&c, &addr, &["stopped", "looking"]).await;

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
        .expect("the gate verdict is the decision that made this change fail");
    assert_eq!(gate["outcome"], "fail");
    assert_eq!(gate["authority"], "devplane");
    assert!(
        gate["subject"].as_str().unwrap().contains("check.sh"),
        "the log names the commands that decided: {gate}"
    );
    assert!(
        gate["reason"].as_str().unwrap().contains("failed"),
        "and why: {gate}"
    );
    assert!(gate["change_id"].is_string(), "attributed to the change");

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_spent_feedback_budget_actually_asks_somebody() {
    // Red gates go back to the same session, bounded, then the inbox asks.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("asked", "feedback", 1);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "fix the flaky login test",
            "agent": agent,
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let change_id = started["change_id"].as_str().unwrap().to_string();
    let settled = await_rest(&c, &addr, &["stopped", "looking"]).await;
    assert_eq!(resting(&settled), "stopped");

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let inbox = inbox["items"].clone();
    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "gate_failed")
        .expect("a spent budget has to reach the person whose turn it now is");

    assert_eq!(item["level"], "high");
    assert_eq!(item["change_id"], change_id.as_str());
    assert!(
        item["detail"].as_str().unwrap().contains("auth::login"),
        "the person and the agent must see the same failures: {item}"
    );

    // The retry offer is real: the session that wrote the code is still alive.
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
        &format!("/api/changes/{change_id}/retry"),
        serde_json::json!({}),
    )
    .await;
    assert!(retried["error"].is_null(), "{retried}");

    // Handed back, counted against the budget, recorded as a person's decision.
    let after = await_rest(&c, &addr, &["stopped", "looking"]).await;
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
        .find(|d| d["action"] == "change:retry")
        .expect("going past the project's bound is a decision somebody made");
    assert_eq!(retry["authority"], "person");
    assert!(
        retry["reason"]
            .as_str()
            .unwrap()
            .contains("past the project")
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_ceiling_stops_a_change_that_gets_expensive() {
    // A spend ceiling bounds one long turn; the fixture reports real usage.
    let Some(agent) = echo_agent() else { return };
    let repo = configured_repo(
        "budget",
        "[budget]\nusd = 10\n\n[gates]\ncheck = [\"true\"]\n",
    );
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
            "prompt": "do something expensive",
            "agent": agent,
        }),
    )
    .await;

    let stopped = await_rest(&c, &addr, &["stopped", "looking"]).await;
    assert_eq!(
        resting(&stopped),
        "stopped",
        "$100 against a $10 ceiling must stop the change: {stopped}"
    );
    // The stop reason is recorded, not inferred: the checks were green here.
    assert_eq!(stopped["stopped"]["reason"], "over_budget", "{stopped}");
    assert!(
        stopped["stopped"]["spent_usd"].as_f64().unwrap() >= 100.0,
        "{stopped}"
    );
    assert_eq!(
        stopped["runs"].as_array().unwrap().len(),
        1,
        "and it stopped rather than paying for another turn"
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
    let inbox = inbox["items"].clone();
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
        .find(|d| d["action"] == "change:stop")
        .expect("stopping something costs a reason");
    assert!(stop["reason"].as_str().unwrap().contains("ceiling"));

    // Handing back would stop at the same ceiling, so no offer; the command says what to change.
    assert_eq!(stopped["can_retry"], false);
    let refused = post(
        &c,
        &addr,
        &format!("/api/changes/{}/retry", stopped["id"].as_str().unwrap()),
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
    // A driven run has no other window, so its transcript is kept.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("said", "escalate", 0);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let run = run_in(
        &state,
        &agent,
        &repo,
        Some("use a tool and tell me about it"),
    )
    .await;

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

    // It starts with the prompt.
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

    // Chunks are joined, not stored one row each.
    assert!(
        said.len() <= 4,
        "a short turn is not a database of syllables: {said:?}"
    );

    // And it stays out of the event log.
    let events = state
        .store
        .events_for_run(&devplane::core::RunId::new(&run), 200)
        .await
        .unwrap();
    let events = serde_json::to_value(events).unwrap();
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
    // Agent prose can quote secrets (e.g. an included `.env`), so the repo can opt out.
    let Some(agent) = echo_agent() else { return };
    let repo = configured_repo("quiet", "[transcripts]\nkeep = false\n");
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let run = run_in(&state, &agent, &repo, Some("say something quotable")).await;
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
    // Shutdown must end every agent it started, not leave them re-parented to init.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("reap", "feedback", 1);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    run_in(&state, &agent, &repo, None).await;
    assert_eq!(state.sessions.lock().await.len(), 1);

    state.shutdown().await;
    assert!(
        state.sessions.lock().await.is_empty(),
        "an agent survived the host that started it"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_config_that_cannot_work_is_refused_before_anything_is_spent() {
    // An empty gate is refused up front.
    let repo = configured_repo("invalid", "[gates.named.empty]\nrun = []\n");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let res = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "add rate limiting",
        }),
    )
    .await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("has no commands"), "got {err:?}");
    assert!(!repo.join(".claude/worktrees").exists());
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_removed_key_refuses_the_start_by_name() {
    // A `[pipelines]` key is refused by name rather than ignored.
    let repo = configured_repo("removed", "[pipelines.feature]\nsteps = []\n");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let res = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({ "cwd": repo.to_string_lossy(), "title": "x" }),
    )
    .await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("`[pipelines]` was removed"), "got {err:?}");
    assert!(!repo.join(".claude/worktrees").exists());
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_worktree_inherits_the_trust_of_its_repository() {
    // Trust covers every checkout of a trusted project.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("inherit", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "in a worktree",
            "agent": agent,
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let worktree = started["change"]["worktree"].as_str().unwrap();

    let spec = devplane::acp::resolve(&agent, &_state.agents).unwrap();
    let again =
        devplane::driven::dispatch(&_state, &spec, PathBuf::from(worktree), None, Vec::new()).await;
    assert!(again.is_ok(), "the worktree is trusted too: {again:?}");
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn snoozing_a_change_actually_quietens_it() {
    // Snooze works on change items that have no live run.
    let repo = scratch_repo("snoozework", "escalate", 0);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let mut work = devplane::core::Change::new(
        devplane::core::ProjectId::from_path(&repo),
        "flaky login test".into(),
        "fix it".into(),
    );
    work.waiting = Some(devplane::core::Waiting::Person);
    work.pull_request = Some(devplane::core::change::PullRequestRef {
        number: 7,
        url: "https://github.com/acme/app/pull/7".into(),
        status: "failing".into(),
        failing_checks: vec!["test".into()],
    });
    let id = work.id.clone();
    // No run has ever touched it.
    assert!(work.current_run().is_none());
    state.changes.lock().await.insert(id.clone(), work);

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
        let v = v["items"].clone();
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
        &format!("/api/changes/{}/snooze?minutes=60", id.as_str()),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(snoozed["ok"], true, "{snoozed}");

    let after = inbox_kinds(c.clone(), addr).await;
    assert!(
        !after.contains(&"ci_red".to_string()),
        "a snoozed change must leave the queue: {after:?}"
    );

    // It comes back: snoozing is "not now", not "never".
    let woken = post(
        &c,
        &addr,
        &format!("/api/changes/{}/snooze?minutes=0", id.as_str()),
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
    // CI finishes after the session ends, and a red build still reaches the inbox.
    let repo = scratch_repo("prinbox", "escalate", 0);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let mut work = devplane::core::Change::new(
        devplane::core::ProjectId::from_path(&repo),
        "fix the flaky login test".into(),
        "fix it".into(),
    );
    work.waiting = Some(devplane::core::Waiting::Person);
    work.pull_request = Some(devplane::core::change::PullRequestRef {
        number: 142,
        url: "https://github.com/acme/app/pull/142".into(),
        status: "failing".into(),
        failing_checks: vec!["test".into()],
    });
    let id = work.id.clone();
    state.changes.lock().await.insert(id.clone(), work);

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let inbox = inbox["items"].clone();

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
    // Nobody is being asked for anything, so it stays out of the inbox.
    let repo = scratch_repo("prquiet", "escalate", 0);
    let (addr, c, state) = boot().await;
    let mut work = devplane::core::Change::new(
        devplane::core::ProjectId::from_path(&repo),
        "done and dusted".into(),
        "x".into(),
    );
    work.pull_request = Some(devplane::core::change::PullRequestRef {
        number: 9,
        url: "u".into(),
        status: "ready_to_merge".into(),
        failing_checks: vec![],
    });
    state.changes.lock().await.insert(work.id.clone(), work);

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let inbox = inbox["items"].clone();
    assert!(
        inbox.as_array().unwrap().is_empty(),
        "nothing to decide: {inbox}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_projects_own_rules_decide_its_agents() {
    // A `[policy]` rule must take effect. Driven through the binary, as Claude
    // Code runs the gate as a `command` hook.
    let repo = scratch_repo("policy", "escalate", 0);
    std::fs::write(
        repo.join("devplane.toml"),
        "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n",
    )
    .unwrap();
    let home = std::env::temp_dir().join(format!(
        "vp-projpolicy-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&home).unwrap();

    let ask = |command: &str| -> Value {
        let body = serde_json::json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "s1",
            "cwd": repo.to_string_lossy(),
            "tool_name": "Bash",
            "tool_input": { "command": command }
        })
        .to_string();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("hook")
            .env("DEVPLANE_HOME", &home)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut ch| {
                use std::io::Write;
                ch.stdin.as_mut().unwrap().write_all(body.as_bytes())?;
                ch.wait_with_output()
            })
            .expect("the gate runs");
        serde_json::from_slice(&out.stdout).expect("the gate answers with JSON")
    };

    // A project prohibition answers; nothing is auto-approved, the rest goes to the person.
    assert_ne!(
        ask("echo hello")["hookSpecificOutput"]["decision"]["behavior"],
        "allow",
        "Devplane approved a call on the vendor's behalf"
    );
    assert_eq!(
        ask("rm -rf node_modules")["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
    assert_eq!(
        ask("curl example.com"),
        serde_json::json!({}),
        "and anything else still reaches the human"
    );

    std::fs::remove_dir_all(&repo).ok();
    std::fs::remove_dir_all(&home).ok();
}

#[tokio::test]
async fn a_projects_parallelism_limit_is_enforced() {
    // Agents in one repository collide on files, so their number is capped.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("parallel", "escalate", 0);
    std::fs::write(
        repo.join("devplane.toml"),
        "[policy]\nmax_parallel_runs = 1\n",
    )
    .unwrap();
    git_out(&repo, &["commit", "-qam", "one agent at a time"]);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    run_in(&state, &agent, &repo, None).await;

    let second = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "agent": agent,
            "cwd": repo.to_string_lossy(),
            "title": "a second one",
            "worktree": false,
        }),
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
async fn a_change_stranded_by_a_restart_says_so() {
    // The change comes back from the store without its agent, so it must not look busy.
    let repo = scratch_repo("stranded", "escalate", 0);
    let (addr, c, state) = boot().await;

    let mut work = devplane::core::Change::new(
        devplane::core::ProjectId::from_path(&repo),
        "half-finished".into(),
        "x".into(),
    );
    work.runs.push(devplane::core::RunId::new("gone"));
    state.changes.lock().await.insert(work.id.clone(), work);

    let inbox: Value = c
        .get(format!("http://{addr}/api/inbox"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let inbox = inbox["items"].clone();

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
async fn a_restart_does_not_lose_the_conversation_the_change_was_having() {
    // A host restart resumes the agent's session instead of starting over. The
    // fixture persists sessions, so a resumed one says `turn 2`, a fresh one `turn 1`.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("resume", "escalate", 0);
    let db = std::env::temp_dir().join(format!("vp-resume-{}.db", uuid::Uuid::new_v4().simple()));

    let _serial = common::one_agent_at_a_time();

    let (change_id, agent_session) = {
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
        let server = tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        let c = reqwest::Client::new();
        trust(&c, &addr, &repo).await;

        let run_id =
            devplane::core::RunId::new(run_in(&state, &agent, &repo, Some("start the work")).await);

        // Resume needs the agent's session id, which exists after the handshake.
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

        let mut work = devplane::core::Change::new(
            devplane::core::ProjectId::from_path(&repo),
            "half-finished".into(),
            "x".into(),
        );
        work.runs.push(run_id.clone());
        let id = work.id.clone();
        state.store.save_change(&work).await.unwrap();
        state.changes.lock().await.insert(id.clone(), work);

        let mut snapshot = state
            .world
            .lock()
            .await
            .run(&run_id)
            .expect("the run exists")
            .clone();
        snapshot.state = devplane::core::RunState::Working;
        state.shutdown().await;
        state.store.save_run(&snapshot).await.unwrap();
        server.abort();
        (id, agent_session)
    };

    // A new host over the same database, with every agent process gone.
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
    let server = tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    let c = reqwest::Client::new();
    assert!(
        state.drivable_runs().await.is_empty(),
        "no agent survived the restart"
    );
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
    // `/api/inbox` answers { items, close }: the list, and what the day
    // came to for a list that is empty.
    let inbox = inbox["items"].clone();
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
        &format!("/api/changes/{}/resume", change_id.as_str()),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(resumed["ok"], true, "{resumed}");

    // The same conversation: a new session would say `turn 1`.
    let run = {
        let changes = state.changes.lock().await;
        changes
            .get(&change_id)
            .unwrap()
            .current_run()
            .cloned()
            .unwrap()
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

    // This test started the resumed agent, so it ends it, keeping the reactor free.
    state.shutdown().await;
    server.abort();
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_restart_does_not_leave_a_run_looking_drivable() {
    // A restored ACP run looks alive (no pid to reconcile) while its process is
    // gone; driveability must not be decided from run state.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("restart", "escalate", 0);
    let db = std::env::temp_dir().join(format!("vp-restart-{}.db", uuid::Uuid::new_v4().simple()));

    let _serial = common::one_agent_at_a_time();
    let change_id = {
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
        let server = tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        let c = reqwest::Client::new();
        trust(&c, &addr, &repo).await;

        // No prompt, so it stays put rather than racing a gate.
        let run_id = devplane::core::RunId::new(run_in(&state, &agent, &repo, None).await);

        let mut work = devplane::core::Change::new(
            devplane::core::ProjectId::from_path(&repo),
            "half-finished".into(),
            "x".into(),
        );
        work.runs.push(run_id.clone());
        let id = work.id.clone();
        state.store.save_change(&work).await.unwrap();
        state.changes.lock().await.insert(id.clone(), work);

        // A crash, not a clean stop: the last recorded state is *working*.
        let mut snapshot = state
            .world
            .lock()
            .await
            .run(&run_id)
            .expect("the run exists")
            .clone();
        snapshot.state = devplane::core::RunState::Working;
        state.shutdown().await;
        state.store.save_run(&snapshot).await.unwrap();
        server.abort();
        id.to_string()
    };

    // The restart.
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
    let c = reqwest::Client::new();

    // The restored run looks alive, which is why run state is the wrong question.
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
    let inbox = inbox["items"].clone();
    let items = inbox.as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|i| i["kind"] == "interrupted" && i["change_id"] == change_id.as_str()),
        "work whose agent died with the host must say so rather than look busy: {inbox}"
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

/// The tasks a start names travel on the run's record and prompt; none named
/// records none; a watched session carries none; an unknown selector refuses.
#[tokio::test]
async fn tasks_sent_travel_with_the_dispatch_and_a_watched_run_has_none() {
    let Some(agent) = echo_agent() else { return };
    let repo = counts_repo("sent");
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let refused = c
        .post(format!("http://{addr}/api/changes"))
        .bearer_auth("tok")
        .json(&serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "nothing",
            "agent": agent,
            "spec": "specs/007-counts",
            "tasks": ["FR-099"],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 400);
    let body: Value = refused.json().await.unwrap();
    let err = body["error"].as_str().unwrap_or("");
    assert!(
        err.contains("no task in specs/007-counts matches `FR-099`"),
        "{err}"
    );
    assert!(err.contains("T001 Scaffold the module (FR-001)"), "{err}");
    assert!(
        !repo.join(".claude/worktrees").exists(),
        "a refused dispatch left a worktree behind"
    );
    assert!(
        get(&c, &addr, "/api/changes")
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );

    let refused = c
        .post(format!("http://{addr}/api/changes"))
        .bearer_auth("tok")
        .json(&serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "nothing",
            "agent": agent,
            "tasks": ["FR-001"],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 400);
    let body: Value = refused.json().await.unwrap();
    assert!(
        body["error"].as_str().unwrap_or("").contains("spec"),
        "{body}"
    );

    // Every line citing the first label.
    let v = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "do the first requirement",
            "agent": agent,
            "spec": "specs/007-counts",
            "tasks": ["FR-001"],
        }),
    )
    .await;
    assert!(v.get("error").is_none(), "{v}");
    let run = v["change"]["runs"][0].as_str().expect("a run").to_string();
    let r = get(&c, &addr, &format!("/api/runs/{run}")).await;
    let sent: Vec<&str> = r["sent"]
        .as_array()
        .expect("sent is a list")
        .iter()
        .map(|t| t["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        sent,
        [
            "T001 Scaffold the module (FR-001)",
            "T002 Wire the route (FR-001)",
            "T003 Validate the input (FR-001)",
            "T015 Remove the flag (FR-001)",
        ],
        "from the dispatch record, not from parsing"
    );
    assert_eq!(r["sent_says"], "sent 4 tasks");
    assert_eq!(r["sent"][0]["cites"], serde_json::json!(["FR-001"]));
    assert_eq!(r["sent"][0]["path"], "specs/007-counts/tasks.md");
    assert_eq!(r["sent"][0]["line"], 5);

    let settled = await_rest(&c, &addr, &["looking", "stopped"]).await;
    let worktree = PathBuf::from(settled["worktree"].as_str().unwrap());
    let heard = std::fs::read_to_string(worktree.join(".devplane/heard.log")).unwrap_or_default();
    assert!(
        heard.contains(
            "Tasks:\n- T001 Scaffold the module (FR-001)\n- T002 Wire the route (FR-001)"
        ),
        "the agent was not told what it was sent: {heard}"
    );
    assert_eq!(
        heard.matches(devplane::core::context::STOP_AND_SAY).count(),
        1,
        "{heard}"
    );
    let row = get(
        &c,
        &addr,
        &format!("/api/changes/{}", settled["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(row["run_rows"][0]["sent_says"], "sent 4 tasks", "{row}");

    // Naming no tasks sends and records none.
    let v = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "another",
            "agent": agent,
            "spec": "specs/007-counts",
        }),
    )
    .await;
    assert!(v.get("error").is_none(), "{v}");
    let run = v["change"]["runs"][0].as_str().expect("a run").to_string();
    let r = get(&c, &addr, &format!("/api/runs/{run}")).await;
    assert!(r["sent"].is_null(), "{r}");
    assert_eq!(r["sent_says"], "no tasks were sent to this run");
    await_rest(&c, &addr, &["looking", "stopped"]).await;

    // A watched session, written by a hook into the same store.
    let payload: devplane::observe::hook::HookPayload = serde_json::from_value(serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": "s-watched",
        "cwd": repo.to_string_lossy(),
        "source": "startup",
    }))
    .unwrap();
    devplane::record::observe(&state.store, &payload, devplane::core::Source::Hook)
        .await
        .unwrap();
    devplane::poller::tail_once(&state).await.unwrap();
    let r = get(&c, &addr, "/api/runs/s-watched").await;
    assert_eq!(r["mode"], "observed", "{r}");
    assert!(r["sent"].is_null());
    assert_eq!(
        r["sent_says"],
        "watched, not dispatched by Devplane — it carries no task edges"
    );

    std::fs::remove_dir_all(&repo).ok();
}

/// Ticked and verified are two counts, never one number; with no gates,
/// verified is absent, not zero.
#[tokio::test]
async fn eleven_ticked_nine_verified_and_never_one_number() {
    let Some(agent) = echo_agent() else { return };
    let repo = counts_repo("counts");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    // A heading sits between the task lines.
    let selectors: Vec<String> = (5..=10)
        .chain(14..=16)
        .map(|n| format!("tasks.md:{n}"))
        .collect();
    let v = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "count the edges",
            "agent": agent,
            "spec": "specs/007-counts",
            "tasks": selectors,
        }),
    )
    .await;
    assert!(v.get("error").is_none(), "{v}");
    let id = v["change_id"].as_str().unwrap().to_string();
    let run = v["change"]["runs"][0].as_str().unwrap().to_string();
    assert_eq!(
        get(&c, &addr, &format!("/api/runs/{run}")).await["sent_says"],
        "sent 9 tasks"
    );

    await_rest(&c, &addr, &["looking"]).await;
    let verified = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        serde_json::json!({}),
    )
    .await;
    assert!(verified.get("error").is_none(), "{verified}");

    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(
        row["standing"]["standing"], "verified",
        "{}",
        row["standing_says"]
    );
    assert_eq!(row["counts"]["tasks"], 15, "{}", row["counts"]);
    assert_eq!(row["counts"]["ticked"], 11);
    assert_eq!(row["counts"]["seen_by_pass"], 9);
    assert_eq!(
        row["counts"]["ticked_unsent"],
        serde_json::json!([
            "T010 Write the docs (FR-004)",
            "T011 Announce the release (FR-004)"
        ]),
        "ticked by hand, sent to nobody"
    );
    assert_eq!(row["counts"]["sent_unticked"], serde_json::json!([]));
    assert_eq!(row["counts_says"], "11 ticked · 9 seen by a passing check");
    let rows: Vec<(String, u64, u64, Value)> = row["token_rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["token"].as_str().unwrap().to_string(),
                r["tasks"].as_u64().unwrap(),
                r["ticked"].as_u64().unwrap(),
                r["seen_by_pass"].clone(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("FR-001".into(), 4, 3, serde_json::json!(3)),
            ("FR-002".into(), 3, 3, serde_json::json!(3)),
            ("FR-003".into(), 4, 3, serde_json::json!(3)),
            ("FR-004".into(), 4, 2, serde_json::json!(0)),
        ]
    );
    assert_eq!(
        row["token_rows"][3]["says"],
        "4 tasks · 2 ticked · 0 seen by a passing check"
    );
    let specs = get(&c, &addr, "/api/specs").await;
    let plan = specs["projects"][0]["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["change_id"] == id)
        .expect("the plan the change works to");
    assert_eq!(plan["counts"]["ticked"], 11, "{plan}");
    assert_eq!(plan["counts"]["seen_by_pass"], 9);
    assert_eq!(plan["token_rows"][0]["token"], "FR-001");

    // No gates declared is different from zero.
    std::fs::write(repo.join("devplane.toml"), "[project]\n").unwrap();
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert!(row["counts"]["seen_by_pass"].is_null(), "{}", row["counts"]);
    assert_eq!(row["counts"]["ticked"], 11);
    assert_eq!(row["counts_says"], "11 ticked · no gates declared");
    assert_eq!(
        row["token_rows"][0]["says"],
        "4 tasks · 3 ticked · no gates declared"
    );

    let body = serde_json::to_string(&row).unwrap();
    assert!(
        !body.contains('%'),
        "a percentage reached the change view: {body}"
    );
    for (k, v) in row.as_object().unwrap() {
        if let Some(text) = v.as_str() {
            assert!(
                !text.contains(" of ") || k == "standing_says" || k == "stopped_summary",
                "{k} reads as a fraction: {text}"
            );
        }
    }

    std::fs::remove_dir_all(&repo).ok();
}

async fn await_item(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    want: impl Fn(&Value) -> bool,
) -> Option<Value> {
    let deadline = std::time::Instant::now() + SETTLE;
    while std::time::Instant::now() < deadline {
        let inbox = get(c, addr, "/api/inbox").await;
        if let Some(hit) = inbox["items"]
            .as_array()
            .and_then(|items| items.iter().find(|i| want(i)))
        {
            return Some(hit.clone());
        }
        tokio::time::sleep(POLL).await;
    }
    None
}

/// Asks the echo agent's question on the counts repo, optionally edits the spec
/// while it waits, answers, and returns the rested change and run id.
async fn run_with_a_question(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    repo: &Path,
    agent: &str,
    title: &str,
    edit: Option<&str>,
) -> (Value, String) {
    // Commit an earlier edit: a dirty checkout refuses a worktree.
    for args in [&["add", "-A"][..], &["commit", "-qm", "amended"][..]] {
        std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git");
    }
    let v = post(
        c,
        addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": title,
            "prompt": "question",
            "agent": agent,
            "spec": "specs/007-counts",
        }),
    )
    .await;
    assert!(v.get("error").is_none(), "{v}");
    let id = v["change_id"].as_str().unwrap().to_string();
    let run = v["change"]["runs"][0].as_str().unwrap().to_string();
    let asked = await_item(c, addr, |i| {
        i["kind"] == "question" && i["run_id"] == run.as_str()
    })
    .await
    .expect("the agent's question reaches the inbox");
    if let Some(line) = edit {
        // The spec is edited in the person's checkout while the run waits.
        let spec = repo.join("specs/007-counts/spec.md");
        let mut text = std::fs::read_to_string(&spec).unwrap();
        text.push_str(line);
        std::fs::write(&spec, text).unwrap();
    }
    let ask = asked["ask"].as_str().expect("answerable").to_string();
    let field = asked["form"][0]["field"]
        .as_str()
        .unwrap_or("question_0")
        .to_string();
    let option = asked["form"][0]["options"][0]["value"]
        .as_str()
        .unwrap_or("Keep it")
        .to_string();
    let answered = post(
        c,
        addr,
        &format!("/api/asks/{ask}/answer"),
        serde_json::json!({ "option": option, "field": field, "from": "test" }),
    )
    .await;
    assert_eq!(answered["open"], false, "{answered}");
    let settled = await_rest(c, addr, &["looking", "stopped"]).await;
    assert_eq!(settled["id"], id.as_str(), "the newest change is this one");
    (settled, run)
}

/// A spec edited during a run raises a drift item. Accepting moves the start;
/// telling resumes the run with the files named; an untouched spec raises nothing.
#[tokio::test]
async fn a_specification_edited_mid_run_raises_a_drift_the_person_can_accept_or_tell() {
    let Some(agent) = echo_agent() else { return };
    let repo = counts_repo("drift");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let (settled, run) = run_with_a_question(
        &c,
        &addr,
        &repo,
        &agent,
        "first",
        Some("- **FR-005** amended while the run waited\n"),
    )
    .await;
    let id = settled["id"].as_str().unwrap().to_string();
    let item = await_item(&c, &addr, |i| {
        i["kind"] == "spec_drifted" && i["run_id"] == run.as_str()
    })
    .await
    .expect("a drift reaches the inbox");
    let started = get(&c, &addr, &format!("/api/runs/{run}")).await["started_at"]
        .as_str()
        .unwrap()
        .parse::<jiff::Timestamp>()
        .unwrap();
    let since: jiff::Timestamp = item["since"].as_str().unwrap().parse().unwrap();
    assert!(since > started, "dated to the edit, not the run: {item}");
    assert!(
        item["title"]
            .as_str()
            .unwrap()
            .contains(&format!("into run {run} and the run never saw it")),
        "{}",
        item["title"]
    );
    assert_eq!(
        item["actions"],
        serde_json::json!(["tell_run", "accept_drift", "snooze"])
    );
    assert_eq!(item["change_id"], id.as_str());
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(row["drifts"][0]["run"], run.as_str(), "{}", row["drifts"]);
    assert_eq!(row["drifts"][0]["says"], item["title"]);
    let before = row["spec_at_start"].as_str().unwrap().to_string();

    let accepted = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/drift/accept"),
        serde_json::json!({ "run": run }),
    )
    .await;
    assert_eq!(accepted["accepted"], true, "{accepted}");
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_ne!(row["spec_at_start"].as_str().unwrap(), before);
    assert_eq!(row["spec_at_start"], accepted["fingerprint"]);
    assert!(
        row["drifts"].as_array().unwrap().is_empty(),
        "{}",
        row["drifts"]
    );
    let inbox = get(&c, &addr, "/api/inbox").await;
    assert!(
        !inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "spec_drifted"),
        "accepted and still raised: {}",
        inbox["items"]
    );
    let decisions = get(&c, &addr, "/api/decisions").await;
    let row = decisions
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "change:drift.accepted")
        .expect("the acceptance is on the record");
    assert_eq!(row["authority"], "person", "{row}");
    assert_eq!(row["subject"], run.as_str());
    // A second decision conflicts: the drift is resolved.
    let again = c
        .post(format!("http://{addr}/api/changes/{id}/drift/accept"))
        .bearer_auth("tok")
        .json(&serde_json::json!({ "run": run }))
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), 409);

    let (settled, run) = run_with_a_question(
        &c,
        &addr,
        &repo,
        &agent,
        "second",
        Some("- **FR-006** amended again\n"),
    )
    .await;
    let id = settled["id"].as_str().unwrap().to_string();
    await_item(&c, &addr, |i| {
        i["kind"] == "spec_drifted" && i["run_id"] == run.as_str()
    })
    .await
    .expect("the second drift reaches the inbox");
    let told = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/drift/tell"),
        serde_json::json!({ "run": run }),
    )
    .await;
    assert_eq!(told["told"], true, "{told}");
    assert_eq!(told["run"], run.as_str());
    let worktree = PathBuf::from(settled["worktree"].as_str().unwrap());
    let deadline = std::time::Instant::now() + SETTLE;
    let mut heard = String::new();
    while std::time::Instant::now() < deadline {
        heard = std::fs::read_to_string(worktree.join(".devplane/heard.log")).unwrap_or_default();
        if heard.contains("while you were working") {
            break;
        }
        tokio::time::sleep(POLL).await;
    }
    // The agent cannot see the change from its worktree, so it is told the text.
    assert!(
        heard.contains("The specification `specs/007-counts` changed in the person's checkout")
            && heard.contains("`specs/007-counts/spec.md` now reads:")
            && heard.contains("FR-006** amended again"),
        "the run was not given what moved: {heard}"
    );
    let inbox = get(&c, &addr, "/api/inbox").await;
    assert!(
        !inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "spec_drifted"),
        "told and still raised: {}",
        inbox["items"]
    );
    let decisions = get(&c, &addr, "/api/decisions").await;
    assert!(
        decisions
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["action"] == "change:drift.told" && d["authority"] == "person"),
        "{decisions}"
    );
    // The resumed run rests with nothing raised.
    await_rest(&c, &addr, &["looking", "stopped"]).await;
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert!(
        row["drifts"].as_array().unwrap().is_empty(),
        "{}",
        row["drifts"]
    );

    // Nothing moved, nothing raised.
    let (settled, run) = run_with_a_question(&c, &addr, &repo, &agent, "third", None).await;
    let id = settled["id"].as_str().unwrap().to_string();
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert!(
        row["drifts"].as_array().unwrap().is_empty(),
        "{}",
        row["drifts"]
    );
    let inbox = get(&c, &addr, "/api/inbox").await;
    assert!(
        !inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "spec_drifted" && i["run_id"] == run.as_str()),
        "{}",
        inbox["items"]
    );

    std::fs::remove_dir_all(&repo).ok();
}

/// An "allow always" grant is logged with its scope, since later calls it covers
/// never reach Devplane ([arXiv:2606.22504](https://arxiv.org/abs/2606.22504)).
#[test]
fn a_standing_grant_is_recorded_as_one() {
    use devplane::core::{Authority, Decision};

    let once = Decision::new(
        Authority::Person,
        "agent:tool.use",
        "Bash: pnpm test",
        "allow",
    );
    let always = Decision::new(
        Authority::Person,
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

/// `explain` separates "no rules", "rules unparsable" (fails closed), and
/// "rules loaded".
#[test]
fn explain_says_why_nothing_answered() {
    let dir = std::env::temp_dir().join(format!(
        "vp-explain-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();

    let explain = |dir: &std::path::Path| -> Value {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .args(["explain", "--json", "--dir"])
            .arg(dir)
            .arg("cat .env")
            .env("DEVPLANE_HOME", &home)
            .output()
            .expect("explain runs");
        serde_json::from_slice(&out.stdout).expect("explain answers with JSON")
    };

    let r = explain(&dir);
    assert_eq!(r["verdict"], "undecided");
    assert_eq!(r["rules"]["state"], "none");

    // 2. An unparsable file: no rules are in force.
    std::fs::write(
        dir.join("devplane.toml"),
        "[project]\nname=\"x\"\n[polcy]\nnever_auto=[\"Read(.env)\"]\n",
    )
    .unwrap();
    let r = explain(&dir);
    // Fails closed: `unresolved`, naming the file, never `undecided`.
    assert_eq!(r["verdict"], "unresolved");
    assert!(r["why"].as_str().unwrap().contains("devplane.toml"), "{r}");
    assert_eq!(r["rules"]["state"], "broken");
    assert_eq!(r["rules"]["in_force"], false);
    assert!(
        r["rules"]["error"].as_str().unwrap().contains("polcy"),
        "it has to name what is wrong, like `devplane check` does"
    );

    // 3. Rules that load, found without a git repository above them.
    std::fs::write(
        dir.join("devplane.toml"),
        "[project]\nname=\"x\"\n[policy]\nnever_auto=[\"Read(.env)\"]\n",
    )
    .unwrap();
    let r = explain(&dir);
    assert_eq!(r["verdict"], "deny");
    assert_eq!(r["rule"], "Read(.env)");
    assert_eq!(r["rules"]["state"], "loaded");
    assert_eq!(r["rules"]["in_force"], true);

    std::fs::remove_dir_all(&dir).ok();
}

/// A change's files against the project's base, uncommitted work included.
async fn change_set_of(repo: &Path, tree: &Path) -> Value {
    let base = devplane::view::base_of(Some(repo), tree).await;
    serde_json::json!({ "changes": devplane::git::change_set(tree, &base).await.unwrap() })
}

/// The change set keeps apart: a real change, a branch that changed nothing,
/// and a file with no lines to show.
#[tokio::test]
async fn the_change_set_tells_its_answers_apart() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("changes", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "show me the change",
            "agent": agent,
        }),
    )
    .await;
    let worktree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    await_rest(&c, &addr, &["stopped", "looking"]).await;

    // 2. A branch that changed nothing is a finding, not an empty state.
    let body = change_set_of(&repo, &worktree).await;
    assert!(
        body["changes"]["base"].is_string(),
        "an empty change set still says what it was compared against, or the finding \
         cannot be stated: {body}"
    );

    // 1. A real change is read back.
    std::fs::write(worktree.join("reviewed.txt"), "one\ntwo\n").unwrap();
    let body = change_set_of(&repo, &worktree).await;
    let files = body["changes"]["files"].as_array().unwrap();
    assert!(
        files.iter().any(|f| f["path"] == "reviewed.txt"),
        "uncommitted work counts — a reviewer approves the checkout as it stands: {body}"
    );

    // A binary file has no lines, so its size is all the view can show.
    std::fs::write(worktree.join("logo.bin"), [0u8, 1, 2, 255, 0, b'x']).unwrap();
    let body = change_set_of(&repo, &worktree).await;
    let logo = body["changes"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "logo.bin")
        .unwrap_or_else(|| panic!("the binary file is not in the set: {body}"));
    assert_eq!(
        logo["body"]["binary"]["bytes"], 6,
        "a binary file carries its size, so the surface can name it rather than \
         rendering broken text: {body}"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn a_graceful_stop_records_interrupted_and_never_completed() {
    // A driven run cut off by shutdown is recorded as interrupted, never `completed`.
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("gracefulstop", "escalate", 0);
    let db = std::env::temp_dir().join(format!("vp-stop-{}.db", uuid::Uuid::new_v4().simple()));
    let _serial = common::one_agent_at_a_time();

    let run_id = {
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
        let server = tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        let c = reqwest::Client::new();
        trust(&c, &addr, &repo).await;
        let run_id = devplane::core::RunId::new(run_in(&state, &agent, &repo, None).await);
        assert!(
            state
                .world
                .lock()
                .await
                .run(&run_id)
                .unwrap()
                .state
                .is_live(),
            "the run is live before the stop, or this test proves nothing"
        );

        state.shutdown().await;

        // 1. Interrupted, not completed.
        assert_eq!(
            state.world.lock().await.run(&run_id).unwrap().state,
            devplane::core::RunState::Interrupted,
            "a host stopping is not the agent finishing"
        );
        // 2. `shutdown()` is quiescent: nothing changes after it returns.
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        assert_eq!(
            state.world.lock().await.run(&run_id).unwrap().state,
            devplane::core::RunState::Interrupted,
            "shutdown() returned with a write still in flight"
        );
        server.abort();
        run_id
    };

    // 3. A new host reads the same; `completed` would be invisible to reconciliation.
    let state = AppState::new(
        db.clone(),
        "tok".into(),
        Policy::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();
    assert_eq!(
        state.world.lock().await.run(&run_id).unwrap().state,
        devplane::core::RunState::Interrupted,
        "the store must not have promoted an interruption to a completion"
    );
    devplane::poller::reconcile_at_startup(&state).await;
    assert_eq!(
        state.world.lock().await.run(&run_id).unwrap().state,
        devplane::core::RunState::Interrupted,
        "reconciliation must leave a known ending alone"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// `devplane quit` from the CLI against a separate host process: it names what
/// it ends first, no Devplane process remains, and no agent is orphaned.
#[tokio::test]
#[cfg(unix)]
async fn quitting_from_the_command_line_names_the_agent_and_leaves_nothing_behind() {
    let Some(agent) = echo_agent() else { return };
    let _serial = common::one_agent_at_a_time();
    let repo = scratch_repo("quitcli", "feedback", 1);
    let home = std::env::temp_dir().join(format!(
        "vp-quit-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&home).unwrap();

    // Port 0: test binaries run in parallel.
    let mut host = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("serve")
        .arg("--port")
        .arg("0")
        .env("DEVPLANE_HOME", &home)
        .env("DEVPLANE_NOTIFY", "0")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("a host");

    // Wait for the published address, not a duration.
    let info_path = home.join("host.json");
    let mut info = None;
    for _ in 0..200 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Ok(text) = std::fs::read_to_string(&info_path)
            && let Ok(v) = serde_json::from_str::<Value>(&text)
        {
            info = Some(v);
            break;
        }
    }
    let info = info.expect("the host never published where it is listening");
    let port = info["port"].as_u64().expect("a port");
    let pid = info["pid"].as_u64().expect("a pid") as u32;
    let token = std::fs::read_to_string(home.join("token")).expect("a token");
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let c = reqwest::Client::new();
    let send = |path: String, body: Value| {
        let c = c.clone();
        let token = token.trim().to_string();
        async move {
            c.post(format!("http://{addr}{path}"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    let trusted = send(
        "/api/projects/trust".into(),
        serde_json::json!({ "path": repo.to_string_lossy() }),
    )
    .await;
    assert_eq!(trusted["trusted"], true, "{trusted}");

    // Pre-existing `echo_agent` pids, to tell this test's agent from leftovers.
    let echoes = || -> std::collections::HashSet<u32> {
        devplane::observe::procs::snapshot()
            .into_iter()
            .filter(|p| p.command.contains("echo_agent"))
            .map(|p| p.pid)
            .collect()
    };
    let before = echoes();

    let started = send(
        "/api/changes".into(),
        serde_json::json!({
            "agent": agent,
            "cwd": repo.to_string_lossy(),
            "title": "keep an agent running",
            "prompt": "slow",
            "worktree": false,
        }),
    )
    .await;
    let run_id = started["change"]["runs"][0]
        .as_str()
        .unwrap_or_else(|| panic!("a run: {started}"))
        .to_string();

    let mut agent_pid = None;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Some(new) = echoes().difference(&before).copied().next() {
            agent_pid = Some(new);
            break;
        }
    }
    let agent_pid = agent_pid.expect("the dispatch started no agent, so this proves nothing");
    // Otherwise an already-dead agent would pass property 3 vacuously.
    assert!(
        devplane::poller::process_alive(agent_pid),
        "the agent was not running before the quit, so its absence after proves nothing"
    );

    // Reap the host: an unreaped zombie still answers `kill(pid, 0)`.
    let mut guard = common::HostGuard::new(host.id());
    let reaper = std::thread::spawn(move || host.wait());

    // 1. It says what it ends, before it ends it.
    let quit = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("quit")
        .env("DEVPLANE_HOME", &home)
        .output()
        .expect("quit runs");
    let said = String::from_utf8_lossy(&quit.stdout).to_string();
    assert!(
        said.contains(&run_id),
        "the quit did not name the agent it was about to stop: {said}"
    );
    // Ordering is asserted in `tests/purity.rs`; from here it is undecidable.

    // 2. No Devplane process remains.
    assert!(
        said.contains("Stopped."),
        "the quit did not report the host as stopped: {said}"
    );
    let status = reaper.join().expect("the reaper thread").expect("wait");
    assert!(
        status.code().is_some(),
        "the host did not exit of its own accord: {status:?}"
    );
    assert!(
        !devplane::poller::process_alive(pid),
        "`Stopped.` was printed while the host (pid {pid}) was still running"
    );
    assert!(
        !info_path.exists(),
        "the host left its record behind, so the next start has a stale pid to guess about"
    );
    guard.disarm();

    // 3. No driven agent is orphaned.
    assert!(
        !devplane::poller::process_alive(agent_pid),
        "the agent (pid {agent_pid}) outlived the host that started it — re-parented to \
         init, with a subscription attached, and nothing left on the machine that knows it \
         is there"
    );

    std::fs::remove_dir_all(&repo).ok();
    std::fs::remove_dir_all(&home).ok();
}

// ── The Change: adopted, verified against the tree, archived, offered ────────

/// A hand-made branch with the fix and a spec folder, checked out nowhere.
fn hand_made_branch(repo: &Path, branch: &str) {
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["checkout", "-qb", branch]);
    std::fs::write(repo.join("fixed.txt"), "yes\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "make the gate pass"]);
    let spec = repo.join("specs/001-rate-limit");
    std::fs::create_dir_all(&spec).unwrap();
    std::fs::write(spec.join("spec.md"), "# Rate limit the login route\n").unwrap();
    std::fs::write(
        spec.join("tasks.md"),
        "- [x] add the limiter\n- [ ] document it\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "write the spec"]);
    git(&["checkout", "-q", "main"]);
}

async fn get(c: &reqwest::Client, addr: &std::net::SocketAddr, path: &str) -> Value {
    c.get(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn change_row(c: &reqwest::Client, addr: &std::net::SocketAddr, id: &str) -> Value {
    get(c, addr, "/api/changes")
        .await
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == id)
        .cloned()
        .expect("the change is listed")
}

/// Verified is computed from the tree: touching one file after a pass makes the
/// change unverified, with no flag written.
#[tokio::test]
async fn touching_a_file_makes_a_verified_change_unverified_with_no_flag_written() {
    let repo = scratch_repo("tree", "escalate", 1);
    hand_made_branch(&repo, "feat/by-hand");
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;

    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/by-hand", "project": repo.to_string_lossy() }),
    )
    .await;
    assert!(adopted["error"].is_null(), "{adopted}");
    let id = adopted["change_id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(adopted["change"]["worktree"].as_str().unwrap());
    assert_eq!(change_row(&c, &addr, &id).await["state"], "isolated");

    let verdict = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(verdict["passed"], true, "{verdict}");
    let row = change_row(&c, &addr, &id).await;
    assert_eq!(row["state"], "verified", "{row}");
    assert_eq!(row["glyph"], "✓");
    assert_eq!(row["standing"]["standing"], "verified");

    // No event announces it; the tick reads it.
    std::fs::write(worktree.join("notes.txt"), "an edit after the gates\n").unwrap();
    devplane::poller::tree_tick(&state).await;
    let row = change_row(&c, &addr, &id).await;
    assert_ne!(row["state"], "verified", "{row}");
    assert_eq!(row["standing"]["standing"], "stale", "{row}");
    let says = row["standing_says"].as_str().unwrap();
    assert!(
        says.contains("gates passed, stale") && says.contains("changed after"),
        "{says}"
    );
    // The record is unchanged: no field says *verified*.
    assert_eq!(row["gate"]["passed"], true);
    assert!(row.get("verified").is_none());

    // The pure rule: verified against the stamp it ran on, not against the current one.
    let record = state
        .changes
        .lock()
        .await
        .get(&devplane::core::ChangeId::new(id.clone()))
        .cloned()
        .unwrap();
    let then = record.gates.last().and_then(|g| g.commit.clone());
    let ran: Vec<String> = record
        .check_report()
        .map(|g| g.commands.iter().map(|c| c.command.clone()).collect())
        .unwrap_or_default();
    let declared = devplane::core::change::Declared::Checks(&ran);
    assert_eq!(
        record.state(declared, then.as_ref()),
        devplane::core::ChangeState::Verified
    );
    assert_ne!(
        record.state(declared, record.tree_now.as_ref()),
        devplane::core::ChangeState::Verified
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// A passing `check` over uncommitted work is verified; the digest is the
/// working tree's, reproducible with a temporary index.
#[tokio::test]
async fn uncommitted_work_that_passes_check_is_verified() {
    let repo = scratch_repo("uncommitted", "escalate", 1);
    hand_made_branch(&repo, "feat/uncommitted");
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/uncommitted", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = adopted["change_id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(adopted["change"]["worktree"].as_str().unwrap());
    std::fs::write(worktree.join("agent.txt"), "written, not committed\n").unwrap();

    let verdict = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(verdict["passed"], true, "{verdict}");
    assert_eq!(verdict["report"]["commit"]["clean"], false, "{verdict}");
    let stamped = verdict["report"]["commit"]["tree"]
        .as_str()
        .expect("a digest");
    assert_eq!(stamped, tree_by_hand(&worktree));
    assert_ne!(
        stamped,
        git_out(&worktree, &["rev-parse", "HEAD^{tree}"]).trim()
    );

    devplane::poller::tree_tick(&state).await;
    let row = change_row(&c, &addr, &id).await;
    assert_eq!(row["state"], "verified", "{row}");
    assert_eq!(row["standing"]["standing"], "verified", "{row}");
    std::fs::remove_dir_all(&repo).ok();
}

/// Only `check` verifies; a named gate passing after a failed `check` does not.
#[test]
fn a_named_gate_after_a_failing_check_is_not_verified() {
    use devplane::core::change::{
        CommandResult, CommitStamp, Declared, GateReport, Reach, Standing,
    };
    let declared = ["x".to_string()];
    let declared = Declared::Checks(&declared);
    let stamp = CommitStamp {
        commit: Some("c0ffee".into()),
        tree: Some("7ree".into()),
        branch: Some("b".into()),
        clean: false,
        changed_files: 1,
        reach: Reach::NoRemote,
        remote: None,
    };
    let report = |gate: &str, code: i32| GateReport {
        gate: gate.into(),
        at: jiff::Timestamp::now(),
        duration_ms: 0,
        commands: vec![CommandResult::exited("x", code)],
        attempt: 1,
        spec: None,
        commit: Some(stamp.clone()),
    };
    let mut change =
        devplane::core::Change::new(devplane::core::ProjectId::new("p"), "t".into(), "t".into());
    change.gates = vec![report("check", 1), report("lint", 0)];
    assert!(matches!(
        change.verdict(declared, Some(&stamp)),
        Standing::Failed { .. }
    ));
    assert_eq!(
        change.last_pass_at(),
        None,
        "a named pass is no pass of check"
    );
    assert!(!devplane::core::change::Completion::of(&change, declared, Some(&stamp)).is_checked());

    change.gates.push(report("check", 0));
    assert_eq!(change.verdict(declared, Some(&stamp)), Standing::Verified);
}

fn tree_by_hand(dir: &Path) -> String {
    let tmp = std::env::temp_dir().join(format!(
        "devplane-test-index-{}-{}",
        std::process::id(),
        jiff::Timestamp::now().as_nanosecond()
    ));
    let run = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_INDEX_FILE", &tmp)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    run(&["read-tree", "HEAD"]);
    run(&["add", "-A"]);
    let tree = run(&["write-tree"]);
    std::fs::remove_file(&tmp).ok();
    tree
}

/// An adopted branch yields the same certificate shape as a started one,
/// with its history read from git.
#[tokio::test]
async fn an_adopted_branch_certifies_exactly_like_a_started_one() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("adopt", "escalate", 1);
    hand_made_branch(&repo, "feat/by-hand");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    // Refused: a branch that is not there, and the base itself.
    for (branch, why) in [("feat/nope", "no branch"), ("main", "base branch")] {
        let refused = post(
            &c,
            &addr,
            "/api/changes/adopt",
            serde_json::json!({ "branch": branch, "project": repo.to_string_lossy() }),
        )
        .await;
        assert!(
            refused["error"].as_str().unwrap_or("").contains(why),
            "{branch}: {refused}"
        );
    }

    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({
            "branch": "feat/by-hand",
            "project": repo.to_string_lossy(),
            "spec": "specs/001-rate-limit",
        }),
    )
    .await;
    assert!(adopted["error"].is_null(), "{adopted}");
    let a = &adopted["change"];
    assert_eq!(
        a["title"], "make the gate pass",
        "the first commit's subject"
    );
    assert_eq!(a["branch"], "feat/by-hand");
    assert_eq!(a["prompt"], "");
    assert_eq!(a["runs"].as_array().unwrap().len(), 0);
    assert_eq!(a["spec"], "specs/001-rate-limit");
    let worktree = PathBuf::from(a["worktree"].as_str().unwrap());
    assert!(
        worktree.starts_with(repo.join(".claude/worktrees")),
        "{}",
        worktree.display()
    );
    let id = adopted["change_id"].as_str().unwrap().to_string();

    // Adopting twice is one change.
    let again = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/by-hand", "project": repo.to_string_lossy() }),
    )
    .await;
    assert!(
        again["error"].as_str().unwrap_or("").contains("already"),
        "{again}"
    );

    let verdict = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(verdict["passed"], true, "{verdict}");
    post(&c, &addr, &format!("/api/changes/{id}/finish"), Value::Null).await;
    let adopted_cert = get(&c, &addr, &format!("/api/changes/{id}/certificate")).await;
    assert_eq!(adopted_cert["finished"], true, "{adopted_cert}");
    assert!(
        adopted_cert["statement"]["subject"][0]["digest"]["gitCommit"].is_string(),
        "the history is git's: {adopted_cert}"
    );
    assert_eq!(
        adopted_cert["statement"]["predicate"]["specification"]["tasks_total"], 2,
        "the specification travelled onto the certificate: {adopted_cert}"
    );

    // The same, from a started change; its spec is committed on `main` first.
    let spec_on_main = repo.join("specs/001-rate-limit");
    std::fs::create_dir_all(&spec_on_main).unwrap();
    std::fs::write(
        spec_on_main.join("spec.md"),
        "# Rate limit the login route\n",
    )
    .unwrap();
    std::fs::write(
        spec_on_main.join("tasks.md"),
        "- [x] add the limiter\n- [ ] document it\n",
    )
    .unwrap();
    for args in [vec!["add", "-A"], vec!["commit", "-qm", "the spec on main"]] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "started here",
            "agent": agent,
            "spec": "specs/001-rate-limit",
        }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let sid = started["change_id"].as_str().unwrap().to_string();
    let w = await_rest(&c, &addr, &["stopped", "looking", "finished"]).await;
    assert_eq!(w["id"], sid);
    let swt = PathBuf::from(w["worktree"].as_str().unwrap());
    std::fs::write(swt.join("fixed.txt"), "yes\n").unwrap();
    for args in [vec!["add", "-A"], vec!["commit", "-qm", "fix it"]] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(&swt)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let verdict = post(
        &c,
        &addr,
        &format!("/api/changes/{sid}/verify"),
        Value::Null,
    )
    .await;
    assert_eq!(verdict["passed"], true, "{verdict}");
    post(
        &c,
        &addr,
        &format!("/api/changes/{sid}/finish"),
        Value::Null,
    )
    .await;
    let started_cert = get(&c, &addr, &format!("/api/changes/{sid}/certificate")).await;
    assert_eq!(started_cert["finished"], true, "{started_cert}");

    fn keys(v: &Value) -> Vec<String> {
        let mut k: Vec<String> = v
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        k.sort();
        k
    }
    assert_eq!(keys(&adopted_cert), keys(&started_cert));
    assert_eq!(
        keys(&adopted_cert["statement"]),
        keys(&started_cert["statement"])
    );
    assert_eq!(
        keys(&adopted_cert["statement"]["predicate"]),
        keys(&started_cert["statement"]["predicate"])
    );
    assert_eq!(
        keys(&adopted_cert["statement"]["predicate"]["evidence"]),
        keys(&started_cert["statement"]["predicate"]["evidence"])
    );
    assert_eq!(keys(&adopted_cert["page"]), keys(&started_cert["page"]));
    assert_eq!(
        adopted_cert["statement"]["predicate"]["completion"]["basis"],
        started_cert["statement"]["predicate"]["completion"]["basis"],
        "both rest on the same basis"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// Archiving removes the worktree but keeps the record; the branch stays
/// unless asked, and is deleted only when fully merged.
#[tokio::test]
async fn archiving_keeps_the_record_and_removes_the_worktree() {
    let repo = scratch_repo("archive", "escalate", 1);
    hand_made_branch(&repo, "feat/by-hand");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/by-hand", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = adopted["change_id"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(adopted["change"]["worktree"].as_str().unwrap());
    post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    post(&c, &addr, &format!("/api/changes/{id}/finish"), Value::Null).await;

    // Unmerged: branch deletion is refused and nothing changes.
    let refused = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive?delete_branch=true"),
        Value::Null,
    )
    .await;
    assert!(
        refused["error"].as_str().unwrap_or("").contains("commit"),
        "{refused}"
    );
    assert!(worktree.exists(), "a refusal removed the worktree");

    // Without `delete_branch` the worktree goes and the branch stays.
    let done = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive"),
        Value::Null,
    )
    .await;
    assert_eq!(done["ok"], true, "{done}");
    assert_eq!(done["worktree_removed"], true);
    assert_eq!(done["branch_deleted"], false);
    assert!(!worktree.exists(), "the worktree is gone");
    let branches = std::process::Command::new("git")
        .args(["branch", "--list", "feat/by-hand"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&branches.stdout).contains("feat/by-hand"),
        "the branch was deleted without being asked"
    );

    let row = change_row(&c, &addr, &id).await;
    assert_eq!(row["state"], "archived", "{row}");
    assert_eq!(row["glyph"], "▣");
    assert!(row["archived_at"].is_string());
    assert_eq!(row["gates"].as_array().unwrap().len(), 1);
    assert!(row["completion"].is_object());
    let cert = get(&c, &addr, &format!("/api/changes/{id}/certificate")).await;
    assert_eq!(
        cert["finished"], true,
        "the certificate outlives the worktree: {cert}"
    );
    let log = get(&c, &addr, &format!("/api/decisions?about={id}")).await;
    assert!(
        log.as_array()
            .unwrap()
            .iter()
            .any(|d| d["action"] == "change:archive"),
        "{log}"
    );
    let again = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive"),
        Value::Null,
    )
    .await;
    assert!(
        again["error"]
            .as_str()
            .unwrap_or("")
            .contains("archived at"),
        "{again}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// Offering without `[github] pull_request` pushes nothing and returns the
/// push command and GitHub's own compare page; the inbox never offers it, and
/// leads with its review. No other tool is named.
#[tokio::test]
async fn offering_without_permission_hands_back_the_commands_and_pushes_nothing() {
    let repo = scratch_repo("offer", "escalate", 1);
    hand_made_branch(&repo, "feat/by-hand");
    let remote = std::process::Command::new("git")
        .args(["remote", "add", "origin", "git@github.com:acme/app.git"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(remote.status.success());
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/by-hand", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = adopted["change_id"].as_str().unwrap().to_string();
    post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;

    // Verified: the inbox leads with the review and never offers.
    let inbox = get(&c, &addr, "/api/inbox").await;
    let ready = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "ready_to_decide" && i["change_id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("no ready row: {inbox}"));
    let acts = ready["actions"].as_array().unwrap();
    assert_eq!(acts[0], "review", "{ready}");
    assert!(!acts.iter().any(|a| a == "offer"), "{ready}");
    assert!(change_row(&c, &addr, &id).await["pull_request"].is_null());

    let offer = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(offer["offer"], "commands", "{offer}");
    let push = offer["push"].as_str().unwrap();
    let create = offer["create"].as_str().unwrap();
    assert!(
        push.starts_with("git ") && push.contains("push") && push.contains("feat/by-hand"),
        "{push}"
    );
    assert!(
        create.starts_with("https://github.com/acme/app/compare/")
            && create.ends_with("...feat/by-hand?expand=1"),
        "the compare page for the branch, a link a person opens: {create}"
    );
    assert!(!create.contains("gh "), "no other tool is named: {create}");
    let row = change_row(&c, &addr, &id).await;
    assert!(row["pull_request"].is_null(), "nothing was opened: {row}");
    assert_eq!(row["state"], "verified");
    std::fs::remove_dir_all(&repo).ok();
}

/// A killed host's `working` run reads as ended by the host stopping, not
/// finished, and the next host starts with the stale discovery record cleared.
#[cfg(unix)]
#[tokio::test]
async fn a_killed_host_is_recovered_by_the_next_one() {
    let Some(agent) = echo_agent() else { return };
    let _serial = common::one_agent_at_a_time();
    let repo = scratch_repo("killed", "feedback", 1);
    let home = std::env::temp_dir().join(format!(
        "vp-kill-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&home).unwrap();

    let spawn_host = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("serve")
            .arg("--port")
            .arg("0")
            .env("DEVPLANE_HOME", &home)
            .env("DEVPLANE_NOTIFY", "0")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("a host")
    };
    let info_path = home.join("host.json");
    // Wait for a port other than `not`: the killed host's record is still on disk.
    async fn published(info_path: &Path, not: Option<u64>) -> Value {
        for _ in 0..200 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Ok(text) = std::fs::read_to_string(info_path)
                && let Ok(v) = serde_json::from_str::<Value>(&text)
                && v["port"].as_u64() != not
            {
                return v;
            }
        }
        panic!("the host never published where it is listening");
    }

    let mut first = spawn_host();
    let mut first_guard = common::HostGuard::new(first.id());
    let info = published(&info_path, None).await;
    let port = info["port"].as_u64().expect("a port");
    let token = std::fs::read_to_string(home.join("token"))
        .expect("a token")
        .trim()
        .to_string();
    let c = reqwest::Client::new();
    let post_to = |port: u64, path: String, body: Value| {
        let c = c.clone();
        let token = token.clone();
        async move {
            c.post(format!("http://127.0.0.1:{port}{path}"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let trusted = post_to(
        port,
        "/api/projects/trust".into(),
        serde_json::json!({ "path": repo.to_string_lossy() }),
    )
    .await;
    assert_eq!(trusted["trusted"], true, "{trusted}");

    let echoes = || -> std::collections::HashSet<u32> {
        devplane::observe::procs::snapshot()
            .into_iter()
            .filter(|p| p.command.contains("echo_agent"))
            .map(|p| p.pid)
            .collect()
    };
    let before = echoes();
    let started = post_to(
        port,
        "/api/changes".into(),
        serde_json::json!({
            "agent": agent,
            "cwd": repo.to_string_lossy(),
            "title": "keep an agent running",
            "prompt": "slow",
            "worktree": false,
        }),
    )
    .await;
    let run_id = started["change"]["runs"][0]
        .as_str()
        .unwrap_or_else(|| panic!("a run: {started}"))
        .to_string();
    let mut agent_pid = None;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Some(new) = echoes().difference(&before).copied().next() {
            agent_pid = Some(new);
            break;
        }
    }
    let agent_pid = agent_pid.expect("the dispatch started no agent, so this proves nothing");

    let pid = info["pid"].as_u64().expect("a pid") as u32;
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    first.wait().expect("reaped");
    first_guard.disarm();
    assert!(
        info_path.exists(),
        "a killed host cannot clear its record; the next start has to"
    );
    // The orphaned agent is the harness's to end.
    unsafe { libc::kill(agent_pid as i32, libc::SIGKILL) };

    let mut second = spawn_host();
    let mut second_guard = common::HostGuard::new(second.id());
    let info = published(&info_path, Some(port)).await;
    let port = info["port"].as_u64().expect("a port");
    let run = c
        .get(format!("http://127.0.0.1:{port}/api/runs/{run_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let state = run["state"].as_str().unwrap_or("").to_string();
    assert!(
        state != "working" && state != "completed",
        "a run the killed host was driving reads `{state}`: {run}"
    );
    let ended_by_host = ["lost", "interrupted"].contains(&state.as_str());
    assert!(
        ended_by_host,
        "the ending does not name the host stopping: {state}"
    );

    let _ = c
        .post(format!("http://127.0.0.1:{port}/api/quit"))
        .bearer_auth(&token)
        .send()
        .await;
    for _ in 0..300 {
        if let Ok(Some(_)) = second.try_wait() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let _ = second.kill();
    let _ = second.wait();
    second_guard.disarm();
    std::fs::remove_dir_all(&repo).ok();
}

// ── A tree of its own ────────────────────────────────────────────────────

/// Waits for one change, by id, to come to rest.
async fn await_rest_of(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    id: &str,
    want: &[&str],
) -> Value {
    let deadline = std::time::Instant::now() + SETTLE;
    let mut last = Value::Null;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(POLL).await;
        let row = change_row(c, addr, id).await;
        if want.contains(&resting(&row)) {
            return row;
        }
        last = row;
    }
    panic!("change {id} never reached {want:?}: {last}");
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn worktrees_of(repo: &Path) -> Vec<String> {
    git_out(repo, &["worktree", "list", "--porcelain"])
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(str::to_string)
        .collect()
}

fn branches_of(repo: &Path) -> Vec<String> {
    git_out(repo, &["branch", "--list", "--format=%(refname:short)"])
        .lines()
        .map(str::to_string)
        .collect()
}

/// Every file in the person's checkout (except `.git` and `.claude`), by path and content.
fn digest_of(repo: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !(dir == root && (name == ".git" || name == ".claude")) {
                    walk(root, &p, out);
                }
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.push((rel, std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(repo, repo, &mut out);
    out
}

async fn start_change(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    repo: &Path,
    title: &str,
    agent: &str,
) -> Value {
    post(
        c,
        addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": title,
            "agent": agent,
        }),
    )
    .await
}

/// Two concurrent agents in one project don't collide, and the checkout is byte-identical.
#[tokio::test]
async fn two_agents_share_a_project_and_the_persons_checkout_is_untouched() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("two", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let before = digest_of(&repo);

    let (one, two) = tokio::join!(
        start_change(&c, &addr, &repo, "one thing", &agent),
        start_change(&c, &addr, &repo, "another thing", &agent)
    );
    assert!(one["error"].is_null(), "{one}");
    assert!(two["error"].is_null(), "{two}");
    let ids = [
        one["change_id"].as_str().unwrap().to_string(),
        two["change_id"].as_str().unwrap().to_string(),
    ];
    let trees = [
        PathBuf::from(one["change"]["worktree"].as_str().unwrap()),
        PathBuf::from(two["change"]["worktree"].as_str().unwrap()),
    ];
    let branches = [
        one["change"]["branch"].as_str().unwrap().to_string(),
        two["change"]["branch"].as_str().unwrap().to_string(),
    ];

    // (a) its own directory, under the vendor's convention; (b) its own branch.
    assert_ne!(trees[0], trees[1]);
    for t in &trees {
        assert!(
            t.starts_with(repo.join(".claude/worktrees")),
            "{}",
            t.display()
        );
        assert!(t.is_dir());
    }
    assert_ne!(branches[0], branches[1]);

    for id in &ids {
        await_rest_of(&c, &addr, id, &["stopped", "looking"]).await;
    }

    // (c) The person's checkout is byte-identical.
    assert_eq!(
        digest_of(&repo),
        before,
        "an agent wrote inside the checkout"
    );

    // (d) Each diff names only its change's files; the fixture commits nothing, so commit one.
    for (i, (tree, name)) in trees.iter().zip(["one.txt", "two.txt"]).enumerate() {
        std::fs::write(tree.join(name), "work\n").unwrap();
        git_out(tree, &["add", name]);
        git_out(tree, &["commit", "-qm", &format!("add {name}")]);
        let diff = change_set_of(&repo, tree).await;
        let files: Vec<&str> = diff["changes"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f["path"].as_str())
            .collect();
        assert!(files.contains(&name), "{diff}");
        let other = if i == 0 { "two.txt" } else { "one.txt" };
        assert!(
            !files.contains(&other),
            "the other agent's file leaked in: {diff}"
        );

        // (e) Reproducible by hand: three-dot diff from the base in the worktree.
        let base = diff["changes"]["base"].as_str().expect("the base is named");
        let by_hand = git_out(tree, &["diff", "--name-only", &format!("{base}...HEAD")]);
        let by_hand: Vec<&str> = by_hand.lines().collect();
        assert!(by_hand.contains(&name), "{by_hand:?}");
        for f in &by_hand {
            assert!(
                files.contains(f),
                "`git diff {base}...HEAD` names {f}, the route does not"
            );
        }

        // (f) The certificate's tree digest is of this change's own worktree.
        let verdict = post(
            &c,
            &addr,
            &format!("/api/changes/{}/verify", ids[i]),
            Value::Null,
        )
        .await;
        let stamped = verdict["report"]["commit"]["tree"]
            .as_str()
            .expect("the gate stamps the tree it ran against");
        assert_eq!(stamped, tree_by_hand(tree), "the working tree's digest");
    }
    assert!(
        worktrees_of(&repo).len() == 3,
        "the main tree and two worktrees: {:?}",
        worktrees_of(&repo)
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// The agent's log appears under the worktree and never under the checkout.
#[tokio::test]
async fn an_agent_never_writes_inside_the_persons_checkout() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("nowrite", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let started = start_change(&c, &addr, &repo, "say something", &agent).await;
    let id = started["change_id"].as_str().unwrap().to_string();
    let tree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    await_rest_of(&c, &addr, &id, &["stopped", "looking"]).await;

    assert!(
        tree.join(".devplane/heard.log").is_file(),
        "the agent ran in the worktree"
    );
    assert!(
        !repo.join(".devplane").exists(),
        "the agent wrote inside the person's checkout"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// Every refusal isolation makes, planted, and nothing written on the way.
#[tokio::test]
async fn isolation_refuses_before_anything_is_written() {
    let (addr, c, _state) = boot().await;
    let untouched = |repo: &Path| {
        assert_eq!(
            worktrees_of(repo).len(),
            1,
            "a worktree was made on the way to refusing"
        );
        assert_eq!(
            branches_of(repo),
            vec!["main".to_string()],
            "a branch was made"
        );
        assert!(
            !repo.join(".claude/worktrees").exists() || {
                std::fs::read_dir(repo.join(".claude/worktrees"))
                    .map(|d| d.count() == 0)
                    .unwrap_or(true)
            }
        );
    };

    // A dirty tracked file is named.
    let dirty = scratch_repo("refuse-dirty", "escalate", 0);
    trust(&c, &addr, &dirty).await;
    std::fs::write(dirty.join("check.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    let res = start_change(&c, &addr, &dirty, "x", "claude").await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("check.sh"), "{err}");
    assert!(err.contains("uncommitted"), "{err}");
    untouched(&dirty);

    // A base branch that does not exist is named.
    let nobase = scratch_repo("refuse-base", "escalate", 0);
    std::fs::write(
        nobase.join("devplane.toml"),
        "[project]\nbase_branch = \"nope\"\n",
    )
    .unwrap();
    git_out(
        &nobase,
        &["commit", "-qam", "point at a branch that is not there"],
    );
    trust(&c, &addr, &nobase).await;
    let res = start_change(&c, &addr, &nobase, "x", "claude").await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("`nope`"), "{err}");
    assert!(err.contains("does not exist"), "{err}");
    untouched(&nobase);

    // An existing target is named; only an adopted branch's target is plantable.
    let taken = scratch_repo("refuse-taken", "escalate", 0);
    hand_made_branch(&taken, "feat/taken");
    std::fs::create_dir_all(taken.join(".claude/worktrees/feat-taken")).unwrap();
    trust(&c, &addr, &taken).await;
    let res = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/taken", "project": taken.to_string_lossy() }),
    )
    .await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("already exists and is a directory"), "{err}");
    assert_eq!(worktrees_of(&taken).len(), 1);

    // An untrusted project names the trust step.
    let untrusted = scratch_repo("refuse-trust", "escalate", 0);
    let res = start_change(&c, &addr, &untrusted, "x", "claude").await;
    let err = res["error"].as_str().unwrap_or_default();
    assert!(err.contains("devplane trust"), "{err}");
    untouched(&untrusted);

    for r in [dirty, nobase, taken, untrusted] {
        std::fs::remove_dir_all(&r).ok();
    }
}

// ── The first run does not look like a hang ──────────────────────────────

/// The row `/api/changes/preflight` reports for one repository.
async fn preflight_row(c: &reqwest::Client, addr: &std::net::SocketAddr, repo: &Path) -> Value {
    let res = post(
        c,
        addr,
        "/api/changes/preflight",
        serde_json::json!({ "projects": [repo.to_string_lossy()], "agent": "claude", "title": "do it" }),
    )
    .await;
    res["targets"]
        .as_array()
        .and_then(|rows| rows.first())
        .cloned()
        .unwrap_or_else(|| panic!("no row for {}: {res}", repo.display()))
}

/// A repository with `devplane.toml` and the extra files, committed.
fn repo_with(tag: &str, config: &str, files: &[(&str, &str)]) -> PathBuf {
    let repo = configured_repo(tag, config);
    for (name, body) in files {
        std::fs::write(repo.join(name), body).unwrap();
    }
    if !files.is_empty() {
        git_out(&repo, &["add", "-A"]);
        git_out(&repo, &["commit", "-qm", "files"]);
    }
    repo
}

#[tokio::test]
async fn the_install_cost_is_stated_before_dispatch() {
    let (addr, c, _state) = boot().await;

    // A declared setup is the install, named.
    let declared = repo_with("note-setup", "[workspace]\nsetup = \"true\"\n", &[]);
    trust(&c, &addr, &declared).await;
    let row = preflight_row(&c, &addr, &declared).await;
    assert_eq!(
        row["notes"],
        serde_json::json!(["the first run installs dependencies: `true`"]),
        "{row}"
    );
    assert!(row["refusal"].is_null(), "a note is never a refusal: {row}");

    // A lockfile and no setup: the note names the lockfile.
    let bare = repo_with("note-lock", "", &[("package-lock.json", "{}\n")]);
    trust(&c, &addr, &bare).await;
    let row = preflight_row(&c, &addr, &bare).await;
    let note = row["notes"][0].as_str().unwrap_or_default();
    assert!(
        note.starts_with("a fresh tree will have no dependencies installed"),
        "{row}"
    );
    assert!(note.contains("package-lock.json"), "{row}");
    assert!(note.contains("[workspace] setup is not declared"), "{row}");

    // Neither: the field is present and empty.
    let quiet = repo_with("note-none", "", &[]);
    trust(&c, &addr, &quiet).await;
    let row = preflight_row(&c, &addr, &quiet).await;
    assert_eq!(row["notes"], serde_json::json!([]), "{row}");

    // A cargo project sharing its cache has no install step to miss.
    let shared = repo_with(
        "note-cargo",
        "[workspace]\nshare = [\"cargo\"]\n",
        &[("Cargo.lock", "# Cargo.lock\n")],
    );
    trust(&c, &addr, &shared).await;
    let row = preflight_row(&c, &addr, &shared).await;
    assert_eq!(row["notes"], serde_json::json!([]), "{row}");

    for r in [declared, bare, quiet, shared] {
        std::fs::remove_dir_all(&r).ok();
    }
}

/// While setup runs the change names the command and duration; afterwards nothing, report kept.
#[tokio::test]
async fn installing_is_visible_while_it_runs() {
    let Some(agent) = echo_agent() else { return };
    let repo = repo_with("installing", "[workspace]\nsetup = \"sleep 2\"\n", &[]);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    // The start blocks on setup, so it runs in the background.
    let starting = {
        let (c, repo) = (c.clone(), repo.clone());
        tokio::spawn(
            async move { start_change(&c, &addr, &repo, "install something", &agent).await },
        )
    };
    let mut seen: Option<Value> = None;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let list = get(&c, &addr, "/api/changes").await;
        if let Some(row) = list
            .as_array()
            .and_then(|a| a.iter().find(|w| w["waiting"]["on"] == "setup"))
        {
            seen = Some(row.clone());
            break;
        }
    }
    let row = seen.expect("the change never said it was installing");
    assert_eq!(row["waiting"]["command"], "sleep 2", "{row}");
    assert!(row["waiting"]["since"].is_string(), "{row}");
    let says = row["waiting_says"].as_str().unwrap_or_default();
    assert!(says.starts_with("installing · `sleep 2` · "), "{says}");
    assert_eq!(row["state"], "isolated", "nothing has run yet: {row}");

    let started = starting.await.unwrap();
    assert!(started["error"].is_null(), "{started}");
    let id = started["change_id"].as_str().unwrap().to_string();
    let row = change_row(&c, &addr, &id).await;
    assert!(
        row["waiting"].is_null() || row["waiting"]["on"] != "setup",
        "{row}"
    );
    assert_eq!(
        row["setup"]["commands"][0]["outcome"]["code"], 0,
        "the setup report is kept, and it is not a gate: {row}"
    );
    assert_eq!(
        row["gates"].as_array().map(Vec::len),
        Some(0),
        "setup counted as a gate"
    );
    await_rest_of(&c, &addr, &id, &["stopped", "looking"]).await;
    std::fs::remove_dir_all(&repo).ok();
}

/// A declared cache is shared by every change; nothing is redirected otherwise.
#[tokio::test]
async fn a_declared_cache_is_shared_between_changes() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, _state) = boot().await;
    let setup = "[workspace]\nsetup = \"sh -c 'echo $CARGO_TARGET_DIR > seen'\"\n";
    let sharing = repo_with("share", &format!("{setup}share = [\"cargo\"]\n"), &[]);
    let alone = repo_with("noshare", setup, &[]);
    trust(&c, &addr, &sharing).await;
    trust(&c, &addr, &alone).await;

    let one = start_change(&c, &addr, &sharing, "first", &agent).await;
    let two = start_change(&c, &addr, &sharing, "second", &agent).await;
    let three = start_change(&c, &addr, &alone, "third", &agent).await;
    for r in [&one, &two, &three] {
        assert!(r["error"].is_null(), "{r}");
    }
    let seen = |r: &Value| {
        let tree = PathBuf::from(r["change"]["worktree"].as_str().unwrap());
        std::fs::read_to_string(tree.join("seen"))
            .unwrap()
            .trim()
            .to_string()
    };
    let expected = sharing.join(".claude/worktrees/.shared/cargo");
    assert_eq!(seen(&one), expected.to_string_lossy());
    assert_eq!(
        seen(&two),
        expected.to_string_lossy(),
        "the second change builds where the first did"
    );
    assert_eq!(seen(&three), "", "nothing declared, nothing redirected");

    for r in [&one, &two, &three] {
        await_rest_of(
            &c,
            &addr,
            r["change_id"].as_str().unwrap(),
            &["stopped", "looking"],
        )
        .await;
    }
    std::fs::remove_dir_all(&sharing).ok();
    std::fs::remove_dir_all(&alone).ok();
}

/// `.worktreeinclude` selects among ignored files, never tracked ones.
#[tokio::test]
async fn worktreeinclude_is_honoured_and_only_for_ignored_files() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, _state) = boot().await;
    let sb = sandbox::Sandbox::new(sandbox::Shape::Full);
    std::fs::write(
        sb.repo.join(".worktreeinclude"),
        ".env\nconfig/*.local\nkeep.txt\n",
    )
    .unwrap();
    std::fs::write(sb.repo.join("keep.txt"), "tracked, and stays git's\n").unwrap();
    git_out(&sb.repo, &["add", ".worktreeinclude", "keep.txt"]);
    git_out(&sb.repo, &["commit", "-qm", "include"]);
    sb.gitignored(".env", "SECRET=1\n");
    sb.gitignored("config/a.local", "local\n");
    sb.gitignored("config/b.txt", "not asked for\n");
    // The pattern names a tracked file; git's listing never offers it.
    let ignored = devplane::git::ignored_files(&sb.repo).await.unwrap();
    assert!(ignored.contains(&".env".to_string()), "{ignored:?}");
    assert!(
        ignored.contains(&"config/a.local".to_string()),
        "{ignored:?}"
    );
    assert!(!ignored.contains(&"keep.txt".to_string()), "{ignored:?}");

    trust(&c, &addr, &sb.repo).await;
    let started = start_change(&c, &addr, &sb.repo, "carry the env", &agent).await;
    assert!(started["error"].is_null(), "{started}");
    let tree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(tree.join(".env")).unwrap(),
        "SECRET=1\n"
    );
    assert_eq!(
        std::fs::read_to_string(tree.join("config/a.local")).unwrap(),
        "local\n"
    );
    assert!(
        !tree.join("config/b.txt").exists(),
        "a file no pattern names was copied"
    );
    assert_eq!(
        std::fs::read(tree.join("keep.txt")).unwrap(),
        std::fs::read(sb.repo.join("keep.txt")).unwrap()
    );
    assert_eq!(git_out(&tree, &["status", "--porcelain", "keep.txt"]), "");

    await_rest_of(
        &c,
        &addr,
        started["change_id"].as_str().unwrap(),
        &["stopped", "looking"],
    )
    .await;
}

// ── Removal never loses work ─────────────────────────────────────────────

/// An unmerged branch cannot be deleted and the refusal names the commits;
/// `force` deletes it anyway.
#[tokio::test]
async fn removal_refuses_on_unmerged_commits_and_names_them() {
    let repo = scratch_repo("unmerged", "escalate", 0);
    hand_made_branch(&repo, "feat/two-commits");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/two-commits", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = adopted["change_id"].as_str().unwrap().to_string();
    let tree = PathBuf::from(adopted["change"]["worktree"].as_str().unwrap());

    let refused = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive?delete_branch=true"),
        Value::Null,
    )
    .await;
    let err = refused["error"].as_str().unwrap_or_default();
    assert!(err.contains("2 commit(s)"), "{err}");
    assert!(
        err.contains("make the gate pass"),
        "the first subject is named: {err}"
    );
    assert!(err.contains("write the spec"), "and the second: {err}");
    assert!(tree.exists(), "a refusal removed the worktree");
    assert!(branches_of(&repo).contains(&"feat/two-commits".to_string()));

    let kept = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive"),
        Value::Null,
    )
    .await;
    assert_eq!(kept["ok"], true, "{kept}");
    assert_eq!(kept["worktree_removed"], true);
    assert_eq!(kept["branch_deleted"], false);
    assert!(!tree.exists());
    assert!(
        branches_of(&repo).contains(&"feat/two-commits".to_string()),
        "the branch was deleted without being asked"
    );
    // `force` alone is refused: it only ever means *delete this branch anyway*.
    let lone = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive?force=true"),
        Value::Null,
    )
    .await;
    assert!(lone["error"].is_string(), "{lone}");
    std::fs::remove_dir_all(&repo).ok();
}

/// Uncommitted work refuses removal, naming the file, unless `discard_uncommitted`.
#[tokio::test]
async fn removal_refuses_on_uncommitted_changes_and_names_the_file() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("dirty-remove", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let started = start_change(&c, &addr, &repo, "leave a draft", &agent).await;
    let id = started["change_id"].as_str().unwrap().to_string();
    let tree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    await_rest_of(&c, &addr, &id, &["stopped", "looking"]).await;

    std::fs::write(tree.join("draft.txt"), "not committed\n").unwrap();
    let refused = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive"),
        Value::Null,
    )
    .await;
    let err = refused["error"].as_str().unwrap_or_default();
    assert!(err.contains("uncommitted"), "{err}");
    assert!(err.contains("draft.txt"), "the file is named: {err}");
    assert!(
        tree.join("draft.txt").exists(),
        "a refusal discarded the work"
    );

    let forced = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/archive?discard_uncommitted=true"),
        Value::Null,
    )
    .await;
    assert_eq!(forced["ok"], true, "{forced}");
    assert!(!tree.exists());
    std::fs::remove_dir_all(&repo).ok();
}

// ── It is ordinary git ───────────────────────────────────────────────────

/// `git worktree list` names the change, and the tree needs nothing from Devplane.
#[tokio::test]
async fn the_tree_is_ordinary_git() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("ordinary", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let started = start_change(&c, &addr, &repo, "plain git", &agent).await;
    let id = started["change_id"].as_str().unwrap().to_string();
    let tree = PathBuf::from(started["change"]["worktree"].as_str().unwrap());
    let branch = started["change"]["branch"].as_str().unwrap().to_string();

    let listed = git_out(&repo, &["worktree", "list", "--porcelain"]);
    assert!(
        listed.contains(&format!("worktree {}", tree.display())),
        "{listed}"
    );
    assert!(
        listed.contains(&format!("branch refs/heads/{branch}")),
        "{listed}"
    );

    await_rest_of(&c, &addr, &id, &["stopped", "looking"]).await;
    // Nothing untracked but the agent's log; only `devplane.toml` names Devplane.
    let extra = git_out(&tree, &["status", "--porcelain", "--ignored", "-uall"]);
    for line in extra.lines() {
        let path = &line[3..];
        assert!(
            path.starts_with(".devplane/"),
            "Devplane put {path} in the tree"
        );
    }
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.file_name()
                .is_some_and(|n| n == ".git" || n == ".devplane")
            {
                continue;
            }
            if p.is_dir() {
                walk(&p, out)
            } else {
                out.push(p)
            }
        }
    }
    let mut files = Vec::new();
    walk(&tree, &mut files);
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            !name.contains("devplane") || name == "devplane.toml",
            "{} requires Devplane to read",
            f.display()
        );
    }
    std::fs::remove_dir_all(&repo).ok();
}

/// An in-place change marks what it gives up; an isolated one does not.
#[tokio::test]
async fn an_in_place_change_marks_what_it_gives_up() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("inplace", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;

    let here = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "edit here",
            "agent": agent,
            "worktree": false,
        }),
    )
    .await;
    assert!(here["error"].is_null(), "{here}");
    let here_id = here["change_id"].as_str().unwrap().to_string();
    let row = change_row(&c, &addr, &here_id).await;
    assert_eq!(row["in_place"], true, "{row}");
    assert_eq!(
        row["in_place_says"],
        "in place — no parallel safety, and the diff is against the working tree",
        "{row}"
    );
    assert!(row["worktree"].is_null());

    let isolated = start_change(&c, &addr, &repo, "edit apart", &agent).await;
    let iso_id = isolated["change_id"].as_str().unwrap().to_string();
    let row = change_row(&c, &addr, &iso_id).await;
    assert_eq!(row["in_place"], false, "{row}");
    assert!(row["in_place_says"].is_null(), "{row}");

    let board = get(&c, &addr, "/api/board").await;
    let briefs = board["changes"]
        .as_array()
        .expect("the board lists changes");
    let brief = briefs.iter().find(|b| b["id"] == here_id).unwrap();
    assert_eq!(brief["in_place"], true);
    assert!(
        brief["in_place_says"]
            .as_str()
            .unwrap_or("")
            .starts_with("in place")
    );
    let brief = briefs.iter().find(|b| b["id"] == iso_id).unwrap();
    assert_eq!(brief["in_place"], false);
    assert!(brief["in_place_says"].is_null());

    await_rest_of(&c, &addr, &iso_id, &["stopped", "looking"]).await;
    let deadline = std::time::Instant::now() + SETTLE;
    while std::time::Instant::now() < deadline && !repo.join(".devplane/heard.log").exists() {
        tokio::time::sleep(POLL).await;
    }
    // Gated in place too: the gate runs in the person's checkout.
    let deadline = std::time::Instant::now() + SETTLE;
    let mut row = change_row(&c, &addr, &here_id).await;
    while std::time::Instant::now() < deadline
        && row["gates"].as_array().is_none_or(|g| g.is_empty())
    {
        tokio::time::sleep(POLL).await;
        row = change_row(&c, &addr, &here_id).await;
    }
    assert_eq!(
        row["gates"][0]["gate"], "check",
        "an in-place change was not gated: {row}"
    );

    let board = get(&c, &addr, "/api/board").await;
    let brief = board["changes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == here_id)
        .cloned()
        .unwrap();
    assert_eq!(
        brief["project_id"],
        repo.to_string_lossy().as_ref(),
        "{brief}"
    );
    assert!(brief["project_name"].is_string(), "{brief}");
    assert!(brief["updated_at"].is_string(), "{brief}");
    assert_eq!(
        brief["standing"], "failed",
        "check.sh fails without fixed.txt: {brief}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

// ── Across several projects, every refusal is reported together ──────────

/// A repository with one commit and a `devplane.toml` declaring one gate.
fn plain_repo(tag: &str) -> PathBuf {
    repo_with(
        tag,
        "[gates]\ncheck = [\"true\"]\n",
        &[("README.md", "# scratch\n")],
    )
}

/// Every refusing project is named in one answer, and nothing is created anywhere.
#[tokio::test]
async fn several_projects_report_every_refusal_together_and_create_nothing() {
    let (addr, c, state) = boot().await;
    let dirty = plain_repo("fanout-dirty");
    let broken = plain_repo("fanout-broken");
    let untrusted = plain_repo("fanout-untrusted");
    let clean = plain_repo("fanout-clean");
    for r in [&dirty, &broken, &clean] {
        trust(&c, &addr, r).await;
    }
    // Registered, so the preflight can see it; never trusted.
    state
        .world
        .lock()
        .await
        .upsert_project(devplane::core::Project::from_root(untrusted.clone()));
    std::fs::write(dirty.join("README.md"), "edited and not committed\n").unwrap();
    // Committed, so the refusal is about parsing, not dirt.
    std::fs::write(broken.join("devplane.toml"), "[gates\ncheck = [\n").unwrap();
    git_out(&broken, &["commit", "-qam", "break the config"]);

    let paths: Vec<String> = [&dirty, &broken, &clean, &untrusted]
        .iter()
        .map(|r| r.to_string_lossy().to_string())
        .collect();
    let res = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({ "projects": paths, "agent": "claude", "title": "bump deps" }),
    )
    .await;
    assert!(
        res["error"]
            .as_str()
            .unwrap_or_default()
            .contains("nothing was started"),
        "{res}"
    );
    let rows = res["targets"].as_array().unwrap_or_else(|| panic!("{res}"));
    assert_eq!(rows.len(), 4, "{res}");
    let refusal_of = |root: &PathBuf| {
        let root = root.display().to_string();
        rows.iter()
            .find(|r| r["root"] == root.as_str())
            .and_then(|r| r["refusal"].as_str().map(str::to_string))
    };
    assert_eq!(refusal_of(&dirty).as_deref(), Some("dirty_worktree"));
    assert_eq!(refusal_of(&broken).as_deref(), Some("config_will_not_load"));
    assert_eq!(refusal_of(&untrusted).as_deref(), Some("untrusted"));
    assert_eq!(refusal_of(&clean), None, "{res}");
    assert_eq!(res["refused"], 3, "three refused, in one answer: {res}");

    // All refusals in one answer; no worktree in any of the four.
    for r in [&dirty, &broken, &untrusted, &clean] {
        assert!(!r.join(".claude/worktrees").exists(), "{}", r.display());
        assert_eq!(worktrees_of(r).len(), 1, "{}", r.display());
    }
    for r in [dirty, broken, untrusted, clean] {
        std::fs::remove_dir_all(&r).ok();
    }
}

/// A message sent mid-turn is recorded at once and heard when the turn ends,
/// without stopping the run.
#[tokio::test]
async fn a_message_to_a_run_in_flight_is_queued_and_recorded() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("steer", "escalate", 0);
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;
    let v = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "take a while",
            "prompt": "slowish",
            "agent": agent,
        }),
    )
    .await;
    assert!(v.get("error").is_none(), "{v}");
    let run = v["change"]["runs"][0].as_str().unwrap().to_string();
    let worktree = PathBuf::from(v["change"]["worktree"].as_str().unwrap());

    let said = post(
        &c,
        &addr,
        &format!("/api/runs/{run}/prompt"),
        serde_json::json!({ "text": "use the existing helper" }),
    )
    .await;
    assert_eq!(said["delivery"], "queued", "{said}");
    assert_eq!(
        said["says"],
        "queued — delivered when the current turn ends"
    );

    let events = serde_json::to_value(
        state
            .store
            .events_for_run(&devplane::core::RunId::new(&run), 200)
            .await
            .unwrap(),
    )
    .unwrap();
    let prompts = events
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["event"]["type"] == "prompt_submitted")
        .count();
    assert!(
        prompts >= 2,
        "the message was not recorded when sent: {events}"
    );
    let heard = std::fs::read_to_string(worktree.join(".devplane/heard.log")).unwrap_or_default();
    assert!(
        !heard.contains("use the existing helper"),
        "the turn was interrupted to deliver it: {heard}"
    );

    let deadline = std::time::Instant::now() + SETTLE;
    loop {
        let heard =
            std::fs::read_to_string(worktree.join(".devplane/heard.log")).unwrap_or_default();
        if let Some(at) = heard.find("use the existing helper") {
            assert!(heard[..at].contains("slowish"), "{heard}");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the queued message was never delivered: {heard}"
        );
        tokio::time::sleep(POLL).await;
    }
    std::fs::remove_dir_all(&repo).ok();
}

/// Stopping says what survives before it stops, and it does.
#[tokio::test]
async fn stopping_says_what_survives_and_it_does() {
    let Some(agent) = echo_agent() else { return };
    let repo = scratch_repo("stopsays", "escalate", 0);
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let v = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "stop me",
            "prompt": "slow",
            "agent": agent,
        }),
    )
    .await;
    assert!(v.get("error").is_none(), "{v}");
    let id = v["change_id"].as_str().unwrap().to_string();
    let run = v["change"]["runs"][0].as_str().unwrap().to_string();
    let worktree = v["change"]["worktree"].as_str().unwrap().to_string();
    let branch = v["change"]["branch"].as_str().unwrap().to_string();

    let before = get(&c, &addr, &format!("/api/runs/{run}/stop")).await;
    let says = before["says"].as_str().unwrap_or_default().to_string();
    assert!(
        says.starts_with("stopping ends the agent's turn; "),
        "{before}"
    );
    assert!(says.contains(&worktree) && says.contains(&branch), "{says}");
    assert_eq!(before["survives"]["change"], id.as_str());

    let stopped = post(&c, &addr, &format!("/api/runs/{run}/stop"), Value::Null).await;
    assert_eq!(stopped["ok"], true, "{stopped}");
    assert_eq!(stopped["says"], says.as_str());

    assert!(Path::new(&worktree).is_dir(), "the worktree went");
    assert!(branches_of(&repo).contains(&branch), "the branch went");
    let row = c
        .get(format!("http://{addr}/api/changes/{id}"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap();
    assert_eq!(row.status(), 200, "the change's record went");
    std::fs::remove_dir_all(&repo).ok();
}

// ── Git is never surprised ─────────────────────────────────────────────────

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A spec never committed is not in the worktree's base: the start refuses,
/// naming folder and base, before creating anything.
#[tokio::test]
async fn a_spec_missing_from_the_base_is_refused_before_anything_is_created() {
    let Some(agent) = echo_agent() else { return };
    let repo = counts_repo("spec-base");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    std::fs::create_dir_all(repo.join("specs/042-new")).unwrap();
    std::fs::write(repo.join("specs/042-new/spec.md"), "# New\n").unwrap();

    let v = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "work to a spec nobody committed",
            "agent": agent,
            "spec": "specs/042-new",
        }),
    )
    .await;
    let err = v["error"].as_str().unwrap_or_else(|| panic!("{v}"));
    assert!(
        err.contains("specs/042-new") && err.contains("`main`"),
        "{err}"
    );
    assert_eq!(worktrees_of(&repo).len(), 1, "a worktree was made");
    assert!(
        get(&c, &addr, "/api/changes")
            .await
            .as_array()
            .unwrap()
            .is_empty(),
        "a change was recorded"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// Offering a dirty worktree, or a branch with nothing past its base, is
/// refused and says which.
#[tokio::test]
async fn offering_a_dirty_or_empty_branch_is_refused() {
    let repo = scratch_repo("offer-refused", "escalate", 1);
    hand_made_branch(&repo, "feat/by-hand");
    assert!(git_ok(&repo, &["branch", "feat/empty", "main"]));
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopt = |branch: &'static str| {
        let (c, repo) = (c.clone(), repo.clone());
        async move {
            post(
                &c,
                &addr,
                "/api/changes/adopt",
                serde_json::json!({ "branch": branch, "project": repo.to_string_lossy() }),
            )
            .await
        }
    };

    let v = adopt("feat/by-hand").await;
    let id = v["change_id"].as_str().unwrap().to_string();
    let wt = PathBuf::from(v["change"]["worktree"].as_str().unwrap());
    std::fs::write(wt.join("left-behind.txt"), "not committed\n").unwrap();
    let offer = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    let err = offer["error"].as_str().unwrap_or_else(|| panic!("{offer}"));
    assert!(err.contains("uncommitted or untracked"), "{err}");

    let v = adopt("feat/empty").await;
    let id = v["change_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{v}"))
        .to_string();
    let offer = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    let err = offer["error"].as_str().unwrap_or_else(|| panic!("{offer}"));
    assert!(err.contains("no commits past `main`"), "{err}");
    std::fs::remove_dir_all(&repo).ok();
}

/// Archiving keeps the branch. Deleting it refuses unmerged commits unless the
/// pull request merged (a squash looks unmerged).
#[tokio::test]
async fn archive_keeps_the_branch_and_a_merged_pull_request_counts_as_merged() {
    let repo = scratch_repo("archive", "escalate", 1);
    hand_made_branch(&repo, "feat/keep");
    assert!(git_ok(&repo, &["checkout", "-q", "main"]));
    hand_made_branch(&repo, "feat/squashed");
    assert!(git_ok(&repo, &["checkout", "-q", "main"]));
    let (addr, c, state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopt = |branch: &'static str| {
        let (c, repo) = (c.clone(), repo.clone());
        async move {
            let v = post(
                &c,
                &addr,
                "/api/changes/adopt",
                serde_json::json!({ "branch": branch, "project": repo.to_string_lossy() }),
            )
            .await;
            v["change_id"]
                .as_str()
                .unwrap_or_else(|| panic!("{v}"))
                .to_string()
        }
    };

    let keep = adopt("feat/keep").await;
    let done = post(
        &c,
        &addr,
        &format!("/api/changes/{keep}/archive"),
        Value::Null,
    )
    .await;
    assert_eq!(done["worktree_removed"], true, "{done}");
    assert_eq!(done["branch_deleted"], false, "{done}");
    assert!(
        git_ok(&repo, &["rev-parse", "--verify", "feat/keep"]),
        "the branch went"
    );

    let squashed = adopt("feat/squashed").await;
    let refused = post(
        &c,
        &addr,
        &format!("/api/changes/{squashed}/archive?delete_branch=true"),
        Value::Null,
    )
    .await;
    let err = refused["error"]
        .as_str()
        .unwrap_or_else(|| panic!("{refused}"));
    assert!(err.contains("commit(s) `main` does not"), "{err}");
    assert!(git_ok(&repo, &["rev-parse", "--verify", "feat/squashed"]));

    // Merged by squash, so no branch commit is an ancestor of `main`.
    if let Some(ch) = state
        .changes
        .lock()
        .await
        .get_mut(&devplane::core::ChangeId::new(squashed.clone()))
    {
        ch.pull_request = Some(devplane::core::change::PullRequestRef {
            number: 7,
            url: "https://example.test/pr/7".into(),
            status: "merged".into(),
            failing_checks: Vec::new(),
        });
    }
    let done = post(
        &c,
        &addr,
        &format!("/api/changes/{squashed}/archive?delete_branch=true"),
        Value::Null,
    )
    .await;
    assert_eq!(done["branch_deleted"], true, "{done}");
    assert!(!git_ok(&repo, &["rev-parse", "--verify", "feat/squashed"]));
    std::fs::remove_dir_all(&repo).ok();
}

// ── Decide honestly: a weakened check guards the offer ──────────────────────

/// The hand-made branch, plus a commit that skips the burst test.
fn branch_that_skips_a_test(repo: &Path, branch: &str) {
    hand_made_branch(repo, branch);
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["checkout", "-q", branch]);
    std::fs::create_dir_all(repo.join("tests")).unwrap();
    std::fs::write(
        repo.join("tests/login.test.ts"),
        "test(\"login\", () => { expect(login()).toBe(true); });\nit.skip(\"rejects the sixth attempt\", () => {});\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "skip the burst test"]);
    git(&["checkout", "-q", "main"]);
}

fn commit_in(dir: &Path, message: &str) {
    for args in [vec!["add", "-A"], vec!["commit", "-qm", message]] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(dir)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}");
    }
}

/// Offering a verified change whose diff skipped a test is refused, naming the
/// row, until a person marks it seen; then it proceeds exactly as before. A
/// different skip marker is a different row, unseen again.
#[tokio::test]
async fn an_offer_waits_for_every_weakened_row_to_be_seen() {
    let repo = scratch_repo("offer-weakened", "escalate", 1);
    branch_that_skips_a_test(&repo, "feat/skips");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/skips", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = adopted["change_id"].as_str().unwrap().to_string();
    let wt = PathBuf::from(adopted["change"]["worktree"].as_str().unwrap());
    post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;

    // The qualifier travels with the green.
    let row = change_row(&c, &addr, &id).await;
    assert_eq!(row["state"], "verified", "{row}");
    assert_eq!(row["qualifier"]["weakened"], 1, "{row}");
    assert_eq!(row["qualifier"]["unseen"], 1, "{row}");
    assert_eq!(row["qualifier"]["says"], "1 check weakened", "{row}");

    // Refused, naming the row and how to mark it; nothing is offered.
    let offer = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(offer["refused"], "weakened_unseen", "{offer}");
    let rows = offer["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{offer}");
    assert_eq!(rows[0]["path"], "tests/login.test.ts");
    assert!(
        rows[0]["why"].as_str().unwrap().contains("it.skip("),
        "{offer}"
    );
    assert!(
        offer["seen_with"]
            .as_str()
            .unwrap()
            .ends_with(&format!("change review {id} --seen tests/login.test.ts")),
        "{offer}"
    );
    assert!(change_row(&c, &addr, &id).await["pull_request"].is_null());

    // A mark for a row that is not there is refused, and marks nothing.
    let none = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/review/seen"),
        serde_json::json!({ "path": "fixed.txt" }),
    )
    .await;
    assert!(
        none["error"].as_str().unwrap().contains("no weakened row"),
        "{none}"
    );
    assert_eq!(none["rows"][0]["path"], "tests/login.test.ts", "{none}");

    // Seen by a person: the review says so, and the offer proceeds as before.
    let seen = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/review/seen"),
        serde_json::json!({ "path": "tests/login.test.ts" }),
    )
    .await;
    assert_eq!(seen["seen"][0]["path"], "tests/login.test.ts", "{seen}");
    let review = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    assert_eq!(review["groups"][0]["weakened"][0]["seen"], true, "{review}");
    assert_eq!(review["qualifier"]["unseen"], 0, "{review}");
    let cert = get(&c, &addr, &format!("/api/changes/{id}/certificate")).await;
    let md = cert["markdown"].as_str().unwrap();
    assert!(
        md.contains("altered the checks it was verified by")
            && md.contains("A person marked each of these read."),
        "the certificate still states it: {md}"
    );

    // The marker's text moves: a new row, unseen, and the offer is refused again.
    std::fs::write(
        wt.join("tests/login.test.ts"),
        "test(\"login\", () => {});\nit.skip(\"rejects a burst\", () => {});\n",
    )
    .unwrap();
    commit_in(&wt, "reword the skipped test");
    let again = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(again["refused"], "weakened_unseen", "{again}");
    post(
        &c,
        &addr,
        &format!("/api/changes/{id}/review/seen"),
        serde_json::json!({ "path": "tests/login.test.ts" }),
    )
    .await;
    let offer = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(offer["offer"], "commands", "{offer}");
    std::fs::remove_dir_all(&repo).ok();
}

/// A diff git will not read is never "nothing weakened": the offer refuses and
/// the certificate says it does not know.
#[cfg(unix)]
#[tokio::test]
async fn an_unreadable_diff_refuses_the_offer_and_is_unknown_on_the_certificate() {
    use std::os::unix::fs::PermissionsExt;
    let repo = scratch_repo("offer-unreadable", "escalate", 1);
    branch_that_skips_a_test(&repo, "feat/skips");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/skips", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = adopted["change_id"].as_str().unwrap().to_string();
    let wt = PathBuf::from(adopted["change"]["worktree"].as_str().unwrap());
    // `git diff` cannot hash a modified file it may not open.
    let file = wt.join("tests/login.test.ts");
    std::fs::write(&file, "it.skip(\"everything\", () => {});\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();

    let offer = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    let err = offer["error"].as_str().unwrap_or_else(|| panic!("{offer}"));
    assert!(err.contains("diff could not be read"), "{err}");
    let cert = get(&c, &addr, &format!("/api/changes/{id}/certificate")).await;
    let md = cert["markdown"].as_str().unwrap();
    assert!(md.contains("the diff could not be read"), "{md}");
    assert!(!md.contains("altered none of its checks"), "{md}");
    let row = change_row(&c, &addr, &id).await;
    assert_eq!(
        row["qualifier"]["says"], "the diff could not be read",
        "{row}"
    );

    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::remove_dir_all(&repo).ok();
}

/// The ready row carries what the review leads with, and its first action is
/// the review; it never offers, weakened or not.
#[tokio::test]
async fn the_ready_row_carries_the_reviews_facts_and_never_offers() {
    let repo = scratch_repo("ready-facts", "escalate", 1);
    branch_that_skips_a_test(&repo, "feat/skips");
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let adopted = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/skips", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = adopted["change_id"].as_str().unwrap().to_string();
    post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    let inbox = get(&c, &addr, "/api/inbox").await;
    let ready = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "ready_to_decide" && i["change_id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("no ready row: {inbox}"));
    let acts = ready["actions"].as_array().unwrap();
    assert_eq!(acts[0], "review", "{ready}");
    assert!(!acts.iter().any(|a| a == "offer"), "{ready}");
    let facts = &ready["facts"];
    assert_eq!(facts["qualifier"]["weakened"], 1, "{ready}");
    assert_eq!(facts["qualifier"]["unseen"], 1, "{ready}");
    assert!(facts["hunks"].as_u64().unwrap() >= 1, "{ready}");
    assert_eq!(facts["says"].as_array().unwrap().len(), 2, "{ready}");
    std::fs::remove_dir_all(&repo).ok();
}

/// Offering, finishing, archiving and marking a weakened row read are a
/// person's decisions: refused from inside an agent's session before anything
/// is read, with the same stated limit as `answer`.
#[test]
fn a_decision_about_a_change_is_refused_from_an_agent_session() {
    for args in [
        vec!["change", "offer", "c-any"],
        vec!["change", "finish", "c-any"],
        vec!["change", "archive", "c-any"],
        vec!["change", "review", "c-any", "--seen", "tests/login.test.ts"],
    ] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .args(&args)
            .env("CLAUDECODE", "1")
            .env(
                "DEVPLANE_HOME",
                std::env::temp_dir().join("vp-no-home-here"),
            )
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{args:?} ran from an agent session");
        assert!(
            err.contains("`CLAUDECODE` is set") && err.contains("does not close it"),
            "{args:?}: {err}"
        );
    }
}

/// Editing the definition of done after a pass does not keep the green: the
/// commands that ran are compared with the commands declared now.
#[test]
fn a_pass_by_other_check_commands_is_not_verified() {
    use devplane::core::change::{
        Change, ChangeState, CommandResult, CommitStamp, Declared, GateReport, Outcome, Reach,
        Standing,
    };
    let stamp = CommitStamp {
        commit: Some("abc".into()),
        tree: Some("t1".into()),
        branch: None,
        clean: true,
        changed_files: 0,
        reach: Reach::LocalOnly,
        remote: None,
    };
    let mut change = Change::new_at(
        devplane::core::ProjectId::new("p"),
        "t".into(),
        "p".into(),
        jiff::Timestamp::now(),
    );
    change.gates.push(GateReport {
        gate: "check".into(),
        at: jiff::Timestamp::now(),
        duration_ms: 1,
        commands: vec![CommandResult {
            command: "cargo test".into(),
            outcome: Outcome::Exited { code: 0 },
            duration_ms: 1,
            output_tail: String::new(),
            output_bytes: 0,
            output_digest: String::new(),
            failures: Vec::new(),
        }],
        attempt: 1,
        spec: None,
        commit: Some(stamp.clone()),
    });
    let same = ["cargo test".to_string()];
    assert_eq!(
        change.verdict(Declared::Checks(&same), Some(&stamp)),
        Standing::Verified
    );
    let loosened = ["cargo test -- --skip slow".to_string()];
    assert!(matches!(
        change.verdict(Declared::Checks(&loosened), Some(&stamp)),
        Standing::ChecksChanged { .. }
    ));
    assert_eq!(
        change
            .verdict(Declared::Checks(&loosened), Some(&stamp))
            .word(),
        "stale"
    );

    // The state is that same verdict, so the two never disagree: a loosened
    // check, a deleted one, and a configuration nobody could read are each
    // not verified in both.
    for declared in [
        Declared::Checks(&same),
        Declared::Checks(&loosened),
        Declared::Checks(&[]),
        Declared::Unreadable,
    ] {
        let standing = change.verdict(declared, Some(&stamp));
        let state = change.state(declared, Some(&stamp));
        assert_eq!(
            state == ChangeState::Verified,
            standing == Standing::Verified,
            "{declared:?}: state {state:?}, standing {standing:?}"
        );
    }
    assert_eq!(
        change.verdict(Declared::Checks(&[]), Some(&stamp)),
        Standing::NoGatesDeclared
    );
    assert_eq!(
        change.verdict(Declared::Unreadable, Some(&stamp)),
        Standing::ConfigUnreadable
    );
    assert_ne!(
        change.state(Declared::Unreadable, Some(&stamp)),
        ChangeState::Verified
    );
}

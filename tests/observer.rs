//! End-to-end tests for the observer.
//!
//! These drive the real HTTP surface with the payloads Claude Code actually
//! sends, because every interesting bug in this product lives at that seam: a
//! hook whose shape changed, a permission answered a millisecond too late, a
//! notification that empties the inbox instead of filling it.

mod common;

use devplane::core::Policy;
use devplane::daemon::{AppState, Shared};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::Path;

/// Boots a daemon on an ephemeral port with a throwaway store.
async fn boot(policy: Policy) -> (SocketAddr, String, reqwest::Client) {
    let db = std::env::temp_dir().join(format!(
        "vp-it-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, _state) = boot_shared(policy, &db).await;
    (addr, token, c)
}

/// The same, keeping the state so a test can reach into the daemon's own view
/// of itself — which is where `gate_down` lives, because nothing observable
/// distinguishes a gate that has stopped deciding from a quiet machine.
async fn boot_shared(policy: Policy, db: &Path) -> (SocketAddr, String, reqwest::Client, Shared) {
    let state = boot_with_db(db, "test-token".into(), policy).await;
    let addr = listen(state.clone()).await;
    (
        addr,
        "test-token".to_string(),
        reqwest::Client::new(),
        state,
    )
}

async fn boot_with_db(db: &Path, token: String, policy: Policy) -> Shared {
    AppState::new(
        db.to_path_buf(),
        token,
        policy,
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .expect("daemon state")
}

/// Serves on an ephemeral port and returns the address.
async fn listen(state: Shared) -> SocketAddr {
    let app = devplane::api::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    addr
}

/// Runs the gate the way Claude Code runs it: the binary, a payload on stdin,
/// the verdict on stdout, with a `~/.devplane` of our own and **no daemon**.
///
/// The gate used to be an HTTP handler in the daemon, and these tests used to
/// drive that handler. It is a `command` hook now, because Claude Code treats a
/// connection failure as a non-blocking error and carries on — so an HTTP gate
/// is one that is off whenever the daemon is, silently. Testing the handler
/// would now be testing something no session ever reaches.
fn gate(home: &Path, machine_rules: &str, payload: &str) -> Value {
    std::fs::create_dir_all(home).unwrap();
    std::fs::write(home.join("policy.toml"), machine_rules).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("hook")
        .env("DEVPLANE_HOME", home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(payload.as_bytes())?;
            child.wait_with_output()
        })
        .expect("the gate runs");
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("stdout was {text:?}: {e}"))
}

/// A throwaway `~/.devplane`.
fn gate_home(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "vp-gate-{}-{name}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

async fn post(
    c: &reqwest::Client,
    addr: &SocketAddr,
    path: &str,
    token: &str,
    body: &str,
) -> String {
    c.post(format!("http://{addr}{path}"))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

async fn get_json(
    c: &reqwest::Client,
    addr: &SocketAddr,
    path: &str,
    token: &str,
) -> serde_json::Value {
    c.get(format!("http://{addr}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// The inbox's **items**, which is what almost every test here is about.
///
/// `/api/inbox` answers `{ items, close }`: the list, and what the day came to
/// for a list that is empty. The close is the daemon's sentences and is asserted
/// in the tests that are about it.
async fn inbox_items(c: &reqwest::Client, addr: &SocketAddr, token: &str) -> serde_json::Value {
    get_json(c, addr, "/api/inbox", token).await["items"].clone()
}

#[tokio::test]
async fn a_session_that_asks_a_question_reaches_the_inbox() {
    let (addr, token, c) = boot(Policy::default()).await;

    // The shape Claude Code posts for an `AskUserQuestion` tool call.
    post(
        &c,
        &addr,
        "/devplane/hook",
        &token,
        r#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/tmp/repo",
            "tool_name":"AskUserQuestion",
            "tool_input":{"questions":[{"question":"Keep the legacy route?",
              "options":[{"label":"Keep"},{"label":"Remove"}]}]}}"#,
    )
    .await;
    // The turn ends immediately afterwards — this is the ordinary shape, and
    // the question must survive it.
    post(
        &c,
        &addr,
        "/devplane/hook",
        &token,
        r#"{"hook_event_name":"Stop","session_id":"s1","cwd":"/tmp/repo"}"#,
    )
    .await;

    let inbox = inbox_items(&c, &addr, &token).await;
    assert_eq!(inbox.as_array().unwrap().len(), 1);
    assert_eq!(inbox[0]["kind"], "question");
    // An option a human can read; no id, because Claude Code owns this dialog
    // and only the person in front of it can answer.
    assert_eq!(inbox[0]["options"][1]["label"], "Remove");
    assert!(inbox[0]["options"][1]["id"].is_null());

    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(board["summary"]["needs_you"], 1);
}

#[tokio::test]
async fn a_permission_no_rule_covers_is_left_to_claude_and_shown_to_the_human() {
    let (addr, token, c) = boot(Policy::default()).await;

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s2","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"rm -rf node_modules"}}"#;

    // An empty object means "no decision": Claude Code prompts exactly as it
    // would have. Anything else would change what the user sees.
    let reply = gate(&gate_home("nocover"), "", payload);
    assert_eq!(reply, serde_json::json!({}));

    // The daemon learns about it through the envelope the gate files.
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        &serde_json::json!({
            "session": "s2", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: rm -rf node_modules", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        })
        .to_string(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let inbox = inbox_items(&c, &addr, &token).await;
    assert_eq!(inbox[0]["kind"], "permission");
    assert!(
        inbox[0]["detail"].as_str().unwrap().contains("rm -rf"),
        "the human needs to see the command to decide"
    );
}

/// **A prohibition answers; nothing approves.**
///
/// This test used to assert that a project allow rule made Devplane answer
/// `allow` on the vendor's behalf. That verdict is gone: saying yes was a claim
/// that Claude Code would also have said yes, and keeping that claim true meant
/// mirroring the vendor's semantics for ever. What remains is the half that
/// costs nothing — a project's prohibition still fires, and everything else
/// reaches the person.
#[tokio::test]
async fn a_prohibition_answers_and_nothing_is_approved() {
    let (addr, token, c) = boot(Policy::default()).await;
    let home = gate_home("matching");
    let rules = r#"
[policy]
never_auto = ["Bash(git push *)"]
"#;

    let permitted = gate(
        &home,
        rules,
        r#"{"hook_event_name":"PermissionRequest","session_id":"s3","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"pnpm test -- --run"}}"#,
    );
    // No approval, whatever the project wrote: the vendor's own permission
    // system decides that, using the vendor's own configuration.
    assert_ne!(
        permitted["hookSpecificOutput"]["decision"]["behavior"],
        "allow"
    );

    let deny = gate(
        &home,
        rules,
        r#"{"hook_event_name":"PermissionRequest","session_id":"s3","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"git push --force origin main"}}"#,
    );
    assert_eq!(deny["hookSpecificOutput"]["decision"]["behavior"], "deny");
    assert!(
        deny["hookSpecificOutput"]["decision"]["message"]
            .as_str()
            .unwrap()
            .contains("Bash(git push *)"),
        "the refusal names the rule that produced it"
    );

    // A decided permission is not a decision the human has to make.
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        r#"{"session":"s3","verdict":"deny","rule":"Bash(git push *)",
            "subject":"Bash: git push --force origin main","tool":"Bash"}"#,
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let inbox = inbox_items(&c, &addr, &token).await;
    assert!(inbox.as_array().unwrap().is_empty());

    // And it is written down, naming the rule.
    let decisions = get_json(&c, &addr, "/api/decisions", &token).await;
    let rows = decisions.as_array().unwrap();
    assert_eq!(rows.len(), 1, "one refusal, one row");
    assert_eq!(rows[0]["outcome"], "deny");
    assert_eq!(rows[0]["reason"], "Bash(git push *)");
}

#[tokio::test]
async fn the_policy_gate_answers_fast_enough_to_be_invisible() {
    // The session is blocked while this runs. A gate that takes long enough to
    // notice is a gate that makes Claude Code feel slower than it is.
    //
    // **This test used to measure an HTTP handler, and then it measured
    // nothing.** When the gate moved to a `command` hook the route went with
    // it, and the assertion went on passing — a 404 is very fast. A latency
    // test pointed at something that no longer exists is worse than no latency
    // test, because it reports a budget nobody is holding. It now spawns the
    // binary, which is what Claude Code does.
    //
    // The budget is what the move was justified on: a cold process answering
    // from disk beat the ~50 ms this path was already allowed over loopback.
    // Measured on the **debug** binary, which is the slow one — release was
    // about 26 ms.
    let home = gate_home("latency");
    let body = r#"{"hook_event_name":"PermissionRequest","session_id":"s4","cwd":"/tmp/repo",
        "tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}"#;
    let rules = "[policy]\nnever_auto = [\"Bash(rm *)\"]\n";

    // Warm the page cache so the measurement is the gate, not the first read of
    // a 40 MB debug binary off a cold disk.
    gate(&home, rules, body);

    let mut runs: Vec<std::time::Duration> = Vec::new();
    for _ in 0..20 {
        let t = std::time::Instant::now();
        let reply = gate(&home, rules, body);
        runs.push(t.elapsed());
        // The latency that matters is a *prohibition's*: that is the only
        // answer Devplane gives, and it is the one that must not be felt.
        assert_eq!(reply["hookSpecificOutput"]["decision"]["behavior"], "deny");
    }
    runs.sort();
    let median = runs[runs.len() / 2];
    let worst = *runs.last().expect("twenty runs");

    // **The median carries the budget, and it did not always.**
    //
    // This asserted on the *worst* of twenty runs, which is the right statistic
    // for "must not be felt" and the wrong one to measure inside `cargo test`:
    // fifteen test binaries run in parallel, each spawning processes, so the
    // tail is the scheduler rather than the gate. It passed for months on luck
    // and began failing the day another test started spawning a process — at
    // 598ms, while the same test alone measured well inside budget.
    //
    // A budget that fails for reasons unrelated to what it measures gets raised
    // until it stops failing, and then it is not a budget. So the median
    // carries it: a real regression in the gate moves every run, and scheduler
    // noise moves the tail.
    assert!(
        median < std::time::Duration::from_millis(250),
        "median gate answer was {median:?} over {} runs; the budget is 250ms for an \
         unoptimised build. This is the statistic a regression moves.",
        runs.len()
    );

    // **And a loose ceiling on the tail**, because the median alone would not
    // notice a gate that answers instantly nineteen times and hangs once. Set
    // where only a hang can reach it, not where the suite's own load can.
    assert!(
        worst < std::time::Duration::from_secs(5),
        "worst gate answer was {worst:?}. The median is the budget; this catches a hang, \
         and five seconds is not something scheduler noise produces."
    );
}

#[tokio::test]
async fn telemetry_gives_the_run_its_cost_and_context() {
    let (addr, token, c) = boot(Policy::default()).await;
    post(
        &c,
        &addr,
        "/devplane/hook",
        &token,
        r#"{"hook_event_name":"UserPromptSubmit","session_id":"s5","cwd":"/tmp/repo","prompt":"hi"}"#,
    )
    .await;

    // OTLP/HTTP JSON, as Claude Code's exporter sends it: 64-bit integers are
    // strings.
    c.post(format!("http://{addr}/devplane/otel/v1/logs"))
        .header("content-type", "application/json")
        .body(
            r#"{"resourceLogs":[{"scopeLogs":[{"logRecords":[{
                "body":{"stringValue":"claude_code.api_request"},
                "attributes":[
                  {"key":"session.id","value":{"stringValue":"s5"}},
                  {"key":"app.entrypoint","value":{"stringValue":"claude-vscode"}},
                  {"key":"model","value":{"stringValue":"claude-opus-5"}},
                  {"key":"cost_usd","value":{"doubleValue":0.25}},
                  {"key":"input_tokens","value":{"intValue":"20000"}},
                  {"key":"cache_read_tokens","value":{"intValue":"160000"}}]}]}]}]}"#,
        )
        .send()
        .await
        .unwrap();

    let board = get_json(&c, &addr, "/api/board", &token).await;
    let run = &board["runs"][0];
    assert_eq!(run["cost_usd"], 0.25);
    assert_eq!(run["entrypoint"], "claude-vscode");
    // 180k of a 200k window, and the gauge counts input plus cache, never
    // output — the same formula the provider's own status line uses.
    assert_eq!(run["context_percent"], 90.0);

    // Which is high enough that the human should hear about it.
    let inbox = inbox_items(&c, &addr, &token).await;
    assert_eq!(inbox[0]["kind"], "context_high");
}

#[tokio::test]
async fn a_worktree_session_belongs_to_the_repository_that_owns_it() {
    let (addr, token, c) = boot(Policy::default()).await;
    for (session, cwd) in [
        ("main", "/tmp/vp-repo"),
        ("wt-a", "/tmp/vp-repo/.claude/worktrees/feature-a"),
        ("wt-b", "/tmp/vp-repo/.claude/worktrees/feature-b"),
    ] {
        post(
            &c,
            &addr,
            "/devplane/hook",
            &token,
            &format!(
                r#"{{"hook_event_name":"UserPromptSubmit","session_id":"{session}","cwd":"{cwd}","prompt":"x"}}"#
            ),
        )
        .await;
    }
    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(board["summary"]["runs"], 3);
    assert_eq!(
        board["summary"]["projects"], 1,
        "three worktrees of one repository are one project"
    );
}

#[tokio::test]
async fn an_unauthorised_client_learns_nothing() {
    let (addr, _token, c) = boot(Policy::default()).await;
    for path in ["/api/board", "/api/inbox", "/api/diagnostics"] {
        let status = c
            .get(format!("http://{addr}{path}"))
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, 401, "{path} must require the token");
    }
    // The live stream too — an empty stream would leak nothing, but it would
    // be indistinguishable from a quiet machine.
    assert_eq!(
        c.get(format!("http://{addr}/api/stream"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // The health probe is deliberately open: it proves the port is ours
    // without revealing anything about what is running on it.
    assert!(
        c.get(format!("http://{addr}/healthz"))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
}

#[tokio::test]
async fn the_browser_shell_is_served_and_needs_no_token_of_its_own() {
    // The page carries no data; it cannot fetch any without the token the
    // user's browser holds. Gating it would only stop it rendering the message
    // that explains as much.
    let (addr, _token, c) = boot(Policy::default()).await;
    let res = c.get(format!("http://{addr}/")).send().await.unwrap();
    assert!(res.status().is_success());
    let body = res.text().await.unwrap();
    assert!(body.contains("<title>Devplane</title>"));
    assert!(
        !body.contains("test-token"),
        "the page must never embed a credential"
    );
}

#[tokio::test]
async fn a_snooze_takes_a_run_out_of_the_inbox_and_gives_it_back() {
    let (addr, token, c) = boot(Policy::default()).await;
    post(
        &c,
        &addr,
        "/devplane/hook",
        &token,
        r#"{"hook_event_name":"PreToolUse","session_id":"s9","cwd":"/tmp/repo",
            "tool_name":"AskUserQuestion",
            "tool_input":{"questions":[{"question":"which?","options":[]}]}}"#,
    )
    .await;
    assert_eq!(
        inbox_items(&c, &addr, &token)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );

    c.post(format!("http://{addr}/api/runs/s9/snooze?minutes=60"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(
        inbox_items(&c, &addr, &token)
            .await
            .as_array()
            .unwrap()
            .is_empty(),
        "a snoozed run is out of the queue"
    );

    // But still on the board: snoozing hides a request, not a session.
    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(board["summary"]["runs"], 1);

    c.post(format!("http://{addr}/api/runs/s9/snooze?minutes=0"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        inbox_items(&c, &addr, &token)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_malformed_hook_payload_never_fails_the_session() {
    let (addr, token, c) = boot(Policy::default()).await;
    // Claude Code shows the user an error if a hook fails. An observer that
    // cannot parse something must swallow it, not interrupt the work.
    let res = c
        .post(format!("http://{addr}/devplane/hook"))
        .bearer_auth(&token)
        .header("content-type", "application/json")
        .body("{ not json at all")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    assert_eq!(
        gate(&gate_home("garbage"), "", "{ also not json }"),
        serde_json::json!({}),
        "and an unreadable permission request is left alone"
    );
}

#[tokio::test]
async fn events_survive_a_restart_and_rebuild_the_same_board() {
    let dir = std::env::temp_dir().join(format!("vp-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("test.db");

    {
        let state = boot_with_db(&db, "tok".into(), Policy::default()).await;
        let addr = listen(state).await;
        let c = reqwest::Client::new();
        post(
            &c,
            &addr,
            "/devplane/hook",
            "tok",
            r#"{"hook_event_name":"PreToolUse","session_id":"keep","cwd":"/tmp/repo",
                "tool_name":"Bash","tool_input":{"command":"cargo build"}}"#,
        )
        .await;
        let board = get_json(&c, &addr, "/api/board", "tok").await;
        assert_eq!(board["summary"]["runs"], 1);
    }

    // A new daemon over the same store.
    let state = boot_with_db(&db, "tok".into(), Policy::default()).await;
    let addr = listen(state).await;
    let c = reqwest::Client::new();
    let board = get_json(&c, &addr, "/api/board", "tok").await;
    assert_eq!(board["summary"]["runs"], 1, "the run survived the restart");
    assert_eq!(board["runs"][0]["summary"], "Bash: cargo build");

    let events = get_json(&c, &addr, "/api/runs/keep/events", "tok").await;
    assert_eq!(events.as_array().unwrap().len(), 1);

    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Driven runs
// ---------------------------------------------------------------------------

/// The fixture agent from `devplane-acp`, built as a sibling of this binary.
fn echo_agent_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe
        .parent()?
        .parent()?
        .join("examples")
        .join(if cfg!(windows) {
            "echo_agent.exe"
        } else {
            "echo_agent"
        });
    bin.exists().then(|| bin.to_string_lossy().to_string())
}

#[tokio::test]
async fn a_driven_run_joins_the_same_board_and_its_permission_can_be_answered() {
    // The whole point of driving an agent is that the answer happens here
    // rather than in somebody's terminal. This walks that path end to end.
    let _serial = common::one_agent_at_a_time();
    let Some(agent) = echo_agent_path() else {
        eprintln!("skipping: build the fixture with `cargo build -p devplane-acp --examples`");
        return;
    };
    let (addr, token, c) = boot(Policy::default()).await;
    let cwd = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Every path that starts an agent goes through the trust gate, dispatch
    // included — so the test has to make the same decision a person would.
    c.post(format!("http://{addr}/api/projects/trust"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "path": cwd }))
        .send()
        .await
        .unwrap();

    let started: Value = c
        .post(format!("http://{addr}/api/dispatch"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "agent": agent, "cwd": cwd }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let run = started["run_id"].as_str().expect("a run id").to_string();

    // It is a run like any other: same board, same project resolution.
    let board = get_json(&c, &addr, "/api/board", &token).await;
    let row = board["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == run.as_str())
        .expect("the driven run is on the board");
    assert_eq!(row["mode"], "driven");

    c.post(format!("http://{addr}/api/runs/{run}/prompt"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "text": "this needs permission" }))
        .send()
        .await
        .unwrap();

    // Wait for the request to surface rather than sleeping a fixed time: a
    // test that races the agent is a test that fails on a busy machine.
    let mut item = Value::Null;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let inbox = inbox_items(&c, &addr, &token).await;
        if let Some(first) = inbox.as_array().and_then(|a| a.first()) {
            item = first.clone();
            break;
        }
    }
    assert_eq!(
        item["kind"], "permission",
        "the request must reach the inbox"
    );
    assert!(
        item["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "allow"),
        "a driven permission is answerable, not merely visible"
    );
    // **The token, not the session.** An inbox item offers the ask's own id,
    // which is what an answer is addressed to from any surface at any later
    // time — including after the process holding the request has gone.
    let ask = item["ask"]
        .as_str()
        .expect("an answerable item carries its ask")
        .to_string();

    let answered: Value = c
        .post(format!("http://{addr}/api/asks/{ask}/answer"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "option": "allow", "from": "test" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(answered["open"], false, "answering settles the ask");
    assert!(
        answered["outcome"]
            .as_str()
            .unwrap_or_default()
            .contains("you answered it"),
        "the outcome names the person: {}",
        answered["outcome"]
    );
    assert_eq!(
        answered["delivery"], "live",
        "a waiting agent takes the answer down the connection it asked on"
    );

    // **Answered exactly once, however many surfaces try.** Two people, two
    // devices, one agent: the second is told who answered rather than being
    // allowed to answer again.
    let again: Value = c
        .post(format!("http://{addr}/api/asks/{ask}/answer"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "option": "allow", "from": "second-surface" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let said = again["error"].as_str().unwrap_or_default();
    assert!(
        said.contains("already answered") && said.contains("test"),
        "the second answer is refused and names the first: {said}"
    );

    // And it is on the record afterwards, which is what makes *who answered
    // this?* answerable without opening a transcript.
    let asks = get_json(&c, &addr, "/api/asks", &token).await;
    let settled = asks["settled"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == ask.as_str())
        .expect("the ask is on the record");
    assert_eq!(settled["answered_from"], "test");

    let mut cleared = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if inbox_items(&c, &addr, &token)
            .await
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(false)
        {
            cleared = true;
            break;
        }
    }
    assert!(cleared, "an answered request leaves the inbox");

    c.post(format!("http://{addr}/api/runs/{run}/stop"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn an_unknown_agent_is_refused_by_name() {
    let (addr, token, c) = boot(Policy::default()).await;
    let res: Value = c
        .post(format!("http://{addr}/api/dispatch"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "agent": "clauude", "cwd": "/tmp" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        res["error"].as_str().unwrap_or("").contains("clauude"),
        "a typo must be reported as an unknown agent, not a missing binary"
    );
}

#[tokio::test]
async fn stopping_the_daemon_is_a_request_rather_than_a_signal() {
    // A record left behind by a crash names a pid the operating system has
    // since given to something else, and killing a stranger's process because a
    // file said so is not a thing a tool should be able to do. A request needs the
    // bearer token, so only something that can read `~/.devplane/token` can
    // stop it.
    let (addr, token, c) = boot(Policy::default()).await;

    let refused = c
        .post(format!("http://{addr}/api/shutdown"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        refused.status(),
        401,
        "without the token it is nobody's business"
    );

    let ok = c
        .post(format!("http://{addr}/api/shutdown"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(ok.status().is_success());
    let body: Value = ok.json().await.unwrap();
    assert_eq!(body["stopping"], true);
    assert_eq!(
        body["pid"].as_u64().unwrap() as u32,
        std::process::id(),
        "and it names the process it is about to stop"
    );
}

#[tokio::test]
async fn dispatching_into_a_directory_that_does_not_exist_is_refused() {
    let (addr, token, c) = boot(Policy::default()).await;
    let _serial = common::one_agent_at_a_time();
    let Some(agent) = echo_agent_path() else {
        return;
    };
    let res: Value = c
        .post(format!("http://{addr}/api/dispatch"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "agent": agent, "cwd": "/no/such/place" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(res["error"].as_str().unwrap_or("").contains("directory"));
}

/// The rule that made auto mode safe to observe.
///
/// In auto mode Claude Code reviews actions with a classifier instead of
/// asking the user. Routine calls are approved with no prompt, so
/// `PermissionRequest` — which fires only when Claude Code is about to ask —
/// never happens, and a project's `never_auto` rule would never be consulted.
/// `PreToolUse` fires before every tool call in every mode, so that is where a
/// prohibition has to be answered.
///
/// It answers with a prohibition or with nothing. Never an allow: a
/// `PreToolUse` allow skips Claude Code's permission system altogether,
/// classifier included, so a rule meaning "no need to ask me" would switch off
/// a safety layer the user chose.
#[tokio::test]
async fn a_prohibition_reaches_a_session_that_is_never_going_to_prompt() {
    let (addr, token, c) = boot(Policy::default()).await;
    let home = gate_home("auto");
    let rules = r#"
[policy]
never_auto = ["Bash(git push *)"]
always_ask = ["Bash(gh release *)"]
"#;
    let pre = |cmd: &str| {
        format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"auto","cwd":"/tmp/repo",
                "permission_mode":"auto","tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#
        )
    };

    // A deny stops the call, and says which rule did it.
    let denied = gate(&home, rules, &pre("git push --force"));
    assert_eq!(
        denied["hookSpecificOutput"]["permissionDecision"], "deny",
        "a never_auto rule has to reach auto mode or it protects nothing"
    );
    assert!(
        denied["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("Bash(git push *)"),
        "and the reason names the rule"
    );

    // An ask forces the prompt the classifier would otherwise have skipped.
    let asked = gate(&home, rules, &pre("gh release create v1"));
    assert_eq!(asked["hookSpecificOutput"]["permissionDecision"], "ask");

    // An *allowed* command is answered with nothing at all. The call still
    // goes through the classifier, which is the layer this must not remove.
    assert_eq!(
        gate(&home, rules, &pre("pnpm test -- --run")),
        serde_json::json!({}),
        "a PreToolUse allow would skip the classifier as well as the prompt"
    );

    // And the prohibition is accounted for afterwards, like every other verdict
    // — through the envelope the gate files rather than by the daemon deciding
    // a second time.
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        r#"{"session":"auto","verdict":"deny","rule":"Bash(git push *)",
            "subject":"Bash: git push --force","tool":"Bash"}"#,
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let log = get_json(&c, &addr, "/api/decisions", &token).await;
    let rules: Vec<&str> = log
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["reason"].as_str())
        .collect();
    assert!(
        rules.iter().any(|r| r.contains("git push")),
        "a decision nobody can account for is the thing this product exists to prevent: {rules:?}"
    );
}

/// The property the move to a `command` hook was for: a stopped daemon costs
/// the *record*, never the enforcement.
///
/// Claude Code documents a connection failure on an HTTP hook as a non-blocking
/// error that lets execution continue, so while the gate lived in the daemon,
/// every `never_auto` rule on the machine was off whenever the daemon was —
/// silently, with nothing in the session to say so.
#[tokio::test]
async fn the_gate_decides_with_no_daemon_and_files_the_decision_afterwards() {
    let home = gate_home("nodaemon");
    let rules = "[policy]\nnever_auto = [\"Read(.env)\"]\n";
    let payload = r#"{"hook_event_name":"PreToolUse","session_id":"offline","cwd":"/tmp/repo",
        "tool_name":"Bash","tool_input":{"command":"cat .env"}}"#;

    // Nothing is listening: `DEVPLANE_HOME` is fresh, so there is no
    // `daemon.json` for the gate to find.
    assert!(!home.join("daemon.json").exists());
    let reply = gate(&home, rules, payload);
    assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");

    // The decision it took is waiting to be written down, with the rule and the
    // time it was actually taken.
    let spool = std::fs::read_to_string(home.join("pending-decisions.jsonl"))
        .expect("a decision taken with no daemon is spooled, not lost");
    let row: Value = serde_json::from_str(spool.lines().next().unwrap()).unwrap();
    assert_eq!(row["verdict"], "deny");
    assert_eq!(row["rule"], "Read(.env)");
    assert_eq!(row["late"], true, "the log has to say it was filed late");
    assert!(row["at"].is_string());

    // An *undecided* call is not spooled: nobody had a rule about it, and the
    // provider can be asked again. Only decisions have to survive.
    let quiet = gate(
        &home,
        rules,
        r#"{"hook_event_name":"PreToolUse","session_id":"offline","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"ls -la"}}"#,
    );
    assert_eq!(quiet, serde_json::json!({}));
    let spool = std::fs::read_to_string(home.join("pending-decisions.jsonl")).unwrap();
    assert_eq!(spool.lines().count(), 1, "observations are not decisions");
}

/// A `Read` deny has to survive the trip through a shell command, because
/// `cat .env` is reading `.env` by any honest reading of the rule.
#[tokio::test]
async fn a_file_rule_reaches_the_files_a_shell_command_names() {
    // `Read` covers the read and `Edit` covers the write, because that is where
    // the running product draws the line: under `Read(.env)` alone it refuses
    // `cat .env` and runs `echo pwned > .env`. Both halves are needed to
    // protect a file from a shell, and saying so is the honest version of a
    // test that used to assert `Read` did both.
    let home = gate_home("filerule");
    let rules = r#"
[policy]
never_auto = ["Read(.env)", "Edit(.env)"]
"#;
    // The glob spellings are here because they were not, and `cat .en?` read
    // the file under this exact rule for two passes: the shell expands the
    // operand before the program sees it, and the matcher was comparing a rule
    // against a pattern as though it were a path.
    for cmd in [
        "cat .env",
        "echo pwned > .env",
        "echo pwned | tee .env",
        "cat .en?",
        "cat .env*",
    ] {
        let body = format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"sh","cwd":"/tmp/repo",
                "tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#
        );
        let v = gate(&home, rules, &body);
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecision"], "deny",
            "`{cmd}` is covered by Read(.env) and a rule that misses it reads as protection and is none"
        );
    }
}

#[tokio::test]
async fn a_repository_with_no_remote_is_asked_about_once() {
    // A regression test for a bug this suite caught before it shipped, and the
    // reason it is worth a test rather than a comment: the symptom was not an
    // error anywhere, it was an unrelated test going flaky one run in six.
    //
    // A launch link names a repository rather than a path, so Devplane reads
    // `git remote get-url origin`. Asking whenever `repo_url` was still empty
    // meant a repository with **no** remote — a scratch checkout, a worktree of
    // something never pushed — spawned a subprocess on *every event, for ever*,
    // on the hot path, starving the process-global reactor the agent sessions
    // run on. The fix is to remember having asked, whatever the answer.
    let dir = std::env::temp_dir().join(format!("vp-noremote-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();

    let db = dir.join("d.db");
    let state = boot_with_db(&db, "t".into(), Policy::default()).await;
    let addr = listen(state.clone()).await;
    let c = reqwest::Client::new();

    let ev = |n: u32| {
        format!(
            r#"{{"hook_event_name":"UserPromptSubmit","session_id":"s{n}",
                 "cwd":"{}","prompt":"x"}}"#,
            dir.display()
        )
    };
    for n in 0..5 {
        post(&c, &addr, "/devplane/hook", "t", &ev(n)).await;
    }

    let asked = state.remote_asked.lock().await;
    assert_eq!(
        asked.len(),
        1,
        "five events in one repository must ask git once, not five times"
    );
    state.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
}

/// `doctor` must **run** the gate, not read a settings file.
///
/// Every way this layer has been wrong was a gate that read as installed and
/// decided nothing: an async entry, an HTTP entry with no daemon behind it, a
/// hook set from before the second event existed. And no hook can enforce its
/// own presence — the vendor's reference says not to count on a stalled hook to
/// act as a gate — so detection is the only defence there is.
#[test]
fn doctor_runs_the_gate_rather_than_reading_about_it() {
    use serde_json::Map;
    let exe = env!("CARGO_BIN_EXE_devplane");

    // A real one answers.
    let installed: Map<String, Value> = serde_json::from_str(&format!(
        r#"{{"hooks": {{"PreToolUse": [{{"hooks": [
            {{"type":"command","command":"{exe} hook","timeout":5}}]}}]}}}}"#
    ))
    .unwrap();
    let probe = devplane::observe::connect::probe_gate(&installed);
    assert!(
        probe.answered,
        "the installed gate refused nothing: {:?}",
        probe.error
    );

    // An HTTP entry is not a command gate, however well-formed it looks.
    let http: Map<String, Value> = serde_json::from_str(
        r#"{"hooks": {"PreToolUse": [{"hooks": [
            {"type":"http","url":"http://127.0.0.1:47831/devplane/policy","timeout":5}]}]}}"#,
    )
    .unwrap();
    assert!(!devplane::observe::connect::probe_gate(&http).answered);

    // A binary that has been moved or uninstalled reads as installed and is
    // not a gate. This is the failure the probe exists for, and the provider
    // lets the call through when it happens.
    let moved: Map<String, Value> = serde_json::from_str(
        r#"{"hooks": {"PreToolUse": [{"hooks": [
            {"type":"command","command":"/nowhere/devplane hook","timeout":5}]}]}}"#,
    )
    .unwrap();
    let probe = devplane::observe::connect::probe_gate(&moved);
    assert!(!probe.answered);
    assert!(probe.error.is_some(), "and it says what went wrong");
}

/// Running the diagnostic must not write history.
///
/// The probe gets a real verdict, because it goes through the real gate. A real
/// verdict used to get a real row: three `devplane doctor` runs left three
/// refusals of a command nobody issued, in the one table that is never pruned
/// and exists to answer "why did that happen".
#[test]
fn probing_the_gate_writes_nothing_down() {
    let home = gate_home("probe-noop");
    let rules = "[policy]\nnever_auto = [\"Read(.env)\"]\n";
    let probe = format!(
        r#"{{"hook_event_name":"PreToolUse","session_id":"{}","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{{"command":"cat .env"}}}}"#,
        devplane::observe::hook::PROBE_SESSION
    );

    // It still decides — a gate that answered the probe differently would be
    // testing something other than the gate.
    let reply = gate(&home, rules, &probe);
    assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");

    // And with no daemon listening, a real decision would have been spooled.
    assert!(
        !home.join("pending-decisions.jsonl").exists(),
        "the diagnostic filed a decision about a call nobody made"
    );

    // The same call from a real session does spool, which is what makes the
    // assertion above mean something.
    let real = probe.replace(devplane::observe::hook::PROBE_SESSION, "s-real");
    gate(&home, rules, &real);
    assert!(home.join("pending-decisions.jsonl").exists());
}

/// A gate that has stopped deciding and a quiet machine are the same thing in
/// the event log: no hook arrives either way.
///
/// So this is the one inbox item Devplane raises about **itself**, and the
/// only one that is about the machine rather than about a run. It is critical,
/// which nothing else is except a lost run, because the board looks completely
/// normal while no rule in any project is being enforced.
#[tokio::test]
async fn a_gate_that_stopped_answering_reaches_the_inbox() {
    let db = std::env::temp_dir().join(format!(
        "vp-gatedown-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, state) = boot_shared(Policy::default(), &db).await;

    // Nothing wrong: the inbox says nothing about the gate.
    let inbox = inbox_items(&c, &addr, &token).await;
    assert!(
        !inbox
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "gate_down"),
        "a working gate is not news"
    );

    // The watcher finds the installed command will not run.
    *state.gate_down.lock().await = Some("it will not start: No such file or directory".into());

    let inbox = inbox_items(&c, &addr, &token).await;
    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "gate_down")
        .expect("the gate being inert has to reach the inbox");
    assert_eq!(item["level"], "critical");
    assert!(item["run_id"].is_null(), "it is about the machine");
    assert!(
        item["detail"].as_str().unwrap().contains("No such file"),
        "and it carries the reason, so `doctor` is a confirmation rather than a hunt"
    );
    // Nothing is offered, because the fix is reinstalling and this product does
    // not rewrite somebody's settings.json from an inbox row.
    assert!(item["actions"].as_array().unwrap().is_empty());

    // **The CLI has to be able to read it**, and this is the assertion that
    // would have caught a bug that was already shipped: `render::InboxItem`
    // typed `run_id` as `String` while the API has always sent `null` for an
    // item with no session — so `devplane inbox` failed to decode the *whole*
    // response the moment one existed, which `AttentionItem` describes as "the
    // ordinary case, not an edge one". Driving the API and never the decoder is
    // how a whole surface stayed broken.
    let decoded: Vec<devplane::render::InboxItem> =
        serde_json::from_value(inbox.clone()).expect("the CLI decodes what the API serves");
    assert!(decoded.iter().any(|i| i.kind == "gate_down"));

    // The shape that was already broken before this item existed: a work item
    // whose runs have all ended. A pull request going red hours later is the
    // ordinary case.
    let work_shaped = serde_json::json!([{
        "kind": "ci_red", "level": "high", "run_id": null,
        "title": "CI is red", "work_id": "w-1", "actions": ["open_pr"]
    }]);
    let decoded: Vec<devplane::render::InboxItem> =
        serde_json::from_value(work_shaped).expect("a work item with no run must decode too");
    assert!(decoded[0].run_id.is_none());
}

#[tokio::test]
async fn the_gates_own_probe_never_reaches_the_board_from_either_receiver() {
    // `doctor` and the daemon's timer run the installed gate against a probe
    // call. The hook declines to report it — and an older hook binary, or a
    // spool it wrote, can still deliver one to the daemon. Both receivers have
    // to drop it: once it arrived as a `devplane-probe-<pid>` project with a
    // working session and two audit rows.
    let (addr, token, c) = boot(Policy::default()).await;
    let probe = devplane::observe::hook::PROBE_SESSION;

    post(
        &c,
        &addr,
        "/devplane/hook",
        &token,
        &format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"{probe}","cwd":"/tmp/devplane-probe-1",
                "tool_name":"Bash","tool_input":{{"command":"cat .devplane-probe"}}}}"#
        ),
    )
    .await;
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        &serde_json::json!({
            "session": probe, "verdict": "deny", "rule": "Read(.devplane-probe)",
            "blocked": true, "subject": "Bash: cat .devplane-probe", "tool": "Bash",
            "late": true,
            "payload": {"hook_event_name":"PreToolUse","session_id":probe,
                        "cwd":"/tmp/devplane-probe-1","tool_name":"Bash",
                        "tool_input":{"command":"cat .devplane-probe"}},
        })
        .to_string(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let board = get_json(&c, &addr, "/api/board?all=true", &token).await;
    assert_eq!(
        board["runs"].as_array().unwrap().len(),
        0,
        "a probe is not a session"
    );
    assert!(
        board["projects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| !p["root"].as_str().unwrap_or("").contains("devplane-probe")),
        "a probe's scratch directory is not a project"
    );
    let decisions = get_json(&c, &addr, "/api/decisions?limit=10", &token).await;
    assert!(
        decisions
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["subject"].as_str().unwrap_or("") != "Bash: cat .devplane-probe"),
        "a probe verdict is real and the call is not; the log must not carry it"
    );
}

#[tokio::test]
async fn the_health_probe_names_the_version_so_a_client_can_restart_a_stale_daemon() {
    // Every command compares this to its own version and restarts a daemon
    // older than itself. Two releases of `audit` and `attention` answered 404
    // on machines that had upgraded without a restart.
    let (addr, _token, c) = boot(Policy::default()).await;
    let text = c
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(text, format!("ok {}", env!("CARGO_PKG_VERSION")));
}

/// The forge endpoint: one route, both halves, what needs the person first.
///
/// Everything else that knows about GitHub here is either pure (the derivation
/// in `core::forge`) or a process running `gh`. The seam between them — the
/// route, its bearer check, the sort, the envelope the board reads — had no
/// test at all, which is the seam every interesting bug in this product lives
/// at.
#[tokio::test]
async fn the_forge_serves_both_halves_in_one_answer_with_what_needs_you_first() {
    use devplane::core::{ForgeIssue, ForgePullRequest, ProjectForge, ProjectId};

    let db = std::env::temp_dir().join(format!(
        "vp-forge-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, state) = boot_shared(Policy::default(), &db).await;

    let pr = |number: u64, mine: bool, review: bool, status: &str, draft: bool| ForgePullRequest {
        number,
        title: format!("pr {number}"),
        url: format!("https://github.com/acme/app/pull/{number}"),
        status: status.into(),
        draft,
        mine,
        review_requested: review,
        head_ref: "x".into(),
        updated_at: None,
    };
    {
        let mut f = state.forge.lock().await;
        f.viewer = Some("me".into());
        f.last_poll_at = Some(jiff::Timestamp::now());
        f.projects.insert(
            ProjectId::new("p1"),
            ProjectForge {
                project_id: ProjectId::new("p1"),
                repo: Some("acme/app".into()),
                fetched_at: jiff::Timestamp::now(),
                issues: vec![
                    ForgeIssue {
                        number: 1,
                        title: "someone else's".into(),
                        url: "u1".into(),
                        labels: vec![],
                        assigned_to_me: false,
                        updated_at: None,
                    },
                    ForgeIssue {
                        number: 2,
                        title: "yours".into(),
                        url: "u2".into(),
                        labels: vec!["bug".into()],
                        assigned_to_me: true,
                        updated_at: None,
                    },
                ],
                // A draft of your own with red checks asks nothing, so it sorts
                // below the review that was actually requested of you.
                pull_requests: vec![
                    pr(10, true, false, "failing", true),
                    pr(11, false, true, "ready_for_review", false),
                ],
                error: None,
            },
        );
    }

    // The bearer check is the thing that separates this from every other
    // process running as this user, and a `?token=` is not it.
    let un = c
        .get(format!("http://{addr}/api/forge?token={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(un.status(), 401, "a query token must not open an API route");

    let v = get_json(&c, &addr, "/api/forge", &token).await;
    assert_eq!(v["viewer"], "me");
    assert!(v["fetched_at"].is_string(), "the board says when it read");

    let issues = v["issues"].as_array().expect("issues");
    assert_eq!(issues.len(), 2, "every open issue, not only yours");
    assert_eq!(
        issues[0]["number"], 2,
        "what is assigned to you comes first"
    );
    // The poller only ever reads registered projects, so a name is normally
    // there. This is the fallback: a forge entry whose project the world no
    // longer has still renders as a row rather than failing to serialise.
    assert_eq!(issues[0]["project"], "p1");
    assert_eq!(issues[0]["project_name"], "");
    assert_eq!(issues[0]["labels"][0], "bug");

    let prs = v["pull_requests"].as_array().expect("pull_requests");
    assert_eq!(prs.len(), 2, "every open pull request, drafts included");
    assert_eq!(
        prs[0]["number"], 11,
        "the review asked of you outranks your own unfinished draft"
    );
    assert_eq!(prs[1]["draft"], true);

    // A project whose last poll failed keeps its numbers and says they are old.
    {
        let mut f = state.forge.lock().await;
        f.projects.get_mut(&ProjectId::new("p1")).unwrap().error =
            Some("gh: connection refused".into());
    }
    let v = get_json(&c, &addr, "/api/forge", &token).await;
    let stale = v["stale"].as_array().expect("stale");
    assert_eq!(stale.len(), 1, "the view names what it could not read");
    assert_eq!(stale[0]["error"], "gh: connection refused");
    assert!(stale[0]["last_good"].is_string(), "and when it last could");
    assert_eq!(
        v["issues"].as_array().unwrap().len(),
        2,
        "a failed poll does not empty the board"
    );
    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(
        board["forge"]["p1"]["stale"], "gh: connection refused",
        "and the heading's counts carry why they might be wrong"
    );
    {
        let mut f = state.forge.lock().await;
        f.projects.get_mut(&ProjectId::new("p1")).unwrap().error = None;
    }

    // And the board's headings count exactly what the inbox would list.
    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(board["summary"]["open_issues"], 2);
    assert_eq!(board["summary"]["open_prs"], 2);
    assert_eq!(
        board["summary"]["forge_needs_you"], 2,
        "one assigned issue and one requested review; the draft asks nothing"
    );
    std::fs::remove_file(&db).ok();
}

/// A project ruled out of the forge is re-checked, and says why it was.
///
/// `is_permanent` decides "this will never have a forge" by looking for the
/// word `remote` in another program's English error message. That is brittle
/// by construction, and it used to be *final*: a wrong guess dropped a project
/// from the forge for the daemon's lifetime, with no row anywhere saying so.
/// It was also simply wrong about time — `git remote add origin …` makes a
/// project a GitHub project, and nothing noticed until a restart.
#[tokio::test]
async fn a_project_ruled_out_of_the_forge_is_re_checked_and_says_why() {
    use devplane::core::ProjectId;

    let db = std::env::temp_dir().join(format!(
        "vp-skip-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, state) = boot_shared(Policy::default(), &db).await;
    {
        let mut f = state.forge.lock().await;
        f.viewer = Some("me".into());
        f.skip.insert(
            ProjectId::new("scratch"),
            (
                "no git remotes configured".into(),
                jiff::Timestamp::now() - jiff::SignedDuration::from_hours(2),
            ),
        );
        f.skip.insert(
            ProjectId::new("fresh"),
            ("no git remotes configured".into(), jiff::Timestamp::now()),
        );
    }

    let d = get_json(&c, &addr, "/api/diagnostics", &token).await;
    let ruled = d["forge"]["skipped_projects"].as_array().expect("skipped");
    assert_eq!(ruled.len(), 2, "both are reported, not just counted");
    assert!(
        ruled
            .iter()
            .all(|r| r["reason"] == "no git remotes configured" && r["at"].is_string()),
        "a count of things ruled out is a number a person can do nothing with"
    );

    // An hour-old ruling is spent; a fresh one still holds. This calls the
    // predicate the poller calls: a test that re-implements it can agree with
    // itself while the code does something else entirely.
    let now = jiff::Timestamp::now();
    let f = state.forge.lock().await;
    assert!(
        !f.should_skip(&ProjectId::new("scratch"), now),
        "a two-hour-old ruling is re-checked"
    );
    assert!(
        f.should_skip(&ProjectId::new("fresh"), now),
        "a fresh one is not re-asked every pass"
    );
    drop(f);
    std::fs::remove_file(&db).ok();
}

/// What is configured, answered for the whole machine in one request.
///
/// The product could always answer this and only in a terminal, one repository
/// at a time. The failure that made it worth an endpoint is the last assertion
/// here: a `devplane.toml` that will not parse takes that repository's
/// prohibitions with it, and nothing on any screen said so.
#[tokio::test]
async fn setup_reads_every_registered_repository_and_names_the_one_that_will_not_parse() {
    let (addr, token, c) = boot(Policy::default()).await;

    let root = std::env::temp_dir().join(format!("vp-setup-{}", uuid::Uuid::new_v4().simple()));
    let broken = root.join("broken");
    let good = root.join("good");
    for d in [&broken, &good] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(
        good.join("devplane.toml"),
        "[gates]\ncheck = [\"cargo test\"]\n\n[policy]\nnever_auto = [\"Bash(git push:*)\"]\n",
    )
    .unwrap();
    std::fs::write(broken.join("devplane.toml"), "[gates]\ncheck = \"oops\"\n").unwrap();

    for d in [&broken, &good] {
        c.post(format!("http://{addr}/api/projects/trust"))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "path": d.to_string_lossy() }))
            .send()
            .await
            .unwrap();
    }

    let s = get_json(&c, &addr, "/api/setup", &token).await;
    assert!(s["machine"]["version"].is_string());
    assert!(s["machine"]["database"].is_string());

    let projects = s["projects"].as_array().expect("projects");
    let find = |name: &str| {
        projects
            .iter()
            .find(|p| p["root"].as_str().unwrap_or_default().ends_with(name))
            .unwrap_or_else(|| panic!("no project for {name}"))
    };

    // The file read back: the gate it will run and the rule it will enforce,
    // in the order the rules are evaluated.
    let ok = find("good");
    assert_eq!(ok["config"]["exists"], true);
    assert_eq!(ok["verified"], true);
    assert_eq!(ok["describes"]["gates"]["check"][0], "cargo test");
    assert_eq!(
        ok["describes"]["policy"]["deny"][0]["rule"],
        "Bash(git push:*)"
    );

    // And the one that cannot. The parser's own reason, not a boolean: a person
    // who is told only that a file is broken has to go and find out why.
    let bad = find("broken");
    let why = bad["error"].as_str().expect("the parser's reason");
    assert!(
        why.contains("invalid type") || why.contains("expected"),
        "{why}"
    );
    assert!(
        bad["describes"].is_null(),
        "a file that will not parse describes nothing"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// The setup endpoint is a credential like every other read on this surface.
#[tokio::test]
async fn setup_requires_the_token() {
    let (addr, _token, c) = boot(Policy::default()).await;
    let status = c
        .get(format!("http://{addr}/api/setup"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 401);
}

/// The offer: the rule that answers this call and the ones like it.
///
/// The half `explain --replay` always had and the inbox never did. Three
/// distinct calls in one family are the evidence; below that the offer is the
/// exact call, because one interruption says this command needed a decision and
/// says nothing about the shape of the ones like it.
#[tokio::test]
async fn a_repeated_permission_is_offered_the_rule_that_answers_its_family() {
    let (addr, token, c) = boot(Policy::default()).await;

    // Calls this machine has already seen, in the same directory. Nothing
    // allows them, so every one of them would interrupt.
    for cmd in ["cargo test --lib diff", "cargo test --doc", "cargo test -q"] {
        post(
            &c,
            &addr,
            "/devplane/hook",
            &token,
            &format!(
                r#"{{"hook_event_name":"PreToolUse","session_id":"s-hist","cwd":"/tmp/repo",
                    "tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#
            ),
        )
        .await;
    }

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s-off","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"cargo test --lib policy"}}"#;
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        &serde_json::json!({
            "session": "s-off", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: cargo test --lib policy", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        })
        .to_string(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let inbox = inbox_items(&c, &addr, &token).await;
    let item = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "permission")
        .expect("a permission item");
    let offer = &item["offer"];
    assert_eq!(offer["basis"], "family", "{item}");
    assert_eq!(
        offer["rule"], "Bash(cargo test *)",
        "the narrowest rule covering the family, not the one the agent asked for"
    );
    assert!(
        offer["covers"].as_u64().unwrap() >= 4,
        "three observed calls plus the one being asked about: {offer}"
    );

    // **Where it goes, and that nothing put it there.**
    //
    // It used to name an allow list in Devplane's own `[policy]`. There is no
    // such key any more — Devplane does not approve tool calls — so an offer
    // pointing at it would be worse than none: somebody pastes it, nothing
    // changes, and the next identical call interrupts again. A grant goes where
    // it is enforced: the agent's own settings. `/tmp/repo` is inside no
    // registered project, so the user-scope file is the honest answer.
    let file = offer["file"].as_str().unwrap();
    assert!(file.ends_with("settings.json"), "{file}");
    assert!(!file.contains("devplane.toml"), "{file}");
    assert_eq!(offer["section"], "permissions.allow");
}

/// One interruption is not evidence about the shape of the ones like it.
#[tokio::test]
async fn a_permission_seen_once_is_offered_only_its_own_call() {
    let (addr, token, c) = boot(Policy::default()).await;

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s-one","cwd":"/tmp/solo",
            "tool_name":"Bash","tool_input":{"command":"pnpm test --run"}}"#;
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        &serde_json::json!({
            "session": "s-one", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: pnpm test --run", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        })
        .to_string(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let inbox = inbox_items(&c, &addr, &token).await;
    let offer = &inbox[0]["offer"];
    assert_eq!(offer["basis"], "call", "{}", inbox[0]);
    assert_eq!(offer["rule"], "Bash(pnpm test --run)");
    assert_eq!(offer["covers"], 1);
}

/// A call no rule can cover says which reason it is, in words.
///
/// The absence is the interesting half: a blank where a rule belongs reads as a
/// surface that failed rather than one with nothing to say.
#[tokio::test]
async fn a_permission_no_rule_can_cover_says_why() {
    let (addr, token, c) = boot(Policy::default()).await;

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s-cmp","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"pnpm build && rm -rf dist"}}"#;
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        &serde_json::json!({
            "session": "s-cmp", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: pnpm build && rm -rf dist", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        })
        .to_string(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let inbox = inbox_items(&c, &addr, &token).await;
    assert!(inbox[0]["offer"].is_null(), "{}", inbox[0]);
    assert_eq!(inbox[0]["no_offer"]["reason"], "compound");
    assert!(
        inbox[0]["no_offer"]["sentence"]
            .as_str()
            .unwrap()
            .contains("several commands"),
        "the reason reaches the person as a sentence: {}",
        inbox[0]["no_offer"]
    );
}

/// What the offer costs, measured rather than asserted.
///
/// The inbox is polled by every open board and this feature adds a store query
/// per permission item. The number worth watching is not the endpoint's cost —
/// it is whether that cost grows with the number of permission items or with
/// the size of the event log, because only one of those is bounded by anything.
///
/// Recorded, not enforced: a timing assertion that fails on a loaded machine
/// teaches people to ignore tests.
#[tokio::test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
async fn sc007_the_offer_costs_one_query_per_permission_item() {
    let (addr, token, c) = boot(Policy::default()).await;

    // A log far larger than the evidence any one offer reads.
    for n in 0..400 {
        post(
            &c,
            &addr,
            "/devplane/hook",
            &token,
            &format!(
                r#"{{"hook_event_name":"PreToolUse","session_id":"s-bulk","cwd":"/tmp/repo",
                    "tool_name":"Bash","tool_input":{{"command":"cargo test --lib m{n}"}}}}"#
            ),
        )
        .await;
    }

    let poll = |c: reqwest::Client, addr: std::net::SocketAddr, token: String| async move {
        let t = std::time::Instant::now();
        for _ in 0..20 {
            inbox_items(&c, &addr, &token).await;
        }
        t.elapsed() / 20
    };

    let quiet = poll(c.clone(), addr, token.clone()).await;

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s-cost","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"cargo test --lib policy"}}"#;
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        &serde_json::json!({
            "session": "s-cost", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: cargo test --lib policy", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        })
        .to_string(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let inbox = inbox_items(&c, &addr, &token).await;
    let covered = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "permission")
        .and_then(|i| i["offer"]["covers"].as_u64())
        .expect("an offer");
    let busy = poll(c.clone(), addr, token.clone()).await;

    println!(
        "inbox latency: 400 observed calls · poll {quiet:?} with no permission item, \
         {busy:?} with one · the offer covers {covered} calls"
    );
}

/// Every row of the edge-case walk, through the daemon rather than the composer.
///
/// The requirement is about what a person reads, and the unit test beside `compose`
/// checks the sentences in isolation. This checks they survive the wire and
/// that each input really produces its own — a variant nothing reaches is a
/// sentence nobody will see.
#[tokio::test]
async fn every_way_a_rule_cannot_be_offered_reads_differently() {
    let (addr, token, c) = boot(Policy::default()).await;

    let raise = |session: &'static str, tool: &'static str, input: Value| {
        let (c, token) = (c.clone(), token.clone());
        async move {
            let payload = serde_json::json!({
                "hook_event_name": "PermissionRequest", "session_id": session,
                "cwd": "/tmp/walk", "tool_name": tool, "tool_input": input,
            });
            post(
                &c,
                &addr,
                "/devplane/decided",
                &token,
                &serde_json::json!({
                    "session": session, "verdict": "undecided", "rule": null,
                    "blocked": true, "subject": format!("{tool}: walk"),
                    "tool": tool, "payload": payload,
                })
                .to_string(),
            )
            .await;
        }
    };

    raise("w-cmp", "Bash", serde_json::json!({"command": "a && b"})).await;
    raise(
        "w-unapp",
        "Bash",
        serde_json::json!({"command": "eval \"$X\""}),
    )
    .await;
    raise("w-shape", "TodoWrite", serde_json::json!({"todos": []})).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let inbox = inbox_items(&c, &addr, &token).await;
    let mut said: Vec<String> = Vec::new();
    for want in ["w-cmp", "w-unapp", "w-shape"] {
        let item = inbox
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["run_id"] == want)
            .unwrap_or_else(|| panic!("{want} reached the inbox: {inbox}"));
        assert!(item["offer"].is_null(), "{want}: {item}");
        let sentence = item["no_offer"]["sentence"]
            .as_str()
            .unwrap_or_else(|| panic!("{want} has no reason: {item}"))
            .to_string();
        assert!(!sentence.trim().is_empty(), "{want} renders a blank");
        assert!(
            !said.contains(&sentence),
            "{want} reads the same as something else: {sentence}"
        );
        said.push(sentence);
    }

    // **A permission nobody named a call for.** Claude Code raises some through
    // a notification that carries no tool input, and skipping those left the one
    // blank this surface exists to avoid.
    post(
        &c,
        &addr,
        "/devplane/hook",
        &token,
        r#"{"hook_event_name":"Notification","session_id":"w-dialog","cwd":"/tmp/walk",
            "notification_type":"permission_prompt","message":"Allow network access?"}"#,
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let again = inbox_items(&c, &addr, &token).await;
    let dialog = again
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["run_id"] == "w-dialog")
        .unwrap_or_else(|| panic!("the dialog item reached the inbox: {again}"));
    assert!(dialog["offer"].is_null(), "{dialog}");
    assert_eq!(dialog["no_offer"]["reason"], "unknown_call", "{dialog}");

    // And the row that is about presence rather than absence: a session
    // Devplane only watches has no `allow` and no `deny`, and that is exactly
    // where a rule is the only remedy.
    let watched = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["run_id"] == "w-unapp")
        .unwrap();
    let actions = watched["actions"].as_array().unwrap();
    assert!(
        !actions.iter().any(|a| a == "allow" || a == "deny"),
        "an observed session cannot be answered from here: {actions:?}"
    );
    assert!(
        watched["no_offer"].is_object(),
        "and it still says why there is no rule"
    );
}

/// **A dead session's version is not a fact about this machine now.**
///
/// `doctor` reported *"726 releases behind a session on this machine"* off a run
/// that had been finished for four days. The sentence was true in the past tense
/// and printed in the present, about a gap that did not exist — which is the
/// exact failure this diagnostic is supposed to catch in the permission layer,
/// committed by the diagnostic itself.
///
/// **The gap itself was deleted on 2026-09-19** — it counted releases since a
/// frozen date, for a compatibility claim this product no longer makes. The
/// invariant it taught outlives it and is what this test now holds: a figure
/// about *this machine right now* counts live sessions only, and the numerator
/// and denominator move together or the ratio lies.
#[tokio::test]
async fn only_a_live_session_reports_the_release_this_machine_is_running() {
    use devplane::core::event::{Event, StatusSample};
    use devplane::core::{RunId, Source};

    let db = std::env::temp_dir().join(format!(
        "vp-ver-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, state) = boot_shared(Policy::default(), &db).await;

    let say_version = |run: &str| {
        let state = state.clone();
        let run = run.to_string();
        async move {
            let s = StatusSample {
                claude_version: Some("2.1.999".into()),
                ..Default::default()
            };
            state
                .ingest(
                    RunId::new(run),
                    Source::Hook,
                    Event::StatusSample(s),
                    None,
                    devplane::core::RunMode::Observed,
                )
                .await;
        }
    };

    say_version("alive").await;
    say_version("finished").await;

    let d = get_json(&c, &addr, "/api/diagnostics", &token).await;
    assert_eq!(
        d["gate"]["sessions_reporting_a_version"], 2,
        "both are live so far, so both count"
    );

    // One of them ends. It is now history, and history is not a claim about
    // what this machine is running.
    state
        .ingest(
            RunId::new("finished"),
            Source::Hook,
            Event::SessionEnded { reason: None },
            None,
            devplane::core::RunMode::Observed,
        )
        .await;

    let d = get_json(&c, &addr, "/api/diagnostics", &token).await;
    assert_eq!(
        d["gate"]["sessions_reporting_a_version"], 1,
        "an ended session still counted as reporting a version"
    );

    // And the thing that used to sit beside it is gone rather than empty. The
    // count of sessions *ahead of a baseline* measured the decay of a claim
    // this product stopped making; a key that is always `[]` would be worse
    // than its absence, because a reader would think it meant something.
    assert!(
        d["gate"].get("sessions_ahead_of_baseline").is_none(),
        "the baseline gap was deleted with the baseline: {}",
        d["gate"]
    );
    assert_eq!(
        d["gate"]["syntax_modelled_on"], "2.1.273",
        "the honest half survives: a release named, with nothing computed from it"
    );
    std::fs::remove_file(&db).ok();
}

/// **The property the feature claimed and did not have until 2026-09-20.**
///
/// A question held in a map on a live connection survived a person leaving for
/// the day and did not survive the daemon restarting: the run, the agent and
/// the question died together, the inbox said *"Nothing needs you"*, and the
/// run read `completed`. This walks the same path across a restart.
///
/// Four of the five durable-execution properties are asserted here: the asking
/// state persists outside the asking process, the wait costs nothing across the
/// restart, resume is addressed by an opaque token, and the answer is recorded
/// before anything is delivered. The fifth — a deadline — is `core::ask`'s, and
/// is off unless a project asks for it.
///
/// **The answer reaches the agent.** The original session is gone with the
/// daemon that held it, so the person's answer is delivered into a *resumed*
/// one and the row says so, which is the difference between this feature
/// working and this feature being polite about failing.
#[tokio::test]
async fn a_question_outlives_the_daemon_that_was_holding_it() {
    let _serial = common::one_agent_at_a_time();
    let Some(agent) = echo_agent_path() else {
        eprintln!("skipping: build the fixture with `cargo build -p devplane-acp --examples`");
        return;
    };
    // One database, two daemons — which is the whole test.
    let db = std::env::temp_dir().join(format!(
        "vp-restart-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let cwd = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let ask_id = {
        let (addr, token, c, state) = boot_shared(Policy::default(), &db).await;
        c.post(format!("http://{addr}/api/projects/trust"))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "path": cwd }))
            .send()
            .await
            .unwrap();
        let started: Value = c
            .post(format!("http://{addr}/api/dispatch"))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "agent": agent, "cwd": cwd }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let run = started["run_id"].as_str().expect("a run id").to_string();
        c.post(format!("http://{addr}/api/runs/{run}/prompt"))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "text": "this needs permission" }))
            .send()
            .await
            .unwrap();

        let mut ask = String::new();
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let inbox = inbox_items(&c, &addr, &token).await;
            if let Some(id) = inbox
                .as_array()
                .and_then(|a| a.first())
                .and_then(|i| i["ask"].as_str())
            {
                ask = id.to_string();
                break;
            }
        }
        assert!(!ask.is_empty(), "the ask reaches the inbox");

        // A graceful stop, which is the case that leaves the question
        // legitimately still answerable — as against an agent whose own turn
        // ended, which does not.
        state.shutdown().await;
        ask
    };

    // A second daemon on the same store, knowing nothing but what was written
    // down.
    let (addr, token, c, _state) = boot_shared(Policy::default(), &db).await;

    let asks = get_json(&c, &addr, "/api/asks", &token).await;
    let open = asks["open"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == ask_id.as_str())
        .expect("the ask survived the daemon that was holding it");
    assert_eq!(open["open"], true);
    assert_eq!(open["outcome"], "waiting for you");

    // And it is in front of the person again, rather than in a table they would
    // have to know to query.
    let inbox = inbox_items(&c, &addr, &token).await;
    assert!(
        inbox
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["ask"] == ask_id.as_str()),
        "a question nobody answered is still in the inbox after a restart"
    );

    // Still answerable, and the answer still arrives: the session the agent
    // left on disk is resumed and the person's own words are delivered into it.
    // The one thing that must not happen is a claim of a delivery that did not
    // occur, so the sentence has to name which of the two it was.
    let answered: Value = c
        .post(format!("http://{addr}/api/asks/{ask_id}/answer"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "decision": "allow", "from": "test" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(answered["open"], false, "the answer settles it");
    assert_eq!(answered["answered_from"], "test");
    let outcome = answered["outcome"].as_str().unwrap_or_default();
    assert!(
        outcome.contains("you answered it"),
        "the person is on the record: {outcome}"
    );
    assert_eq!(
        answered["delivery"], "resumed",
        "the agent that asked was gone, so the answer went into a resumed session"
    );
    assert!(
        outcome.contains("resumed session"),
        "the surface says the turn had already ended rather than implying a reply: {outcome}"
    );

    // And nothing is offered twice: the settled ask leaves the inbox.
    let inbox = inbox_items(&c, &addr, &token).await;
    assert!(
        !inbox
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["ask"] == ask_id.as_str()),
        "an answered ask stops being asked"
    );
}

/// **A question waiting from an earlier session is in the header, not only in
/// the inbox.**
///
/// The durable ask exists so a question outlives the process that asked it. The
/// summary line counts *sessions* by state, and an ask whose run has ended is
/// not a session — so without a number of its own, a machine with a question
/// waiting since yesterday prints `0 need you` and is telling the same lie the
/// feature was built to stop, one layer up.
#[tokio::test]
async fn an_ask_that_outlived_its_session_is_counted_in_the_summary() {
    let db = std::env::temp_dir().join(format!(
        "vp-summary-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, state) = boot_shared(Policy::default(), &db).await;

    // An ask nobody answered, on a run with no live session — which is what a
    // restart leaves behind.
    let ask = devplane::core::ask::Ask::new(
        devplane::core::AskId::new("ask-1"),
        devplane::core::RunId::new("run-gone"),
        devplane::core::ask::Asked {
            kind: devplane::core::ask::Kind::Question,
            request_id: "req".into(),
            message: "Keep the legacy route?".into(),
            payload: serde_json::json!({}),
            at: jiff::Timestamp::now(),
            deadline: devplane::core::ask::Deadline::Never,
        },
    );
    state.store.save_ask(&ask).await.unwrap();

    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(
        board["summary"]["asks_waiting"], 1,
        "the header counts what the inbox lists"
    );

    // And the two agree, which is the property that matters: a person reading
    // the line and then the list must not find different numbers of things
    // waiting on them.
    let inbox = inbox_items(&c, &addr, &token).await;
    let listed = inbox
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["ask"] == "ask-1")
        .count();
    assert_eq!(listed, 1, "the inbox lists it exactly once");

    // Answered, it leaves both.
    let mut answered = ask.clone();
    answered
        .answer(serde_json::json!({}), "test", jiff::Timestamp::now())
        .unwrap();
    state.store.save_ask(&answered).await.unwrap();
    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(board["summary"]["asks_waiting"], 0);
}

/// The close: an empty inbox says what the day came to.
///
/// **Every board in this category is built to be full.** An empty list rendered
/// as an absence is the surface failing at the exact moment it has the best
/// thing it will ever have to say.
#[tokio::test]
async fn an_empty_inbox_says_what_the_day_came_to() {
    let (addr, token, c) = boot(Policy::default()).await;

    let body = get_json(&c, &addr, "/api/inbox", &token).await;
    assert_eq!(
        body["items"].as_array().map(Vec::len),
        Some(0),
        "a fresh daemon has nothing in its inbox"
    );
    let close = &body["close"];
    assert_eq!(close["clear"], true);
    assert_eq!(
        close["quiet"], true,
        "a day with nothing in it is quiet, and gets a sentence rather than a table of zeroes"
    );
    assert!(
        close["sentences"].as_array().is_some_and(Vec::is_empty),
        "a quiet day has no tally sentences: {close}"
    );
    assert!(
        close["keeps_running"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "the close must say what continues while somebody is away"
    );
    // Nothing is scheduled, so nothing is promised.
    assert!(close["next"].is_null(), "{close}");
}

/// A poll is not a look.
///
/// The board fetches the inbox every couple of seconds. Advancing the boundary
/// on every fetch would erase the thing it exists to draw, and the failure would
/// be invisible — the hairline would simply always read `0m`.
#[tokio::test]
async fn the_boundary_moves_when_somebody_reads_and_not_when_a_page_polls() {
    let (addr, token, c) = boot(Policy::default()).await;

    // Nothing has been read, so there is no *last* to be since.
    let first = get_json(&c, &addr, "/api/inbox", &token).await;
    assert!(
        first["close"]["looked_at"].is_null(),
        "there is no boundary before the first look: {}",
        first["close"]
    );

    // Polling does not create one.
    for _ in 0..3 {
        let _ = get_json(&c, &addr, "/api/inbox", &token).await;
    }
    let polled = get_json(&c, &addr, "/api/inbox", &token).await;
    assert!(
        polled["close"]["looked_at"].is_null(),
        "a poll is not a look: {}",
        polled["close"]
    );

    // Reading does.
    let _ = get_json(&c, &addr, "/api/inbox?read=true", &token).await;
    let after = get_json(&c, &addr, "/api/inbox", &token).await;
    assert!(
        after["close"]["looked_at"].is_string(),
        "after a read there is a boundary: {}",
        after["close"]
    );
    // And the *hairline* stays absent, because under a minute is not a boundary
    // worth drawing — two different reasons to print nothing, and only one of
    // them is a decision.
    assert!(
        after["close"]["since_last_look"].is_null(),
        "a twelve-second gap is not a boundary: {}",
        after["close"]
    );
}

/// What was decided on somebody's behalf, and what the tool did, are two counts.
///
/// Folding them together would make the headline number a measure of how much
/// Devplane did, which is the opposite of what the seat is for.
#[tokio::test]
async fn the_tally_counts_what_was_decided_for_you_apart_from_what_the_tool_did() {
    let (addr, token, c) = boot(Policy::rules(&["Bash(rm *)".into()], &[])).await;

    // A rule refuses a call: a decision taken on somebody's behalf.
    post(
        &c,
        &addr,
        "/devplane/hook",
        &token,
        &serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s-close",
            "cwd": "/tmp",
            "tool_name": "Bash",
            "tool_input": { "command": "rm -rf /tmp/x" },
        })
        .to_string(),
    )
    .await;
    post(
        &c,
        &addr,
        "/devplane/decided",
        &token,
        &serde_json::json!({
            "session": "s-close",
            "verdict": "deny",
            "rule": "Bash(rm *)",
            "subject": "rm -rf /tmp/x",
            "tool": "Bash",
        })
        .to_string(),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let body = get_json(&c, &addr, "/api/inbox", &token).await;
    let close = &body["close"];
    // The inbox is not empty now, so only the boundary is served — which is the
    // other half of the contract: the tally is what an empty list is for.
    if body["items"].as_array().is_some_and(Vec::is_empty) {
        assert_eq!(close["quiet"], false, "a rule decided something: {close}");
        let text = close["sentences"].to_string();
        assert!(text.contains("by a rule"), "{text}");
        assert!(
            !text.contains('%'),
            "there is no rate anywhere in it: {text}"
        );
    }
}

/// **`doctor` answers *what is watched here* without anybody reading the
/// source.**
///
/// The product's headline sentence is about every agent on your machine, and
/// that sentence covers two different lists: an agent Devplane **drives**
/// reports through the protocol by construction, and an agent somebody started
/// **themselves** is readable only as far as that vendor publishes channels.
/// Collapsing the two is how the claim becomes half true without anybody lying,
/// and it had been collapsed in the README.
///
/// So the matrix is a command rather than a paragraph: a person deciding
/// whether to install this can find out what it covers first.
#[test]
fn doctor_says_what_is_watched_per_vendor_and_per_channel() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("doctor")
        .env("NO_COLOR", "1")
        .output()
        .expect("the binary runs");
    let text = String::from_utf8_lossy(&out.stdout);

    assert!(
        text.contains("watched"),
        "`doctor` does not say what is watched here"
    );

    // Every vendor that can be driven is named, because the failure this
    // catches is the second list silently inheriting the first one's length.
    for vendor in devplane::core::vendors::vendors() {
        assert!(
            text.contains(vendor),
            "`doctor` does not mention `{vendor}`, which this machine can drive"
        );
    }

    // **The three reaches are distinguishable in the output**, not only in the
    // type. `unproved` is the one that matters: a channel that is built and has
    // never been run is neither working nor absent, and printing it as either
    // is the lie.
    for word in ["read", "unproved", "not published"] {
        assert!(
            text.contains(word),
            "`doctor` never prints `{word}`, so the three states cannot be told apart on screen"
        );
    }

    // And the reason travels with the row: a state with no reason is a verdict
    // a person cannot act on.
    assert!(
        text.contains("no session roster"),
        "`doctor` reports a channel as absent without saying what that costs"
    );

    // The table is a claim about somebody else's product, so it carries the
    // date it was checked. A row nobody re-reads is how this table came to say
    // Copilot had no permission event while its own reference documented one.
    assert!(
        text.contains(devplane::core::vendors::CHECKED),
        "`doctor` prints vendor facts with no date, so nobody can tell how stale they are"
    );
}

/// **An empty list names the vendors it cannot see — on either way of being
/// empty.**
///
/// `devplane ls` has two empty states and they are different facts: Claude Code
/// is installed and running nothing, or Claude Code is not here at all. Both are
/// lists that show nothing, and both have to say what Devplane could not have
/// shown — a session opened in Codex never appears, and nothing else on the
/// machine says so.
///
/// **The second branch is the one a person who does not use Claude Code sees**,
/// and it said nothing about them: it told them to install a vendor they had not
/// chosen and left their own question open.
///
/// # Why this is driven rather than observed
///
/// The first version read whatever `ls` printed on the machine running it. It
/// passed here, where `claude` is on `PATH`, and failed on CI, where it is not —
/// it had asserted a precondition that is a property of the developer's laptop.
/// A guard whose branch depends on the host tests the host.
#[test]
fn an_empty_list_names_the_vendors_it_cannot_see() {
    let driven_only = core_vendors_driven_only();
    assert!(
        !driven_only.is_empty(),
        "this guard assumes at least one vendor is driven-only; if that changed, \
         the empty-list sentence needs rewriting rather than this assert relaxing"
    );

    // An empty roster, so neither branch finds a session to list.
    let claude = gate_home("empty-list-claude");
    std::fs::create_dir_all(claude.join("projects")).expect("an empty roster");

    // `claude_binary()` takes `DEVPLANE_CLAUDE_BIN` when it names a real file,
    // then falls back to `PATH`. Both branches are reached by controlling those
    // two rather than by hoping about the machine.
    let cases: [(&str, &str, &str); 2] = [
        (
            "installed and quiet",
            "/bin/echo",
            "No Claude Code sessions are running",
        ),
        (
            "not installed at all",
            "/nonexistent/claude",
            "Claude Code was not found on this machine",
        ),
    ];

    for (what, bin, expect) in cases {
        let home = gate_home("empty-list");
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("ls")
            .env("NO_COLOR", "1")
            .env("DEVPLANE_HOME", &home)
            .env("CLAUDE_CONFIG_DIR", &claude)
            .env("DEVPLANE_CLAUDE_BIN", bin)
            // Emptied so the fallback cannot find a `claude` the developer has
            // and CI does not. This is the difference that broke the first
            // version of this guard.
            //
            // `HOME` too: the lookup also tries the native installer's path and
            // a VS Code extension under it, and this machine has the second.
            .env("PATH", "/nonexistent")
            .env("HOME", &home)
            .output()
            .expect("the binary runs");
        let text = String::from_utf8_lossy(&out.stdout);

        assert!(
            text.contains(expect),
            "`{what}`: expected the empty list to say `{expect}`. Output was:\n{text}"
        );
        for vendor in &driven_only {
            assert!(
                text.contains(vendor.as_str()),
                "`{what}`: an empty list without saying it cannot see `{vendor}`. \
                 A session opened there never appears, and nothing else says so.\n{text}"
            );
        }
        assert!(
            text.contains("not listed here"),
            "`{what}`: the unwatchable vendors are named without saying what that means"
        );
    }
}

/// The driven-only vendors, as the product's own table reports them.
fn core_vendors_driven_only() -> Vec<String> {
    devplane::core::vendors::watching()
        .driven_only
        .iter()
        .map(|v| (*v).to_string())
        .collect()
}

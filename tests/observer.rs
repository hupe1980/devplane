//! End-to-end tests for the observer.
//!
//! These drive the real HTTP surface with the payloads Claude Code actually
//! sends, because every interesting bug in this product lives at that seam: a
//! hook whose shape changed, a permission answered a millisecond too late, a
//! notification that empties the inbox instead of filling it.

mod common;

use serde_json::Value;
use std::net::SocketAddr;
use std::path::Path;
use vibeplane::core::Policy;
use vibeplane::daemon::{AppState, Shared};

/// Boots a daemon on an ephemeral port with a throwaway store.
async fn boot(policy: Policy) -> (SocketAddr, String, reqwest::Client) {
    let db = std::env::temp_dir().join(format!(
        "vp-it-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let state = boot_with_db(&db, "test-token".into(), policy).await;
    let addr = listen(state).await;
    (addr, "test-token".to_string(), reqwest::Client::new())
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
    let app = vibeplane::api::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    addr
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

#[tokio::test]
async fn a_session_that_asks_a_question_reaches_the_inbox() {
    let (addr, token, c) = boot(Policy::default()).await;

    // The shape Claude Code posts for an `AskUserQuestion` tool call.
    post(
        &c,
        &addr,
        "/vibeplane/hook",
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
        "/vibeplane/hook",
        &token,
        r#"{"hook_event_name":"Stop","session_id":"s1","cwd":"/tmp/repo"}"#,
    )
    .await;

    let inbox = get_json(&c, &addr, "/api/inbox", &token).await;
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

    let reply = post(
        &c,
        &addr,
        "/vibeplane/policy",
        &token,
        r#"{"hook_event_name":"PermissionRequest","session_id":"s2","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"rm -rf node_modules"}}"#,
    )
    .await;

    // An empty object means "no decision": Claude Code prompts exactly as it
    // would have. Anything else would change what the user sees.
    assert_eq!(reply, "{}");

    // The block is recorded asynchronously, after the reply.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let inbox = get_json(&c, &addr, "/api/inbox", &token).await;
    assert_eq!(inbox[0]["kind"], "permission");
    assert!(
        inbox[0]["detail"].as_str().unwrap().contains("rm -rf"),
        "the human needs to see the command to decide"
    );
}

#[tokio::test]
async fn a_matching_rule_answers_without_bothering_anyone() {
    let policy = Policy::new(&["Bash(pnpm test *)".into()], &["Bash(git push *)".into()]);
    let (addr, token, c) = boot(policy).await;

    let allow = post(
        &c,
        &addr,
        "/vibeplane/policy",
        &token,
        r#"{"hook_event_name":"PermissionRequest","session_id":"s3","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"pnpm test -- --run"}}"#,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&allow).unwrap();
    assert_eq!(v["hookSpecificOutput"]["decision"]["behavior"], "allow");

    let deny = post(
        &c,
        &addr,
        "/vibeplane/policy",
        &token,
        r#"{"hook_event_name":"PermissionRequest","session_id":"s3","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"git push --force origin main"}}"#,
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&deny).unwrap();
    assert_eq!(v["hookSpecificOutput"]["decision"]["behavior"], "deny");

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let inbox = get_json(&c, &addr, "/api/inbox", &token).await;
    assert!(
        inbox.as_array().unwrap().is_empty(),
        "a decided permission is not a decision the human has to make"
    );
}

#[tokio::test]
async fn the_policy_gate_answers_fast_enough_to_be_invisible() {
    // The session is blocked while this runs. A gate that takes long enough to
    // notice is a gate that makes Claude Code feel slower than it is.
    let policy = Policy::new(&["Bash(ls *)".into()], &[]);
    let (addr, token, c) = boot(policy).await;
    let body = r#"{"hook_event_name":"PermissionRequest","session_id":"s4","cwd":"/tmp/repo",
        "tool_name":"Bash","tool_input":{"command":"ls -la"}}"#;

    // Warm the connection so the measurement is the handler, not the TCP setup.
    post(&c, &addr, "/vibeplane/policy", &token, body).await;

    let mut worst = std::time::Duration::ZERO;
    for _ in 0..50 {
        let t = std::time::Instant::now();
        post(&c, &addr, "/vibeplane/policy", &token, body).await;
        worst = worst.max(t.elapsed());
    }
    assert!(
        worst < std::time::Duration::from_millis(50),
        "worst policy round trip was {worst:?}; the budget is 50ms over loopback"
    );
}

#[tokio::test]
async fn telemetry_gives_the_run_its_cost_and_context() {
    let (addr, token, c) = boot(Policy::default()).await;
    post(
        &c,
        &addr,
        "/vibeplane/hook",
        &token,
        r#"{"hook_event_name":"UserPromptSubmit","session_id":"s5","cwd":"/tmp/repo","prompt":"hi"}"#,
    )
    .await;

    // OTLP/HTTP JSON, as Claude Code's exporter sends it: 64-bit integers are
    // strings.
    c.post(format!("http://{addr}/vibeplane/otel/v1/logs"))
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
    let inbox = get_json(&c, &addr, "/api/inbox", &token).await;
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
            "/vibeplane/hook",
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
    assert!(body.contains("<title>Vibeplane</title>"));
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
        "/vibeplane/hook",
        &token,
        r#"{"hook_event_name":"PreToolUse","session_id":"s9","cwd":"/tmp/repo",
            "tool_name":"AskUserQuestion",
            "tool_input":{"questions":[{"question":"which?","options":[]}]}}"#,
    )
    .await;
    assert_eq!(
        get_json(&c, &addr, "/api/inbox", &token)
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
        get_json(&c, &addr, "/api/inbox", &token)
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
        get_json(&c, &addr, "/api/inbox", &token)
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
        .post(format!("http://{addr}/vibeplane/hook"))
        .bearer_auth(&token)
        .header("content-type", "application/json")
        .body("{ not json at all")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());

    let reply = post(&c, &addr, "/vibeplane/policy", &token, "{ also not json }").await;
    assert_eq!(
        reply, "{}",
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
            "/vibeplane/hook",
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

/// The fixture agent from `vibeplane-acp`, built as a sibling of this binary.
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
        eprintln!("skipping: build the fixture with `cargo build -p vibeplane-acp --examples`");
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
        let inbox = get_json(&c, &addr, "/api/inbox", &token).await;
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
    let request_id = item["request_id"].as_str().expect("something to answer");

    c.post(format!("http://{addr}/api/runs/{run}/decide"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "request_id": request_id, "option_id": "allow" }))
        .send()
        .await
        .unwrap();

    let mut cleared = false;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if get_json(&c, &addr, "/api/inbox", &token)
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
    // bearer token, so only something that can read `~/.vibeplane/token` can
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
    let policy = Policy::with_ask(
        &["Bash(pnpm test *)".into()],
        &["Bash(git push *)".into()],
        &["Bash(gh release *)".into()],
    );
    let (addr, token, c) = boot(policy).await;

    let pre = |cmd: &str| {
        format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"auto","cwd":"/tmp/repo",
                "permission_mode":"auto","tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#
        )
    };

    // A deny stops the call, and says which rule did it.
    let denied = post(
        &c,
        &addr,
        "/vibeplane/policy",
        &token,
        &pre("git push --force"),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&denied).unwrap();
    assert_eq!(
        v["hookSpecificOutput"]["permissionDecision"], "deny",
        "a never_auto rule has to reach auto mode or it protects nothing"
    );
    assert!(
        v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("Bash(git push *)"),
        "and the reason names the rule"
    );

    // An ask forces the prompt the classifier would otherwise have skipped.
    let asked = post(
        &c,
        &addr,
        "/vibeplane/policy",
        &token,
        &pre("gh release create v1"),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&asked).unwrap();
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "ask");

    // An *allowed* command is answered with nothing at all. The call still
    // goes through the classifier, which is the layer this must not remove.
    let allowed = post(
        &c,
        &addr,
        "/vibeplane/policy",
        &token,
        &pre("pnpm test -- --run"),
    )
    .await;
    assert_eq!(
        allowed, "{}",
        "a PreToolUse allow would skip the classifier as well as the prompt"
    );

    // And the prohibition is accounted for afterwards, like every other verdict.
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

/// A `Read` deny has to survive the trip through a shell command, because
/// `cat .env` is reading `.env` by any honest reading of the rule.
#[tokio::test]
async fn a_file_rule_reaches_the_files_a_shell_command_names() {
    let policy = Policy::new(
        &["Bash(cat *)".into(), "Bash(echo *)".into()],
        &["Read(.env)".into()],
    );
    let (addr, token, c) = boot(policy).await;

    for cmd in ["cat .env", "echo pwned > .env"] {
        let body = format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"sh","cwd":"/tmp/repo",
                "tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#
        );
        let reply = post(&c, &addr, "/vibeplane/policy", &token, &body).await;
        let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
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
    // A launch link names a repository rather than a path, so Vibeplane reads
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
        post(&c, &addr, "/vibeplane/hook", "t", &ev(n)).await;
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

//! End-to-end tests for the observer, driven with the payloads Claude Code sends.
//!
//! A test feeds the host as a hook does — `record::*` into the host's store,
//! then one pass of the tail — and reads the result over the API.

mod common;

use devplane::core::Policy;
use devplane::host::{AppState, Shared};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

/// Boots a host on an ephemeral port with a throwaway store.
async fn boot(policy: Policy) -> (SocketAddr, String, reqwest::Client) {
    let db = std::env::temp_dir().join(format!(
        "vp-it-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, _state) = boot_shared(policy, &db).await;
    (addr, token, c)
}

/// [`boot`], keeping the state, for tests that feed the host through its store.
async fn boot_with_state(policy: Policy) -> (SocketAddr, String, reqwest::Client, Shared) {
    let db = std::env::temp_dir().join(format!(
        "vp-it-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    boot_shared(policy, &db).await
}

/// Keeps the state so a test can reach `gate_down`, which nothing observable exposes.
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
    .expect("host state")
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

/// Runs the gate as Claude Code does: the binary, a payload on stdin, the
/// verdict on stdout, with our own `~/.devplane` and no host.
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

/// The inbox's items (`/api/inbox` answers `{ items, close }`).
async fn inbox_items(c: &reqwest::Client, addr: &SocketAddr, token: &str) -> serde_json::Value {
    get_json(c, addr, "/api/inbox", token).await["items"].clone()
}

/// Feeds an observation as `devplane hook` does: appended to the store, folded
/// in on the host's next pass of the tail.
async fn hook(state: &Shared, payload: &str) {
    let value: Value = serde_json::from_str(payload).expect("a hook payload");
    ensure_cwd(&value);
    let payload: devplane::observe::hook::HookPayload =
        serde_json::from_value(value).expect("a hook payload");
    devplane::record::observe(&state.store, &payload, devplane::core::Source::Hook)
        .await
        .unwrap();
    devplane::poller::tail_once(state).await.unwrap();
}

/// Files a verdict the gate already gave, as the hook does after answering.
async fn decided(state: &Shared, env: Value) {
    if let Some(payload) = env.get("payload") {
        ensure_cwd(payload);
    }
    let env: devplane::core::DecidedEnvelope = serde_json::from_value(env).expect("an envelope");
    devplane::record::decided(&state.store, env).await.unwrap();
    devplane::poller::tail_once(state).await.unwrap();
}

/// Creates the directory a payload names; the host only folds a row into a
/// project whose directory it can see.
fn ensure_cwd(payload: &Value) {
    if let Some(cwd) = payload.get("cwd").and_then(|v| v.as_str()) {
        std::fs::create_dir_all(cwd).ok();
    }
}

/// Holds a watched session's permission in its own task, as a `PermissionRequest`
/// hook does while the session waits.
fn hold(
    state: &Shared,
    session: &'static str,
    call: &'static str,
    wait: Duration,
) -> tokio::task::JoinHandle<Option<String>> {
    let state = state.clone();
    tokio::spawn(async move {
        devplane::record::hold(
            &state.store,
            session,
            Path::new("/tmp/alpha"),
            "Bash",
            call,
            wait,
            false,
        )
        .await
    })
}

#[tokio::test]
async fn a_session_that_asks_a_question_reaches_the_inbox() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    // The shape Claude Code hands the hook for an `AskUserQuestion` tool call.
    hook(
        &state,
        r#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/tmp/repo",
            "tool_name":"AskUserQuestion",
            "tool_input":{"questions":[{"question":"Keep the legacy route?",
              "options":[{"label":"Keep"},{"label":"Remove"}]}]}}"#,
    )
    .await;
    // The turn ends immediately afterwards; the question must survive it.
    hook(
        &state,
        r#"{"hook_event_name":"Stop","session_id":"s1","cwd":"/tmp/repo"}"#,
    )
    .await;

    let inbox = inbox_items(&c, &addr, &token).await;
    assert_eq!(inbox.as_array().unwrap().len(), 1);
    assert_eq!(inbox[0]["kind"], "question");
    // Human-readable, no id: only the person at Claude Code's dialog can answer.
    assert_eq!(inbox[0]["options"][1]["label"], "Remove");
    assert!(inbox[0]["options"][1]["id"].is_null());

    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(board["summary"]["needs_you"], 1);
}

#[tokio::test]
async fn a_permission_no_rule_covers_is_left_to_claude_and_shown_to_the_human() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s2","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"rm -rf node_modules"}}"#;

    // An empty object means "no decision": Claude Code prompts as it would have.
    let reply = gate(&gate_home("nocover"), "", payload);
    assert_eq!(reply, serde_json::json!({}));

    decided(
        &state,
        serde_json::json!({
            "session": "s2", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: rm -rf node_modules", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        }),
    )
    .await;
    let inbox = inbox_items(&c, &addr, &token).await;
    assert_eq!(inbox[0]["kind"], "permission");
    assert!(
        inbox[0]["detail"].as_str().unwrap().contains("rm -rf"),
        "the human needs to see the command to decide"
    );
}

/// With no host, a prohibition is refused naming its rule, an `always_ask` rule
/// still reaches a person, and an uncovered call is left alone. Nothing approves.
#[test]
fn the_gate_decides_with_no_host_running() {
    let home = gate_home("no-host");
    let rules = r#"
[policy]
never_auto = ["Bash(rm -rf *)"]
always_ask = ["Bash(git push *)"]
"#;
    let denied = gate(
        &home,
        rules,
        r#"{"hook_event_name":"PreToolUse","session_id":"s","tool_name":"Bash",
            "tool_input":{"command":"rm -rf node_modules"}}"#,
    );
    let out = &denied["hookSpecificOutput"];
    assert_eq!(out["permissionDecision"], "deny", "got {denied}");
    assert!(
        out["permissionDecisionReason"]
            .as_str()
            .is_some_and(|r| r.contains("rm -rf")),
        "a refusal must name the rule that refused it: {denied}"
    );

    let asked = gate(
        &home,
        rules,
        r#"{"hook_event_name":"PreToolUse","session_id":"s","tool_name":"Bash",
            "tool_input":{"command":"git push origin main"}}"#,
    );
    assert_eq!(asked["hookSpecificOutput"]["permissionDecision"], "ask");

    // And a call no rule covers is left alone rather than guessed at.
    let untouched = gate(
        &home,
        rules,
        r#"{"hook_event_name":"PreToolUse","session_id":"s","tool_name":"Bash",
            "tool_input":{"command":"cargo test"}}"#,
    );
    assert_eq!(untouched, serde_json::json!({}), "got {untouched}");
}

#[tokio::test]
async fn a_prohibition_answers_and_nothing_is_approved() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
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
    // No approval, whatever the project wrote: the vendor's permission system decides that.
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
    decided(
        &state,
        serde_json::json!({
            "session": "s3", "verdict": "deny", "rule": "Bash(git push *)",
            "subject": "Bash: git push --force origin main", "tool": "Bash",
        }),
    )
    .await;
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
    // The session is blocked while the gate runs, so it must not be felt. This
    // spawns the binary, as Claude Code does.
    let home = gate_home("latency");
    let body = r#"{"hook_event_name":"PermissionRequest","session_id":"s4","cwd":"/tmp/repo",
        "tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}"#;
    let rules = "[policy]\nnever_auto = [\"Bash(rm *)\"]\n";

    // Warm the page cache so the first read of the debug binary is not measured.
    gate(&home, rules, body);

    // Control: `--version` shares the spawn cost and none of the path under test,
    // so the difference is reading the hook and deciding it. The control must not
    // share rule evaluation, or a slow evaluator cancels out.
    let control_run = || {
        let t = std::time::Instant::now();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("--version")
            .output()
            .expect("the binary runs");
        assert!(out.status.success(), "the control must succeed");
        t.elapsed()
    };
    control_run();

    let mut runs: Vec<std::time::Duration> = Vec::new();
    let mut controls: Vec<std::time::Duration> = Vec::new();
    for _ in 0..20 {
        let t = std::time::Instant::now();
        let reply = gate(&home, rules, body);
        runs.push(t.elapsed());
        // A prohibition's latency is the one that must not be felt.
        assert_eq!(reply["hookSpecificOutput"]["decision"]["behavior"], "deny");

        controls.push(control_run());
    }
    runs.sort();
    controls.sort();
    let median = runs[runs.len() / 2];
    let control = controls[controls.len() / 2];
    let worst = *runs.last().expect("twenty runs");

    // The budget is the difference over process start.
    let own_cost = median.saturating_sub(control);
    assert!(
        own_cost < std::time::Duration::from_millis(60),
        "deciding a prohibition cost {own_cost:?} more than the same binary answering \
         --version (gate {median:?}, control {control:?}, {} runs each). The spawn is shared and \
         the policy path is not, so this difference is reading the hook and deciding it.",
        runs.len()
    );

    // The median carries it: a gate regression moves every run, while scheduler
    // noise from parallel test binaries moves the tail.
    // A loose absolute ceiling, only to catch a gate that has become unusable.
    assert!(
        median < std::time::Duration::from_millis(1_500),
        "median gate answer was {median:?} over {} runs, which is unusable whatever the machine \
         is doing. The statistic that carries the budget is the difference from the control.",
        runs.len()
    );

    // A loose tail ceiling, so a single hang is still caught.
    assert!(
        worst < std::time::Duration::from_secs(5),
        "worst gate answer was {worst:?}. The median is the budget; this catches a hang, \
         and five seconds is not something scheduler noise produces."
    );
}

/// The page renders agent-written text and holds the token, so it may reach
/// no other origin, tell no linked page where it came from, and be framed by
/// nothing.
#[tokio::test]
async fn the_page_is_served_confined_to_this_host() {
    let (addr, _token, c) = boot(Policy::default()).await;
    let r = c.get(format!("http://{addr}/")).send().await.unwrap();
    let h = r.headers();
    let csp = h["content-security-policy"].to_str().unwrap();
    for part in [
        "default-src 'self'",
        "connect-src 'self'",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.contains(part), "{csp}");
    }
    assert_eq!(h["referrer-policy"], "no-referrer");
    assert_eq!(h["x-content-type-options"], "nosniff");
}

/// Vendor settings are read by the agent they configure, so the token they
/// carry writes telemetry and opens nothing else — not the inbox, not an
/// answer, not a trust.
#[tokio::test]
async fn the_telemetry_token_opens_only_the_telemetry_routes() {
    let (addr, token, c) = boot(Policy::default()).await;
    let ingest = devplane::config::ingest_token(&token);
    assert_ne!(ingest, token);
    let otel = c
        .post(format!("http://{addr}/devplane/otel/v1/logs"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {ingest}"))
        .body(r#"{"resourceLogs":[]}"#)
        .send()
        .await
        .unwrap();
    assert_ne!(otel.status(), 401, "the telemetry route accepts it");
    for path in ["/api/board", "/api/inbox"] {
        let r = c
            .get(format!("http://{addr}{path}"))
            .header("authorization", format!("Bearer {ingest}"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401, "{path} refuses the telemetry token");
    }
    let r = c
        .post(format!("http://{addr}/api/projects/trust"))
        .header("authorization", format!("Bearer {ingest}"))
        .json(&serde_json::json!({"path": "/tmp"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "trust refuses the telemetry token");
}

#[tokio::test]
async fn telemetry_gives_the_run_its_cost_and_context() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
    hook(
        &state,
        r#"{"hook_event_name":"UserPromptSubmit","session_id":"s5","cwd":"/tmp/repo","prompt":"hi"}"#,
    )
    .await;

    // OTLP/HTTP JSON as Claude Code exports it: 64-bit integers are strings.
    c.post(format!("http://{addr}/devplane/otel/v1/logs"))
        .header("content-type", "application/json")
        // The bearer `observe::connect` writes into `OTEL_EXPORTER_OTLP_HEADERS`:
        // the telemetry-only one.
        .header(
            "authorization",
            format!("Bearer {}", devplane::config::ingest_token(&token)),
        )
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
    // 180k of 200k; the gauge counts input plus cache, never output.
    assert_eq!(run["context_percent"], 90.0);

    let inbox = inbox_items(&c, &addr, &token).await;
    assert_eq!(inbox[0]["kind"], "context_high");
}

#[tokio::test]
async fn a_worktree_session_belongs_to_the_repository_that_owns_it() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
    for (session, cwd) in [
        ("main", "/tmp/vp-repo"),
        ("wt-a", "/tmp/vp-repo/.claude/worktrees/feature-a"),
        ("wt-b", "/tmp/vp-repo/.claude/worktrees/feature-b"),
    ] {
        hook(
            &state,
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
    // The live stream too: an empty stream is indistinguishable from a quiet machine.
    assert_eq!(
        c.get(format!("http://{addr}/api/stream"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // The health probe is open: it proves the port is ours and reveals nothing.
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
    // The page carries no data and cannot fetch any without the token.
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
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
    hook(
        &state,
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

#[test]
fn a_malformed_hook_payload_never_fails_the_session() {
    // A hook that cannot parse its input must not interrupt the work: exit zero
    // and an empty object. Run with no rules; under rules it asks (`tests/hook.rs`).
    let home = gate_home("garbage");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("hook")
        .current_dir(&home)
        .env("DEVPLANE_HOME", &home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"{ not json at all")?;
            child.wait_with_output()
        })
        .expect("the hook runs");
    assert!(
        out.status.success(),
        "an unreadable payload failed the hook, which Claude Code shows as an error"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "{}",
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
        let addr = listen(state.clone()).await;
        let c = reqwest::Client::new();
        hook(
            &state,
            r#"{"hook_event_name":"PreToolUse","session_id":"keep","cwd":"/tmp/repo",
                "tool_name":"Bash","tool_input":{"command":"cargo build"}}"#,
        )
        .await;
        let board = get_json(&c, &addr, "/api/board", "tok").await;
        assert_eq!(board["summary"]["runs"], 1);
    }

    let state = boot_with_db(&db, "tok".into(), Policy::default()).await;
    let addr = listen(state.clone()).await;
    let c = reqwest::Client::new();
    let board = get_json(&c, &addr, "/api/board", "tok").await;
    assert_eq!(board["summary"]["runs"], 1, "the run survived the restart");
    assert_eq!(board["runs"][0]["summary"], "Bash: cargo build");

    let events = state
        .store
        .events_for_run(&devplane::core::RunId::new("keep"), 200)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);

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
    // The answer happens here rather than in a terminal, end to end.
    let _serial = common::one_agent_at_a_time();
    let Some(agent) = echo_agent_path() else {
        eprintln!("skipping: build the fixture with `cargo build -p devplane-acp --examples`");
        return;
    };
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
    let cwd = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Every path that starts an agent goes through the trust gate.
    c.post(format!("http://{addr}/api/projects/trust"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "path": cwd }))
        .send()
        .await
        .unwrap();

    let spec = devplane::acp::resolve(&agent, &state.agents).expect("the fixture resolves");
    let run = devplane::driven::dispatch(&state, &spec, cwd.clone().into(), None, Vec::new())
        .await
        .expect("a run id")
        .to_string();

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

    // Poll for the request rather than sleeping a fixed time.
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
    // Answered by the ask's own id, valid from any surface after the process is gone.
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

    // Answered exactly once: a second surface is told who answered.
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

    // And it is on the record afterwards.
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
    let cwd = std::env::temp_dir().canonicalize().unwrap();
    c.post(format!("http://{addr}/api/projects/trust"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "path": cwd }))
        .send()
        .await
        .unwrap();
    let res: Value = c
        .post(format!("http://{addr}/api/changes"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "agent": "clauude", "cwd": cwd, "title": "x" }))
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
async fn stopping_the_host_is_a_request_rather_than_a_signal() {
    // A crash's record may name a pid now reused; stopping requires the bearer token.
    let (addr, token, c) = boot(Policy::default()).await;

    let refused = c
        .post(format!("http://{addr}/api/quit"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        refused.status(),
        401,
        "without the token it is nobody's business"
    );

    let ok = c
        .post(format!("http://{addr}/api/quit"))
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
        .post(format!("http://{addr}/api/changes"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "agent": agent, "cwd": "/no/such/place", "title": "x" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(res["error"].as_str().unwrap_or("").contains("directory"));
}

/// `PreToolUse` answers a prohibition in every mode, including auto mode where
/// `PermissionRequest` never fires. It never answers allow, which would skip
/// Claude Code's classifier.
#[tokio::test]
async fn a_prohibition_reaches_a_session_that_is_never_going_to_prompt() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
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

    // An allowed command gets nothing, so the classifier still runs.
    assert_eq!(
        gate(&home, rules, &pre("pnpm test -- --run")),
        serde_json::json!({}),
        "a PreToolUse allow would skip the classifier as well as the prompt"
    );

    // And the prohibition is recorded from the envelope the gate writes.
    decided(
        &state,
        serde_json::json!({
            "session": "auto", "verdict": "deny", "rule": "Bash(git push *)",
            "subject": "Bash: git push --force", "tool": "Bash",
        }),
    )
    .await;
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

/// A stopped host costs nothing: the gate still enforces, and the decision goes
/// straight into the store (the spool is only for a store that will not open).
#[tokio::test]
async fn the_gate_decides_with_no_host_and_files_the_decision_afterwards() {
    let home = gate_home("nohost");
    let rules = "[policy]\nnever_auto = [\"Read(.env)\"]\n";
    let payload = r#"{"hook_event_name":"PreToolUse","session_id":"offline","cwd":"/tmp/repo",
        "tool_name":"Bash","tool_input":{"command":"cat .env"}}"#;

    // Fresh `DEVPLANE_HOME`: no `host.json` for the gate to find.
    assert!(!home.join("host.json").exists());
    let reply = gate(&home, rules, payload);
    assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");

    // Recorded in the ledger with the rule and the time it was taken.
    assert!(
        !home.join("pending-decisions.jsonl").exists(),
        "a store that opens is written, not spooled"
    );
    let store = devplane::store::Store::open(&home.join("devplane.db"))
        .await
        .expect("the gate left a store behind");
    let rows = store.decisions(Some("offline"), 10).await.unwrap();
    assert_eq!(rows.len(), 1, "one refusal, one row: {rows:?}");
    assert_eq!(rows[0].outcome, "deny");
    assert_eq!(rows[0].reason.as_deref(), Some("Read(.env)"));
    assert!(
        !rows[0]
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("filed late"),
        "written at the time, so nothing about it is late"
    );

    // An undecided call is observed but not a decision, so the ledger stays at one.
    let quiet = gate(
        &home,
        rules,
        r#"{"hook_event_name":"PreToolUse","session_id":"offline","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"ls -la"}}"#,
    );
    assert_eq!(quiet, serde_json::json!({}));
    let rows = store.decisions(Some("offline"), 10).await.unwrap();
    assert_eq!(rows.len(), 1, "observations are not decisions: {rows:?}");
    let seen = store.shim_events_since(0, 100).await.unwrap();
    assert!(
        seen.iter().any(|(_, e)| e.run_id.as_str() == "offline"),
        "the call itself is on the record for a host to fold in: {seen:?}"
    );
}

/// A `Read` deny survives a shell command: `cat .env` reads `.env`.
#[tokio::test]
async fn a_file_rule_reaches_the_files_a_shell_command_names() {
    // `Read` covers the read and `Edit` the write; both are needed to protect a
    // file from a shell.
    let home = gate_home("filerule");
    let rules = r#"
[policy]
never_auto = ["Read(.env)", "Edit(.env)"]
"#;
    // Globs: the shell expands the operand, so the matcher must treat it as a pattern.
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

/// On the built binary, every shell spelling of the prohibited command is a
/// refusal or a question, and a line that merely mentions it is neither.
#[test]
fn a_prohibition_is_never_walked_past_by_a_spelling() {
    let home = gate_home("spellings");
    let rules = "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n";
    let run = |cmd: &str| {
        let body = serde_json::json!({
            "hook_event_name": "PreToolUse", "session_id": "sp", "cwd": "/tmp/repo",
            "tool_name": "Bash", "tool_input": {"command": cmd},
        });
        gate(&home, rules, &body.to_string())
    };
    for cmd in [
        "rm  -rf /",
        "rm\t-rf /",
        "rm -fr /",
        "rm -r -f /",
        "rm -RF /",
        "RM -rf /",
        "sh -lc 'rm -rf /'",
        "bash -ec 'rm -rf /'",
        "bash -c'rm -rf /'",
        "bash x.sh",
        "python x.py",
        "echo / | xargs -0 rm -rf",
        "xargs -I{} rm -rf {}",
        "busybox rm -rf /",
        "exec -a x rm -rf /",
        "env -S 'rm -rf /'",
        "su -c 'rm -rf /'",
        "ssh h rm -rf /",
        "flock /tmp/l rm -rf /",
        "awk 'BEGIN{system(\"rm -rf /\")}'",
        "git -c core.pager='rm -rf /' log",
        "docker run x rm -rf /",
        "node --eval 'x'",
        "sh <<EOF\nrm -rf /\nEOF",
    ] {
        let v = run(cmd);
        let decision = v["hookSpecificOutput"]["permissionDecision"]
            .as_str()
            .unwrap_or("");
        assert!(
            decision == "deny" || decision == "ask",
            "{cmd:?} was handed to the vendor without a word: {v}"
        );
    }
    // A readable line that only mentions the command is nobody's business.
    for cmd in [
        "echo \"a; rm -rf x\"",
        "cat <<EOF\nrm -rf /\nEOF",
        "rm -r x",
    ] {
        assert_eq!(
            run(cmd),
            serde_json::json!({}),
            "{cmd:?} is readable and matches nothing"
        );
    }
}

/// On the built binary: shell reader spellings of `Bash(rm -rf *)` are refused
/// or asked; an exception excuses one simple command, never the line; an exact
/// rule compares its flags as a set.
#[test]
fn the_gate_fails_closed_on_every_hole_the_audit_found() {
    let home = gate_home("us9");
    let run = |rules: &str, cmd: &str| {
        let body = serde_json::json!({
            "hook_event_name": "PreToolUse", "session_id": "us9", "cwd": "/tmp/repo",
            "tool_name": "Bash", "tool_input": {"command": cmd},
        });
        let v = gate(&home, rules, &body.to_string());
        v["hookSpecificOutput"]["permissionDecision"]
            .as_str()
            .unwrap_or("undecided")
            .to_string()
    };
    let rm = "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n";
    for cmd in [
        "function f { rm -rf /; }; f",
        "coproc rm -rf /",
        "/bin/r? -rf /",
        "/bin/r[m] -rf /",
        "{rm,-rf,/}",
        "bash <<< 'rm -rf /'",
        "sh < x.sh",
        "builtin rm -rf /",
        "sudo --user root rm -rf /",
        "sudo -R /tmp rm -rf /",
        "env -P /bin rm -rf /",
        "/usr/bin/time -o f rm -rf /",
        "caffeinate -i rm -rf /",
        "arch -arm64 rm -rf /",
        "strace -f rm -rf /",
        "watch rm -rf /",
        "script -qc 'rm -rf /' /dev/null",
    ] {
        let d = run(rm, cmd);
        assert!(d == "deny" || d == "ask", "{cmd:?} was {d}");
    }

    let except = "[policy]\nnever_auto = [\"Bash(git *)\", \"!Bash(git status *)\"]\n";
    assert_eq!(run(except, "git status && git push --force"), "deny");
    assert_eq!(run(except, "git status"), "undecided");

    let exact = "[policy]\nnever_auto = [\"Bash(rm -rf /)\"]\n";
    for cmd in ["rm -rf /", "rm -r -f /", "rm -fr /"] {
        assert_eq!(run(exact, cmd), "deny", "{cmd}");
    }
}

/// Measures the gate on the release binary (cold, warm median, p95); run
/// `cargo build --release` first.
#[test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
fn measure_the_gate_on_the_release_binary() {
    let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/release/devplane");
    assert!(bin.is_file(), "build the release binary first");
    let home = gate_home("measure");
    std::fs::write(
        home.join("policy.toml"),
        "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n",
    )
    .unwrap();
    let run = |cmd: &str| {
        let body = serde_json::json!({
            "hook_event_name": "PreToolUse", "session_id": "m", "cwd": "/tmp/repo",
            "tool_name": "Bash", "tool_input": {"command": cmd},
        })
        .to_string();
        let t = std::time::Instant::now();
        let out = std::process::Command::new(&bin)
            .arg("hook")
            .env("DEVPLANE_HOME", &home)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.as_mut().unwrap().write_all(body.as_bytes())?;
                child.wait_with_output()
            })
            .expect("the gate runs");
        let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(serde_json::json!({}));
        (v, t.elapsed())
    };
    let cold = run("rm -rf /tmp/x").1;
    let mut warm: Vec<_> = (0..40).map(|_| run("rm -rf /tmp/x").1).collect();
    warm.sort();
    println!(
        "release gate: cold {cold:?}, warm median {:?}, p95 {:?}, min {:?}",
        warm[20], warm[37], warm[0]
    );
    for cmd in [
        "rm  -rf /",
        "rm\t-rf /",
        "rm -fr /",
        "rm -r -f /",
        "rm -RF /",
        "RM -rf /",
        "sh -lc 'rm -rf /'",
        "bash -ec 'rm -rf /'",
        "bash -c'rm -rf /'",
        "bash x.sh",
        "python x.py",
        "echo / | xargs -0 rm -rf",
        "xargs -I{} rm -rf {}",
        "busybox rm -rf /",
        "exec -a x rm -rf /",
        "env -S 'rm -rf /'",
        "su -c 'rm -rf /'",
        "ssh h rm -rf /",
        "flock /tmp/l rm -rf /",
        "awk 'BEGIN{system(\"rm -rf /\")}'",
        "git -c core.pager='rm -rf /' log",
        "docker run x rm -rf /",
        "node --eval 'require(\"child_process\").execSync(\"rm -rf /\")'",
        "echo \"a; rm -rf x\"",
        "cat <<EOF\nrm -rf /\nEOF",
        "sh <<EOF\nrm -rf /\nEOF",
    ] {
        let (v, _) = run(cmd);
        let out = &v["hookSpecificOutput"];
        println!(
            "{cmd:?} => {} {}",
            out["permissionDecision"].as_str().unwrap_or("undecided"),
            out["permissionDecisionReason"].as_str().unwrap_or("")
        );
    }
}

#[tokio::test]
async fn a_repository_with_no_remote_is_asked_about_once() {
    // A repository with no remote must not spawn `git remote get-url` on every
    // event: the host remembers having asked, whatever the answer.
    let dir = std::env::temp_dir().join(format!("vp-noremote-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();

    let db = dir.join("d.db");
    let state = boot_with_db(&db, "t".into(), Policy::default()).await;

    let ev = |n: u32| {
        format!(
            r#"{{"hook_event_name":"UserPromptSubmit","session_id":"s{n}",
                 "cwd":"{}","prompt":"x"}}"#,
            dir.display()
        )
    };
    for n in 0..5 {
        hook(&state, &ev(n)).await;
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

/// `doctor` runs the gate rather than reading a settings file, since no hook
/// can enforce its own presence.
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

    // A moved or uninstalled binary reads as installed and is not a gate.
    let moved: Map<String, Value> = serde_json::from_str(
        r#"{"hooks": {"PreToolUse": [{"hooks": [
            {"type":"command","command":"/nowhere/devplane hook","timeout":5}]}]}}"#,
    )
    .unwrap();
    let probe = devplane::observe::connect::probe_gate(&moved);
    assert!(!probe.answered);
    assert!(probe.error.is_some(), "and it says what went wrong");
}

/// Running the diagnostic writes no history: the probe's verdict is not recorded.
#[tokio::test]
async fn probing_the_gate_writes_nothing_down() {
    let probe_session = devplane::observe::hook::PROBE_SESSION;
    let home = gate_home("probe-noop");
    let rules = "[policy]\nnever_auto = [\"Read(.env)\"]\n";
    let probe = format!(
        r#"{{"hook_event_name":"PreToolUse","session_id":"{probe_session}","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{{"command":"cat .env"}}}}"#
    );

    // It still decides, through the real gate.
    let reply = gate(&home, rules, &probe);
    assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");

    // Neither the store nor the spool carries the probe.
    assert!(
        !home.join("pending-decisions.jsonl").exists(),
        "the diagnostic spooled a decision about a call nobody made"
    );
    let store = devplane::store::Store::open(&home.join("devplane.db"))
        .await
        .unwrap();
    assert!(
        store
            .decisions(Some(probe_session), 10)
            .await
            .unwrap()
            .is_empty(),
        "the diagnostic filed a decision about a call nobody made"
    );
    assert!(
        store
            .shim_events_since(0, 100)
            .await
            .unwrap()
            .iter()
            .all(|(_, e)| e.run_id.as_str() != probe_session),
        "the diagnostic left an observation of a session that never existed"
    );

    // The same call from a real session is recorded, so the absence above means something.
    let real = probe.replace(probe_session, "s-real");
    gate(&home, rules, &real);
    assert_eq!(store.decisions(Some("s-real"), 10).await.unwrap().len(), 1);
}

/// A gate that has stopped deciding raises a critical inbox item about the
/// machine itself, since the event log looks the same as a quiet machine.
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
    // Nothing is offered: the fix is reinstalling, and settings.json is not rewritten.
    assert!(item["actions"].as_array().unwrap().is_empty());

    // The CLI decodes it too: an item with no session sends `run_id: null`.
    let decoded: Vec<devplane::render::InboxItem> =
        serde_json::from_value(inbox.clone()).expect("the CLI decodes what the API serves");
    assert!(decoded.iter().any(|i| i.kind == "gate_down"));

    // A change whose runs have all ended.
    let work_shaped = serde_json::json!([{
        "kind": "ci_red", "level": "high", "run_id": null,
        "title": "CI is red", "change_id": "w-1", "actions": ["open_pr"]
    }]);
    let decoded: Vec<devplane::render::InboxItem> =
        serde_json::from_value(work_shaped).expect("a change with no run must decode too");
    assert!(decoded[0].run_id.is_none());
}

#[tokio::test]
async fn the_gates_own_probe_never_reaches_the_board_from_either_receiver() {
    // A probe call handed to the recorders must be dropped by both, not appear
    // as a `devplane-probe-<pid>` project.
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
    let probe = devplane::observe::hook::PROBE_SESSION;

    hook(
        &state,
        &format!(
            r#"{{"hook_event_name":"PreToolUse","session_id":"{probe}","cwd":"/tmp/devplane-probe-1",
                "tool_name":"Bash","tool_input":{{"command":"cat .devplane-probe"}}}}"#
        ),
    )
    .await;
    decided(
        &state,
        serde_json::json!({
            "session": probe, "verdict": "deny", "rule": "Read(.devplane-probe)",
            "blocked": true, "subject": "Bash: cat .devplane-probe", "tool": "Bash",
            "late": true,
            "payload": {"hook_event_name":"PreToolUse","session_id":probe,
                        "cwd":"/tmp/devplane-probe-1","tool_name":"Bash",
                        "tool_input":{"command":"cat .devplane-probe"}},
        }),
    )
    .await;

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
async fn the_health_probe_names_the_version_so_a_client_can_refuse_a_stale_host() {
    // Every command refuses a host of another version.
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

/// The forge endpoint: one route, both halves, what needs the person first,
/// including its bearer check and sort.
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
                host: Some("github.com".into()),
                fetched_at: jiff::Timestamp::now(),
                issues_total: 2,
                pull_requests_total: 2,
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
                // A draft of your own with red checks sorts below a requested review.
                pull_requests: vec![
                    pr(10, true, false, "failing", true),
                    pr(11, false, true, "ready_for_review", false),
                ],
                error: None,
            },
        );
    }

    // The bearer check, and a `?token=` does not count.
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
    // A forge entry whose project is gone still renders as a row.
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
            Some("GitHub unreachable: could not connect".into());
    }
    let v = get_json(&c, &addr, "/api/forge", &token).await;
    let stale = v["stale"].as_array().expect("stale");
    assert_eq!(stale.len(), 1, "the view names what it could not read");
    assert_eq!(stale[0]["error"], "GitHub unreachable: could not connect");
    assert!(stale[0]["last_good"].is_string(), "and when it last could");
    assert_eq!(
        v["issues"].as_array().unwrap().len(),
        2,
        "a failed poll does not empty the board"
    );
    let board = get_json(&c, &addr, "/api/board", &token).await;
    assert_eq!(
        board["forge"]["p1"]["stale"], "GitHub unreachable: could not connect",
        "and the heading's counts carry why they might be wrong"
    );
    assert_eq!(
        (
            board["summary"]["open_issues"].as_u64(),
            board["summary"]["forge_stale"].as_u64()
        ),
        (Some(0), Some(1)),
        "last-good numbers are said as stale, not summed as current"
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

/// A project ruled out of the forge is re-checked, and says why: the ruling
/// is a reading of the git remote, and a remote can be added later.
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

    // An hour-old ruling is spent; a fresh one holds. Calls the poller's own predicate.
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

/// The machine's configuration in one request, including a `devplane.toml`
/// that will not parse and so takes its prohibitions with it.
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

    // The gates and rules read back, in evaluation order.
    let ok = find("good");
    assert_eq!(ok["config"]["exists"], true);
    assert_eq!(ok["declares_gates"], true);
    assert_eq!(ok["describes"]["gates"]["check"][0], "cargo test");
    assert_eq!(
        ok["describes"]["policy"]["deny"][0]["rule"],
        "Bash(git push:*)"
    );

    // The unparseable one carries the parser's reason, not a boolean.
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

/// The offer: a rule covering this call's family once three distinct calls are
/// seen; below that, the exact call.
#[tokio::test]
async fn a_repeated_permission_is_offered_the_rule_that_answers_its_family() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    // Calls already seen in the same directory; each would interrupt.
    for cmd in ["cargo test --lib diff", "cargo test --doc", "cargo test -q"] {
        hook(
            &state,
            &format!(
                r#"{{"hook_event_name":"PreToolUse","session_id":"s-hist","cwd":"/tmp/repo",
                    "tool_name":"Bash","tool_input":{{"command":"{cmd}"}}}}"#
            ),
        )
        .await;
    }

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s-off","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"cargo test --lib policy"}}"#;
    decided(
        &state,
        serde_json::json!({
            "session": "s-off", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: cargo test --lib policy", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        }),
    )
    .await;

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

    // A grant goes where it is enforced, the agent's own settings; `/tmp/repo` is
    // in no registered project, so that is the user-scope file.
    let file = offer["file"].as_str().unwrap();
    assert!(file.ends_with("settings.json"), "{file}");
    assert!(!file.contains("devplane.toml"), "{file}");
    assert_eq!(offer["section"], "permissions.allow");
}

/// One interruption is not evidence about the shape of the ones like it.
#[tokio::test]
async fn a_permission_seen_once_is_offered_only_its_own_call() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s-one","cwd":"/tmp/solo",
            "tool_name":"Bash","tool_input":{"command":"pnpm test --run"}}"#;
    decided(
        &state,
        serde_json::json!({
            "session": "s-one", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: pnpm test --run", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        }),
    )
    .await;

    let inbox = inbox_items(&c, &addr, &token).await;
    let offer = &inbox[0]["offer"];
    assert_eq!(offer["basis"], "call", "{}", inbox[0]);
    assert_eq!(offer["rule"], "Bash(pnpm test --run)");
    assert_eq!(offer["covers"], 1);
}

/// A call no rule can cover says why, in words, rather than leaving a blank.
#[tokio::test]
async fn a_permission_no_rule_can_cover_says_why() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    let payload = r#"{"hook_event_name":"PermissionRequest","session_id":"s-cmp","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"pnpm build && rm -rf dist"}}"#;
    decided(
        &state,
        serde_json::json!({
            "session": "s-cmp", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: pnpm build && rm -rf dist", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        }),
    )
    .await;

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

/// Records what the offer costs, and whether it grows with permission items or
/// with the event log. Recorded, not enforced.
#[tokio::test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
async fn sc007_the_offer_costs_one_query_per_permission_item() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    // A log far larger than the evidence any one offer reads.
    for n in 0..400 {
        hook(
            &state,
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
    decided(
        &state,
        serde_json::json!({
            "session": "s-cost", "verdict": "undecided", "rule": null, "blocked": true,
            "subject": "Bash: cargo test --lib policy", "tool": "Bash",
            "payload": serde_json::from_str::<Value>(payload).unwrap(),
        }),
    )
    .await;

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

/// Every edge case of the offer sentence, through the host: each input produces
/// its own sentence over the wire.
#[tokio::test]
async fn every_way_a_rule_cannot_be_offered_reads_differently() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    let raise = |session: &'static str, tool: &'static str, input: Value| {
        let state = state.clone();
        async move {
            let payload = serde_json::json!({
                "hook_event_name": "PermissionRequest", "session_id": session,
                "cwd": "/tmp/walk", "tool_name": tool, "tool_input": input,
            });
            decided(
                &state,
                serde_json::json!({
                    "session": session, "verdict": "undecided", "rule": null,
                    "blocked": true, "subject": format!("{tool}: walk"),
                    "tool": tool, "payload": payload,
                }),
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

    // A permission raised by a notification with no tool input.
    hook(
        &state,
        r#"{"hook_event_name":"Notification","session_id":"w-dialog","cwd":"/tmp/walk",
            "notification_type":"permission_prompt","message":"Allow network access?"}"#,
    )
    .await;
    let again = inbox_items(&c, &addr, &token).await;
    let dialog = again
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["run_id"] == "w-dialog")
        .unwrap_or_else(|| panic!("the dialog item reached the inbox: {again}"));
    assert!(dialog["offer"].is_null(), "{dialog}");
    assert_eq!(dialog["no_offer"]["reason"], "unknown_call", "{dialog}");

    // A watched-only session has no allow or deny, so a rule is the only remedy.
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

/// A figure about this machine counts live sessions only; a finished run's
/// version is history, not a claim about now.
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

    // One ends and becomes history.
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

    // There is no ahead-of-baseline key at all, not even an empty one.
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

/// A durable ask survives a host restart: it is persisted outside the asking
/// process, resumed by an opaque token, recorded before delivery, and the answer
/// is delivered into a resumed session, which the row names.
#[tokio::test]
async fn a_question_outlives_the_host_that_was_holding_it() {
    let _serial = common::one_agent_at_a_time();
    let Some(agent) = echo_agent_path() else {
        eprintln!("skipping: build the fixture with `cargo build -p devplane-acp --examples`");
        return;
    };
    // One database, two hosts — which is the whole test.
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
        let spec = devplane::acp::resolve(&agent, &state.agents).expect("the fixture resolves");
        let run = devplane::driven::dispatch(&state, &spec, cwd.clone().into(), None, Vec::new())
            .await
            .expect("a run id")
            .to_string();
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

        // A graceful stop leaves the question answerable; an agent whose turn ended does not.
        state.shutdown().await;
        ask
    };

    // A second host on the same store.
    let (addr, token, c, _state) = boot_shared(Policy::default(), &db).await;

    let asks = get_json(&c, &addr, "/api/asks", &token).await;
    let open = asks["open"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == ask_id.as_str())
        .expect("the ask survived the host that was holding it");
    assert_eq!(open["open"], true);
    assert_eq!(open["outcome"], "waiting for you");

    // And it is back in the inbox.
    let inbox = inbox_items(&c, &addr, &token).await;
    assert!(
        inbox
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["ask"] == ask_id.as_str()),
        "a question nobody answered is still in the inbox after a restart"
    );

    // Still answerable: the answer is delivered into the resumed session, and the
    // row says which delivery happened.
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

/// A question waiting from an earlier session counts in the summary header, not
/// only in the inbox.
#[tokio::test]
async fn an_ask_that_outlived_its_session_is_counted_in_the_summary() {
    let db = std::env::temp_dir().join(format!(
        "vp-summary-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let (addr, token, c, state) = boot_shared(Policy::default(), &db).await;

    // An unanswered ask on a run with no live session, as a restart leaves.
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

    // The header and the list agree.
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

/// An empty inbox says what the day came to.
#[tokio::test]
async fn an_empty_inbox_says_what_the_day_came_to() {
    let (addr, token, c) = boot(Policy::default()).await;

    let body = get_json(&c, &addr, "/api/inbox", &token).await;
    assert_eq!(
        body["items"].as_array().map(Vec::len),
        Some(0),
        "a fresh host has nothing in its inbox"
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

/// Polling the inbox does not advance the "since last read" boundary.
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
    // The hairline stays absent: under a minute is not worth drawing.
    assert!(
        after["close"]["since_last_look"].is_null(),
        "a twelve-second gap is not a boundary: {}",
        after["close"]
    );
}

/// Decisions taken on somebody's behalf and actions the tool took are separate counts.
#[tokio::test]
async fn the_tally_counts_what_was_decided_for_you_apart_from_what_the_tool_did() {
    let (addr, token, c, state) = boot_with_state(Policy::rules(&["Bash(rm *)".into()], &[])).await;

    // A rule refuses a call: a decision taken on somebody's behalf.
    hook(
        &state,
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
    decided(
        &state,
        serde_json::json!({
            "session": "s-close",
            "verdict": "deny",
            "rule": "Bash(rm *)",
            "subject": "rm -rf /tmp/x",
            "tool": "Bash",
        }),
    )
    .await;

    let body = get_json(&c, &addr, "/api/inbox", &token).await;
    let close = &body["close"];
    // The inbox is not empty, so only the boundary is served.
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

/// `doctor` reports what is watched: driven agents versus those only readable
/// through vendor-published channels.
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

    // Every drivable vendor is named.
    for vendor in devplane::core::vendors::vendors() {
        assert!(
            text.contains(vendor),
            "`doctor` does not mention `{vendor}`, which this machine can drive"
        );
    }

    // The three reaches are distinguishable in the output; `unproved` is neither
    // working nor absent.
    for word in ["read", "unproved", "not published"] {
        assert!(
            text.contains(word),
            "`doctor` never prints `{word}`, so the three states cannot be told apart on screen"
        );
    }

    // Each row carries its reason.
    assert!(
        text.contains("no session roster"),
        "`doctor` reports a channel as absent without saying what that costs"
    );

    // The table is a claim about other products, so it carries its checked date.
    assert!(
        text.contains(devplane::core::vendors::CHECKED),
        "`doctor` prints vendor facts with no date, so nobody can tell how stale they are"
    );
}

/// Both empty states of `devplane ls` (Claude Code idle, or not installed) name
/// the vendors it cannot see. Driven via env so neither branch depends on the
/// developer's machine.
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

    // `claude_binary()` tries `DEVPLANE_CLAUDE_BIN`, then `PATH`; both are controlled here.
    let cases: [(&str, &str, &str); 2] = [
        (
            "installed and quiet",
            "/bin/echo",
            "No agent sessions are running",
        ),
        (
            "not installed at all",
            "/nonexistent/claude",
            "No `claude` binary was found",
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
            // Emptied so the lookup cannot find a local `claude`; `HOME` too, since it
            // also checks the native installer and VS Code extension paths.
            .env("PATH", "/nonexistent")
            .env("HOME", &home)
            .output()
            .expect("the binary runs");

        // `ls` must not start a host: with none answering it reads the store. A host
        // here would leave a `host.json`.
        assert!(
            !home.join("host.json").exists(),
            "`{what}`: `ls` started a host, which nothing does by asking any more"
        );

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
        assert!(
            !text.contains("using /"),
            "`{what}`: the empty list prints which binary it found, which says nothing \
             about why the list is empty:\n{text}"
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

/// The telemetry endpoints require the bearer: otherwise any local process or
/// page could forge cost, context and session records.
#[tokio::test]
async fn telemetry_without_the_token_is_refused_on_every_signal() {
    let (addr, token, c) = boot(Policy::default()).await;
    for signal in ["logs", "traces"] {
        let url = format!("http://{addr}/devplane/otel/v1/{signal}");
        let anonymous = c
            .post(&url)
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(
            anonymous.status(),
            reqwest::StatusCode::UNAUTHORIZED,
            "/devplane/otel/v1/{signal} accepted a record from nobody"
        );

        let wrong = c
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", "Bearer not-the-token")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(
            wrong.status(),
            reqwest::StatusCode::UNAUTHORIZED,
            "/devplane/otel/v1/{signal} accepted a wrong bearer"
        );

        let ours = c
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body("{}")
            .send()
            .await
            .unwrap();
        assert!(
            ours.status().is_success(),
            "/devplane/otel/v1/{signal} refused the real exporter"
        );
    }
}

/// Board and terminal narrow identically because the host narrows once; driven
/// through the API since the claim is about both callers.
#[tokio::test]
async fn both_surfaces_narrow_from_one_computation() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    // Two projects, each with a session that asked a question and stopped.
    for (n, dir) in [("alpha", "/tmp/alpha"), ("beta", "/tmp/beta")] {
        hook(
            &state,
            &format!(
                r#"{{"hook_event_name":"PreToolUse","session_id":"s-{n}","cwd":"{dir}",
                    "tool_name":"AskUserQuestion",
                    "tool_input":{{"questions":[{{"question":"Keep it in {n}?",
                      "options":[{{"label":"Keep"}},{{"label":"Remove"}}]}}]}}}}"#
            ),
        )
        .await;
        hook(
            &state,
            &format!(r#"{{"hook_event_name":"Stop","session_id":"s-{n}","cwd":"{dir}"}}"#),
        )
        .await;
    }

    let whole = get_json(&c, &addr, "/api/inbox", &token).await;
    let all = whole["items"].as_array().map(Vec::len).unwrap_or(0);
    assert!(all >= 2, "expected both sessions to be waiting, got {all}");

    // The URL the board fetches for `#inbox/<project>` and the CLI for `--project`;
    // a discovered project is named after the last segment of its root.
    let narrowed = get_json(&c, &addr, "/api/inbox?project=alpha", &token).await;
    let listed = narrowed["items"].as_array().map(Vec::len).unwrap_or(0);
    let left_out = narrowed["narrowed"]["count"].as_u64().unwrap_or(0) as usize;

    assert!(listed < all, "narrowing to one project showed everything");
    assert_eq!(
        listed + left_out,
        all,
        "listed + narrowed away must equal what was raised — a row that is in \
         neither has vanished, and this list is sold on being complete"
    );
    assert_eq!(
        narrowed["narrowed"]["no_such_project"], false,
        "a project that exists was reported as a typo"
    );

    // The close does not render over a narrowed list: a day is not one project.
    assert!(
        narrowed["close"].is_null(),
        "the close was composed for a narrowed list"
    );
    assert!(
        !whole["close"].is_null(),
        "the unnarrowed list lost its close"
    );

    // A name nobody has is its own answer, not an empty project.
    let typo = get_json(&c, &addr, "/api/inbox?project=zzzz", &token).await;
    assert_eq!(typo["narrowed"]["no_such_project"], true);
    assert_eq!(typo["items"].as_array().map(Vec::len).unwrap_or(9), 0);
}

/// Narrowing is a view: it changes nothing in the `devplane attention` record.
#[tokio::test]
async fn a_narrowing_does_not_change_what_was_raised() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;
    hook(
        &state,
        r#"{"hook_event_name":"PreToolUse","session_id":"s-a","cwd":"/tmp/alpha",
            "tool_name":"AskUserQuestion",
            "tool_input":{"questions":[{"question":"Keep it?",
              "options":[{"label":"Keep"},{"label":"Remove"}]}]}}"#,
    )
    .await;

    let _ = get_json(&c, &addr, "/api/inbox?read=true", &token).await;

    // The record is written by the host's loop, so this checks structurally that
    // the narrowing filter cannot reach this route.
    let plain = get_json(&c, &addr, "/api/attention?days=7", &token).await;
    let with_filter = get_json(
        &c,
        &addr,
        "/api/attention?days=7&project=alpha&needs_you=true",
        &token,
    )
    .await;

    // Everything but `since`, which moves with the clock.
    for field in ["kinds", "oversight", "agents", "days"] {
        assert_eq!(
            plain[field], with_filter[field],
            "a narrowing changed `{field}` in the attention record, which is about \
             what was raised rather than about what somebody chose to look at"
        );
    }

    // And narrowing the inbox does not write to it either.
    let before = get_json(&c, &addr, "/api/attention?days=7", &token).await;
    let _ = get_json(&c, &addr, "/api/inbox?project=nothing-like-this", &token).await;
    let _ = get_json(&c, &addr, "/api/inbox?needs_you=true", &token).await;
    let after = get_json(&c, &addr, "/api/attention?days=7", &token).await;
    for field in ["kinds", "oversight", "agents", "days"] {
        assert_eq!(
            before[field], after[field],
            "reading a narrowed inbox moved `{field}`"
        );
    }
}

/// A hold nobody answers behaves as with no hold: the timed-out hook is
/// discarded and Claude Code shows its own dialog, a few seconds later.
#[tokio::test]
async fn a_hold_nobody_answers_lapses_and_decides_nothing() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    let before = get_json(&c, &addr, "/api/decisions", &token).await;
    let before = before.as_array().map(Vec::len).unwrap_or(0);

    let started = std::time::Instant::now();
    let said = hold(
        &state,
        "s-hold",
        "git push origin main",
        Duration::from_millis(400),
    )
    .await
    .expect("the hold returns");
    let took = started.elapsed();

    assert!(
        said.is_none(),
        "an unanswered hold produced a decision: {said:?}"
    );
    assert!(
        took >= Duration::from_millis(300),
        "the hold returned in {took:?} without waiting for anybody"
    );

    // Nothing was decided, so nothing is recorded; the ask closes as nobody's.
    let after = get_json(&c, &addr, "/api/decisions", &token).await;
    assert_eq!(
        after.as_array().map(Vec::len).unwrap_or(0),
        before,
        "a lapsed hold wrote to the decision log"
    );
    let asks = get_json(&c, &addr, "/api/asks", &token).await;
    assert!(
        asks["open"].as_array().is_some_and(Vec::is_empty),
        "a lapsed hold is still open: {}",
        asks["open"]
    );
    let lapsed = asks["settled"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["run"] == "s-hold"))
        .expect("the lapsed hold is on the record");
    assert!(
        lapsed["ended"]["nobody"].is_object(),
        "a lapse ends as nobody's: {lapsed}"
    );
}

/// A person's selection is carried and recorded as theirs; the policy engine
/// never sees it.
#[tokio::test]
async fn a_person_can_answer_a_watched_permission_from_anywhere() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    // The hook holds. Answered from another surface while it waits.
    let held = hold(
        &state,
        "s-ans",
        "git push origin main",
        Duration::from_millis(8000),
    );

    // Find the ask the hold raised, the way a surface would.
    let mut id = String::new();
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let asks = get_json(&c, &addr, "/api/asks", &token).await;
        if let Some(a) = asks["open"].as_array().and_then(|a| a.first())
            && let Some(s) = a["id"].as_str()
        {
            id = s.to_string();
            break;
        }
    }
    assert!(
        !id.is_empty(),
        "the hold raised nothing a surface could answer"
    );

    let answered = post(
        &c,
        &addr,
        &format!("/api/asks/{id}/answer"),
        &token,
        r#"{"option":"allow"}"#,
    )
    .await;
    let answered: Value = serde_json::from_str(&answered).expect("the answer is JSON");
    assert_eq!(
        answered["open"], false,
        "answering the held permission failed: {answered}"
    );

    let said = held.await.expect("the hold returns");
    assert_eq!(
        said.as_deref(),
        Some("allow"),
        "the person's selection was not carried"
    );

    // And it is recorded as theirs: the ledger names a person, not a rule.
    let log = get_json(&c, &addr, "/api/decisions?about=s-ans", &token).await;
    assert!(
        log.as_array()
            .unwrap()
            .iter()
            .any(|d| d["authority"] == "person"),
        "the ledger does not name the person who answered: {log}"
    );
}

/// Only a recorded human selection can produce an `allow` here.
#[tokio::test]
async fn no_rule_clock_or_default_can_allow_through_a_hold() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    for answer in [
        r#"{"option":"maybe"}"#,
        r#"{"option":""}"#,
        r#"{"text":"allow"}"#,
    ] {
        let held = hold(&state, "s-x", "rm x", Duration::from_millis(700));
        tokio::time::sleep(Duration::from_millis(120)).await;
        let asks = get_json(&c, &addr, "/api/asks", &token).await;
        if let Some(a) = asks["open"].as_array().and_then(|a| a.first())
            && let Some(id) = a["id"].as_str()
        {
            let _ = post(&c, &addr, &format!("/api/asks/{id}/answer"), &token, answer).await;
        }
        let said = held.await.expect("the hold returns");
        assert!(
            said.is_none(),
            "`{answer}` produced a behavior this product cannot stand behind: {said:?}"
        );
    }
}

/// An answer must name an offered option: an unknown id is refused before
/// anything is written, rather than recorded as an `allow` no person gave.
#[tokio::test]
async fn an_option_nobody_offered_is_refused_rather_than_recorded_as_an_allow() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    let held = hold(&state, "s-opt", "rm -rf x", Duration::from_millis(1500));

    tokio::time::sleep(Duration::from_millis(200)).await;
    let asks = get_json(&c, &addr, "/api/asks", &token).await;
    let id = asks["open"][0]["id"]
        .as_str()
        .expect("the hold raised an ask")
        .to_string();

    let said = post(
        &c,
        &addr,
        &format!("/api/asks/{id}/answer"),
        &token,
        r#"{"option":"maybe"}"#,
    )
    .await;
    assert!(
        said.contains("not one of the answers"),
        "an option nobody offered was accepted: {said}"
    );
    // And it names what *was* offered, so the refusal is actionable.
    assert!(said.contains("allow") && said.contains("deny"), "{said}");

    // The hold lapses, because nothing was answered.
    let said = held.await.unwrap();
    assert!(said.is_none(), "a refused option still decided: {said:?}");

    // And an option that *was* offered still works, or this is a mute button.
    let held = hold(&state, "s-opt2", "rm -rf x", Duration::from_millis(4000));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let asks = get_json(&c, &addr, "/api/asks", &token).await;
    let id = asks["open"][0]["id"]
        .as_str()
        .expect("a second ask")
        .to_string();
    let _ = post(
        &c,
        &addr,
        &format!("/api/asks/{id}/answer"),
        &token,
        r#"{"option":"deny"}"#,
    )
    .await;
    assert_eq!(
        held.await.unwrap().as_deref(),
        Some("deny"),
        "an offered option stopped working"
    );
}

/// A held permission says an agent is waiting and does not offer `focus`.
#[tokio::test]
async fn a_held_permission_says_somebody_is_waiting_and_offers_no_editor() {
    let (addr, token, c, state) = boot_with_state(Policy::default()).await;

    let held = hold(&state, "s-wait", "git push", Duration::from_millis(3000));

    tokio::time::sleep(Duration::from_millis(400)).await;
    let inbox = get_json(&c, &addr, "/api/inbox", &token).await;
    let row = inbox["items"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["kind"] == "permission"))
        .expect("the hold reached the inbox");

    let detail = row["detail"].as_str().unwrap_or("");
    assert!(
        detail.contains("waiting for you right now"),
        "a held permission does not say somebody is waiting: {detail}"
    );
    assert!(
        detail.contains("left"),
        "a held permission does not say how long remains: {detail}"
    );
    assert!(
        !detail.contains("no longer running"),
        "a held permission claims its agent is gone while it sits there waiting"
    );

    let actions: Vec<&str> = row["actions"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    assert!(
        !actions.contains(&"focus"),
        "answering here offers to take you to the editor, which is the thing a \
         hold exists to avoid: {actions:?}"
    );
    assert!(
        actions.contains(&"choose") || actions.contains(&"reply"),
        "a held permission cannot be answered: {actions:?}"
    );

    // Once it lapses, only the vendor's dialog can answer, so `focus` is offered.
    let _ = held.await;
    let inbox = get_json(&c, &addr, "/api/inbox", &token).await;
    if let Some(row) = inbox["items"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["kind"] == "permission"))
    {
        let detail = row["detail"].as_str().unwrap_or("");
        assert!(
            detail.contains("hold ran out"),
            "a lapsed hold does not say so: {detail}"
        );
    }
}

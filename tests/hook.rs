//! The store is the contract between processes: a hook decides in its own process
//! and writes to the store, with nothing else running. These tests run the real
//! binary against a throwaway home and read the store as a host would.

use devplane::store::Store;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn home(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dp-hook-{}-{name}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs `devplane hook` as Claude Code does: payload on stdin, reply on stdout,
/// a private `~/.devplane`, nothing listening.
fn hook(home: &Path, rules: &str, payload: &str) -> Value {
    std::fs::write(home.join("policy.toml"), rules).unwrap();
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
        .expect("the hook runs");
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("stdout was {text:?}: {e}"))
}

/// A command with agent-session markers removed, like a person's own terminal.
fn outside_any_session(cmd: &mut std::process::Command) -> &mut std::process::Command {
    for v in devplane::hook::AGENT_SESSION_VARS {
        cmd.env_remove(v);
    }
    cmd
}

fn hook_in(home: &Path, cwd: &Path, args: &[&str], payload: &str) -> Value {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("hook")
        .args(args)
        .current_dir(cwd)
        .env("DEVPLANE_HOME", home)
        .env("DEVPLANE_NOTIFY", "0")
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
        .expect("the hook runs");
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("stdout was {text:?}: {e}"))
}

async fn store(home: &Path) -> Store {
    Store::open(&home.join("devplane.db")).await.unwrap()
}

/// A gate refusal lands in the store's ledger with no other process alive.
#[tokio::test]
async fn a_prohibition_is_enforced_and_recorded_with_nothing_running() {
    let home = home("deny");
    let reply = hook(
        &home,
        "[policy]\nnever_auto = [\"Read(.env)\"]\n",
        r#"{"hook_event_name":"PreToolUse","session_id":"s-deny","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"cat .env"}}"#,
    );
    assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        !home.join("pending-decisions.jsonl").exists(),
        "a store that opens is written, not spooled"
    );

    let s = store(&home).await;
    let decisions = s.decisions(Some("s-deny"), 10).await.unwrap();
    assert_eq!(decisions.len(), 1, "{decisions:?}");
    assert_eq!(decisions[0].authority.as_str(), "rule");
    assert_eq!(decisions[0].outcome, "deny");
    assert!(
        decisions[0]
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("Read(.env)")
    );
    assert!(
        !decisions[0]
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("filed late"),
        "written at the time, not filed afterwards"
    );
    // The observation beside it, waiting for a host to fold in.
    let pending = s.shim_events_since(0, 100).await.unwrap();
    assert!(
        pending.iter().any(|(_, e)| e.run_id.as_str() == "s-deny"),
        "the tool call itself is on the record: {pending:?}"
    );
    assert_eq!(
        s.projected_through().await.unwrap(),
        0,
        "the hook never projects"
    );
}

/// An observation is written like a decision, so a host that was closed all
/// session still learns what happened.
#[tokio::test]
async fn an_observation_is_kept_for_a_host_that_was_not_there() {
    let home = home("observe");
    let quiet = hook(
        &home,
        "",
        r#"{"hook_event_name":"SessionStart","session_id":"s-obs","cwd":"/tmp/repo","source":"startup"}"#,
    );
    assert_eq!(
        quiet,
        serde_json::json!({}),
        "an observation answers nothing"
    );
    let s = store(&home).await;
    let rows = s.shim_events_since(0, 100).await.unwrap();
    assert!(
        rows.iter()
            .any(|(_, e)| matches!(e.event, devplane::core::event::Event::SessionStarted { .. })),
        "{rows:?}"
    );
    // A read with no host replays it in memory.
    let local = {
        let _guard = EnvHome::set(&home);
        devplane::local::Local::open().await.unwrap()
    };
    let snap = local.snapshot().await.unwrap();
    assert!(
        snap.world
            .run(&devplane::core::RunId::new("s-obs"))
            .is_some(),
        "a host-less read folds the rows in for itself"
    );
    assert!(!snap.from_host);
}

/// A held permission is answered from another process with no host: the hook
/// polls the row `devplane answer` writes and returns the person's choice.
#[tokio::test]
async fn a_held_permission_is_answered_by_another_process_with_no_host() {
    let home = home("hold");
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("devplane.toml"),
        "[policy]\nalways_ask = [\"Bash(git push *)\"]\n[questions]\nhold = \"20s\"\n",
    )
    .unwrap();
    let payload = format!(
        r#"{{"hook_event_name":"PermissionRequest","session_id":"s-hold","cwd":"{}",
            "tool_name":"Bash","tool_input":{{"command":"git push origin main"}}}}"#,
        repo.display()
    );

    // The person, in another terminal, a moment later.
    let answering = {
        let home = home.clone();
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the ask never appeared"
                );
                std::thread::sleep(std::time::Duration::from_millis(100));
                let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
                    .args(["inbox", "--all", "--json"])
                    .env("DEVPLANE_HOME", &home)
                    .env("DEVPLANE_NOTIFY", "0")
                    .output()
                    .unwrap();
                let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
                let Some(id) = v["open"][0]["id"].as_str() else {
                    continue;
                };
                // The person's own terminal: no agent session around it.
                let out = outside_any_session(
                    std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
                        .args(["answer", id, "--allow"])
                        .env("DEVPLANE_HOME", &home),
                )
                .output()
                .unwrap();
                assert!(
                    out.status.success(),
                    "answering failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                return;
            }
        })
    };

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("hook")
        .env("DEVPLANE_HOME", &home)
        .env("DEVPLANE_NOTIFY", "0")
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
        .expect("the hook runs");
    answering.join().unwrap();
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        reply["hookSpecificOutput"]["decision"]["behavior"], "allow",
        "the person's answer was carried: {reply}"
    );

    let s = store(&home).await;
    let asks = s.asks(10).await.unwrap();
    let ask = asks
        .iter()
        .find(|a| a.run.as_str() == "s-hold")
        .expect("the ask row");
    assert!(!ask.is_open());
    assert!(matches!(
        ask.ended,
        Some(devplane::core::ask::Ended::Person)
    ));
    let d = s.decisions(Some("s-hold"), 10).await.unwrap();
    assert!(
        d.iter().any(|d| d.authority.as_str() == "person"),
        "the ledger names the person: {d:?}"
    );
}

/// An unanswered hold lapses: the vendor's dialog takes over, the row closes as
/// `nobody`, and no decision is written.
#[tokio::test]
async fn a_hold_nobody_answers_lapses_into_the_vendors_dialog() {
    let home = home("lapse");
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("devplane.toml"),
        "[policy]\nalways_ask = [\"Bash(git push *)\"]\n[questions]\nhold = \"1s\"\n",
    )
    .unwrap();
    let payload = format!(
        r#"{{"hook_event_name":"PermissionRequest","session_id":"s-lapse","cwd":"{}",
            "tool_name":"Bash","tool_input":{{"command":"git push origin main"}}}}"#,
        repo.display()
    );
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .arg("hook")
        .env("DEVPLANE_HOME", &home)
        .env("DEVPLANE_NOTIFY", "0")
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
        .expect("the hook runs");
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        reply,
        serde_json::json!({}),
        "lapsed: the vendor asks, {reply}"
    );
    let s = store(&home).await;
    let ask = s
        .asks(10)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.run.as_str() == "s-lapse")
        .expect("the ask row");
    assert!(matches!(
        ask.ended,
        Some(devplane::core::ask::Ended::Nobody { .. })
    ));
    let d = s.decisions(Some("s-lapse"), 10).await.unwrap();
    assert!(
        d.iter().all(|d| d.authority.as_str() != "person"),
        "nobody decided anything: {d:?}"
    );
}

/// Sets `DEVPLANE_HOME` for this process while alive, so an in-process `Local`
/// opens the store the binary wrote.
struct EnvHome(Option<std::ffi::OsString>);
impl EnvHome {
    fn set(home: &Path) -> Self {
        let before = std::env::var_os("DEVPLANE_HOME");
        // SAFETY: tests touching the variable do so under this guard; nothing reads
        // the environment concurrently.
        unsafe { std::env::set_var("DEVPLANE_HOME", home) };
        Self(before)
    }
}
impl Drop for EnvHome {
    fn drop(&mut self) {
        // SAFETY: see `set`.
        unsafe {
            match &self.0 {
                Some(v) => std::env::set_var("DEVPLANE_HOME", v),
                None => std::env::remove_var("DEVPLANE_HOME"),
            }
        }
    }
}

/// A planted typo beside `never_auto` makes every call `Unresolved`: asked,
/// naming the file, and recorded under `rule`, not as a tool decision.
#[tokio::test]
async fn a_devplane_toml_with_a_typo_asks_about_every_call() {
    let home = home("typo");
    std::fs::write(home.join("policy.toml"), "").unwrap();
    let repo = home.join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(
        repo.join("devplane.toml"),
        "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\nstall_timout = \"5m\"\n",
    )
    .unwrap();
    for (tool, input) in [
        ("Bash", r#"{"command":"rm -rf /"}"#),
        ("Bash", r#"{"command":"ls"}"#),
        ("Read", r#"{"file_path":"README.md"}"#),
    ] {
        let reply = hook_in(
            &home,
            &home,
            &[],
            &format!(
                r#"{{"hook_event_name":"PreToolUse","session_id":"s-typo","cwd":"{}",
                    "tool_name":"{tool}","tool_input":{input}}}"#,
                repo.display()
            ),
        );
        let out = &reply["hookSpecificOutput"];
        assert_eq!(out["permissionDecision"], "ask", "{tool} {input}: {reply}");
        let reason = out["permissionDecisionReason"].as_str().unwrap();
        assert!(
            reason.contains("devplane.toml"),
            "the reason names the file: {reason}"
        );
    }
    let s = store(&home).await;
    let d = s.decisions(Some("s-typo"), 10).await.unwrap();
    assert!(!d.is_empty());
    assert!(
        d.iter()
            .all(|d| d.outcome == "unresolved" && d.authority.as_str() == "rule"),
        "{d:?}"
    );
}

/// Codex parses `ask` and runs the call anyway, so on Codex every question
/// Devplane would put to a person is a refusal that says why.
#[test]
fn on_codex_a_question_is_a_refusal_never_a_pass() {
    let home = home("codex-ask");
    std::fs::write(
        home.join("policy.toml"),
        "[policy]\nalways_ask = [\"Bash(git push *)\"]\n",
    )
    .unwrap();
    let repo = home.join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let call = |command: &str| {
        hook_in(
            &home,
            &home,
            &["--vendor", "codex"],
            &format!(
                r#"{{"hook_event_name":"PreToolUse","session_id":"s-codex","cwd":"{}",
                    "tool_name":"Bash","tool_input":{{"command":"{command}"}}}}"#,
                repo.display()
            ),
        )
    };
    let reply = call("git push origin main");
    let out = &reply["hookSpecificOutput"];
    assert_eq!(out["permissionDecision"], "deny", "{reply}");
    assert!(
        out["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("Codex hooks cannot ask a person"),
        "{reply}"
    );
    // Nothing to decide stays silent.
    let reply = call("ls");
    assert_ne!(
        reply["hookSpecificOutput"]["permissionDecision"], "deny",
        "{reply}"
    );
}

/// The same for the machine-wide file.
#[test]
fn a_machine_wide_policy_that_will_not_load_asks_about_every_call() {
    let home = home("global-typo");
    std::fs::write(home.join("policy.toml"), "[policy\nnever_auto = [").unwrap();
    let reply = hook_in(
        &home,
        &home,
        &[],
        r#"{"hook_event_name":"PreToolUse","session_id":"s-gtypo","cwd":"/",
            "tool_name":"Bash","tool_input":{"command":"ls"}}"#,
    );
    assert_eq!(
        reply["hookSpecificOutput"]["permissionDecision"], "ask",
        "{reply}"
    );
    assert!(
        reply["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("policy.toml")
    );
    // Copilot's gate and Codex share the engine.
    let copilot = hook_in(
        &home,
        &home,
        &["--gate", "copilot"],
        r#"{"sessionId":"s-gtypo-c","cwd":"/","toolName":"bash","toolArgs":{"command":"ls"}}"#,
    );
    assert_eq!(copilot["permissionDecision"], "ask", "{copilot}");
}

/// A payload that will not parse, with rules in force, is a question.
#[test]
fn an_unreadable_payload_where_rules_are_in_force_is_asked_about() {
    let home = home("unreadable");
    std::fs::write(
        home.join("policy.toml"),
        "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n",
    )
    .unwrap();
    // `cwd` of the wrong type: the event is readable, the payload is not.
    let reply = hook_in(
        &home,
        &home,
        &[],
        r#"{"hook_event_name":"PreToolUse","session_id":"s-u","cwd":7,
            "tool_name":"Bash","tool_input":{"command":"rm -rf /"}}"#,
    );
    assert_eq!(
        reply["hookSpecificOutput"]["permissionDecision"], "ask",
        "{reply}"
    );
    // A `Bash` call with no command at all has nothing to match.
    let reply = hook_in(
        &home,
        &home,
        &[],
        r#"{"hook_event_name":"PreToolUse","session_id":"s-u2","cwd":"/",
            "tool_name":"Bash","tool_input":{"cmd":"rm -rf /"}}"#,
    );
    assert_eq!(
        reply["hookSpecificOutput"]["permissionDecision"], "ask",
        "{reply}"
    );
    // Copilot's arguments as a JSON string are read; as anything else, asked.
    let copilot = |args: &str| {
        hook_in(
            &home,
            &home,
            &["--gate", "copilot"],
            &format!(r#"{{"sessionId":"s-c","cwd":"/","toolName":"bash","toolArgs":{args}}}"#),
        )
    };
    assert_eq!(
        copilot(r#""{\"command\":\"rm -rf /\"}""#)["permissionDecision"],
        "deny"
    );
    assert_eq!(copilot(r#""not json""#)["permissionDecision"], "ask");
    assert_eq!(copilot("[1,2]")["permissionDecision"], "ask");
    // And with no rules at all, nothing is asked.
    std::fs::write(home.join("policy.toml"), "").unwrap();
    let quiet = hook_in(
        &home,
        &home,
        &[],
        r#"{"hook_event_name":"PreToolUse","session_id":"s-u3","cwd":7}"#,
    );
    assert_eq!(quiet, serde_json::json!({}));
}

/// Inside an agent's session, `--allow` is refused with a reason; `--deny` goes through.
#[tokio::test]
async fn an_agent_may_not_answer_its_own_permission() {
    for var in ["CLAUDECODE", "DEVPLANE_RUN", "CODEX_SANDBOX", "COPILOT_CLI"] {
        let home = home("self-answer");
        let s = store(&home).await;
        let id = raise_hold(&s, "s-self").await;
        let answer = |flag: &str| {
            outside_any_session(
                std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
                    .args(["answer", &id, flag])
                    .env("DEVPLANE_HOME", &home),
            )
            .env(var, "1")
            .output()
            .unwrap()
        };
        let out = answer("--allow");
        assert!(!out.status.success(), "{var}: an agent approved itself");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains(var) && err.contains("may not approve its own"),
            "the refusal says why: {err}"
        );
        assert!(
            s.ask(&id).await.unwrap().unwrap().is_open(),
            "nothing was recorded"
        );
        let out = answer("--deny");
        assert!(
            out.status.success(),
            "a refusal is not an approval: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Writes a held permission's row as the hook does, and returns its id.
async fn raise_hold(s: &Store, session: &str) -> String {
    let store = s.clone();
    let session = session.to_string();
    let hold = tokio::spawn(async move {
        devplane::record::hold(
            &store,
            &session,
            Path::new("/tmp/alpha"),
            "Bash",
            "git push",
            std::time::Duration::from_secs(20),
            false,
        )
        .await
    });
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        if let Some(a) = s.open_asks().await.unwrap().first() {
            // Leave the hold polling; the test's runtime ends it.
            drop(hold);
            return a.id.as_str().to_string();
        }
    }
    panic!("the hold raised nothing");
}

/// Two surfaces answering one hold at once: exactly one answer and one `person`
/// row are recorded, and the other surface is told.
#[tokio::test]
async fn two_surfaces_answering_one_hold_record_one_answer() {
    let home = home("race");
    let s = store(&home).await;
    let id = raise_hold(&s, "s-race").await;
    // Denials, so an agent-session environment does not refuse them before they race.
    let deny = || devplane::driven::Answer::Permission(devplane::driven::Decision::Deny);
    let (a, b) = tokio::join!(
        devplane::driven::answer_without_host(&s, &id, deny(), "window"),
        devplane::driven::answer_without_host(&s, &id, deny(), "cli"),
    );
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "exactly one answer lands: {a:?} {b:?}"
    );
    let loser = a.err().or(b.err()).unwrap().to_string();
    assert!(!loser.is_empty());
    let d = s.decisions(Some("s-race"), 10).await.unwrap();
    assert_eq!(
        d.iter()
            .filter(|d| d.authority.as_str() == "person")
            .count(),
        1,
        "{d:?}"
    );
}

/// A hold whose hook is gone ends as `nobody` past `held_until`, and a later
/// answer is refused rather than recorded as delivered.
#[tokio::test]
async fn a_hold_whose_hook_is_gone_ends_as_nobody() {
    let home = home("orphan");
    let s = store(&home).await;
    let asked = devplane::core::ask::Asked {
        kind: devplane::core::ask::Kind::Permission,
        request_id: String::new(),
        message: "Bash · git push".into(),
        payload: serde_json::json!({
            "tool": "Bash",
            "call": "git push",
            "held_until": (jiff::Timestamp::now() - jiff::SignedDuration::from_secs(60)).to_string(),
        }),
        at: jiff::Timestamp::now() - jiff::SignedDuration::from_secs(90),
        deadline: devplane::core::ask::Deadline::Never,
    };
    let id = devplane::core::AskId::new("orphan-1");
    let ask =
        devplane::core::ask::Ask::new(id.clone(), devplane::core::RunId::new("s-orphan"), asked);
    s.save_ask(&ask).await.unwrap();
    let deny = devplane::driven::Answer::Permission(devplane::driven::Decision::Deny);
    let refused = devplane::driven::answer_without_host(&s, id.as_str(), deny, "cli").await;
    assert!(
        refused.is_err(),
        "an answer to nobody was recorded as delivered"
    );
    let row = s.ask(id.as_str()).await.unwrap().unwrap();
    assert!(
        matches!(row.ended, Some(devplane::core::ask::Ended::Nobody { .. })),
        "{:?}",
        row.ended
    );
}

/// A permission's answer is said once. Two bypasses found by audit: a
/// `decision` beside an `option` let `--deny --option allow_always` allow, and
/// a `field` on a permission let an allowing option past the self-answer check.
#[test]
fn a_permission_answer_that_says_two_things_is_refused() {
    let asked = devplane::core::ask::Asked {
        kind: devplane::core::ask::Kind::Permission,
        request_id: String::new(),
        message: "Bash · git push".into(),
        payload: serde_json::json!({
            "tool": "Bash",
            "call": "git push",
            "options": [{"id": "allow_always"}, {"id": "reject_once"}],
        }),
        at: jiff::Timestamp::now(),
        deadline: devplane::core::ask::Deadline::Never,
    };
    let ask = devplane::core::ask::Ask::new(
        devplane::core::AskId::new("two-things"),
        devplane::core::RunId::new("s-two"),
        asked,
    );
    let parse = |decision: Option<&str>, option: Option<&str>, field: Option<&str>| {
        devplane::driven::parse_answer(
            &ask,
            decision,
            option.map(str::to_string),
            None,
            field.map(str::to_string),
            &[],
        )
    };
    assert!(
        parse(Some("deny"), Some("allow_always"), None).is_err(),
        "a deny with an allowing option beside it was not refused"
    );
    assert!(
        parse(None, Some("allow_always"), Some("x")).is_err(),
        "a field on a permission was accepted"
    );
    assert!(parse(Some("deny"), None, None).is_ok());
    assert!(parse(None, Some("reject_once"), None).is_ok());
}

/// The same two contradictions are refused by the command line before
/// anything is read, so neither reaches a store or a host.
#[test]
fn the_answer_command_refuses_a_contradiction_before_it_runs() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["answer", "any", "--deny", "--option", "allow_always"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("cannot be used with"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// An answer to a held permission is not *delivered* until the hook holding
/// it has read it: the row says so before, and `Live` only after.
#[tokio::test]
async fn a_held_answer_is_live_only_once_the_hook_has_read_it() {
    let home = home("delivered");
    let s = store(&home).await;
    let store = s.clone();
    let hold = tokio::spawn(async move {
        devplane::record::hold(
            &store,
            "s-deliver",
            Path::new("/tmp/alpha"),
            "Bash",
            "git push",
            std::time::Duration::from_secs(20),
            false,
        )
        .await
    });
    let mut id = None;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        if let Some(a) = s.open_asks().await.unwrap().first() {
            id = Some(a.id.as_str().to_string());
            break;
        }
    }
    let id = id.expect("the hold raised a row");
    let deny = devplane::driven::Answer::Permission(devplane::driven::Decision::Deny);
    let answered = devplane::driven::answer_without_host(&s, &id, deny, "cli")
        .await
        .unwrap();
    assert_eq!(
        answered.delivery, None,
        "nothing has read the answer yet, so nothing was delivered"
    );
    let behavior = tokio::time::timeout(std::time::Duration::from_secs(10), hold)
        .await
        .expect("the hook reads the answer")
        .unwrap();
    assert_eq!(behavior.as_deref(), Some("deny"));
    let row = s.ask(&id).await.unwrap().unwrap();
    assert_eq!(row.delivery, Some(devplane::core::ask::Delivery::Live));
}

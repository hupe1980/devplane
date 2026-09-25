//! Codex's hooks speak Claude Code's shapes, so one shim serves both, and the run
//! is named after the registering vendor.

use serde_json::Value;
use std::path::{Path, PathBuf};

fn home(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dp-codex-{}-{name}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn hook(home: &Path, payload: &str) -> Value {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["hook", "--vendor", "codex"])
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

/// A Codex session is filed under the Codex source, answered in Codex's shape,
/// and a host-less read names the run `codex`.
#[tokio::test]
async fn a_codex_session_is_recorded_gated_and_named_after_its_vendor() {
    let home = home("codex");
    std::fs::write(
        home.join("policy.toml"),
        "[policy]\nnever_auto = [\"Read(.env)\"]\n",
    )
    .unwrap();
    let quiet = hook(
        &home,
        r#"{"hook_event_name":"SessionStart","session_id":"cx-1","cwd":"/tmp/repo","source":"startup"}"#,
    );
    assert_eq!(quiet, serde_json::json!({}));
    let reply = hook(
        &home,
        r#"{"hook_event_name":"PreToolUse","session_id":"cx-1","cwd":"/tmp/repo",
            "tool_name":"Bash","tool_input":{"command":"cat .env"}}"#,
    );
    assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");

    let store = devplane::store::Store::open(&home.join("devplane.db"))
        .await
        .unwrap();
    let rows = store.shim_events_since(0, 100).await.unwrap();
    assert!(!rows.is_empty());
    assert!(
        rows.iter()
            .all(|(_, e)| e.source == devplane::core::event::Source::CodexHook),
        "every row says which vendor's shim wrote it: {rows:?}"
    );
    let decisions = store.decisions(Some("cx-1"), 10).await.unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].outcome, "deny");

    // Replayed with nothing running, the run carries the vendor's name.
    let mut world = devplane::core::World::new();
    world.replay(rows.into_iter().map(|(_, e)| e));
    let run = world
        .run(&devplane::core::RunId::new("cx-1"))
        .expect("the run");
    assert_eq!(run.agent, "codex");
}

//! Codex — hooks, and only hooks.
//!
//! Codex uses Claude Code's hook shapes (payload, `hookSpecificOutput`,
//! `hooks.json` layout), so the same shim serves it; only the file and the
//! vendor name differ. Codex runs a hook only after the person trusts it in
//! Codex's own dialog, and skips untrusted ones silently — so `connect` says
//! so. Unproved against a live session; the vendor row says so.

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// The word the shim is registered with, and the agent a run is named after.
pub const VENDOR: &str = "codex";

/// Whether an event is registered, and why not when it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ledger {
    /// Registered; the Claude Code reader produces something for it.
    Mapped,
    /// Deliberately not registered.
    Silent,
}

/// The one ledger of Codex's documented hook events, in the vendor's
/// spelling. Every count is computed from here.
///
/// Two are silent: `PreCompact` (the gauge resets on the compaction), and
/// `Interrupt` (a turn id the reader has no row for).
pub const EVENTS: &[(&str, Ledger)] = &[
    ("SessionStart", Ledger::Mapped),
    ("SessionEnd", Ledger::Mapped),
    ("SubagentStart", Ledger::Mapped),
    ("SubagentStop", Ledger::Mapped),
    ("PreToolUse", Ledger::Mapped),
    ("PostToolUse", Ledger::Mapped),
    ("PermissionRequest", Ledger::Mapped),
    ("PreCompact", Ledger::Silent),
    ("PostCompact", Ledger::Mapped),
    ("UserPromptSubmit", Ledger::Mapped),
    ("Stop", Ledger::Mapped),
    ("Interrupt", Ledger::Silent),
];

/// The two events whose answer is read. The rest get the short timeout, as
/// Codex has no `async` flag.
const DECIDING: &[&str] = &["PreToolUse", "PermissionRequest"];

/// Where Codex reads user-level hooks from.
pub fn hooks_path() -> Option<PathBuf> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".codex")))?;
    Some(home.join("hooks.json"))
}

/// The command every entry runs.
fn command(exe: &Path) -> String {
    format!(
        "{} hook --vendor {VENDOR}",
        super::connect::shell_quote(exe)
    )
}

/// Writes Devplane's entries into a `hooks.json` document, replacing earlier
/// ones of ours and touching nothing else. Returns how many are registered.
pub fn install(doc: &mut Map<String, Value>, exe: &Path) -> usize {
    let hooks = doc
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut();
    let Some(hooks) = hooks else {
        return 0;
    };
    let mut added = 0;
    for (event, ledger) in EVENTS {
        if *ledger != Ledger::Silent {
            let list = hooks
                .entry((*event).to_string())
                .or_insert_with(|| json!([]))
                .as_array_mut();
            if let Some(list) = list {
                list.retain(|e| !super::connect::is_ours(e));
                list.push(json!({
                    "hooks": [{
                        "type": "command",
                        "command": command(exe),
                        // A slower gate was not consulted; the permission
                        // event must outlast the longest hold.
                        "timeout": match *event {
                            "PermissionRequest" => crate::observe::hook::HOLD_TIMEOUT_SECS,
                            e if DECIDING.contains(&e) => crate::observe::hook::GATE_TIMEOUT_SECS,
                            _ => 2,
                        },
                    }]
                }));
                added += 1;
            }
        }
    }
    added
}

/// Removes every entry of ours and nothing else. Returns how many went.
pub fn uninstall(doc: &mut Map<String, Value>) -> usize {
    let Some(hooks) = doc.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
        return 0;
    };
    let mut removed = 0;
    hooks.retain(|_, list| {
        if let Some(list) = list.as_array_mut() {
            let before = list.len();
            list.retain(|e| !super::connect::is_ours(e));
            removed += before - list.len();
            !list.is_empty()
        } else {
            true
        }
    });
    removed
}

/// Which of our events an existing document registers, for `doctor`.
pub fn installed(doc: &Map<String, Value>) -> Vec<String> {
    doc.get("hooks")
        .and_then(|h| h.as_object())
        .map(|hooks| {
            hooks
                .iter()
                .filter(|(_, list)| {
                    list.as_array()
                        .is_some_and(|l| l.iter().any(super::connect::is_ours))
                })
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from("/usr/local/bin/devplane")
    }

    /// Every mapped event is one the Claude Code reader produces a row for,
    /// except the deciding events, whose rows the gate writes.
    #[test]
    fn every_mapped_event_is_one_the_reader_knows() {
        for (event, ledger) in EVENTS {
            if DECIDING.contains(event) {
                continue;
            }
            let payload: crate::observe::hook::HookPayload = serde_json::from_value(json!({
                "hook_event_name": event,
                "session_id": "cx-1",
                "cwd": "/tmp/repo",
                "source": "startup",
                "tool_name": "Bash",
                "tool_input": {"command": "ls"},
                "prompt": "hello",
                "reason": "end_turn",
            }))
            .unwrap();
            let produced = !crate::observe::hook::to_events(&payload).events.is_empty();
            match ledger {
                Ledger::Mapped => assert!(produced, "`{event}` is mapped and produces no event"),
                Ledger::Silent => assert!(!produced, "`{event}` is silent and produces an event"),
            }
        }
    }

    /// The person's own entries survive a connect and a disconnect; ours are
    /// replaced on a reconnect rather than doubled.
    #[test]
    fn a_persons_own_hooks_are_never_touched() {
        let mut doc: Map<String, Value> = serde_json::from_value(json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "python3 ~/.codex/hooks/lint.py"}]
                }]
            }
        }))
        .unwrap();
        let mapped = EVENTS.iter().filter(|(_, l)| *l == Ledger::Mapped).count();
        assert_eq!(install(&mut doc, &exe()), mapped);
        assert_eq!(install(&mut doc, &exe()), mapped, "a reconnect replaces");
        let pre = doc["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "theirs and ours, once each: {pre:?}");
        assert!(
            pre[1]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .ends_with("devplane hook --vendor codex")
        );
        assert_eq!(installed(&doc).len(), mapped);

        assert_eq!(uninstall(&mut doc), mapped);
        let pre = doc["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        assert!(
            pre[0]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .contains("lint.py")
        );
        assert!(
            doc["hooks"].get("SessionStart").is_none(),
            "an event left with no entries is not left as an empty list"
        );
        assert!(installed(&doc).is_empty());
    }

    /// The deciding hooks get the longer bound; the rest must not hold a turn.
    #[test]
    fn only_the_deciding_hooks_may_take_their_time() {
        let mut doc = Map::new();
        install(&mut doc, &exe());
        let timeout = |event: &str| doc["hooks"][event][0]["hooks"][0]["timeout"].as_u64();
        assert_eq!(timeout("PreToolUse"), Some(5));
        // The event a permission is held on outlasts the hold.
        assert!(
            timeout("PermissionRequest").unwrap() > crate::core::config::Hold::CEILING.as_secs()
        );
        assert_eq!(timeout("PostToolUse"), Some(2));
    }
}

//! GitHub Copilot — hooks, and the tool vocabulary they arrive in.
//!
//! The other two channels live elsewhere: the ACP server is a line in
//! `acp::builtin()`, and the telemetry is a dialect [`super::otel`] reads.
//!
//! **Three things differ from Claude Code, and each decides something here.**
//!
//! *Only a `command` hook fails closed.* An HTTP `preToolUse` hook falls
//! through to the default permission flow on any error, so a prohibition sent
//! that way evaporates under load. The deciding events ride `vibeplane hook`;
//! the informational ones stay on HTTP, where a process per tool call would
//! cost something for nothing. (Loopback HTTP is available with
//! `COPILOT_HOOK_ALLOW_LOCALHOST=1`, so the reason is the failure mode rather
//! than the transport.)
//!
//! *A hook timeout is fail-open on every event*, administrator policy hooks
//! included. A slow Vibeplane here is not one that blocks the session; it is
//! one that was not consulted.
//!
//! *The tool vocabulary is Copilot's own* — `bash`, `view`, `create` — mapped
//! below to the names the policy already speaks. That mapping is the only
//! translation in this module and deliberately the only one: rules are never
//! rewritten into Copilot's `--allow-tool` vocabulary, because a rule written
//! into somebody else's configuration cannot be read back by `vibeplane audit`.

use crate::core::event::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Copilot's runtime tool names, mapped to the names the policy already uses.
///
/// The vendor's own table, `powershell` → `Bash` included: Copilot reports both
/// shells under one Claude name, so a `Bash(…)` rule governs both here.
const TOOL_NAMES: &[(&str, &str)] = &[
    ("bash", "Bash"),
    ("powershell", "Bash"),
    ("view", "Read"),
    ("create", "Write"),
    ("edit", "Edit"),
    ("str_replace_editor", "Edit"),
    ("apply_patch", "Edit"),
    ("grep", "Grep"),
    ("rg", "Grep"),
    ("glob", "Glob"),
    ("web_fetch", "WebFetch"),
    ("web_search", "WebSearch"),
    ("ask_user", "AskUserQuestion"),
    ("update_todo", "TodoWrite"),
    ("task", "Agent"),
];

/// The name the policy engine knows a Copilot tool by. A tool with no
/// equivalent keeps its own name rather than being guessed into somebody
/// else's.
pub fn tool_name(runtime: &str) -> String {
    TOOL_NAMES
        .iter()
        .find(|(from, _)| *from == runtime)
        .map(|(_, to)| (*to).to_string())
        .unwrap_or_else(|| runtime.to_string())
}

/// One hook payload, in Copilot's native (camelCase) shape.
///
/// Not the Claude-compatible PascalCase aliases, which arrive pre-translated:
/// that form documents the *input* field names and says nothing about the
/// **output** a `preToolUse` hook must return.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookPayload {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<std::path::PathBuf>,
    /// The event, taken from the file it was registered under rather than from
    /// the payload: Copilot's native events do not carry their own name.
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_args: Option<Value>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
}

impl HookPayload {
    /// The tool input. The policy reads a tool's content field (`command`,
    /// `file_path`, `url`), which is inside this object whatever the wrapper
    /// around it is called.
    pub fn input(&self) -> Value {
        self.tool_args.clone().unwrap_or_else(|| json!({}))
    }

    /// The name the policy engine knows this call's tool by.
    pub fn tool(&self) -> String {
        self.tool_name.as_deref().map(tool_name).unwrap_or_default()
    }
}

/// What one Copilot hook payload means, in Vibeplane's own event model.
///
/// Smaller than the Claude mapping because Copilot publishes fewer events that
/// say anything new: no `PostCompact`, no model switch, no elicitation pair, no
/// roster.
pub fn to_events(p: &HookPayload) -> Vec<Event> {
    match p.event.as_str() {
        "sessionStart" => vec![Event::SessionStarted {
            cwd: p.cwd.clone().unwrap_or_default(),
            source: Some("copilot".into()),
            model: None,
            // Copilot publishes no entrypoint of its own, so the honest answer
            // is the vendor — which is what the board needs from this field and
            // is known from the channel the payload arrived on.
            entrypoint: Some("copilot".into()),
        }],
        "userPromptSubmitted" => vec![Event::PromptSubmitted {
            chars: p.prompt.as_ref().map(|t| t.chars().count()).unwrap_or(0),
        }],
        "preToolUse" => vec![Event::ToolStarted {
            tool: p.tool(),
            input: p.input(),
        }],
        "postToolUse" => vec![Event::ToolFinished {
            tool: p.tool(),
            ok: true,
            duration_ms: None,
        }],
        "postToolUseFailure" => vec![Event::ToolFinished {
            tool: p.tool(),
            ok: false,
            duration_ms: None,
        }],
        "agentStop" => vec![Event::TurnEnded],
        "errorOccurred" => vec![Event::TurnFailed {
            message: p
                .error
                .clone()
                .unwrap_or_else(|| "the agent reported an error".into()),
        }],
        "subagentStart" => vec![Event::SubagentStarted {
            agent_id: p.agent_name.clone().unwrap_or_else(|| "subagent".into()),
            kind: p.agent_name.clone(),
        }],
        "subagentStop" => vec![Event::SubagentStopped {
            agent_id: p.agent_name.clone().unwrap_or_else(|| "subagent".into()),
        }],
        "sessionEnd" => vec![Event::SessionEnded {
            reason: p.reason.clone(),
        }],
        // An event this map does not know costs one event rather than the
        // batch — the same rule the telemetry reader follows, and the reason a
        // provider can add one without taking the channel down.
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// The gate's reply
// ---------------------------------------------------------------------------

/// What a `preToolUse` hook may answer: a prohibition, or nothing.
///
/// Copilot reads a single top-level object. `allow` is never sent — it would
/// skip the provider's own permission flow, including approvals a person
/// already saved, which is a widening dressed as a convenience.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct GateReply {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,
}

impl GateReply {
    /// No opinion: the provider's own permission flow decides.
    pub fn undecided() -> Self {
        Self::default()
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            permission_decision: Some("deny".into()),
            permission_decision_reason: Some(reason.into()),
        }
    }

    /// Force a prompt the provider's own flow would have skipped.
    pub fn ask(reason: impl Into<String>) -> Self {
        Self {
            permission_decision: Some("ask".into()),
            permission_decision_reason: Some(reason.into()),
        }
    }

    /// The wire form. Copilot reads `permissionDecision`, not the nested
    /// `hookSpecificOutput` object Claude Code uses.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(d) = &self.permission_decision {
            m.insert("permissionDecision".into(), json!(d));
        }
        if let Some(r) = &self.permission_decision_reason {
            m.insert("permissionDecisionReason".into(), json!(r));
        }
        Value::Object(m)
    }
}

// ---------------------------------------------------------------------------
// connect
// ---------------------------------------------------------------------------

/// Where Copilot reads user-level hook files from.
pub fn hooks_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("COPILOT_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".copilot")))?;
    Some(home.join("hooks"))
}

/// The one file `connect copilot` writes, and `disconnect copilot` deletes.
pub const HOOKS_FILE: &str = "vibeplane.json";

/// The hook registration: one file, entirely Vibeplane's.
///
/// Copilot loads every `*.json` in `~/.copilot/hooks/`, so the whole
/// installation can be written and removed without touching a line the user
/// wrote — unlike Claude Code, whose hooks live inside the user's own
/// `settings.json`.
pub fn hooks_file(base_url: &str, token: &str, exe: &std::path::Path) -> Value {
    // The events that decide. A command hook, because the HTTP one fails open.
    let gate = json!([{
        "type": "command",
        "exec": exe.to_string_lossy(),
        "args": ["hook", "--gate", "copilot"],
        "timeoutSec": 5,
    }]);
    let mut hooks = Map::new();
    hooks.insert("preToolUse".into(), gate.clone());

    // Everything else is informational and goes over HTTP, where it costs no
    // process. `async` has no equivalent here; the timeout is what bounds it.
    for event in [
        "sessionStart",
        "sessionEnd",
        "userPromptSubmitted",
        "postToolUse",
        "postToolUseFailure",
        "agentStop",
        "subagentStart",
        "subagentStop",
        "errorOccurred",
    ] {
        hooks.insert(
            event.into(),
            json!([{
                "type": "http",
                "url": format!("{base_url}/vibeplane/copilot/hook?event={event}"),
                "headers": { "Authorization": format!("Bearer {token}") },
                "timeoutSec": 5,
            }]),
        );
    }
    json!({ "version": 1, "hooks": Value::Object(hooks) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copilot_tool_is_known_by_the_name_the_rules_use() {
        // The one translation in this module, from the vendor's own table.
        // Without it a `Read(.env)` rule would not govern Copilot's `view`, and
        // the failure would be silent in the direction that grants.
        assert_eq!(tool_name("bash"), "Bash");
        assert_eq!(tool_name("view"), "Read");
        assert_eq!(tool_name("create"), "Write");
        assert_eq!(tool_name("str_replace_editor"), "Edit");
        assert_eq!(tool_name("web_fetch"), "WebFetch");
        // A tool with no equivalent keeps its own name rather than being
        // guessed into somebody else's.
        assert_eq!(tool_name("some_future_tool"), "some_future_tool");
    }

    #[test]
    fn the_gate_never_answers_allow() {
        // The same restraint as on Claude Code's `PreToolUse`: a grant here
        // skips the provider's own permission flow, including approvals a
        // person already saved.
        assert_eq!(GateReply::undecided().to_json(), json!({}));
        assert_eq!(
            GateReply::deny("Bash(rm -rf *)").to_json(),
            json!({"permissionDecision": "deny", "permissionDecisionReason": "Bash(rm -rf *)"})
        );
        assert_eq!(
            GateReply::ask("Bash(git push *)").to_json()["permissionDecision"],
            json!("ask")
        );
    }

    #[test]
    fn a_tool_call_becomes_the_same_event_a_claude_one_does() {
        let p: HookPayload = serde_json::from_value(json!({
            "sessionId": "s-1",
            "cwd": "/repo",
            "toolName": "bash",
            "toolArgs": {"command": "pnpm test"},
        }))
        .unwrap();
        let p = HookPayload {
            event: "preToolUse".into(),
            ..p
        };
        assert_eq!(p.tool(), "Bash");
        assert_eq!(p.input()["command"], json!("pnpm test"));
        match &to_events(&p)[..] {
            [Event::ToolStarted { tool, input }] => {
                assert_eq!(tool, "Bash");
                assert_eq!(input["command"], json!("pnpm test"));
            }
            other => panic!("expected a started tool, got {other:?}"),
        }
    }

    #[test]
    fn an_event_this_map_does_not_know_costs_one_event() {
        let p = HookPayload {
            event: "somethingNew".into(),
            ..Default::default()
        };
        assert!(to_events(&p).is_empty());
    }

    #[test]
    fn the_deciding_hook_is_a_command_and_the_rest_are_not() {
        // The whole reason this provider's registration differs from Claude
        // Code's: an HTTP `preToolUse` hook fails open.
        let f = hooks_file("http://127.0.0.1:47831", "t", std::path::Path::new("/vp"));
        assert_eq!(f["hooks"]["preToolUse"][0]["type"], json!("command"));
        assert_eq!(f["hooks"]["postToolUse"][0]["type"], json!("http"));
        assert_eq!(f["version"], json!(1));
    }
}

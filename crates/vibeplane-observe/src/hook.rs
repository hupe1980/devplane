//! Claude Code hooks — the lifecycle channel.
//!
//! Hooks are the only documented way to learn about a session that Vibeplane
//! did not start, and they cover every surface: a terminal, the VS Code
//! extension, the desktop app, a headless run.
//!
//! # Which events are worth subscribing to
//!
//! `PermissionRequest` is the important one, and not for the reason it looks
//! like. It fires *the moment* Claude asks for permission, whereas the
//! `permission_prompt` notification waits about six seconds and, in a terminal,
//! defers again on every keystroke — so for an attended session it may never
//! arrive at all. `PermissionRequest` therefore does two jobs: it is the policy
//! gate, and it is the instant "this session is blocked" signal.
//!
//! It must be answered immediately, whatever the answer. Holding it open would
//! stall the session before its dialog ever appears, which is the opposite of
//! the product. A run that no rule covers is marked blocked *and* handed back
//! to Claude to prompt the human itself.
//!
//! # What is deliberately not installed
//!
//! `WorktreeCreate` replaces Claude Code's own `git worktree` logic entirely
//! when a hook is configured for it. An observer that installed it would break
//! `claude --worktree`, subagent isolation and background sessions for every
//! repository on the machine. Worktrees are learned from `CwdChanged`, the
//! status line and the background roster instead.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use vibeplane_domain::event::{Event, WaitingFor};

/// The common fields every hook payload carries, plus the ones we read.
#[derive(Debug, Clone, Deserialize)]
pub struct HookPayload {
    pub hook_event_name: String,
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub transcript_path: Option<PathBuf>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<Value>,
    #[serde(default)]
    pub tool_response: Option<Value>,
    /// `Notification` only: which notification type fired.
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    /// `SessionStart` / `SessionEnd`.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    /// `SubagentStart` / `SubagentStop`.
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub model: Option<Value>,
    /// Anything else, so a new field never costs us a parse.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, Value>,
}

impl HookPayload {
    pub fn model_name(&self) -> Option<String> {
        match &self.model {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Object(o)) => o
                .get("id")
                .or_else(|| o.get("display_name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }
    }
}

/// What the daemon should do with a hook payload.
#[derive(Debug, Clone, PartialEq)]
pub struct HookOutcome {
    /// Events to record. Usually one; a permission request that no rule covers
    /// produces the block, and one that a rule covers produces the decision.
    pub events: Vec<Event>,
    /// Whether the session is asking a question we should answer. Only
    /// `PermissionRequest` sets this.
    pub needs_decision: bool,
}

impl HookOutcome {
    fn just(event: Event) -> Self {
        Self {
            events: vec![event],
            needs_decision: false,
        }
    }
    fn none() -> Self {
        Self {
            events: Vec::new(),
            needs_decision: false,
        }
    }
}

/// Translates a hook payload into domain events.
///
/// Unknown events yield nothing rather than an error: Claude Code adds hook
/// events regularly, and an observer that fails on one it has not heard of is
/// an observer that breaks on upgrade.
pub fn to_events(p: &HookPayload) -> HookOutcome {
    let cwd = p.cwd.clone().unwrap_or_else(|| PathBuf::from("."));
    match p.hook_event_name.as_str() {
        "SessionStart" => HookOutcome::just(Event::SessionStarted {
            cwd,
            source: p.source.clone(),
            model: p.model_name(),
            entrypoint: None,
        }),

        "UserPromptSubmit" => HookOutcome::just(Event::PromptSubmitted {
            chars: p.prompt.as_deref().map(str::len).unwrap_or(0),
        }),

        "PreToolUse" => {
            let tool = p.tool_name.clone().unwrap_or_default();
            // `AskUserQuestion` is a tool call, and it is the only one whose
            // arrival means a human is needed. Reading it here is instant;
            // waiting for a notification about it is not.
            if tool == "AskUserQuestion" {
                let (question, options) = parse_ask_user_question(p.tool_input.as_ref());
                return HookOutcome::just(Event::QuestionAsked { question, options });
            }
            HookOutcome::just(Event::ToolStarted {
                tool,
                input: p.tool_input.clone().unwrap_or(Value::Null),
            })
        }

        "PostToolUse" => HookOutcome::just(Event::ToolFinished {
            tool: p.tool_name.clone().unwrap_or_default(),
            ok: true,
            duration_ms: None,
        }),

        "PostToolUseFailure" => HookOutcome::just(Event::ToolFinished {
            tool: p.tool_name.clone().unwrap_or_default(),
            ok: false,
            duration_ms: None,
        }),

        // The instant blocked signal. The caller evaluates policy and decides
        // which of the two possible events this becomes.
        "PermissionRequest" => HookOutcome {
            events: Vec::new(),
            needs_decision: true,
        },

        "PermissionDenied" => HookOutcome::just(Event::PermissionDecided {
            tool: p.tool_name.clone().unwrap_or_default(),
            decision: "deny".into(),
            by: "claude".into(),
        }),

        "Notification" => match p.notification_type.as_deref() {
            // A late backstop for a prompt we already know about, and the only
            // signal for a sandboxed command's network request, which
            // `PermissionRequest` does not fire for.
            Some("permission_prompt") => HookOutcome::just(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: p.message.clone(),
                request_id: None,
            }),
            Some("elicitation_dialog") | Some("elicitation_url_dialog") => {
                HookOutcome::just(Event::Blocked {
                    waiting_for: WaitingFor::Question,
                    message: p.message.clone(),
                    request_id: None,
                })
            }
            Some("idle_prompt") => HookOutcome::just(Event::Blocked {
                waiting_for: WaitingFor::Idle,
                message: None,
                request_id: None,
            }),
            _ => HookOutcome::none(),
        },

        "Stop" => HookOutcome::just(Event::TurnEnded),

        "StopFailure" => HookOutcome::just(Event::TurnFailed {
            message: p
                .message
                .clone()
                .or_else(|| p.reason.clone())
                .unwrap_or_else(|| "turn ended with an API error".into()),
        }),

        "SubagentStart" => HookOutcome::just(Event::SubagentStarted {
            agent_id: p.agent_id.clone().unwrap_or_default(),
            kind: p.agent_type.clone(),
        }),
        "SubagentStop" => HookOutcome::just(Event::SubagentStopped {
            agent_id: p.agent_id.clone().unwrap_or_default(),
        }),

        "CwdChanged" => {
            // Entering a worktree moves the session's working directory, which
            // is how a worktree is detected without replacing Claude's own
            // worktree machinery.
            if is_worktree_path(&cwd) {
                HookOutcome {
                    events: vec![
                        Event::CwdChanged { cwd: cwd.clone() },
                        Event::WorktreeEntered {
                            path: cwd,
                            branch: None,
                        },
                    ],
                    needs_decision: false,
                }
            } else {
                HookOutcome::just(Event::CwdChanged { cwd })
            }
        }

        "PreCompact" => HookOutcome::just(Event::Compacted),

        "SessionEnd" => HookOutcome::just(Event::SessionEnded {
            reason: p.reason.clone(),
        }),

        _ => HookOutcome::none(),
    }
}

/// A path Claude Code created as an isolated worktree.
fn is_worktree_path(p: &std::path::Path) -> bool {
    p.to_string_lossy().contains("/.claude/worktrees/")
}

/// Pulls the question and its options out of an `AskUserQuestion` tool input.
/// Tolerant of shape changes: a question with no options is still a question.
fn parse_ask_user_question(input: Option<&Value>) -> (String, Vec<String>) {
    let Some(v) = input else {
        return ("Agent asked a question".into(), Vec::new());
    };
    // The tool takes a list of questions; the first is the one on screen.
    let q = v
        .get("questions")
        .and_then(|q| q.as_array())
        .and_then(|a| a.first())
        .unwrap_or(v);

    let question = q
        .get("question")
        .or_else(|| q.get("header"))
        .and_then(|s| s.as_str())
        .unwrap_or("Agent asked a question")
        .to_string();

    let options = q
        .get("options")
        .and_then(|o| o.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|o| {
                    o.get("label")
                        .and_then(|l| l.as_str())
                        .or_else(|| o.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    (question, options)
}

/// The JSON a `PermissionRequest` hook returns.
///
/// An empty response means "no decision": Claude Code shows its own dialog, and
/// the human answers where they already are. That is the right answer for a
/// session Vibeplane only observes, and it is why the hook can be answered in a
/// millisecond without waiting for anybody.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PermissionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: Option<PermissionHookOutput>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PermissionHookOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: &'static str,
    pub decision: PermissionDecision,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PermissionDecision {
    pub behavior: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl PermissionResponse {
    /// No decision: Claude Code prompts as it normally would.
    pub fn undecided() -> Self {
        Self {
            hook_specific_output: None,
        }
    }

    pub fn allow() -> Self {
        Self::decide("allow", None)
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self::decide("deny", Some(reason.into()))
    }

    fn decide(behavior: &'static str, message: Option<String>) -> Self {
        Self {
            hook_specific_output: Some(PermissionHookOutput {
                hook_event_name: "PermissionRequest",
                decision: PermissionDecision { behavior, message },
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload(v: serde_json::Value) -> HookPayload {
        serde_json::from_value(v).expect("payload parses")
    }

    #[test]
    fn a_tool_call_becomes_a_tool_event() {
        let out = to_events(&payload(json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "cwd": "/repo",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}
        })));
        assert!(matches!(out.events[0], Event::ToolStarted { .. }));
        assert!(!out.needs_decision);
    }

    #[test]
    fn ask_user_question_is_read_at_pre_tool_use() {
        // Waiting for a notification about a question costs six seconds and,
        // in a terminal, may never arrive at all.
        let out = to_events(&payload(json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "tool_name": "AskUserQuestion",
            "tool_input": {"questions": [{
                "question": "Keep the legacy /v1/login route?",
                "options": [{"label": "Keep"}, {"label": "Remove"}]
            }]}
        })));
        match &out.events[0] {
            Event::QuestionAsked { question, options } => {
                assert!(question.starts_with("Keep the legacy"));
                assert_eq!(options, &["Keep", "Remove"]);
            }
            other => panic!("expected a question, got {other:?}"),
        }
    }

    #[test]
    fn permission_request_asks_the_caller_to_decide() {
        let out = to_events(&payload(json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf node_modules"}
        })));
        assert!(out.needs_decision);
        assert!(out.events.is_empty());
    }

    #[test]
    fn notification_types_map_to_the_right_block() {
        for (ty, expect) in [
            ("permission_prompt", WaitingFor::Permission),
            ("elicitation_dialog", WaitingFor::Question),
            ("idle_prompt", WaitingFor::Idle),
        ] {
            let out = to_events(&payload(json!({
                "hook_event_name": "Notification",
                "session_id": "s1",
                "notification_type": ty
            })));
            match &out.events[0] {
                Event::Blocked { waiting_for, .. } => assert_eq!(waiting_for, &expect),
                other => panic!("{ty} produced {other:?}"),
            }
        }
    }

    #[test]
    fn agent_notifications_are_ignored() {
        // `agent_needs_input` and `agent_completed` fire only while `claude
        // agents` is open in a terminal, so treating them as state would make
        // the board depend on whether a TUI happens to be running.
        let out = to_events(&payload(json!({
            "hook_event_name": "Notification",
            "session_id": "s1",
            "notification_type": "agent_needs_input"
        })));
        assert!(out.events.is_empty());
    }

    #[test]
    fn entering_a_worktree_is_detected_from_the_directory_change() {
        let out = to_events(&payload(json!({
            "hook_event_name": "CwdChanged",
            "session_id": "s1",
            "cwd": "/repo/.claude/worktrees/feature-auth"
        })));
        assert_eq!(out.events.len(), 2);
        assert!(matches!(out.events[1], Event::WorktreeEntered { .. }));
    }

    #[test]
    fn an_unknown_hook_event_is_ignored_not_an_error() {
        let out = to_events(&payload(json!({
            "hook_event_name": "SomethingNewInTheNextRelease",
            "session_id": "s1"
        })));
        assert!(out.events.is_empty());
    }

    #[test]
    fn an_undecided_response_is_empty_json() {
        // Claude Code must see no decision at all, not a decision that says
        // nothing — anything else changes what the user sees.
        let s = serde_json::to_string(&PermissionResponse::undecided()).unwrap();
        assert_eq!(s, "{}");
    }

    #[test]
    fn an_allow_response_carries_the_documented_shape() {
        let v = serde_json::to_value(PermissionResponse::allow()).unwrap();
        assert_eq!(
            v["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(v["hookSpecificOutput"]["decision"]["behavior"], "allow");
    }

    #[test]
    fn model_can_be_a_string_or_an_object() {
        let a = payload(json!({"hook_event_name":"SessionStart","session_id":"s","model":"opus"}));
        assert_eq!(a.model_name().as_deref(), Some("opus"));
        let b = payload(
            json!({"hook_event_name":"SessionStart","session_id":"s","model":{"id":"claude-opus-5"}}),
        );
        assert_eq!(b.model_name().as_deref(), Some("claude-opus-5"));
    }
}

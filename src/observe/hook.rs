//! Claude Code hooks — the lifecycle channel.
//!
//! The only documented way to learn about a session Vibeplane did not start,
//! and it covers every surface: a terminal, the VS Code extension, the desktop
//! app, a headless run.
//!
//! `PermissionRequest` is the one that matters, and it does two jobs: it is the
//! policy gate, and it is the instant "this session is blocked" signal. The
//! `permission_prompt` notification says the same thing about six seconds later
//! and, in a terminal, defers again on every keystroke. It must be answered
//! immediately whatever the answer — holding it open stalls the session before
//! its own dialog appears — so a call no rule covers is marked blocked *and*
//! handed back for Claude Code to prompt about itself.
//!
//! `WorktreeCreate` is deliberately never installed: configuring it replaces
//! Claude Code's own `git worktree` logic, which would break `claude
//! --worktree`, subagent isolation and background sessions machine-wide.
//! Worktrees are learned from `CwdChanged`, the status line and the roster.
use crate::core::event::{Choice, Event, WaitingFor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

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
    /// `Elicitation` / `ElicitationResult`: which MCP server is asking.
    #[serde(default)]
    pub mcp_server_name: Option<String>,
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
    /// `ElicitationResult`: `accept`, `decline` or `cancel`.
    #[serde(default)]
    pub action: Option<String>,
    /// `ConfigChange`: which file changed, when it names one.
    #[serde(default)]
    pub file_path: Option<PathBuf>,
    /// `TaskCreated` / `TaskCompleted`.
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub task_subject: Option<String>,
    /// `PreModelSwitch`: the model the session is about to move to. The context
    /// gauge is a percentage *of* that model's window, so it changes here
    /// rather than when the first request on the new model returns.
    #[serde(default)]
    pub to_model: Option<String>,
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

/// What the daemon should record for a hook payload.
///
/// Events only. A permission request arrives at its own endpoint, because it is
/// the one hook whose *reply* matters; nothing on this path is answerable.
#[derive(Debug, Clone, PartialEq)]
pub struct HookOutcome {
    /// Events to record. Usually one; `CwdChanged` into a worktree is two.
    pub events: Vec<Event>,
}

impl HookOutcome {
    fn just(event: Event) -> Self {
        Self {
            events: vec![event],
        }
    }
    fn none() -> Self {
        Self { events: Vec::new() }
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
            // Characters, not bytes. The prompt itself is never stored — this
            // is the only thing recorded about it — so a number that says
            // "chars" and counts UTF-8 bytes is the whole field being wrong for
            // anybody not typing ASCII.
            chars: p.prompt.as_deref().map(|s| s.chars().count()).unwrap_or(0),
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

        // The instant blocked signal, and the only hook whose reply matters.
        // It is delivered to `/vibeplane/policy` instead, which answers it; a
        // copy arriving here has nothing to record.
        "PermissionRequest" => HookOutcome::none(),

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
                // An observed session's dialog is Claude Code's own: Vibeplane
                // can show that it is there, never answer it.
                request_id: None,
                options: Vec::new(),
            }),
            Some("elicitation_dialog") | Some("elicitation_url_dialog") => {
                HookOutcome::just(Event::Blocked {
                    waiting_for: WaitingFor::Question,
                    message: p.message.clone(),
                    request_id: None,
                    options: Vec::new(),
                })
            }
            Some("idle_prompt") => HookOutcome::just(Event::Blocked {
                waiting_for: WaitingFor::Idle,
                message: None,
                request_id: None,
                options: Vec::new(),
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
                }
            } else {
                HookOutcome::just(Event::CwdChanged { cwd })
            }
        }

        // After, not before. The gauge is a level, and it only drops once the
        // window has actually been rewritten.
        "PostCompact" => HookOutcome::just(Event::Compacted),
        // Kept readable because a settings file written by an older Vibeplane
        // still has it, and an event an observer does not understand is an
        // event it drops silently.
        "PreCompact" => HookOutcome::none(),

        // The session changed model, so the context window it is measured
        // against changed with it.
        "PostModelSwitch" => match p.model_name() {
            Some(model) => HookOutcome::just(Event::ModelChanged { model }),
            None => HookOutcome::none(),
        },

        // An MCP server is asking the user something, reported the moment it
        // asks rather than six seconds later through a notification.
        "Elicitation" => HookOutcome::just(Event::Blocked {
            waiting_for: WaitingFor::Question,
            message: p
                .message
                .clone()
                .or_else(|| p.mcp_server_name.clone().map(|s| format!("{s} is asking"))),
            // The dialog belongs to Claude Code. Vibeplane can say it is there
            // and raise the window that has it; it cannot answer it.
            request_id: None,
            options: Vec::new(),
        }),

        // A question has been answered, wherever it was answered. Without this
        // an elicitation resolved in a terminal sat in the inbox for ever,
        // asking for a decision that had already been made.
        "ElicitationResult" => HookOutcome::just(Event::QuestionAnswered {
            action: p.action.clone().unwrap_or_else(|| "accept".into()),
        }),

        // Somebody edited the settings Vibeplane writes its own hooks into.
        // `policy_settings` is the interesting one: managed policy can block
        // loopback hooks, and the only other symptom is silence.
        "ConfigChange" => HookOutcome::just(Event::ConfigChanged {
            source: p.source.clone().unwrap_or_else(|| "settings".into()),
            path: p.file_path.clone(),
        }),

        // The agent's own task list, for a session Vibeplane only watches.
        "TaskCreated" | "TaskCompleted" => match (&p.task_id, &p.task_subject) {
            (Some(id), Some(subject)) => HookOutcome::just(Event::TaskChanged {
                id: id.clone(),
                subject: subject.clone(),
                done: p.hook_event_name == "TaskCompleted",
            }),
            _ => HookOutcome::none(),
        },

        // The size of the context window is about to change, which is the
        // denominator of the gauge. `PostModelSwitch` says the same thing
        // afterwards; this says it before the next turn is priced against it.
        "PreModelSwitch" => match p.to_model.clone().or_else(|| p.model_name()) {
            Some(model) => HookOutcome::just(Event::ModelChanged { model }),
            None => HookOutcome::none(),
        },

        "SessionEnd" => HookOutcome::just(Event::SessionEnded {
            reason: p.reason.clone(),
        }),

        _ => HookOutcome::none(),
    }
}

/// A checkout that some other repository owns.
///
/// The same test the board and the policy use, so a worktree that is a worktree
/// to one of them is a worktree to all three. It covers a `git worktree add`
/// anywhere on the disk as well as Claude Code's `.claude/worktrees/`.
fn is_worktree_path(p: &std::path::Path) -> bool {
    crate::core::project::is_worktree(p)
}

/// Pulls the question and its options out of an `AskUserQuestion` tool input.
/// Tolerant of shape changes: a question with no options is still a question.
fn parse_ask_user_question(input: Option<&Value>) -> (String, Vec<Choice>) {
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
                        // A label and no id: Claude Code owns this dialog, so
                        // the options can be read here and chosen only there.
                        .map(Choice::label)
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

/// The JSON a `PreToolUse` hook returns.
///
/// Differently shaped from [`PermissionResponse`], and not interchangeable with
/// it: this hook fires before every tool call in every mode, which is the only
/// way a prohibition reaches a session in auto mode, where a classifier
/// approves routine calls and no prompt is ever shown.
///
/// It may only ever carry a prohibition. An `allow` here skips the permission
/// system, the classifier included. `ask` is the useful half — a hook's `ask`
/// forces a prompt the classifier "can't approve the call silently".
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreToolUseResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: Option<PreToolUseOutput>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreToolUseOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: &'static str,
    #[serde(rename = "permissionDecision")]
    pub permission_decision: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "permissionDecisionReason")]
    pub permission_decision_reason: Option<String>,
}

impl PreToolUseResponse {
    /// No decision: the call proceeds into Claude Code's own permission flow.
    pub fn undecided() -> Self {
        Self {
            hook_specific_output: None,
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self::decide("deny", reason.into())
    }

    /// Force a prompt, in every mode. The only way a project's `always_ask`
    /// rule reaches a session running in auto mode.
    pub fn ask(reason: impl Into<String>) -> Self {
        Self::decide("ask", reason.into())
    }

    fn decide(decision: &'static str, reason: String) -> Self {
        Self {
            hook_specific_output: Some(PreToolUseOutput {
                hook_event_name: "PreToolUse",
                permission_decision: decision,
                permission_decision_reason: Some(reason),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Deciding, in whichever process is holding the payload
// ---------------------------------------------------------------------------

/// The session id `vibeplane doctor` uses when it runs the gate to see whether
/// it answers.
///
/// The gate is a real gate however it was started, so the probe gets a real
/// verdict — and a real verdict used to get a real row in the decision log.
/// **A diagnostic must not write history.** Running `vibeplane doctor` three
/// times left three refusals of a command nobody ran, in the one table that is
/// never pruned and exists to answer "why did that happen".
pub const PROBE_SESSION: &str = "vibeplane-doctor";

/// What a tool call is called in the decision log: the tool, and as much of its
/// specifier as fits.
///
/// Here rather than in the API because the process that *decides* is now the
/// one that names the subject, and the daemon only writes down what it is told.
pub fn describe_call(tool: &str, input: &serde_json::Value) -> String {
    match crate::core::policy::rule_content(tool, input) {
        Some(c) => format!("{tool}: {}", crate::core::text::clip(&c, 120)),
        None => tool.to_string(),
    }
}

/// Turns a verdict into what a Claude Code `PermissionRequest` hook returns.
///
/// Extracted so the daemon and the `command` hook cannot drift: two processes
/// answering the same question in two spellings is a difference nobody would
/// see until it mattered.
pub fn permission_reply(verdict: &crate::core::Verdict) -> PermissionResponse {
    use crate::core::Verdict;
    match verdict {
        Verdict::Allow { .. } => PermissionResponse::allow(),
        Verdict::Deny { rule } => {
            PermissionResponse::deny(format!("denied by Vibeplane policy rule {rule}"))
        }
        // A project said a person decides this one. The reply is the same as
        // `Undecided` — Claude Code prompts exactly as it would have — but the
        // rule is recorded, because "nobody had an opinion" and "the project
        // asked to be asked" are different facts.
        Verdict::Ask { .. } | Verdict::Undecided => PermissionResponse::undecided(),
    }
}

/// Turns a verdict into what a Claude Code `PreToolUse` hook returns.
///
/// Only ever a prohibition. An `allow` here skips the permission system
/// altogether, the auto-mode classifier included, so a rule that merely meant
/// "no need to ask me" would switch off a safety layer the user chose.
pub fn pre_tool_use_reply(verdict: &crate::core::Verdict) -> PreToolUseResponse {
    use crate::core::Verdict;
    match verdict {
        Verdict::Deny { rule } => {
            PreToolUseResponse::deny(format!("denied by Vibeplane policy rule {rule}"))
        }
        Verdict::Ask { rule } => {
            PreToolUseResponse::ask(format!("{rule} asks that a person decides this"))
        }
        _ => PreToolUseResponse::undecided(),
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
    fn an_answered_elicitation_stops_being_asked() {
        // The bug: `Elicitation` opened a question and nothing ever closed it,
        // so a dialog answered in the person's own terminal left an inbox item
        // for a decision already made. An inbox that shows resolved requests
        // is one people learn to skim — and that costs the permission request
        // beside it, not just the stale row.
        let asked = to_events(&payload(json!({
            "hook_event_name": "Elicitation",
            "session_id": "s1",
            "mcp_server_name": "linear",
        })));
        assert!(matches!(
            asked.events.first(),
            Some(Event::Blocked {
                waiting_for: WaitingFor::Question,
                ..
            })
        ));

        let answered = to_events(&payload(json!({
            "hook_event_name": "ElicitationResult",
            "session_id": "s1",
            "mcp_server_name": "linear",
            "action": "accept",
            "mode": "form",
        })));
        match answered.events.first() {
            Some(Event::QuestionAnswered { action }) => assert_eq!(action, "accept"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_observed_session_reports_its_own_task_list() {
        let created = to_events(&payload(json!({
            "hook_event_name": "TaskCreated",
            "session_id": "s1",
            "task_id": "task-001",
            "task_subject": "Implement user authentication",
        })));
        match created.events.first() {
            Some(Event::TaskChanged { id, subject, done }) => {
                assert_eq!(id, "task-001");
                assert_eq!(subject, "Implement user authentication");
                assert!(!done);
            }
            other => panic!("{other:?}"),
        }
        let finished = to_events(&payload(json!({
            "hook_event_name": "TaskCompleted",
            "session_id": "s1",
            "task_id": "task-001",
            "task_subject": "Implement user authentication",
        })));
        assert!(matches!(
            finished.events.first(),
            Some(Event::TaskChanged { done: true, .. })
        ));
        // A task with no subject names nothing a person could read.
        assert!(
            to_events(&payload(json!({
                "hook_event_name": "TaskCreated", "session_id": "s1", "task_id": "t"
            })))
            .events
            .is_empty()
        );
    }

    #[test]
    fn a_settings_change_is_recorded_with_its_source() {
        // Vibeplane writes its own hooks into one of these files. Somebody
        // removing them, or a managed policy arriving that blocks loopback,
        // otherwise shows up only as a channel that stopped speaking.
        match to_events(&payload(json!({
            "hook_event_name": "ConfigChange",
            "session_id": "s1",
            "source": "user_settings",
            "file_path": "/Users/x/.claude/settings.json",
        })))
        .events
        .first()
        {
            Some(Event::ConfigChanged { source, path }) => {
                assert_eq!(source, "user_settings");
                assert!(path.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_context_window_changes_before_the_switch_not_after() {
        // The gauge is a percentage *of* the model's window. `PostModelSwitch`
        // says so afterwards; this says it before the next turn is priced
        // against the wrong denominator.
        match to_events(&payload(json!({
            "hook_event_name": "PreModelSwitch",
            "session_id": "s1",
            "from_model": "claude-sonnet-5",
            "to_model": "claude-opus-5",
            "source": "command",
        })))
        .events
        .first()
        {
            Some(Event::ModelChanged { model }) => assert_eq!(model, "claude-opus-5"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_gauge_resets_after_compaction_not_before() {
        // `PreCompact` fires while the window is still full. Resetting there
        // made the gauge drop to zero, the `context_high` item disappear, and
        // both come back a moment later when the summarisation request landed.
        assert!(
            to_events(&payload(json!({
                "hook_event_name": "PreCompact", "session_id": "s1", "trigger": "auto"
            })))
            .events
            .is_empty()
        );
        assert!(matches!(
            to_events(&payload(json!({
                "hook_event_name": "PostCompact", "session_id": "s1", "trigger": "auto"
            })))
            .events[0],
            Event::Compacted
        ));
    }

    #[test]
    fn switching_model_changes_the_window_the_gauge_divides_by() {
        let out = to_events(&payload(json!({
            "hook_event_name": "PostModelSwitch",
            "session_id": "s1",
            "model": {"id": "claude-opus-5[1m]"}
        })));
        assert!(
            matches!(&out.events[0], Event::ModelChanged { model } if model.contains("1m")),
            "{:?}",
            out.events
        );
    }

    #[test]
    fn an_mcp_server_asking_a_question_blocks_the_run_at_once() {
        // The `elicitation_dialog` notification says the same thing about six
        // seconds later and, in a terminal, defers again on every keystroke —
        // the identical argument that makes `PermissionRequest` the hinge.
        let out = to_events(&payload(json!({
            "hook_event_name": "Elicitation",
            "session_id": "s1",
            "mcp_server_name": "linear",
            "message": "Which team?"
        })));
        match &out.events[0] {
            Event::Blocked {
                waiting_for,
                message,
                request_id,
                ..
            } => {
                assert_eq!(*waiting_for, WaitingFor::Question);
                assert_eq!(message.as_deref(), Some("Which team?"));
                assert!(request_id.is_none(), "the dialog is Claude Code's own");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_permission_claude_code_refused_by_itself_is_recorded() {
        // Auto mode denying a call is a decision about this session that
        // nothing else reports. The receiver always knew how to read it; until
        // now `connect` never subscribed to the event.
        let out = to_events(&payload(json!({
            "hook_event_name": "PermissionDenied",
            "session_id": "s1",
            "tool_name": "Bash"
        })));
        assert!(matches!(
            &out.events[0],
            Event::PermissionDecided { decision, .. } if decision == "deny"
        ));
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
                let labels: Vec<&str> = options.iter().map(|o| o.label.as_str()).collect();
                assert_eq!(labels, ["Keep", "Remove"]);
                assert!(
                    options.iter().all(|o| o.id.is_none()),
                    "an observed dialog can be read here and answered only there"
                );
            }
            other => panic!("expected a question, got {other:?}"),
        }
    }

    #[test]
    fn a_permission_request_is_answered_elsewhere_and_recorded_there() {
        // It goes to `/vibeplane/policy`, which is the endpoint that replies.
        // Recording it here too would double every blocked signal.
        let out = to_events(&payload(json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf node_modules"}
        })));
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

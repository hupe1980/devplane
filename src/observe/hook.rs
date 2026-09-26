//! Claude Code hooks — the lifecycle channel.
//!
//! The only documented way to learn about a session Devplane did not start,
//! on every surface. Each event runs `devplane hook` as a `command` hook: the
//! payload arrives on stdin and the process records it (see `crate::record`).
//! `PreToolUse` and `PermissionRequest` answer on stdout first and write
//! second; the rest answer `{}`. This module is the reading and the replies.
//!
//! `PermissionRequest` is both the policy gate and the instant "blocked"
//! signal (the `permission_prompt` notification is ~6 s later). A call no rule
//! covers is handed back for Claude Code to prompt; one a project leaves to a
//! person is held for as long as that project says.
//!
//! `WorktreeCreate` is never installed: it replaces Claude Code's own
//! `git worktree` logic machine-wide. Worktrees are learned from `CwdChanged`,
//! the status line and the roster.
use crate::core::event::{Choice, Event, PermissionContext, WaitingFor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// The common fields every hook payload carries, plus the ones we read.
#[derive(Debug, Clone, Deserialize)]
pub struct HookPayload {
    pub hook_event_name: String,
    pub session_id: String,
    #[serde(default, deserialize_with = "crate::observe::canonical_cwd")]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<Value>,
    #[serde(default)]
    pub tool_response: Option<Value>,
    /// `PostToolUse` / `PostToolUseFailure`: how long the tool itself ran,
    /// excluding time in permission prompts and `PreToolUse` hooks.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// What the vendor's permission system had worked out when a
    /// `PermissionRequest` fired. Undocumented, so optional and carried
    /// verbatim rather than interpreted.
    #[serde(default)]
    pub permission_context: Option<PermissionContext>,
    /// `Notification` only: which notification type fired.
    #[serde(default)]
    pub notification_type: Option<String>,
    /// `Elicitation` / `ElicitationResult`: which MCP server is asking.
    #[serde(default)]
    pub mcp_server_name: Option<String>,
    /// Where the MCP server behind this tool call came from: a `name` and a
    /// `source` (`plugin`, `sdk`, or a config scope such as `user`), on the
    /// tool-call events since Claude Code v2.1.274.
    ///
    /// Recorded for the ledger, never read by a verdict: a server a cloned
    /// repository defined otherwise reads like one the person installed.
    #[serde(default)]
    pub mcp_server: Option<McpServer>,
    #[serde(default)]
    pub message: Option<String>,
    /// `SessionStart` / `SessionEnd`.
    #[serde(default)]
    pub source: Option<String>,
    /// Devplane's own field: `CLAUDE_AFK_TIMEOUT_MS` from the session's
    /// environment, injected by `devplane hook`, which inherits it as the
    /// session's child (the host does not). Prefixed to avoid vendor fields.
    #[serde(default)]
    pub devplane_afk_timeout_ms: Option<String>,
    /// `SessionEnd`: why. `PermissionDenied`: the classifier's own words.
    #[serde(default)]
    pub reason: Option<String>,
    /// Present on every hook fired inside a subagent (under the parent's
    /// session id), which tells its tool calls from the main thread's.
    /// `SubagentStart` / `SubagentStop` name the subagent with it.
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub agent_type: Option<String>,
    /// `StopFailure`: the error type, its details, and the rendered error
    /// text as the conversation showed it.
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_details: Option<String>,
    #[serde(default)]
    pub last_assistant_message: Option<String>,
    /// `Stop`: what the session left running. Empty when nothing is in
    /// flight; absent means not checked.
    #[serde(default)]
    pub background_tasks: Option<Vec<Value>>,
    /// `CwdChanged`: where the session went. `cwd` carries the same value and
    /// is the fallback.
    #[serde(default)]
    pub new_cwd: Option<PathBuf>,
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
    /// `PreModelSwitch`: the model the session is about to move to, which
    /// changes the context gauge's denominator.
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

/// The MCP server behind a tool call, as the vendor reports it.
///
/// `source` stays a string: mapping an unknown value to a default would
/// replace a fact with a guess.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct McpServer {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// What a hook payload means: the events to append.
///
/// Events only. The two deciding hooks are answered by whoever holds the
/// payload (`permission_reply`, `pre_tool_use_reply`), recorded beside these.
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
/// Unknown events yield nothing rather than an error, so a vendor upgrade
/// cannot break the observer.
pub fn to_events(p: &HookPayload) -> HookOutcome {
    let mut out = to_events_inner(p);
    // The mode rides on whatever payload carries `permission_mode`; notably
    // `PreToolUse` does not. Appended after the event, never instead of it.
    if let Some(raw) = p.permission_mode.as_deref()
        && !raw.is_empty()
    {
        out.events.push(Event::PermissionModeSeen {
            mode: raw.to_string(),
        });
    }
    out
}

fn to_events_inner(p: &HookPayload) -> HookOutcome {
    let cwd = p.cwd.clone().unwrap_or_else(|| PathBuf::from("."));
    match p.hook_event_name.as_str() {
        "SessionStart" => HookOutcome::just(Event::SessionStarted {
            cwd,
            source: p.source.clone(),
            model: p.model_name(),
            entrypoint: None,
            // Read here or never: only a process that inherited the session's
            // environment can see it.
            question_clock: p
                .devplane_afk_timeout_ms
                .as_deref()
                .and_then(crate::core::clock::from_environment),
            clock_read: true,
        }),

        "UserPromptSubmit" => HookOutcome::just(Event::PromptSubmitted {
            // Characters, not bytes; the prompt itself is never stored.
            chars: p.prompt.as_deref().map(|s| s.chars().count()).unwrap_or(0),
        }),

        "PreToolUse" => {
            let tool = p.tool_name.clone().unwrap_or_default();
            // `AskUserQuestion` is the one tool call whose arrival means a
            // human is needed; reading it here is instant.
            if tool == "AskUserQuestion" {
                let (question, options) = parse_ask_user_question(p.tool_input.as_ref());
                // An observed session's dialog belongs to the provider: no
                // `request_id`, so nothing offers to answer it from here.
                return HookOutcome::just(Event::QuestionAsked {
                    question,
                    options,
                    ask: None,
                    request_id: None,
                    form: None,
                });
            }
            HookOutcome::just(Event::ToolStarted {
                tool,
                input: p.tool_input.clone().unwrap_or(Value::Null),
                server_source: p
                    .mcp_server
                    .as_ref()
                    .and_then(|m| m.source.clone())
                    .filter(|s| !s.is_empty()),
                // Under the parent's session id: only the main thread moving
                // on ends a question the main thread asked.
                agent_id: p.agent_id.clone().filter(|a| !a.is_empty()),
                call_id: None,
            })
        }

        "PostToolUse" => HookOutcome::just(Event::ToolFinished {
            tool: p.tool_name.clone().unwrap_or_default(),
            ok: true,
            duration_ms: p.duration_ms,
            call_id: None,
        }),

        "PostToolUseFailure" => HookOutcome::just(Event::ToolFinished {
            tool: p.tool_name.clone().unwrap_or_default(),
            ok: false,
            duration_ms: p.duration_ms,
            call_id: None,
        }),

        // The instant blocked signal. The process holding it answers and
        // records the verdict with its block, so nothing is produced here.
        "PermissionRequest" => HookOutcome::none(),

        // Auto mode refusing a call by itself, with the classifier's reason.
        "PermissionDenied" => HookOutcome::just(Event::PermissionDecided {
            tool: p.tool_name.clone().unwrap_or_default(),
            decision: "deny".into(),
            by: "auto mode".into(),
            reason: p.reason.clone().filter(|r| !r.is_empty()),
            context: p.permission_context.clone(),
            call_id: None,
        }),

        "Notification" => match p.notification_type.as_deref() {
            // A late backstop, and the only signal for a sandboxed command's
            // network request, which `PermissionRequest` does not fire for.
            Some("permission_prompt") => HookOutcome::just(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: p.message.clone(),
                // Claude Code's own dialog: shown, never answered from here.
                request_id: None,
                ask: None,
                options: Vec::new(),
                call: None,
                context: None,
            }),
            Some("elicitation_dialog") | Some("elicitation_url_dialog") => {
                HookOutcome::just(Event::Blocked {
                    waiting_for: WaitingFor::Question,
                    message: p.message.clone(),
                    request_id: None,
                    ask: None,
                    options: Vec::new(),
                    call: None,
                    context: None,
                })
            }
            Some("idle_prompt") => HookOutcome::just(Event::Blocked {
                waiting_for: WaitingFor::Idle,
                message: None,
                request_id: None,
                ask: None,
                options: Vec::new(),
                call: None,
                context: None,
            }),
            _ => HookOutcome::none(),
        },

        // The turn, and what the session says it left running; absent means
        // nobody checked.
        "Stop" => {
            let mut events = vec![Event::TurnEnded];
            if let Some(tasks) = &p.background_tasks {
                events.push(Event::JobsSeen {
                    running: tasks.len().min(u32::MAX as usize) as u32,
                });
            }
            HookOutcome { events }
        }

        // Prefer the rendered error a person saw, then the vendor's details,
        // then the type — never the generic sentence when there is more.
        "StopFailure" => HookOutcome::just(Event::TurnFailed {
            message: p
                .last_assistant_message
                .clone()
                .or_else(|| p.error_details.clone())
                .or_else(|| p.error.clone())
                .filter(|m| !m.is_empty())
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
            // `new_cwd` is documented; `cwd` is the fallback.
            let cwd = p.new_cwd.clone().unwrap_or(cwd);
            // Entering a worktree is how worktrees are detected without
            // replacing Claude's own worktree machinery.
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

        // After, not before: the gauge only drops once the window is rewritten.
        "PostCompact" => HookOutcome::just(Event::Compacted),
        // Recognised so a settings file that still registers it stays silent.
        "PreCompact" => HookOutcome::none(),

        // The context window changes with the model. `to_model`, as on
        // `PreModelSwitch`; `model` is only on `SessionStart`.
        "PostModelSwitch" => match p.to_model.clone().or_else(|| p.model_name()) {
            Some(model) => HookOutcome::just(Event::ModelChanged { model }),
            None => HookOutcome::none(),
        },

        // An MCP server asking the user, reported at once.
        "Elicitation" => HookOutcome::just(Event::Blocked {
            waiting_for: WaitingFor::Question,
            message: p
                .message
                .clone()
                .or_else(|| p.mcp_server_name.clone().map(|s| format!("{s} is asking"))),
            // The dialog belongs to Claude Code: Devplane can show it and
            // raise its window, never answer it, so it carries no ask.
            request_id: None,
            ask: None,
            options: Vec::new(),
            call: None,
            context: None,
        }),

        // Answered wherever it was answered; without this an elicitation
        // resolved in a terminal would sit in the inbox.
        "ElicitationResult" => HookOutcome::just(Event::QuestionAnswered {
            action: p.action.clone().unwrap_or_else(|| "accept".into()),
        }),

        // A settings file Devplane writes hooks into changed. Managed policy
        // can block loopback hooks, and the only other symptom is silence.
        "ConfigChange" => HookOutcome::just(Event::ConfigChanged {
            source: p.source.clone().unwrap_or_else(|| "settings".into()),
            path: p.file_path.clone(),
        }),

        // The agent's own task list, for a session Devplane only watches.
        "TaskCreated" | "TaskCompleted" => match (&p.task_id, &p.task_subject) {
            (Some(id), Some(subject)) => HookOutcome::just(Event::TaskChanged {
                id: id.clone(),
                subject: subject.clone(),
                done: p.hook_event_name == "TaskCompleted",
            }),
            _ => HookOutcome::none(),
        },

        // The gauge's denominator is about to change; this arrives before the
        // next turn is priced against it.
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
/// The same test the board and the policy use: any `git worktree add`
/// checkout, including Claude Code's `.claude/worktrees/`.
fn is_worktree_path(p: &std::path::Path) -> bool {
    crate::repo::is_worktree(p)
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
/// Empty means "no decision": Claude Code shows its own dialog, so the hook
/// answers in a millisecond for a session Devplane only observes.
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

    /// Carries a person's allow back to the vendor: the selection they made
    /// on a held permission, after a recorded answer — never a verdict.
    pub fn carrying_a_persons_allow() -> Self {
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
/// Not interchangeable with [`PermissionResponse`]: this fires before every
/// tool call in every mode, the only way a prohibition reaches auto mode. It
/// only ever prohibits or asks — an `allow` would skip the permission system,
/// classifier included.
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

/// The vendor's timeout, in seconds, for a hook that only decides: every
/// `PreToolUse`, and every event that is not a hold.
///
/// Generous against a cold disk and still a bound: past it the vendor cancels
/// the hook and prompts the person itself.
pub const GATE_TIMEOUT_SECS: u64 = 5;

/// The vendor's timeout, in seconds, for the event a permission is **held**
/// on — `PermissionRequest`.
///
/// Derived from [`Hold::CEILING`](crate::core::config::Hold::CEILING): a hold
/// the vendor kills part-way is a question that never ends. The margin covers
/// opening the store and the one read after the deadline.
pub const HOLD_TIMEOUT_SECS: u64 =
    crate::core::config::Hold::CEILING.as_secs() + GATE_TIMEOUT_SECS + 25;

/// The session id `devplane doctor` uses when it runs the gate to see whether
/// it answers.
///
/// The probe gets a real verdict but writes no decision row: a diagnostic must
/// not write history.
pub const PROBE_SESSION: &str = "devplane-doctor";

/// What a tool call is called in the decision log: the tool, and as much of its
/// specifier as fits.
///
/// Named by the deciding process; the record writes down what it is told.
pub fn describe_call(tool: &str, input: &serde_json::Value) -> String {
    match crate::core::policy::rule_content(tool, input) {
        Some(c) => format!("{tool}: {}", crate::core::text::clip(&c, 120)),
        None => tool.to_string(),
    }
}

/// Turns a verdict into what a Claude Code `PermissionRequest` hook returns.
///
/// One place, so nothing answering beside the `command` hook can drift. There
/// is no allow arm: Devplane prohibits, defers and reports; it never claims
/// the vendor would have said yes.
pub fn permission_reply(verdict: &crate::core::Verdict) -> PermissionResponse {
    use crate::core::Verdict;
    match verdict {
        Verdict::Deny { rule } => {
            PermissionResponse::deny(format!("denied by Devplane policy rule {rule}"))
        }
        // The dialog is already on its way to a person, which is what `ask`
        // and `unresolved` want, so the reply equals `undecided`. The verdict
        // is still recorded: the three are different facts.
        Verdict::Ask { .. } | Verdict::Unresolved { .. } | Verdict::Undecided => {
            PermissionResponse::undecided()
        }
    }
}

/// Turns a verdict into what a Claude Code `PreToolUse` hook returns.
///
///
/// Only ever a prohibition: an `allow` would skip the permission system,
/// auto-mode classifier included.
pub fn pre_tool_use_reply(verdict: &crate::core::Verdict) -> PreToolUseResponse {
    use crate::core::Verdict;
    match verdict {
        Verdict::Deny { rule } => {
            PreToolUseResponse::deny(format!("denied by Devplane policy rule {rule}"))
        }
        Verdict::Ask { rule } => {
            PreToolUseResponse::ask(format!("{rule} asks that a person decides this"))
        }
        // The one case where Devplane asks without a rule to name: a
        // prohibition exists and this line hides what runs, so `undecided`
        // would hand it to a classifier reading the same unreadable string.
        Verdict::Unresolved { why } => PreToolUseResponse::ask(format!(
            "Devplane cannot tell whether a prohibition covers this: {why}"
        )),
        Verdict::Undecided => PreToolUseResponse::undecided(),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn session_start(extra: serde_json::Value) -> Vec<crate::core::Event> {
        let mut body = serde_json::json!({
            "hook_event_name": "SessionStart",
            "session_id": "s1",
            "cwd": "/tmp/repo",
            "source": "startup",
        });
        let obj = body.as_object_mut().expect("an object");
        for (k, v) in extra.as_object().expect("an object") {
            obj.insert(k.clone(), v.clone());
        }
        let p: HookPayload = serde_json::from_value(body).expect("parses");
        to_events(&p).events
    }

    fn clock_of(
        events: &[crate::core::Event],
    ) -> (Option<crate::core::clock::QuestionClock>, bool) {
        match events.first() {
            Some(crate::core::Event::SessionStarted {
                question_clock,
                clock_read,
                ..
            }) => (question_clock.clone(), *clock_read),
            other => panic!("expected a SessionStarted, got {other:?}"),
        }
    }

    /// A session whose environment carries a timer, on a machine whose
    /// settings say `never`, reports the session's timer.
    #[test]
    fn a_session_reports_the_timer_its_own_environment_put_on_it() {
        let (clock, read) = clock_of(&session_start(
            serde_json::json!({"devplane_afk_timeout_ms": "60000"}),
        ));
        let c = clock.expect("the session carries its own clock");
        assert!(read, "the environment was read");
        assert_eq!(c.source, crate::core::clock::Source::Environment);
        assert_eq!(c.after.says(), "60s", "the vendor's own spelling");
        assert_eq!(c.where_set, crate::core::clock::ENV_KEY);
        assert!(
            c.says().contains("overrides your settings"),
            "a person has to be told it beats the file they set: {}",
            c.says()
        );
    }

    /// Zero ends every question in that session the instant it is asked.
    #[test]
    fn zero_is_reported_as_closing_immediately() {
        let (clock, _) = clock_of(&session_start(
            serde_json::json!({"devplane_afk_timeout_ms": "0"}),
        ));
        let c = clock.expect("a clock");
        assert!(c.is_immediate());
        assert!(!c.says().contains("0s"), "{}", c.says());
    }

    /// Read, and nothing was set — a different row from not read.
    #[test]
    fn a_session_with_no_such_variable_is_read_and_empty() {
        let (clock, read) = clock_of(&session_start(serde_json::json!({})));
        assert_eq!(clock, None);
        assert!(
            read,
            "a SessionStart that reached this process is one whose environment was read"
        );
    }

    /// A value the vendor decides the meaning of is not one this product
    /// invents a reading for.
    #[test]
    fn a_timeout_that_is_not_a_number_is_ignored_rather_than_guessed() {
        for bad in ["", "soon", "5m", "-1"] {
            let (clock, read) = clock_of(&session_start(
                serde_json::json!({"devplane_afk_timeout_ms": bad}),
            ));
            assert_eq!(clock, None, "{bad}");
            assert!(read, "{bad}");
        }
    }

    /// The mode rides on whatever carried it, and `PreToolUse` does not.
    #[test]
    fn the_mode_is_read_from_any_payload_that_has_it_and_invented_for_none() {
        let carrying = |event: &str, mode: &str| -> Vec<String> {
            let body = serde_json::json!({
                "hook_event_name": event,
                "session_id": "s1",
                "cwd": "/tmp/repo",
                "permission_mode": mode,
                "prompt": "hi",
                "tool_name": "Bash",
                "tool_input": {"command": "ls"},
            });
            let p: HookPayload = serde_json::from_value(body).unwrap();
            to_events(&p)
                .events
                .iter()
                .map(|e| e.label().to_string())
                .collect()
        };

        // A carrier reports the mode *and* whatever else it meant.
        let prompt = carrying("UserPromptSubmit", "auto");
        assert!(
            prompt.contains(&"prompt_submitted".to_string()),
            "{prompt:?}"
        );
        assert!(
            prompt.contains(&"permission_mode_seen".to_string()),
            "{prompt:?}"
        );

        // `PermissionRequest` records nothing of its own and still reports the
        // mode.
        assert_eq!(
            carrying("PermissionRequest", "default"),
            vec!["permission_mode_seen".to_string()]
        );

        // And a payload without the field invents nothing.
        let bare: HookPayload = serde_json::from_value(serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "cwd": "/tmp/repo",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"},
        }))
        .unwrap();
        let kinds: Vec<_> = to_events(&bare)
            .events
            .iter()
            .map(|e| e.label().to_string())
            .collect();
        assert_eq!(
            kinds,
            vec!["tool_started".to_string()],
            "a mode was reported for a hook that does not carry one"
        );
    }

    use serde_json::json;

    fn payload(v: serde_json::Value) -> HookPayload {
        serde_json::from_value(v).expect("payload parses")
    }

    #[test]
    fn an_answered_elicitation_stops_being_asked() {
        // An elicitation answered in the person's own terminal must close the
        // inbox item.
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
        // Otherwise hook removal or a managed policy blocking loopback shows up
        // only as a channel that stopped speaking.
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
        // The gauge is a percentage of the model's window; this arrives before
        // the next turn is priced.
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
        // `PreCompact` fires while the window is still full; resetting there
        // would flicker the gauge and the `context_high` item.
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
        // The vendor sends `from_model` and `to_model` here, never `model`.
        let out = to_events(&payload(json!({
            "hook_event_name": "PostModelSwitch",
            "session_id": "s1",
            "from_model": "claude-sonnet-5",
            "to_model": "claude-opus-5[1m]",
            "source": "command"
        })));
        assert!(
            matches!(&out.events[0], Event::ModelChanged { model } if model.contains("1m")),
            "{:?}",
            out.events
        );
    }

    #[test]
    fn an_mcp_server_asking_a_question_blocks_the_run_at_once() {
        // The `elicitation_dialog` notification arrives ~6 s later and, in a
        // terminal, defers on every keystroke.
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
        // Auto mode denying a call is reported nowhere else, with the
        // classifier's reason; the decider is the mode.
        let out = to_events(&payload(json!({
            "hook_event_name": "PermissionDenied",
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf /tmp/build"},
            "reason": "[Irreversible Local Destruction]"
        })));
        match &out.events[0] {
            Event::PermissionDecided {
                decision,
                by,
                reason,
                ..
            } => {
                assert_eq!(decision, "deny");
                assert_eq!(by, "auto mode");
                assert_eq!(
                    reason.as_deref(),
                    Some("[Irreversible Local Destruction]"),
                    "the classifier's reason is the whole point of the row"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    /// **A subagent's tool call says whose it was.**
    ///
    /// Subagent hooks fire under the parent's session id with `agent_id` set;
    /// without it a subagent's first call would read as the parent moving
    /// past its open question.
    #[test]
    fn a_subagents_tool_call_carries_the_subagent_and_the_main_threads_does_not() {
        let sub = to_events(&payload(json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "cwd": "/repo",
            "agent_id": "agent-abc123",
            "agent_type": "Explore",
            "tool_name": "Grep",
            "tool_input": {"pattern": "login"}
        })));
        match &sub.events[0] {
            Event::ToolStarted { agent_id, .. } => {
                assert_eq!(agent_id.as_deref(), Some("agent-abc123"));
            }
            other => panic!("{other:?}"),
        }
        let main = to_events(&payload(json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}
        })));
        assert!(matches!(
            &main.events[0],
            Event::ToolStarted { agent_id: None, .. }
        ));
    }

    /// The reply can never defer.
    ///
    /// Claude Code falls open when every `PreToolUse` hook defers, so that
    /// word would be an allow no rule wrote. Both reply types choose their word
    /// in a private constructor; this checks them and the source.
    #[test]
    fn a_reply_has_no_way_to_defer() {
        let word = ["de", "fer"].concat();
        for reply in [
            serde_json::to_string(&PermissionResponse::undecided()).unwrap(),
            serde_json::to_string(&PermissionResponse::carrying_a_persons_allow()).unwrap(),
            serde_json::to_string(&PermissionResponse::deny("x")).unwrap(),
            serde_json::to_string(&PreToolUseResponse::undecided()).unwrap(),
            serde_json::to_string(&PreToolUseResponse::deny("x")).unwrap(),
            serde_json::to_string(&PreToolUseResponse::ask("x")).unwrap(),
        ] {
            assert!(!reply.contains(&word), "{reply}");
        }
        // No quoted literal of the word in this file: the words are set only
        // inside `decide`.
        let quoted = format!("\"{word}\"");
        assert!(
            !include_str!("hook.rs").contains(&quoted),
            "a reply constructor can spell {quoted}"
        );
        assert!(
            !include_str!("copilot.rs").contains(&quoted),
            "a Copilot reply constructor can spell {quoted}"
        );
    }

    /// **The vendor's permission context rides on the payload, verbatim.**
    #[test]
    fn a_permission_context_is_parsed_and_an_absent_one_costs_nothing() {
        let p = payload(json!({
            "hook_event_name": "PermissionRequest",
            "session_id": "s1",
            "tool_name": "Bash",
            "tool_input": {"command": "git push"},
            "permission_context": {
                "classifier_verdict": "ask",
                "classifier_confidence": 0.62,
                "permission_rule_matched": "Bash(git push *)",
                "agent_id": "agent-1",
                "agent_type": "Explore",
                "something_new": true
            }
        }));
        let ctx = p.permission_context.clone().expect("parsed");
        assert_eq!(ctx.classifier_verdict.as_deref(), Some("ask"));
        assert_eq!(ctx.classifier_confidence, Some(0.62));
        assert_eq!(
            ctx.permission_rule_matched.as_deref(),
            Some("Bash(git push *)")
        );
        assert_eq!(ctx.agent_type.as_deref(), Some("Explore"));
        assert!(
            payload(json!({"hook_event_name": "PermissionRequest", "session_id": "s1"}))
                .permission_context
                .is_none()
        );
    }

    /// The fields the vendor documents, read under the names it documents.
    #[test]
    fn documented_field_names_are_the_ones_read() {
        // `PostToolUse` carries the tool's own duration.
        match &to_events(&payload(json!({
            "hook_event_name": "PostToolUse",
            "session_id": "s1",
            "tool_name": "Bash",
            "duration_ms": 4187
        })))
        .events[0]
        {
            Event::ToolFinished { duration_ms, .. } => assert_eq!(*duration_ms, Some(4187)),
            other => panic!("{other:?}"),
        }

        // `CwdChanged` documents `new_cwd`.
        match &to_events(&payload(json!({
            "hook_event_name": "CwdChanged",
            "session_id": "s1",
            "cwd": "/old",
            "old_cwd": "/old",
            "new_cwd": "/new"
        })))
        .events[0]
        {
            Event::CwdChanged { cwd } => assert_eq!(cwd, std::path::Path::new("/new")),
            other => panic!("{other:?}"),
        }

        // `StopFailure` sends `error`, `error_details` and the rendered text.
        match &to_events(&payload(json!({
            "hook_event_name": "StopFailure",
            "session_id": "s1",
            "error": "rate_limit",
            "error_details": "429 Too Many Requests",
            "last_assistant_message": "API Error: Rate limit reached"
        })))
        .events[0]
        {
            Event::TurnFailed { message } => assert_eq!(message, "API Error: Rate limit reached"),
            other => panic!("{other:?}"),
        }
        match &to_events(&payload(json!({
            "hook_event_name": "StopFailure",
            "session_id": "s1",
            "error": "overloaded"
        })))
        .events[0]
        {
            Event::TurnFailed { message } => assert_eq!(message, "overloaded"),
            other => panic!("{other:?}"),
        }

        // `Stop` says what the session left running, and absent is not zero.
        let with = to_events(&payload(json!({
            "hook_event_name": "Stop",
            "session_id": "s1",
            "background_tasks": [{"id": "t1", "type": "shell", "status": "running"}]
        })))
        .events;
        assert!(matches!(with[0], Event::TurnEnded));
        assert!(matches!(with[1], Event::JobsSeen { running: 1 }));
        let without = to_events(&payload(json!({
            "hook_event_name": "Stop",
            "session_id": "s1"
        })))
        .events;
        assert_eq!(
            without.len(),
            1,
            "a payload that did not report tasks was given a count"
        );
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
        // A notification costs six seconds and, in a terminal, may never come.
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
            Event::QuestionAsked {
                question, options, ..
            } => {
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
        // The process holding it records the verdict and block; recording
        // here too would double every blocked signal.
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
        // agents` is open, so they are not state.
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
        let v = serde_json::to_value(PermissionResponse::carrying_a_persons_allow()).unwrap();
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

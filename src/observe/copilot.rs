//! GitHub Copilot — hooks, and the tool vocabulary they arrive in.
//!
//! The other two channels live elsewhere: the ACP server is a line in
//! `acp::builtin()`, and the telemetry is a dialect [`super::otel`] reads.
//!
//! **Three things differ from Claude Code, and each decides something here.**
//!
//! *Only a `command` hook fails closed.* An HTTP `preToolUse` hook falls
//! through to the default permission flow on any error, so a prohibition sent
//! that way evaporates under load. The deciding events ride `devplane hook`;
//! the informational ones stay on HTTP, where a process per tool call would
//! cost something for nothing. (Loopback HTTP is available with
//! `COPILOT_HOOK_ALLOW_LOCALHOST=1`, so the reason is the failure mode rather
//! than the transport.)
//!
//! *A hook timeout is fail-open on every event*, administrator policy hooks
//! included. A slow Devplane here is not one that blocks the session; it is
//! one that was not consulted.
//!
//! *The tool vocabulary is Copilot's own* — `bash`, `view`, `create` — mapped
//! below to the names the policy already speaks. That mapping is the only
//! translation in this module and deliberately the only one: rules are never
//! rewritten into Copilot's `--allow-tool` vocabulary, because a rule written
//! into somebody else's configuration cannot be read back by `devplane audit`.

use crate::core::event::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Copilot's runtime tool names, mapped to the names the policy already uses.
///
/// `powershell` maps to `PowerShell` and **not** to `Bash`, which is what it
/// used to do on the strength of the vendor's own table putting both under one
/// heading. The table is about which shell Copilot ran; the policy engine's
/// name decides which *language* the command is parsed as, and
/// `Get-Content .env` read by a POSIX parser is a confident wrong answer in
/// both directions — no `Read(.env)` deny reaches it, and a `Bash(...)` allow
/// rule speaks for a line it cannot parse. Claude Code documents `PowerShell`
/// as a tool with its own rule syntax, so the honest mapping already exists.
const TOOL_NAMES: &[(&str, &str)] = &[
    ("bash", "Bash"),
    ("powershell", "PowerShell"),
    ("pwsh", "PowerShell"),
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
    /// Which kind of notification, for the `notification` event.
    ///
    /// The same vocabulary Claude Code uses — `permission_prompt`,
    /// `idle_prompt` — which is why the mapping below is the same mapping.
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
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

/// What one Copilot hook payload means, in Devplane's own event model.
///
/// **Copilot documents fourteen events and this maps twelve of them.** The
/// shape is close to Claude Code's on purpose — the two vendors publish the
/// same notification vocabulary — so the differences below are real
/// differences rather than translation.
///
/// What Copilot does not have: a `PostCompact`, a model switch, an elicitation
/// pair, and a roster. The absence of a roster is the one that costs something,
/// and it is why a session Devplane has never received a hook for cannot be
/// discovered at all.
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
            // **Not read, which is not the same as nothing set.** The question
            // timer is Claude Code's, and this payload arrives over HTTP from a
            // process Devplane did not spawn, so there is no environment to
            // read. The surface says *not read* for this session rather than
            // implying its questions wait.
            question_clock: None,
            clock_read: false,
        }],
        "userPromptSubmitted" => vec![Event::PromptSubmitted {
            chars: p.prompt.as_ref().map(|t| t.chars().count()).unwrap_or(0),
        }],
        // Copilot documents no equivalent of the vendor's `mcp_server`, so a
        // row from it says nothing about provenance rather than saying it is
        // unknown.
        "preToolUse" => vec![Event::tool_started(p.tool(), p.input())],
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
        // **A person is being asked, right now.**
        //
        // The same `notification_type` vocabulary Claude Code publishes, which
        // is why this is the same mapping rather than an adapter. It was
        // missing until 2026-09-21: eleven events were mapped and this one —
        // the only one that says *somebody is waiting on you*, which is the
        // product — was not. A vendor's channel being read is not the same as
        // its channel being read for the thing the product is about.
        "notification" => match p.notification_type.as_deref() {
            Some("permission_prompt") => vec![Event::Blocked {
                waiting_for: crate::core::WaitingFor::Permission,
                message: p.message.clone(),
                // An observed session's dialog is Copilot's own: Devplane can
                // show that it is there, never answer it.
                request_id: None,
                ask: None,
                options: Vec::new(),
                call: None,
            }],
            Some("idle_prompt") => vec![Event::Blocked {
                waiting_for: crate::core::WaitingFor::Idle,
                message: None,
                request_id: None,
                ask: None,
                options: Vec::new(),
                call: None,
            }],
            _ => Vec::new(),
        },

        // **Deliberately no event, exactly as Claude Code's `PermissionRequest`
        // produces none.**
        //
        // It fires *before* the permission service runs — before the rules
        // engine, session approvals, auto-allow and auto-deny — so it says a
        // decision is about to be taken and not what it was. Recording it as a
        // block would mark every auto-allowed call as waiting on a person, and
        // that error runs in the alarming direction on a surface whose whole
        // job is to be trusted about what is waiting.
        //
        // What it is for is the **gate**, which rides `preToolUse` here for the
        // reason documented at the top of this file.
        "permissionRequest" => Vec::new(),

        // `preCompact` fires while the window is still full, so there is
        // nothing to record yet — and Copilot publishes no `postCompact` to
        // pair it with, so unlike Claude Code the compaction itself is never
        // observed. A row claiming a reset that may not have happened is worse
        // than no row.
        "preCompact" => Vec::new(),

        // The prompt after Copilot's own rewriting. `userPromptSubmitted`
        // already counted the characters a person typed, which is the figure
        // the surface is about; counting both would double every prompt.
        "userPromptTransformed" => Vec::new(),

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
pub const HOOKS_FILE: &str = "devplane.json";

/// The hook registration: one file, entirely Devplane's.
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
                "url": format!("{base_url}/devplane/copilot/hook?event={event}"),
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
            [
                Event::ToolStarted {
                    tool,
                    input,
                    server_source,
                },
            ] => {
                assert_eq!(tool, "Bash");
                assert_eq!(input["command"], json!("pnpm test"));
                // Copilot documents no equivalent, so a row from it says
                // nothing about provenance rather than saying it is unknown.
                assert_eq!(*server_source, None);
            }
            other => panic!("expected a started tool, got {other:?}"),
        }
    }

    /// **A field Copilot does not report is absent, never zero and never Claude
    /// Code's meaning.**
    ///
    /// This is the shape of every cross-vendor mistake available here: a
    /// missing figure rendered as `0` reads as *measured, and it was none*,
    /// which is the reassuring answer over no evidence. Three fields, three
    /// reasons, and all three are `None` rather than a default.
    #[test]
    fn a_field_copilot_does_not_report_is_absent_rather_than_zero() {
        let payload: HookPayload = serde_json::from_value(serde_json::json!({
            "event": "sessionStart",
            "cwd": "/repo",
        }))
        .expect("a session start with nothing else in it");

        let events = to_events(&payload);
        let [
            Event::SessionStarted {
                model,
                question_clock,
                clock_read,
                entrypoint,
                ..
            },
        ] = events.as_slice()
        else {
            panic!("expected one session start, got {events:?}");
        };

        // **The model.** Copilot's hook payload carries none, and a blank model
        // is not the same claim as a model nobody asked about.
        assert_eq!(
            *model, None,
            "a model Copilot did not report must be absent"
        );

        // **The question clock.** There is no environment to read: the payload
        // arrives over HTTP from a process Devplane did not spawn. `None` with
        // `clock_read: false` is *not read*; `None` with `clock_read: true`
        // would be *nothing is set*, which is the sentence meaning **your
        // questions wait for you** — and it would be false here.
        assert_eq!(*question_clock, None);
        assert!(
            !*clock_read,
            "an unread environment may never render as one that was read and found empty"
        );

        // **The entrypoint**, by contrast, is genuinely known — from the
        // channel the payload arrived on — so it is present. Absent and known
        // are different answers and this asserts the difference rather than
        // that everything is missing.
        assert_eq!(entrypoint.as_deref(), Some("copilot"));
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
    fn payload(event: &str, v: Value) -> HookPayload {
        let mut o = v.as_object().cloned().unwrap_or_default();
        o.insert("event".into(), json!(event));
        serde_json::from_value(Value::Object(o)).expect("a payload this reader can parse")
    }

    /// **Somebody is being asked, and the board can say so.**
    ///
    /// This is the event the product is about, and it was unmapped until
    /// 2026-09-21 while eleven of its neighbours were not. Copilot publishes
    /// the same `notification_type` vocabulary Claude Code does, so this is the
    /// same mapping rather than an adapter — and reading a vendor's channel is
    /// not the same as reading it for the thing the product exists to show.
    #[test]
    fn a_copilot_session_waiting_on_a_person_is_a_block_the_board_can_render() {
        let out = to_events(&payload(
            "notification",
            json!({ "notificationType": "permission_prompt", "message": "Run `rm -rf build`?" }),
        ));
        match out.as_slice() {
            [
                Event::Blocked {
                    waiting_for,
                    message,
                    request_id,
                    ..
                },
            ] => {
                assert_eq!(*waiting_for, crate::core::WaitingFor::Permission);
                assert_eq!(message.as_deref(), Some("Run `rm -rf build`?"));
                // **Never answerable from here.** An observed session's dialog
                // belongs to Copilot; Devplane can show that it is there and
                // must not imply it can reply.
                assert!(
                    request_id.is_none(),
                    "an observed dialog was made to look answerable"
                );
            }
            other => panic!("a permission prompt did not become a block: {other:?}"),
        }

        let idle = to_events(&payload(
            "notification",
            json!({ "notificationType": "idle_prompt" }),
        ));
        assert!(matches!(
            idle.as_slice(),
            [Event::Blocked {
                waiting_for: crate::core::WaitingFor::Idle,
                ..
            }]
        ));

        // A notification kind this map does not know says nothing, rather than
        // guessing at a state.
        assert!(
            to_events(&payload(
                "notification",
                json!({ "notificationType": "shell_completed" })
            ))
            .is_empty()
        );
    }

    /// **`permissionRequest` fires before anything has been decided.**
    ///
    /// It runs ahead of the rules engine, session approvals, auto-allow and
    /// auto-deny — so it says a decision is *about to be taken*, not what it
    /// was. Recording it as a block would mark every auto-allowed call as
    /// waiting on a person, and that error runs in the alarming direction on
    /// the one surface whose job is to be trusted about what is waiting.
    #[test]
    fn a_permission_about_to_be_decided_is_not_a_person_waiting() {
        assert!(
            to_events(&payload(
                "permissionRequest",
                json!({ "toolName": "bash", "toolArgs": { "command": "ls" } }),
            ))
            .is_empty(),
            "a permission the vendor may auto-allow was recorded as somebody waiting"
        );
    }

    /// **Every event Copilot documents is either mapped or silent on purpose.**
    ///
    /// The list is the vendor's, read from its hooks reference on 2026-09-21.
    /// The failure this catches is the one that already happened: eleven of
    /// fourteen mapped, the gap unnoticed, and the missing one was the event
    /// the product is named for. A vendor adding an event is normal; nobody
    /// noticing is the defect.
    #[test]
    fn every_event_the_vendor_documents_has_been_decided_about() {
        // Mapped: it produces at least one event.
        const MAPPED: &[&str] = &[
            "sessionStart",
            "sessionEnd",
            "userPromptSubmitted",
            "preToolUse",
            "postToolUse",
            "postToolUseFailure",
            "agentStop",
            "errorOccurred",
            "subagentStart",
            "subagentStop",
            "notification",
        ];
        // Silent by decision, each with its reason in `to_events`.
        const SILENT: &[&str] = &["permissionRequest", "preCompact", "userPromptTransformed"];

        for e in MAPPED {
            let v = match *e {
                "notification" => json!({ "notificationType": "idle_prompt" }),
                _ => json!({ "toolName": "bash", "agentName": "a", "prompt": "hi" }),
            };
            assert!(
                !to_events(&payload(e, v)).is_empty(),
                "`{e}` is listed as mapped and produces no event"
            );
        }
        for e in SILENT {
            assert!(
                to_events(&payload(e, json!({ "toolName": "bash" }))).is_empty(),
                "`{e}` is listed as silent by decision and produces an event"
            );
        }
        assert_eq!(
            MAPPED.len() + SILENT.len(),
            14,
            "Copilot documents fourteen hook events and this ledger accounts for {}. \
             Read the vendor's hooks reference and decide about the difference — a vendor \
             adding an event is normal, and nobody noticing is the defect this catches.",
            MAPPED.len() + SILENT.len()
        );
    }
}

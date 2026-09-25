//! GitHub Copilot — hooks, and the tool vocabulary they arrive in.
//!
//! The ACP server is a line in `acp::builtin()` and the telemetry a dialect
//! [`super::otel`] reads.
//!
//! Every hook runs the binary with `exec` and `args` (no shell), payload on
//! stdin, like Claude Code's. HTTP is not an option: loopback is refused
//! unless `COPILOT_HOOK_ALLOW_LOCALHOST=1`, which `connect` cannot set, and an
//! HTTP `preToolUse` fails open. A hook timeout is fail-open on every event,
//! so a slow Devplane is simply not consulted.
//!
//! Copilot's tool names (`bash`, `view`, `create`) are mapped to the policy's
//! — the only translation here. Rules are never written into Copilot's
//! `--allow-tool` config, which `devplane audit` could not read back.

use crate::core::event::{Event, EventEnvelope, Source};
use crate::core::{RunId, World};
use crate::store::Store;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Copilot's runtime tool names, mapped to the names the policy already uses.
///
/// `powershell` maps to `PowerShell`, not `Bash`: the policy name decides
/// which language parses the command, and a POSIX parser reading
/// `Get-Content .env` is confidently wrong both ways.
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
/// equivalent keeps its own name.
pub fn tool_name(runtime: &str) -> String {
    TOOL_NAMES
        .iter()
        .find(|(from, _)| *from == runtime)
        .map(|(_, to)| (*to).to_string())
        .unwrap_or_else(|| runtime.to_string())
}

/// One hook payload, in Copilot's native (camelCase) shape.
///
/// Not the Claude-compatible PascalCase aliases: that form documents input
/// field names, not the output a `preToolUse` hook must return.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookPayload {
    #[serde(default)]
    pub session_id: String,
    #[serde(default, deserialize_with = "crate::observe::canonical_cwd")]
    pub cwd: Option<std::path::PathBuf>,
    /// The event, from the argument the hook was registered with: native
    /// payloads do not name themselves.
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_args: Option<Value>,
    #[serde(default)]
    pub prompt: Option<String>,
    /// `sessionStart`: `startup`, `resume` or `new`. `sessionEnd` documents
    /// `reason` instead.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    /// A string on `postToolUseFailure`; an object with `message` and `name`
    /// on `errorOccurred`. Read as either.
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub agent_name: Option<String>,
    /// Which kind of notification, for the `notification` event.
    ///
    /// Claude Code's vocabulary (`permission_prompt`, `elicitation_dialog`),
    /// and snake_case even on the camelCase payload.
    #[serde(default, rename = "notification_type", alias = "notificationType")]
    pub notification_type: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl HookPayload {
    /// The tool input; the policy reads its content field (`command`,
    /// `file_path`, `url`).
    pub fn input(&self) -> Value {
        self.readable_input()
            .or_else(|| self.tool_args.clone())
            .unwrap_or_else(|| json!({}))
    }

    /// The call's arguments as an object. `toolArgs` may be an object or a
    /// JSON string of one; `None` otherwise, which the gate treats as
    /// unreadable.
    pub fn readable_input(&self) -> Option<Value> {
        match &self.tool_args {
            None => Some(json!({})),
            Some(v @ Value::Object(_)) => Some(v.clone()),
            Some(Value::String(s)) => serde_json::from_str::<Value>(s)
                .ok()
                .filter(Value::is_object),
            Some(_) => None,
        }
    }

    /// The name the policy engine knows this call's tool by.
    pub fn tool(&self) -> String {
        self.tool_name.as_deref().map(tool_name).unwrap_or_default()
    }

    /// The error as a sentence, whichever shape it arrived in.
    fn error_message(&self) -> Option<String> {
        match &self.error {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            Some(Value::Object(o)) => o
                .get("message")
                .and_then(|m| m.as_str())
                .filter(|m| !m.is_empty())
                .map(str::to_string),
            _ => None,
        }
    }
}

/// What this reader has decided about one documented event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ledger {
    /// Registered, and `to_events` produces something for it.
    Mapped,
    /// Deliberately not registered; the reason is at its arm in `to_events`.
    Silent,
}

/// The one ledger of Copilot's documented hook events; every count is
/// computed from it, and a test compares it to the vendor reference. All are
/// `command` hooks; only the gate's (`preToolUse`) stdout is read.
pub const EVENTS: &[(&str, Ledger)] = &[
    ("sessionStart", Ledger::Mapped),
    ("sessionEnd", Ledger::Mapped),
    ("userPromptSubmitted", Ledger::Mapped),
    ("userPromptTransformed", Ledger::Silent),
    ("preToolUse", Ledger::Mapped),
    ("postToolUse", Ledger::Mapped),
    ("postToolUseFailure", Ledger::Mapped),
    ("permissionRequest", Ledger::Silent),
    ("agentStop", Ledger::Mapped),
    ("subagentStart", Ledger::Mapped),
    ("subagentStop", Ledger::Mapped),
    ("errorOccurred", Ledger::Mapped),
    ("preCompact", Ledger::Silent),
    ("notification", Ledger::Mapped),
];

/// The event that decides. Every other one only reports.
pub const GATE_EVENT: &str = "preToolUse";

/// What one Copilot hook payload means, in Devplane's own event model.
///
/// Close to Claude Code's shape, since both publish the same notification
/// vocabulary. Copilot has no `PostCompact`, model switch, elicitation pair or
/// roster — so a session that never sent a hook cannot be discovered.
pub fn to_events(p: &HookPayload) -> Vec<Event> {
    match p.event.as_str() {
        "sessionStart" => vec![Event::SessionStarted {
            cwd: p.cwd.clone().unwrap_or_default(),
            source: p.source.clone().or_else(|| Some("copilot".into())),
            model: None,
            // Copilot publishes no entrypoint, so the vendor is the answer.
            entrypoint: Some("copilot".into()),
            // Not read, which differs from nothing set: the question timer
            // is Claude Code's and Copilot has no equivalent.
            question_clock: None,
            clock_read: false,
        }],
        "userPromptSubmitted" => vec![Event::PromptSubmitted {
            chars: p.prompt.as_ref().map(|t| t.chars().count()).unwrap_or(0),
        }],
        // No `mcp_server` or per-call agent id in Copilot's payload.
        "preToolUse" => vec![Event::tool_started(p.tool(), p.input())],
        "postToolUse" => vec![Event::ToolFinished {
            tool: p.tool(),
            ok: true,
            duration_ms: None,
            call_id: None,
        }],
        "postToolUseFailure" => vec![Event::ToolFinished {
            tool: p.tool(),
            ok: false,
            duration_ms: None,
            call_id: None,
        }],
        // A person is being asked, right now — same `notification_type`
        // vocabulary as Claude Code, same mapping.
        "notification" => match p.notification_type.as_deref() {
            Some("permission_prompt") => vec![Event::Blocked {
                waiting_for: crate::core::WaitingFor::Permission,
                message: p.message.clone(),
                // Copilot's own dialog: shown, never answered from here.
                request_id: None,
                ask: None,
                options: Vec::new(),
                call: None,
                context: None,
            }],
            Some("elicitation_dialog") => vec![Event::Blocked {
                waiting_for: crate::core::WaitingFor::Question,
                message: p.message.clone(),
                request_id: None,
                ask: None,
                options: Vec::new(),
                call: None,
                context: None,
            }],
            // `agent_idle` is a *background* agent going idle, and
            // `shell_completed` a job finishing: neither is a person's turn.
            _ => Vec::new(),
        },

        // No event, as with Claude Code's `PermissionRequest`: it fires before
        // the rules engine, approvals and auto-allow/deny, so recording a
        // block would mark every auto-allowed call as waiting on a person.
        // The gate rides `preToolUse`.
        "permissionRequest" => Vec::new(),

        // `preCompact` fires while the window is full and there is no
        // `postCompact`, so a compaction is never observed.
        "preCompact" => Vec::new(),

        // The prompt after Copilot's rewriting; `userPromptSubmitted` already
        // counted what the person typed.
        "userPromptTransformed" => Vec::new(),

        "agentStop" => vec![Event::TurnEnded],
        "errorOccurred" => vec![Event::TurnFailed {
            message: p
                .error_message()
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
        // An unknown event costs itself, not the channel.
        _ => Vec::new(),
    }
}

/// Appends the observation events one Copilot payload carries, for the
/// `devplane hook` process that read it.
///
/// Like the Claude Code path: the row carries its resolved project (new
/// directories become projects), source `hook` for the host to fold in.
/// Nothing here projects; `runs` has one writer.
pub async fn observe_copilot(store: &Store, event: &str, p: &HookPayload) -> anyhow::Result<()> {
    if p.session_id.is_empty() {
        // A run keyed on an empty string would gather every such payload.
        return Ok(());
    }
    let payload = HookPayload {
        event: event.to_string(),
        ..p.clone()
    };
    let events = to_events(&payload);
    if events.is_empty() {
        return Ok(());
    }
    let mut world = World::new();
    for project in store.load_projects().await? {
        world.upsert_project(project);
    }
    let resolved = p.cwd.as_deref().and_then(|dir| world.resolve_project(dir));
    if let Some((id, Some(_discovered))) = &resolved
        && let Some(project) = world.project(id)
    {
        store.note_project(project).await?;
    }
    let project = resolved.map(|(id, _)| id);
    let run = RunId::new(p.session_id.clone());
    for event in events {
        let mut env = EventEnvelope::new(run.clone(), Source::CopilotHook, event);
        env.project_id = project.clone();
        store.append_event(&env).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The gate's reply
// ---------------------------------------------------------------------------

/// What a `preToolUse` hook may answer: a prohibition, or nothing.
///
///
/// Copilot reads a single top-level object. `allow` is never sent: it would
/// skip the provider's own permission flow, saved approvals included.
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

/// The arguments `devplane hook` is run with for one Copilot event.
///
/// The gate answers on stdout, so it has its own spelling; other events name
/// themselves, as the native payload does not.
pub fn hook_args(event: &str) -> Vec<String> {
    if event == GATE_EVENT {
        vec!["hook".into(), "--gate".into(), "copilot".into()]
    } else {
        vec![
            "hook".into(),
            "--observe".into(),
            "copilot".into(),
            "--event".into(),
            event.into(),
        ]
    }
}

/// The hook registration: one file, entirely Devplane's.
///
/// Copilot loads every `*.json` in `~/.copilot/hooks/`, so the installation
/// is one file of ours, never the user's. Silent events are not registered.
pub fn hooks_file(exe: &std::path::Path) -> Value {
    let mut hooks = Map::new();
    for (event, ledger) in EVENTS {
        if *ledger != Ledger::Mapped {
            continue;
        }
        hooks.insert(
            (*event).into(),
            json!([{
                "type": "command",
                "exec": exe.to_string_lossy(),
                "args": hook_args(event),
                // A bound; past it the hook fails open, the same outcome as
                // `undecided`.
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
        // Without it a `Read(.env)` rule would not govern Copilot's `view`,
        // silently granting.
        assert_eq!(tool_name("bash"), "Bash");
        assert_eq!(tool_name("view"), "Read");
        assert_eq!(tool_name("create"), "Write");
        assert_eq!(tool_name("str_replace_editor"), "Edit");
        assert_eq!(tool_name("web_fetch"), "WebFetch");
        // A tool with no equivalent keeps its own name.
        assert_eq!(tool_name("some_future_tool"), "some_future_tool");
    }

    #[test]
    fn the_gate_never_answers_allow() {
        // A grant would skip the provider's own permission flow.
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
                    agent_id,
                    ..
                },
            ] => {
                assert_eq!(tool, "Bash");
                assert_eq!(input["command"], json!("pnpm test"));
                // Copilot documents neither.
                assert_eq!(*server_source, None);
                assert_eq!(*agent_id, None);
            }
            other => panic!("expected a started tool, got {other:?}"),
        }
    }

    /// A field Copilot does not report is absent: never zero, never Claude
    /// Code's meaning.
    #[test]
    fn a_field_copilot_does_not_report_is_absent_rather_than_zero() {
        let payload: HookPayload = serde_json::from_value(serde_json::json!({
            "event": "sessionStart",
            "cwd": "/repo",
            "source": "resume",
        }))
        .expect("a session start with nothing else in it");

        let events = to_events(&payload);
        let [
            Event::SessionStarted {
                model,
                question_clock,
                clock_read,
                entrypoint,
                source,
                ..
            },
        ] = events.as_slice()
        else {
            panic!("expected one session start, got {events:?}");
        };

        // The model: Copilot's hook payload carries none.
        assert_eq!(
            *model, None,
            "a model Copilot did not report must be absent"
        );

        // The question clock: not read (`clock_read: false`). `true` would
        // claim nothing is set, i.e. questions wait — false here.
        assert_eq!(*question_clock, None);
        assert!(
            !*clock_read,
            "an unread environment may never render as one that was read and found empty"
        );

        // The entrypoint is genuinely known, from the channel.
        assert_eq!(entrypoint.as_deref(), Some("copilot"));
        // The documented `source` is read rather than replaced.
        assert_eq!(source.as_deref(), Some("resume"));
    }

    #[test]
    fn an_event_this_map_does_not_know_costs_one_event() {
        let p = HookPayload {
            event: "somethingNew".into(),
            ..Default::default()
        };
        assert!(to_events(&p).is_empty());
    }

    /// Every hook runs the binary, none uses HTTP; the gate's answer is read,
    /// the rest name their event.
    #[test]
    fn every_registered_hook_runs_the_binary_and_names_its_event() {
        let f = hooks_file(std::path::Path::new("/vp"));
        assert_eq!(f["version"], json!(1));
        let hooks = f["hooks"].as_object().unwrap();
        let mapped = EVENTS.iter().filter(|(_, l)| *l == Ledger::Mapped).count();
        assert_eq!(hooks.len(), mapped, "every mapped event and nothing else");
        assert!(
            hooks.contains_key("notification"),
            "the event the product is about was not registered"
        );
        for (event, entries) in hooks {
            let h = &entries[0];
            assert_eq!(h["type"], json!("command"), "{event}");
            assert_eq!(h["exec"], json!("/vp"), "{event}");
            assert!(h.get("url").is_none(), "{event} rides HTTP");
            let args: Vec<&str> = h["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap())
                .collect();
            if event == GATE_EVENT {
                assert_eq!(args, ["hook", "--gate", "copilot"]);
            } else {
                assert_eq!(args, ["hook", "--observe", "copilot", "--event", event]);
            }
        }
        // Silent events are not registered: a process that records nothing.
        assert!(!hooks.contains_key("permissionRequest"));
        assert!(!hooks.contains_key("preCompact"));
        // No HTTP anywhere in the file.
        assert!(!f.to_string().contains("http"));
    }

    fn payload(event: &str, v: Value) -> HookPayload {
        let mut o = v.as_object().cloned().unwrap_or_default();
        o.insert("event".into(), json!(event));
        serde_json::from_value(Value::Object(o)).expect("a payload this reader can parse")
    }

    /// Somebody is being asked, and the board can say so.
    #[test]
    fn a_copilot_session_waiting_on_a_person_is_a_block_the_board_can_render() {
        let out = to_events(&payload(
            "notification",
            json!({ "notification_type": "permission_prompt", "message": "Run `rm -rf build`?" }),
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
                // Copilot's dialog: never answerable from here.
                assert!(
                    request_id.is_none(),
                    "an observed dialog was made to look answerable"
                );
            }
            other => panic!("a permission prompt did not become a block: {other:?}"),
        }

        let asked = to_events(&payload(
            "notification",
            json!({ "notification_type": "elicitation_dialog", "message": "Which team?" }),
        ));
        assert!(matches!(
            asked.as_slice(),
            [Event::Blocked {
                waiting_for: crate::core::WaitingFor::Question,
                ..
            }]
        ));

        // A notification that is not a person's turn says nothing.
        for kind in ["shell_completed", "agent_idle", "agent_completed"] {
            assert!(
                to_events(&payload(
                    "notification",
                    json!({ "notification_type": kind })
                ))
                .is_empty(),
                "{kind} was read as somebody waiting"
            );
        }
    }

    /// `permissionRequest` fires before anything has been decided, so it is
    /// not a block.
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

    /// `errorOccurred` sends an object; `postToolUseFailure` a string.
    #[test]
    fn an_error_is_read_in_both_shapes_the_vendor_sends() {
        match to_events(&payload(
            "errorOccurred",
            json!({ "error": { "message": "model call failed", "name": "Error" },
                    "errorContext": "model_call", "recoverable": false }),
        ))
        .as_slice()
        {
            [Event::TurnFailed { message }] => assert_eq!(message, "model call failed"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            to_events(&payload(
                "postToolUseFailure",
                json!({ "toolName": "bash", "error": "exit 1" })
            ))
            .as_slice(),
            [Event::ToolFinished { ok: false, .. }]
        ));
    }

    /// Every event Copilot documents is in the ledger, compared against the
    /// vendor reference. Mapped events produce something; silent ones
    /// produce nothing.
    #[test]
    fn the_ledger_matches_the_vendors_table_and_describes_itself() {
        let reference = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/concepts/reference/copilot/hooks-reference.md"
        );
        // Without the reference files the vendor half cannot run, and says so.
        if let Ok(text) = std::fs::read_to_string(reference) {
            let table = text
                .split("## Hook events")
                .nth(1)
                .and_then(|t| t.split("## Hook event input payloads").next())
                .expect("the reference's event table");
            let documented: Vec<&str> = table
                .lines()
                .filter(|l| l.starts_with("| `"))
                .filter_map(|l| l.trim_start_matches("| `").split('`').next())
                .collect();
            assert_eq!(
                documented.len(),
                EVENTS.len(),
                "the vendor documents {documented:?} and the ledger has {:?}",
                EVENTS.iter().map(|(e, _)| *e).collect::<Vec<_>>()
            );
            for e in &documented {
                assert!(
                    EVENTS.iter().any(|(n, _)| n == e),
                    "`{e}` is in the vendor's table and not in the ledger"
                );
            }
        }

        for (e, ledger) in EVENTS {
            let v = match *e {
                "notification" => json!({ "notification_type": "permission_prompt" }),
                _ => json!({ "toolName": "bash", "agentName": "a", "prompt": "hi" }),
            };
            let produced = !to_events(&payload(e, v)).is_empty();
            match ledger {
                Ledger::Mapped => assert!(produced, "`{e}` is mapped and produces no event"),
                Ledger::Silent => assert!(!produced, "`{e}` is silent and produces an event"),
            }
        }
    }

    /// An observed payload lands in the store as a `Source::CopilotHook` row,
    /// keyed on Copilot's session id; an unmapped payload costs no row.
    #[tokio::test]
    async fn an_observed_payload_is_appended_with_the_hook_source() {
        let store = Store::open_in_memory().await.unwrap();
        let p: HookPayload = serde_json::from_value(json!({
            "sessionId": "cop-1",
            "cwd": "/tmp/nowhere-in-particular",
            "prompt": "fix the login test",
        }))
        .unwrap();
        observe_copilot(&store, "userPromptSubmitted", &p)
            .await
            .unwrap();
        let rows = store
            .events_for_run(&RunId::new("cop-1"), 10)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, Source::CopilotHook);
        assert!(matches!(
            rows[0].event,
            Event::PromptSubmitted { chars: 18 }
        ));

        // Silent by decision means silent in the store too.
        observe_copilot(&store, "permissionRequest", &p)
            .await
            .unwrap();
        assert_eq!(
            store
                .events_for_run(&RunId::new("cop-1"), 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}

//! The event model.
//!
//! Everything Devplane learns about a session arrives as an [`Event`] in an
//! [`EventEnvelope`]. Events are append-only observations, never edited; run
//! state is a pure reduction over them ([`crate::core::reduce`]). Anything
//! Devplane causes itself belongs in the runtime journal instead.

use crate::core::ids::{ProjectId, RunId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where an observation came from: shows which channel is alive and resolves
/// conflicts during reconciliation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// `devplane hook`, run by Claude Code on a lifecycle event.
    #[default]
    Hook,
    /// The same shim, run by GitHub Copilot.
    CopilotHook,
    /// The same shim, run by Codex.
    CodexHook,
    /// An OpenTelemetry log record or metric datapoint.
    Otel,
    /// The `claude agents --json` poller.
    AgentsJson,
    /// The status-line shim.
    StatusLine,
    /// A vendor's own event stream, connected out to. Nothing is installed into
    /// the agent, so an ending it carries is a fact the vendor published, not
    /// one Devplane derived.
    Feed,
    /// The host itself: reconciliation, timers, what a driven agent said.
    Host,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Hook => "hook",
            Source::CopilotHook => "copilot_hook",
            Source::CodexHook => "codex_hook",
            Source::Otel => "otel",
            Source::AgentsJson => "agents_json",
            Source::StatusLine => "statusline",
            Source::Feed => "feed",
            Source::Host => "host",
        }
    }

    /// The inverse of [`Self::as_str`]. `None` for an unknown name (the caller
    /// drops the row) — never a default, which would mislabel the channel.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "hook" => Source::Hook,
            "copilot_hook" => Source::CopilotHook,
            "codex_hook" => Source::CodexHook,
            "otel" => Source::Otel,
            "agents_json" => Source::AgentsJson,
            "statusline" => Source::StatusLine,
            "feed" => Source::Feed,
            "host" => Source::Host,
            _ => return None,
        })
    }

    /// The agent a run first seen on this source is named after. A hook
    /// carries no vendor field of its own; the shim was registered with one.
    pub fn agent(&self) -> Option<&'static str> {
        match self {
            Source::Hook | Source::StatusLine => Some("claude"),
            Source::CopilotHook => Some("copilot"),
            Source::CodexHook => Some("codex"),
            _ => None,
        }
    }
}

/// Why a session is blocked. Derived from `PermissionRequest` and the waiting
/// `Notification` matchers — never from `PreToolUse`, since most tool calls
/// never wait on anybody.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitingFor {
    /// A permission dialog is on screen.
    Permission,
    /// The agent asked a question (`AskUserQuestion`, MCP elicitation).
    Question,
    /// The turn ended and the agent is waiting for the next prompt.
    Idle,
    /// The turn ended and the agent is waiting on a command it started (a
    /// background job, often a test suite). Providers report this as `idle`,
    /// but it resumes by itself and no person is the blocker.
    Job,
    /// A provider reason we do not model (`sandbox request`, `dialog open`, …).
    /// The roster reports `waitingFor` only while waiting, so this still means
    /// a human is being waited on — unrecognised never means not urgent.
    Other(String),
}

impl WaitingFor {
    /// What the roster's `waitingFor` says a session is blocked on. The field
    /// is present only while `status` is `waiting`, so `None` is still a wait
    /// (mapped to [`Question`](Self::Question)) and unknown values keep their
    /// own words.
    pub fn parse_roster(waiting_for: Option<&str>) -> Self {
        match waiting_for {
            Some(w) if w.contains("permission") => WaitingFor::Permission,
            Some(w) if w.contains("input") || w.contains("question") => WaitingFor::Question,
            Some(other) => WaitingFor::Other(other.to_string()),
            None => WaitingFor::Question,
        }
    }
}

/// What one API call cost, as OpenTelemetry reports it per request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ApiUsage {
    pub model: Option<String>,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    /// The context window's current level, where the channel reports it
    /// directly (ACP's `used`). A level, never summed into a running total.
    #[serde(default)]
    pub context_level: Option<u64>,
}

impl ApiUsage {
    /// Tokens occupying the context window: the reported level if any, else
    /// input plus both cache figures (not output), as `used_percentage` counts.
    pub fn context_tokens(&self) -> u64 {
        self.context_level
            .unwrap_or(self.input_tokens + self.cache_read_tokens + self.cache_creation_tokens)
    }
}

/// One answer a blocked run will accept. The label is for humans; the `id` is
/// what the protocol requires back — ACP option ids are the agent's to choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Exported because `AbandonedQuestion` carries a `Vec<Choice>`; a type
// reachable from an exported one must be exported too.
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Choice {
    /// The protocol's id for this option. Absent for an observed session,
    /// whose dialog belongs to the provider and cannot be answered from here.
    #[serde(default)]
    pub id: Option<String>,
    pub label: String,
    /// `allow_once`, `allow_always`, `reject_once`, `reject_always` where the
    /// protocol says so.
    #[serde(default)]
    pub kind: Option<String>,
}

impl Choice {
    /// A label with no id: an option a human can read and only the provider can
    /// act on.
    pub fn label(label: impl Into<String>) -> Self {
        Self {
            id: None,
            label: label.into(),
            kind: None,
        }
    }

    pub fn is_allow(&self) -> bool {
        self.kind.as_deref().is_some_and(|k| k.starts_with("allow"))
    }

    pub fn is_reject(&self) -> bool {
        self.kind
            .as_deref()
            .is_some_and(|k| k.starts_with("reject"))
    }
}

impl<T: Into<String>> From<T> for Choice {
    fn from(label: T) -> Self {
        Self::label(label)
    }
}
/// One sample of the status line's payload. Only fields with a consumer
/// (`RunTotals`, `doctor`, the board) are kept.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusSample {
    /// The provider's own context percentage, preferred over the derived one.
    pub context_used_percent: Option<f64>,
    /// The window the percentage is of, as the provider states it — not
    /// inferred from the model.
    pub context_window_size: Option<u64>,
    /// Every rate-limit window the payload carried; the reducer keeps the one
    /// closest to its limit.
    pub rate_limits: Vec<RateWindow>,
    pub session_name: Option<String>,
    /// The live model, available without telemetry.
    pub model: Option<String>,
    /// The Claude Code release this session is running.
    pub claude_version: Option<String>,
    /// Session cost as the provider computes it, with no telemetry connected.
    pub cost_usd: Option<f64>,
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,
}

/// One rate-limit window: how much is used, and when it resets.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RateWindow {
    /// `five_hour`, `seven_day`, or `spend_limit`.
    pub name: String,
    pub used_percent: f64,
    /// Unix epoch seconds.
    pub resets_at: Option<i64>,
}

/// A single observation about a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A session started or resumed. `entrypoint` is OpenTelemetry's
    /// `app.entrypoint` where known (`cli`, `claude-vscode`, `sdk-ts`, …).
    SessionStarted {
        cwd: PathBuf,
        source: Option<String>,
        model: Option<String>,
        entrypoint: Option<String>,
        /// A question clock set by this session's environment
        /// (`CLAUDE_AFK_TIMEOUT_MS`), which overrides the settings files.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        question_clock: Option<crate::core::clock::QuestionClock>,
        /// Whether the environment was read at all. `question_clock: None`
        /// with this false means *not read*, never *nothing is set*.
        #[serde(default)]
        clock_read: bool,
    },
    /// A driven agent finished its handshake and named its own session — the
    /// id `session/resume` needs to continue the conversation after a restart.
    AgentSessionOpened {
        agent_session: String,
    },
    /// The OS process behind a driven run. Not liveness (that is the ACP
    /// connection); used to find a leaked agent that outlived a killed host.
    /// Absent when concurrent spawns made attribution a guess.
    AgentProcessSpawned {
        /// Also the process-group id: the protocol crate spawns an agent as
        /// its own group leader.
        pid: u32,
    },
    /// The human submitted a prompt. The text is never stored.
    PromptSubmitted {
        chars: usize,
    },
    /// A tool call is about to run.
    ToolStarted {
        tool: String,
        /// The tool input, for the permission card. Untrusted text wherever
        /// rendered.
        input: serde_json::Value,
        /// The MCP server's origin as the vendor reported it (`plugin`, `sdk`,
        /// `user`, `project`, or anything else verbatim). `None` for a non-MCP
        /// tool or an agent that does not report it — distinct from an
        /// unrecognised value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server_source: Option<String>,
        /// The subagent making the call, when not the main thread. Subagent
        /// hooks arrive under the parent's session id; without this they would
        /// read as the main thread moving past an open question.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
        /// The protocol's id for the call; starts and finishes correlate on it,
        /// never on a title.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
    },
    /// A tool call finished.
    ToolFinished {
        tool: String,
        ok: bool,
        duration_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
    },
    /// A driven agent's plan for the turn, replaced whole on every update.
    PlanUpdated {
        steps: Vec<crate::core::run::PlanStep>,
    },
    /// The permission mode a watched session reports. No hook announces a mode
    /// change, so this is emitted from any payload carrying the field and is
    /// idempotent: only a different mode means anything.
    PermissionModeSeen {
        /// The vendor's own spelling, parsed by `PermissionMode` at the edge
        /// and kept verbatim when it is a value this build does not know.
        mode: String,
    },
    /// A permission decision was taken, by a rule, a hook or the human.
    PermissionDecided {
        tool: String,
        decision: String,
        by: String,
        /// Why, in the decider's own words (e.g. an auto-mode classifier rule).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// What the vendor's permission system had already worked out, where
        /// the payload carried it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<PermissionContext>,
    },
    /// A question the agent asked was answered anywhere (Claude Code's
    /// `ElicitationResult`), including in the person's own terminal.
    QuestionAnswered {
        /// `accept`, `decline` or `cancel`, as the provider reports it.
        action: String,
    },
    /// A configuration file Claude Code reads has changed. Devplane's hooks
    /// live in one of them, and losing them otherwise only shows as silence.
    ConfigChanged {
        /// `user_settings`, `project_settings`, `local_settings`,
        /// `policy_settings` or `skills`.
        source: String,
        path: Option<std::path::PathBuf>,
    },
    /// One entry of an observed session's own task list changed — the
    /// observed equivalent of a driven run's plan.
    TaskChanged {
        id: String,
        subject: String,
        done: bool,
    },
    /// The agent is blocked on a human.
    Blocked {
        waiting_for: WaitingFor,
        message: Option<String>,
        /// The protocol's id for the outstanding request: present for a driven
        /// run (decidable here), absent for an observed one (look only).
        #[serde(default)]
        request_id: Option<String>,
        /// The durable ask an answer is addressed to; outlives the connection
        /// `request_id` names.
        #[serde(default)]
        ask: Option<String>,
        /// The answers the agent will accept, with the ids it expects back.
        #[serde(default)]
        options: Vec<Choice>,
        /// The call being waited on, where known. Carried here because Claude
        /// Code resolves permission before invoking the tool, so there may be
        /// no `ToolStarted` to read it from.
        #[serde(default)]
        call: Option<ToolCallRef>,
        /// Which vendor mechanism was about to decide (classifier, rule, or
        /// plain prompt), where the payload said.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<PermissionContext>,
    },
    /// The agent asked a question with options. An observed session's dialog
    /// belongs to the provider (no `request_id`); a driven run's arrives as a
    /// form elicitation and is answerable here.
    QuestionAsked {
        question: String,
        options: Vec<Choice>,
        /// Present when this question can be answered from here.
        #[serde(default)]
        request_id: Option<String>,
        /// The durable ask an answer is addressed to; outlives the connection
        /// `request_id` names.
        #[serde(default)]
        ask: Option<String>,
        /// The field each option answers when several questions were asked at
        /// once, and the free-text box where one was offered.
        #[serde(default)]
        form: Option<serde_json::Value>,
    },
    /// A question stopped being answerable without an answer (call cancelled or
    /// run ended), so the inbox stops offering it.
    QuestionEnded {
        request_id: String,
    },
    /// The turn ended normally.
    TurnEnded,
    /// Background tasks in flight, from the session's `Stop` payload — the
    /// session's own version of `RosterSeen::jobs`. A `0` is a checked zero
    /// and may clear a job block.
    JobsSeen {
        running: u32,
    },
    /// The turn ended with an error.
    TurnFailed {
        message: String,
    },
    /// An API call completed. The only per-request cost signal.
    ApiRequest {
        usage: ApiUsage,
    },
    /// An API call failed.
    ApiError {
        error: String,
        status: Option<i64>,
    },
    /// A subagent started or stopped, for the sub-agent tree.
    SubagentStarted {
        agent_id: String,
        kind: Option<String>,
    },
    SubagentStopped {
        agent_id: String,
    },
    /// The session moved to another directory or worktree.
    CwdChanged {
        cwd: PathBuf,
    },
    WorktreeEntered {
        path: PathBuf,
        branch: Option<String>,
    },
    /// Context was compacted, which resets the context gauge.
    Compacted,
    /// The session switched model, which changes the context gauge's window.
    ModelChanged {
        model: String,
    },
    /// The session ended.
    SessionEnded {
        reason: Option<String>,
    },
    /// An ACP agent's current mode. A bare string, deliberately not a
    /// `PermissionMode`: ACP modes are agent-declared with no cross-vendor
    /// vocabulary, and mapping them would assert a vendor decision we cannot
    /// derive.
    AgentModeSeen {
        mode: String,
    },
    /// What the agent's own `session/list` says about this session. An event,
    /// not a direct edit, so it survives replay. It carries no state field and
    /// the reducer applies it only to fill gaps.
    SessionListed {
        agent_session: String,
        title: Option<String>,
    },
    /// A row of `claude agents --json`, listing every live session. A
    /// background row's `state` is authoritative (the provider owns that
    /// process); for an interactive row, hooks stay the authority.
    RosterSeen {
        kind: String,
        state: Option<String>,
        /// `busy` or `idle` for a session that is reporting. Absent: the
        /// process exists but has said nothing.
        status: Option<String>,
        waiting_for: Option<String>,
        pid: Option<u32>,
        name: Option<String>,
        entrypoint: Option<String>,
        /// When the session started, in epoch milliseconds.
        started_at_ms: Option<i64>,
        /// Commands this session has running, from the process table. `None`
        /// means nobody looked, never zero: `Some(0)` may clear a stale job
        /// block, `None` must leave what is known alone.
        #[serde(default)]
        jobs: Option<u32>,
    },
    /// A status-line sample: the only channel that carries rate limits.
    StatusSample(StatusSample),
    /// A live run has produced nothing for longer than the stall timeout. The
    /// host records it once per quiet period, not once per check.
    Stalled {
        idle_seconds: i64,
    },
    /// Reconciliation could not find the process or session any more.
    Lost {
        reason: String,
    },
    /// The tasks sent to this run. Recorded when sent, never inferred: a run
    /// without one was sent nothing.
    TasksSent {
        tasks: Vec<crate::core::spec::SentTask>,
    },
    /// The specification's ticked boxes when the run closed, and the fingerprint
    /// of the folder, so *ticked* sits beside *sent* and plan drift is a
    /// comparison.
    SpecObserved {
        fingerprint: Option<String>,
        /// The keys of every ticked task in the task file.
        ticked: Vec<String>,
        /// The newest modification time among the folder's documents.
        changed_at: Option<Timestamp>,
    },
    /// Something changed that no observation describes (a change phase, a gate
    /// verdict, a snooze). Tells subscribers to ask again; applied to no run.
    Refresh,
}

/// A tool call named on an event that is not itself a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRef {
    pub tool: String,
    pub input: serde_json::Value,
}

/// Claude Code's `permission_context` on a `PermissionRequest`. Every field is
/// optional and verbatim; never read by a verdict, so nothing here can widen
/// anything.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionContext {
    /// The auto-mode classifier's verdict, where it ran.
    pub classifier_verdict: Option<String>,
    pub classifier_confidence: Option<f64>,
    /// The permission rule the request matched, where one did.
    pub permission_rule_matched: Option<String>,
    /// The subagent the request came from, when it was not the main thread.
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
}

#[cfg(test)]
impl Event {
    /// The channel an event of this kind actually arrives on, for test
    /// envelopes. Labelling everything `Hook` would make the reducer stand the
    /// roster down and skip the roster paths under test.
    pub fn test_source(&self) -> Source {
        match self {
            Event::RosterSeen { .. } => Source::AgentsJson,
            Event::StatusSample(_) => Source::StatusLine,
            Event::Stalled { .. }
            | Event::Lost { .. }
            | Event::TasksSent { .. }
            | Event::SpecObserved { .. }
            | Event::Refresh => Source::Host,
            _ => Source::Hook,
        }
    }
}

impl Event {
    /// A short, stable label for logs and the diagnostics view.
    pub fn label(&self) -> &'static str {
        match self {
            Event::SessionStarted { .. } => "session_started",
            Event::AgentSessionOpened { .. } => "agent_session_opened",
            Event::AgentProcessSpawned { .. } => "agent_process_spawned",
            Event::PromptSubmitted { .. } => "prompt_submitted",
            Event::ToolStarted { .. } => "tool_started",
            Event::ToolFinished { .. } => "tool_finished",
            Event::PlanUpdated { .. } => "plan_updated",
            Event::PermissionModeSeen { .. } => "permission_mode_seen",
            Event::PermissionDecided { .. } => "permission_decided",
            Event::QuestionAnswered { .. } => "question_answered",
            Event::ConfigChanged { .. } => "config_changed",
            Event::TaskChanged { .. } => "task_changed",
            Event::Blocked { .. } => "blocked",
            Event::QuestionAsked { .. } => "question_asked",
            Event::QuestionEnded { .. } => "question_ended",
            Event::TurnEnded => "turn_ended",
            Event::JobsSeen { .. } => "jobs_seen",
            Event::TurnFailed { .. } => "turn_failed",
            Event::ApiRequest { .. } => "api_request",
            Event::ApiError { .. } => "api_error",
            Event::SubagentStarted { .. } => "subagent_started",
            Event::SubagentStopped { .. } => "subagent_stopped",
            Event::CwdChanged { .. } => "cwd_changed",
            Event::WorktreeEntered { .. } => "worktree_entered",
            Event::Compacted => "compacted",
            Event::ModelChanged { .. } => "model_changed",
            Event::SessionEnded { .. } => "session_ended",
            Event::AgentModeSeen { .. } => "agent_mode_seen",
            Event::SessionListed { .. } => "session_listed",
            Event::RosterSeen { .. } => "roster_seen",
            Event::StatusSample(_) => "status_sample",
            Event::Stalled { .. } => "stalled",
            Event::Lost { .. } => "lost",
            Event::TasksSent { .. } => "tasks_sent",
            Event::SpecObserved { .. } => "spec_observed",
            Event::Refresh => "refresh",
        }
    }

    /// Whether this event means the session is doing something. Used for the
    /// stall timer: a session that only emits telemetry is still alive.
    pub fn is_activity(&self) -> bool {
        // The two spec edges are the host's own acts about the run, not the
        // run doing something.
        !matches!(
            self,
            Event::StatusSample { .. }
                | Event::RosterSeen { .. }
                | Event::Stalled { .. }
                | Event::Lost { .. }
                | Event::TasksSent { .. }
                | Event::SpecObserved { .. }
                | Event::Refresh
        )
    }
}

/// An event with its correlation keys and arrival time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: String,
    pub at: Timestamp,
    pub run_id: RunId,
    pub project_id: Option<ProjectId>,
    pub source: Source,
    pub event: Event,
}

impl Event {
    /// A tool call from a source that reports no server provenance, agent id
    /// or call id.
    #[must_use]
    pub fn tool_started(tool: impl Into<String>, input: serde_json::Value) -> Self {
        Event::ToolStarted {
            tool: tool.into(),
            input,
            server_source: None,
            agent_id: None,
            call_id: None,
        }
    }
}

impl EventEnvelope {
    pub fn new(run_id: RunId, source: Source, event: Event) -> Self {
        Self {
            id: crate::core::ids::new_event_id(),
            at: Timestamp::now(),
            run_id,
            project_id: None,
            source,
            event,
        }
    }
}

#[cfg(test)]
mod source_tests {
    use super::Source;

    /// Every variant survives the store's spelling, and an unknown name is
    /// `None` rather than the nearest variant.
    #[test]
    fn every_source_round_trips_by_name_and_an_unknown_one_is_none() {
        for s in [
            Source::Hook,
            Source::Otel,
            Source::AgentsJson,
            Source::StatusLine,
            Source::Feed,
            Source::Host,
        ] {
            assert_eq!(Source::parse(s.as_str()), Some(s));
        }
        assert_eq!(Source::parse("telepathy"), None);
    }
}

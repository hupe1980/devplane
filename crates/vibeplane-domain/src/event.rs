//! The event model.
//!
//! Everything Vibeplane learns about a session arrives as an [`Event`] wrapped
//! in an [`EventEnvelope`]. Events are append-only observations: they are never
//! edited, and run state is a pure reduction over them
//! ([`vibeplane_core::reduce`]).
//!
//! Events are *observations*, not effects. Anything Vibeplane causes itself
//! belongs in the runtime journal instead.

use crate::ids::{ProjectId, RunId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where an observation came from. Used by diagnostics to show which channel is
/// alive, and to resolve conflicts during reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A Claude Code hook posted to the daemon.
    Hook,
    /// An OpenTelemetry log record or metric datapoint.
    Otel,
    /// The `claude agents --json` poller.
    AgentsJson,
    /// The status-line shim.
    StatusLine,
    /// Vibeplane itself (reconciliation, timers).
    Daemon,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Hook => "hook",
            Source::Otel => "otel",
            Source::AgentsJson => "agents_json",
            Source::StatusLine => "statusline",
            Source::Daemon => "daemon",
        }
    }
}

/// The reason a session is blocked. Derived from `Notification` matchers, which
/// fire only when Claude is actually waiting — unlike `PermissionRequest`,
/// which fires on every tool check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitingFor {
    /// A permission dialog is on screen.
    Permission,
    /// The agent asked a question (`AskUserQuestion`, MCP elicitation).
    Question,
    /// The turn ended and the agent is waiting for the next prompt.
    Idle,
    /// Something else the provider named but we do not model.
    Other(String),
}

/// What one API call cost. Reported by OpenTelemetry per request, which is the
/// only channel with per-request granularity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ApiUsage {
    pub model: Option<String>,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

impl ApiUsage {
    /// Tokens occupying the context window. Matches the status line's
    /// `used_percentage`, which counts input plus both cache figures and
    /// deliberately excludes output.
    pub fn context_tokens(&self) -> u64 {
        self.input_tokens + self.cache_read_tokens + self.cache_creation_tokens
    }
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
    },
    /// The human submitted a prompt. The text itself is not stored: telemetry
    /// is redacted by default and Vibeplane never turns that off.
    PromptSubmitted {
        chars: usize,
    },
    /// A tool call is about to run.
    ToolStarted {
        tool: String,
        /// The tool input, kept for the permission card. Treated as untrusted
        /// text everywhere it is rendered.
        input: serde_json::Value,
    },
    /// A tool call finished.
    ToolFinished {
        tool: String,
        ok: bool,
        duration_ms: Option<u64>,
    },
    /// A permission decision was taken, by a rule, a hook or the human.
    PermissionDecided {
        tool: String,
        decision: String,
        by: String,
    },
    /// The agent is blocked on a human.
    Blocked {
        waiting_for: WaitingFor,
        message: Option<String>,
    },
    /// The agent asked a question with options. Observed sessions cannot be
    /// answered from Vibeplane, only focused.
    QuestionAsked {
        question: String,
        options: Vec<String>,
    },
    /// The turn ended normally.
    TurnEnded,
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
    /// The session ended.
    SessionEnded {
        reason: Option<String>,
    },
    /// A row of `claude agents --json`.
    ///
    /// The roster lists **every** live session, interactive ones included — it
    /// is how the board is populated the moment Vibeplane is installed, before
    /// a single hook has fired. What it means depends on the row:
    ///
    /// * a background row carries `state`, and the provider's daemon owns that
    ///   process, so its verdict is authoritative;
    /// * an interactive row carries identity and sometimes `status`, and hooks
    ///   remain the authority on what the session is doing.
    RosterSeen {
        kind: String,
        state: Option<String>,
        status: Option<String>,
        waiting_for: Option<String>,
        pid: Option<u32>,
        name: Option<String>,
        entrypoint: Option<String>,
    },
    /// A status-line sample: the only channel that carries rate limits.
    StatusSample {
        context_used_percent: Option<f64>,
        rate_limit_five_hour: Option<f64>,
        rate_limit_seven_day: Option<f64>,
        session_name: Option<String>,
    },
    /// A live run has produced nothing for longer than the stall timeout.
    ///
    /// Nothing reports a stall — it is the absence of evidence — so the daemon
    /// notices it by looking at the clock and records that it did. Emitted once
    /// per quiet period, so a run that is silent overnight produces one event
    /// rather than one a minute.
    Stalled {
        idle_seconds: i64,
    },
    /// Reconciliation could not find the process or session any more.
    Lost {
        reason: String,
    },
}

impl Event {
    /// A short, stable label for logs and the diagnostics view.
    pub fn label(&self) -> &'static str {
        match self {
            Event::SessionStarted { .. } => "session_started",
            Event::PromptSubmitted { .. } => "prompt_submitted",
            Event::ToolStarted { .. } => "tool_started",
            Event::ToolFinished { .. } => "tool_finished",
            Event::PermissionDecided { .. } => "permission_decided",
            Event::Blocked { .. } => "blocked",
            Event::QuestionAsked { .. } => "question_asked",
            Event::TurnEnded => "turn_ended",
            Event::TurnFailed { .. } => "turn_failed",
            Event::ApiRequest { .. } => "api_request",
            Event::ApiError { .. } => "api_error",
            Event::SubagentStarted { .. } => "subagent_started",
            Event::SubagentStopped { .. } => "subagent_stopped",
            Event::CwdChanged { .. } => "cwd_changed",
            Event::WorktreeEntered { .. } => "worktree_entered",
            Event::Compacted => "compacted",
            Event::SessionEnded { .. } => "session_ended",
            Event::RosterSeen { .. } => "roster_seen",
            Event::StatusSample { .. } => "status_sample",
            Event::Stalled { .. } => "stalled",
            Event::Lost { .. } => "lost",
        }
    }

    /// Whether this event means the session is doing something. Used for the
    /// stall timer: a session that only emits telemetry is still alive.
    pub fn is_activity(&self) -> bool {
        !matches!(
            self,
            Event::StatusSample { .. }
                | Event::RosterSeen { .. }
                | Event::Stalled { .. }
                | Event::Lost { .. }
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

impl EventEnvelope {
    pub fn new(run_id: RunId, source: Source, event: Event) -> Self {
        Self {
            id: crate::ids::new_event_id(),
            at: Timestamp::now(),
            run_id,
            project_id: None,
            source,
            event,
        }
    }

    pub fn with_project(mut self, project: Option<ProjectId>) -> Self {
        self.project_id = project;
        self
    }
}

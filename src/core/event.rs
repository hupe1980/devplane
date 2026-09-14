//! The event model.
//!
//! Everything Vibeplane learns about a session arrives as an [`Event`] wrapped
//! in an [`EventEnvelope`]. Events are append-only observations: they are never
//! edited, and run state is a pure reduction over them
//! ([`crate::core::reduce`]).
//!
//! Events are *observations*, not effects. Anything Vibeplane causes itself
//! belongs in the runtime journal instead.

use crate::core::ids::{ProjectId, RunId};
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
    /// Something else the provider named but we do not model — `sandbox
    /// request`, `worker request`, `dialog open`, or whatever is added next.
    ///
    /// The roster reports `waitingFor` only while a session is *waiting*, so
    /// any value here means a human is being waited on. Modelling it as "not a
    /// thing we recognise, therefore not urgent" is how three documented
    /// blocked states reached the board and never reached the inbox.
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
    /// How full the context window is *right now*, where the channel reports
    /// the level directly rather than the tokens of one request.
    ///
    /// The Agent Client Protocol does exactly that — its usage update carries
    /// `used` and `size` for the session — and a level must never be added to a
    /// running total. Putting it in `input_tokens` meant a driven run's token
    /// count was the sum of every window level it had ever reported, which
    /// grows roughly with the square of the conversation and means nothing.
    #[serde(default)]
    pub context_level: Option<u64>,
}

impl ApiUsage {
    /// Tokens occupying the context window.
    ///
    /// A reported level wins, because it is the provider's own answer. Derived
    /// otherwise from input plus both cache figures and deliberately not
    /// output — which is what the status line's `used_percentage` counts.
    pub fn context_tokens(&self) -> u64 {
        self.context_level
            .unwrap_or(self.input_tokens + self.cache_read_tokens + self.cache_creation_tokens)
    }
}

/// One answer a blocked run will accept.
///
/// The label is what a human reads; the `id` is what the protocol requires back
/// and is the whole reason this is not a list of strings. Vibeplane used to
/// carry only labels and then send the literal `"allow"` as the choice, which
/// works against a fixture that happens to name its option that and against no
/// real agent at all — ACP option ids are the agent's to choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// A driven agent finished its handshake and named its own session.
    ///
    /// Recorded because it is the *only* thing that can continue this
    /// conversation later. A run id is Vibeplane's; the id an agent will accept
    /// on `session/resume` is the agent's, it arrives after the handshake, and
    /// without it a daemon restart can only start a new conversation and pay to
    /// rediscover everything the last one knew.
    AgentSessionOpened {
        agent_session: String,
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
    /// A question the agent asked has been answered — anywhere.
    ///
    /// The partner of [`PermissionDecided`](Event::PermissionDecided). Claude
    /// Code fires `ElicitationResult` when a person answers an MCP elicitation,
    /// so a question answered in their own terminal clears here too.
    QuestionAnswered {
        /// `accept`, `decline` or `cancel`, as the provider reports it.
        action: String,
    },
    /// A configuration file Claude Code reads has changed.
    ///
    /// Worth an event because Vibeplane *writes* one of them: the hooks the
    /// observer depends on live in `~/.claude/settings.json`, and the only
    /// other symptom of losing them is a channel going quiet.
    ConfigChanged {
        /// `user_settings`, `project_settings`, `local_settings`,
        /// `policy_settings` or `skills`.
        source: String,
        path: Option<std::path::PathBuf>,
    },
    /// One entry of an observed session's own task list changed.
    ///
    /// The equivalent of the plan the protocol streams for a driven run: what
    /// the agent is trying to do, for a session Vibeplane only watches.
    TaskChanged {
        id: String,
        subject: String,
        done: bool,
    },
    /// The agent is blocked on a human.
    Blocked {
        waiting_for: WaitingFor,
        message: Option<String>,
        /// The protocol's id for the outstanding request, when there is one to
        /// answer. Present for a driven run, absent for an observed session —
        /// which is exactly the difference between an inbox item you can
        /// decide and one you can only go and look at.
        #[serde(default)]
        request_id: Option<String>,
        /// The answers the agent will accept, with the ids it expects back.
        #[serde(default)]
        options: Vec<Choice>,
    },
    /// The agent asked a question with options. Observed sessions cannot be
    /// answered from Vibeplane, only focused.
    QuestionAsked {
        question: String,
        options: Vec<Choice>,
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
    /// The session switched model.
    ///
    /// The window a context gauge is a percentage *of* belongs to the model, so
    /// a `/model` mid-session changes the denominator. Without this the run
    /// kept the first model it was ever seen using, and the gauge went on
    /// dividing by that one's window.
    ModelChanged {
        model: String,
    },
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
        /// `busy` or `idle` for a session that is reporting. Absent means the
        /// process exists but has not said anything — a tab left open rather
        /// than a session in use.
        status: Option<String>,
        waiting_for: Option<String>,
        pid: Option<u32>,
        name: Option<String>,
        entrypoint: Option<String>,
        /// When the session itself started, in milliseconds since the epoch.
        /// Without it the board dates every discovered session to the moment
        /// the daemon happened to look, and a three-day-old tab reads as new.
        started_at_ms: Option<i64>,
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
    /// Something changed that no observation describes — a work phase, a gate
    /// verdict, a snooze. Carries nothing: it exists so a subscriber knows to
    /// ask again, and it is not applied to any run.
    Refresh,
}

impl Event {
    /// A short, stable label for logs and the diagnostics view.
    pub fn label(&self) -> &'static str {
        match self {
            Event::SessionStarted { .. } => "session_started",
            Event::AgentSessionOpened { .. } => "agent_session_opened",
            Event::PromptSubmitted { .. } => "prompt_submitted",
            Event::ToolStarted { .. } => "tool_started",
            Event::ToolFinished { .. } => "tool_finished",
            Event::PermissionDecided { .. } => "permission_decided",
            Event::QuestionAnswered { .. } => "question_answered",
            Event::ConfigChanged { .. } => "config_changed",
            Event::TaskChanged { .. } => "task_changed",
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
            Event::ModelChanged { .. } => "model_changed",
            Event::SessionEnded { .. } => "session_ended",
            Event::RosterSeen { .. } => "roster_seen",
            Event::StatusSample { .. } => "status_sample",
            Event::Stalled { .. } => "stalled",
            Event::Lost { .. } => "lost",
            Event::Refresh => "refresh",
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

    pub fn with_project(mut self, project: Option<ProjectId>) -> Self {
        self.project_id = project;
        self
    }
}

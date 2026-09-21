//! The event model.
//!
//! Everything Devplane learns about a session arrives as an [`Event`] wrapped
//! in an [`EventEnvelope`]. Events are append-only observations: they are never
//! edited, and run state is a pure reduction over them
//! ([`crate::core::reduce`]).
//!
//! Events are *observations*, not effects. Anything Devplane causes itself
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
    /// Devplane itself (reconciliation, timers).
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
    /// The turn ended and the agent is waiting on a command **it** started —
    /// a background job, most often a test suite.
    ///
    /// **This is not idleness and it is not a person's turn.** Every provider
    /// reports such a session as `idle`, because from the model's side it is:
    /// no tokens are being generated. But the session will resume by itself
    /// when the job exits, and nothing is owed by anybody. Rendering it as
    /// *waiting for a prompt* tells a person they are the blocker in the one
    /// situation where they are not, and a board that does that on every long
    /// test run is a board people stop believing.
    Job,
    /// Something else the provider named but we do not model — `sandbox
    /// request`, `worker request`, `dialog open`, or whatever is added next.
    ///
    /// The roster reports `waitingFor` only while a session is *waiting*, so
    /// any value here means a human is being waited on. Modelling it as "not a
    /// thing we recognise, therefore not urgent" is how three documented
    /// blocked states reached the board and never reached the inbox.
    Other(String),
}

impl WaitingFor {
    /// What the roster's `waitingFor` says a session is blocked on.
    ///
    /// The vendor documents five values — *"`permission prompt` for an
    /// approval, `input needed` for a question from Claude or an MCP server's
    /// input request, `sandbox request`, `worker request`, or `dialog
    /// open`"* — and the field is present **only while `status` is
    /// `waiting`**, so reaching this function at all means a person is being
    /// waited on.
    ///
    /// `None` maps to [`Question`](Self::Question) rather than to nothing: the
    /// roster said the session is waiting, and a wait whose reason did not
    /// arrive is still a wait. Anything unrecognised keeps its own words —
    /// this vocabulary has grown twice, and *not a value we model* must never
    /// become *not urgent*.
    pub fn parse_roster(waiting_for: Option<&str>) -> Self {
        match waiting_for {
            Some(w) if w.contains("permission") => WaitingFor::Permission,
            Some(w) if w.contains("input") || w.contains("question") => WaitingFor::Question,
            Some(other) => WaitingFor::Other(other.to_string()),
            None => WaitingFor::Question,
        }
    }
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
/// and is the whole reason this is not a list of strings. Devplane used to
/// carry only labels and then send the literal `"allow"` as the choice, which
/// works against a fixture that happens to name its option that and against no
/// real agent at all — ACP option ids are the agent's to choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Exported because `AbandonedQuestion` carries a `Vec<Choice>` and the interface
// renders the options an agent chose between. A type reachable from an exported
// one has to be exported too, or the feature does not compile — which is how
// this was found: `cargo build --features typescript` failed on a tree where
// everything else was green, because nothing in CI builds that feature.
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
/// One sample of the status line's payload.
///
/// A struct rather than a wide variant: the payload has roughly forty
/// documented fields. Each one here has a consumer — `RunTotals`, `doctor` or
/// the board — because a field nothing reads is a claim, not a feature.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusSample {
    /// The provider's own context percentage, preferred over the derived one.
    pub context_used_percent: Option<f64>,
    /// The window the percentage is *of* — 200 000, or 1 000 000 on an
    /// extended-context model. Stated by the provider rather than inferred from
    /// which model is in play, which is a lookup table that rots on every
    /// model launch.
    pub context_window_size: Option<u64>,
    /// Every rate-limit window the payload carried, with when each resets.
    /// The reducer keeps the one closest to its limit.
    pub rate_limits: Vec<RateWindow>,
    pub session_name: Option<String>,
    /// The live model, with no telemetry connected. Otherwise this needs the
    /// OTEL channel, or a `SessionStart` the reference says Claude Code
    /// "doesn't always include".
    pub model: Option<String>,
    /// The Claude Code release **this session** is running.
    ///
    /// Carried because a fact about *this* session beats an assumption about
    /// the machine. It used to feed a warning about how far past the gate's
    /// measured baseline a session had drifted; that baseline measured the
    /// decay of a claim this product no longer makes, and went with it.
    pub claude_version: Option<String>,
    /// Session cost as the provider computes it, with no telemetry connected.
    pub cost_usd: Option<f64>,
    /// What the session changed. Nothing else here reports it cheaply.
    pub lines_added: Option<u64>,
    pub lines_removed: Option<u64>,
}

/// One rate-limit window: how much is used, and when it resets.
///
/// The reset time is what makes the percentage actionable.
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
        /// A clock this session's **environment** put on its questions.
        ///
        /// `CLAUDE_AFK_TIMEOUT_MS` overrides the settings files and turns
        /// auto-continue on even where they say `never`, so a session can be
        /// answering for somebody while every file on the machine says nothing
        /// is set. It is per session because the override is.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        question_clock: Option<crate::core::clock::QuestionClock>,
        /// Whether the environment was **read at all** for this session.
        ///
        /// Kept apart from `question_clock: None`, which means *read, and
        /// nothing was set*. A session that started before `devplane connect`,
        /// or on a channel that carries no environment, has neither — and
        /// *not read* must never render as *nothing is set*, which is the
        /// distinction the roster already makes for observation channels.
        #[serde(default)]
        clock_read: bool,
    },
    /// A driven agent finished its handshake and named its own session.
    ///
    /// Recorded because it is the *only* thing that can continue this
    /// conversation later. A run id is Devplane's; the id an agent will accept
    /// on `session/resume` is the agent's, it arrives after the handshake, and
    /// without it a daemon restart can only start a new conversation and pay to
    /// rediscover everything the last one knew.
    AgentSessionOpened {
        agent_session: String,
    },
    /// The operating-system process behind a **driven** run, once Devplane has
    /// worked out which one it is.
    ///
    /// Recorded for one purpose and it is not liveness: a driven run's liveness
    /// is decided by whether its ACP connection exists, never by whether a
    /// process does. This is how a *leaked* agent is found — one that outlived
    /// the daemon that started it because the daemon was killed rather than
    /// stopped, and which is now holding a worktree with nobody able to reach
    /// it. Absent when two dispatches raced closely enough that the new child
    /// could not be attributed to one of them without guessing.
    AgentProcessSpawned {
        /// Also the process-group id: the protocol crate spawns an agent as
        /// its own group leader, which is half of what identifies it later.
        pid: u32,
    },
    /// The human submitted a prompt. The text itself is not stored: telemetry
    /// is redacted by default and Devplane never turns that off.
    PromptSubmitted {
        chars: usize,
    },
    /// A tool call is about to run.
    ToolStarted {
        tool: String,
        /// The tool input, kept for the permission card. Treated as untrusted
        /// text everywhere it is rendered.
        input: serde_json::Value,
        /// Where the MCP server behind this call came from, as the vendor
        /// reported it: `plugin`, `sdk`, `user`, `project`, or a value this
        /// build has never seen, carried through as received.
        ///
        /// `None` for a tool that is not an MCP tool, and for an agent older
        /// than the release that began reporting it — **which reads differently
        /// from a source nobody understood**, and must keep doing so.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server_source: Option<String>,
    },
    /// A tool call finished.
    ToolFinished {
        tool: String,
        ok: bool,
        duration_ms: Option<u64>,
    },
    /// The permission mode a watched session reports it is running in.
    ///
    /// **Observed off whatever hook happened to carry it**, because no hook
    /// announces a mode *change*. Eleven of the vendor's events include the
    /// field and `PreToolUse` — the obvious candidate — is not one of them, so
    /// this is emitted from any payload that has it rather than from a chosen
    /// event. That makes it idempotent by necessity: the same mode arrives
    /// over and over, and only a *different* one means anything.
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
    /// Worth an event because Devplane *writes* one of them: the hooks the
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
    /// the agent is trying to do, for a session Devplane only watches.
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
        /// The durable ask this belongs to — the token an answer is addressed
        /// to, which outlives the connection `request_id` names.
        #[serde(default)]
        ask: Option<String>,
        /// The answers the agent will accept, with the ids it expects back.
        #[serde(default)]
        options: Vec<Choice>,
        /// The call being waited on, where the source knows it.
        ///
        /// **The permission hook knows it and the tool hook may never have
        /// fired.** Claude Code resolves permission *before* invoking the tool,
        /// so a run blocked on a permission often has no in-flight
        /// `ToolStarted` to read it off — which is why this travels on the
        /// event rather than being recovered from the run.
        #[serde(default)]
        call: Option<ToolCallRef>,
    },
    /// The agent asked a question with options.
    ///
    /// Two sources, and the difference is whether it can be answered here.
    /// An **observed** session's dialog belongs to the provider: it carries no
    /// `request_id` and can only be focused. A **driven** run's question arrives
    /// over the protocol as a form elicitation and *is* answerable, which is
    /// what `request_id` and `detail` are for.
    QuestionAsked {
        question: String,
        options: Vec<Choice>,
        /// Present when this question can be answered from here.
        #[serde(default)]
        request_id: Option<String>,
        /// The durable ask this belongs to — the token an answer is addressed
        /// to, which outlives the connection `request_id` names.
        #[serde(default)]
        ask: Option<String>,
        /// The field each option answers, where several questions were asked at
        /// once, and the free-text box where one was offered.
        #[serde(default)]
        form: Option<serde_json::Value>,
    },
    /// A question stopped being answerable without having been answered — the
    /// call was cancelled, or the run ended under it.
    ///
    /// Recorded for the reason [`PermissionDecided`](Event::PermissionDecided)
    /// is: an inbox that goes on offering an answer nothing can receive is an
    /// inbox that lies.
    QuestionEnded {
        request_id: String,
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
    /// An ACP agent says which mode it is now in.
    ///
    /// **Deliberately a separate event from `PermissionModeSeen`, carrying a
    /// bare string rather than a `PermissionMode`.** That type is the four
    /// values Claude Code documents plus `Unrecognised`; an ACP session mode is
    /// an identifier the *agent* declares, with no cross-vendor vocabulary and
    /// no specification mapping it onto anybody's four. Parsing it into
    /// `PermissionMode` would turn every unknown agent string into
    /// `Unrecognised(..)` and file it under a heading that says what a *vendor*
    /// decided — which is this product asserting an authority it cannot derive.
    AgentModeSeen {
        mode: String,
    },
    /// A row of `claude agents --json`.
    ///
    /// The roster lists **every** live session, interactive ones included — it
    /// is how the board is populated the moment Devplane is installed, before
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
        /// How many commands this session has running right now, from the
        /// process table — the evidence the roster's `status` does not carry.
        ///
        /// **`None` means nobody looked, and never means zero.** The
        /// distinction is the whole value of the field: a `Some(0)` is a
        /// checked, empty answer that may clear a stale job block, and a
        /// `None` must leave what is already known alone. Old stored events
        /// decode as `None`, which is exactly right — they were not checked.
        #[serde(default)]
        jobs: Option<u32>,
    },
    /// A status-line sample: the only channel that carries rate limits.
    StatusSample(StatusSample),
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

/// A tool call named on an event that is not itself a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRef {
    pub tool: String,
    pub input: serde_json::Value,
}

#[cfg(test)]
impl Event {
    /// The channel an event of this kind actually arrives on.
    ///
    /// **For tests only, and it exists because getting this wrong hid three
    /// bugs at once.** Every test envelope in this crate was labelled
    /// `Source::Hook`, including roster samples, which no hook produces. The
    /// reducer stands the roster down as soon as a hook has spoken, so those
    /// tests were asserting against a machine where a channel was connected
    /// that was not — and the roster paths they were meant to cover never ran.
    ///
    /// One reader, in one place: the same rule was written twice in two test
    /// modules before it was put here.
    pub fn test_source(&self) -> Source {
        match self {
            Event::RosterSeen { .. } => Source::AgentsJson,
            Event::StatusSample(_) => Source::StatusLine,
            Event::Stalled { .. } | Event::Lost { .. } | Event::Refresh => Source::Daemon,
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
            Event::PermissionModeSeen { .. } => "permission_mode_seen",
            Event::PermissionDecided { .. } => "permission_decided",
            Event::QuestionAnswered { .. } => "question_answered",
            Event::ConfigChanged { .. } => "config_changed",
            Event::TaskChanged { .. } => "task_changed",
            Event::Blocked { .. } => "blocked",
            Event::QuestionAsked { .. } => "question_asked",
            Event::QuestionEnded { .. } => "question_ended",
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
            Event::AgentModeSeen { .. } => "agent_mode_seen",
            Event::RosterSeen { .. } => "roster_seen",
            Event::StatusSample(_) => "status_sample",
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

impl Event {
    /// A tool call from a source that reports no server provenance.
    ///
    /// Most callers are tests and the two vendors that have no such field, and
    /// spelling `server_source: None` at each of them buries the one place that
    /// does carry it.
    #[must_use]
    pub fn tool_started(tool: impl Into<String>, input: serde_json::Value) -> Self {
        Event::ToolStarted {
            tool: tool.into(),
            input,
            server_source: None,
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

//! Runs — one agent execution, observed or driven.

use crate::core::event::{ApiUsage, Choice, WaitingFor};
use crate::core::ids::{ProjectId, RunId, SessionId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// How much control Vibeplane has over a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Started elsewhere — a terminal, the VS Code extension, the desktop app.
    /// Vibeplane can show and focus it and decide permissions by policy, but
    /// cannot type into it.
    Observed,
    /// An ACP session Vibeplane owns. Full control.
    Driven,
    /// Delegated to a provider daemon (`claude --bg`).
    Background,
}

impl RunMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunMode::Observed => "observed",
            RunMode::Driven => "driven",
            RunMode::Background => "background",
        }
    }
}

/// Lifecycle of a run.
///
/// `Waiting` is the state the whole product exists for: it is the only one that
/// costs the human something. It is entered from `Notification` matchers that
/// fire when the agent is genuinely blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Starting,
    Working,
    Waiting(WaitingFor),
    Idle,
    Completed,
    Failed,
    Stopped,
    /// Recorded as alive, but reconciliation could not find it.
    Lost,
}

impl RunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunState::Starting => "starting",
            RunState::Working => "working",
            RunState::Waiting(_) => "waiting",
            RunState::Idle => "idle",
            RunState::Completed => "completed",
            RunState::Failed => "failed",
            RunState::Stopped => "stopped",
            RunState::Lost => "lost",
        }
    }

    /// Whether the session can still do work. A terminal run is never stalled
    /// and never needs a nudge.
    pub fn is_live(&self) -> bool {
        matches!(
            self,
            RunState::Starting | RunState::Working | RunState::Waiting(_) | RunState::Idle
        )
    }

    /// Whether a person is being waited on.
    ///
    /// Every `Waiting` except `Idle`, including a reason the provider named and
    /// this code does not model: the roster reports `waitingFor` only while a
    /// session is waiting, so a value we do not recognise is still somebody
    /// being waited on. Listing known-good is the wrong default for a signal
    /// whose vocabulary the vendor extends.
    pub fn needs_human(&self) -> bool {
        matches!(self, RunState::Waiting(w) if !matches!(w, WaitingFor::Idle))
    }
}

/// One entry of an agent's plan for the turn.
///
/// What it intends to do, and how far it has got. Structured and low-volume,
/// which is what makes it the one part of a driven agent's output that belongs
/// on the board itself rather than behind a transcript view: "what is it about
/// to do" answered without reading a word of prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub content: String,
    /// `pending`, `in_progress`, `completed`.
    pub status: String,
}

impl PlanStep {
    pub fn is_done(&self) -> bool {
        self.status == "completed"
    }
}

/// A tool call in flight or recently finished, for the transcript preview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub at: Timestamp,
    pub ok: Option<bool>,
}

/// A tool call that was refused, and by what.
///
/// `by` is the same string the decision log records — `policy:Bash(git push *)`
/// when a project rule matched, `claude` when the vendor's own auto mode
/// refused, `human` when a person did — so an item raised from these can say
/// *which* gate is stopping the work rather than only that something is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub tool: String,
    pub by: String,
    pub at: Timestamp,
}

/// Running totals for a run. Cost and tokens come from OpenTelemetry, which is
/// the only documented per-request channel; the context gauge is derived from
/// the last request's input tokens rather than a running sum, because the
/// context window is a level, not a total.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunTotals {
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub api_requests: u64,
    /// Turns this run has completed.
    ///
    /// Counted at the turn boundary rather than from `api_requests`: a driven
    /// run streams several usage updates within one turn, so counting those
    /// would make a turn bound fire far too early.
    pub turns: u64,
    pub tool_calls: u64,
    pub errors: u64,
    /// Context tokens reported by the most recent API request.
    pub last_context_tokens: u64,
    /// The model's window, when known, so a percentage can be shown.
    pub context_window: Option<u64>,
    /// Percentage reported directly by the status line, when the shim is on.
    /// Preferred over the derived figure because it is the provider's own.
    pub reported_context_percent: Option<f64>,
    /// How much of a subscription rate-limit window is used, and which window.
    ///
    /// The status line is the only channel that carries this, which is why the
    /// shim exists. It was parsed, put on an event, and then dropped by the
    /// reducer — so the `rate_limit` inbox kind, the threshold beside it and
    /// half the argument for installing the shim at all were a feature nothing
    /// implemented.
    #[serde(default)]
    pub rate_limit_percent: Option<f64>,
    /// `five_hour` or `seven_day`: which window is the one under pressure.
    #[serde(default)]
    pub rate_limit_window: Option<String>,
}

impl RunTotals {
    /// Context usage as a percentage, from the status line if available and
    /// derived from token counts otherwise. `None` when neither is known.
    pub fn context_percent(&self) -> Option<f64> {
        if let Some(p) = self.reported_context_percent {
            return Some(p);
        }
        let window = self.context_window?;
        if window == 0 || self.last_context_tokens == 0 {
            return None;
        }
        Some((self.last_context_tokens as f64 / window as f64) * 100.0)
    }

    pub fn apply(&mut self, usage: &ApiUsage) {
        self.cost_usd += usage.cost_usd;
        self.input_tokens += usage.input_tokens;
        self.output_tokens += usage.output_tokens;
        self.cache_read_tokens += usage.cache_read_tokens;
        self.api_requests += 1;
        let ctx = usage.context_tokens();
        if ctx > 0 {
            self.last_context_tokens = ctx;
        }
        if self.context_window.is_none() {
            self.context_window = usage.model.as_deref().and_then(context_window_for_model);
        }
    }
}

/// The context window of a model, by name. Only the families we can recognise;
/// an unknown model yields `None` and the gauge says "unknown" rather than
/// inventing a denominator.
pub fn context_window_for_model(model: &str) -> Option<u64> {
    let m = model.to_ascii_lowercase();
    if m.contains("[1m]") || m.contains("-1m") {
        return Some(1_000_000);
    }
    if m.contains("claude") || m.contains("opus") || m.contains("sonnet") || m.contains("haiku") {
        return Some(200_000);
    }
    None
}

/// One agent execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub session_id: SessionId,
    pub project_id: Option<ProjectId>,
    /// The agent that is running: `claude`, or an ACP registry id.
    pub agent: String,
    /// The command line that started it, for a run Vibeplane drives.
    ///
    /// Recorded because the id alone is not always enough to start the agent
    /// again: `vibeplane dispatch --agent /path/to/my-agent` resolves to the id
    /// `custom`, which names nothing in the registry. Without the command, such
    /// a run could be observed and never resumed.
    #[serde(default)]
    pub agent_command: Option<String>,
    pub mode: RunMode,
    pub state: RunState,
    pub cwd: PathBuf,
    /// Set while the session is inside a worktree.
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    pub model: Option<String>,
    /// OpenTelemetry's `app.entrypoint`: which surface started this session.
    pub entrypoint: Option<String>,
    /// A name the provider gave the session, when it has one.
    pub name: Option<String>,
    /// The id the *agent* knows this conversation by, for a driven run.
    ///
    /// Distinct from `id`, which is Vibeplane's, and from `session_id`, which
    /// for an observed session is the provider's. This is the one an agent will
    /// accept on `session/resume`, so it is what makes a run survive a restart
    /// of the daemon rather than only a row about it.
    #[serde(default)]
    pub agent_session: Option<String>,
    pub pid: Option<u32>,
    pub started_at: Timestamp,
    /// Last event of any kind. Drives "last seen" in the UI.
    pub last_event_at: Timestamp,
    /// Last event that counted as activity. Drives the stall timer.
    pub last_activity_at: Timestamp,
    pub totals: RunTotals,
    /// The question or permission the run is blocked on, for the inbox card.
    pub blocked_on: Option<BlockedOn>,
    /// Recent tool calls, newest last, capped.
    pub recent_tools: Vec<ToolCall>,
    /// Tool calls refused in this run, and the most recent refusal.
    ///
    /// Counted because a refused agent does not stop, it tries something else —
    /// so a rule that is too tight and one that is working look identical, and
    /// the difference shows up on the bill. See `AttentionKind::Refused`.
    #[serde(default)]
    pub refusals: u64,
    #[serde(default)]
    pub last_refusal: Option<Refusal>,
    /// What the agent says it is going to do, where it reports a plan. Replaced
    /// wholesale on each update, because the protocol sends the whole list.
    #[serde(default)]
    pub plan: Vec<PlanStep>,
    /// Live subagents by id.
    pub subagents: BTreeMap<String, Option<String>>,
    /// A one-line summary of what the run is doing.
    pub summary: Option<String>,
    /// Set when a stall has already been recorded for the current quiet
    /// period, so one silent night is one event rather than one a minute.
    #[serde(default)]
    pub stall_noticed: bool,
    /// Whether this session has ever told us anything itself.
    ///
    /// A session discovered in the roster is a process that exists. That is not
    /// the same as a session somebody is using: an editor tab left open for
    /// three days is also a process that exists. Until a hook, telemetry or a
    /// live status arrives, the run is dormant and stays out of the way.
    #[serde(default)]
    pub reporting: bool,
    /// Which of this run's inbox items are hidden, and until when.
    ///
    /// Per *kind*, not per run: "not this one, not now" is a thought about the
    /// thing on screen, and a blanket snooze on the run swallowed whatever
    /// arrived next — including the permission request that is the one item
    /// this product exists to deliver. See [`Snoozed`](crate::core::attention::Snoozed).
    #[serde(default)]
    pub snoozed: crate::core::attention::Snoozed,
}

/// What a blocked run is waiting for, with enough detail to decide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedOn {
    pub waiting_for: WaitingFor,
    pub message: Option<String>,
    /// Set when this can be answered from Vibeplane.
    #[serde(default)]
    pub request_id: Option<String>,
    pub tool: Option<String>,
    pub input: Option<serde_json::Value>,
    /// What the run will accept as an answer, with the ids the protocol needs.
    pub options: Vec<Choice>,
    pub since: Timestamp,
}

impl Run {
    pub fn new(session_id: SessionId, cwd: PathBuf, mode: RunMode, agent: &str) -> Self {
        let now = Timestamp::now();
        Self {
            id: RunId::from_session(&session_id),
            session_id,
            agent_session: None,
            project_id: None,
            agent: agent.to_string(),
            agent_command: None,
            mode,
            state: RunState::Starting,
            cwd,
            worktree: None,
            branch: None,
            model: None,
            entrypoint: None,
            name: None,
            pid: None,
            started_at: now,
            last_event_at: now,
            last_activity_at: now,
            totals: RunTotals::default(),
            refusals: 0,
            last_refusal: None,
            blocked_on: None,
            recent_tools: Vec::new(),
            plan: Vec::new(),
            subagents: BTreeMap::new(),
            summary: None,
            stall_noticed: false,
            reporting: false,
            snoozed: Default::default(),
        }
    }

    /// The directory the run is actually working in.
    pub fn working_dir(&self) -> &PathBuf {
        self.worktree.as_ref().unwrap_or(&self.cwd)
    }

    /// Seconds since the last activity, for the stall timer and the UI.
    pub fn idle_seconds(&self) -> i64 {
        (Timestamp::now() - self.last_activity_at).get_seconds()
    }

    /// Whether this run belongs in the working set: something is happening, or
    /// somebody is being asked for something.
    ///
    /// The distinction matters at scale. Twenty-two rows that all look equally
    /// alive answer no question at all; five that are actually in play answer
    /// the only one the board exists for.
    pub fn is_active(&self) -> bool {
        self.state.needs_human()
            || matches!(self.state, RunState::Working | RunState::Starting)
            || matches!(self.state, RunState::Failed | RunState::Lost)
            || self.reporting
    }

    /// How far through its own plan the agent says it is, as `done/total`.
    /// `None` when it reports no plan, which most agents do not.
    pub fn plan_progress(&self) -> Option<(usize, usize)> {
        match self.plan.is_empty() {
            true => None,
            false => Some((
                self.plan.iter().filter(|s| s.is_done()).count(),
                self.plan.len(),
            )),
        }
    }

    /// Whether anything about this run is currently hidden. The board's
    /// marker; the inbox asks per kind instead.
    pub fn is_snoozed(&self) -> bool {
        self.snoozed.any()
    }
}

//! Runs — one agent execution, observed or driven.

use crate::event::{ApiUsage, WaitingFor};
use crate::ids::{ProjectId, RunId, SessionId};
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
    /// A cloud run, observed through its pull request only.
    Remote,
}

impl RunMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunMode::Observed => "observed",
            RunMode::Driven => "driven",
            RunMode::Background => "background",
            RunMode::Remote => "remote",
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

    pub fn needs_human(&self) -> bool {
        matches!(
            self,
            RunState::Waiting(WaitingFor::Permission) | RunState::Waiting(WaitingFor::Question)
        )
    }
}

/// A tool call in flight or recently finished, for the transcript preview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub at: Timestamp,
    pub ok: Option<bool>,
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
    pub tool_calls: u64,
    pub errors: u64,
    /// Context tokens reported by the most recent API request.
    pub last_context_tokens: u64,
    /// The model's window, when known, so a percentage can be shown.
    pub context_window: Option<u64>,
    /// Percentage reported directly by the status line, when the shim is on.
    /// Preferred over the derived figure because it is the provider's own.
    pub reported_context_percent: Option<f64>,
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
    /// Live subagents by id.
    pub subagents: BTreeMap<String, Option<String>>,
    /// A one-line summary of what the run is doing.
    pub summary: Option<String>,
}

/// What a blocked run is waiting for, with enough detail to decide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedOn {
    pub waiting_for: WaitingFor,
    pub message: Option<String>,
    pub tool: Option<String>,
    pub input: Option<serde_json::Value>,
    pub options: Vec<String>,
    pub since: Timestamp,
}

impl Run {
    pub fn new(session_id: SessionId, cwd: PathBuf, mode: RunMode, agent: &str) -> Self {
        let now = Timestamp::now();
        Self {
            id: RunId::from_session(&session_id),
            session_id,
            project_id: None,
            agent: agent.to_string(),
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
            blocked_on: None,
            recent_tools: Vec::new(),
            subagents: BTreeMap::new(),
            summary: None,
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
}

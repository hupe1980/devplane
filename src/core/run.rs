//! Runs — one agent execution, observed or driven.

use crate::core::event::{ApiUsage, Choice, WaitingFor};
use crate::core::ids::{ProjectId, RunId, SessionId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The permission mode a watched session is running in, as its vendor reports it.
///
/// The mode labelled Manual arrives on the wire as `default`, never `manual`.
/// [`Self::Unrecognised`] keeps unknown text rather than collapsing into
/// `Default`: an unknown mode has unknown supervision, and reading it as the
/// most supervised value would widen by silence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Shown as **Manual**. Arrives as `default`.
    Default,
    /// Reads and plans; changes nothing until the plan is accepted.
    Plan,
    /// Edits go through without asking. Other tools still ask.
    AcceptEdits,
    /// A classifier decides, not a person. The default on Pro, Max and Team.
    Auto,
    /// Nothing is asked.
    DontAsk,
    /// The permission system is off.
    BypassPermissions,
    /// A value this build does not know, kept verbatim.
    Unrecognised(String),
}

impl PermissionMode {
    /// From the wire. Never guesses: an unknown string is kept as one.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "default" => Self::Default,
            "plan" => Self::Plan,
            "acceptEdits" => Self::AcceptEdits,
            "auto" => Self::Auto,
            "dontAsk" => Self::DontAsk,
            "bypassPermissions" => Self::BypassPermissions,
            other => Self::Unrecognised(other.to_string()),
        }
    }

    /// The vendor's own spelling, so a value round-trips.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Default => "default",
            Self::Plan => "plan",
            Self::AcceptEdits => "acceptEdits",
            Self::Auto => "auto",
            Self::DontAsk => "dontAsk",
            Self::BypassPermissions => "bypassPermissions",
            Self::Unrecognised(raw) => raw,
        }
    }

    /// What a person reads. `default` is shown as the vendor labels it.
    pub fn label(&self) -> &str {
        match self {
            Self::Default => "manual",
            Self::Plan => "plan",
            Self::AcceptEdits => "accept edits",
            Self::Auto => "auto",
            Self::DontAsk => "don't ask",
            Self::BypassPermissions => "no permissions",
            Self::Unrecognised(raw) => raw,
        }
    }

    /// Whether a person is in the loop for an ordinary tool call. `None` (not
    /// `false`) for an unrecognised mode: the honest answer is *unknown*.
    pub fn asks_a_person(&self) -> Option<bool> {
        match self {
            Self::Default | Self::Plan => Some(true),
            // `acceptEdits` still asks about everything that is not an edit.
            Self::AcceptEdits => Some(true),
            Self::Auto | Self::DontAsk | Self::BypassPermissions => Some(false),
            Self::Unrecognised(_) => None,
        }
    }
}

/// How much control Devplane has over a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Started elsewhere (terminal, IDE, desktop app). Devplane can show it and
    /// decide permissions by policy, but cannot type into it.
    Observed,
    /// An ACP session Devplane owns. Full control.
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

/// Lifecycle of a run. `Waiting` is the only state that costs a person
/// something.
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
    /// Devplane stopped this run because the host was shutting down. Not
    /// `Stopped` (a person), not `Lost` (cause unknown), never `Completed`.
    Interrupted,
    /// Recorded as alive, but reconciliation could not find it.
    Lost,
}

/// How a message to a run reaches it. ACP takes one prompt per session at a
/// time, so a message sent mid-turn waits for the turn to end rather than
/// interrupting it; it is recorded the moment it is sent either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Queued,
    Sent,
}

impl Delivery {
    /// Queued while a turn is under way — starting, working, or waiting on a
    /// person mid-turn; sent otherwise.
    pub fn of(state: &RunState) -> Self {
        match state {
            RunState::Starting | RunState::Working | RunState::Waiting(_) => Delivery::Queued,
            _ => Delivery::Sent,
        }
    }

    pub fn says(self) -> &'static str {
        match self {
            Delivery::Queued => "queued — delivered when the current turn ends",
            Delivery::Sent => "sent",
        }
    }
}

/// What an agent's own session listing says about one session. A roster, never
/// an attention signal: ACP's `SessionInfo` has no state field, so a listing
/// can never say a session is waiting for somebody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedSession {
    /// The id the agent knows this conversation by.
    pub agent_session: String,
    pub title: Option<String>,
}

/// How long after its last activity a session is still part of the working set:
/// roughly a working day. A judgement, not a setting.
pub const IN_PLAY_SECONDS: i64 = 6 * 3600;

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
            RunState::Interrupted => "interrupted",
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

    /// Whether a person is being waited on: every `Waiting` except `Idle` and
    /// `Job`, including reasons this code does not model, since the vendor
    /// extends that vocabulary.
    pub fn needs_human(&self) -> bool {
        matches!(self, RunState::Waiting(w) if !matches!(w, WaitingFor::Idle | WaitingFor::Job))
    }

    /// Whether the session is waiting on a command it started rather than on a
    /// person: nothing is owed, but the row stays on the board while it runs.
    pub fn waits_on_a_job(&self) -> bool {
        matches!(self, RunState::Waiting(WaitingFor::Job))
    }
}

/// One entry of an agent's plan for the turn: what it intends to do and how
/// far it has got.
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
    /// What the tool was asked to do; dropped the moment the call finishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    /// The protocol's id, where the source had one; the finish is matched on
    /// it rather than on the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
}

/// A tool call that was refused, and by what. `by` is the decision log's string
/// (`policy:Bash(git push *)`, `claude`, `human`), so an item can name the gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub tool: String,
    pub by: String,
    /// What the rule said, when it said anything.
    #[serde(default)]
    pub reason: Option<String>,
    pub at: Timestamp,
}

/// Running totals for a run. Cost and tokens come from OpenTelemetry; the
/// context gauge uses the last request's input tokens, since context is a
/// level, not a total.
// `default` on the struct: a field added later must not make rows an earlier
// build wrote unreadable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunTotals {
    pub cost_usd: f64,
    /// Whether any channel (telemetry or status line) has stated a cost.
    /// `api_requests` counts only telemetry, so it cannot answer this.
    #[serde(default)]
    pub cost_reported: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub api_requests: u64,
    /// Turns completed, counted at the turn boundary: a driven run streams
    /// several usage updates per turn.
    pub turns: u64,
    pub tool_calls: u64,
    pub errors: u64,
    /// Context tokens reported by the most recent API request.
    pub last_context_tokens: u64,
    /// The model's window, when known, so a percentage can be shown.
    pub context_window: Option<u64>,
    /// Percentage from the status line; preferred over the derived figure.
    pub reported_context_percent: Option<f64>,
    /// How much of a subscription rate-limit window is used. Only the status
    /// line carries this.
    #[serde(default)]
    pub rate_limit_percent: Option<f64>,
    /// `five_hour`, `seven_day` or `spend_limit`.
    #[serde(default)]
    pub rate_limit_window: Option<String>,
    /// When that window resets, in Unix epoch seconds.
    #[serde(default)]
    pub rate_limit_resets_at: Option<i64>,
    /// Lines the session added and removed, as the provider counts them.
    #[serde(default)]
    pub lines_added: u64,
    #[serde(default)]
    pub lines_removed: u64,
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
        self.cost_reported = true;
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

/// The context window of a model, by name; `None` for an unknown model rather
/// than an invented denominator.
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
    /// The command line that started a driven run. Needed to resume one whose
    /// agent id is `custom`, which names nothing in the registry.
    #[serde(default)]
    pub agent_command: Option<String>,
    pub mode: RunMode,
    pub state: RunState,
    pub cwd: PathBuf,
    /// Set while the session is inside a worktree.
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    pub model: Option<String>,
    /// The Claude Code release this session runs, from the status line only.
    /// `doctor` counts sessions reporting it: *nothing reporting* is not
    /// *nothing wrong*.
    #[serde(default)]
    pub claude_version: Option<String>,
    /// The permission mode this session last reported, and when Devplane first
    /// saw it. No hook announces a mode change, so surfaces say "seen", never
    /// "since".
    pub permission_mode: Option<PermissionMode>,
    pub permission_mode_seen: Option<Timestamp>,
    /// A clock this session's environment put on its questions. Per session:
    /// `CLAUDE_AFK_TIMEOUT_MS` overrides the settings files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_clock: Option<crate::core::clock::QuestionClock>,
    /// Questions this session asked that nobody answered. The next tool call
    /// clears `blocked_on`, so without this *answered* and *given up on* would
    /// be indistinguishable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abandoned_questions: Vec<AbandonedQuestion>,
    /// How many fell off the end of that bounded list.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub abandoned_dropped: u32,
    /// When the environment was read; `None` means never. *Not read* and
    /// *read, nothing set* are different facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_clock_read: Option<Timestamp>,
    /// The mode an ACP agent declared, in its own spelling. Kept apart from
    /// `permission_mode`, a documented vendor mode: no spec makes them equivalent.
    pub agent_mode: Option<String>,
    pub agent_mode_seen: Option<Timestamp>,
    /// OpenTelemetry's `app.entrypoint`: which surface started this session.
    pub entrypoint: Option<String>,
    /// A name the provider gave the session, when it has one.
    pub name: Option<String>,
    /// The id the agent knows a driven run by: what `session/resume` accepts,
    /// so the run survives a host restart.
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
    /// Tool calls refused in this run. A refused agent tries something else,
    /// so a too-tight rule is only visible as a count. See `AttentionKind::Refused`.
    #[serde(default)]
    pub refusals: u64,
    #[serde(default)]
    pub last_refusal: Option<Refusal>,
    /// The agent's reported plan, replaced wholesale on each update.
    #[serde(default)]
    pub plan: Vec<PlanStep>,
    /// Live subagents by id.
    pub subagents: BTreeMap<String, Option<String>>,
    /// A one-line summary of what the run is doing.
    pub summary: Option<String>,
    /// Set once a stall is recorded for the current quiet period, so one
    /// silent night is one event.
    #[serde(default)]
    pub stall_noticed: bool,
    /// Whether this session has ever told us anything itself. A roster entry
    /// alone may be an idle editor tab, so the run stays dormant until then.
    #[serde(default)]
    pub reporting: bool,
    /// When a hook last spoke about this session. Where a hook has spoken its
    /// word on state wins over the roster's late, coarse sample; otherwise the
    /// roster is believed.
    #[serde(default)]
    pub last_hook_at: Option<Timestamp>,
    /// Whether this session ever produced an event that *is* activity (hook,
    /// telemetry, driven turn). Unlike [`reporting`](Self::reporting), which
    /// the roster sets, this is what makes the quiet clock meaningful.
    #[serde(default)]
    pub activity_seen: bool,
    /// Which of this run's inbox items are hidden, and until when. Per kind,
    /// so a snooze never swallows the next permission request.
    #[serde(default)]
    pub snoozed: crate::core::attention::Snoozed,
    /// The tasks Devplane sent this run. `None` means nothing was ever sent,
    /// never an empty list standing in for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent: Option<Vec<crate::spec::SentTask>>,
    /// What the specification said when this run last closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<Observed>,
    /// Files this run's editing tool calls named, repository-relative, in
    /// first-seen order. Read as calls start, so refused edits are included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wrote: Vec<String>,
    /// The `seq` of the last stored event folded into this run. Projection is
    /// at-least-once and reducers count, so events at or below it are skipped.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub last_seq: i64,
}

fn is_zero_i64(n: &i64) -> bool {
    *n == 0
}

/// The specification as the host read it when a run closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observed {
    /// Of the folder the person edits; `None` when it was not there.
    pub fingerprint: Option<String>,
    /// The keys of every ticked task in the task file, as the run left it.
    pub ticked: Vec<String>,
    /// The newest document's modification time.
    pub changed_at: Option<Timestamp>,
    /// When this was taken — the run's close, for the counts.
    pub at: Timestamp,
}

/// A question an agent asked and then moved past. Not answerable: the tool call
/// is over, so the type has nowhere to put an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct AbandonedQuestion {
    /// As the agent wrote it. Rendered, never summarised.
    pub question: String,
    /// The options it offered, so a person can see what it chose between.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<crate::core::event::Choice>,
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub asked_at: Timestamp,
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub abandoned_at: Timestamp,
    /// What the agent did instead, where that is the next thing it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_on_to: Option<String>,
    pub ended_by: EndedBy,
}

/// How a question came to be abandoned: derived from hook events, or asserted
/// by the vendor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum EndedBy {
    /// Derived: another tool call started while the question stood.
    MovedOn,
    /// Derived: the session ended with the question still open.
    SessionEnded,
    /// Asserted by the vendor, e.g. OpenCode's `question.rejected`.
    VendorRejected,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// What a blocked run is waiting for, with enough detail to decide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedOn {
    pub waiting_for: WaitingFor,
    pub message: Option<String>,
    /// Set when this can be answered from Devplane.
    #[serde(default)]
    pub request_id: Option<String>,
    /// The durable ask: what a surface addresses an answer to. Unlike
    /// `request_id`, it outlives the connection that asked.
    #[serde(default)]
    pub ask: Option<crate::core::AskId>,
    pub tool: Option<String>,
    pub input: Option<serde_json::Value>,
    /// What the run will accept as an answer, with the ids the protocol needs.
    pub options: Vec<Choice>,
    /// The whole form, for several questions or free text; answers are
    /// validated against it. `options` is its first question flattened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<serde_json::Value>,
    pub since: Timestamp,
}

impl Run {
    /// Fills a missing name from the agent's own session listing. Observation
    /// outranks the listing: only gaps are filled, state is never touched, and
    /// a blank title is refused.
    pub fn enrich_from_listing(&mut self, listed: &ListedSession) -> bool {
        if self.agent_session.as_deref() != Some(listed.agent_session.as_str()) {
            return false;
        }
        let Some(title) = listed.title.as_deref() else {
            return false;
        };
        let title = title.trim();
        if title.is_empty() || self.name.is_some() {
            return false;
        }
        self.name = Some(title.to_string());
        true
    }

    /// A run first seen at `now`. [`Run::new`] (outside the pure half)
    /// stamps the wall clock.
    pub fn new_at(
        session_id: SessionId,
        cwd: PathBuf,
        mode: RunMode,
        agent: &str,
        now: Timestamp,
    ) -> Self {
        Self {
            id: RunId::from_session(&session_id),
            session_id,
            last_hook_at: None,
            agent_session: None,
            project_id: None,
            agent: agent.to_string(),
            agent_command: None,
            mode,
            state: RunState::Starting,
            // Unknown until a hook carrying it arrives (`PreToolUse` does not).
            permission_mode: None,
            permission_mode_seen: None,
            question_clock: None,
            question_clock_read: None,
            abandoned_questions: Vec::new(),
            abandoned_dropped: 0,
            agent_mode: None,
            agent_mode_seen: None,
            cwd,
            worktree: None,
            branch: None,
            model: None,
            claude_version: None,
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
            activity_seen: false,
            snoozed: Default::default(),
            sent: None,
            observed: None,
            wrote: Vec::new(),
            last_seq: 0,
        }
    }

    /// Records a file an editing tool call named, relative to where the run
    /// works. An absolute path under that directory loses the prefix; one
    /// outside it is kept whole, where it can match no file of the change.
    pub fn note_written(&mut self, tool: &str, input: &serde_json::Value) {
        if !matches!(tool, "Edit" | "Write" | "MultiEdit" | "NotebookEdit") {
            return;
        }
        let Some(raw) = ["file_path", "notebook_path", "path"]
            .iter()
            .find_map(|k| input.get(*k).and_then(|v| v.as_str()))
        else {
            return;
        };
        let path = std::path::Path::new(raw);
        let rel = match path.is_absolute() {
            true => [self.worktree.as_ref(), Some(&self.cwd)]
                .into_iter()
                .flatten()
                .find_map(|root| path.strip_prefix(root).ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| raw.to_string()),
            false => raw.trim_start_matches("./").to_string(),
        };
        if !rel.is_empty() && !self.wrote.contains(&rel) {
            self.wrote.push(rel);
        }
    }

    /// The directory the run is actually working in.
    pub fn working_dir(&self) -> &PathBuf {
        self.worktree.as_ref().unwrap_or(&self.cwd)
    }

    /// Seconds since the last activity as of `now`, for the stall timer and
    /// the UI.
    pub fn idle_seconds_at(&self, now: Timestamp) -> i64 {
        (now - self.last_activity_at).get_seconds()
    }

    /// Whether this run belongs in the working set. A session asking for
    /// something, working, or waiting on a job never ages out; anything else
    /// that reported, failed or was lost stays for [`IN_PLAY_SECONDS`] and is
    /// then counted rather than listed (`--all` lists it).
    pub fn is_active_at(&self, now: Timestamp) -> bool {
        if self.state.needs_human()
            || matches!(self.state, RunState::Working | RunState::Starting)
            || self.state.waits_on_a_job()
        {
            return true;
        }
        let recent = self.idle_seconds_at(now) < IN_PLAY_SECONDS;
        recent && (self.reporting || matches!(self.state, RunState::Failed | RunState::Lost))
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

    /// The one sentence every surface shows about the tasks this run carries.
    /// A watched run says it has none rather than guessing from the diff.
    pub fn sent_says(&self) -> String {
        match (&self.sent, self.mode) {
            (Some(sent), _) => match sent.len() {
                1 => "sent 1 task".to_string(),
                n => format!("sent {n} tasks"),
            },
            (None, RunMode::Observed) => {
                "watched, not dispatched by Devplane — it carries no task edges".to_string()
            }
            (None, _) => "no tasks were sent to this run".to_string(),
        }
    }
}

#[cfg(test)]
mod permission_mode_tests {
    use super::PermissionMode;

    /// `default` is the one labelled Manual; `manual` is not a wire value.
    #[test]
    fn the_wire_spelling_is_the_vendors_and_manual_is_not_one() {
        assert_eq!(PermissionMode::parse("default"), PermissionMode::Default);
        assert_eq!(PermissionMode::Default.label(), "manual");
        assert_eq!(PermissionMode::Default.as_str(), "default");
        // Kept verbatim, not mapped onto `Default`.
        assert!(matches!(
            PermissionMode::parse("manual"),
            PermissionMode::Unrecognised(_)
        ));
        // camelCase, not snake: `acceptEdits`, not `accept_edits`.
        assert_eq!(
            PermissionMode::parse("acceptEdits"),
            PermissionMode::AcceptEdits
        );
        assert!(matches!(
            PermissionMode::parse("accept_edits"),
            PermissionMode::Unrecognised(_)
        ));
    }

    /// An unknown mode is not "supervised": it answers neither yes nor no.
    #[test]
    fn an_unknown_mode_answers_neither_yes_nor_no() {
        assert_eq!(PermissionMode::Default.asks_a_person(), Some(true));
        assert_eq!(PermissionMode::Auto.asks_a_person(), Some(false));
        assert_eq!(PermissionMode::DontAsk.asks_a_person(), Some(false));
        assert_eq!(
            PermissionMode::BypassPermissions.asks_a_person(),
            Some(false)
        );
        assert_eq!(
            PermissionMode::parse("hypervigilant").asks_a_person(),
            None,
            "an unrecognised mode must not answer this question at all"
        );
    }

    #[test]
    fn every_mode_round_trips_through_its_wire_spelling() {
        for raw in [
            "default",
            "plan",
            "acceptEdits",
            "auto",
            "dontAsk",
            "bypassPermissions",
            "something-new-in-2027",
        ] {
            assert_eq!(PermissionMode::parse(raw).as_str(), raw, "{raw}");
        }
    }

    #[test]
    fn no_two_modes_share_a_label() {
        let all = [
            PermissionMode::Default,
            PermissionMode::Plan,
            PermissionMode::AcceptEdits,
            PermissionMode::Auto,
            PermissionMode::DontAsk,
            PermissionMode::BypassPermissions,
        ];
        let mut labels: Vec<&str> = all.iter().map(|m| m.label()).collect();
        labels.sort_unstable();
        let before = labels.len();
        labels.dedup();
        assert_eq!(
            before,
            labels.len(),
            "two modes render the same: {labels:?}"
        );
    }
}

/// What one agent advertised at `initialize` the last time Devplane started it,
/// dated. A missing record means *not probed*, never *not supported*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilityRecord {
    /// What was actually run; keyed on this because two registry entries can
    /// share one binary.
    pub command: String,
    /// What the agent calls itself, where it says.
    pub agent_name: Option<String>,
    /// `session/resume` — continue without replaying.
    pub resume: bool,
    /// `session/load` — continue *with* a replay (Copilot has only this one).
    pub load_session: bool,
    /// `session/list` — the agent can enumerate its own sessions.
    pub list_sessions: bool,
    /// Whether the session response carried a mode; the protocol has no
    /// capability flag for modes.
    pub declares_modes: bool,
    /// Whether the agent advertised any authentication method.
    pub needs_auth: bool,
    pub measured_at: Timestamp,
}

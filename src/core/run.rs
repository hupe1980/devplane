//! Runs — one agent execution, observed or driven.

use crate::core::event::{ApiUsage, Choice, WaitingFor};
use crate::core::ids::{ProjectId, RunId, SessionId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The permission mode a watched session is running in, as its vendor reports it.
///
/// **This is the one thing a person with six repositories cannot find out
/// today**, and it is the first slice of the seat: not *what was decided
/// without you*, which measured as everything, but *which of my projects is
/// deciding without me at all*.
///
/// Six values because the vendor documents six. The one labelled **Manual** in
/// the interface arrives on the wire as `default`, never as `manual`, which is
/// exactly the sort of detail that makes a hand-rolled string comparison wrong
/// in a way nobody notices.
///
/// [`Self::Unrecognised`] carries the text rather than collapsing into
/// `Default`. A mode this build has never heard of is a mode whose supervision
/// properties are unknown, and reading it as the vendor's *most* supervised
/// value is the widening-by-silence failure this project keeps finding in
/// itself: the screen would say `default` about a session running something
/// stricter or looser, and nothing would be wrong anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Shown as **Manual**. Arrives as `default`.
    Default,
    /// Reads and plans; changes nothing until the plan is accepted.
    Plan,
    /// Edits go through without asking. Other tools still ask.
    AcceptEdits,
    /// **A classifier decides**, not a person. The built-in starting mode on
    /// Pro, Max and Team, which is the whole reason this type exists.
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

    /// Whether a person is in the loop for an ordinary tool call.
    ///
    /// `None` for a mode this build does not recognise — **not `false`**. The
    /// question "is anybody supervising this" has three answers and the third
    /// is *we do not know*, which is the one a row has to be able to show.
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
    /// Started elsewhere — a terminal, the VS Code extension, the desktop app.
    /// Devplane can show and focus it and decide permissions by policy, but
    /// cannot type into it.
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
    /// Devplane stopped this run, because the daemon was shutting down.
    ///
    /// Distinct from `Stopped`, which is a person, and from `Lost`, which is a
    /// process that was expected and not found. Here the cause is known and it
    /// is us — and it is never `Completed`, because the work did not finish.
    Interrupted,
    /// Recorded as alive, but reconciliation could not find it.
    Lost,
}

/// How long after its last activity a session is still part of the working set.
///
/// A working day, near enough: a session you touched this morning is still
/// yours to finish, and one quiet since yesterday is an editor tab. It is a
/// judgement rather than a measurement, which is why it is one constant with
/// one name rather than a setting nobody would tune.
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

    /// Whether a person is being waited on.
    ///
    /// Every `Waiting` except `Idle`, including a reason the provider named and
    /// this code does not model: the roster reports `waitingFor` only while a
    /// session is waiting, so a value we do not recognise is still somebody
    /// being waited on. Listing known-good is the wrong default for a signal
    /// whose vocabulary the vendor extends.
    pub fn needs_human(&self) -> bool {
        matches!(self, RunState::Waiting(w) if !matches!(w, WaitingFor::Idle | WaitingFor::Job))
    }

    /// Whether the session is waiting on a command it started rather than on a
    /// person. Kept separate from [`needs_human`](Self::needs_human) because
    /// the two answers differ: nothing is owed, and the row still belongs on
    /// the board for as long as the job runs.
    pub fn waits_on_a_job(&self) -> bool {
        matches!(self, RunState::Waiting(WaitingFor::Job))
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
    /// What the tool was asked to do, kept only while the call is in flight.
    ///
    /// **`BlockedOn::input` claimed to carry this and never did**: every
    /// construction site in the reducer wrote `None`, so the one consumer — the
    /// rule offered on a permission item — was dead for every session the
    /// product watches rather than drives. A field nothing fills is worse than
    /// a missing one, because the code above it reads as working.
    ///
    /// Dropped the moment the call finishes, so this is one input per in-flight
    /// call rather than a log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
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
// `default` on the struct: a run row is a projection of the event log, kept so
// the board is on screen before a replay finishes, and a field added in a later
// build must not make every row an earlier build wrote unreadable — fourteen
// were, silently, until `doctor` was asked.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// `five_hour`, `seven_day` or `spend_limit`: which window is under
    /// pressure.
    #[serde(default)]
    pub rate_limit_window: Option<String>,
    /// When that window resets, in Unix epoch seconds — what makes the
    /// percentage actionable.
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
    /// The command line that started it, for a run Devplane drives.
    ///
    /// Recorded because the id alone is not always enough to start the agent
    /// again: `devplane dispatch --agent /path/to/my-agent` resolves to the id
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
    /// The Claude Code release **this session** is running.
    ///
    /// Only the status line reports it, so it is absent without the shim.
    ///
    /// It existed for one question — how far past the gate's measured baseline
    /// this session had drifted — and that baseline was deleted with the claim
    /// it protected. What it is still for is smaller and honest: `doctor`
    /// reports how many live sessions are telling us what they run, because
    /// *nothing is reporting* and *nothing is wrong* are different answers.
    #[serde(default)]
    pub claude_version: Option<String>,
    /// The permission mode this session last reported, and when we first saw
    /// it at that value.
    ///
    /// **Observed, never subscribed.** No hook announces a mode change, so the
    /// timestamp is *when Devplane first heard this mode*, which is at best the
    /// person's next prompt after they switched. Anything on screen has to say
    /// "seen" rather than "since", because "in auto since 09:14" would be a
    /// claim about a moment nothing here witnessed.
    pub permission_mode: Option<PermissionMode>,
    pub permission_mode_seen: Option<Timestamp>,
    /// The mode an **ACP agent** declared for this session, in the agent's own
    /// spelling.
    ///
    /// Kept apart from `permission_mode` on purpose. That one is a vendor's
    /// documented mode, read from a named settings file, with an authority
    /// behind it. This one is a string an agent chose, and the two are only
    /// superficially the same question — presenting them under one heading
    /// would assert an equivalence no specification supports.
    pub agent_mode: Option<String>,
    pub agent_mode_seen: Option<Timestamp>,
    /// OpenTelemetry's `app.entrypoint`: which surface started this session.
    pub entrypoint: Option<String>,
    /// A name the provider gave the session, when it has one.
    pub name: Option<String>,
    /// The id the *agent* knows this conversation by, for a driven run.
    ///
    /// Distinct from `id`, which is Devplane's, and from `session_id`, which
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
    /// When a hook last spoke about this session, if one ever has.
    ///
    /// **It decides whose word counts about the state.** A hook reports an
    /// event at the moment it happens and carries the reason — which tool,
    /// which question, which request id to answer by. The roster reports a
    /// coarse sample two seconds late. Where both exist the hook wins, and
    /// where only the roster exists it must be believed rather than consulted
    /// once and then ignored, which is what it was.
    #[serde(default)]
    pub last_hook_at: Option<Timestamp>,
    /// Whether this session has ever produced an event that *is* activity — a
    /// hook, a telemetry record, a turn of a driven run.
    ///
    /// Not the same question as [`reporting`](Self::reporting), and conflating
    /// the two put a contradiction on the screen: the roster sets `reporting`
    /// when it gives a session a status, so a machine with no hooks installed
    /// had a session the board showed as `busy` and the inbox showed as
    /// *"No activity for 519 min"*. There had been no activity because there
    /// was no channel carrying any — which is a fact about the installation,
    /// not about the session.
    ///
    /// The quiet clock only means something for a session that would have said
    /// so, which is what this flag answers.
    #[serde(default)]
    pub activity_seen: bool,
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
    /// Set when this can be answered from Devplane.
    #[serde(default)]
    pub request_id: Option<String>,
    /// The durable ask, when there is one.
    ///
    /// **The token a surface offers, and `request_id` is not.** A request id
    /// means something only to the connection that issued it; this one is what
    /// an answer is addressed to from any surface at any later time, including
    /// after the process that asked has gone.
    #[serde(default)]
    pub ask: Option<crate::core::AskId>,
    pub tool: Option<String>,
    pub input: Option<serde_json::Value>,
    /// What the run will accept as an answer, with the ids the protocol needs.
    pub options: Vec<Choice>,
    /// The whole form, where the agent asked several questions at once or
    /// offered a free-text box. `options` is the first question flattened for
    /// surfaces that show one line; this is what an answer is validated
    /// against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<serde_json::Value>,
    pub since: Timestamp,
}

impl Run {
    pub fn new(session_id: SessionId, cwd: PathBuf, mode: RunMode, agent: &str) -> Self {
        let now = Timestamp::now();
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
            // Unknown until a hook that carries it arrives. `PreToolUse` is
            // not one of them, whatever it feels like it should be.
            permission_mode: None,
            permission_mode_seen: None,
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

    /// Whether this run belongs in the working set: something is happening,
    /// somebody is being asked for something, or it went wrong or quiet
    /// recently enough to still be today's business.
    ///
    /// The distinction matters at scale, and getting it wrong is not a matter
    /// of taste. This read `|| self.reporting`, which is true of every session
    /// the roster ever gave a status to — so a machine with one session in
    /// play showed **thirty-eight rows**, twenty-five of them editor tabs
    /// reading "waiting for a prompt" since Tuesday. A board that lists the
    /// inventory answers no question at all.
    ///
    /// Two things never age out, because both are somebody waiting: a session
    /// asking for something, and a session doing something. Everything else —
    /// idle, failed, lost — is today's business for [`IN_PLAY_SECONDS`] and
    /// history afterwards, counted rather than listed (`--all` lists them).
    pub fn is_active(&self) -> bool {
        if self.state.needs_human()
            || matches!(self.state, RunState::Working | RunState::Starting)
            // **A running job is something happening**, so it never ages out.
            // Without this a four-hour test suite drops off the board after
            // six, and the session most worth watching is the one that
            // vanishes — the person comes back to a finished run they were
            // never shown.
            || self.state.waits_on_a_job()
        {
            return true;
        }
        let recent = self.idle_seconds() < IN_PLAY_SECONDS;
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

    /// Whether anything about this run is currently hidden. The board's
    /// marker; the inbox asks per kind instead.
    pub fn is_snoozed(&self) -> bool {
        self.snoozed.any()
    }
}

#[cfg(test)]
mod permission_mode_tests {
    use super::PermissionMode;

    /// **`default` is the one labelled Manual, and `manual` is not a value.**
    ///
    /// The vendor's reference says so in a sentence buried in a table: *"The
    /// mode labeled Manual arrives as `default`, never as `manual`, so scripts
    /// that match `default` keep working."* A matcher written from the visible
    /// interface instead of the wire would miss every supervised session.
    #[test]
    fn the_wire_spelling_is_the_vendors_and_manual_is_not_one() {
        assert_eq!(PermissionMode::parse("default"), PermissionMode::Default);
        assert_eq!(PermissionMode::Default.label(), "manual");
        assert_eq!(PermissionMode::Default.as_str(), "default");
        // `manual` on the wire would be a value the vendor does not send, so
        // it is kept rather than helpfully mapped onto `Default`.
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

    /// **A mode nobody here has heard of is not "supervised".**
    ///
    /// The whole class of bug this project keeps finding in itself is a widening
    /// that nothing reports. Collapsing an unknown mode into `Default` would put
    /// the most-supervised label on a session running something else entirely,
    /// and no test anywhere would fail. So the third answer exists.
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

    /// Every value survives the round trip, so a mode is never silently
    /// rewritten on its way through the store.
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

    /// No two modes read the same on screen. A label collision would make two
    /// different supervision states indistinguishable in the one place the
    /// distinction is the entire point.
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

/// What one agent said it could do, the last time Devplane started it.
///
/// Capabilities are advertised per agent at `initialize`, so this is a fact
/// about *that* agent at *that* version, with a date on it.
///
/// **Absence is *not probed*, never *not supported*.** An agent nobody has
/// started has no record at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilityRecord {
    /// What was actually run. Keyed on this rather than a friendly name because
    /// two registry entries can point at one binary, and a rename must not read
    /// as a new agent.
    pub command: String,
    /// What the agent calls itself, where it says.
    pub agent_name: Option<String>,
    /// `session/resume` — continue without replaying.
    pub resume: bool,
    /// `session/load` — continue *with* a replay. An agent may have either, and
    /// GitHub Copilot advertises this one and not `resume`.
    pub load_session: bool,
    /// `session/list` — the agent can enumerate its own sessions.
    pub list_sessions: bool,
    /// Whether the agent declared a session mode. Not a capability flag: the
    /// protocol has none for modes, and what is observable is whether the
    /// session response carried one.
    pub declares_modes: bool,
    /// Whether the agent advertised any authentication method.
    pub needs_auth: bool,
    pub measured_at: Timestamp,
}

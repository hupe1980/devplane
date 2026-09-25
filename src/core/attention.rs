//! The inbox: what needs a human, ranked.
//!
//! Attention items are *derived* from run state, never stored as authoritative
//! facts — a rebuild from events produces the same inbox.

use crate::core::event::{Choice, WaitingFor};
use crate::core::ids::{AttentionId, ProjectId, RunId};
use crate::core::run::{Run, RunState};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// How loudly an item asks for the human. Only `High` and `Critical` may raise
/// an OS notification. Not the sort key: where a row sits is its [`Band`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Normal,
    High,
    Critical,
}

impl Level {
    const ALL: &'static [Level] = &[Level::Normal, Level::High, Level::Critical];

    /// The one spelling this level is stored, reported and serialised under;
    /// the serde impls read it so there is no second list to drift from.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Normal => "normal",
            Level::High => "high",
            Level::Critical => "critical",
        }
    }
}

impl Serialize for Level {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Level::ALL
            .iter()
            .copied()
            .find(|l| l.as_str() == s)
            .ok_or_else(|| serde::de::Error::custom(format!("`{s}` is not a level")))
    }
}

/// Where a row sits in the inbox, and why it outranks the next.
///
/// Derived from the kind alone — nothing weighed, learned or guessed. Within a
/// band the order is age, oldest first, and never the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    /// A question or permission an agent is waiting on. Work is halted, now.
    StopsWithoutYou,
    /// An abandoned question, a run that is gone. Halted, and nobody said so.
    AlreadyStopped,
    /// A gate red after the agent finished; a run that failed.
    BrokeAfterTheFact,
    /// A change verified and waiting for accept or reject.
    ReadyToDecide,
    /// A review requested, an issue assigned, a marker in a specification.
    OwedByYou,
    /// Overlap, a cost anomaly, the machine's own health. Nothing is blocked.
    WorthKnowing,
}

impl Band {
    /// Every band, in rank order.
    pub const ALL: &'static [Band] = &[
        Band::StopsWithoutYou,
        Band::AlreadyStopped,
        Band::BrokeAfterTheFact,
        Band::ReadyToDecide,
        Band::OwedByYou,
        Band::WorthKnowing,
    ];
}

/// What the human must do — not which subsystem produced it.
///
/// `Ord` so folding groups deterministically, never by hash seed. Serialised
/// through [`AttentionKind::as_str`] so wire and store share one spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttentionKind {
    /// A tool wants permission and no policy rule matched.
    Permission,
    /// The agent asked a question.
    Question,
    /// A specification an in-flight change names carries a line the project's
    /// own declared words mark unresolved (e.g. `[NEEDS CLARIFICATION]`).
    ///
    /// One item per project, never per marker. A project that declared no
    /// words gets none — this tool has no default list.
    PlanQuestion,
    /// The agent asked a question and moved on without an answer. Normal, not
    /// critical: nothing is blocked any more.
    QuestionAbandoned,
    /// A run ended with an error.
    RunFailed,
    /// A live run has produced no activity for longer than the stall timeout.
    Stalled,
    /// Reconciliation lost the process.
    Lost,
    /// The context window is nearly full; quality drops and compaction looms.
    ContextHigh,
    /// A subscription rate limit is nearly exhausted.
    RateLimit,
    /// A change reached the spending ceiling its project set.
    CostSpike,
    /// The project's own checks did not pass, and the feedback budget is spent.
    GateFailed,
    /// A check on a pull request Devplane opened is red.
    CiRed,
    /// A reviewer asked for changes on a pull request Devplane opened.
    ChangesRequested,
    /// A pull request is green and waiting for a person.
    PrReady,
    /// A change whose last gate passed, sitting in review with nobody looking.
    /// Raised only while the change has no pull request; once one is open,
    /// [`Self::PrReady`] carries the same decision.
    ReadyToDecide,
    /// An issue on one of the person's projects is assigned to them. Raised
    /// from polled GitHub state; answered by opening a browser.
    IssueAssigned,
    /// A review was requested from the person on a pull request they did not
    /// open through Devplane.
    ReviewRequested,
    /// Another change in this repository is editing the same files — the only
    /// warning before the merge that two isolated checkouts are jointly
    /// impossible.
    Conflict,
    /// A change was mid-flight when the host stopped, and its agent is gone.
    Interrupted,
    /// The change could not continue — its worktree is gone. Nothing an agent
    /// can fix, so nothing is offered to hand back to one.
    ChangeBroken,
    /// This run keeps being refused, and is still going. A refused agent does
    /// not stop, so a rule that is too tight looks like one that works.
    Refused,
    /// The gate is installed and not answering, so no rule in any project is
    /// enforced. No hook can enforce its own presence — a timed-out hook does
    /// not block — so detection here is the whole defence. Critical.
    GateDown,
    /// A repository's `devplane.toml` will not parse. A host restarted against
    /// a broken file has no last-good rules to keep, so that repository's
    /// `never_auto` list is gone. Critical, like [`AttentionKind::GateDown`].
    ConfigBroken,
    /// Something Devplane decided or observed could not be written down, so
    /// the record is incomplete while looking complete. Critical.
    RecordIncomplete,
    /// An agent a previous host started is still running with nothing attached
    /// — re-parented to pid 1, holding a worktree, possibly spending.
    AgentLeaked,
    /// The specification moved under a run, and the run never saw it. Derived
    /// at close from the folder fingerprint against the change's start one.
    SpecDrifted,
    /// Another project filed a finding about this one, or a drafted GitHub
    /// issue waits. For the person, never an agent: it crosses a trust boundary.
    ReportFiled,
    /// A report nobody has answered for longer than the target's declared
    /// `[questions] deadline` — the same class of failure as a question
    /// nobody answered, so the same window.
    ReportWaiting,
}

impl Serialize for AttentionKind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AttentionKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ALL_KINDS
            .iter()
            .copied()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| serde::de::Error::custom(format!("`{s}` is not an attention kind")))
    }
}

impl AttentionKind {
    /// The one spelling of a kind: on the wire, in the store, in a snooze.
    pub fn as_str(&self) -> &'static str {
        match self {
            AttentionKind::Permission => "permission",
            AttentionKind::Question => "question",
            AttentionKind::QuestionAbandoned => "question_abandoned",
            AttentionKind::PlanQuestion => "plan_question",
            AttentionKind::RunFailed => "run_failed",
            AttentionKind::Stalled => "stalled",
            AttentionKind::Lost => "lost",
            AttentionKind::ContextHigh => "context_high",
            AttentionKind::RateLimit => "rate_limit",
            AttentionKind::CostSpike => "cost_spike",
            AttentionKind::GateFailed => "gate_failed",
            AttentionKind::CiRed => "ci_red",
            AttentionKind::ChangesRequested => "changes_requested",
            AttentionKind::PrReady => "pr_ready",
            AttentionKind::ReadyToDecide => "ready_to_decide",
            AttentionKind::IssueAssigned => "issue_assigned",
            AttentionKind::ReviewRequested => "review_requested",
            AttentionKind::Conflict => "conflict",
            AttentionKind::Interrupted => "interrupted",
            AttentionKind::ChangeBroken => "change_broken",
            AttentionKind::Refused => "refused",
            AttentionKind::GateDown => "gate_down",
            AttentionKind::ConfigBroken => "config_broken",
            AttentionKind::RecordIncomplete => "record_incomplete",
            AttentionKind::AgentLeaked => "agent_leaked",
            AttentionKind::SpecDrifted => "spec_drifted",
            AttentionKind::ReportFiled => "report_filed",
            AttentionKind::ReportWaiting => "report_waiting",
        }
    }

    /// How loudly this kind may interrupt. **Not where it sorts** — that is
    /// [`Self::band`].
    pub fn default_level(&self) -> Level {
        match self {
            AttentionKind::Permission
            | AttentionKind::Question
            | AttentionKind::RunFailed
            | AttentionKind::GateFailed
            | AttentionKind::ChangesRequested
            | AttentionKind::ChangeBroken
            | AttentionKind::CiRed => Level::High,
            AttentionKind::Lost => Level::Critical,
            // Machine health: enforcement or the record is silently missing,
            // and nothing else on the board would say so.
            AttentionKind::GateDown => Level::Critical,
            AttentionKind::ConfigBroken => Level::Critical,
            AttentionKind::RecordIncomplete => Level::Critical,
            AttentionKind::AgentLeaked => Level::Critical,
            AttentionKind::Interrupted => Level::High,
            // Nothing is blocked on any of these: a row, not a notification.
            AttentionKind::QuestionAbandoned => Level::Normal,
            AttentionKind::PlanQuestion => Level::Normal,
            AttentionKind::ReadyToDecide => Level::Normal,
            AttentionKind::SpecDrifted => Level::Normal,
            AttentionKind::ReportFiled | AttentionKind::ReportWaiting => Level::Normal,
            AttentionKind::CostSpike
            | AttentionKind::Refused
            | AttentionKind::Stalled
            | AttentionKind::ContextHigh
            | AttentionKind::RateLimit
            | AttentionKind::PrReady
            | AttentionKind::IssueAssigned
            | AttentionKind::ReviewRequested
            | AttentionKind::Conflict => Level::Normal,
        }
    }

    /// Where this kind sits in the list. Exhaustive, so a new kind without a
    /// band is a compile error.
    pub fn band(&self) -> Band {
        match self {
            AttentionKind::Permission | AttentionKind::Question => Band::StopsWithoutYou,
            AttentionKind::QuestionAbandoned | AttentionKind::Lost | AttentionKind::Interrupted => {
                Band::AlreadyStopped
            }
            AttentionKind::GateFailed
            | AttentionKind::RunFailed
            | AttentionKind::CiRed
            | AttentionKind::ChangeBroken => Band::BrokeAfterTheFact,
            // A drift or a report is a decision too, and nothing is blocked
            // until it is made.
            AttentionKind::ReadyToDecide
            | AttentionKind::PrReady
            | AttentionKind::SpecDrifted
            | AttentionKind::ReportFiled
            | AttentionKind::ReportWaiting => Band::ReadyToDecide,
            AttentionKind::ReviewRequested
            | AttentionKind::IssueAssigned
            | AttentionKind::ChangesRequested
            | AttentionKind::PlanQuestion => Band::OwedByYou,
            // Machine health is the loudest notification yet not something a
            // person answers from a list.
            AttentionKind::Conflict
            | AttentionKind::CostSpike
            | AttentionKind::Stalled
            | AttentionKind::ContextHigh
            | AttentionKind::RateLimit
            | AttentionKind::Refused
            | AttentionKind::GateDown
            | AttentionKind::ConfigBroken
            | AttentionKind::RecordIncomplete
            | AttentionKind::AgentLeaked => Band::WorthKnowing,
        }
    }
}

/// An action offered on an item. An offered action is always implemented for
/// the kind of run it is offered on, and an implemented one is offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Pick one of the `options` by its protocol id. Offered only when options
    /// carry ids, and listed before `Allow`/`Deny`.
    Choose,
    /// Grant the outstanding request. Offered only when Devplane can actually
    /// answer it — a driven run — never for a session it merely watches.
    Allow,
    /// Refuse it.
    Deny,
    /// Send free text back to a driven run: the answer to a question that
    /// offered no options, or a correction mid-turn.
    Reply,
    /// Raise the window that owns this session — the only way to answer an
    /// observed session.
    Focus,
    Attach,
    Open,
    /// Open the pull request in a browser. The item carries its `url`.
    OpenPr,
    /// Open the issue in a browser. The item carries its `url`.
    OpenIssue,
    /// Hand the failures back once more, past the project's bound. Recorded as
    /// the person's choice.
    Retry,
    /// Push the branch and open the pull request, or be handed the commands.
    /// Verified changes only, and only ever done by a person.
    Offer,
    /// Continue a change whose agent is gone, against the same agent-side
    /// session. Offered only when the run recorded its `session/resume` id.
    Resume,
    /// Resume the run with a prompt naming the specification files that
    /// changed under it, so it re-reads them. Recorded as the person's.
    TellRun,
    /// Accept that the specification moved: the change's start fingerprint
    /// moves forward and the run's work is measured against what it saw.
    AcceptDrift,
    /// Start a change in the target project from a report, with the report
    /// attached and quoted in the prompt.
    StartFromReport,
    /// Refuse a report, with a reason the project that filed it is told.
    RejectReport,
    /// Put a report off, with a reason the project that filed it is told.
    DeferReport,
    /// Open a drafted GitHub issue with the person's own `gh`. The only
    /// action anywhere that writes to a forge.
    OpenDraft,
    DiscardDraft,
    Snooze,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Choose => "choose",
            Action::Allow => "allow",
            Action::Deny => "deny",
            Action::Reply => "reply",
            Action::Focus => "focus",
            Action::Attach => "attach",
            Action::Open => "open",
            Action::OpenPr => "open_pr",
            Action::OpenIssue => "open_issue",
            Action::Retry => "retry",
            Action::Offer => "offer",
            Action::Resume => "resume",
            Action::TellRun => "tell_run",
            Action::AcceptDrift => "accept_drift",
            Action::StartFromReport => "start_from_report",
            Action::RejectReport => "reject_report",
            Action::DeferReport => "defer_report",
            Action::OpenDraft => "open_draft",
            Action::DiscardDraft => "discard_draft",
            Action::Snooze => "snooze",
        }
    }
}

/// Which of a subject's inbox items are hidden, and until when.
///
/// Per kind, not per subject: a snooze covers the kinds on screen when it was
/// taken, and a kind that turns up afterwards is shown. So silencing one thing
/// never silences another.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Snoozed(std::collections::BTreeMap<String, Timestamp>);

impl Snoozed {
    pub fn hides(&self, kind: &AttentionKind) -> bool {
        self.0
            .get(kind.as_str())
            .is_some_and(|until| Timestamp::now() < *until)
    }

    /// Hide these kinds until `until`. The caller passes the kinds currently
    /// derived for the subject, so a snooze is always about what was visible.
    pub fn hide(&mut self, kinds: impl IntoIterator<Item = AttentionKind>, until: Timestamp) {
        for k in kinds {
            self.0.insert(k.as_str().to_string(), until);
        }
    }

    /// Un-snooze everything — what `minutes = 0` means.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// When the last hidden kind comes back, for the surfaces that say so.
    pub fn until(&self) -> Option<Timestamp> {
        let now = Timestamp::now();
        self.0.values().filter(|u| now < **u).max().copied()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttentionItem {
    pub id: AttentionId,
    pub kind: AttentionKind,
    pub level: Level,
    /// `None` for an item about a change whose runs have all ended (e.g. a pull
    /// request going red hours later).
    #[serde(default)]
    pub run_id: Option<RunId>,
    pub project_id: Option<ProjectId>,
    /// One line naming the decision, not the subsystem.
    pub title: String,
    /// The detail a human needs to decide: the tool input, the question, the
    /// error. Rendered as untrusted text.
    pub detail: Option<String>,
    /// Why there is no yes-or-no here, when there is not — e.g. a permission on
    /// a watched session, which only the agent's own dialog can answer.
    /// Composed here so every surface explains it the same way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_in: Option<String>,
    /// The answers on offer. An option with an `id` can be chosen from here; one
    /// without can only be read, and the actions say so.
    pub options: Vec<Choice>,
    pub actions: Vec<Action>,
    /// The protocol request this item answers. Useful only to the connection
    /// that issued it; surfaces answer by `ask`.
    #[serde(default)]
    pub request_id: Option<String>,
    /// The durable ask every surface answers by. Present exactly when an answer
    /// can still be recorded — including after a host restart.
    #[serde(default)]
    pub ask: Option<crate::core::AskId>,
    /// The whole form behind a question: which field each answer goes back
    /// under, and any free-text box. `options` alone is a smaller question.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<serde_json::Value>,
    /// Where `open_pr` goes. Present exactly when that action is offered.
    #[serde(default)]
    pub url: Option<String>,
    /// A `claude-cli://` link that opens an agent in the right repository with
    /// a prompt already typed — and not sent. On items naming work somebody is
    /// about to do anyway: red CI, a spent feedback budget, requested changes.
    #[serde(default)]
    pub launch: Option<String>,
    /// The change this item is about; the run that did the work may be gone.
    #[serde(default)]
    pub change_id: Option<crate::core::ids::ChangeId>,
    /// The rule to paste so this is never asked again, on a permission item.
    /// The exact call unless the store's counts justify a pattern, so the host
    /// fills it: `build` is pure and the count is a query.
    #[serde(default)]
    pub offer: Option<crate::core::offer::RuleOffer>,
    /// Why there is no offer, on a permission item without one.
    #[serde(default)]
    pub no_offer: Option<crate::core::offer::NoOfferView>,
    /// The report this row is about, on a report row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<crate::core::ReportId>,
    pub since: Timestamp,
}

impl AttentionItem {
    /// The stable id of an item, so that the same open question keeps its
    /// place in the list across rebuilds and can be snoozed.
    pub fn make_id(run: &RunId, kind: &AttentionKind) -> AttentionId {
        AttentionId::new(format!("{}:{}", run.as_str(), kind.as_str()))
    }

    /// The bare item, everything else `None` or empty — what a machine row
    /// carries, and the one literal every other builder starts from.
    pub fn machine(
        kind: AttentionKind,
        id: AttentionId,
        title: String,
        detail: Option<String>,
        since: Timestamp,
    ) -> Self {
        AttentionItem {
            id,
            level: kind.default_level(),
            kind,
            run_id: None,
            project_id: None,
            title,
            detail,
            answer_in: None,
            options: Vec::new(),
            actions: Vec::new(),
            request_id: None,
            ask: None,
            form: None,
            url: None,
            launch: None,
            change_id: None,
            offer: None,
            no_offer: None,
            report: None,
            since,
        }
    }

    pub fn band(&self) -> Band {
        self.kind.band()
    }

    /// Sort key: the band, then how long it has been waiting — oldest first.
    /// Never the project, never the level.
    pub fn rank(&self) -> (Band, Timestamp) {
        (self.band(), self.since)
    }

    /// Whether this row can be answered from here — what `--needs-you` means.
    /// Derived from what is offered, never from the kind, so it cannot drift
    /// from the actions.
    pub fn has_answer_path(&self) -> bool {
        self.ask.is_some()
            || !self.options.is_empty()
            || self
                .actions
                .iter()
                .any(|a| !matches!(a, Action::Open | Action::Snooze))
    }
}

/// How an inbox item stopped needing a person, recorded so a kind that cries
/// wolf shows up. Three outcomes rather than one "precision" ratio, because
/// `Elsewhere` is ambiguous and averaging it away hides the distinction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// A person used one of the item's own actions.
    Acted,
    /// A person snoozed it — the clearest signal a kind is too loud.
    Dismissed,
    /// It went away on its own, or was answered somewhere else.
    Elsewhere,
}

impl Resolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            Resolution::Acted => "acted",
            Resolution::Dismissed => "dismissed",
            Resolution::Elsewhere => "elsewhere",
        }
    }
}

/// What the inbox did for one kind over a window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct KindStats {
    pub raised: i64,
    pub acted: i64,
    pub dismissed: i64,
    pub elsewhere: i64,
    pub open: i64,
    /// How many were folded into a summary rather than listed.
    #[serde(default)]
    pub folded: i64,
    /// How many were folded and then acted on once opened — a sign the kind
    /// is being folded wrongly.
    #[serde(default)]
    pub folded_then_acted: i64,
}

impl KindStats {
    /// The fraction of resolved items a person acted on. `None`, not zero, when
    /// nothing has resolved: never-fired and always-ignored are opposite facts.
    pub fn acted_share(&self) -> Option<f64> {
        let closed = self.acted + self.dismissed + self.elsewhere;
        (closed > 0).then(|| self.acted as f64 / closed as f64)
    }
}

/// Thresholds the inbox uses.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub stall_seconds: i64,
    pub context_high_percent: f64,
    pub rate_limit_percent: f64,
    /// How many refused tool calls in one run before somebody is told.
    pub refusals: u64,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            stall_seconds: 600,
            context_high_percent: 85.0,
            rate_limit_percent: 90.0,
            // Low enough to catch a loop early, high enough that one refused
            // `git push` and a person answering "no" twice do not raise it.
            refusals: 5,
        }
    }
}

/// [`gate_down_item`] as first seen at `since`. The age is the caller's: a row
/// aged `now` on every poll would read as new for ever.
pub fn gate_down_item_at(why: &str, since: Timestamp) -> AttentionItem {
    AttentionItem::machine(
        AttentionKind::GateDown,
        // Stable ids here and below: one open item, not one per poll.
        AttentionId::from("gate-down".to_string()),
        "The permission gate is installed and not answering".into(),
        Some(format!(
            "No rule in any project is being enforced right now.\n{why}\n\n\
             Run `devplane doctor` for the command it tried, then \
             `devplane connect claude` to reinstall it."
        )),
        since,
    )
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// [`record_incomplete_item`] as first seen at `since`.
pub fn record_incomplete_item_at(
    events: u64,
    decisions: u64,
    last: &str,
    since: Timestamp,
) -> AttentionItem {
    let what = match (events, decisions) {
        (0, d) => format!("{d} decision{}", plural(d)),
        (e, 0) => format!("{e} event{}", plural(e)),
        (e, d) => format!("{e} event{} and {d} decision{}", plural(e), plural(d)),
    };
    AttentionItem::machine(
        AttentionKind::RecordIncomplete,
        AttentionId::from("record-incomplete".to_string()),
        format!("{what} could not be written down"),
        Some(format!(
            "Devplane kept going and the board looks complete; it is not. \
             Every count, every audit row and every answer to \"who decided \
             this\" is now missing at least this much.\n{last}\n\n\
             Check the disk and the permissions on the store, then restart \
             the host. The gap does not fill in afterwards."
        )),
        since,
    )
}

/// [`agent_leaked_item`] as first seen at `since`.
pub fn agent_leaked_item_at(
    pid: u32,
    command: &str,
    worktree: Option<&str>,
    since: Timestamp,
) -> AttentionItem {
    AttentionItem::machine(
        AttentionKind::AgentLeaked,
        AttentionId::from(format!("agent-leaked-{pid}")),
        format!("An agent from a previous host is still running (pid {pid})"),
        Some(format!(
            "Nothing is attached to it: it is blocked on a pipe that closed when \
             the host it belonged to was killed, and it is holding{} — and \
             spending, if it was mid-turn.\n{command}\n\n\
             End it with:  kill -TERM -{pid}\n\n\
             Devplane will not do that for you: it may be part-way through \
             writing what it was last asked to do.",
            match worktree {
                Some(w) => format!(" {w}"),
                None => String::new(),
            }
        )),
        since,
    )
}

/// An ask that outlived the process that asked it — derived from the stored
/// row, not a live run, so a restart never hides an unanswered question.
/// It says plainly the agent is gone; an answer is recorded, then delivered by
/// resuming the session where possible.
pub fn stranded_ask_item(ask: &crate::core::ask::Ask) -> AttentionItem {
    let kind = match ask.kind {
        crate::core::ask::Kind::Permission => AttentionKind::Permission,
        crate::core::ask::Kind::Question => AttentionKind::Question,
    };
    let options: Vec<Choice> = ask
        .payload
        .get("options")
        .and_then(|o| serde_json::from_value(o.clone()).ok())
        .unwrap_or_default();
    // A *held* ask is an agent blocked right now, for a few more seconds, on a
    // watched session — not a stranded one whose agent is gone. The detail
    // must not say the agent stopped while it is still waiting.
    let held_until: Option<Timestamp> = ask
        .payload
        .get("held_until")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());
    let now = Timestamp::now();
    let holding = held_until.is_some_and(|t| t > now);
    let lapsed = held_until.is_some_and(|t| t <= now);

    let detail = match (holding, lapsed, held_until) {
        (true, _, Some(until)) => format!(
            "An agent is waiting for you right now — about {}s left. Answer it \
             here and it carries straight back; the editor it runs in stays \
             where it is.",
            until.duration_since(now).as_secs().max(0)
        ),
        // The hold ran out: the agent's own dialog is up.
        (_, true, _) => "The hold ran out, so the agent is asking in its own window now. \
             Nothing was decided here."
            .to_string(),
        _ => format!(
            "Asked {} and still unanswered. The agent that asked is no longer \
             running, so answering this records your answer and delivers it by \
             resuming that session — {}",
            ask.asked_at,
            ask.deadline.says()
        ),
    };
    AttentionItem {
        run_id: Some(ask.run.clone()),
        project_id: ask.project.clone(),
        ask: Some(ask.id.clone()),
        options,
        // No `focus` while a hold runs; once lapsed, only the vendor's dialog
        // can answer, so raising its window is the offer.
        actions: match (holding, lapsed) {
            (true, _) => vec![Action::Choose, Action::Reply],
            (_, true) => vec![Action::Focus],
            _ => vec![Action::Choose, Action::Reply],
        },
        request_id: Some(ask.request_id.clone()),
        form: ask.payload.get("form").cloned(),
        ..AttentionItem::machine(
            kind,
            // Keyed on the ask: one row across any number of restarts.
            AttentionId::from(format!("ask-{}", ask.id)),
            ask.message.clone(),
            Some(detail),
            ask.asked_at,
        )
    }
}

/// [`config_broken_item`] as first seen at `since`.
pub fn config_broken_item_at(root: &Path, why: &str, since: Timestamp) -> AttentionItem {
    let where_ = root.display().to_string();
    AttentionItem::machine(
        AttentionKind::ConfigBroken,
        AttentionId::from(format!("config-broken:{where_}")),
        format!(
            "{}/devplane.toml will not load",
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| where_.clone())
        ),
        Some(format!(
            "The rules this repository commits are not in force.\n{why}\n\n\
             Run `devplane check {where_}` for the line, and the gates and \
             prohibitions come back as soon as the file parses."
        )),
        since,
    )
}

/// Derives the inbox for one run. Pure, so the whole inbox is a map over runs
/// and a rebuild after a restart produces exactly the same list.
///
/// `stall_seconds` is this run's threshold as the sweeper resolved it per
/// project, passed in rather than read from `cfg` so the inbox and the
/// `Stalled` event cannot disagree.
pub fn items_for_run(run: &Run, cfg: &AttentionConfig, stall_seconds: i64) -> Vec<AttentionItem> {
    // TODO(purity): take `now` from the caller rather than the clock.
    items_for_run_at(run, cfg, stall_seconds, Timestamp::now())
}

/// [`items_for_run`] as of `now`, with the snoozed rows dropped and not
/// counted. Prefer [`run_items_at`], which counts them.
pub fn items_for_run_at(
    run: &Run,
    cfg: &AttentionConfig,
    stall_seconds: i64,
    now: Timestamp,
) -> Vec<AttentionItem> {
    run_items_at(run, cfg, stall_seconds, now).items
}

/// What a builder derived, and how many rows a snooze kept off the list.
/// Hidden means counted: the count travels with the items to the surface.
/// The sibling of [`Narrowed`], which counts what a view left out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Derived {
    pub items: Vec<AttentionItem>,
    pub snoozed: usize,
}

impl Derived {
    /// [`rank`], keeping the snoozed count.
    #[must_use]
    pub fn rank(mut self) -> Self {
        self.items = rank(self.items);
        self
    }
}

impl Extend<Derived> for Derived {
    fn extend<I: IntoIterator<Item = Derived>>(&mut self, iter: I) {
        for d in iter {
            self.items.extend(d.items);
            self.snoozed += d.snoozed;
        }
    }
}

impl FromIterator<Derived> for Derived {
    fn from_iter<I: IntoIterator<Item = Derived>>(iter: I) -> Self {
        let mut out = Derived::default();
        out.extend(iter);
        out
    }
}

/// Derives the inbox for one run as of `now`, counting what its snooze hid.
/// The stall reads [`crate::core::reduce::facts::stalled`], shared with the
/// sweeper.
pub fn run_items_at(
    run: &Run,
    cfg: &AttentionConfig,
    stall_seconds: i64,
    now: Timestamp,
) -> Derived {
    use crate::core::reduce::facts;
    let mut out = Derived::default();
    let mut push = |kind: AttentionKind,
                    title: String,
                    detail: Option<String>,
                    options: Vec<Choice>,
                    actions: Vec<Action>,
                    since: Timestamp| {
        // Set where no surface can answer, only reach the session.
        let answer_in = match actions.iter().any(|a| {
            matches!(
                a,
                Action::Allow | Action::Deny | Action::Choose | Action::Reply
            )
        }) {
            true => None,
            false if matches!(kind, AttentionKind::Permission | AttentionKind::Question) => {
                Some(match crate::core::vendors::vendor_of(&run.agent) {
                    Some(v) => format!("{v} owns this dialog — answer it in its own window"),
                    None => "the agent's own window owns this dialog — answer it there".to_string(),
                })
            }
            false => None,
        };
        // Snoozed per kind, and counted.
        if run.snoozed.hides(&kind) {
            out.snoozed += 1;
            return;
        }
        out.items.push(AttentionItem {
            run_id: Some(run.id.clone()),
            project_id: run.project_id.clone(),
            answer_in,
            options,
            actions,
            ask: run.blocked_on.as_ref().and_then(|b| b.ask.clone()),
            request_id: run.blocked_on.as_ref().and_then(|b| b.request_id.clone()),
            form: run.blocked_on.as_ref().and_then(|b| b.form.clone()),
            // No launch link (the session is already open) and no offer
            // (`offer::compose` fills it from the store).
            ..AttentionItem::machine(
                kind,
                AttentionItem::make_id(&run.id, &kind),
                title,
                detail,
                since,
            )
        });
    };

    // Outside the state match: an abandoned question already happened,
    // whatever state the run is in now. One row per run (ids are `run:kind`):
    // the newest question, the rest a count.
    if let Some(q) = run.abandoned_questions.last() {
        let older = run.abandoned_questions.len() - 1 + run.abandoned_dropped as usize;
        let what_next = match &q.moved_on_to {
            Some(next) => format!(" \u{2014} it did `{next}` instead"),
            None => ", and the session ended".to_string(),
        };
        let and_more = match older {
            0 => String::new(),
            1 => " One other question went the same way.".to_string(),
            n => format!(" {n} other questions went the same way."),
        };
        // No answer action: the tool call is over and nothing could deliver it.
        push(
            AttentionKind::QuestionAbandoned,
            q.question.clone(),
            Some(format!(
                "The agent asked this and moved on without an answer{what_next}.{and_more} \
                 Open the session to raise it again."
            )),
            q.options.clone(),
            reach(run, &[Action::Open, Action::Snooze]),
            q.abandoned_at,
        );
    }

    match &run.state {
        RunState::Waiting(WaitingFor::Permission) => {
            let b = run.blocked_on.as_ref();
            let tool = b.and_then(|b| b.tool.clone()).unwrap_or_default();
            let title = if tool.is_empty() {
                "Permission needed".to_string()
            } else {
                format!("Permission: {tool}")
            };
            // Answerable only when Devplane owns the session.
            let answerable = b.and_then(|b| b.request_id.as_ref()).is_some();
            let options = b.map(|b| b.options.clone()).unwrap_or_default();
            let actions = answerable_actions(answerable, run, &options, WaitingFor::Permission);
            push(
                AttentionKind::Permission,
                title,
                b.and_then(|b| b.message.clone())
                    .or_else(|| b.and_then(|b| b.input.as_ref().map(summarise_input))),
                options,
                actions,
                b.map(|b| b.since).unwrap_or(run.last_event_at),
            );
        }
        RunState::Waiting(WaitingFor::Question) => {
            let b = run.blocked_on.as_ref();
            // Answerable exactly when a protocol request is behind it.
            let answerable = b.and_then(|b| b.request_id.as_ref()).is_some();
            let options = b.map(|b| b.options.clone()).unwrap_or_default();
            let actions = answerable_actions(answerable, run, &options, WaitingFor::Question);
            push(
                AttentionKind::Question,
                b.and_then(|b| b.message.clone())
                    .unwrap_or_else(|| "Agent asked a question".to_string()),
                None,
                options,
                actions,
                b.map(|b| b.since).unwrap_or(run.last_event_at),
            );
        }
        // A blocked state the vendor named and this code does not model: never
        // answerable here, but it still reaches the inbox without a release.
        RunState::Waiting(WaitingFor::Other(what)) => push(
            AttentionKind::Question,
            format!("Waiting: {what}"),
            Some(
                "Claude Code is waiting on a person for something Devplane \
                 does not model. Its own window has the dialog."
                    .into(),
            ),
            Vec::new(),
            reach(run, &[Action::Open, Action::Snooze]),
            run.blocked_on
                .as_ref()
                .map(|b| b.since)
                .unwrap_or(run.last_event_at),
        ),
        RunState::Failed => push(
            AttentionKind::RunFailed,
            "Run failed".to_string(),
            run.summary.clone(),
            Vec::new(),
            reach(run, &[Action::Open, Action::Snooze]),
            run.last_event_at,
        ),
        RunState::Lost => push(
            AttentionKind::Lost,
            "Session lost".to_string(),
            // What it was doing, not why it was lost.
            run.summary.clone(),
            Vec::new(),
            // Snooze, so a critical item with nothing to reach can be dismissed.
            reach(run, &[Action::Open, Action::Snooze]),
            run.last_event_at,
        ),
        // Only a session on a channel that carries activity can be seen to go
        // quiet; `facts::stalled` checks that.
        RunState::Working if facts::stalled(run, now, stall_seconds) => push(
            AttentionKind::Stalled,
            format!("No activity for {} min", facts::idle_for(run, now) / 60),
            run.summary.clone(),
            Vec::new(),
            reach(run, &[Action::Open, Action::Snooze]),
            run.last_activity_at,
        ),
        _ => {}
    }

    if run.state.is_live()
        && let Some(pct) = run.totals.context_percent()
        && pct >= cfg.context_high_percent
    {
        push(
            AttentionKind::ContextHigh,
            format!("Context {pct:.0}% full"),
            Some(
                "Compaction is close; ask the agent to commit or hand off to a fresh session."
                    .into(),
            ),
            Vec::new(),
            reach(run, &[Action::Open, Action::Snooze]),
            run.last_event_at,
        );
    }

    // Live and refused over and over by a rule that answers without asking —
    // nothing else distinguishes a protective policy from a wrong one.
    if run.state.is_live()
        && run.refusals >= cfg.refusals
        && let Some(last) = &run.last_refusal
    {
        let by = match last.by.strip_prefix("policy:") {
            Some(rule) => format!("`{rule}`"),
            None if last.by == "claude" => "Claude Code's own auto mode".to_string(),
            None => last.by.clone(),
        };
        push(
            AttentionKind::Refused,
            format!("{} calls refused in this run", run.refusals),
            Some(format!(
                "The last one was {}, refused by {by}. A refused agent does not stop — it \
                 tries something else — so a rule that is too tight costs a run that \
                 finishes worse and bills more. Check that the rule means what you meant.",
                if last.tool.is_empty() {
                    "a tool call".to_string()
                } else {
                    format!("`{}`", last.tool)
                }
            )),
            Vec::new(),
            reach(run, &[Action::Open, Action::Snooze]),
            last.at,
        );
    }

    // From the status-line shim: a subscription window about to close is
    // worth knowing before the agent finds out mid-turn.
    if run.state.is_live()
        && let Some(pct) = run.totals.rate_limit_percent
        && pct >= cfg.rate_limit_percent
    {
        let window = match run.totals.rate_limit_window.as_deref() {
            Some("seven_day") => "seven-day",
            _ => "five-hour",
        };
        push(
            AttentionKind::RateLimit,
            format!("{window} limit {pct:.0}% used"),
            Some(
                "Dispatching more work will run into this. Pause, or switch this \
                 project to another agent."
                    .into(),
            ),
            Vec::new(),
            reach(run, &[Action::Open, Action::Snooze]),
            run.last_event_at,
        );
    }

    out
}

/// [`plan_question_item`] as first seen at `since`.
pub fn plan_question_item_at(
    project: &str,
    project_id: &crate::core::ProjectId,
    plans: &[(String, crate::core::spec::Plan)],
    since: Timestamp,
) -> Option<AttentionItem> {
    let total: u32 = plans.iter().map(|(_, p)| p.open_questions).sum();
    if total == 0 {
        return None;
    }
    let where_ = match plans.iter().filter(|(_, p)| p.open_questions > 0).count() {
        1 => {
            let (_, p) = plans.iter().find(|(_, p)| p.open_questions > 0)?;
            p.path.clone()
        }
        n => format!("{n} specifications"),
    };
    let first = plans
        .iter()
        .flat_map(|(_, p)| p.questions.iter())
        .next()
        .map(|q| crate::core::text::clip(q.text.trim(), 160));
    Some(AttentionItem {
        project_id: Some(project_id.clone()),
        answer_in: Some(
            "These are lines in committed files. Answer one by editing the \
             specification it is in."
                .into(),
        ),
        ..AttentionItem::machine(
            AttentionKind::PlanQuestion,
            // Stable per project; clears when the line goes.
            AttentionId::from(format!("plan-question:{}", project_id.as_str())),
            match total {
                1 => format!("{project}: one question in {where_} nobody has answered"),
                n => format!("{project}: {n} questions in {where_} nobody has answered"),
            },
            // Repository text: rendered as untrusted, like every detail.
            first,
            since,
        )
    })
}

/// The specification moved under one run of this change. Keyed on the run;
/// `since` is when the document was written (or `now`), never the run's start.
pub fn spec_drifted_item_at(
    change: &crate::core::change::Change,
    drift: &crate::core::change::Drift,
    now: Timestamp,
) -> AttentionItem {
    AttentionItem {
        run_id: Some(drift.run.clone()),
        project_id: Some(change.project_id.clone()),
        change_id: Some(change.id.clone()),
        actions: vec![Action::TellRun, Action::AcceptDrift, Action::Snooze],
        ..AttentionItem::machine(
            AttentionKind::SpecDrifted,
            AttentionItem::make_id(&drift.run, &AttentionKind::SpecDrifted),
            drift.says(),
            Some(format!(
                "{} · changed {} · the run started {}",
                change.spec.as_deref().unwrap_or("the specification"),
                drift
                    .changed_at
                    .map_or("at a time the folder does not say".to_string(), |t| t
                        .to_string()),
                drift.started_at
            )),
            drift.changed_at.unwrap_or(now),
        )
    }
}

/// The row a report raises in the project that has to decide about it.
///
/// An open report goes to its target (becoming [`AttentionKind::ReportWaiting`]
/// past the window); a drafted GitHub issue goes to the project that filed it.
/// Anything else raises nothing. The detail is
/// [`Report::quoted`](crate::core::report::Report::quoted): the finding under
/// its attribution, never in this product's voice.
pub fn report_item_at(
    report: &crate::core::report::Report,
    window: Option<std::time::Duration>,
    now: Timestamp,
) -> Option<AttentionItem> {
    use crate::core::report::{State, Target};
    let (kind, project, actions) = match (&report.state, &report.target) {
        (State::Open, Target::Project { project, .. }) => (
            match report.waiting(window, now) {
                true => AttentionKind::ReportWaiting,
                false => AttentionKind::ReportFiled,
            },
            project.clone(),
            vec![
                Action::StartFromReport,
                Action::RejectReport,
                Action::DeferReport,
            ],
        ),
        (State::Drafted, Target::GitHub { .. }) => (
            AttentionKind::ReportFiled,
            report.provenance.project.clone(),
            vec![Action::OpenDraft, Action::DiscardDraft],
        ),
        _ => return None,
    };
    let title = match &report.state {
        State::Drafted => format!(
            "An issue for {} is drafted: {}",
            report.target_says(),
            crate::core::text::clip(&report.title, 80)
        ),
        _ => format!(
            "A {} from {}: {}",
            report.kind.as_str(),
            report.provenance.project_name,
            crate::core::text::clip(&report.title, 80)
        ),
    };
    let mut item = AttentionItem::machine(
        kind,
        AttentionId::new(format!("{}:{}", report.id, kind.as_str())),
        title,
        Some(report.quoted()),
        report.provenance.at,
    );
    item.project_id = Some(project);
    item.actions = actions;
    item.report = Some(report.id.clone());
    Some(item)
}

pub fn items_for_change(
    change: &crate::core::change::Change,
    can_drive: bool,
    can_resume: bool,
) -> Vec<AttentionItem> {
    items_for_change_in(change, can_drive, can_resume, None)
}

/// The same, told the repository slug so it can build a launch link, with the
/// snoozed rows dropped and not counted. Prefer [`change_items_in`].
pub fn items_for_change_in(
    change: &crate::core::change::Change,
    can_drive: bool,
    can_resume: bool,
    repo: Option<&str>,
) -> Vec<AttentionItem> {
    change_items_in(change, can_drive, can_resume, repo).items
}

/// Derives the inbox entries for a change, counting what its snooze hid.
///
/// `repo` is a slug, not a path, so a launch link resolves to whichever clone
/// the reader has. Gate rows age from the gate's own timestamp, since
/// `updated_at` moves on every write; rows with no timestamp of their own fall
/// back to `updated_at`.
pub fn change_items_in(
    change: &crate::core::change::Change,
    can_drive: bool,
    can_resume: bool,
    repo: Option<&str>,
) -> Derived {
    let mut out = Derived::default();
    let url = change.pull_request.as_ref().map(|p| p.url.clone());
    let gate_at = change.last_gate().map(|g| g.at);
    let launch_for = |kind: &AttentionKind| -> Option<String> {
        let repo = repo?;
        // Only the kinds that name work somebody is about to do anyway.
        let prompt = match kind {
            AttentionKind::CiRed => format!(
                "The checks on pull request #{} are failing. Find out why and fix it.",
                change.pull_request.as_ref()?.number
            ),
            AttentionKind::ChangesRequested => format!(
                "A reviewer asked for changes on pull request #{}. Read the comments and address them.",
                change.pull_request.as_ref()?.number
            ),
            AttentionKind::GateFailed => format!(
                "The project's checks did not pass for \"{}\" and the feedback budget is spent. \
                 Look at what is failing and fix the cause rather than the check.",
                change.title
            ),
            _ => return None,
        };
        crate::core::deeplink::open_repo(repo, &prompt)
    };
    // Builds one row, or counts it as hidden (per kind; see `Snoozed`).
    let mut mk = |kind: AttentionKind,
                  title: String,
                  detail: Option<String>,
                  actions: Vec<Action>,
                  since: Option<Timestamp>| {
        if change.snoozed.hides(&kind) {
            out.snoozed += 1;
            return;
        }
        out.items.push(AttentionItem {
            launch: launch_for(&kind),
            url: actions
                .contains(&Action::OpenPr)
                .then(|| url.clone())
                .flatten(),
            // Absent once every run on this change has ended.
            run_id: change.current_run().cloned(),
            project_id: Some(change.project_id.clone()),
            actions,
            change_id: Some(change.id.clone()),
            ..AttentionItem::machine(
                kind,
                AttentionId::new(format!("{}:{}", change.id.as_str(), kind.as_str())),
                title,
                detail,
                since.unwrap_or(change.updated_at),
            )
        });
    };

    // Mid-flight when the host stopped: the change came back from the store,
    // the agent did not, and nothing will move it on its own.
    if !can_drive
        && !change.is_settled()
        && !change.needs_a_person()
        && change.current_state() == crate::core::change::ChangeState::InFlight
    {
        let mut actions = vec![Action::Open, Action::Snooze];
        if can_resume {
            actions.insert(0, Action::Resume);
        }
        mk(
            AttentionKind::Interrupted,
            format!("Interrupted: {}", change.title),
            Some(match can_resume {
                true => "The agent is gone and this was still in progress. Its branch and \
                         worktree are untouched, and its conversation can be picked up \
                         where it stopped."
                    .into(),
                false => "The agent is gone and this was still in progress. Its branch and \
                          worktree are untouched, but the conversation cannot be continued \
                          — start the change again when you want it finished."
                    .to_string(),
            }),
            actions,
            None,
        );
    }

    // Why the change stopped is read, never inferred from the last gate
    // report: each reason wants a different answer from a person.
    match change.stopped.as_ref() {
        None => {}

        Some(crate::core::change::Stopped::OverBudget { bound, .. }) => {
            mk(
                AttentionKind::CostSpike,
                format!("{}: {}", bound, change.title),
                Some(
                    "This reached one of the bounds `[budget]` sets. Raise it in \
                     devplane.toml if the change is worth more, or pick it up yourself."
                        .into(),
                ),
                vec![Action::Open, Action::Snooze],
                None,
            );
        }

        Some(crate::core::change::Stopped::GateFailed { gate }) => {
            let report = change.last_gate();
            let detail = match report {
                // The same failing lines the agent was handed.
                Some(g) => {
                    let mut d = g.summary();
                    let failing: Vec<&str> = g
                        .commands
                        .iter()
                        .filter(|c| !c.passed())
                        .flat_map(|c| c.failures.iter().map(String::as_str))
                        .take(5)
                        .collect();
                    if !failing.is_empty() {
                        d.push('\n');
                        d.push_str(&failing.join("\n"));
                    }
                    d
                }
                None => format!("{gate} failed."),
            };
            // Retry only when there is an agent left to hand it to.
            let mut actions = vec![Action::Open, Action::Snooze];
            if change.retryable(can_drive) {
                actions.insert(0, Action::Retry);
            }
            mk(
                AttentionKind::GateFailed,
                format!("{gate}: {}", change.title),
                Some(detail),
                actions,
                gate_at,
            );
        }

        Some(crate::core::change::Stopped::Broken { detail }) => {
            mk(
                AttentionKind::ChangeBroken,
                format!("stopped: {}", change.title),
                Some(format!(
                    "{detail}\n\nThis is not a problem with the code, so there is \
                     nothing to hand back to an agent."
                )),
                vec![Action::Open, Action::Snooze],
                None,
            );
        }
    }

    // Green and sitting in review, aged from the gate that passed. Only while
    // there is no pull request, so one decision is one row (see `pr_ready`).
    if matches!(change.waiting, Some(crate::core::change::Waiting::Person))
        && change.pull_request.is_none()
        && change.completion.is_none()
        && let Some(g) = change.last_gate()
        && g.passed()
    {
        // Offer only while the pass still describes the tree; a stale pass
        // says so and offers nothing.
        let verified = change.current_state() == crate::core::change::ChangeState::Verified;
        let (detail, actions) = match verified {
            true => (
                g.summary(),
                vec![Action::Offer, Action::Open, Action::Snooze],
            ),
            false => (
                crate::core::change::Completion::of(change, true, change.tree_now.as_ref())
                    .headline(),
                vec![Action::Open, Action::Snooze],
            ),
        };
        mk(
            AttentionKind::ReadyToDecide,
            format!("ready: {}", change.title),
            Some(detail),
            actions,
            gate_at,
        );
    }

    if !change.overlaps.is_empty() {
        let files: Vec<&str> = change
            .overlaps
            .iter()
            .flat_map(|o| o.files.iter().map(String::as_str))
            .take(5)
            .collect();
        let others: Vec<&str> = change.overlaps.iter().map(|o| o.title.as_str()).collect();
        mk(
            AttentionKind::Conflict,
            format!(
                "{} is editing the same files as {}",
                change.title,
                others.join(", ")
            ),
            Some(format!(
                "Both are in flight and both are locally correct; they cannot both land \
                 unchanged.\n\n{}",
                files.join("\n")
            )),
            vec![Action::Open, Action::Snooze],
            None,
        );
    }

    // The forge record has no timestamp, so these age from `updated_at`.
    if let Some(pr) = &change.pull_request {
        match pr.status.as_str() {
            "failing" => mk(
                AttentionKind::CiRed,
                format!("#{} is red: {}", pr.number, change.title),
                Some(if pr.failing_checks.is_empty() {
                    "a check failed".to_string()
                } else {
                    pr.failing_checks.join(", ")
                }),
                vec![Action::OpenPr, Action::Snooze],
                None,
            ),
            // `ready_to_merge` asks nobody for anything, so it raises nothing.
            "ready_for_review" => mk(
                AttentionKind::PrReady,
                format!("#{} is ready: {}", pr.number, change.title),
                None,
                vec![Action::OpenPr, Action::Snooze],
                None,
            ),
            "changes_requested" => mk(
                AttentionKind::ChangesRequested,
                format!("#{} has review comments: {}", pr.number, change.title),
                Some(
                    "A person asked for changes. Read them before asking an agent to act on them."
                        .into(),
                ),
                vec![Action::OpenPr, Action::Snooze],
                None,
            ),
            _ => {}
        }
    }
    out
}

/// What a blocked run offers: the answers the agent will take where Devplane
/// can send one, otherwise the ways to reach the session.
///
/// * Options with ids: `Choose` leads; a permission keeps `Allow`/`Deny` as
///   shorthand, a question gets no invented yes/no.
/// * No options on a question: `Reply`.
/// * Not answerable (observed session): reach it via [`reach`].
fn answerable_actions(
    answerable: bool,
    run: &Run,
    options: &[Choice],
    waiting_for: WaitingFor,
) -> Vec<Action> {
    if !answerable {
        return reach(run, &[Action::Open]);
    }
    let choosable = options.iter().any(|o| o.id.is_some());
    let mut out = Vec::new();
    if choosable {
        out.push(Action::Choose);
    }
    match waiting_for {
        WaitingFor::Permission => out.extend([Action::Allow, Action::Deny]),
        WaitingFor::Question if !choosable => out.push(Action::Reply),
        _ => {}
    }
    out.push(Action::Open);
    out
}

/// The ways to reach this run's session, ahead of `then`. Empty for a driven
/// run, which has no window and no `claude --resume`.
fn reach(run: &Run, then: &[Action]) -> Vec<Action> {
    let mut out = match run.mode {
        crate::core::run::RunMode::Driven => Vec::new(),
        _ => vec![Action::Focus, Action::Attach],
    };
    out.extend_from_slice(then);
    out
}

// ---------------------------------------------------------------------------
// Folding: a list that can be read
// ---------------------------------------------------------------------------

/// How many rows a person reads before they start scanning. Below this
/// nothing is folded. A judgement, and not a cap: truncating a ranked list
/// would hide its tail.
pub const READABLE: usize = 12;

/// Every kind. `every_kind_is_in_all_kinds` fails for a variant missing here.
pub const ALL_KINDS: &[AttentionKind] = &[
    AttentionKind::Permission,
    AttentionKind::Question,
    AttentionKind::QuestionAbandoned,
    AttentionKind::PlanQuestion,
    AttentionKind::RunFailed,
    AttentionKind::Stalled,
    AttentionKind::Lost,
    AttentionKind::ContextHigh,
    AttentionKind::RateLimit,
    AttentionKind::CostSpike,
    AttentionKind::GateFailed,
    AttentionKind::CiRed,
    AttentionKind::ChangesRequested,
    AttentionKind::PrReady,
    AttentionKind::ReadyToDecide,
    AttentionKind::IssueAssigned,
    AttentionKind::ReviewRequested,
    AttentionKind::Conflict,
    AttentionKind::Interrupted,
    AttentionKind::ChangeBroken,
    AttentionKind::Refused,
    AttentionKind::GateDown,
    AttentionKind::ConfigBroken,
    AttentionKind::RecordIncomplete,
    AttentionKind::AgentLeaked,
    AttentionKind::SpecDrifted,
    AttentionKind::ReportFiled,
    AttentionKind::ReportWaiting,
];

/// Kinds whose members are interchangeable to a person. Enumerated, never
/// inferred: a new kind is listed in full until somebody decides otherwise.
pub const FOLDABLE: &[AttentionKind] = &[
    // Resource warnings.
    AttentionKind::ContextHigh,
    AttentionKind::RateLimit,
    AttentionKind::CostSpike,
    // Forge queues.
    AttentionKind::IssueAssigned,
    AttentionKind::ReviewRequested,
    AttentionKind::PrReady,
    AttentionKind::CiRed,
    // Session health.
    AttentionKind::Stalled,
    AttentionKind::Lost,
    AttentionKind::Interrupted,
    AttentionKind::RunFailed,
    AttentionKind::Conflict,
    AttentionKind::RecordIncomplete,
];

/// Kinds that may never be folded or inhibited: each needs an answer only this
/// person can give. A separate decision from absence in [`FOLDABLE`].
pub const NEVER_FOLDED: &[AttentionKind] = &[
    AttentionKind::Permission,
    AttentionKind::Question,
    AttentionKind::QuestionAbandoned,
    AttentionKind::SpecDrifted,
    AttentionKind::ReportFiled,
    AttentionKind::ReportWaiting,
];

impl AttentionKind {
    #[must_use]
    pub fn foldable(self) -> bool {
        !NEVER_FOLDED.contains(&self) && FOLDABLE.contains(&self)
    }
}

/// A group of items shown as one row. Nothing is hidden: the ids are here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Not a generated wire type, like `AttentionItem`; the board reads plain JSON.
pub struct Summary {
    pub kind: AttentionKind,
    /// `None` where the items belong to no project, or to several.
    pub project: Option<ProjectId>,
    pub count: usize,
    /// The loudest member's level, so folding never quietens anything.
    pub level: Level,
    pub ids: Vec<AttentionId>,
}

/// Every item is in exactly one place: `placed` (listed, narrowed away,
/// inhibited, folded) must sum to `raised`. A `debug_assert_eq!`, so a release
/// build never panics an inbox over its own arithmetic.
pub fn accounted(raised: usize, placed: &[usize]) -> bool {
    let total: usize = placed.iter().sum();
    debug_assert_eq!(
        total, raised,
        "an inbox row vanished or was counted twice: {placed:?} do not sum to {raised}"
    );
    total == raised
}

/// Folds a ranked inbox into what a person can read, plus what was summarised.
/// Never drops: `rendered + summarised == raised`. Below [`READABLE`] the
/// output is the input.
#[must_use]
pub fn fold(items: Vec<AttentionItem>) -> (Vec<AttentionItem>, Vec<Summary>) {
    let raised = items.len();
    if items.len() <= READABLE {
        return (items, Vec::new());
    }

    // Group by kind and project; a group of one is not a summary.
    let mut groups: std::collections::BTreeMap<(AttentionKind, Option<ProjectId>), Vec<usize>> =
        std::collections::BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if it.kind.foldable() {
            groups
                .entry((it.kind, it.project_id.clone()))
                .or_default()
                .push(i);
        }
    }
    let folded: std::collections::BTreeSet<usize> = groups
        .values()
        .filter(|g| g.len() > 1)
        .flatten()
        .copied()
        .collect();

    let summaries: Vec<Summary> = groups
        .into_iter()
        .filter(|(_, g)| g.len() > 1)
        .map(|((kind, project), g)| Summary {
            kind,
            project,
            count: g.len(),
            level: g
                .iter()
                .map(|&i| items[i].level)
                .max()
                .unwrap_or(Level::Normal),
            ids: g.iter().map(|&i| items[i].id.clone()).collect(),
        })
        .collect();

    let rendered: Vec<AttentionItem> = items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !folded.contains(i))
        .map(|(_, it)| it)
        .collect();

    accounted(
        raised,
        &[rendered.len(), summaries.iter().map(|s| s.count).sum()],
    );
    (rendered, summaries)
}

// ---------------------------------------------------------------------------
// Inhibition: a symptom counted on its cause
// ---------------------------------------------------------------------------

/// How far a cause reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Only items in the same project.
    Project,
    /// Every item on the machine.
    Machine,
}

/// A named cause, a kind it explains, and why. Enumerated, never inferred
/// from project or time proximity — correlation is not causation, and
/// suppressing on it would hide a real row.
#[derive(Debug, Clone, Copy)]
pub struct Cause {
    pub cause: AttentionKind,
    pub consequence: AttentionKind,
    pub reach: Reach,
    /// Shown on the cause's row: what the count is of.
    pub because: &'static str,
}

/// The pairs that are certain: each is mechanical rather than probable.
pub const CAUSES: &[Cause] = &[
    Cause {
        cause: AttentionKind::ConfigBroken,
        consequence: AttentionKind::Refused,
        reach: Reach::Project,
        because: "the project's configuration will not parse, so no rule in it could be read",
    },
    Cause {
        cause: AttentionKind::ConfigBroken,
        consequence: AttentionKind::GateFailed,
        reach: Reach::Project,
        because: "the project's configuration will not parse, so its gates could not run",
    },
    Cause {
        cause: AttentionKind::GateDown,
        consequence: AttentionKind::Refused,
        reach: Reach::Machine,
        because: "the gate is not answering, so these calls got no verdict",
    },
    Cause {
        cause: AttentionKind::AgentLeaked,
        consequence: AttentionKind::Stalled,
        reach: Reach::Project,
        because: "a leaked agent is holding this project's worktree",
    },
    Cause {
        cause: AttentionKind::AgentLeaked,
        consequence: AttentionKind::Lost,
        reach: Reach::Project,
        because: "a leaked agent is holding this project's worktree",
    },
];

/// Consequences counted on one cause's row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Not a generated wire type, like `Summary`.
pub struct Inhibited {
    /// The item that explains them.
    pub cause: AttentionId,
    pub count: usize,
    pub because: String,
    pub ids: Vec<AttentionId>,
}

/// What a narrowing left out — counted, never implied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Narrowed {
    /// How many items the narrowing removed.
    pub count: usize,
    /// The projects those items belonged to.
    pub projects: Vec<String>,
    /// A project was asked for and matched nothing — distinct from that
    /// project having nothing waiting.
    pub no_such_project: bool,
}

/// How to narrow the inbox. A view, never a persisted preference.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Narrowing<'a> {
    /// Case-insensitive substring of the project name — the same rule as
    /// `devplane ls --project`.
    pub project: Option<&'a str>,
    /// Only rows with an answer path, as `ls --needs-you` has it.
    pub needs_you: bool,
}

impl Narrowing<'_> {
    pub fn is_none(&self) -> bool {
        self.project.is_none() && !self.needs_you
    }
}

/// Narrows a ranked list, and says what it left out. One computation so board
/// and terminal narrow identically. Applied before folding, so the counts
/// never overlap.
pub fn narrow(
    items: Vec<AttentionItem>,
    by: &Narrowing<'_>,
    project_name: &dyn Fn(&ProjectId) -> Option<String>,
    known_projects: &[String],
) -> (Vec<AttentionItem>, Option<Narrowed>) {
    if by.is_none() {
        return (items, None);
    }
    let raised = items.len();

    let needle = by.project.map(str::to_lowercase);
    let no_such_project = needle.as_ref().is_some_and(|n| {
        !known_projects
            .iter()
            .any(|p| p.to_lowercase().contains(n.as_str()))
    });

    let mut kept = Vec::with_capacity(items.len());
    let mut dropped: Vec<AttentionItem> = Vec::new();
    for item in items {
        let name = item.project_id.as_ref().and_then(project_name);
        let project_ok = match (&needle, &name) {
            (None, _) => true,
            // A machine-wide row is in no project, so no project filter keeps it.
            (Some(_), None) => false,
            (Some(n), Some(p)) => p.to_lowercase().contains(n.as_str()),
        };
        let answerable = !by.needs_you || item.has_answer_path();
        if project_ok && answerable {
            kept.push(item);
        } else {
            dropped.push(item);
        }
    }

    let mut projects: Vec<String> = dropped
        .iter()
        .filter_map(|i| i.project_id.as_ref().and_then(project_name))
        .collect();
    projects.sort();
    projects.dedup();

    accounted(raised, &[kept.len(), dropped.len()]);
    (
        kept,
        Some(Narrowed {
            count: dropped.len(),
            projects,
            no_such_project,
        }),
    )
}

/// Moves named consequences onto the rows that explain them. Never drops;
/// nothing is stored, so a symptom returns the moment its cause resolves.
/// A [`NEVER_FOLDED`] kind is always listed, and a symptom two causes claim is
/// counted once, under the louder.
#[must_use]
pub fn inhibit(items: Vec<AttentionItem>) -> (Vec<AttentionItem>, Vec<Inhibited>) {
    let causes: Vec<&AttentionItem> = items
        .iter()
        .filter(|i| CAUSES.iter().any(|c| c.cause == i.kind))
        .collect();
    if causes.is_empty() {
        return (items, Vec::new());
    }
    let raised = items.len();

    // For each item, the loudest cause that explains it.
    let mut claim: std::collections::BTreeMap<usize, (AttentionId, &'static str, Level)> =
        std::collections::BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if NEVER_FOLDED.contains(&it.kind) {
            continue;
        }
        for c in CAUSES {
            if c.consequence != it.kind {
                continue;
            }
            let Some(cause) = causes.iter().find(|x| {
                x.kind == c.cause
                    && match c.reach {
                        Reach::Machine => true,
                        Reach::Project => x.project_id == it.project_id,
                    }
            }) else {
                continue;
            };
            let better = claim.get(&i).is_none_or(|(_, _, lvl)| cause.level > *lvl);
            if better {
                claim.insert(i, (cause.id.clone(), c.because, cause.level));
            }
        }
    }

    let mut by_cause: std::collections::BTreeMap<AttentionId, (String, Vec<AttentionId>)> =
        std::collections::BTreeMap::new();
    for (i, (cause, because, _)) in &claim {
        let e = by_cause
            .entry(cause.clone())
            .or_insert_with(|| ((*because).to_string(), Vec::new()));
        e.1.push(items[*i].id.clone());
    }

    let suppressed: Vec<Inhibited> = by_cause
        .into_iter()
        .map(|(cause, (because, ids))| Inhibited {
            cause,
            count: ids.len(),
            because,
            ids,
        })
        .collect();

    let kept: Vec<AttentionItem> = items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !claim.contains_key(i))
        .map(|(_, it)| it)
        .collect();

    accounted(raised, &[kept.len(), claim.len()]);
    (kept, suppressed)
}

/// Ranks the whole inbox: by [`Band`], then oldest first. Nothing else — not
/// the level, not the project, never anything a model produced. Stable, so
/// ties keep derivation order.
pub fn rank(mut items: Vec<AttentionItem>) -> Vec<AttentionItem> {
    items.sort_by_key(AttentionItem::rank);
    items
}

fn summarise_input(v: &serde_json::Value) -> String {
    if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
        return cmd.to_string();
    }
    if let Some(p) = v.get("file_path").and_then(|c| c.as_str()) {
        return p.to_string();
    }
    // Arbitrary JSON can be huge and non-ASCII: `clip`, never a byte slice.
    crate::core::text::clip(&v.to_string(), 200)
}

// Test-only shorthands for tests that check a row's shape, not its age.
#[cfg(test)]
fn gate_down_item(why: &str) -> AttentionItem {
    gate_down_item_at(why, jiff::Timestamp::UNIX_EPOCH)
}
#[cfg(test)]
fn record_incomplete_item(events: u64, decisions: u64, last: &str) -> AttentionItem {
    record_incomplete_item_at(events, decisions, last, jiff::Timestamp::UNIX_EPOCH)
}
#[cfg(test)]
fn agent_leaked_item(pid: u32, command: &str, worktree: Option<&str>) -> AttentionItem {
    agent_leaked_item_at(pid, command, worktree, jiff::Timestamp::UNIX_EPOCH)
}
#[cfg(test)]
fn config_broken_item(root: &std::path::Path, why: &str) -> AttentionItem {
    config_broken_item_at(root, why, jiff::Timestamp::UNIX_EPOCH)
}
#[cfg(test)]
mod tests {
    /// One item of a kind, in a project, for the folding tests.
    fn it_of(kind: AttentionKind, project: Option<&str>, n: usize) -> AttentionItem {
        AttentionItem {
            level: Level::Normal,
            project_id: project.map(ProjectId::new),
            ..AttentionItem::machine(
                kind,
                AttentionId::from(format!("{}-{n}", kind.as_str())),
                format!("{} {n}", kind.as_str()),
                None,
                jiff::Timestamp::now(),
            )
        }
    }

    /// `listed + narrowed + inhibited + folded == raised` over every kind,
    /// composed as the API composes it: narrow, inhibit, fold.
    #[test]
    fn narrow_inhibit_and_fold_account_for_every_item_exactly_once() {
        let names = |id: &ProjectId| Some(id.as_str().to_string());
        let known = vec!["alpha".to_string(), "beta".to_string()];

        // Three of every kind over two projects, all ids distinct.
        let raised: Vec<AttentionItem> = ALL_KINDS
            .iter()
            .enumerate()
            .flat_map(|(i, k)| {
                (0..3).map(move |n| {
                    let mut it = it_of(
                        *k,
                        Some(if (i + n) % 2 == 0 { "alpha" } else { "beta" }),
                        i * 10 + n,
                    );
                    it.level = k.default_level();
                    if i % 3 == 0 {
                        it.actions = vec![Action::Reply];
                    }
                    it
                })
            })
            .collect();
        let total = raised.len();

        for by in [
            Narrowing::default(),
            Narrowing {
                project: Some("alpha"),
                needs_you: false,
            },
            Narrowing {
                project: None,
                needs_you: true,
            },
            Narrowing {
                project: Some("beta"),
                needs_you: true,
            },
            Narrowing {
                project: Some("nothing-like-this"),
                needs_you: false,
            },
        ] {
            let (in_scope, narrowed) = narrow(raised.clone(), &by, &names, &known);
            let away = narrowed.as_ref().map_or(0, |n| n.count);
            let (kept, inhibited) = inhibit(in_scope);
            let moved: usize = inhibited.iter().map(|i| i.count).sum();
            let (listed, summaries) = fold(kept);
            let folded: usize = summaries.iter().map(|s| s.ids.len()).sum();
            assert!(
                accounted(total, &[listed.len(), away, moved, folded]),
                "narrowing {by:?} lost or double-counted an item"
            );
            // Non-vacuous: every stage did something.
            if by.is_none() {
                assert!(moved > 0, "no cause explained anything");
                assert!(folded > 0, "nothing folded");
            }

            // And no item is in two places at once.
            let mut seen: std::collections::BTreeSet<&str> = Default::default();
            for i in &listed {
                assert!(seen.insert(i.id.as_str()), "{} listed twice", i.id.as_str());
            }
            for su in &summaries {
                for id in &su.ids {
                    assert!(
                        seen.insert(id.as_str()),
                        "{} folded and listed",
                        id.as_str()
                    );
                }
            }
            for inh in &inhibited {
                for id in &inh.ids {
                    assert!(
                        seen.insert(id.as_str()),
                        "{} inhibited and elsewhere",
                        id.as_str()
                    );
                }
            }
        }
    }

    /// The six bands in order, every kind in one, and a live permission above
    /// every machine row.
    #[test]
    fn the_bands_are_the_six_the_interaction_model_names_in_that_order() {
        assert_eq!(
            Band::ALL,
            &[
                Band::StopsWithoutYou,
                Band::AlreadyStopped,
                Band::BrokeAfterTheFact,
                Band::ReadyToDecide,
                Band::OwedByYou,
                Band::WorthKnowing,
            ]
        );
        for w in Band::ALL.windows(2) {
            assert!(w[0] < w[1], "{:?} does not outrank {:?}", w[0], w[1]);
        }

        let in_band = |b: Band| -> Vec<AttentionKind> {
            ALL_KINDS
                .iter()
                .copied()
                .filter(|k| k.band() == b)
                .collect()
        };
        use AttentionKind::*;
        assert_eq!(in_band(Band::StopsWithoutYou), [Permission, Question]);
        assert_eq!(
            in_band(Band::AlreadyStopped),
            [QuestionAbandoned, Lost, Interrupted]
        );
        assert_eq!(
            in_band(Band::BrokeAfterTheFact),
            [RunFailed, GateFailed, CiRed, ChangeBroken]
        );
        assert_eq!(
            in_band(Band::ReadyToDecide),
            [
                PrReady,
                ReadyToDecide,
                SpecDrifted,
                ReportFiled,
                ReportWaiting
            ]
        );
        assert_eq!(
            in_band(Band::OwedByYou),
            [
                PlanQuestion,
                ChangesRequested,
                IssueAssigned,
                ReviewRequested
            ]
        );
        assert_eq!(
            in_band(Band::WorthKnowing),
            [
                Stalled,
                ContextHigh,
                RateLimit,
                CostSpike,
                Conflict,
                Refused,
                GateDown,
                ConfigBroken,
                RecordIncomplete,
                AgentLeaked
            ]
        );
        let placed: usize = Band::ALL.iter().map(|b| in_band(*b).len()).sum();
        assert_eq!(placed, ALL_KINDS.len());

        // Bands win over level.
        let mut leaked = gate_down_item("it exited 127");
        leaked.since = Timestamp::now() - jiff::SignedDuration::from_hours(9);
        let mut perm = it_of(Permission, Some("p1"), 1);
        perm.level = Level::Normal;
        let ranked = rank(vec![leaked, perm]);
        assert_eq!(ranked[0].kind, Permission);
        assert_eq!(ranked[1].kind, GateDown);
    }

    #[test]
    fn within_a_band_the_order_is_age_and_never_the_project() {
        let now = Timestamp::now();
        let mut raised = Vec::new();
        for (n, (project, hours)) in [("p1", 5), ("p1", 4), ("p1", 3), ("p2", 2), ("p1", 1)]
            .into_iter()
            .enumerate()
        {
            let mut it = it_of(AttentionKind::CiRed, Some(project), n);
            it.since = now - jiff::SignedDuration::from_hours(hours);
            // A louder level must not move a row either.
            it.level = if n == 3 {
                Level::Critical
            } else {
                Level::Normal
            };
            raised.push(it);
        }
        let ranked = rank(raised);
        let ages: Vec<i64> = ranked
            .iter()
            .map(|i| now.duration_since(i.since).as_hours())
            .collect();
        assert_eq!(ages, [5, 4, 3, 2, 1], "not oldest first: {ages:?}");
        let projects: Vec<&str> = ranked
            .iter()
            .map(|i| i.project_id.as_ref().map(|p| p.as_str()).unwrap_or(""))
            .collect();
        assert_eq!(projects, ["p1", "p1", "p1", "p2", "p1"]);
    }

    #[test]
    fn a_machine_row_carries_the_age_it_was_given() {
        let then = "2026-09-20T08:00:00Z".parse::<Timestamp>().unwrap();
        let id = ProjectId::new("p");
        let plan = crate::core::spec::Plan {
            path: "specs/001".into(),
            present: true,
            files: 1,
            progress: None,
            truncated: None,
            questions: vec![crate::core::spec::Question {
                path: "spec.md".into(),
                text: "[NEEDS CLARIFICATION] which?".into(),
            }],
            open_questions: 1,
            outline: Vec::new(),
            fingerprint: None,
        };
        for item in [
            gate_down_item_at("it exited 127", then),
            config_broken_item_at(Path::new("/repo"), "line 3", then),
            agent_leaked_item_at(4242, "claude --acp", None, then),
            record_incomplete_item_at(1, 0, "disk full", then),
            plan_question_item_at("proj", &id, &[("w1".into(), plan)], then).expect("a marker"),
        ] {
            assert_eq!(
                item.since, then,
                "{:?} ignored the age it was given",
                item.kind
            );
        }
    }

    #[test]
    fn a_gate_failure_is_as_old_as_the_gate_that_failed() {
        let mut w = crate::core::Change::new(
            crate::core::ProjectId::new("p"),
            "fix it".into(),
            "…".into(),
        );
        let gate_at = Timestamp::now() - jiff::SignedDuration::from_hours(6);
        w.gates.push(crate::core::change::GateReport {
            gate: "check".into(),
            at: gate_at,
            duration_ms: 1,
            commands: vec![],
            attempt: 1,
            spec: None,
            commit: None,
        });
        w.stopped = Some(crate::core::change::Stopped::GateFailed {
            gate: "check".into(),
        });
        // The change was touched just now.
        w.updated_at = Timestamp::now();
        let item = items_for_change(&w, false, false)
            .into_iter()
            .find(|i| i.kind == AttentionKind::GateFailed)
            .expect("a gate failure");
        assert_eq!(item.since, gate_at);
    }

    /// Aged from the gate that passed, and gone once a pull request exists.
    #[test]
    fn a_change_in_review_with_green_gates_is_ready_to_decide() {
        let mut w = crate::core::Change::new(
            crate::core::ProjectId::new("p"),
            "rate-limit login".into(),
            "…".into(),
        );
        w.waiting = Some(crate::core::Waiting::Person);
        assert!(
            !items_for_change(&w, false, false)
                .iter()
                .any(|i| i.kind == AttentionKind::ReadyToDecide)
        );

        let gate_at = Timestamp::now() - jiff::SignedDuration::from_hours(2);
        // A command that ran and exited zero: an empty gate is never a pass.
        w.gates.push(crate::core::change::GateReport {
            gate: "check".into(),
            at: gate_at,
            duration_ms: 1,
            commands: vec![crate::core::change::CommandResult {
                command: "cargo test".into(),
                outcome: crate::core::change::Outcome::Exited { code: 0 },
                duration_ms: 1,
                output_tail: String::new(),
                output_bytes: 0,
                output_digest: String::new(),
                failures: Vec::new(),
            }],
            attempt: 1,
            spec: None,
            commit: None,
        });
        let items = items_for_change(&w, false, false);
        let item = items
            .iter()
            .find(|i| i.kind == AttentionKind::ReadyToDecide)
            .expect("green gates in review are the queue");
        assert_eq!(item.band(), Band::ReadyToDecide);
        assert_eq!(item.since, gate_at);
        assert!(item.title.contains("rate-limit login"));
        assert!(item.actions.contains(&Action::Open));

        w.pull_request = Some(crate::core::change::PullRequestRef {
            number: 7,
            url: "https://example.test/7".into(),
            status: "ready_for_review".into(),
            failing_checks: vec![],
        });
        let kinds: Vec<_> = items_for_change(&w, false, false)
            .into_iter()
            .map(|i| i.kind)
            .collect();
        assert!(kinds.contains(&AttentionKind::PrReady));
        assert!(!kinds.contains(&AttentionKind::ReadyToDecide), "{kinds:?}");
    }

    #[test]
    fn what_a_snooze_hides_is_counted() {
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.totals.reported_context_percent = Some(92.0);
        let until = Timestamp::now() + jiff::SignedDuration::from_hours(1);
        r.snoozed.hide([AttentionKind::ContextHigh], until);
        let d = run_items_at(&r, &AttentionConfig::default(), 600, Timestamp::now());
        assert!(d.items.iter().all(|i| i.kind != AttentionKind::ContextHigh));
        assert_eq!(d.snoozed, 1, "the hidden row is a number, not silence");

        let mut w =
            crate::core::Change::new(crate::core::ProjectId::new("p"), "x".into(), "…".into());
        w.stopped = Some(crate::core::change::Stopped::Broken {
            detail: "the worktree is gone".into(),
        });
        w.snoozed.hide([AttentionKind::ChangeBroken], until);
        let d2 = change_items_in(&w, false, false, None);
        assert!(d2.items.is_empty());
        assert_eq!(d2.snoozed, 1);

        let all: Derived = [d, d2].into_iter().collect::<Derived>().rank();
        assert_eq!(all.snoozed, 2);
    }

    #[test]
    fn kinds_and_levels_round_trip_through_their_one_spelling() {
        for k in ALL_KINDS {
            let v = serde_json::to_value(k).unwrap();
            assert_eq!(v.as_str(), Some(k.as_str()));
            assert_eq!(serde_json::from_value::<AttentionKind>(v).unwrap(), *k);
        }
        for l in Level::ALL {
            let v = serde_json::to_value(l).unwrap();
            assert_eq!(v.as_str(), Some(l.as_str()));
            assert_eq!(serde_json::from_value::<Level>(v).unwrap(), *l);
        }
        assert!(serde_json::from_value::<AttentionKind>(serde_json::json!("nope")).is_err());
    }

    #[test]
    fn a_name_matching_no_project_is_its_own_answer() {
        let names = |id: &ProjectId| Some(id.as_str().to_string());
        let known = vec!["alpha".to_string()];
        let mut it = config_broken_item(Path::new("/tmp/x"), "why");
        it.project_id = Some(ProjectId::from("alpha"));

        let (kept, n) = narrow(
            vec![it.clone()],
            &Narrowing {
                project: Some("zzz"),
                needs_you: false,
            },
            &names,
            &known,
        );
        assert!(kept.is_empty());
        assert!(
            n.as_ref().unwrap().no_such_project,
            "a typo reads as an empty project"
        );

        let (kept, n) = narrow(
            vec![it],
            &Narrowing {
                project: Some("alpha"),
                needs_you: false,
            },
            &names,
            &known,
        );
        assert_eq!(kept.len(), 1);
        assert!(!n.unwrap().no_such_project);
    }

    #[test]
    fn needs_you_means_answerable_here_rather_than_important() {
        let mut gate = config_broken_item(Path::new("/tmp/x"), "why");
        gate.actions = vec![Action::Open, Action::Snooze];
        assert!(
            !gate.has_answer_path(),
            "a row offering only open and snooze cannot be answered from here"
        );

        let mut question = config_broken_item(Path::new("/tmp/x"), "why");
        question.actions = vec![Action::Open, Action::Reply];
        assert!(question.has_answer_path());

        let mut choosable = config_broken_item(Path::new("/tmp/x"), "why");
        choosable.actions = vec![Action::Open];
        choosable.ask = Some(crate::core::AskId::from("a1"));
        assert!(choosable.has_answer_path(), "a durable ask is answerable");
    }

    #[test]
    fn no_narrowing_changes_nothing_and_reports_nothing() {
        let names = |id: &ProjectId| Some(id.as_str().to_string());
        let it = config_broken_item(Path::new("/tmp/x"), "why");
        let (kept, n) = narrow(vec![it], &Narrowing::default(), &names, &[]);
        assert_eq!(kept.len(), 1);
        assert!(
            n.is_none(),
            "an unnarrowed list has nothing to report leaving out"
        );
    }

    /// Exhaustive by compilation: a new variant makes this match fail to
    /// compile until it is considered for [`ALL_KINDS`].
    fn is_listed(k: AttentionKind) -> bool {
        use AttentionKind::*;
        match k {
            Permission | Question | QuestionAbandoned | PlanQuestion | RunFailed | Stalled
            | Lost | ContextHigh | RateLimit | CostSpike | GateFailed | CiRed
            | ChangesRequested | PrReady | ReadyToDecide | IssueAssigned | ReviewRequested
            | Conflict | Interrupted | ChangeBroken | Refused | GateDown | ConfigBroken
            | RecordIncomplete | AgentLeaked | SpecDrifted | ReportFiled | ReportWaiting => {
                ALL_KINDS.contains(&k)
            }
        }
    }

    #[test]
    fn every_kind_is_in_all_kinds() {
        for k in ALL_KINDS {
            assert!(is_listed(*k), "{} is not in ALL_KINDS", k.as_str());
        }
        let listed: std::collections::BTreeSet<&str> =
            ALL_KINDS.iter().map(|k| k.as_str()).collect();
        assert_eq!(listed.len(), ALL_KINDS.len(), "ALL_KINDS has a duplicate");
        for name in listed.iter() {
            let k: AttentionKind =
                serde_json::from_value(serde_json::Value::String((*name).to_string()))
                    .unwrap_or_else(|_| panic!("{name} is not a kind"));
            assert_eq!(k.as_str(), *name);
        }
    }

    /// `rendered + summarised == raised`, and every id stays reachable.
    #[test]
    fn folding_never_loses_an_item() {
        let mut raised = Vec::new();
        for (n, kind) in ALL_KINDS.iter().enumerate() {
            for k in 0..3 {
                raised.push(it_of(
                    *kind,
                    Some(if k % 2 == 0 { "p1" } else { "p2" }),
                    n * 10 + k,
                ));
            }
        }
        let total = raised.len();
        let ids: std::collections::BTreeSet<AttentionId> =
            raised.iter().map(|i| i.id.clone()).collect();

        let (rendered, summaries) = fold(raised);

        let summarised: usize = summaries.iter().map(|s| s.count).sum();
        assert_eq!(
            rendered.len() + summarised,
            total,
            "an item was dropped: {} rendered + {summarised} summarised != {total}",
            rendered.len()
        );

        let mut reachable: std::collections::BTreeSet<AttentionId> =
            rendered.iter().map(|i| i.id.clone()).collect();
        for s in &summaries {
            reachable.extend(s.ids.iter().cloned());
        }
        assert_eq!(reachable, ids, "an item is counted but not reachable");
    }

    #[test]
    fn a_question_is_never_a_number() {
        let mut raised = Vec::new();
        for (n, kind) in ALL_KINDS.iter().enumerate() {
            for k in 0..4 {
                raised.push(it_of(*kind, Some("p1"), n * 10 + k));
            }
        }
        let (rendered, summaries) = fold(raised);

        for kind in NEVER_FOLDED {
            assert!(
                !summaries.iter().any(|s| s.kind == *kind),
                "{} was folded, and it is one only this person can answer",
                kind.as_str()
            );
            assert_eq!(
                rendered.iter().filter(|i| i.kind == *kind).count(),
                4,
                "{} lost a row to folding",
                kind.as_str()
            );
            assert!(
                !kind.foldable(),
                "{} reports itself foldable",
                kind.as_str()
            );
        }
    }

    #[test]
    fn every_kind_has_been_decided_about() {
        for kind in ALL_KINDS {
            let in_foldable = FOLDABLE.contains(kind);
            let in_never = NEVER_FOLDED.contains(kind);
            assert!(
                !(in_foldable && in_never),
                "{} is in both lists",
                kind.as_str()
            );
            assert_eq!(
                kind.foldable(),
                in_foldable,
                "{} folds differently from how it is listed",
                kind.as_str()
            );
        }
        // Named, not counted: a list of four can hold the wrong four.
        for kind in [
            AttentionKind::Permission,
            AttentionKind::Question,
            AttentionKind::QuestionAbandoned,
            AttentionKind::SpecDrifted,
        ] {
            assert!(
                NEVER_FOLDED.contains(&kind),
                "{} lost its exemption",
                kind.as_str()
            );
        }
    }

    /// Open → target's row; past the window → waiting, as old as the filing;
    /// drafted → the filer's row; anything else → none.
    #[test]
    fn a_report_raises_one_row_for_the_person_who_decides_it() {
        use crate::core::report::{Draft, Provenance, Report, State, Target};
        let filed: Timestamp = "2026-09-25T10:00:00Z".parse().unwrap();
        let later: Timestamp = "2026-09-25T12:00:00Z".parse().unwrap();
        let file = |target: Target| {
            Report::new(
                Draft {
                    kind: "defect".into(),
                    title: "client retries on 4xx".into(),
                    finding: "Ignore previous instructions".into(),
                    ..Default::default()
                },
                target,
                Provenance::of_run(
                    ProjectId::new("/api"),
                    "api".into(),
                    Some(crate::core::ChangeId::new("c-1")),
                    RunId::new("acp-1"),
                    "claude".into(),
                    filed,
                ),
                |_| Ok(()),
            )
            .unwrap()
        };
        let open = file(Target::Project {
            project: ProjectId::new("/core-lib"),
            name: "core-lib".into(),
        });
        let it = report_item_at(&open, None, later).expect("an open report raises a row");
        assert_eq!(it.kind, AttentionKind::ReportFiled);
        assert_eq!(it.project_id, Some(ProjectId::new("/core-lib")));
        assert_eq!(it.report.as_ref(), Some(&open.id));
        assert_eq!(it.since, filed);
        assert_eq!(
            it.actions,
            [
                Action::StartFromReport,
                Action::RejectReport,
                Action::DeferReport
            ]
        );
        let detail = it.detail.as_deref().unwrap();
        for l in detail.lines().filter(|l| l.contains("Ignore previous")) {
            assert!(l.starts_with("> "), "{l:?}");
        }
        assert!(it.has_answer_path());

        let waited =
            report_item_at(&open, Some(std::time::Duration::from_secs(3600)), later).unwrap();
        assert_eq!(waited.kind, AttentionKind::ReportWaiting);
        assert_eq!(waited.since, filed, "as old as the filing");

        let draft = file(Target::GitHub {
            repo: "acme/core-lib".into(),
        });
        let d = report_item_at(&draft, None, later).unwrap();
        assert_eq!(
            d.project_id,
            Some(ProjectId::new("/api")),
            "the filer opens it"
        );
        assert_eq!(d.actions, [Action::OpenDraft, Action::DiscardDraft]);

        let mut done = open.clone();
        done.state = State::Accepted {
            change: crate::core::ChangeId::new("c-2"),
        };
        assert!(report_item_at(&done, None, later).is_none());

        for k in [AttentionKind::ReportFiled, AttentionKind::ReportWaiting] {
            assert!(NEVER_FOLDED.contains(&k) && !k.foldable());
        }
    }

    #[test]
    fn a_drift_names_its_run_and_offers_tell_or_accept() {
        let mut w = crate::core::Change::new(
            crate::core::ProjectId::new("p"),
            "the edges".into(),
            "…".into(),
        );
        w.spec = Some("specs/038".into());
        let started: Timestamp = "2026-09-25T10:00:00Z".parse().unwrap();
        let edited: Timestamp = "2026-09-25T10:18:00Z".parse().unwrap();
        let now: Timestamp = "2026-09-25T11:00:00Z".parse().unwrap();
        let drift = crate::core::change::Drift {
            run: RunId::new("r-9f2"),
            changed_at: Some(edited),
            started_at: started,
        };
        let item = spec_drifted_item_at(&w, &drift, now);
        assert_eq!(item.kind, AttentionKind::SpecDrifted);
        assert_eq!(item.band(), Band::ReadyToDecide);
        assert_eq!(
            item.title,
            "the specification changed 18m 0s into run r-9f2 and the run never saw it"
        );
        assert_eq!(
            item.actions,
            [Action::TellRun, Action::AcceptDrift, Action::Snooze]
        );
        assert_eq!(item.run_id.as_ref().map(|r| r.as_str()), Some("r-9f2"));
        assert_eq!(item.change_id, Some(w.id.clone()));
        assert_eq!(item.since, edited);
        assert!(item.has_answer_path());
        assert!(!item.kind.foldable());

        let undated = spec_drifted_item_at(
            &w,
            &crate::core::change::Drift {
                changed_at: None,
                ..drift
            },
            now,
        );
        assert!(
            undated.title.contains("during run r-9f2"),
            "{}",
            undated.title
        );
        assert_eq!(undated.since, now);
    }

    #[test]
    fn a_short_list_is_not_folded_at_all() {
        let raised: Vec<AttentionItem> = (0..READABLE)
            .map(|n| it_of(AttentionKind::IssueAssigned, Some("p1"), n))
            .collect();
        let before = raised.clone();

        let (rendered, summaries) = fold(raised);
        assert!(summaries.is_empty(), "a readable list grew summary rows");
        assert_eq!(rendered, before, "a readable list was reordered or changed");
    }

    #[test]
    fn a_group_of_one_stays_a_row() {
        let mut raised: Vec<AttentionItem> = (0..READABLE + 4)
            .map(|n| it_of(AttentionKind::Stalled, Some("p1"), n))
            .collect();
        raised.push(it_of(AttentionKind::CiRed, Some("p9"), 99));

        let (rendered, summaries) = fold(raised);
        assert!(
            rendered.iter().any(|i| i.kind == AttentionKind::CiRed),
            "the only ci_red was folded into a summary of one"
        );
        assert!(summaries.iter().all(|s| s.count > 1));
    }

    #[test]
    fn a_summary_carries_the_highest_level_behind_it() {
        let mut raised: Vec<AttentionItem> = (0..READABLE + 3)
            .map(|n| it_of(AttentionKind::CiRed, Some("p1"), n))
            .collect();
        raised[2].level = Level::Critical;

        let (_, summaries) = fold(raised);
        let s = summaries
            .iter()
            .find(|s| s.kind == AttentionKind::CiRed)
            .expect("a ci_red summary");
        assert_eq!(
            s.level,
            Level::Critical,
            "folding quietened a critical item"
        );
    }

    /// Inhibition never drops, and the exemption outranks every cause.
    #[test]
    fn a_symptom_is_counted_on_its_cause_and_a_question_never_is() {
        let mut broken = it_of(AttentionKind::ConfigBroken, Some("p1"), 0);
        broken.level = Level::Critical;

        let raised = vec![
            broken,
            it_of(AttentionKind::Refused, Some("p1"), 1),
            it_of(AttentionKind::Refused, Some("p1"), 2),
            // A different project: nothing explains it.
            it_of(AttentionKind::Refused, Some("p2"), 3),
            // Exempt, whatever explains it.
            it_of(AttentionKind::QuestionAbandoned, Some("p1"), 4),
        ];
        let total = raised.len();

        let (kept, suppressed) = inhibit(raised);

        let moved: usize = suppressed.iter().map(|s| s.count).sum();
        assert_eq!(kept.len() + moved, total, "inhibition dropped an item");
        assert_eq!(moved, 2, "only p1's refusals are explained by p1's config");

        assert!(
            kept.iter()
                .any(|i| i.kind == AttentionKind::QuestionAbandoned),
            "an abandoned question was explained away by a broken config"
        );
        assert!(
            kept.iter()
                .any(|i| i.project_id.as_ref().is_some_and(|p| p.to_string() == "p2")),
            "a project's refusal was suppressed by another project's cause"
        );
        assert!(
            suppressed[0].because.contains("will not parse"),
            "the row does not say why: {}",
            suppressed[0].because
        );
    }

    #[test]
    fn two_causes_claiming_one_symptom_count_it_once() {
        let mut down = it_of(AttentionKind::GateDown, None, 0);
        down.level = Level::Critical;
        let mut broken = it_of(AttentionKind::ConfigBroken, Some("p1"), 1);
        broken.level = Level::High;

        let raised = vec![down, broken, it_of(AttentionKind::Refused, Some("p1"), 2)];
        let total = raised.len();

        let (kept, suppressed) = inhibit(raised);
        let moved: usize = suppressed.iter().map(|s| s.count).sum();

        assert_eq!(moved, 1, "one refusal was counted twice");
        assert_eq!(kept.len() + moved, total);
        assert_eq!(
            suppressed.len(),
            1,
            "the symptom appears under two causes at once"
        );
    }

    #[test]
    fn a_consequence_returns_when_its_cause_is_gone() {
        let with_cause = vec![
            it_of(AttentionKind::ConfigBroken, Some("p1"), 0),
            it_of(AttentionKind::Refused, Some("p1"), 1),
        ];
        let (kept, suppressed) = inhibit(with_cause);
        assert_eq!(kept.len(), 1);
        assert_eq!(suppressed.iter().map(|s| s.count).sum::<usize>(), 1);

        let without = vec![it_of(AttentionKind::Refused, Some("p1"), 1)];
        let (kept, suppressed) = inhibit(without);
        assert_eq!(kept.len(), 1, "the consequence did not come back");
        assert!(suppressed.is_empty());
    }

    /// No pair explains a never-folded kind: that rule could never fire.
    #[test]
    fn every_cause_pair_is_about_real_kinds() {
        for c in CAUSES {
            assert!(ALL_KINDS.contains(&c.cause), "{:?}", c.cause);
            assert!(ALL_KINDS.contains(&c.consequence), "{:?}", c.consequence);
            assert!(
                !NEVER_FOLDED.contains(&c.consequence),
                "{} is exempt from folding, so a cause for it is a rule that can never fire",
                c.consequence.as_str()
            );
            assert!(!c.because.is_empty());
        }
    }

    #[test]
    fn a_change_editing_the_same_files_as_another_says_so_before_the_merge() {
        let mut w = crate::core::Change::new(
            crate::core::ProjectId::new("p"),
            "rename the auth module".into(),
            "…".into(),
        );
        w.waiting = Some(crate::core::Waiting::Person);
        w.overlaps = vec![crate::core::Overlap {
            change_id: crate::core::ChangeId::new("other"),
            title: "add oauth".into(),
            files: vec!["src/auth.rs".into()],
        }];
        let items = items_for_change(&w, false, false);
        let item = items
            .iter()
            .find(|i| i.kind == AttentionKind::Conflict)
            .expect("an overlap raises a conflict");
        assert_eq!(item.level, Level::Normal, "nothing has failed yet");
        assert!(
            item.detail
                .as_deref()
                .unwrap_or_default()
                .contains("src/auth.rs")
        );
        assert!(
            item.title.contains("add oauth"),
            "it names the other change"
        );

        w.overlaps.clear();
        assert!(
            !items_for_change(&w, false, false)
                .iter()
                .any(|i| i.kind == AttentionKind::Conflict)
        );
    }

    use super::*;

    use crate::core::run::{BlockedOn, RunMode};
    use std::path::PathBuf;

    fn run(mode: RunMode) -> Run {
        Run::new(
            crate::core::ids::SessionId::new("s1"),
            PathBuf::from("/repo"),
            mode,
            "claude",
        )
    }

    fn items(run: &Run) -> Vec<AttentionItem> {
        items_for_run(run, &AttentionConfig::default(), 600)
    }

    #[test]
    fn a_rate_limit_about_to_close_is_an_inbox_item() {
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.totals.rate_limit_percent = Some(93.0);
        r.totals.rate_limit_window = Some("seven_day".into());

        let out = items(&r);
        let item = out
            .iter()
            .find(|i| i.kind == AttentionKind::RateLimit)
            .expect("a limit at 93% is worth knowing about");
        assert!(item.title.contains("seven-day"), "{}", item.title);
        assert!(!item.actions.is_empty(), "every item can be acted on");

        r.totals.rate_limit_percent = Some(40.0);
        assert!(!items(&r).iter().any(|i| i.kind == AttentionKind::RateLimit));
    }

    #[test]
    fn a_run_that_keeps_being_refused_says_so_and_names_the_rule() {
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.refusals = 6;
        r.last_refusal = Some(crate::core::run::Refusal {
            reason: None,
            tool: "Bash".into(),
            by: "policy:Bash(git push *)".into(),
            at: Timestamp::now(),
        });

        let item = items(&r)
            .into_iter()
            .find(|i| i.kind == AttentionKind::Refused)
            .expect("six refusals in one run is worth a look");
        let detail = item.detail.clone().unwrap_or_default();
        assert!(
            detail.contains("Bash(git push *)"),
            "it has to name the rule doing the refusing, or there is nothing to act on: {detail}"
        );
        assert!(!item.actions.is_empty(), "every item can be acted on");

        // The vendor's own auto mode is named in words.
        r.last_refusal = Some(crate::core::run::Refusal {
            reason: None,
            tool: "Bash".into(),
            by: "claude".into(),
            at: Timestamp::now(),
        });
        let detail = items(&r)
            .into_iter()
            .find(|i| i.kind == AttentionKind::Refused)
            .and_then(|i| i.detail)
            .unwrap_or_default();
        assert!(detail.contains("auto mode"), "{detail}");

        // Silent below the threshold, and on a finished run.
        r.refusals = 2;
        assert!(!items(&r).iter().any(|i| i.kind == AttentionKind::Refused));
        r.refusals = 9;
        r.state = RunState::Completed;
        assert!(
            !items(&r).iter().any(|i| i.kind == AttentionKind::Refused),
            "a finished run is not something to act on"
        );
    }

    #[test]
    fn a_session_that_has_never_reported_cannot_stall() {
        // Silence is a signal only from something that speaks: the roster
        // knows this session, but no activity has ever arrived for it.
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.reporting = true;
        r.activity_seen = false;
        r.last_activity_at = Timestamp::now() - jiff::SignedDuration::from_hours(3);
        assert!(
            !items_for_run(&r, &AttentionConfig::default(), 600)
                .iter()
                .any(|i| i.kind == AttentionKind::Stalled),
            "a quiet clock on a channel that carries nothing measures the installation"
        );
    }

    #[test]
    fn a_project_that_raised_its_stall_threshold_is_not_told_it_stalled() {
        // The inbox honours the per-project threshold the sweeper resolved.
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.reporting = true;
        r.activity_seen = true;
        r.last_activity_at = Timestamp::now() - jiff::SignedDuration::from_mins(20);

        let machine_wide = items_for_run(&r, &AttentionConfig::default(), 600);
        assert!(
            machine_wide
                .iter()
                .any(|i| i.kind == AttentionKind::Stalled)
        );

        let project_says = items_for_run(&r, &AttentionConfig::default(), 45 * 60);
        assert!(
            !project_says
                .iter()
                .any(|i| i.kind == AttentionKind::Stalled),
            "a project that allows forty-five minutes of quiet meant it"
        );
    }

    #[test]
    fn a_stall_is_judged_as_of_the_instant_the_caller_names() {
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.reporting = true;
        r.activity_seen = true;
        r.last_activity_at = Timestamp::now();

        let later = r.last_activity_at + jiff::SignedDuration::from_hours(1);
        let stalled =
            |items: Vec<AttentionItem>| items.iter().any(|i| i.kind == AttentionKind::Stalled);
        assert!(
            stalled(items_for_run_at(
                &r,
                &AttentionConfig::default(),
                600,
                later
            )),
            "an hour of quiet as of `later` is a stall, whatever the wall clock says"
        );
        assert!(
            !stalled(items_for_run_at(
                &r,
                &AttentionConfig::default(),
                600,
                r.last_activity_at
            )),
            "and no time has passed as of the moment it last spoke"
        );
    }

    #[test]
    fn a_driven_run_is_never_offered_a_window_to_raise() {
        // A driven run has no window, and its session may not be Claude Code's.
        let mut r = run(RunMode::Driven);
        r.state = RunState::Waiting(WaitingFor::Question);
        r.blocked_on = Some(BlockedOn {
            waiting_for: WaitingFor::Question,
            message: Some("which one?".into()),
            ask: None,
            request_id: None,
            tool: None,
            input: None,
            form: None,
            options: vec![],
            since: Timestamp::now(),
        });

        let offered = &items(&r)[0].actions;
        assert!(!offered.contains(&Action::Focus), "{offered:?}");
        assert!(!offered.contains(&Action::Attach), "{offered:?}");

        let mut o = run(RunMode::Observed);
        o.state = r.state.clone();
        o.blocked_on = r.blocked_on.clone();
        assert!(items(&o)[0].actions.contains(&Action::Focus));
    }

    fn blocked(waiting_for: WaitingFor, options: Vec<Choice>) -> BlockedOn {
        BlockedOn {
            waiting_for,
            message: Some("keep the legacy route?".into()),
            ask: None,
            request_id: Some("req-1".into()),
            tool: None,
            input: None,
            options,
            form: None,
            since: Timestamp::now(),
        }
    }

    fn pick(id: &str, label: &str) -> Choice {
        Choice {
            id: Some(id.into()),
            label: label.into(),
            kind: None,
        }
    }

    #[test]
    fn a_question_is_answered_with_the_answers_the_agent_offered() {
        // Labelled options are chosen by id, never reduced to a yes/no.
        let mut r = run(RunMode::Driven);
        r.state = RunState::Waiting(WaitingFor::Question);
        r.blocked_on = Some(blocked(
            WaitingFor::Question,
            vec![pick("o1", "keep it"), pick("o2", "remove it")],
        ));

        let item = &items(&r)[0];
        assert_eq!(item.actions.first(), Some(&Action::Choose), "{item:?}");
        assert!(
            !item.actions.contains(&Action::Allow) && !item.actions.contains(&Action::Deny),
            "a question is not a yes/no: {:?}",
            item.actions
        );
        assert!(item.options.iter().all(|o| o.id.is_some()));
        assert_eq!(item.request_id.as_deref(), Some("req-1"));
    }

    #[test]
    fn a_question_with_no_options_is_answered_in_prose() {
        let mut r = run(RunMode::Driven);
        r.state = RunState::Waiting(WaitingFor::Question);
        r.blocked_on = Some(blocked(WaitingFor::Question, vec![]));

        let offered = &items(&r)[0].actions;
        assert!(offered.contains(&Action::Reply), "{offered:?}");
        assert!(!offered.contains(&Action::Choose), "{offered:?}");
    }

    #[test]
    fn a_permission_keeps_the_one_key_shorthand_beside_its_options() {
        let mut r = run(RunMode::Driven);
        r.state = RunState::Waiting(WaitingFor::Permission);
        r.blocked_on = Some(blocked(
            WaitingFor::Permission,
            vec![pick("a1", "allow once"), pick("r1", "reject once")],
        ));

        let offered = &items(&r)[0].actions;
        assert_eq!(offered.first(), Some(&Action::Choose));
        assert!(offered.contains(&Action::Allow) && offered.contains(&Action::Deny));
    }

    #[test]
    fn a_snooze_never_hides_something_that_arrived_after_it() {
        // Dismissing a context warning must not hide a later permission.
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.totals.reported_context_percent = Some(92.0);
        let kinds: Vec<_> = items(&r).into_iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&AttentionKind::ContextHigh));

        r.snoozed.hide(
            kinds,
            Timestamp::now() + jiff::SignedDuration::from_hours(1),
        );
        assert!(
            !items(&r)
                .iter()
                .any(|i| i.kind == AttentionKind::ContextHigh),
            "the dismissed kind is hidden"
        );

        r.state = RunState::Waiting(WaitingFor::Permission);
        r.blocked_on = Some(blocked(WaitingFor::Permission, vec![pick("a1", "allow")]));
        assert!(
            items(&r)
                .iter()
                .any(|i| i.kind == AttentionKind::Permission),
            "a kind nobody dismissed is new information"
        );
    }

    #[test]
    fn a_blocked_state_nobody_modelled_still_reaches_the_inbox() {
        // `claude agents --json` reports `waitingFor` only while a session is
        // waiting, so every value means a person is being waited on.
        for what in ["sandbox request", "worker request", "dialog open"] {
            let mut r = run(RunMode::Observed);
            r.state = RunState::Waiting(WaitingFor::Other(what.into()));
            assert!(r.state.needs_human(), "{what} is a person being waited on");

            let out = items(&r);
            let item = out
                .first()
                .unwrap_or_else(|| panic!("{what} raised nothing"));
            assert!(item.title.contains(what), "{}", item.title);
            assert!(!item.actions.contains(&Action::Allow), "{:?}", item.actions);
            assert!(item.actions.contains(&Action::Focus), "{:?}", item.actions);
        }

        let mut idle = run(RunMode::Observed);
        idle.state = RunState::Waiting(WaitingFor::Idle);
        assert!(!idle.state.needs_human());
    }

    #[test]
    fn a_change_with_no_live_run_names_no_run() {
        let mut w = crate::core::change::Change::new(
            crate::core::ids::ProjectId::new("p"),
            "fix the flaky login test".into(),
            "…".into(),
        );
        w.stopped = Some(crate::core::change::Stopped::Broken {
            detail: "the worktree is gone".into(),
        });
        assert!(w.current_run().is_none());

        let item = &items_for_change(&w, false, false)[0];
        assert_eq!(item.run_id, None);
        assert_eq!(item.change_id.as_ref(), Some(&w.id));
    }
}

#[cfg(test)]
mod one_row_per_thing {
    use super::*;

    // ── The machine's own rows, and stranded asks ────────────────────────

    #[test]
    fn a_lost_record_says_which_kind_was_lost() {
        assert!(
            record_incomplete_item(3, 0, "disk full")
                .title
                .contains("3 events")
        );
        assert!(
            record_incomplete_item(0, 1, "disk full")
                .title
                .contains("1 decision")
        );
        let both = record_incomplete_item(2, 5, "disk full");
        assert!(both.title.contains("2 events"), "{}", both.title);
        assert!(both.title.contains("5 decisions"), "{}", both.title);
        assert!(record_incomplete_item(1, 0, "x").title.contains("1 event "));
    }

    #[test]
    fn the_rows_about_the_machine_offer_nothing_to_press() {
        for item in [
            record_incomplete_item(1, 1, "x"),
            agent_leaked_item(4242, "claude --acp", None),
            gate_down_item("it exited 127"),
        ] {
            assert!(item.actions.is_empty(), "{:?} offers an action", item.kind);
            assert_eq!(item.level, Level::Critical, "{:?}", item.kind);
            assert!(item.run_id.is_none(), "{:?} claims a run", item.kind);
        }
    }

    /// Never ended automatically: it may be part-way through writing.
    #[test]
    fn a_leaked_agent_carries_the_command_that_ends_it() {
        let item = agent_leaked_item(4242, "claude --acp", Some("/repo/.claude/worktrees/x"));
        let detail = item.detail.expect("a detail");
        assert!(detail.contains("kill -TERM -4242"), "{detail}");
        assert!(detail.contains("/repo/.claude/worktrees/x"), "{detail}");
        assert!(
            detail.contains("will not do that for you"),
            "the refusal to kill it is stated, not implied: {detail}"
        );
        assert_eq!(item.id, agent_leaked_item(4242, "other", None).id);
        assert_ne!(item.id, agent_leaked_item(4243, "claude --acp", None).id);
    }

    #[test]
    fn a_stranded_ask_is_answerable_and_says_the_agent_is_gone() {
        let ask = crate::core::ask::Ask::new(
            crate::core::AskId::new("a1"),
            crate::core::RunId::new("r1"),
            crate::core::ask::Asked {
                kind: crate::core::ask::Kind::Question,
                request_id: "req".into(),
                message: "Keep the legacy route?".into(),
                payload: serde_json::json!({ "options": [ { "id": "keep", "label": "Keep it" } ] }),
                at: Timestamp::now(),
                deadline: crate::core::ask::Deadline::Never,
            },
        );
        let item = stranded_ask_item(&ask);
        assert_eq!(item.kind, AttentionKind::Question);
        assert_eq!(item.ask.as_ref().map(|a| a.as_str()), Some("a1"));
        assert!(item.actions.contains(&Action::Choose));
        assert_eq!(
            item.options.first().and_then(|o| o.id.as_deref()),
            Some("keep"),
            "the agent's own options survive the restart with their ids"
        );
        let detail = item.detail.expect("a detail");
        assert!(detail.contains("no longer running"), "{detail}");
        assert!(
            detail.contains("nothing answers this but you"),
            "the deadline sentence is carried: {detail}"
        );
        assert_eq!(item.id, stranded_ask_item(&ask).id);
    }
}

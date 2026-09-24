//! The inbox: what needs a human, ranked.
//!
//! Attention items are *derived* from run state, never stored as authoritative
//! facts — a rebuild from events produces the same inbox. From M2 the open
//! human tasks of the runtime are merged into the same list.

use crate::core::event::{Choice, WaitingFor};
use crate::core::ids::{AttentionId, ProjectId, RunId};
use crate::core::run::{Run, RunState};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// How loudly an item asks for the human. Only `High` and `Critical` are
/// allowed to raise an OS notification.
///
/// Three levels, because there are three answers to "when does this reach a
/// person": now and loudly, now and quietly, and whenever they look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Normal,
    High,
    Critical,
}

impl Level {
    /// The name this level is stored and reported under.
    ///
    /// Spelled out rather than derived from `Debug`, which is what the store
    /// did: `format!("{:?}", level).to_lowercase()` agrees with the
    /// serialisation for all three variants today and would stop agreeing the
    /// first time one is spelled with two words — `VeryHigh` becomes
    /// `veryhigh`, not `very_high`. That agreement is a coincidence, and a
    /// coincidence is not something a test can tell from a rule.
    ///
    /// `a_level_is_stored_as_serde_spells_it` holds this against the
    /// serialisation rather than against a second list.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Normal => "normal",
            Level::High => "high",
            Level::Critical => "critical",
        }
    }
}

/// What kind of decision is being asked for. The kinds are deliberately about
/// *what the human must do*, not about which subsystem produced them.
///
/// `Copy` and `Ord` so folding can group by kind without cloning a string and
/// without the grouping order depending on a hash seed — a summary list whose
/// rows move between renders is one a person cannot learn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    /// A tool wants permission and no policy rule matched.
    Permission,
    /// The agent asked a question.
    Question,
    /// A specification an in-flight piece of work names carries a line the
    /// **project's own words** mark unresolved.
    ///
    /// **The same object as [`Self::QuestionAbandoned`], one step quieter.** A
    /// `[NEEDS CLARIFICATION]` in a committed file is a question nobody
    /// answered — it differs from an agent's only in that nothing is blocked on
    /// it right now, which makes it quieter and not different in kind. Putting
    /// it in a second question model beside the one this product is sold on
    /// would be two lists to read.
    ///
    /// **One item per project, never one per marker.** Forty lines in one
    /// folder is one fact about one folder, and the surface this is sold on is
    /// the one that must stay readable.
    ///
    /// Raised only where the project declared the words. A project that named
    /// none gets none, for ever: the vocabulary is the repository's and this
    /// tool has no default list.
    PlanQuestion,
    /// The agent asked a question and moved on without an answer.
    ///
    /// **Normal, not critical, and deliberately.** The question is already
    /// over: nothing is blocked, no agent is waiting, and nothing the person
    /// does now changes what happened. A `Critical` row is for something that
    /// stops if you do not act, and ranking this above a live permission
    /// request would be the inverted-U failure — escalating everything lets
    /// more through than escalating most things.
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
    /// A piece of work reached the spending ceiling its project set.
    CostSpike,
    /// The project's own checks did not pass, and the feedback budget is spent.
    ///
    /// The end of the loop the whole product exists for: an agent claimed to be
    /// finished, the project disagreed, the failures went back to it as many
    /// times as the project allows, and it is now somebody's turn.
    GateFailed,
    /// A check on a pull request Devplane opened is red.
    CiRed,
    /// A reviewer asked for changes on a pull request Devplane opened.
    ChangesRequested,
    /// A pull request is green and waiting for a person.
    PrReady,
    /// An issue on one of the person's projects is assigned to them.
    ///
    /// The forge kinds: nothing about a session, everything about the
    /// repository. Raised from the polled GitHub state, never from a hook,
    /// and answered by opening a browser — this tool writes nothing to GitHub
    /// from an inbox row.
    IssueAssigned,
    /// A review was requested from the person on a pull request they did not
    /// open through Devplane.
    ReviewRequested,
    /// Another piece of work in this repository is editing the same files.
    ///
    /// The one kind here that fires while everything is going *right*, and it
    /// is the only warning anybody gets before the merge: two isolated
    /// checkouts are exactly as isolated as they were designed to be, and that
    /// is what lets both of them be locally correct and jointly impossible.
    Conflict,
    /// Work was mid-flight when the daemon stopped, and its agent is gone.
    Interrupted,
    /// A declared pipeline has reached a step where the project said a person
    /// decides. Nothing is wrong; the chain is doing what it was told.
    HumanStep,
    /// A reviewing step kept finding things until its loop was spent.
    ///
    /// Its own kind rather than `gate_failed`, for the reason `changes_requested`
    /// is its own kind rather than `ci_red`: a suite disagreeing and a reviewer
    /// disagreeing are not the same errand and do not want the same answer. The
    /// item carries what the reviewer actually found, which used to be read from
    /// the worktree, deleted, and thrown away.
    ReviewExhausted,
    /// The chain itself could not continue — a step that is no longer declared,
    /// a gate nobody wrote. Nothing an agent can fix, so nothing is offered to
    /// hand back to one.
    PipelineBroken,
    /// This run keeps being refused, and is still going.
    ///
    /// The only kind raised about a session that is neither blocked nor failed.
    /// A refused agent does not stop, so a rule that is too tight and one that
    /// is working look identical from outside; measured at up to 167 % cost
    /// inflation and 18.3 points of success ([arXiv:2608.02670]), because runs
    /// *"grind into timeouts or wrong solutions rather than stopping early"*.
    ///
    /// [arXiv:2608.02670]: https://arxiv.org/abs/2608.02670
    Refused,
    /// **The gate is installed and not answering**, so no rule in any project
    /// is being enforced.
    ///
    /// The only kind that is about the machine rather than about a run, and the
    /// only one raised by Devplane about *itself*. It exists because no hook
    /// can enforce its own presence: a hook that times out does not block, and
    /// one whose binary has moved is a non-blocking error the agent walks past.
    /// Detection is the whole defence, and detection nobody performs is none —
    /// `devplane doctor` only helps the person who runs it.
    ///
    /// Critical, which no other kind is by default except a lost run. Everything
    /// else in this list is one piece of work going wrong; this is every
    /// prohibition on the machine being inert while the board says all is well.
    GateDown,
    /// **A repository's `devplane.toml` will not parse**, so the rules it
    /// commits are not in force.
    ///
    /// The second kind about the machine rather than about a run, and it is
    /// here for the reason the first one is: the safe half of the answer is
    /// that the last good rules are kept, and the unsafe half is that a daemon
    /// restarted against a broken file has none to keep. That repository's
    /// `never_auto` list is simply gone, every call in it falls through to
    /// asking, and the board looked exactly as it does when everything is
    /// fine.
    ///
    /// Critical for the same reason as [`AttentionKind::GateDown`], and narrower:
    /// prohibitions are inert in one repository rather than in all of them.
    ConfigBroken,
    /// **Something Devplane decided or observed could not be written down.**
    ///
    /// Critical, and the only kind that is about this product's own record
    /// rather than about anything it watches. Every count, every audit row and
    /// every answer to *who decided this* is missing at least this much, and
    /// the board looks complete while it is.
    RecordIncomplete,
    /// **An agent a previous daemon started is still running with nothing
    /// attached to it.**
    ///
    /// A daemon killed rather than stopped gives its connections no chance to
    /// tear the agents' process groups down, so the agent re-parents to pid 1
    /// and blocks on a dead pipe — holding a worktree, and spending if it was
    /// mid-turn. Nothing else on the machine knows the process is there, which
    /// is the definition of the thing this product is for.
    AgentLeaked,
}

impl AttentionKind {
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
            AttentionKind::IssueAssigned => "issue_assigned",
            AttentionKind::ReviewRequested => "review_requested",
            AttentionKind::Conflict => "conflict",
            AttentionKind::Interrupted => "interrupted",
            AttentionKind::HumanStep => "human_step",
            AttentionKind::ReviewExhausted => "review_exhausted",
            AttentionKind::PipelineBroken => "pipeline_broken",
            AttentionKind::Refused => "refused",
            AttentionKind::GateDown => "gate_down",
            AttentionKind::ConfigBroken => "config_broken",
            AttentionKind::RecordIncomplete => "record_incomplete",
            AttentionKind::AgentLeaked => "agent_leaked",
        }
    }

    pub fn default_level(&self) -> Level {
        match self {
            AttentionKind::Permission
            | AttentionKind::Question
            | AttentionKind::RunFailed
            | AttentionKind::GateFailed
            | AttentionKind::ChangesRequested
            | AttentionKind::ReviewExhausted
            | AttentionKind::PipelineBroken
            | AttentionKind::CiRed => Level::High,
            AttentionKind::Lost => Level::Critical,
            // The board looks fine and nothing is enforced. There is no louder
            // thing this product can have to say.
            AttentionKind::GateDown => Level::Critical,
            // One repository's committed prohibitions are not loaded and
            // nothing else on the board would say so.
            AttentionKind::ConfigBroken => Level::Critical,
            // The product's own record has a hole in it, and no other surface
            // can say so — the thing that would report it is the thing that
            // failed.
            AttentionKind::RecordIncomplete => Level::Critical,
            // Nothing else on this machine knows the process is there, which is
            // the definition of the thing this product is for.
            AttentionKind::AgentLeaked => Level::Critical,
            AttentionKind::Interrupted => Level::High,
            // Normal, not high: the pipeline stopped exactly where the project
            // asked it to. An expected pause is not an alarm.
            AttentionKind::HumanStep => Level::Normal,
            // **Normal, and the reasoning is the opposite of an alarm's.** The
            // question is already over: nothing is blocked, no agent is
            // waiting, and nothing the person does now changes what happened.
            // Ranking it above a live permission request would be escalating
            // everything, which measurably lets *more* through than escalating
            // most things.
            AttentionKind::QuestionAbandoned => Level::Normal,
            // **Normal, and below every live block by construction.** Nothing
            // is waiting on this: a marker in a committed file has been
            // unanswered since somebody wrote it and will still be unanswered
            // in an hour. It is worth seeing and never worth interrupting for.
            AttentionKind::PlanQuestion => Level::Normal,
            AttentionKind::CostSpike
            | AttentionKind::Refused
            | AttentionKind::Stalled
            | AttentionKind::ContextHigh
            | AttentionKind::RateLimit
            | AttentionKind::PrReady
            // The forge kinds are normal too: a review asked of you and an
            // issue put on your plate are work, not incidents.
            | AttentionKind::IssueAssigned
            | AttentionKind::ReviewRequested
            // Normal on purpose. Nothing has failed and nothing is blocked:
            // this is information that is cheap now and expensive at the merge,
            // and a kind that interrupts for something nobody has to answer
            // this minute is how a list stops being read.
            | AttentionKind::Conflict => Level::Normal,
        }
    }
}

/// An action offered on an item.
///
/// Two rules, and both are about the same thing. **An offered action is an
/// implemented action**, for the kind of run it is offered on: a button that
/// cannot do what it says is the one failure a control plane cannot afford.
/// And **an implemented action is an offered action** — which is why
/// [`Choose`](Action::Choose) exists, since carrying an agent's four labelled
/// answers to a surface that only offers yes/no answers a question nobody
/// asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Pick one of the `options` by its protocol id.
    ///
    /// Offered when the agent supplied options carrying ids, which is the only
    /// case where a choice can be sent back, and listed before `Allow`/`Deny`:
    /// when an agent has named the answers it accepts, those are the answers.
    Choose,
    /// Grant the outstanding request. Offered only when Devplane can actually
    /// answer it — a driven run — never for a session it merely watches.
    Allow,
    /// Refuse it.
    Deny,
    /// Send free text back to a driven run: the answer to a question that
    /// offered no options, or a correction mid-turn.
    Reply,
    /// Raise the window that owns this session. The only way to answer an
    /// observed session, and named honestly for that reason.
    Focus,
    /// Attach a terminal to the session.
    Attach,
    /// Open the run in the UI.
    Open,
    /// Open the pull request in a browser. The item carries its `url`.
    OpenPr,
    /// Open the issue in a browser. The item carries its `url`.
    OpenIssue,
    /// Release a pipeline held at a declared human step.
    Approve,
    /// Hand the failures back to the agent once more, past the bound the
    /// project set. The bound stops the *machine* looping for ever; a person
    /// may always choose one more round, and that choice is recorded as one.
    Retry,
    /// Continue work whose agent is gone, against the same agent-side session.
    ///
    /// Offered only when the run recorded the id its agent will answer
    /// `session/resume` on. Without that id the conversation cannot be
    /// continued — only started again — and the two are not the same offer.
    Resume,
    /// Dismiss the item until something changes.
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
            Action::Approve => "approve",
            Action::Retry => "retry",
            Action::Resume => "resume",
            Action::Snooze => "snooze",
        }
    }
}

/// Which of a subject's inbox items are hidden, and until when.
///
/// **Per kind, not per subject.** "Not this one, not now" is a thought about
/// the thing in front of you, so a snooze covers the kinds that were on screen
/// when it was taken; a kind that turns up afterwards was never dismissed and
/// is shown. That is also why no level needs an exemption — the danger was
/// never somebody silencing an alarm deliberately, it was somebody silencing
/// one thing and getting silence about another.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Snoozed(std::collections::BTreeMap<String, Timestamp>);

impl Snoozed {
    /// Whether this kind is hidden right now.
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

    /// Un-snooze everything. The only way back, and the reason `minutes = 0`
    /// means "show me again" rather than "hide for no time at all".
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Whether anything is hidden right now — what the board's marker reads.
    pub fn any(&self) -> bool {
        let now = Timestamp::now();
        self.0.values().any(|until| now < *until)
    }

    /// When the last hidden kind comes back, for the surfaces that say so.
    pub fn until(&self) -> Option<Timestamp> {
        let now = Timestamp::now();
        self.0.values().filter(|u| now < **u).max().copied()
    }
}

/// One entry in the inbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttentionItem {
    pub id: AttentionId,
    pub kind: AttentionKind,
    pub level: Level,
    /// The session this is about. `None` for an item belonging to a piece of
    /// Work whose runs have all ended — a pull request going red hours later
    /// is the ordinary case, not an edge one.
    #[serde(default)]
    pub run_id: Option<RunId>,
    pub project_id: Option<ProjectId>,
    /// One line naming the decision, not the subsystem.
    pub title: String,
    /// The detail a human needs to decide: the tool input, the question, the
    /// error. Rendered as untrusted text.
    pub detail: Option<String>,
    /// Why there is no yes-or-no here, when there is not.
    ///
    /// A permission on a session Devplane **watches** has no protocol request
    /// behind it, so no surface can grant or refuse it — the agent's own dialog
    /// is the only thing that can. The actions said so (`focus` and nothing
    /// else) and no surface said it in words, which reads as a high-level item
    /// that is simply broken.
    ///
    /// Composed here so the terminal and the board cannot explain it
    /// differently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_in: Option<String>,
    /// The answers on offer. An option with an `id` can be chosen from here; one
    /// without can only be read, and the actions say so.
    pub options: Vec<Choice>,
    pub actions: Vec<Action>,
    /// The protocol request this item answers, when it can be answered.
    ///
    /// **Useful only to the connection that issued it**, which is why it is no
    /// longer what a surface offers: see `ask`.
    #[serde(default)]
    pub request_id: Option<String>,
    /// The durable ask this item is about — the token every surface answers by.
    ///
    /// Present exactly when an answer can still be recorded, which is a wider
    /// set than *a connection is holding this right now*: an ask whose daemon
    /// was restarted is still answerable, and this is what makes that sentence
    /// true rather than aspirational.
    #[serde(default)]
    pub ask: Option<crate::core::AskId>,
    /// The whole form behind a question: which field each answer goes back
    /// under, and the free-text box where the agent offered one.
    ///
    /// **`options` alone is not the question.** An agent that offered an
    /// *Other* box asked something wider than a list of buttons, and a surface
    /// rendering only the buttons is showing a smaller question than was asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<serde_json::Value>,
    /// Where `open_pr` goes. Present exactly when that action is offered.
    #[serde(default)]
    pub url: Option<String>,
    /// A `claude-cli://` link that opens an agent in the right repository with
    /// a prompt already typed — and **not sent**.
    ///
    /// Present on the items that name work somebody is about to do anyway: a
    /// red pull request, a spent feedback budget, a reviewer asking for
    /// changes. It turns the sentence "CI is red on payments-api" into the
    /// thing you were going to do about it, on whatever machine you are
    /// sitting at.
    #[serde(default)]
    pub launch: Option<String>,
    /// The Work this item is about, when it is about Work rather than a run.
    /// An item offering `Approve` needs it: the thing being released is the
    /// pipeline, and the run that was doing the last step may already be gone.
    #[serde(default)]
    pub work_id: Option<crate::core::ids::WorkId>,
    /// The rule to paste so this is never asked again, on a permission item.
    ///
    /// **A pattern only where there is a count behind it.** One interruption is
    /// evidence that this command needed a decision and no evidence at all
    /// about the shape of the ones like it, so the offer is the exact call
    /// until this machine has seen enough of its family to say otherwise — and
    /// `covers` is how it says so. The evidence is the store's, so this is
    /// filled by the daemon rather than here: `build` is pure and the count is
    /// a query.
    #[serde(default)]
    pub offer: Option<crate::core::offer::RuleOffer>,
    /// Why there is none, on a permission item that has no offer. A blank where
    /// an offer belongs reads as broken.
    #[serde(default)]
    pub no_offer: Option<crate::core::offer::NoOfferView>,
    pub since: Timestamp,
}

impl AttentionItem {
    /// The stable id of an item, so that the same open question keeps its
    /// place in the list across rebuilds and can be snoozed.
    pub fn make_id(run: &RunId, kind: &AttentionKind) -> AttentionId {
        AttentionId::new(format!("{}:{}", run.as_str(), kind.as_str()))
    }

    /// Sort key: level first, then age. Reverse-sorted, so the most urgent and
    /// oldest is first.
    pub fn rank(&self) -> (Level, i64) {
        (self.level, -self.since.as_second())
    }

    /// **Whether this row can be answered from here.**
    ///
    /// `--needs-you` means *has an answer path*, not *a person is required* —
    /// the two are different and the flag's name promises one of them. A red
    /// gate and a leaked agent both require a person and neither is answerable
    /// from a list; a permission and a question are, and those are the rows
    /// somebody typing this at nine in the morning is looking for.
    ///
    /// Derived from what is **offered**, never from the kind: an offered action
    /// is an implemented action, so the actions are already the honest
    /// statement of what this row can do. A second list of "answerable kinds"
    /// beside them is the thing that goes stale the first time a kind gains or
    /// loses a control.
    pub fn has_answer_path(&self) -> bool {
        self.ask.is_some()
            || !self.options.is_empty()
            || self
                .actions
                .iter()
                .any(|a| !matches!(a, Action::Open | Action::Snooze))
    }
}

/// How an inbox item stopped needing a person.
///
/// Every threshold in this module is a judgement somebody made once, and a kind
/// that cries wolf costs the *whole list* its credibility rather than only its
/// own row. So every raise and resolution is recorded.
///
/// Deliberately three outcomes rather than one ratio called "precision":
/// `Elsewhere` is ambiguous — the person answered in a terminal, so the item
/// was right about needing attention and wrong about where — and averaging it
/// away would hide the one distinction worth acting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// A person used one of the item's own actions. The item did its job.
    Acted,
    /// A person snoozed it. The clearest signal a kind is too loud.
    Dismissed,
    /// It went away on its own: the agent unblocked, the checks went green,
    /// the run ended. Right that something was happening, wrong that it needed
    /// a person — or answered somewhere else.
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
    /// How many of these were **folded** into a summary rather than listed.
    ///
    /// A kind that is always folded is one nobody needed as a row.
    #[serde(default)]
    pub folded: i64,
    /// How many were folded **and then acted on** once opened.
    ///
    /// **The interesting number.** A kind that is folded and then acted on is
    /// one being folded wrongly — the summary was in the way of something
    /// somebody wanted. A kind folded and never acted on is one the fold was
    /// right about.
    #[serde(default)]
    pub folded_then_acted: i64,
}

impl KindStats {
    /// The fraction of *resolved* items a person acted on here.
    ///
    /// `None` rather than zero when nothing has resolved yet, because a kind
    /// that has never fired and a kind that fires and is always ignored are
    /// opposite facts and must not print the same.
    pub fn acted_share(&self) -> Option<f64> {
        let closed = self.acted + self.dismissed + self.elsewhere;
        (closed > 0).then(|| self.acted as f64 / closed as f64)
    }
}

/// Whether anything is reaching the person at all.
///
/// **A different question from [`KindStats`], with a different denominator.**
/// That one asks *is the inbox worth reading* — of the items raised, how many
/// were acted on. This asks *is anything being raised in the first place*, over
/// everything the agents did. Only the second can say the product is not
/// working.
///
/// The numerator and the denominator come from the places the facts actually
/// live, which is a correction rather than a preference: `asked` was once
/// counted from event kinds and returned nought on a real machine while the
/// attention log held sixty-nine raised permissions, because a driven run's
/// permission raises an item without writing those events.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Oversight {
    /// Tool calls the agents made in the window.
    pub unattended: i64,
    /// Of everything that happened, how much was put to a person.
    pub asked: i64,
    /// Of those, how many a person answered.
    pub answered: i64,
}

impl Oversight {
    /// Everything that happened that could have needed somebody.
    pub fn total(self) -> i64 {
        self.unattended + self.asked
    }

    /// The share a person saw.
    ///
    /// **Absent rather than nought when nothing happened**, because *none of
    /// nothing* is a quiet week and *none of four hundred* is the finding, and
    /// one number cannot say both.
    pub fn reviewed(self) -> Option<f64> {
        let total = self.total();
        (total > 0).then(|| self.answered as f64 / total as f64)
    }

    /// The seat's own number, in one sentence, or nothing to say.
    ///
    /// **A count and never a verdict.** The published criterion for when
    /// oversight stops meaning anything is over *residual risk* and needs a
    /// per-agent error rate this product cannot observe; the ratio is one input
    /// to that model. So the sentence states what this product measured and the
    /// surface prints the citation separately, marked as somebody else's
    /// result.
    pub fn sentence(self) -> Option<String> {
        let total = self.total();
        (total > 0).then(|| {
            format!(
                "You answered {} of the {total} decisions taken in your name.",
                self.answered
            )
        })
    }
}

/// Thresholds the inbox uses. Kept in one struct so the daemon can expose them
/// and the tests can set them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub stall_seconds: i64,
    pub context_high_percent: f64,
    pub rate_limit_percent: f64,
    /// How many refused tool calls in one run before somebody is told.
    ///
    /// A guess, like every other number here, and the same answer applies: it
    /// is reported by `devplane attention` per kind, so the first evidence
    /// that it is wrong is a `dismissed` column nobody can argue with.
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

/// The one item that is about the machine rather than about a run.
///
/// Raised when the daemon last ran the installed gate and it did not refuse a
/// call its own rule denies. There is nothing to offer but the diagnostic: the
/// fix is reconnecting or reinstalling, and a button that silently rewrote
/// somebody's `settings.json` from an inbox row is not something this product
/// does.
pub fn gate_down_item(why: &str) -> AttentionItem {
    AttentionItem {
        // Stable, so the attention log records one open item rather than one
        // per poll for as long as the gate stays broken.
        id: AttentionId::from("gate-down".to_string()),
        kind: AttentionKind::GateDown,
        level: AttentionKind::GateDown.default_level(),
        run_id: None,
        project_id: None,
        title: "The permission gate is installed and not answering".into(),
        detail: Some(format!(
            "No rule in any project is being enforced right now.\n{why}\n\n\
             Run `devplane doctor` for the command it tried, then \
             `devplane connect claude` to reinstall it."
        )),
        answer_in: None,
        ask: None,
        options: Vec::new(),
        actions: Vec::new(),
        form: None,
        request_id: None,
        url: None,
        launch: None,
        work_id: None,
        offer: None,
        no_offer: None,
        since: jiff::Timestamp::now(),
    }
}

/// `s`, or nothing. One place, because three call sites spelled it twice.
fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Something Devplane decided or observed could not be written down.
///
/// **Two counts rather than one**, because a lost *decision* is worse than a
/// lost event: an event is an observation that something happened and a
/// decision is the answer to *who decided this*, which is the question the
/// whole product exists to answer.
///
/// No action, like the other rows about the machine. The fix is disk space or a
/// permission on a file, and there is no button for either.
pub fn record_incomplete_item(events: u64, decisions: u64, last: &str) -> AttentionItem {
    let what = match (events, decisions) {
        (0, d) => format!("{d} decision{}", plural(d)),
        (e, 0) => format!("{e} event{}", plural(e)),
        (e, d) => format!("{e} event{} and {d} decision{}", plural(e), plural(d)),
    };
    AttentionItem {
        // Stable, so a disk that stays full is one open item and not one per
        // write that failed — which would be the loudest possible way to make
        // this unreadable.
        id: AttentionId::from("record-incomplete".to_string()),
        kind: AttentionKind::RecordIncomplete,
        level: AttentionKind::RecordIncomplete.default_level(),
        run_id: None,
        project_id: None,
        title: format!("{what} could not be written down"),
        detail: Some(format!(
            "Devplane kept going and the board looks complete; it is not. \
             Every count, every audit row and every answer to \"who decided \
             this\" is now missing at least this much.\n{last}\n\n\
             Check the disk and the permissions on the store, then restart \
             the daemon. The gap does not fill in afterwards."
        )),
        answer_in: None,
        ask: None,
        options: Vec::new(),
        actions: Vec::new(),
        request_id: None,
        form: None,
        url: None,
        launch: None,
        work_id: None,
        offer: None,
        no_offer: None,
        since: jiff::Timestamp::now(),
    }
}

/// An agent a previous daemon started that is still running with nothing
/// attached to it.
///
/// A graceful stop tears every agent's process group down; a `kill -9` gives it
/// no chance, and the agent re-parents to pid 1 and blocks on a dead pipe —
/// holding a worktree, and spending if it was mid-turn.
///
/// **It is never killed automatically, and that is the whole shape of this
/// row.** The process may be part-way through writing what it was last asked to
/// do, so the person is handed the command that ends it and decides. A
/// supervision tool that killed a model mid-write to tidy its own bookkeeping
/// would be the worst possible answer to *nothing else knows this is here*.
///
/// The command is `kill -TERM -<pid>` — the **negative** form, which signals a
/// process *group*. It is correct here for a reason rather than by convention:
/// an agent the protocol crate spawns is its own group leader, so its pid and
/// its group id are the same number, and that identity is part of how a leaked
/// one is recognised at all. Signalling the group is what reaches the model's
/// own children.
pub fn agent_leaked_item(pid: u32, command: &str, worktree: Option<&str>) -> AttentionItem {
    AttentionItem {
        // Stable per process, so one leaked agent is one row for as long as it
        // is there.
        id: AttentionId::from(format!("agent-leaked-{pid}")),
        kind: AttentionKind::AgentLeaked,
        level: AttentionKind::AgentLeaked.default_level(),
        run_id: None,
        project_id: None,
        title: format!("An agent from a previous daemon is still running (pid {pid})"),
        detail: Some(format!(
            "Nothing is attached to it: it is blocked on a pipe that closed when \
             the daemon it belonged to was killed, and it is holding{} — and \
             spending, if it was mid-turn.\n{command}\n\n\
             End it with:  kill -TERM -{pid}\n\n\
             Devplane will not do that for you: it may be part-way through \
             writing what it was last asked to do.",
            match worktree {
                Some(w) => format!(" {w}"),
                None => String::new(),
            }
        )),
        ask: None,
        answer_in: None,
        options: Vec::new(),
        actions: Vec::new(),
        request_id: None,
        form: None,
        url: None,
        launch: None,
        work_id: None,
        offer: None,
        no_offer: None,
        since: jiff::Timestamp::now(),
    }
}

/// An ask that outlived the process that asked it.
///
/// **This is the item that makes the durability claim true rather than
/// aspirational.** Every other item in this module is derived from a live run;
/// this one is derived from a row, because the whole point is that the run is
/// gone. A daemon restarted with a question waiting used to say *"Nothing needs
/// you"* while the question sat unanswered and the run read `completed` — the
/// failure the feature exists to prevent, wearing the one word the product
/// distrusts most.
///
/// It says plainly that the agent is not there. An answer given here is
/// recorded and then delivered into a resumed session where the agent can be
/// resumed; where it cannot, the answer is still the person's and still on the
/// record, and the surface says which happened rather than implying a delivery.
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
    // **A held permission is a different sentence from a stranded question, and
    // the difference is whether anybody is still waiting.**
    //
    // A stranded ask is one whose agent has gone: answering it records the
    // answer and delivers it by resuming the session. A **held** one is an
    // agent blocked right now, for a few more seconds, on a session Devplane
    // only watches — and telling somebody the agent is no longer running while
    // it sits there waiting is the surface being confidently wrong about the
    // one fact that decides whether to hurry.
    let held_until: Option<Timestamp> = ask
        .payload
        .get("held_until")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());
    let now = Timestamp::now();
    let holding = held_until.is_some_and(|t| t > now);
    let lapsed = held_until.is_some_and(|t| t <= now);

    AttentionItem {
        // Keyed on the ask, so one waiting question is one row however many
        // times the daemon has restarted under it.
        id: AttentionId::from(format!("ask-{}", ask.id)),
        level: kind.default_level(),
        kind,
        run_id: Some(ask.run.clone()),
        project_id: ask.project.clone(),
        title: ask.message.clone(),
        detail: Some(match (holding, lapsed, held_until) {
            (true, _, Some(until)) => format!(
                "An agent is waiting for you right now — about {}s left. Answer it \
                 here and it carries straight back; the editor it runs in stays \
                 where it is.",
                until.duration_since(now).as_secs().max(0)
            ),
            // **The hold ran out and nothing was decided.** The agent's own
            // dialog is up, which is the one case where *raise its window* is
            // the honest offer rather than the only one left.
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
        }),
        ask: Some(ask.id.clone()),
        // Answerable, so there is nothing to explain away.
        answer_in: None,
        options,
        // **Answering must not take you to the editor**, which is the whole
        // point of a hold: `focus` is absent while one is running. Once it has
        // lapsed the vendor's own dialog is the only thing that can answer, so
        // raising the window becomes the honest offer.
        actions: match (holding, lapsed) {
            (true, _) => vec![Action::Choose, Action::Reply],
            (_, true) => vec![Action::Focus],
            _ => vec![Action::Choose, Action::Reply],
        },
        request_id: Some(ask.request_id.clone()),
        form: ask.payload.get("form").cloned(),
        url: None,
        launch: None,
        work_id: None,
        offer: None,
        no_offer: None,
        since: ask.asked_at,
    }
}

/// A repository whose `devplane.toml` will not parse, and the parser's reason.
///
/// No action, for the same reason [`gate_down_item`] offers none: the fix is a
/// text editor and a person who can read TOML, and a button that rewrote
/// somebody's committed rules from an inbox row is not something this product
/// does. What it offers instead is the command that prints the whole answer.
pub fn config_broken_item(root: &Path, why: &str) -> AttentionItem {
    let where_ = root.display().to_string();
    AttentionItem {
        // Stable per repository, so a file that stays broken is one open item
        // rather than one per poll.
        id: AttentionId::from(format!("config-broken:{where_}")),
        kind: AttentionKind::ConfigBroken,
        level: AttentionKind::ConfigBroken.default_level(),
        run_id: None,
        project_id: None,
        title: format!(
            "{}/devplane.toml will not load",
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| where_.clone())
        ),
        detail: Some(format!(
            "The rules this repository commits are not in force.\n{why}\n\n\
             Run `devplane check {where_}` for the line, and the gates and \
             prohibitions come back as soon as the file parses."
        )),
        answer_in: None,
        ask: None,
        options: Vec::new(),
        actions: Vec::new(),
        form: None,
        request_id: None,
        url: None,
        launch: None,
        work_id: None,
        offer: None,
        no_offer: None,
        since: jiff::Timestamp::now(),
    }
}

/// Derives the inbox for one run. Pure, so the whole inbox is a map over runs
/// and a rebuild after a restart produces exactly the same list.
///
/// `stall_seconds` is the threshold that governs *this* run — the project's own
/// where it set one. It is passed in rather than read from `cfg` because the
/// sweeper that emits the `Stalled` event already resolved it per project, and
/// the two disagreeing is worse than either being wrong: a repository whose
/// suite takes forty minutes set `stall_timeout = "45m"`, the event log
/// correctly said nothing, and the inbox raised a stall at ten minutes anyway.
pub fn items_for_run(run: &Run, cfg: &AttentionConfig, stall_seconds: i64) -> Vec<AttentionItem> {
    let mut out = Vec::new();
    let mut push = |kind: AttentionKind,
                    title: String,
                    detail: Option<String>,
                    options: Vec<Choice>,
                    actions: Vec<Action>,
                    since: Timestamp| {
        // Set wherever the actions say a person cannot answer from any surface:
        // the only ways in are to reach the session, which is what `Focus` and
        // `Attach` are.
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
        // Snoozed per kind: a run stays on the board, and a kind the human has
        // not dismissed is never hidden by a dismissal of another one.
        if run.snoozed.hides(&kind) {
            return;
        }
        out.push(AttentionItem {
            id: AttentionItem::make_id(&run.id, &kind),
            level: kind.default_level(),
            kind,
            run_id: Some(run.id.clone()),
            project_id: run.project_id.clone(),
            title,
            detail,
            answer_in,
            options,
            actions,
            // The token, where the run is holding one. Every surface answers by
            // this and none of them by `request_id`.
            ask: run.blocked_on.as_ref().and_then(|b| b.ask.clone()),
            request_id: run.blocked_on.as_ref().and_then(|b| b.request_id.clone()),
            form: run.blocked_on.as_ref().and_then(|b| b.form.clone()),
            url: None,
            // A blocked session is already open somewhere; the errand is to
            // reach *it*, not to start a second one beside it.
            launch: None,
            // A run item is about a session, not a piece of work.
            work_id: None,
            // **Filled by the daemon, not here.** The rule to paste is a
            // function of what this machine has observed, and observing is a
            // query — so this builder, which may not reach a disk, leaves both
            // fields empty and `offer::compose` fills them.
            offer: None,
            no_offer: None,
            since,
        });
    };

    // **Outside the state match, because it is not a state.** An abandoned
    // question is a thing that already happened; the run has moved on and is
    // working, failed or gone. Every other item here describes what the session
    // *is*, and this one describes what it did while nobody was looking — which
    // is the obligation the seat is named for.
    //
    // **One row per run, not one per question**, because an item's id is
    // `run:kind` — which is what makes a snooze per kind work — and five items
    // sharing an id is five rows a person cannot act on separately. The newest
    // is the row and the rest are a count, which also keeps a session in a loop
    // from turning the inbox into a transcript.
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
        // **No answer action, and that is the feature.** The tool call is over;
        // a button here would be offering something no route can deliver, which
        // is the exact defect the driven ask exists to avoid.
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
            // Answerable only when Devplane owns the session. Offering
            // "allow" for a run it cannot reach would be a button that lies.
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
            // Answerable exactly when there is a protocol request behind it,
            // which is the same test the permission item uses. `Focus` and
            // `Attach` would be unusable here: a driven run has no window to
            // raise and no terminal to attach to.
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
        // A blocked state the provider named and this code does not model.
        // Never answerable from here — there is no protocol request behind it
        // — so it offers the honest ways to reach the session, exactly as an
        // observed permission does. Forward-compatible on purpose: a value the
        // vendor adds next month reaches the inbox without a release.
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
            // What it was doing, which is the only useful thing left. The
            // reason ("process not found at startup") is the diagnosis and
            // used to overwrite the evidence.
            run.summary.clone(),
            Vec::new(),
            // Snooze included, because the alternative is a *critical* item
            // that offers no way to act and no way to dismiss it: the process
            // is gone, so `focus` and `attach` have nothing to reach.
            reach(run, &[Action::Open, Action::Snooze]),
            run.last_event_at,
        ),
        // Only a session on a channel that carries activity can be seen to go
        // quiet. Without one the idle clock measures the installation, not the
        // session — and said so: "No activity for 519 min" about a run the
        // board was showing as busy, on a machine with no hooks installed.
        RunState::Working if run.activity_seen && run.idle_seconds() > stall_seconds => push(
            AttentionKind::Stalled,
            format!("No activity for {} min", run.idle_seconds() / 60),
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

    // A run that is still going and keeps being stopped. Not blocked — a
    // blocked run is a `permission` item and a person is already being asked —
    // but *refused*, over and over, by a rule that answers without asking. The
    // agent carries on regardless, which is the whole problem: nothing else on
    // any surface distinguishes "the policy is protecting you" from "the policy
    // is wrong and this run is burning money finding out".
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

    // The one thing the status-line shim is installed for, and the one thing it
    // did not do: the percentage arrived, the reducer dropped it, and no item
    // was ever produced. A subscription window about to close is worth knowing
    // before the agent finds out mid-turn.
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

/// Derives the inbox entries for a piece of work.
///
/// Work produces items that no run can: a pull request going red hours after
/// the agent stopped is the clearest example of why Work is the durable unit
/// and the session is not.
///
/// `can_drive` says whether Devplane still holds a session it could prompt —
/// not whether a run row looks alive. Only the first is a reason to offer
/// anything that talks to an agent.
/// **A specification an in-flight work names, carrying lines the project's own
/// words mark unresolved.**
///
/// One item per project, naming the count — never one per marker. Forty
/// clarification lines in one folder is one fact about one folder, and the
/// surface this product is sold on is the one that must stay readable.
///
/// Composed here so the sentence has one home, and called from the daemon
/// because reading a specification touches a disk.
pub fn plan_question_item(
    project: &str,
    project_id: &crate::core::ProjectId,
    plans: &[(String, crate::core::spec::Plan)],
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
        // Stable per project, so a marker that stays unanswered is one open
        // item rather than one per poll — and so it clears by disappearing
        // when the line goes, with nobody dismissing it.
        id: AttentionId::from(format!("plan-question:{}", project_id.as_str())),
        kind: AttentionKind::PlanQuestion,
        level: AttentionKind::PlanQuestion.default_level(),
        run_id: None,
        project_id: Some(project_id.clone()),
        title: match total {
            1 => format!("{project}: one question in {where_} nobody has answered"),
            n => format!("{project}: {n} questions in {where_} nobody has answered"),
        },
        // The line, in the project's own words. Rendered as untrusted text like
        // every other detail here: it came out of a file in a repository.
        detail: first,
        answer_in: Some(
            "These are lines in committed files. Answer one by editing the \
             specification it is in."
                .into(),
        ),
        ask: None,
        options: Vec::new(),
        actions: Vec::new(),
        form: None,
        request_id: None,
        url: None,
        launch: None,
        work_id: None,
        offer: None,
        no_offer: None,
        since: jiff::Timestamp::now(),
    })
}

pub fn items_for_work(
    work: &crate::core::work::Work,
    can_drive: bool,
    can_resume: bool,
) -> Vec<AttentionItem> {
    items_for_work_in(work, can_drive, can_resume, None)
}

/// The same, told the repository slug so it can build a launch link.
///
/// A slug rather than a path, because the link is the sort of thing that ends
/// up in a notification read on another machine, and `repo=` resolves to
/// whichever clone the reader actually has.
pub fn items_for_work_in(
    work: &crate::core::work::Work,
    can_drive: bool,
    can_resume: bool,
    repo: Option<&str>,
) -> Vec<AttentionItem> {
    let mut out = Vec::new();
    let url = work.pull_request.as_ref().map(|p| p.url.clone());
    let launch_for = |kind: &AttentionKind| -> Option<String> {
        let repo = repo?;
        // Only the kinds that name work somebody is about to do anyway. A
        // launch link on an item that is merely informational is one more
        // thing to read.
        let prompt = match kind {
            AttentionKind::CiRed => format!(
                "The checks on pull request #{} are failing. Find out why and fix it.",
                work.pull_request.as_ref()?.number
            ),
            AttentionKind::ChangesRequested => format!(
                "A reviewer asked for changes on pull request #{}. Read the comments and address them.",
                work.pull_request.as_ref()?.number
            ),
            AttentionKind::GateFailed => format!(
                "The project's checks did not pass for \"{}\" and the feedback budget is spent. \
                 Look at what is failing and fix the cause rather than the check.",
                work.title
            ),
            _ => return None,
        };
        crate::core::deeplink::open_repo(repo, &prompt)
    };
    let mk = |kind: AttentionKind, title: String, detail: Option<String>, actions: Vec<Action>| {
        // Per kind, for the reason on `Snoozed`: dismissing a red pull request
        // must not also swallow the gate failure that follows it.
        if work.snoozed.hides(&kind) {
            return None;
        }
        Some(AttentionItem {
            launch: launch_for(&kind),
            url: actions
                .contains(&Action::OpenPr)
                .then(|| url.clone())
                .flatten(),
            id: AttentionId::new(format!("{}:{}", work.id.as_str(), kind.as_str())),
            level: kind.default_level(),
            kind,
            // Genuinely absent once every run on this work has ended, which is
            // the ordinary case for a pull request that goes red the next day.
            run_id: work.current_run().cloned(),
            project_id: Some(work.project_id.clone()),
            title,
            detail,
            ask: None,
            answer_in: None,
            options: Vec::new(),
            actions,
            form: None,
            request_id: None,
            work_id: Some(work.id.clone()),
            // A work item is not about a tool call, so no rule answers it.
            offer: None,
            no_offer: None,
            since: work.updated_at,
        })
    };

    // Work that was mid-flight when the daemon stopped. The Work came back from
    // the store; the agent process did not, and nothing will ever move it on
    // its own — so it has to be said rather than left looking busy for ever.
    if !can_drive
        && matches!(
            work.phase,
            crate::core::work::Phase::Implement | crate::core::work::Phase::Verify
        )
    {
        // The one thing that actually rescues it, where the agent kept the
        // conversation. Without `can_resume` this item could only describe the
        // problem.
        let mut actions = vec![Action::Open, Action::Snooze];
        if can_resume {
            actions.insert(0, Action::Resume);
        }
        out.extend(mk(
            AttentionKind::Interrupted,
            format!("Interrupted: {}", work.title),
            Some(match can_resume {
                true => "The agent is gone and this was still in progress. Its branch and \
                         worktree are untouched, and its conversation can be picked up \
                         where it stopped."
                    .into(),
                false => "The agent is gone and this was still in progress. Its branch and \
                          worktree are untouched, but the conversation cannot be continued \
                          — start the work again when you want it finished."
                    .to_string(),
            }),
            actions,
        ));
    }

    // The end of the verified-done loop: the project's checks did not pass and
    // the budget for arguing about it is spent, so somebody is asked.
    //
    // Expensive and stopped is a different question from wrong and stopped: one
    // asks whether to spend more, the other whether the code is right, so they
    // are different kinds.
    // Why the work stopped is read, never inferred. Four unrelated reasons
    // reach `Phase::Failed`, they want four different answers from a person,
    // and guessing between them by reading the last gate report announced a
    // gate that had passed as the thing that failed.
    match work.stopped.as_ref() {
        None => {}

        Some(crate::core::work::Stopped::OverBudget { bound, .. }) => {
            out.extend(mk(
                AttentionKind::CostSpike,
                format!("{}: {}", bound, work.title),
                Some(
                    "This reached one of the bounds `[budget]` sets. Raise it in \
                     devplane.toml if the work is worth more, or pick it up yourself."
                        .into(),
                ),
                vec![Action::Open, Action::Snooze],
            ));
        }

        Some(crate::core::work::Stopped::GateFailed { gate }) => {
            let report = work.last_gate();
            let detail = match report {
                // The failing lines, not the log: the same summary the agent
                // was handed, so the person and the agent saw the same thing.
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
            // Offering "one more round" only when there is an agent left to
            // hand it to. A button that cannot do what it says is worse than
            // no button.
            let mut actions = vec![Action::Open, Action::Snooze];
            if work.retryable(can_drive) {
                actions.insert(0, Action::Retry);
            }
            out.extend(mk(
                AttentionKind::GateFailed,
                format!("{gate}: {}", work.title),
                Some(detail),
                actions,
            ));
        }

        Some(crate::core::work::Stopped::ReviewExhausted {
            step,
            back_to,
            findings,
        }) => {
            let mut actions = vec![Action::Open, Action::Snooze];
            if work.retryable(can_drive) {
                actions.insert(0, Action::Retry);
            }
            out.extend(mk(
                AttentionKind::ReviewExhausted,
                format!("{step} kept finding things: {}", work.title),
                Some(format!(
                    "`{step}` sent the work back to `{back_to}` as many times as the \
                     pipeline allows and still found this:\n\n{findings}"
                )),
                actions,
            ));
        }

        Some(crate::core::work::Stopped::Broken { detail }) => {
            out.extend(mk(
                AttentionKind::PipelineBroken,
                format!("the chain stopped: {}", work.title),
                Some(format!(
                    "{detail}\n\nThis is a problem with the pipeline rather than with \
                     the code, so there is nothing to hand back to an agent. \
                     `devplane check` reads the file the same way this did."
                )),
                vec![Action::Open, Action::Snooze],
            ));
        }
    }

    // A pipeline that has reached a human step. This is the one inbox item
    // that means everything went right.
    if work.phase == crate::core::work::Phase::Human {
        let step = work
            .pipeline
            .as_ref()
            .and_then(|p| p.role())
            .unwrap_or("a decision")
            .to_string();
        out.extend(mk(
            AttentionKind::HumanStep,
            format!("{step}: {}", work.title),
            work.pipeline.as_ref().map(|p| p.stepper()),
            vec![Action::Approve, Action::Open, Action::Snooze],
        ));
    }

    // Somebody else is editing the same files. Raised before the pull request
    // rather than by the merge, which is the only point at which it is cheap.
    if !work.overlaps.is_empty() {
        let files: Vec<&str> = work
            .overlaps
            .iter()
            .flat_map(|o| o.files.iter().map(String::as_str))
            .take(5)
            .collect();
        let others: Vec<&str> = work.overlaps.iter().map(|o| o.title.as_str()).collect();
        out.extend(mk(
            AttentionKind::Conflict,
            format!(
                "{} is editing the same files as {}",
                work.title,
                others.join(", ")
            ),
            Some(format!(
                "Both are in flight and both are locally correct; they cannot both land \
                 unchanged.\n\n{}",
                files.join("\n")
            )),
            vec![Action::Open, Action::Snooze],
        ));
    }

    let Some(pr) = &work.pull_request else {
        return out;
    };

    out.extend(
        match pr.status.as_str() {
            "failing" => vec![mk(
                AttentionKind::CiRed,
                format!("#{} is red: {}", pr.number, work.title),
                Some(if pr.failing_checks.is_empty() {
                    "a check failed".to_string()
                } else {
                    pr.failing_checks.join(", ")
                }),
                vec![Action::OpenPr, Action::Snooze],
            )],
            // `ready_to_merge` is approved *and* green, so nobody is being asked
            // for anything; it does not belong in a queue of decisions.
            "ready_for_review" => vec![mk(
                AttentionKind::PrReady,
                format!("#{} is ready: {}", pr.number, work.title),
                None,
                vec![Action::OpenPr, Action::Snooze],
            )],
            "changes_requested" => vec![mk(
                AttentionKind::ChangesRequested,
                format!("#{} has review comments: {}", pr.number, work.title),
                Some(
                    "A person asked for changes. Read them before asking an agent to act on them."
                        .into(),
                ),
                vec![Action::OpenPr, Action::Snooze],
            )],
            _ => Vec::new(),
        }
        .into_iter()
        .flatten(),
    );
    out
}

/// What a blocked run offers: the answers the agent will actually take where
/// Devplane can send one, and otherwise the honest ways to reach the session.
///
/// `Focus` raises the editor window that owns a directory and `Attach` hands
/// the terminal to `claude --resume`. Neither means anything for a run
/// Devplane started over the protocol: it has no window, and its session id
/// belongs to an agent that may not be Claude Code at all. Offering them there
/// was a button that lies, which is the one failure a control plane cannot
/// afford.
///
/// The shape of the answer comes from the agent, not from us:
///
/// * **Options with ids** — the agent named what it will accept, so `Choose`
///   leads and a surface renders one control per option. For a *permission*
///   those options are `allow_once`/`reject_once` and friends, so `Allow` and
///   `Deny` stay beside `Choose` as the one-key shorthand a person wants at
///   3 a.m.; for a *question* they are arbitrary answers and a yes/no would be
///   an invention, so there is none.
/// * **No options** — a question with free-text expected, which is `Reply`.
/// * **Nothing answerable** — an observed session, whose dialog belongs to its
///   own window.
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
        // A permission is a grant or a refusal however many ways the agent
        // spells it, so the shorthand is always meaningful.
        WaitingFor::Permission => out.extend([Action::Allow, Action::Deny]),
        // A question is whatever the agent asked. If it offered no options,
        // the answer is prose.
        WaitingFor::Question if !choosable => out.push(Action::Reply),
        _ => {}
    }
    out.push(Action::Open);
    out
}

/// The ways to reach this run's session, ahead of whatever else is offered.
///
/// Empty for a driven run, which has neither a window nor a `claude --resume`
/// to hand a terminal to.
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

/// How many rows a person reads before they start scanning.
///
/// **Below this nothing is folded at all**, which is the property that protects
/// every ordinary day: an inbox small enough to read renders exactly as it did
/// before this existed.
///
/// The number is a judgement and is written down as one. What it is not is a
/// cap — truncating a ranked list hides its tail, which is the failure the
/// whole feature is arranged against. Oversight modelled as a finite attention
/// budget is an **inverted U**: at a reviewer capacity of 50, escalating 72 %
/// of actions lets 22 % of danger through and escalating 100 % lets **39 %**
/// through. A list that grows without bound stops being read exactly when it
/// matters.
pub const READABLE: usize = 12;

/// Kinds whose members are interchangeable to a person.
///
/// **Enumerated, never inferred.** A kind that is not in this list is listed in
/// full, so a kind added later is unfoldable until somebody decides otherwise —
/// which is the safe direction: the cost of listing something foldable is a
/// longer list, and the cost of folding something unfoldable is a decision
/// nobody was shown.
///
/// The test is *would this person act the same way on any one of these?* Five
/// issues assigned across four projects are a queue. Five questions are five
/// questions.
/// Every kind, so a sweep over them is a list rather than a hand-written one
/// that drifts.
///
/// **The guards read this**, and a kind added to the enum without being added
/// here fails `every_kind_is_in_all_kinds` — which is what makes
/// *every kind has been decided about* a real check rather than a check over
/// whichever kinds somebody remembered.
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
    AttentionKind::IssueAssigned,
    AttentionKind::ReviewRequested,
    AttentionKind::Conflict,
    AttentionKind::Interrupted,
    AttentionKind::HumanStep,
    AttentionKind::ReviewExhausted,
    AttentionKind::PipelineBroken,
    AttentionKind::Refused,
    AttentionKind::GateDown,
    AttentionKind::ConfigBroken,
    AttentionKind::RecordIncomplete,
    AttentionKind::AgentLeaked,
];

pub const FOLDABLE: &[AttentionKind] = &[
    // Resource warnings. The row says a number is high; which session it is
    // about changes nothing a person does next.
    AttentionKind::ContextHigh,
    AttentionKind::RateLimit,
    AttentionKind::CostSpike,
    // Forge queues. Genuinely a list, and the one that grows without bound on a
    // machine with eight projects.
    AttentionKind::IssueAssigned,
    AttentionKind::ReviewRequested,
    AttentionKind::PrReady,
    AttentionKind::CiRed,
    // Session health. A stalled session and another stalled session are the
    // same errand.
    AttentionKind::Stalled,
    AttentionKind::Lost,
    AttentionKind::Interrupted,
    AttentionKind::RunFailed,
    AttentionKind::Conflict,
    AttentionKind::RecordIncomplete,
];

/// Kinds that may **never** be folded, whatever else is true.
///
/// **Each one needs an answer only this person can give.** Folding a question
/// into *3 questions in saas* is the product failing at the only thing it
/// claims: the whole argument is that a question nobody saw is the defect, and
/// a summary row is a question nobody saw with a number next to it.
///
/// Kept as its own list rather than as the complement of [`FOLDABLE`], because
/// the two say different things. A kind absent from `FOLDABLE` is one nobody
/// has considered; a kind here is one somebody decided about.
pub const NEVER_FOLDED: &[AttentionKind] = &[
    AttentionKind::Permission,
    AttentionKind::Question,
    AttentionKind::QuestionAbandoned,
    AttentionKind::HumanStep,
];

impl AttentionKind {
    /// Whether members of this kind are interchangeable enough to summarise.
    #[must_use]
    pub fn foldable(self) -> bool {
        !NEVER_FOLDED.contains(&self) && FOLDABLE.contains(&self)
    }
}

/// A group of items shown as one row, with everything needed to open them.
///
/// **Nothing is hidden: it is counted, and the ids are here.** A summary that
/// could not be expanded would be a cap wearing a feature's clothes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Not a generated wire type: `AttentionItem` is not one either, so exporting
// its summary would export half a shape. The board reads these as plain JSON,
// exactly as it reads the items they stand for.
pub struct Summary {
    pub kind: AttentionKind,
    /// `None` where the items belong to no project, or to several.
    pub project: Option<ProjectId>,
    pub count: usize,
    /// The highest level among the items behind this row, so a summary can
    /// never be quieter than its loudest member.
    pub level: Level,
    /// Every item this row stands for. A reader can open them; a test can
    /// prove nothing was lost.
    pub ids: Vec<AttentionId>,
}

/// Folds a ranked inbox into what a person can read, plus what was summarised.
///
/// **Never drops.** `rendered + summarised == raised`, over every input — which
/// is the assertion that makes this safe to turn on, because the failure mode
/// of every other approach to a long list is that something stops being
/// reachable.
///
/// Below [`READABLE`] nothing is folded and the output is the input.
#[must_use]
pub fn fold(items: Vec<AttentionItem>) -> (Vec<AttentionItem>, Vec<Summary>) {
    if items.len() <= READABLE {
        return (items, Vec::new());
    }

    // Group candidates by kind and project. A group of one is not a summary:
    // *1 issue assigned in saas* is longer than the row it replaces.
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
            // The loudest member decides, so folding cannot quieten anything.
            level: g
                .iter()
                .map(|&i| items[i].level)
                .max()
                .unwrap_or(Level::Normal),
            ids: g.iter().map(|&i| items[i].id.clone()).collect(),
        })
        .collect();

    let rendered = items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !folded.contains(i))
        .map(|(_, it)| it)
        .collect();

    (rendered, summaries)
}

// ---------------------------------------------------------------------------
// Inhibition: a symptom counted on its cause
// ---------------------------------------------------------------------------

/// How far a cause reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Only items in the same project. A configuration that will not parse
    /// breaks that repository and says nothing about any other.
    Project,
    /// Every item on the machine. The permission hook being down is not a fact
    /// about one repository.
    Machine,
}

/// A named cause, a kind it explains, and why.
///
/// **Enumerated, never inferred.** Deriving this from project or time
/// proximity would be a correlation engine — two things went wrong in one
/// repository within a minute is not evidence that one caused the other, and a
/// surface that suppressed a real problem on that reasoning would be hiding
/// exactly the row somebody needed.
#[derive(Debug, Clone, Copy)]
pub struct Cause {
    pub cause: AttentionKind,
    pub consequence: AttentionKind,
    pub reach: Reach,
    /// The sentence shown on the cause's row, so a person can see what the
    /// count is *of* without opening it.
    pub because: &'static str,
}

/// The pairs that are certain. Three, and each one is mechanical rather than
/// probable.
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
// Not a generated wire type: `AttentionItem` is not one either, so exporting
// its summary would export half a shape. The board reads these as plain JSON,
// exactly as it reads the items they stand for.
pub struct Inhibited {
    /// The item that explains them, so a surface can attach the count to it.
    pub cause: AttentionId,
    pub count: usize,
    pub because: String,
    pub ids: Vec<AttentionId>,
}

/// What a narrowing left out.
///
/// **Counted rather than implied.** A list that silently shows a subset of what
/// needs you is the one failure this surface cannot take: the product is sold
/// on *one page for everything*, and a page that quietly became a filter has
/// broken that promise without saying so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Narrowed {
    /// How many items the narrowing removed.
    pub count: usize,
    /// The projects those items belonged to, named, so the sentence can be
    /// *4 elsewhere, in saas and payments-api* rather than a bare number.
    pub projects: Vec<String>,
    /// Whether a project was asked for and matched nothing — which is a
    /// different fact from *that project has nothing waiting*, and the two read
    /// identically unless something says so.
    pub no_such_project: bool,
}

/// How to narrow the inbox.
///
/// **A view, never a preference.** Nothing is remembered between runs: a filter
/// that persists is a filter somebody forgets they set, and the next morning
/// they are reading a subset of what needs them and do not know it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Narrowing<'a> {
    /// Matched the way `devplane ls --project` matches: case-insensitive
    /// substring against the project name. **The same rule, deliberately** —
    /// two commands on one machine that disagree about what `--project pay`
    /// means is worse than either rule on its own.
    pub project: Option<&'a str>,
    /// What has an answer path, as `ls --needs-you` has it.
    ///
    /// **The flag's name promises one of two things and they are different.**
    /// *A person is required* would include a red gate and a leaked agent;
    /// *has an answer path* is what `ls` means by it and what somebody typing
    /// it at 9am wants — the rows they can do something about **from here**.
    /// This is the second.
    pub needs_you: bool,
}

impl Narrowing<'_> {
    pub fn is_none(&self) -> bool {
        self.project.is_none() && !self.needs_you
    }
}

/// Narrows a ranked list, and says what it left out.
///
/// **Pure, and beside `fold` for the reason `fold` is here**: the two surfaces
/// must narrow identically, and the only way that stays true is one
/// computation. A board and a terminal each implementing *contains,
/// case-insensitive* agree until one of them is changed.
///
/// **Applied before folding**, so the counts do not overlap: an item this
/// removes is never also an item the fold hid, and `listed + narrowed_away +
/// folded == raised` holds over any input.
pub fn narrow(
    items: Vec<AttentionItem>,
    by: &Narrowing<'_>,
    project_name: &dyn Fn(&ProjectId) -> Option<String>,
    known_projects: &[String],
) -> (Vec<AttentionItem>, Option<Narrowed>) {
    if by.is_none() {
        return (items, None);
    }

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
            // **An item belonging to no project is not in any project.** A
            // machine-wide row — the gate being down, the record incomplete —
            // is about every repository at once, and showing it under one
            // project's name would be claiming something untrue about it.
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

    (
        kept,
        Some(Narrowed {
            count: dropped.len(),
            projects,
            no_such_project,
        }),
    )
}

/// Moves named consequences onto the rows that explain them.
///
/// **Never drops; moves and counts.** A suppressed item returns the moment its
/// cause resolves, because nothing is stored — the cause is either in this
/// render's list or it is not.
///
/// Two refusals, and both are the exemption rather than the mechanism:
/// a kind in [`NEVER_FOLDED`] is listed however well explained it is, and a
/// symptom claimed by two causes is counted **once**, under the louder one.
#[must_use]
pub fn inhibit(items: Vec<AttentionItem>) -> (Vec<AttentionItem>, Vec<Inhibited>) {
    // Causes present in *this* list. A cause that has resolved is simply not
    // here, which is what makes a resolved cause free rather than a subscription.
    let causes: Vec<&AttentionItem> = items
        .iter()
        .filter(|i| CAUSES.iter().any(|c| c.cause == i.kind))
        .collect();
    if causes.is_empty() {
        return (items, Vec::new());
    }

    // For each item, the best cause that explains it: louder first, so a
    // symptom two causes claim is counted once and under the higher level.
    let mut claim: std::collections::BTreeMap<usize, (AttentionId, &'static str, Level)> =
        std::collections::BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        // **The exemption wins over every cause.** An abandoned question in a
        // project whose configuration is broken is still a question nobody
        // answered, and explaining it away is the product failing at its
        // subject.
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

    let kept = items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !claim.contains_key(i))
        .map(|(_, it)| it)
        .collect();

    (kept, suppressed)
}

/// Builds and ranks the whole inbox.
pub fn rank(mut items: Vec<AttentionItem>) -> Vec<AttentionItem> {
    items.sort_by_key(|i| std::cmp::Reverse(i.rank()));
    decorrelate(items)
}

/// Spreads runs of items that came from the same place, **within a level**.
///
/// Twelve rows from one project read as one problem, and the eye stops at the
/// third. The thirteenth row — the one from somewhere else — is the one worth
/// seeing, and sorting by level then age buries it behind its own neighbours.
///
/// **Level always outranks this**, which is the property that keeps it safe: a
/// critical row never moves below a normal one to break up a cluster. Inside a
/// level the order is age, and this only ever reorders among items that are
/// already interchangeable by both.
///
/// Correlated means *the same project*, which is what this product can observe
/// — a run belongs to a project and a project is the file cluster. It is a
/// fact on the item, never a similarity score: ordering by what a model thinks
/// is related is the one change that would make this surface unexplainable.
fn decorrelate(items: Vec<AttentionItem>) -> Vec<AttentionItem> {
    let mut out: Vec<AttentionItem> = Vec::with_capacity(items.len());
    // One band per level, preserving the age order inside it.
    let mut band: Vec<AttentionItem> = Vec::new();
    let mut level: Option<Level> = None;

    let flush = |band: &mut Vec<AttentionItem>, out: &mut Vec<AttentionItem>| {
        // Greedy: take the oldest item whose project differs from the one just
        // emitted; if every remaining item shares it, take the oldest. That
        // second clause is why this cannot starve — there is always a move.
        let mut last: Option<Option<ProjectId>> = None;
        while !band.is_empty() {
            let pick = band
                .iter()
                .position(|i| last.as_ref() != Some(&i.project_id))
                .unwrap_or(0);
            let it = band.remove(pick);
            last = Some(it.project_id.clone());
            out.push(it);
        }
    };

    for it in items {
        if level != Some(it.level) {
            flush(&mut band, &mut out);
            level = Some(it.level);
        }
        band.push(it);
    }
    flush(&mut band, &mut out);
    out
}

fn summarise_input(v: &serde_json::Value) -> String {
    if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
        return cmd.to_string();
    }
    if let Some(p) = v.get("file_path").and_then(|c| c.as_str()) {
        return p.to_string();
    }
    // `to_string` on arbitrary JSON can be megabytes and is full of non-ASCII:
    // a byte slice here panicked the receiver that was describing the call.
    crate::core::text::clip(&v.to_string(), 200)
}

#[cfg(test)]
mod tests {
    /// One item of a kind, in a project, for the folding tests.
    fn it_of(kind: AttentionKind, project: Option<&str>, n: usize) -> AttentionItem {
        AttentionItem {
            id: AttentionId::from(format!("{}-{n}", kind.as_str())),
            kind,
            level: Level::Normal,
            run_id: None,
            project_id: project.map(ProjectId::new),
            title: format!("{} {n}", kind.as_str()),
            detail: None,
            answer_in: None,
            options: Vec::new(),
            actions: Vec::new(),
            request_id: None,
            ask: None,
            form: None,
            url: None,
            launch: None,
            work_id: None,
            offer: None,
            no_offer: None,
            since: jiff::Timestamp::now(),
        }
    }

    /// **The counting property: nothing vanishes and nothing is counted twice.**
    ///
    /// `listed + narrowed away + folded == raised`, over a set spanning every
    /// kind. This is the whole safety argument for narrowing a list the product
    /// is sold on being complete: a page that quietly became a filter has
    /// broken its promise without saying so, and the only defence is that every
    /// item is in exactly one of three places and the two that are not on
    /// screen are counted out loud.
    #[test]
    fn narrowing_and_folding_account_for_every_item_exactly_once() {
        let names = |id: &ProjectId| Some(id.as_str().to_string());
        let known = vec!["alpha".to_string(), "beta".to_string()];

        // One item per kind, alternating between two projects, so the set spans
        // every kind and both sides of the narrowing.
        let raised: Vec<AttentionItem> = ALL_KINDS
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let mut it = config_broken_item(Path::new("/tmp/x"), "why");
                it.id = AttentionId::from(format!("i{i}"));
                it.kind = *k;
                it.level = k.default_level();
                it.project_id = Some(ProjectId::from(if i % 2 == 0 { "alpha" } else { "beta" }));
                if i % 3 == 0 {
                    it.actions = vec![Action::Reply];
                }
                it
            })
            .collect();
        let total = raised.len();

        for by in [
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
            let (kept, narrowed) = narrow(raised.clone(), &by, &names, &known);
            let away = narrowed.as_ref().map_or(0, |n| n.count);
            let (listed, summaries) = fold(kept);
            let folded: usize = summaries.iter().map(|s| s.ids.len()).sum();
            assert_eq!(
                listed.len() + folded + away,
                total,
                "narrowing {by:?} lost or double-counted an item"
            );

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
        }
    }

    /// A narrowing that matches no project is a different fact from a project
    /// with nothing waiting, and the two read identically unless something says
    /// so.
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

    /// `--needs-you` is *has an answer path*, not *a person is required*.
    ///
    /// A red gate needs a person and cannot be answered from a list; a question
    /// with a reply can. Derived from what is offered, so it cannot disagree
    /// with the controls the row actually shows.
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

    /// **A narrowing is a view, and no narrowing is the identity.**
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

    /// **Exhaustive by compilation.** Adding a variant to `AttentionKind`
    /// makes this match non-exhaustive, and the compiler names the variant
    /// that is missing from [`ALL_KINDS`].
    ///
    /// A hand-written list checked against another hand-written list proves
    /// only that somebody wrote the same thing twice; this cannot pass with a
    /// variant missing.
    fn is_listed(k: AttentionKind) -> bool {
        use AttentionKind::*;
        match k {
            Permission | Question | QuestionAbandoned | PlanQuestion | RunFailed | Stalled
            | Lost | ContextHigh | RateLimit | CostSpike | GateFailed | CiRed
            | ChangesRequested | PrReady | IssueAssigned | ReviewRequested | Conflict
            | Interrupted | HumanStep | ReviewExhausted | PipelineBroken | Refused | GateDown
            | ConfigBroken | RecordIncomplete | AgentLeaked => ALL_KINDS.contains(&k),
        }
    }

    /// The enum and the list cannot drift: a kind added to one and not the
    /// other makes every sweep below a sweep over the wrong set.
    #[test]
    fn every_kind_is_in_all_kinds() {
        for k in ALL_KINDS {
            assert!(is_listed(*k), "{} is not in ALL_KINDS", k.as_str());
        }
        let listed: std::collections::BTreeSet<&str> =
            ALL_KINDS.iter().map(|k| k.as_str()).collect();
        assert_eq!(listed.len(), ALL_KINDS.len(), "ALL_KINDS has a duplicate");
        // Round-trips through the serialisation, which is the enum's own
        // spelling — so a new variant with a new name is absent from the set
        // and this fails.
        for name in listed.iter() {
            let k: AttentionKind =
                serde_json::from_value(serde_json::Value::String((*name).to_string()))
                    .unwrap_or_else(|_| panic!("{name} is not a kind"));
            assert_eq!(k.as_str(), *name);
        }
    }

    /// **The arithmetic, as an assertion: nothing is dropped.**
    ///
    /// `rendered + summarised == raised`, over a set spanning every kind. This
    /// is the property that makes folding safe to turn on at all — the failure
    /// mode of every other approach to a long list is that something stops
    /// being reachable, and a cap on a ranked list hides exactly the tail the
    /// inverted-U result is about.
    #[test]
    fn folding_never_loses_an_item() {
        let mut raised = Vec::new();
        for (n, kind) in ALL_KINDS.iter().enumerate() {
            // Several of each, across two projects, so every foldable kind has
            // a group to fold and every unfoldable one has a reason not to.
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

        // Reachability, not just arithmetic: every id is either on screen or
        // behind a summary that names it.
        let mut reachable: std::collections::BTreeSet<AttentionId> =
            rendered.iter().map(|i| i.id.clone()).collect();
        for s in &summaries {
            reachable.extend(s.ids.iter().cloned());
        }
        assert_eq!(reachable, ids, "an item is counted but not reachable");
    }

    /// **The four kinds only this person can answer are never folded.**
    ///
    /// Folding a question into *3 questions in saas* is the product failing at
    /// the one thing it claims: a summary row is a question nobody saw with a
    /// number beside it.
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

    /// **Every kind is foldable on purpose or unfoldable on purpose.**
    ///
    /// A kind added later is unfoldable until somebody puts it in the list,
    /// which is the safe direction — the cost of listing something foldable is
    /// a longer list, and the cost of folding something unfoldable is a
    /// decision nobody was shown. This fails when a new kind appears so that
    /// the choice is made deliberately rather than by default.
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
        // The four exemptions are mandatory, so they are named rather than
        // counted: a list that merely has four entries can have the wrong four.
        for kind in [
            AttentionKind::Permission,
            AttentionKind::Question,
            AttentionKind::QuestionAbandoned,
            AttentionKind::HumanStep,
        ] {
            assert!(
                NEVER_FOLDED.contains(&kind),
                "{} lost its exemption",
                kind.as_str()
            );
        }
    }

    /// **A list short enough to read renders exactly as it did before this
    /// existed.** The regression that protects every ordinary day.
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

    /// A group of one is not a summary: *1 issue assigned in saas* is longer
    /// than the row it would replace.
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

    /// A summary is never quieter than its loudest member.
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

    /// **Inhibition never drops, and the exemption outranks every cause.**
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
            // **The exemption.** An abandoned question inside a broken-config
            // project is still a question nobody answered.
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

    /// **A symptom two causes claim is counted once, under the louder one.**
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

    /// **When a cause resolves, its consequences come back** — free, because
    /// nothing is stored: a resolved cause is simply not in the next list.
    #[test]
    fn a_consequence_returns_when_its_cause_is_gone() {
        let with_cause = vec![
            it_of(AttentionKind::ConfigBroken, Some("p1"), 0),
            it_of(AttentionKind::Refused, Some("p1"), 1),
        ];
        let (kept, suppressed) = inhibit(with_cause);
        assert_eq!(kept.len(), 1);
        assert_eq!(suppressed.iter().map(|s| s.count).sum::<usize>(), 1);

        // The same symptom, with the cause resolved.
        let without = vec![it_of(AttentionKind::Refused, Some("p1"), 1)];
        let (kept, suppressed) = inhibit(without);
        assert_eq!(kept.len(), 1, "the consequence did not come back");
        assert!(suppressed.is_empty());
    }

    /// Every pair names a cause and a consequence that exist, and no pair
    /// explains a kind that may never be folded — which would be a rule the
    /// exemption then has to override at run time.
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

    /// **Level always outranks decorrelation**, and a cluster is broken up
    /// only among items that are already interchangeable by level and age.
    #[test]
    fn spreading_a_cluster_never_moves_a_row_past_a_louder_one() {
        let mut raised = vec![
            it_of(AttentionKind::CiRed, Some("p1"), 0),
            it_of(AttentionKind::CiRed, Some("p1"), 1),
            it_of(AttentionKind::CiRed, Some("p1"), 2),
            it_of(AttentionKind::CiRed, Some("p2"), 3),
        ];
        // One critical, at the back of the input.
        raised[3].level = Level::Critical;

        let ranked = rank(raised);

        assert_eq!(
            ranked[0].level,
            Level::Critical,
            "a critical row was moved below a normal one to break up a cluster"
        );
        // Levels stay monotonically non-increasing: decorrelation reorders
        // inside a band and never across one.
        for w in ranked.windows(2) {
            assert!(w[0].level >= w[1].level, "the level order was broken");
        }
    }

    /// Where an alternative of the same level exists, two adjacent rows do not
    /// share a project.
    #[test]
    fn a_cluster_is_interleaved_with_what_else_is_waiting() {
        let raised = vec![
            it_of(AttentionKind::CiRed, Some("p1"), 0),
            it_of(AttentionKind::CiRed, Some("p1"), 1),
            it_of(AttentionKind::CiRed, Some("p1"), 2),
            it_of(AttentionKind::CiRed, Some("p2"), 3),
            it_of(AttentionKind::CiRed, Some("p3"), 4),
        ];
        let ranked = rank(raised);

        // p2 and p3 exist, so the first three rows cannot all be p1.
        let first_three: Vec<String> = ranked
            .iter()
            .take(3)
            .map(|i| {
                i.project_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            })
            .collect();
        assert!(
            first_three
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1,
            "three rows from one project in a row while others were waiting: {first_three:?}"
        );

        // Nothing is lost or duplicated.
        assert_eq!(ranked.len(), 5);
    }

    /// **It cannot starve.** Where every remaining row shares a project there
    /// is no alternative, and the oldest is taken — the list still drains in
    /// age order.
    #[test]
    fn one_project_alone_keeps_its_age_order() {
        let raised: Vec<AttentionItem> = (0..5)
            .map(|n| it_of(AttentionKind::CiRed, Some("p1"), n))
            .collect();
        let before: Vec<AttentionId> = raised.iter().map(|i| i.id.clone()).collect();
        let after: Vec<AttentionId> = rank(raised).iter().map(|i| i.id.clone()).collect();
        assert_eq!(
            before, after,
            "a single-project list was reordered for no reason"
        );
    }

    #[test]
    fn a_level_is_stored_as_serde_spells_it() {
        // The rule that exists because reconstructing a wire value from `Debug`
        // has already cost this project two silent bugs. Held against the
        // serialisation itself, not against a second hand-written list — a
        // second list is the thing that drifts.
        for level in [Level::Normal, Level::High, Level::Critical] {
            let serde = serde_json::to_value(level).unwrap();
            assert_eq!(
                serde.as_str(),
                Some(level.as_str()),
                "{level:?} is stored under a different name from the one it serialises to"
            );
        }
    }

    #[test]
    fn work_editing_the_same_files_as_other_work_says_so_before_the_merge() {
        // The failure isolated checkouts cannot prevent, and are in fact the
        // reason for: two branches, each locally correct, that cannot both
        // land. `max_parallel_runs` does not see it — it counts agents, and
        // the question is which files.
        let mut w = crate::core::Work::new(
            crate::core::ProjectId::new("p"),
            crate::core::WorkKind::Quick,
            "rename the auth module".into(),
            "…".into(),
        );
        w.phase = crate::core::Phase::Review;
        w.overlaps = vec![crate::core::Overlap {
            work_id: crate::core::WorkId::new("other"),
            title: "add oauth".into(),
            files: vec!["src/auth.rs".into()],
        }];
        let items = items_for_work(&w, false, false);
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
        assert!(item.title.contains("add oauth"), "it names the other work");

        // And no overlap is silence, not an empty warning.
        w.overlaps.clear();
        assert!(
            !items_for_work(&w, false, false)
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
        // The status-line shim exists for this number and nothing else read it:
        // the reducer dropped both windows, no item was ever produced, and the
        // threshold beside it decided nothing. A feature with a kind, a config
        // key and a documented trigger, and no code.
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

        // And below the threshold it says nothing.
        r.totals.rate_limit_percent = Some(40.0);
        assert!(!items(&r).iter().any(|i| i.kind == AttentionKind::RateLimit));
    }

    #[test]
    fn a_run_that_keeps_being_refused_says_so_and_names_the_rule() {
        // The only kind raised about a session that is neither blocked nor
        // failed. A refused agent carries on, so "the policy is protecting
        // you" and "the policy is wrong and this run is burning money finding
        // out" look identical on every other surface.
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        r.refusals = 6;
        r.last_refusal = Some(crate::core::run::Refusal {
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

        // The vendor's own auto mode is named in words rather than left as a
        // bare token nobody can look up.
        r.last_refusal = Some(crate::core::run::Refusal {
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

        // Below the threshold, and on a run that has finished, it says nothing:
        // one refused `git push` is the policy working, and a run nobody can
        // affect any more is history rather than a decision.
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
        // Without hooks a roster row emits no activity, so "no activity for
        // 77 min" was raised about a session that was busy the whole time.
        // Silence is a signal only from something that speaks.
        let mut r = run(RunMode::Observed);
        r.state = RunState::Working;
        // The roster gave it a status — so it is real, and on the board — but
        // no hook, telemetry record or turn has ever arrived for it.
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
        // The sweeper already resolved the project's own `stall_timeout` before
        // emitting the event; the inbox derived its item from the machine-wide
        // number instead. A repository whose suite takes forty minutes was told
        // every ten minutes that it had stalled, and the event log disagreed.
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
    fn a_driven_run_is_never_offered_a_window_to_raise() {
        // `Focus` raises the editor window that owns a directory and `Attach`
        // runs `claude --resume`. A run Devplane started over the protocol has
        // no window, and its session id may not belong to Claude Code at all —
        // so both were buttons that could not do what they said.
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

        // An observed one still gets both: its window is where the answer is.
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
        // The bug this pins: a question carrying four labelled options was
        // offered as `allow`/`deny`. The API could already answer it by option
        // id and two documents promised `1`–`9` would pick one — so the one
        // question Devplane could genuinely answer was reduced to a yes/no
        // nobody asked, and the options rode along as decoration.
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
        // A permission is a grant or a refusal however many ways the agent
        // spells it, so `allow`/`deny` stay — unlike a question, where a binary
        // would be Devplane inventing an answer.
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
        // The failure this replaces: one timestamp on the run. Dismissing a
        // context-window warning also swallowed the permission request that
        // came five minutes later — the single item the product exists to
        // deliver, hidden by a gesture about something else, silently.
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

        // Now it blocks on a permission. That was never dismissed.
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
        // waiting, so every value it carries means a person is being waited
        // on. Three documented ones — `sandbox request`, `worker request`,
        // `dialog open` — mapped to `Other`, `needs_human` listed only the two
        // it recognised, and all three reached the board and never the inbox.
        // Silently, for as long as they have existed.
        for what in ["sandbox request", "worker request", "dialog open"] {
            let mut r = run(RunMode::Observed);
            r.state = RunState::Waiting(WaitingFor::Other(what.into()));
            assert!(r.state.needs_human(), "{what} is a person being waited on");

            let out = items(&r);
            let item = out
                .first()
                .unwrap_or_else(|| panic!("{what} raised nothing"));
            assert!(item.title.contains(what), "{}", item.title);
            // Never answerable from here: there is no protocol request behind
            // it, so it offers the honest ways to reach the session.
            assert!(!item.actions.contains(&Action::Allow), "{:?}", item.actions);
            assert!(item.actions.contains(&Action::Focus), "{:?}", item.actions);
        }

        // And `Idle` is still not a person being waited on.
        let mut idle = run(RunMode::Observed);
        idle.state = RunState::Waiting(WaitingFor::Idle);
        assert!(!idle.state.needs_human());
    }

    #[test]
    fn a_work_item_with_no_live_run_names_no_run() {
        // It used to carry `RunId::new("")`, which every surface rendered as a
        // run and offered actions against. A pull request going red the day
        // after the agent finished is the ordinary case, not an edge one.
        let mut w = crate::core::work::Work::new(
            crate::core::ids::ProjectId::new("p"),
            crate::core::work::WorkKind::Bug,
            "fix the flaky login test".into(),
            "…".into(),
        );
        w.phase = crate::core::work::Phase::Human;
        assert!(w.current_run().is_none());

        let item = &items_for_work(&w, false, false)[0];
        assert_eq!(item.run_id, None);
        assert_eq!(item.work_id.as_ref(), Some(&w.id));
    }
}

#[cfg(test)]
mod one_row_per_thing {
    use super::*;

    /// **One underlying thing produces one row — and no mechanism enforces it,
    /// because measuring first showed none was needed.**
    ///
    /// The worry was that a permission and the work it belongs to could both
    /// raise a row about one decision, and that a list which double-counts
    /// teaches people to distrust its count. Checking rather than building
    /// found the structure already prevents it:
    ///
    /// - A row's identity is its subject and its kind — `{run}:{kind}` for a
    ///   session, `{work}:{kind}` for a piece of work — so two namespaces that
    ///   cannot collide, and one kind per subject inside each.
    /// - No work-shaped kind means *a permission is waiting*. `Permission` is
    ///   raised against a run and nothing else describes the same decision.
    ///
    /// So this is the guard on that remaining true, rather than a
    /// de-duplication layer that would have to guess which row to keep.
    #[test]
    fn no_work_kind_describes_a_permission() {
        // The kinds a piece of work raises, all of which outlive its session.
        let work_shaped = [
            AttentionKind::GateFailed,
            AttentionKind::HumanStep,
            AttentionKind::ReviewExhausted,
            AttentionKind::PipelineBroken,
            AttentionKind::Conflict,
            AttentionKind::PrReady,
            AttentionKind::CiRed,
            AttentionKind::ChangesRequested,
            AttentionKind::ReviewRequested,
        ];
        for k in work_shaped {
            assert_ne!(
                k.as_str(),
                AttentionKind::Permission.as_str(),
                "a work-shaped kind now describes a permission, so one decision \
                 can raise two rows"
            );
        }
    }

    /// **A session's row and a piece of work's row cannot be confused**, because
    /// the ids they are built from carry their kind.
    ///
    /// A row's id is `{subject}:{kind}`, and the subject is a run id or a work
    /// id. Those are minted with distinct prefixes — `w-` for work, and a
    /// provider tag such as `acp-` for a driven run — so the two namespaces are
    /// disjoint by construction rather than by coincidence.
    ///
    /// The first draft of this test asserted the two ids were *equal* for the
    /// same stem and called that acceptable, which proved nothing: it is only
    /// true when somebody hands both types the same string, which nothing does.
    #[test]
    fn work_ids_and_run_ids_are_minted_into_different_namespaces() {
        let src = concat!(include_str!("work.rs"), include_str!("../driven.rs"),);
        assert!(
            src.contains(r#"WorkId::new(format!("w-{}""#),
            "work ids stopped carrying a prefix, so a work row's id could \
             collide with a session row's"
        );
        assert!(
            src.contains(r#"RunId::new(format!("acp-{}""#),
            "driven run ids stopped carrying a prefix"
        );
    }
    // ── The machine's own rows, and the seat's own number ────────────────
    //
    // These cover the pieces reconstructed on 2026-09-20 after `git checkout`
    // discarded this file's uncommitted work — the destructive command this
    // repository's own standing rules say never to use on a scratch edit. The
    // behaviour is re-derived from the callers and from the notes that describe
    // it, so it is pinned here rather than trusted.

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
        // Singular and plural, because a row that says "1 events" is a row
        // somebody stops reading carefully.
        assert!(record_incomplete_item(1, 0, "x").title.contains("1 event "));
    }

    /// The machine rows offer no action, and the reason is the same for all
    /// three: there is no button for disk space, for a TOML file, or for
    /// deciding to kill somebody else's model mid-write.
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

    /// A leaked agent is handed over with the command that ends it — and is
    /// never ended automatically, because it may be part-way through writing.
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
        // Stable per process: one leaked agent is one row, not one per sweep.
        assert_eq!(item.id, agent_leaked_item(4242, "other", None).id);
        assert_ne!(item.id, agent_leaked_item(4243, "claude --acp", None).id);
    }

    /// The item that makes the durability claim true: an ask whose run is gone
    /// is still answerable, and says so.
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
        // One row per ask, however many times the daemon restarts under it.
        assert_eq!(item.id, stranded_ask_item(&ask).id);
    }

    /// *None of nothing* is a quiet week and *none of four hundred* is the
    /// finding. One number cannot say both, so a quiet week says nothing.
    #[test]
    fn the_oversight_ratio_is_absent_rather_than_nought_when_nothing_happened() {
        let quiet = Oversight {
            unattended: 0,
            asked: 0,
            answered: 0,
        };
        assert_eq!(quiet.total(), 0);
        assert_eq!(quiet.reviewed(), None);
        assert_eq!(quiet.sentence(), None);
    }

    #[test]
    fn the_oversight_sentence_is_a_count_and_never_a_verdict() {
        let o = Oversight {
            unattended: 47,
            asked: 2,
            answered: 1,
        };
        assert_eq!(o.total(), 49);
        assert_eq!(o.reviewed(), Some(1.0 / 49.0));
        let said = o.sentence().expect("a sentence");
        assert!(said.contains("1 of the 49"), "{said}");
        // No threshold, no grade, no judgement about the person: the published
        // criterion needs inputs this product cannot observe, so the surface
        // prints the citation separately.
        for weasel in ["should", "too few", "not enough", "vacuous", "risk"] {
            assert!(!said.to_lowercase().contains(weasel), "{said}");
        }
    }

    #[test]
    fn the_two_machine_kinds_spell_themselves_on_the_wire() {
        for (kind, spelt) in [
            (AttentionKind::RecordIncomplete, "record_incomplete"),
            (AttentionKind::AgentLeaked, "agent_leaked"),
        ] {
            assert_eq!(kind.as_str(), spelt);
            assert_eq!(kind.default_level(), Level::Critical);
        }
    }
}

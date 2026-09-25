//! Change — the durable unit.
//!
//! A session is a process that crashes and resumes; a change is what survives:
//! the branch, the worktree, the gates that decide whether it is finished, and
//! the runs that have tried. It has no stored state — where it has got to is
//! computed from the record and the tree, so nothing can write *verified*.

use crate::core::ids::{ChangeId, ProjectId, RunId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where a change has got to, computed by [`Change::state`]; never stored.
/// Every surface reads the word and glyph from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum ChangeState {
    /// A record exists. No branch, no worktree, nothing has run.
    #[serde(rename = "drafted")]
    Drafted,
    /// A branch and a worktree exist. Nothing has run.
    #[serde(rename = "isolated")]
    Isolated,
    /// At least one run has worked on it, and nothing below applies.
    #[serde(rename = "in flight")]
    InFlight,
    /// Every declared gate exited zero against the tree as it stands.
    #[serde(rename = "verified")]
    Verified,
    /// A pull request exists.
    #[serde(rename = "offered")]
    Offered,
    /// The worktree is gone and the record is kept.
    #[serde(rename = "archived")]
    Archived,
}

impl ChangeState {
    /// The word, and the spelling on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeState::Drafted => "drafted",
            ChangeState::Isolated => "isolated",
            ChangeState::InFlight => "in flight",
            ChangeState::Verified => "verified",
            ChangeState::Offered => "offered",
            ChangeState::Archived => "archived",
        }
    }

    /// The glyph beside the word. Never on its own.
    pub fn glyph(self) -> &'static str {
        match self {
            ChangeState::Drafted => "·",
            ChangeState::Isolated => "⎇",
            ChangeState::InFlight => "▶",
            ChangeState::Verified => "✓",
            ChangeState::Offered => "↗",
            ChangeState::Archived => "▣",
        }
    }
}

/// What a change is waiting on right now, shown beside the state and never as
/// one. `None` while an agent is working or nothing is happening.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum Waiting {
    /// The machine's part is done; it is a person's turn to look.
    Person,
    /// The gates are running.
    Gates,
    /// Failures were handed back to the agent and it is on this round.
    Feedback { round: u32 },
    /// Workspace setup is running in a fresh tree, before the agent starts.
    Setup { command: String, since: Timestamp },
}

impl Waiting {
    /// One phrase, for the word beside the state.
    pub fn says(&self) -> String {
        match self {
            Waiting::Person => "needs you".into(),
            Waiting::Gates => "gates running".into(),
            Waiting::Feedback { round } => format!("feedback round {round}"),
            Waiting::Setup { command, since } => {
                let elapsed = Timestamp::now().duration_since(*since).unsigned_abs();
                format!(
                    "installing · `{command}` · {}",
                    elapsed_words(elapsed.as_secs())
                )
            }
        }
    }
}

/// `12s`, `1m 12s`, `1h 2m`; seconds drop out past an hour.
fn elapsed_words(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m {}s", secs / 60, secs % 60),
        _ => format!("{}h {}m", secs / 3600, (secs % 3600) / 60),
    }
}

/// The gate whose latest report decides whether a change is verified. Named
/// gates are evidence a person asked for and never make a change verified.
pub const CHECK: &str = "check";

/// What the gates say about a change right now — the one derivation of
/// `verified` ([`Change::verdict`]). Only the latest [`CHECK`] report counts.
/// A stale pass is neither pass nor failure; both digests are carried so a
/// reader can see they differ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "standing", rename_all = "snake_case")]
pub enum Standing {
    /// The latest `check` exited zero and the working-tree digest is unchanged.
    Verified,
    /// `check` passed, and the tree is not provably the tree it passed on.
    Stale {
        /// The digest `check` ran against; `None` when none was recorded.
        then: Option<String>,
        /// The working-tree digest now; `None` when it could not be read.
        now: Option<String>,
    },
    /// The latest `check` did not pass.
    Failed { summary: String },
    /// Gates are declared and `check` has not run against this change.
    NotRun,
    /// The project never said what done means.
    NoGatesDeclared,
}

impl Standing {
    /// Shorthand for [`Change::verdict`].
    pub fn of(change: &Change, declares_gates: bool, now: Option<&CommitStamp>) -> Self {
        change.verdict(declares_gates, now)
    }

    /// The word the state table pairs a glyph with.
    pub fn word(&self) -> &'static str {
        match self {
            Standing::Verified => "verified",
            Standing::Stale { .. } => "stale",
            Standing::Failed { .. } => "failed",
            Standing::NotRun => "not run",
            Standing::NoGatesDeclared => "no gates declared",
        }
    }

    /// The sentence beside the word.
    pub fn says(&self) -> String {
        match self {
            Standing::Verified => {
                "`check` exited zero against the working tree as it stands".into()
            }
            Standing::Stale { then, now } => {
                format!(
                    "gates passed, stale — {}",
                    stale_clause(then.as_deref(), now.as_deref())
                )
            }
            Standing::Failed { summary } => summary.clone(),
            Standing::NotRun => "`check` has not run against this change".into(),
            Standing::NoGatesDeclared => {
                "no gates declared — this project never said what done means".into()
            }
        }
    }
}

/// Why a passing `check` no longer describes the tree: which negation of
/// `still_current` happened.
fn stale_clause(then: Option<&str>, now: Option<&str>) -> String {
    let short = |d: &str| d.chars().take(7).collect::<String>();
    match (then, now) {
        (None, _) => "no tree digest was recorded when it ran (the tree moved while it ran, \
                      or could not be read), so nothing says what it passed against"
            .into(),
        (Some(t), None) => format!(
            "it ran against tree {} and the tree now could not be read",
            short(t)
        ),
        (Some(t), Some(n)) => format!(
            "the working tree changed after it ran (tree {} then, {} now)",
            short(t),
            short(n)
        ),
    }
}

/// Why a change stopped, recorded when it stops: a reason cannot be
/// re-derived from the last gate report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Stopped {
    /// The project's checks did not pass and the feedback budget is spent.
    GateFailed { gate: String },
    /// The change passed a bound its project set; `bound` names which one.
    OverBudget { spent_usd: f64, bound: String },
    /// The change itself could not continue (e.g. a missing gate or worktree).
    /// Nothing an agent can fix, so never handed back.
    Broken { detail: String },
}

impl Stopped {
    /// A one-line summary for a list.
    pub fn headline(&self) -> String {
        match self {
            Stopped::GateFailed { gate } => format!("{gate} failed"),
            Stopped::OverBudget { spent_usd, bound } => {
                if spent_usd > &0.0 {
                    format!("{bound} (${spent_usd:.2} spent)")
                } else {
                    bound.clone()
                }
            }
            Stopped::Broken { detail } => detail.clone(),
        }
    }

    /// The text to hand back to the agent if a person overrules the bound, and
    /// `None` where handing anything back would be dishonest.
    pub fn feedback(&self) -> Option<String> {
        match self {
            // The gate report holds the failing lines; the caller has it.
            Stopped::GateFailed { .. } | Stopped::OverBudget { .. } => None,
            Stopped::Broken { .. } => None,
        }
    }
}

/// Why a change counts as finished, written when it finishes — the partner of
/// `Stopped`, so a checked finish never reads like an unchecked one. No person
/// is named: there is no identity here that could fill a `who` truthfully.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum Completion {
    /// The project declares gates, and the run this names passed.
    GatesPassed {
        gate: String,
        attempt: u32,
        attempts: u32,
        at: Timestamp,
    },
    /// The gates passed, but not against the tree as it stands (or the tree
    /// could not be read). Not `ByHand`: they did pass, at `ran_at`; but it is
    /// not checked either.
    GatesStale {
        gate: String,
        attempt: u32,
        attempts: u32,
        /// The tree when the gate ran; `None` when it ran outside a repository.
        /// Boxed, with `now`, to keep the enum small.
        ran_at: Option<Box<CommitStamp>>,
        /// The tree when the change was finished; `None` is unknown, never
        /// unchanged.
        now: Option<Box<CommitStamp>>,
        at: Timestamp,
    },
    /// The project never said what done means. Must not read as a pass.
    NoGateDeclared { at: Timestamp },
    /// Gates are declared, the evidence does not support a pass, and a person
    /// finished it anyway. Legitimate, but the record says so.
    ByHand {
        at: Timestamp,
        /// What the last gate said, when there was one.
        last_gate: Option<String>,
    },
}

impl Completion {
    /// What this change rests on. Total: always a basis, never `None`.
    /// `now` is the tree as it stands (`None` if unreadable); a pass counts
    /// only against it, so touching a file makes `verified` false.
    pub fn of(change: &Change, project_declares_gates: bool, now: Option<&CommitStamp>) -> Self {
        let at = Timestamp::now();
        let attempts = change.gates.iter().filter(|g| g.gate == CHECK).count() as u32;
        let report = change.check_report();
        match (change.verdict(project_declares_gates, now), report) {
            (Standing::NoGatesDeclared, _) => Completion::NoGateDeclared { at },
            (Standing::Verified, Some(r)) => Completion::GatesPassed {
                gate: r.gate.clone(),
                attempt: r.attempt,
                attempts,
                at,
            },
            (Standing::Stale { .. }, Some(r)) => Completion::GatesStale {
                gate: r.gate.clone(),
                attempt: r.attempt,
                attempts,
                ran_at: r.commit.clone().map(Box::new),
                now: now.cloned().map(Box::new),
                at,
            },
            (_, r) => Completion::ByHand {
                at,
                last_gate: r.map(GateReport::summary),
            },
        }
    }

    /// One sentence, identifiable without the other three beside it.
    pub fn headline(&self) -> String {
        match self {
            Completion::GatesPassed { gate, attempt, attempts, .. } => {
                format!("gates passed — `{gate}`, attempt {attempt} of {attempts}")
            }
            Completion::GatesStale { gate, ran_at, now, .. } => {
                format!(
                    "the gates passed — `{gate}` — {}",
                    stale_because(ran_at.as_deref(), now.as_deref())
                )
            }
            Completion::NoGateDeclared { .. } => {
                "no gate declared — this project never said what done means, and nothing was checked"
                    .into()
            }
            Completion::ByHand { last_gate, .. } => match last_gate {
                Some(g) => format!("finished by hand — the gates did not pass ({g})"),
                None => "finished by hand — no gate had run".into(),
            },
        }
    }

    /// Whether a current check stands behind this. A stale pass does not.
    pub fn is_checked(&self) -> bool {
        matches!(self, Completion::GatesPassed { .. })
    }

    /// The warning a reader needs before treating this as a pass; `None` for
    /// a current pass. One place, so every surface warns in the same words.
    pub fn unchecked_note(&self) -> Option<&'static str> {
        match self {
            Completion::GatesPassed { .. } => None,
            Completion::GatesStale { .. } => Some(
                "The gates passed, but not against the tree as it stands: it has changed since \
                 they ran. Read the basis before reading anything below as a pass.",
            ),
            Completion::NoGateDeclared { .. } | Completion::ByHand { .. } => Some(
                "Nothing was checked by this tool for this completion. Read the basis before \
                 reading anything below as a pass.",
            ),
        }
    }

    pub fn at(&self) -> &Timestamp {
        match self {
            Completion::GatesPassed { at, .. }
            | Completion::GatesStale { at, .. }
            | Completion::NoGateDeclared { at }
            | Completion::ByHand { at, .. } => at,
        }
    }

    /// The gate run this basis points at, stale passes included.
    pub fn gate(&self) -> Option<(&str, u32)> {
        match self {
            Completion::GatesPassed { gate, attempt, .. }
            | Completion::GatesStale { gate, attempt, .. } => Some((gate.as_str(), *attempt)),
            _ => None,
        }
    }
}

/// Why a passing gate no longer describes the checkout, in one clause.
fn stale_because(ran_at: Option<&CommitStamp>, now: Option<&CommitStamp>) -> String {
    let Some(then) = ran_at else {
        return "but nothing was recorded about the tree when it ran, so nothing says what it \
                passed against"
            .into();
    };
    format!(
        "but {}",
        stale_clause(then.tree.as_deref(), now.and_then(|n| n.tree.as_deref()))
    )
}

/// Another change editing files this one is also editing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overlap {
    pub change_id: ChangeId,
    pub title: String,
    /// The files both are touching, most useful first, bounded.
    pub files: Vec<String>,
}

/// How a command ended. Distinct variants so *never ran* and *could not tell*
/// are distinguishable from each other and from an exit code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// It ran and returned a code. The only state that carries a verdict.
    Exited { code: i32 },
    /// It ran out of time and was killed, with the whole process group.
    TimedOut { after_secs: u64 },
    /// The shell could not start it: a broken gate, not a broken change.
    NeverStarted { reason: String },
    /// It ran, and its result could not be collected. Never an answer.
    Unknown { reason: String },
}

impl Outcome {
    /// The word every surface uses.
    pub fn headline(&self) -> String {
        match self {
            Outcome::Exited { code } => format!("exit {code}"),
            Outcome::TimedOut { after_secs } => format!("timed out after {after_secs}s"),
            Outcome::NeverStarted { .. } => "never started".into(),
            Outcome::Unknown { .. } => "could not be determined".into(),
        }
    }

    /// The exit code; `None` when the command never reached one.
    pub fn code(&self) -> Option<i32> {
        match self {
            Outcome::Exited { code } => Some(*code),
            _ => None,
        }
    }
}

/// What one command in a gate did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub outcome: Outcome,
    pub duration_ms: u64,
    /// The bounded tail of the output.
    pub output_tail: String,
    /// Total output size, so a reader knows what the tail is a tail of.
    pub output_bytes: u64,
    /// Digest of the whole captured output. Binds this record to that run; it
    /// does not reproduce, since both pipes interleave by scheduling.
    pub output_digest: String,
    /// Lines that look like the actual failures, where the runner is
    /// recognised; empty otherwise rather than a guess.
    pub failures: Vec<String>,
}

impl CommandResult {
    pub fn passed(&self) -> bool {
        matches!(self.outcome, Outcome::Exited { code: 0 })
    }

    /// Whether this command produced a verdict at all.
    pub fn answered(&self) -> bool {
        matches!(self.outcome, Outcome::Exited { .. })
    }

    /// A command that ran and returned a code.
    pub fn exited(command: impl Into<String>, code: i32) -> Self {
        Self {
            command: command.into(),
            outcome: Outcome::Exited { code },
            duration_ms: 0,
            output_tail: String::new(),
            output_bytes: 0,
            output_digest: crate::core::hash::hex(b""),
            failures: Vec::new(),
        }
    }

    /// A command that never produced a verdict.
    pub fn without_verdict(command: impl Into<String>, outcome: Outcome) -> Self {
        Self {
            outcome,
            ..Self::exited(command, 0)
        }
    }

    pub fn timed_out(&self) -> bool {
        matches!(self.outcome, Outcome::TimedOut { .. })
    }

    /// Ran, and said no. The only thing that reproduces a problem.
    pub fn refused(&self) -> bool {
        matches!(self.outcome, Outcome::Exited { code } if code != 0)
    }
}

/// Whether a commit is reachable by anybody other than the machine that made
/// it, so a certificate never points a reviewer at a commit they cannot fetch.
/// *No remote* is not a problem; *not on the remote that exists* is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    /// On at least one remote ref. The instructions in the certificate work.
    Remote,
    /// Here and nowhere else. Nobody else can check this.
    LocalOnly,
    /// This repository has no remote configured. Not a failure.
    NoRemote,
    /// The query failed. Never rendered as any of the other three.
    Unknown,
}

/// What the working tree was at one moment. Commit and cleanliness come from
/// a single `git status --porcelain=v2 --branch`, so they cannot straddle a
/// commit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommitStamp {
    /// `None` when the repository has no commits yet — a state, not a failure.
    pub commit: Option<String>,
    /// The working-tree digest: `git add -A && git write-tree` over the files
    /// on disk (untracked included, ignored excluded) — not `HEAD^{tree}`,
    /// since agents do not commit. `None` when it could not be computed or the
    /// tree moved while a gate ran: *cannot verify*.
    #[serde(default)]
    pub tree: Option<String>,
    /// `None` on a detached head.
    pub branch: Option<String>,
    /// Whether nothing was uncommitted or untracked. Shown, never a condition
    /// of `verified` (the digest covers what the commit does not).
    pub clean: bool,
    pub changed_files: u32,
    pub reach: Reach,
    /// Where a reviewer would obtain this commit, when anywhere.
    pub remote: Option<String>,
}

impl CommitStamp {
    /// Whether somebody else could obtain this commit and run the commands.
    pub fn checkable_by_others(&self) -> bool {
        self.commit.is_some() && matches!(self.reach, Reach::Remote)
    }
}

/// The specification a gate ran against, as it was at that moment: the path
/// the change names and a fingerprint of its bytes, so the certificate says
/// *which* version was checked. The fingerprint detects change, not forgery
/// (not cryptographic; that threat model is out of scope).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecStamp {
    /// As the change named it, relative to the project root.
    pub path: String,
    /// Of every document under it when the gate ran. `None` when the named
    /// specification was not there — a finding, not a blank.
    pub fingerprint: Option<String>,
    /// How many documents it covered: one for a file, more for a folder.
    #[serde(default)]
    pub files: u32,
    /// Ticked and total boxes when the gate ran — counted, never interpreted,
    /// so unticked work can sit beside a passing gate.
    #[serde(default)]
    pub tasks_done: u32,
    #[serde(default)]
    pub tasks_total: u32,
    /// Questions the specification says it has not answered, in the words the
    /// project chose. Zero unless it chose some.
    #[serde(default)]
    pub open_questions: u32,
}

impl Change {
    /// Whether the plan moved under this change. `None` means nobody recorded
    /// the start, which must not read as *unchanged*.
    #[must_use]
    pub fn plan_drifted(&self, now: Option<&str>) -> Option<bool> {
        let then = self.spec_at_start.as_deref()?;
        let now = now?;
        Some(then != now)
    }
}

/// The specification moved under a run, and the run never saw it. Derived
/// from the run's close against the change's start; never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "wire/", rename = "SpecDrift")
)]
pub struct Drift {
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub run: RunId,
    /// When the newest document was written; `None` when the folder could
    /// not be dated.
    #[cfg_attr(feature = "typescript", ts(type = "string | null"))]
    pub changed_at: Option<Timestamp>,
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub started_at: Timestamp,
}

impl Drift {
    /// *the specification changed 18m into run r-9f2 and the run never saw
    /// it* — or *during run r-9f2* when the change could not be dated.
    pub fn says(&self) -> String {
        match self.changed_at {
            Some(at) => {
                let secs = at
                    .duration_since(self.started_at)
                    .as_secs()
                    .max(0)
                    .unsigned_abs();
                format!(
                    "the specification changed {} into run {} and the run never saw it",
                    elapsed_words(secs),
                    self.run
                )
            }
            None => format!(
                "the specification changed during run {} and the run never saw it",
                self.run
            ),
        }
    }
}

impl Change {
    /// Closed runs that saw a different specification from this change's
    /// start, oldest first. No recorded start raises nothing (unknown is not
    /// moved). Only closes after the last one that agreed with the start count,
    /// so accepting a later run's reading does not raise every earlier run.
    #[must_use]
    pub fn drifts(&self, runs: &[&crate::core::run::Run]) -> Vec<Drift> {
        let Some(start) = self.spec_at_start.as_deref() else {
            return Vec::new();
        };
        let mine: Vec<&crate::core::run::Run> = self
            .runs
            .iter()
            .filter_map(|id| runs.iter().copied().find(|r| r.id == *id))
            .collect();
        let agreed = mine
            .iter()
            .filter_map(|r| r.observed.as_ref())
            .filter(|o| o.fingerprint.as_deref() == Some(start))
            .map(|o| o.at)
            .max();
        mine.iter()
            .filter_map(|r| {
                let seen = r.observed.as_ref()?;
                let now = seen.fingerprint.as_deref()?;
                let after = agreed.is_none_or(|t| seen.at > t);
                (now != start && after).then(|| Drift {
                    run: r.id.clone(),
                    changed_at: seen.changed_at,
                    started_at: r.started_at,
                })
            })
            .collect()
    }

    /// Moves the start forward to what the run saw; the drift disappears
    /// because the comparison does.
    pub fn accept_drift(&mut self, fingerprint: String) {
        self.spec_at_start = Some(fingerprint);
    }

    /// When the latest passing `check` report was taken — the moment
    /// *verified* is measured against for the tasks the runs were sent.
    #[must_use]
    pub fn last_pass_at(&self) -> Option<Timestamp> {
        self.gates
            .iter()
            .filter(|g| g.gate == CHECK && g.passed())
            .map(|g| g.at)
            .max()
    }
}

impl SpecStamp {
    /// Reads and fingerprints the specification a change names. A missing one
    /// is kept unfingerprinted rather than dropped. Built from the same
    /// [`Plan`] surfaces are served, so the two cannot disagree.
    ///
    /// [`Plan`]: crate::core::spec::Plan
    pub fn of(path: &str, root: &std::path::Path, markers: &[String]) -> Self {
        Self::from_plan(&crate::core::spec::Plan::read(root, path, markers))
    }

    pub fn from_plan(plan: &crate::core::spec::Plan) -> Self {
        Self {
            path: plan.path.clone(),
            fingerprint: plan.fingerprint.clone(),
            files: plan.files,
            tasks_done: plan.progress.map_or(0, |p| p.done),
            tasks_total: plan.progress.map_or(0, |p| p.total),
            open_questions: plan.open_questions,
        }
    }

    /// `(done, total)`; `None` rather than `0/0` when there is no task list.
    pub fn tasks(&self) -> Option<(u32, u32)> {
        (self.tasks_total > 0).then_some((self.tasks_done, self.tasks_total))
    }

    /// Whether boxes remain unticked. Shown beside the gate, never judged.
    pub fn has_unticked_work(&self) -> bool {
        self.tasks_total > self.tasks_done
    }
}

/// The verdict of one gate run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateReport {
    pub gate: String,
    pub at: Timestamp,
    pub duration_ms: u64,
    pub commands: Vec<CommandResult>,
    /// Which attempt this was, counting from one.
    pub attempt: u32,
    /// The specification this change is answering, stamped when the gate ran.
    #[serde(default)]
    pub spec: Option<SpecStamp>,
    /// The tree when this gate ran; `None` outside a repository (a finding).
    #[serde(default)]
    pub commit: Option<CommitStamp>,
}

/// What a gate run in a repository amounts to, for callers outside this
/// process. Four values so *no gates* and *unreadable config* never collapse
/// into *verified* or *failed*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum GateState {
    /// Gates were declared and every command passed.
    Verified,
    /// Gates were declared and at least one did not pass.
    Failed,
    /// No `devplane.toml`, or one that declares no checks.
    NoGates,
    /// The configuration exists and will not parse, so the project's own rules
    /// are not in force and nothing was run.
    ConfigUnreadable,
}

impl GateState {
    pub fn as_str(self) -> &'static str {
        match self {
            GateState::Verified => "verified",
            GateState::Failed => "failed",
            GateState::NoGates => "no_gates",
            GateState::ConfigUnreadable => "config_unreadable",
        }
    }

    /// Only `Verified` is a pass; *nothing was checked* is never success.
    pub fn passed(self) -> bool {
        matches!(self, GateState::Verified)
    }

    /// One sentence, and no two of them read alike.
    pub fn says(self) -> &'static str {
        match self {
            GateState::Verified => "This project's own checks passed.",
            GateState::Failed => "A check did not pass. Nothing here has been marked done.",
            GateState::NoGates => {
                "This repository declares no checks, so nothing is verified and nothing pretends to be."
            }
            GateState::ConfigUnreadable => {
                "This repository's devplane.toml could not be read, so nothing was run — and this is not a pass."
            }
        }
    }
}

impl GateReport {
    pub fn passed(&self) -> bool {
        // An empty gate proves nothing, so it is never a pass.
        if self.commands.is_empty() {
            return false;
        }
        self.commands.iter().all(CommandResult::passed)
    }

    /// A short summary a human can act on, and an agent can be told.
    pub fn summary(&self) -> String {
        if self.passed() {
            return format!("{} passed", self.gate);
        }
        let failed: Vec<&CommandResult> = self.commands.iter().filter(|c| !c.passed()).collect();
        let first = failed.first();
        match first {
            Some(c) if c.timed_out() => format!("{} timed out: {}", self.gate, c.command),
            Some(c) if !c.answered() => {
                format!(
                    "{} could not run: {} ({})",
                    self.gate,
                    c.command,
                    c.outcome.headline()
                )
            }
            Some(c) if !c.failures.is_empty() => format!(
                "{} failed: {} ({} failing)",
                self.gate,
                c.command,
                c.failures.len()
            ),
            Some(c) => format!("{} failed: {}", self.gate, c.command),
            None => format!("{} failed", self.gate),
        }
    }

    /// What to send back to the agent: failures only, not the whole log,
    /// to spare its context window.
    pub fn feedback(&self) -> String {
        let mut out = String::from(
            "The project's checks did not pass. Fix the cause, do not disable the check.\n",
        );
        for c in self.commands.iter().filter(|c| !c.passed()) {
            out.push_str(&format!("\n$ {}\n", c.command));
            if !c.answered() {
                out.push_str(&c.outcome.headline());
                out.push('\n');
                continue;
            }
            if c.failures.is_empty() {
                out.push_str(&c.output_tail);
            } else {
                for f in c.failures.iter().take(20) {
                    out.push_str(f);
                    out.push('\n');
                }
            }
        }
        out
    }
}

/// The pull request a change produced. Number and URL are fixed; the status
/// is refreshed by the poller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PullRequestRef {
    pub number: u64,
    pub url: String,
    /// `pending`, `failing`, `ready_for_review`, `ready_to_merge`,
    /// `changes_requested`, `draft`, `merged`, `closed`.
    pub status: String,
    /// Names of the checks that are red, for the inbox card.
    #[serde(default)]
    pub failing_checks: Vec<String>,
}

/// A unit of work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub id: ChangeId,
    pub project_id: ProjectId,
    pub title: String,
    /// What the agent was asked for.
    pub prompt: String,
    /// The specification this change answers, relative to the project root.
    /// The gate stamps it onto the done certificate.
    #[serde(default)]
    pub spec: Option<String>,
    /// The specification's fingerprint when this change started, compared
    /// against what later runs saw to detect drift. `None` is *unknown*, not
    /// *unchanged* (see [`Change::plan_drifted`]).
    #[serde(default)]
    pub spec_at_start: Option<String>,
    /// The isolated checkout, when the change has one.
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    /// The report this change was started from, when a person started it from
    /// one. Offering or finishing the change answers that report as fixed.
    #[serde(default)]
    pub from_report: Option<crate::core::ReportId>,
    /// Runs that have worked on this, oldest first.
    pub runs: Vec<RunId>,
    /// What preparing the checkout did. Not a gate and not in `gates`: it says
    /// nothing about done and is not an attempt.
    #[serde(default)]
    pub setup: Option<GateReport>,
    pub gates: Vec<GateReport>,
    /// How many times failures have been handed back to the agent.
    pub feedback_rounds: u32,
    /// Other changes in this repository editing the same files, as of this
    /// change's last checkpoint — two valid branches that may not both land.
    /// Measured, never refused.
    #[serde(default)]
    pub overlaps: Vec<Overlap>,
    /// Cost so far, summed over runs; kept because runs age off the board.
    #[serde(default)]
    pub cost_usd: f64,
    /// Why this change counts as finished. Only `finish` writes it.
    #[serde(default)]
    pub completion: Option<Completion>,
    /// Why the change stopped; cleared when a person hands it back. The inbox
    /// reads this rather than guessing from the last gate report.
    #[serde(default)]
    pub stopped: Option<Stopped>,
    /// What this is waiting on, shown beside the state, never as it.
    #[serde(default)]
    pub waiting: Option<Waiting>,
    /// The tree as the host last saw it (refreshed on run end, gate finish and
    /// a slow tick). `state` compares it with the last gate's stamp, so
    /// *verified* follows the tree. `None` is unknown, never unchanged.
    #[serde(default)]
    pub tree_now: Option<CommitStamp>,
    /// When the worktree was removed. Archived is not deleted: the record stays.
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    /// The pull request a person offered this as, once there is one.
    #[serde(default)]
    pub pull_request: Option<PullRequestRef>,
    /// Hidden from the inbox until this moment. Separate from the run's
    /// snooze: change-level items (a red pull request, a spent feedback
    /// budget) often outlive every run.
    #[serde(default)]
    pub snoozed: crate::core::attention::Snoozed,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Change {
    pub fn new(project_id: ProjectId, title: String, prompt: String) -> Self {
        let now = Timestamp::now();
        Self {
            id: ChangeId::new(format!("c-{}", uuid::Uuid::now_v7().simple())),
            project_id,
            title,
            prompt,
            spec: None,
            spec_at_start: None,
            worktree: None,
            branch: None,
            from_report: None,
            runs: Vec::new(),
            setup: None,
            gates: Vec::new(),
            feedback_rounds: 0,
            overlaps: Vec::new(),
            cost_usd: 0.0,
            completion: None,
            stopped: None,
            waiting: None,
            tree_now: None,
            archived_at: None,
            pull_request: None,
            snoozed: Default::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// What stopping one of this change's runs leaves behind: it ends the
    /// agent's turn only; change, checkout, branch and record stay.
    pub fn stop_says(&self) -> String {
        let mut stays = vec!["the change".to_string()];
        if let Some(w) = &self.worktree {
            stays.push(format!("its worktree {}", w.display()));
        }
        if let Some(b) = &self.branch {
            stays.push(format!("its branch {b}"));
        }
        stays.push("its record".to_string());
        let last = stays.pop().unwrap_or_default();
        format!(
            "stopping ends the agent's turn; {} and {last} stay",
            stays.join(", ")
        )
    }

    /// Where this change is, from the record and `now` (the tree at read time;
    /// `None` is unknown and never verified). Pure, so nothing stores it.
    pub fn state(&self, now: Option<&CommitStamp>) -> ChangeState {
        if self.archived_at.is_some() {
            return ChangeState::Archived;
        }
        if self.pull_request.is_some() {
            return ChangeState::Offered;
        }
        if self.is_verified(now) {
            return ChangeState::Verified;
        }
        if !self.runs.is_empty() {
            return ChangeState::InFlight;
        }
        if self.worktree.is_some() {
            return ChangeState::Isolated;
        }
        ChangeState::Drafted
    }

    /// The state against the tree the host last saw.
    pub fn current_state(&self) -> ChangeState {
        self.state(self.tree_now.as_ref())
    }

    /// `check` exited zero against the tree as it stands. Reports exist only
    /// for declared gates, so `declares_gates` is implied.
    pub fn is_verified(&self, now: Option<&CommitStamp>) -> bool {
        self.verdict(true, now) == Standing::Verified
    }

    /// The one derivation of `verified`: the latest [`CHECK`] report passed
    /// and its working-tree digest equals `now`'s. `now` is `None` when the
    /// tree could not be read, which is never *unchanged*.
    pub fn verdict(&self, declares_gates: bool, now: Option<&CommitStamp>) -> Standing {
        if !declares_gates {
            return Standing::NoGatesDeclared;
        }
        let Some(report) = self.check_report() else {
            return Standing::NotRun;
        };
        if !report.passed() {
            return Standing::Failed {
                summary: report.summary(),
            };
        }
        if crate::core::reduce::facts::still_current(report.commit.as_ref(), now) {
            return Standing::Verified;
        }
        Standing::Stale {
            then: report.commit.as_ref().and_then(|c| c.tree.clone()),
            now: now.and_then(|c| c.tree.clone()),
        }
    }

    /// The latest [`CHECK`] report — the only one that can verify a change.
    pub fn check_report(&self) -> Option<&GateReport> {
        self.gates.iter().rev().find(|g| g.gate == CHECK)
    }

    /// Nothing more will happen on its own: finished, stopped, or archived.
    pub fn is_settled(&self) -> bool {
        self.archived_at.is_some() || self.completion.is_some() || self.stopped.is_some()
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.is_some()
    }

    /// The machine's part is done and it is a person's turn.
    pub fn needs_a_person(&self) -> bool {
        matches!(self.waiting, Some(Waiting::Person))
    }

    /// Whether handing this back to the agent would do something — one
    /// answer for the API, inbox and CLI. `can_drive`: whether the session
    /// that wrote the code is still there, which only the caller can see.
    pub fn retryable(&self, can_drive: bool) -> bool {
        if self.archived_at.is_some() || !can_drive {
            return false;
        }
        match &self.stopped {
            // `change retry` clears this reason first when the ceiling was
            // raised, so here it is never retryable.
            Some(Stopped::OverBudget { .. }) => false,
            Some(Stopped::Broken { .. }) | None => false,
            // The failing lines come from the gate report.
            Some(Stopped::GateFailed { .. }) => self.last_gate().is_some(),
        }
    }

    /// Validates the specification (a file or a folder) this change answers.
    /// Untrusted input: refused unless it exists inside the project — an error,
    /// never a silently dropped field.
    pub fn with_spec(root: &std::path::Path, spec: &str) -> Result<String, String> {
        let joined = root.join(spec);
        if !crate::core::policy::within(root, &joined) {
            return Err(format!("{spec} is outside the project"));
        }
        if !joined.exists() {
            return Err(format!("{spec} is not in this project"));
        }
        // A folder with no Markdown is a typo; catch it now, not at the gate.
        if joined.is_dir()
            && crate::core::spec::Spec::read(root, spec, &[])
                .docs
                .is_empty()
        {
            return Err(format!("{spec} holds no markdown"));
        }
        // Stored relative, so the certificate reads the same on any machine.
        Ok(joined
            .strip_prefix(root)
            .unwrap_or(&joined)
            .to_string_lossy()
            .replace('\\', "/"))
    }

    /// Whether the agent's own account is worth showing: only beside a failed
    /// gate that can contradict it. Alone it reads as a summary and is not
    /// one; neither is judged, the person reads both.
    pub fn claim_is_worth_showing(&self) -> bool {
        self.last_gate().is_some_and(|g| !g.passed())
    }

    /// The most recent gate verdict, which is what the board shows.
    pub fn last_gate(&self) -> Option<&GateReport> {
        self.gates.last()
    }

    /// Whether this change works in the person's own checkout.
    pub fn in_place(&self) -> bool {
        self.worktree.is_none()
    }

    /// What an in-place change gives up, one sentence for every surface.
    /// `None` for a change with a tree of its own.
    pub fn in_place_says(&self) -> Option<&'static str> {
        self.in_place()
            .then_some("in place — no parallel safety, and the diff is against the working tree")
    }

    pub fn current_run(&self) -> Option<&RunId> {
        self.runs.last()
    }

    /// A filesystem-safe slug for the branch and worktree: the title, plus
    /// part of the id so equal titles cannot collide.
    pub fn slug(&self) -> String {
        let base: String = self
            .title
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .take(5)
            .collect::<Vec<_>>()
            .join("-");
        // The tail of a v7 uuid is random; the head is a timestamp.
        let id = self.id.as_str();
        let short = &id[id.len().saturating_sub(6)..];
        if base.is_empty() {
            format!("change-{short}")
        } else {
            format!("{base}-{short}")
        }
    }

    pub fn branch_name(&self) -> String {
        format!("change/{}", self.slug())
    }
}

#[cfg(test)]
mod drift_tests {
    use super::*;
    use crate::core::run::{Observed, Run, RunMode};
    use crate::core::{ProjectId, SessionId};

    fn change(start: Option<&str>) -> Change {
        let mut w = Change::new(ProjectId::new("p"), "x".into(), "…".into());
        w.spec = Some("specs/1".into());
        w.spec_at_start = start.map(str::to_string);
        w
    }

    /// A run that started `minute` minutes past ten and closed a hundred
    /// seconds later, having read `saw`.
    fn closed(id: &str, saw: Option<&str>, minute: i64) -> Run {
        let mut r = Run::new(
            SessionId::new(id),
            std::path::PathBuf::from("/repo"),
            RunMode::Driven,
            "echo",
        );
        let started: Timestamp = "2026-09-25T10:00:00Z".parse().unwrap();
        r.started_at = started + jiff::SignedDuration::from_mins(minute);
        r.observed = Some(Observed {
            fingerprint: saw.map(str::to_string),
            ticked: vec![],
            changed_at: Some(r.started_at + jiff::SignedDuration::from_secs(90)),
            at: r.started_at + jiff::SignedDuration::from_secs(100),
        });
        r
    }

    /// Held still: nothing. Moved: one per run that closed against it.
    /// Accepted: nothing again, and a start nobody recorded is not a move.
    #[test]
    fn a_drift_is_one_per_run_that_saw_a_moved_specification() {
        let a = closed("a", Some("v1"), 0);
        let b = closed("b", Some("v2"), 5);
        let open = Run::new(
            SessionId::new("c"),
            std::path::PathBuf::from("/repo"),
            RunMode::Driven,
            "echo",
        );
        let mut w = change(Some("v1"));
        w.runs = vec![a.id.clone(), b.id.clone(), open.id.clone()];
        let runs = [&a, &b, &open];

        assert_eq!(w.drifts(&runs[..1]).len(), 0, "held still");
        let drifts = w.drifts(&runs);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].run, b.id);
        assert_eq!(
            drifts[0].says(),
            format!(
                "the specification changed 1m 30s into run {} and the run never saw it",
                b.id
            )
        );

        // Two runs that both closed against a moved document are two items,
        // oldest first, in the change's own run order.
        let c = closed("d", Some("v3"), 10);
        w.runs.push(c.id.clone());
        let both = w.drifts(&[&a, &b, &c]);
        assert_eq!(
            both.iter().map(|d| d.run.as_str()).collect::<Vec<_>>(),
            [b.id.as_str(), c.id.as_str()]
        );

        w.accept_drift("v2".into());
        assert_eq!(w.drifts(&[&a, &b]).len(), 0, "accepted");
        assert_eq!(w.spec_at_start.as_deref(), Some("v2"));
        // Accepting a later reading does not raise earlier runs; a still later
        // version does.
        let later = w.drifts(&[&a, &b, &c]);
        assert_eq!(
            later.iter().map(|d| d.run.as_str()).collect::<Vec<_>>(),
            [c.id.as_str()]
        );

        // Unknown is not unchanged: no start recorded raises nothing.
        let unknown = change(None);
        let mut unknown = unknown;
        unknown.runs = vec![b.id.clone()];
        assert!(unknown.drifts(&[&b]).is_empty());

        // A close that found no specification is not a drift either.
        let gone = closed("e", None, 0);
        let mut w2 = change(Some("v1"));
        w2.runs = vec![gone.id.clone()];
        assert!(w2.drifts(&[&gone]).is_empty());
    }

    /// The moment *verified* is measured against is the latest pass.
    #[test]
    fn the_last_pass_is_the_latest_passing_gate() {
        let mut w = change(None);
        assert_eq!(w.last_pass_at(), None);
        let at = |s: &str| -> Timestamp { s.parse().unwrap() };
        let report = |when: &str, ok: bool| GateReport {
            gate: "check".into(),
            at: at(when),
            duration_ms: 1,
            commands: vec![CommandResult::exited("true", if ok { 0 } else { 1 })],
            attempt: 1,
            spec: None,
            commit: None,
        };
        w.gates.push(report("2026-09-25T10:00:00Z", true));
        w.gates.push(report("2026-09-25T11:00:00Z", false));
        assert_eq!(w.last_pass_at(), Some(at("2026-09-25T10:00:00Z")));
        w.gates.push(report("2026-09-25T12:00:00Z", true));
        assert_eq!(w.last_pass_at(), Some(at("2026-09-25T12:00:00Z")));
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn stopping_names_what_stays() {
        let mut w = Change::new(
            crate::core::ids::ProjectId::from_path(std::path::Path::new("/tmp/x")),
            "t".into(),
            "p".into(),
        );
        assert_eq!(
            w.stop_says(),
            "stopping ends the agent's turn; the change and its record stay"
        );
        w.worktree = Some("/tmp/x/.claude/worktrees/t".into());
        w.branch = Some("quick/t".into());
        assert_eq!(
            w.stop_says(),
            "stopping ends the agent's turn; the change, its worktree /tmp/x/.claude/worktrees/t, \
             its branch quick/t and its record stay"
        );
    }

    fn specdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-spec-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_agents_account_is_shown_beside_a_failed_gate_and_nowhere_else() {
        let mut w = Change::new(
            crate::core::ids::ProjectId::from_path(std::path::Path::new("/tmp/x")),
            "t".into(),
            "p".into(),
        );
        // No gate has run: nothing to read it against.
        assert!(!w.claim_is_worth_showing());

        let report = |ok: bool| GateReport {
            gate: "check".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            attempt: 1,
            spec: None,
            commit: None,
            commands: vec![CommandResult {
                duration_ms: 1,
                ..CommandResult::exited("cargo test", if ok { 0 } else { 101 })
            }],
        };

        w.gates.push(report(true));
        assert!(!w.claim_is_worth_showing());

        w.gates.push(report(false));
        assert!(w.claim_is_worth_showing());
    }

    #[test]
    fn a_change_may_name_a_specification_inside_its_own_project() {
        let root = specdir("ok");
        std::fs::write(root.join("spec.md"), "# it must log in\n").unwrap();
        assert_eq!(Change::with_spec(&root, "spec.md").unwrap(), "spec.md");

        // And a folder, as spec-driven frameworks produce.
        let feature = root.join("specs/001-login");
        std::fs::create_dir_all(&feature).unwrap();
        std::fs::write(feature.join("spec.md"), "# Login\n").unwrap();
        std::fs::write(feature.join("tasks.md"), "- [ ] T001\n").unwrap();
        assert_eq!(
            Change::with_spec(&root, "specs/001-login").unwrap(),
            "specs/001-login"
        );

        // A folder with nothing to read in it is a mistyped path.
        std::fs::create_dir_all(root.join("specs/002-empty")).unwrap();
        assert!(Change::with_spec(&root, "specs/002-empty").is_err());
    }

    #[test]
    fn a_specification_outside_the_project_is_refused_rather_than_dropped() {
        let root = specdir("escape");
        std::fs::write(root.join("spec.md"), "x").unwrap();
        assert!(Change::with_spec(&root, "../outside.md").is_err());
        assert!(Change::with_spec(&root, "/etc/passwd").is_err());
        std::fs::create_dir_all(root.join("specs")).unwrap();
        // An empty folder and an absent path are both refused.
        assert!(Change::with_spec(&root, "specs").is_err());
        assert!(Change::with_spec(&root, "nope.md").is_err());
    }

    #[test]
    fn the_stamp_records_what_the_specification_was_and_says_when_it_was_not_there() {
        let root = specdir("stamp");
        std::fs::write(root.join("spec.md"), "# One\n\n- [x] a\n- [ ] b\n").unwrap();
        let first = SpecStamp::of("spec.md", &root, &[]);
        assert_eq!(first.path, "spec.md");
        assert_eq!(first.files, 1);
        assert_eq!(first.tasks(), Some((1, 2)));
        assert!(first.has_unticked_work());
        let a = first
            .fingerprint
            .clone()
            .expect("a present file is fingerprinted");

        // The same path with different content is a different stamp.
        std::fs::write(root.join("spec.md"), "# One\n\n- [x] a\n- [x] b\n").unwrap();
        let second = SpecStamp::of("spec.md", &root, &[]);
        assert_ne!(Some(a), second.fingerprint, "a ticked box is a change");
        assert_eq!(second.tasks(), Some((2, 2)));
        assert!(!second.has_unticked_work());

        // A missing specification keeps its path and has no fingerprint.
        std::fs::remove_file(root.join("spec.md")).unwrap();
        let gone = SpecStamp::of("spec.md", &root, &[]);
        assert_eq!(gone.path, "spec.md");
        assert!(gone.fingerprint.is_none());
        // And no task list is not the same as no progress.
        assert_eq!(gone.tasks(), None);
    }
    use super::*;

    fn change(title: &str) -> Change {
        Change::new(ProjectId::new("/repo"), title.into(), "fix it".into())
    }

    #[test]
    fn a_running_setup_says_the_command_and_how_long() {
        let since = Timestamp::now() - jiff::SignedDuration::from_secs(72);
        let w = Waiting::Setup {
            command: "pnpm install --frozen-lockfile".into(),
            since,
        };
        let said = w.says();
        assert!(
            said.starts_with("installing · `pnpm install --frozen-lockfile` · 1m 1"),
            "{said}"
        );
        // Setup waits on a machine, so a change in it is not settled.
        let mut c = change("install");
        c.waiting = Some(w);
        assert!(!c.is_settled());
    }

    #[test]
    fn an_in_place_change_marks_what_it_gives_up_and_an_isolated_one_says_nothing() {
        let mut c = change("edit here");
        assert!(c.in_place());
        assert_eq!(
            c.in_place_says(),
            Some("in place — no parallel safety, and the diff is against the working tree")
        );
        c.worktree = Some(std::path::PathBuf::from("/repo/.claude/worktrees/x"));
        assert!(!c.in_place());
        assert_eq!(c.in_place_says(), None);
    }

    #[test]
    fn elapsed_is_written_the_way_people_say_it() {
        assert_eq!(elapsed_words(0), "0s");
        assert_eq!(elapsed_words(59), "59s");
        assert_eq!(elapsed_words(72), "1m 12s");
        assert_eq!(elapsed_words(3600), "1h 0m");
        assert_eq!(elapsed_words(3725), "1h 2m");
    }

    #[test]
    fn a_slug_is_readable_and_unique() {
        let w = change("Fix the flaky login test on CI");
        let slug = w.slug();
        assert!(slug.starts_with("fix-the-flaky-login-test"), "{slug}");
        assert_eq!(w.branch_name(), format!("change/{slug}"));
        // Two items with the same title must not collide on disk.
        assert_ne!(change("Fix the flaky login test on CI").slug(), slug);
    }

    #[test]
    fn a_title_of_punctuation_still_produces_a_usable_name() {
        let w = change("!!! ???");
        assert!(!w.slug().contains(' '));
        assert!(w.slug().starts_with("change-"));
    }

    #[test]
    fn a_gate_passes_only_when_every_command_does() {
        let ok = CommandResult {
            duration_ms: 10,
            ..CommandResult::exited("cargo test", 0)
        };
        let bad = CommandResult {
            outcome: Outcome::Exited { code: 101 },
            failures: vec!["test auth::login ... FAILED".into()],
            ..ok.clone()
        };
        let report = |cmds: Vec<CommandResult>| GateReport {
            gate: "check".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            commands: cmds,
            attempt: 1,
            spec: None,
            commit: None,
        };
        assert!(report(vec![ok.clone()]).passed());
        assert!(!report(vec![ok.clone(), bad.clone()]).passed());
        // An empty gate is not a passing gate.
        assert!(!report(vec![]).passed());
    }

    #[test]
    fn feedback_names_the_failures_not_the_whole_log() {
        let report = GateReport {
            gate: "check".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            attempt: 1,
            spec: None,
            commit: None,
            commands: vec![CommandResult {
                duration_ms: 1,
                output_tail: "a hundred lines of noise".repeat(50),
                failures: vec!["test auth::login ... FAILED".into()],
                ..CommandResult::exited("cargo test", 101)
            }],
        };
        let f = report.feedback();
        assert!(f.contains("auth::login"));
        assert!(
            !f.contains("hundred lines of noise"),
            "the log is not the message"
        );
        assert!(f.contains("do not disable the check"));
    }

    #[test]
    fn a_timeout_is_reported_as_one() {
        let report = GateReport {
            gate: "check".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            attempt: 2,
            spec: None,
            commit: None,
            commands: vec![CommandResult {
                output_tail: String::new(),
                ..CommandResult::without_verdict(
                    "cargo test",
                    Outcome::TimedOut { after_secs: 600 },
                )
            }],
        };
        assert!(!report.passed());
        assert!(report.summary().contains("timed out"));
    }
    /// Four situations, four distinct sentences, one pass.
    #[test]
    fn only_a_verified_gate_is_a_pass_and_no_two_states_read_alike() {
        use GateState::*;
        let all = [Verified, Failed, NoGates, ConfigUnreadable];

        assert!(Verified.passed());
        for state in [Failed, NoGates, ConfigUnreadable] {
            assert!(!state.passed(), "{state:?} must not be a pass");
        }

        let said: Vec<&str> = all.iter().map(|s| s.says()).collect();
        for (i, a) in said.iter().enumerate() {
            for b in said.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }

        // Neither non-failure claims a pass; the failure says it did not pass.
        assert!(!NoGates.says().contains("passed"), "{}", NoGates.says());
        assert!(
            !ConfigUnreadable.says().contains("passed"),
            "{}",
            ConfigUnreadable.says()
        );
        assert!(Failed.says().contains("did not pass"));

        // The wire spellings are asked of serde rather than reconstructed.
        for state in all {
            assert_eq!(
                serde_json::to_string(&state).unwrap(),
                format!("\"{}\"", state.as_str())
            );
        }
    }
}

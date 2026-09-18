//! Work — the durable unit.
//!
//! A session is a process; it crashes, gets resumed, is handed to an editor.
//! Work is what survives all of that: the branch, the worktree, the gates that
//! decide whether it is finished, and the runs that have tried. One issue can
//! span several sessions and produce several pull requests, which is why the
//! thing being tracked cannot be the session.
//!
//! The phase a Work item is in is the answer to "what has to happen next", and
//! `Verify` is the phase that makes the product worth using: it is where "the
//! agent says it is done" becomes "the project's own checks agree".

use crate::core::ids::{ProjectId, RunId, WorkId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What kind of work this is. The kind chooses the pipeline, not the priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    /// One prompt, gates, done. No ceremony, and the default.
    Quick,
    /// Templated and batchable: bump dependencies, fix CI, address review.
    Chore,
    /// Reproduce first, then fix. The reproduction is the gate.
    Bug,
    /// Spec, plan, implement, review — the full pipeline.
    Feature,
}

impl WorkKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkKind::Quick => "quick",
            WorkKind::Chore => "chore",
            WorkKind::Bug => "bug",
            WorkKind::Feature => "feature",
        }
    }

    /// The branch prefix for this kind, so a glance at `git branch` says what
    /// each one was for.
    pub fn branch_prefix(&self) -> &'static str {
        match self {
            WorkKind::Quick => "work",
            WorkKind::Chore => "chore",
            WorkKind::Bug => "fix",
            WorkKind::Feature => "feat",
        }
    }
}

/// Where a Work item has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Created, nothing started.
    Ready,
    /// An agent is working in the worktree.
    Implement,
    /// The gates are running.
    Verify,
    /// Gates passed; a human should look.
    Review,
    /// A declared human step has been reached and the pipeline is suspended
    /// until somebody releases it. Distinct from `Review`, which is where work
    /// lands when it is simply finished: this one the project asked for.
    Human,
    /// Finished.
    Done,
    /// Gates failed past their feedback budget, or the run died.
    Failed,
    Cancelled,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Ready => "ready",
            Phase::Implement => "implement",
            Phase::Verify => "verify",
            Phase::Review => "review",
            Phase::Human => "human",
            Phase::Done => "done",
            Phase::Failed => "failed",
            Phase::Cancelled => "cancelled",
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, Phase::Done | Phase::Failed | Phase::Cancelled)
    }
}

/// Why a piece of work stopped, written when it stops.
///
/// Four unrelated reasons reach `Phase::Failed` and they want four different
/// answers from a person, so the reason is recorded rather than reconstructed
/// from the last gate report — which on a chain stopped by its *reviewer* is a
/// gate that passed. The decision log's rule, one level down: an observation
/// can be re-derived, a reason cannot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Stopped {
    /// The project's checks did not pass and the feedback budget is spent.
    GateFailed { gate: String },
    /// A reviewing step kept finding things until its loop was spent. Carries
    /// what the reviewer last found, because that is the evidence a person
    /// needs and the text the agent should be handed if they say to continue.
    ReviewExhausted {
        step: String,
        back_to: String,
        findings: String,
    },
    /// The work passed a bound its project set.
    ///
    /// `bound` says **which** one: the three are not interchangeable, and a
    /// person needs to know whether the answer is "raise the money" or "this
    /// has been running for three hours".
    OverBudget { spent_usd: f64, bound: String },
    /// The chain itself could not continue — a step that is no longer declared,
    /// a gate nobody wrote, a worktree that has gone. Nothing an agent can fix,
    /// so this one is never handed back.
    Broken { detail: String },
}

impl Stopped {
    /// A one-line summary for a list.
    pub fn headline(&self) -> String {
        match self {
            Stopped::GateFailed { gate } => format!("{gate} failed"),
            Stopped::ReviewExhausted { step, .. } => format!("{step} kept finding things"),
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
            Stopped::ReviewExhausted { findings, .. } => Some(format!(
                "A reviewer found these and they are not yet addressed. Fix the cause \
                 rather than the symptom, and do not disable a check to make it pass.\n\n{findings}"
            )),
            // The gate report holds the failing lines; the caller has it.
            Stopped::GateFailed { .. } | Stopped::OverBudget { .. } => None,
            Stopped::Broken { .. } => None,
        }
    }
}

/// Another piece of work editing files this one is also editing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overlap {
    pub work_id: WorkId,
    pub title: String,
    /// The files both are touching, most useful first and bounded — a list
    /// nobody can read is a list nobody reads.
    pub files: Vec<String>,
}

/// What one command in a gate did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    /// The tail of the output. Bounded, because a failing test suite can
    /// produce megabytes and none of it belongs in a database row.
    pub output_tail: String,
    /// Lines that look like the actual failures, where the runner is
    /// recognised. Empty when it is not — an honest nothing beats a guess.
    pub failures: Vec<String>,
    pub timed_out: bool,
}

impl CommandResult {
    pub fn passed(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }
}

/// The specification a gate ran against, as it was at that moment.
///
/// **The path is the project's, and nothing here opens it for meaning.** No
/// format is known, no layout is detected, no checkbox is parsed — that is the
/// subsystem this design deleted and keeps deleted. What is recorded is the
/// file the work says it is answering, and a fingerprint of its bytes when the
/// gate ran.
///
/// The fingerprint is the half that makes the certificate worth anything.
/// *Checked against `spec.md`* is not a claim a reviewer can act on once
/// `spec.md` has moved; *checked against `spec.md` at `3f9a…`* is. It is the
/// drift problem applied to the record of the drift check.
///
/// **Not a cryptographic hash, and the reason is the one the decision log
/// already gives.** This detects *change*, not forgery: an adversary with write
/// access to this file has write access to the machine the agents run on, which
/// is not a threat model this product claims to defend against. A 64-bit
/// content hash costs no dependency; `#audit-chain` is where a stronger one
/// would arrive if that threat model ever changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecStamp {
    /// As the work named it, relative to the project root.
    pub path: String,
    /// Of every document under it when the gate ran. `None` when the work names
    /// a specification that was not there — which is a finding, not a blank.
    pub fingerprint: Option<String>,
    /// How many documents it covered. One for a file; more for the folder every
    /// spec-driven framework in the field actually produces.
    #[serde(default)]
    pub files: u32,
    /// Ticked boxes, and how many there were, at the moment the gate ran.
    ///
    /// **The number that can contradict a report.** An agent's account of its
    /// own work references about one action in eleven, so *the gate passed and
    /// the specification it answers has eleven boxes unticked* is a sentence
    /// neither the exit code nor the agent can produce alone. Counted, never
    /// interpreted: a task list is the one thing these frameworks spell the
    /// same way.
    #[serde(default)]
    pub tasks_done: u32,
    #[serde(default)]
    pub tasks_total: u32,
    /// Questions the specification says it has not answered, in the words the
    /// project chose. Zero unless it chose some.
    #[serde(default)]
    pub open_questions: u32,
}

impl SpecStamp {
    /// Reads and fingerprints the specification a work names.
    ///
    /// A missing or unreadable specification is recorded as
    /// present-but-unfingerprinted rather than dropped: *the work says it
    /// answers `specs/reset/` and there was no such folder* is exactly the kind
    /// of thing a done certificate exists to say out loud.
    pub fn of(path: &str, root: &std::path::Path, markers: &[String]) -> Self {
        let spec = crate::core::spec::Spec::read(root, path, markers);
        let (tasks_done, tasks_total) = spec.tasks();
        Self {
            path: path.to_string(),
            fingerprint: spec.fingerprint(),
            files: spec.docs.len() as u32,
            tasks_done,
            tasks_total,
            open_questions: spec.questions() as u32,
        }
    }

    /// What the boxes say, when there are any.
    ///
    /// `None` rather than `0/0`: a specification with no task list has not
    /// reported no progress, it has reported nothing, and the two must not read
    /// the same.
    pub fn tasks(&self) -> Option<(u32, u32)> {
        (self.tasks_total > 0).then_some((self.tasks_done, self.tasks_total))
    }

    /// Whether the boxes contradict a passing gate.
    ///
    /// The product judges nothing here — it puts the two on one screen and the
    /// person reads, which is the same rule the agent's own account follows.
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
    /// Set when the gate is a reproduction: the commands are *supposed* to
    /// fail, and one that succeeds has not reproduced anything. Carried on the
    /// report so that `passed` means the same thing everywhere it is read.
    #[serde(default)]
    pub expect_fail: bool,
    /// The specification this work is answering, stamped when the gate ran.
    #[serde(default)]
    pub spec: Option<SpecStamp>,
}

impl GateReport {
    pub fn passed(&self) -> bool {
        // An empty gate is never a pass. A definition of done with no commands
        // in it proves nothing, and neither does a reproduction with none.
        if self.commands.is_empty() {
            return false;
        }
        if self.expect_fail {
            // **A reproduction is a command that ran and said no**, not merely
            // one that failed to say yes. `!passed()` is true for a command
            // that timed out, one the shell could not start, and one killed
            // with a signal — none of which is evidence that a bug exists, and
            // all of which would otherwise report "reproduced the problem" and
            // let the fix step begin against a demonstration nobody has seen.
            // It is the rule the worktree code already follows in the other
            // direction: *"I could not tell" must never read as an answer.*
            if self.commands.iter().any(|c| c.timed_out) {
                return false;
            }
            return self
                .commands
                .iter()
                .any(|c| c.exit_code.is_some_and(|code| code != 0));
        }
        self.commands.iter().all(CommandResult::passed)
    }

    /// A short summary a human can act on, and an agent can be told.
    pub fn summary(&self) -> String {
        if self.passed() {
            return match self.expect_fail {
                true => format!("{} reproduced the problem", self.gate),
                false => format!("{} passed", self.gate),
            };
        }
        if self.expect_fail {
            // The two ways a reproduction fails want different sentences: one
            // says the bug is not there, the other says nobody found out.
            if let Some(c) = self.commands.iter().find(|c| c.timed_out) {
                return format!(
                    "{} timed out, so nothing was reproduced either way: {}",
                    self.gate, c.command
                );
            }
            if let Some(c) = self.commands.iter().find(|c| c.exit_code.is_none()) {
                return format!("{} could not run: {}", self.gate, c.command);
            }
            return format!("{} did not reproduce the problem", self.gate);
        }
        let failed: Vec<&CommandResult> = self.commands.iter().filter(|c| !c.passed()).collect();
        let first = failed.first();
        match first {
            Some(c) if c.timed_out => format!("{} timed out: {}", self.gate, c.command),
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

    /// What to send back to the agent. Compact on purpose: a whole test log
    /// re-fills the context window the agent needs to fix the problem.
    pub fn feedback(&self) -> String {
        if self.expect_fail {
            // A reproduction that never ran is a different message from one
            // that ran and passed: telling an agent to "write a check that
            // fails" when its check timed out sends it to rewrite something
            // that may already be right.
            if let Some(c) = self
                .commands
                .iter()
                .find(|c| c.timed_out || c.exit_code.is_none())
            {
                return format!(
                    "`{}` did not finish, so nothing has been reproduced either way:\n\n$ {}\n{}\n\n\
                     Make it terminate — a reproduction nobody can run is not one.\n",
                    self.gate,
                    c.command,
                    if c.timed_out {
                        "timed out"
                    } else {
                        "could not run"
                    }
                );
            }
            // Every command succeeded when the point was to fail. Handing back
            // a log of passing tests would read as good news.
            return format!(
                "`{}` was supposed to fail and did not, so the problem is not \
                 reproduced yet. Write a check that fails for the reason \
                 described, and do not change the behaviour under test.\n",
                self.gate
            );
        }
        let mut out = String::from(
            "The project's checks did not pass. Fix the cause, do not disable the check.\n",
        );
        for c in self.commands.iter().filter(|c| !c.passed()) {
            out.push_str(&format!("\n$ {}\n", c.command));
            if c.timed_out {
                out.push_str("timed out\n");
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

/// The pull request a piece of work produced, and what it is waiting for.
///
/// Kept on the Work rather than looked up each time: the number and URL are
/// what a human needs to click, and they do not change. The status does, and is
/// refreshed by the poller.
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

/// Where a piece of work has got to in its declared chain of steps.
///
/// The cursor lives on the Work, not in the process running it: a pipeline that
/// survives a daemon restart is the whole point of declaring it, and a crash
/// between review and verify must resume at verify rather than re-run an
/// implement step that already cost money.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pipeline {
    /// The name of the pipeline in `devplane.toml`.
    pub name: String,
    /// Which step is running or about to, counting from zero.
    pub step: usize,
    /// The role of each step, in order. Copied from the config when the work
    /// starts so that editing `devplane.toml` mid-flight cannot renumber a
    /// chain that is already running.
    pub roles: Vec<String>,
    /// How many times each step has been entered. Bounds the review loop.
    pub entries: Vec<u32>,
    /// What the last reviewing step found, waiting to be handed back.
    #[serde(default)]
    pub findings: Option<String>,
}

impl Pipeline {
    pub fn new(name: String, roles: Vec<String>) -> Self {
        let entries = vec![0; roles.len()];
        Self {
            name,
            step: 0,
            roles,
            entries,
            findings: None,
        }
    }

    pub fn role(&self) -> Option<&str> {
        self.roles.get(self.step).map(String::as_str)
    }

    /// The index of a step by role name, for `back_to`.
    pub fn index_of(&self, role: &str) -> Option<usize> {
        self.roles.iter().position(|r| r == role)
    }

    /// Records that the current step has been entered, and says how many times
    /// it now has been.
    ///
    /// `None` when the cursor is past the end, which is the caller's bug rather
    /// than a count of zero — and a zero here would read as "never entered" and
    /// let a bounded loop run for ever.
    pub fn enter(&mut self) -> Option<u32> {
        let n = self.entries.get_mut(self.step)?;
        *n += 1;
        Some(*n)
    }

    /// How many times a step has been entered, by name.
    pub fn entries_for(&self, role: &str) -> u32 {
        self.index_of(role)
            .and_then(|i| self.entries.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// A one-line `implement › review › verify` with the current step marked,
    /// which is what the board shows.
    pub fn stepper(&self) -> String {
        self.roles
            .iter()
            .enumerate()
            .map(|(i, r)| match i.cmp(&self.step) {
                std::cmp::Ordering::Less => format!("✓{r}"),
                std::cmp::Ordering::Equal => format!("▸{r}"),
                std::cmp::Ordering::Greater => r.clone(),
            })
            .collect::<Vec<_>>()
            .join(" › ")
    }
}

/// A unit of work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work {
    pub id: WorkId,
    pub project_id: ProjectId,
    pub kind: WorkKind,
    pub phase: Phase,
    pub title: String,
    /// What the agent was asked for.
    pub prompt: String,
    /// The isolated checkout, when the work has one.
    /// The specification this work answers, relative to the project root.
    ///
    /// A path and nothing else: the project's own file, never opened here for
    /// meaning. It is what the gate stamps onto the done certificate.
    #[serde(default)]
    pub spec: Option<String>,
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    /// Runs that have worked on this, oldest first.
    pub runs: Vec<RunId>,
    /// What preparing the checkout did, when the project asked for it.
    ///
    /// Not a gate, and deliberately not in `gates`: it says nothing about
    /// whether the work is done. Keeping it there made `pnpm install` the
    /// "latest gate" on the board, and counted it as an attempt, so the first
    /// real check was attempt two.
    #[serde(default)]
    pub setup: Option<GateReport>,
    pub gates: Vec<GateReport>,
    /// How many times failures have been handed back to the agent.
    pub feedback_rounds: u32,
    /// Other work in this repository that is editing the same files, recorded
    /// the last time this work reached a checkpoint.
    ///
    /// Isolated checkouts stop two agents overwriting each other while they
    /// work, and do nothing about the failure at integration: two locally valid
    /// branches that cannot both land. Measured rather than admitted — nothing
    /// is declared in advance and nothing is refused.
    #[serde(default)]
    pub overlaps: Vec<Overlap>,
    /// What this work has cost so far, summed over its runs.
    ///
    /// Kept on the Work rather than recomputed: runs age off the board after a
    /// week and the bill does not stop being true.
    #[serde(default)]
    pub cost_usd: f64,
    /// Why the work stopped, written at the moment it stops.
    ///
    /// Always `Some` when the phase is `Failed` and `None` otherwise — the
    /// inbox reads this rather than guessing from the last gate report, which
    /// is how a passing gate came to be announced as the failure.
    #[serde(default)]
    pub stopped: Option<Stopped>,
    /// The pull request this work opened, once it has one.
    #[serde(default)]
    pub pull_request: Option<PullRequestRef>,
    /// The declared pipeline this work is running, if any.
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
    /// Hidden from the inbox until this moment.
    ///
    /// Work needs its own, separate from the run's: the items only Work can
    /// produce — a pull request gone red, a spent feedback budget, a pipeline
    /// held at a human step — outlive every session that worked on it, so there
    /// is frequently no run left to snooze. Without this the `snooze` action
    /// those items offer was a button that did nothing.
    #[serde(default)]
    pub snoozed: crate::core::attention::Snoozed,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Work {
    pub fn new(project_id: ProjectId, kind: WorkKind, title: String, prompt: String) -> Self {
        let now = Timestamp::now();
        Self {
            id: WorkId::new(format!("w-{}", uuid::Uuid::now_v7().simple())),
            project_id,
            kind,
            phase: Phase::Ready,
            title,
            prompt,
            spec: None,
            worktree: None,
            branch: None,
            runs: Vec::new(),
            setup: None,
            gates: Vec::new(),
            feedback_rounds: 0,
            overlaps: Vec::new(),
            cost_usd: 0.0,
            stopped: None,
            pull_request: None,
            pipeline: None,
            snoozed: Default::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Whether anything about this work is currently hidden.
    pub fn is_snoozed(&self) -> bool {
        self.snoozed.any()
    }

    /// The most recent gate verdict, which is what the board shows.
    /// Whether handing this back to the agent once more would actually do
    /// something — the *domain's* answer, so the API, the inbox and the CLI
    /// cannot disagree about which rows offer a Retry button. One definition of
    /// every derived value, served already judged.
    ///
    /// `can_drive` is the caller's: whether the session that wrote the code is
    /// still there. Nothing in here can see that.
    pub fn retryable(&self, can_drive: bool) -> bool {
        if self.phase != Phase::Failed || !can_drive {
            return false;
        }
        match &self.stopped {
            // Over budget until the ceiling is raised; `work retry` re-reads
            // the file and says so, so the button is offered and explains.
            Some(Stopped::OverBudget { .. }) => false,
            // Nothing an agent can fix.
            Some(Stopped::Broken { .. }) | None => false,
            // The findings are the thing to hand back, and they are on the row.
            Some(Stopped::ReviewExhausted { .. }) => true,
            // The failing lines come from the gate report, so there has to be
            // one.
            Some(Stopped::GateFailed { .. }) => self.last_gate().is_some(),
        }
    }

    /// Records the specification this work answers.
    ///
    /// **A file or a folder.** One document is the exception: Spec Kit writes
    /// `specs/NNN-name/`, Kiro `.kiro/specs/<feature>/`, OpenSpec
    /// `openspec/changes/<id>/`, and a flag that took only a file made a person
    /// pick one of the three or four documents the tool had just written.
    ///
    /// Refused unless it exists **inside the project**, for the reason
    /// `[workspace] include` is: the path arrives from a command line or a
    /// committed file, and a certificate naming `../../etc/passwd` is worse
    /// than one naming nothing. The check is the same containment rule, and a
    /// path that fails it is an error rather than a silently dropped field.
    pub fn with_spec(root: &std::path::Path, spec: &str) -> Result<String, String> {
        let joined = root.join(spec);
        if !crate::core::policy::within(root, &joined) {
            return Err(format!("{spec} is outside the project"));
        }
        if !joined.exists() {
            return Err(format!("{spec} is not in this project"));
        }
        // A folder with no Markdown in it is a path somebody mistyped, and
        // finding that out when the first gate runs costs an agent's turn.
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

    /// Whether the agent's own account of this work is worth showing.
    ///
    /// Only beside a **failed** gate. An end-of-task report references about
    /// one action in eleven and drifts toward the plan as the run leaves it, so
    /// on its own it is worse than nothing — it reads as a summary and is not
    /// one. Next to an exit code that contradicts it, it is the whole point.
    ///
    /// The product judges neither: no model separates a truthful trajectory
    /// report from an untruthful one better than a bag-of-words detector does,
    /// so both go on screen and the person reads.
    pub fn claim_is_worth_showing(&self) -> bool {
        self.last_gate().is_some_and(|g| !g.passed())
    }

    pub fn last_gate(&self) -> Option<&GateReport> {
        self.gates.last()
    }

    pub fn current_run(&self) -> Option<&RunId> {
        self.runs.last()
    }

    /// A filesystem-safe, readable slug for the branch and worktree.
    ///
    /// Derived from the title so that `git branch` and `ls .claude/worktrees`
    /// are readable months later; the id is appended so two items with the same
    /// title cannot collide.
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
        // The *tail* of a v7 uuid is its random part; the head is a
        // millisecond timestamp, so two items created in the same millisecond
        // would share a prefix — and then a branch name.
        let id = self.id.as_str();
        let short = &id[id.len().saturating_sub(6)..];
        if base.is_empty() {
            format!("{}-{short}", self.kind.branch_prefix())
        } else {
            format!("{base}-{short}")
        }
    }

    pub fn branch_name(&self) -> String {
        format!("{}/{}", self.kind.branch_prefix(), self.slug())
    }
}

#[cfg(test)]
mod tests {

    fn specdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-spec-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_agents_account_is_shown_beside_a_failed_gate_and_nowhere_else() {
        let mut w = Work::new(
            crate::core::ids::ProjectId::from_path(std::path::Path::new("/tmp/x")),
            WorkKind::Quick,
            "t".into(),
            "p".into(),
        );
        // No gate has run: there is nothing measured to read it against, and a
        // report on its own reads as a summary and is not one.
        assert!(!w.claim_is_worth_showing());

        let report = |ok: bool| GateReport {
            gate: "check".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            attempt: 1,
            expect_fail: false,
            spec: None,
            commands: vec![CommandResult {
                command: "cargo test".into(),
                exit_code: Some(if ok { 0 } else { 101 }),
                duration_ms: 1,
                output_tail: String::new(),
                failures: vec![],
                timed_out: false,
            }],
        };

        // Green: the account adds nothing the exit code has not already said.
        w.gates.push(report(true));
        assert!(!w.claim_is_worth_showing());

        // Red: this is the one place the two are worth reading together.
        w.gates.push(report(false));
        assert!(w.claim_is_worth_showing());
    }

    #[test]
    fn a_work_item_may_name_a_specification_inside_its_own_project() {
        let root = specdir("ok");
        std::fs::write(root.join("spec.md"), "# it must log in\n").unwrap();
        assert_eq!(Work::with_spec(&root, "spec.md").unwrap(), "spec.md");

        // And a folder, which is what every spec-driven framework in the field
        // actually produces — a flag that took only a file made a person pick
        // one of the four documents their tool had just written.
        let feature = root.join("specs/001-login");
        std::fs::create_dir_all(&feature).unwrap();
        std::fs::write(feature.join("spec.md"), "# Login\n").unwrap();
        std::fs::write(feature.join("tasks.md"), "- [ ] T001\n").unwrap();
        assert_eq!(
            Work::with_spec(&root, "specs/001-login").unwrap(),
            "specs/001-login"
        );

        // A folder with nothing to read in it is a mistyped path, and finding
        // that out when the first gate runs costs an agent's turn.
        std::fs::create_dir_all(root.join("specs/002-empty")).unwrap();
        assert!(Work::with_spec(&root, "specs/002-empty").is_err());
    }

    #[test]
    fn a_specification_outside_the_project_is_refused_rather_than_dropped() {
        // The path arrives from a command line or a committed file. A
        // certificate naming `../../etc/passwd` is worse than one naming
        // nothing, and a silently dropped field is worse than both.
        let root = specdir("escape");
        std::fs::write(root.join("spec.md"), "x").unwrap();
        assert!(Work::with_spec(&root, "../outside.md").is_err());
        assert!(Work::with_spec(&root, "/etc/passwd").is_err());
        std::fs::create_dir_all(root.join("specs")).unwrap();
        // An empty folder and an absent path: both refused at the point the
        // person can still fix the typo.
        assert!(Work::with_spec(&root, "specs").is_err());
        assert!(Work::with_spec(&root, "nope.md").is_err());
    }

    #[test]
    fn the_stamp_records_what_the_specification_was_and_says_when_it_was_not_there() {
        let root = specdir("stamp");
        std::fs::write(root.join("spec.md"), "# One\n\n- [x] a\n- [ ] b\n").unwrap();
        let first = SpecStamp::of("spec.md", &root, &[]);
        assert_eq!(first.path, "spec.md");
        assert_eq!(first.files, 1);
        // What the boxes said when the gate ran — the figure an agent's own
        // account of the same work cannot move.
        assert_eq!(first.tasks(), Some((1, 2)));
        assert!(first.has_unticked_work());
        let a = first
            .fingerprint
            .clone()
            .expect("a present file is fingerprinted");

        // The whole point: the same path after the file moves is a different
        // stamp, so a certificate cannot quietly refer to different content.
        std::fs::write(root.join("spec.md"), "# One\n\n- [x] a\n- [x] b\n").unwrap();
        let second = SpecStamp::of("spec.md", &root, &[]);
        assert_ne!(Some(a), second.fingerprint, "a ticked box is a change");
        assert_eq!(second.tasks(), Some((2, 2)));
        assert!(!second.has_unticked_work());

        // A work that names a specification which is not there is a finding,
        // not a blank: the path is kept and the fingerprint is absent.
        std::fs::remove_file(root.join("spec.md")).unwrap();
        let gone = SpecStamp::of("spec.md", &root, &[]);
        assert_eq!(gone.path, "spec.md");
        assert!(gone.fingerprint.is_none());
        // And no task list is not the same as no progress.
        assert_eq!(gone.tasks(), None);
    }
    use super::*;

    fn work(title: &str) -> Work {
        Work::new(
            ProjectId::new("/repo"),
            WorkKind::Bug,
            title.into(),
            "fix it".into(),
        )
    }

    #[test]
    fn a_reproduction_that_passes_has_reproduced_nothing() {
        // The one gate whose success is a failure: if the check that
        // demonstrates a bug passes, the bug has not been demonstrated, and
        // letting that count as green would wave the whole class of "fixed it
        // by not testing it" straight through.
        let ok = CommandResult {
            command: "cargo test --test repro".into(),
            exit_code: Some(0),
            duration_ms: 1,
            output_tail: String::new(),
            failures: vec![],
            timed_out: false,
        };
        let bad = CommandResult {
            exit_code: Some(101),
            ..ok.clone()
        };
        let repro = |commands| GateReport {
            gate: "repro".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            attempt: 1,
            expect_fail: true,
            commands,
            spec: None,
        };
        assert!(repro(vec![bad.clone()]).passed(), "failing is the point");
        assert!(!repro(vec![ok.clone()]).passed());

        // **And a command that never finished has reproduced nothing either.**
        // `!passed()` was the test, and it is true of a command that timed
        // out, one the shell could not start, and one a signal killed — so a
        // hung test suite reported "reproduced the problem" and the fix step
        // began against a demonstration nobody had seen. A reproduction is a
        // command that *ran and said no*.
        let hung = CommandResult {
            exit_code: None,
            timed_out: true,
            ..ok.clone()
        };
        let unstartable = CommandResult {
            exit_code: None,
            timed_out: false,
            ..ok.clone()
        };
        assert!(
            !repro(vec![hung.clone()]).passed(),
            "a timeout is not a reproduction"
        );
        assert!(
            !repro(vec![unstartable.clone()]).passed(),
            "a command that could not run is not a reproduction"
        );
        assert!(
            !repro(vec![bad.clone(), hung.clone()]).passed(),
            "and one timeout anywhere makes the whole gate inconclusive, because \
             a repro gate runs every command and cannot say which mattered"
        );

        // Each says which of the two things happened, because "write a check
        // that fails" is the wrong instruction for a check that hung.
        assert!(
            repro(vec![hung.clone()]).summary().contains("timed out"),
            "{}",
            repro(vec![hung.clone()]).summary()
        );
        assert!(
            repro(vec![hung]).feedback().contains("did not finish"),
            "a hung reproduction must not be told it passed when it should fail"
        );
        assert!(repro(vec![unstartable]).summary().contains("could not run"),);
        assert!(
            repro(vec![ok.clone()])
                .feedback()
                .contains("was supposed to fail"),
            "and the ordinary case keeps its message"
        );
        assert!(!repro(vec![]).passed(), "and an empty one proves nothing");

        assert!(
            repro(vec![ok.clone()])
                .feedback()
                .contains("was supposed to fail"),
            "the agent must be told what actually went wrong"
        );
        assert!(
            !repro(vec![ok]).feedback().contains("do not disable"),
            "and not handed advice for the opposite problem"
        );
        assert!(
            repro(vec![bad])
                .summary()
                .contains("reproduced the problem")
        );
    }

    #[test]
    fn the_stepper_says_where_a_pipeline_is() {
        let mut p = Pipeline::new(
            "feature".into(),
            vec!["implement".into(), "review".into(), "merge".into()],
        );
        p.step = 1;
        assert_eq!(p.stepper(), "✓implement › ▸review › merge");
        assert_eq!(p.role(), Some("review"));
        assert_eq!(p.index_of("implement"), Some(0));
        assert_eq!(p.index_of("deploy"), None);
    }

    #[test]
    fn entering_a_step_counts_that_step_only() {
        // The count is what bounds a review loop, so counting the wrong step
        // would either cut a chain short or let it run forever.
        let mut p = Pipeline::new("feature".into(), vec!["implement".into(), "review".into()]);
        assert_eq!(p.enter(), Some(1));
        p.step = 1;
        assert_eq!(p.enter(), Some(1));
        p.step = 0;
        assert_eq!(p.enter(), Some(2));
        assert_eq!(p.entries, vec![2, 1]);
        assert_eq!(p.entries_for("review"), 1);
        assert_eq!(p.entries_for("nobody"), 0);
        // Off the end is the caller's bug, not a count of zero: zero reads as
        // "never entered", which is how a bounded loop stops being bounded.
        p.step = 9;
        assert_eq!(p.enter(), None);
    }

    #[test]
    fn a_slug_is_readable_and_unique() {
        let w = work("Fix the flaky login test on CI");
        let slug = w.slug();
        assert!(slug.starts_with("fix-the-flaky-login-test"), "{slug}");
        assert_eq!(w.branch_name(), format!("fix/{slug}"));
        // Two items with the same title must not collide on disk.
        assert_ne!(work("Fix the flaky login test on CI").slug(), slug);
    }

    #[test]
    fn a_title_of_punctuation_still_produces_a_usable_name() {
        let w = work("!!! ???");
        assert!(!w.slug().contains(' '));
        assert!(w.slug().starts_with("fix-"));
    }

    #[test]
    fn a_gate_passes_only_when_every_command_does() {
        let ok = CommandResult {
            command: "cargo test".into(),
            exit_code: Some(0),
            duration_ms: 10,
            output_tail: String::new(),
            failures: vec![],
            timed_out: false,
        };
        let bad = CommandResult {
            exit_code: Some(101),
            failures: vec!["test auth::login ... FAILED".into()],
            ..ok.clone()
        };
        let report = |cmds: Vec<CommandResult>| GateReport {
            gate: "check".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            commands: cmds,
            attempt: 1,
            expect_fail: false,
            spec: None,
        };
        assert!(report(vec![ok.clone()]).passed());
        assert!(!report(vec![ok.clone(), bad.clone()]).passed());
        // An empty gate is not a passing gate: a Definition of Done with no
        // checks in it proves nothing, and must not read as success.
        assert!(!report(vec![]).passed());
    }

    #[test]
    fn feedback_names_the_failures_not_the_whole_log() {
        let report = GateReport {
            gate: "check".into(),
            at: Timestamp::now(),
            duration_ms: 1,
            attempt: 1,
            expect_fail: false,
            spec: None,
            commands: vec![CommandResult {
                command: "cargo test".into(),
                exit_code: Some(101),
                duration_ms: 1,
                output_tail: "a hundred lines of noise".repeat(50),
                failures: vec!["test auth::login ... FAILED".into()],
                timed_out: false,
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
            expect_fail: false,
            spec: None,
            commands: vec![CommandResult {
                command: "cargo test".into(),
                exit_code: None,
                duration_ms: 600_000,
                output_tail: String::new(),
                failures: vec![],
                timed_out: true,
            }],
        };
        assert!(!report.passed());
        assert!(report.summary().contains("timed out"));
    }
}

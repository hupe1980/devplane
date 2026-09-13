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

use crate::ids::{ProjectId, RunId, WorkId};
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
}

impl GateReport {
    pub fn passed(&self) -> bool {
        // An empty gate is never a pass. A definition of done with no commands
        // in it proves nothing, and neither does a reproduction with none.
        if self.commands.is_empty() {
            return false;
        }
        if self.expect_fail {
            return self.commands.iter().any(|c| !c.passed());
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

/// Where a piece of work has got to in its declared chain of steps (D34).
///
/// The cursor lives on the Work, not in the process running it: a pipeline that
/// survives a daemon restart is the whole point of declaring it, and a crash
/// between review and verify must resume at verify rather than re-run an
/// implement step that already cost money.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pipeline {
    /// The name of the pipeline in `vibeplane.toml`.
    pub name: String,
    /// Which step is running or about to, counting from zero.
    pub step: usize,
    /// The role of each step, in order. Copied from the config when the work
    /// starts so that editing `vibeplane.toml` mid-flight cannot renumber a
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
    pub fn enter(&mut self) -> u32 {
        if let Some(n) = self.entries.get_mut(self.step) {
            *n += 1;
            return *n;
        }
        0
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
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    /// Runs that have worked on this, oldest first.
    pub runs: Vec<RunId>,
    pub gates: Vec<GateReport>,
    /// How many times failures have been handed back to the agent.
    pub feedback_rounds: u32,
    /// The pull request this work opened, once it has one.
    #[serde(default)]
    pub pull_request: Option<PullRequestRef>,
    /// The declared pipeline this work is running, if any.
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
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
            worktree: None,
            branch: None,
            runs: Vec::new(),
            gates: Vec::new(),
            feedback_rounds: 0,
            pull_request: None,
            pipeline: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// The most recent gate verdict, which is what the board shows.
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
        };
        assert!(repro(vec![bad.clone()]).passed(), "failing is the point");
        assert!(!repro(vec![ok.clone()]).passed());
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
        assert_eq!(p.enter(), 1);
        p.step = 1;
        assert_eq!(p.enter(), 1);
        p.step = 0;
        assert_eq!(p.enter(), 2);
        assert_eq!(p.entries, vec![2, 1]);
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

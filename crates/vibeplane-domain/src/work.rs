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
}

impl GateReport {
    pub fn passed(&self) -> bool {
        !self.commands.is_empty() && self.commands.iter().all(CommandResult::passed)
    }

    /// A short summary a human can act on, and an agent can be told.
    pub fn summary(&self) -> String {
        if self.passed() {
            return format!("{} passed", self.gate);
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

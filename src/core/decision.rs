//! The decision log: what Vibeplane decided, and on whose authority.
//!
//! Two questions have to be answerable months later, and neither is answerable
//! from the event log. *Why did this command run without anybody being asked?*
//! — because a rule in that repository allowed it, and the rule is named. *Why
//! is there a pull request on this branch?* — because these checks passed, and
//! they are named too.
//!
//! Observations are things that happened to Vibeplane. A decision is something
//! Vibeplane *did*, or allowed, and the difference matters: an observation can
//! be re-derived from the provider, a decision cannot be re-derived from
//! anything. It is appended, never edited, and pruning the event log leaves it
//! alone.
//!
//! What this deliberately is not: a hash-chained tamper-evident journal. This
//! is a developer tool on a laptop, and the threat it defends against is *"I
//! cannot remember why that happened"*, not an adversary with write access to
//! the same machine as the agents themselves.

use crate::core::ids::{ProjectId, RunId, WorkId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Who decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    /// A rule in a `vibeplane.toml` or the machine-wide policy.
    Policy,
    /// A person, through the inbox, the board or the CLI.
    Human,
    /// Vibeplane itself, following a rule the project wrote down — a gate
    /// verdict, a pipeline advancing, a pull request opening.
    Daemon,
}

impl Actor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Actor::Policy => "policy",
            Actor::Human => "human",
            Actor::Daemon => "daemon",
        }
    }
}

/// One thing that was decided.
///
/// `action` is the verb, in the same vocabulary the policy speaks:
/// `agent:tool.use`, `gate:run`, `git:push`, `gh:pr.create`, `work:advance`.
/// `subject` is what it was about — the command, the gate, the branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub at: Timestamp,
    pub actor: Actor,
    pub action: String,
    pub subject: String,
    /// `allow`, `deny`, `pass`, `fail`, `done`.
    pub outcome: String,
    /// The rule that decided, or the reason. This is the field the whole table
    /// exists for: "auto-approved" is not an answer, "auto-approved by
    /// `Bash(pnpm test *)`" is.
    pub reason: Option<String>,
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    #[serde(default)]
    pub run_id: Option<RunId>,
    #[serde(default)]
    pub work_id: Option<WorkId>,
}

impl Decision {
    pub fn new(actor: Actor, action: &str, subject: impl Into<String>, outcome: &str) -> Self {
        Self {
            id: crate::core::ids::new_event_id(),
            at: Timestamp::now(),
            actor,
            action: action.to_string(),
            subject: subject.into(),
            outcome: outcome.to_string(),
            reason: None,
            project_id: None,
            run_id: None,
            work_id: None,
        }
    }

    pub fn because(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn for_run(mut self, run: &RunId) -> Self {
        self.run_id = Some(run.clone());
        self
    }

    pub fn for_work(mut self, work: &WorkId) -> Self {
        self.work_id = Some(work.clone());
        self
    }

    pub fn in_project(mut self, project: Option<ProjectId>) -> Self {
        self.project_id = project;
        self
    }

    /// One line, the way `vibeplane audit` prints it.
    pub fn line(&self) -> String {
        let reason = match &self.reason {
            Some(r) => format!(" — {r}"),
            None => String::new(),
        };
        format!(
            "{} {} {} {}{}",
            self.actor.as_str(),
            self.outcome,
            self.action,
            crate::core::text::clip(&self.subject, 60),
            reason
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decision_says_who_decided_and_on_what_authority() {
        // "auto-approved" is not an answer to "why did this run".
        let d = Decision::new(
            Actor::Policy,
            "agent:tool.use",
            "pnpm test -- --run",
            "allow",
        )
        .because("Bash(pnpm test *)")
        .for_run(&RunId::new("s1"));
        let line = d.line();
        assert!(line.contains("policy allow agent:tool.use"));
        assert!(line.contains("pnpm test"));
        assert!(line.contains("Bash(pnpm test *)"));
        assert_eq!(d.run_id, Some(RunId::new("s1")));
    }

    #[test]
    fn a_long_subject_is_cut_rather_than_wrapped() {
        let d = Decision::new(Actor::Daemon, "gate:run", "x".repeat(500), "pass");
        assert!(d.line().chars().count() < 120);
    }
}

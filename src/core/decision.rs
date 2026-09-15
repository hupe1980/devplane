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

    /// Overrides the timestamp, for a decision recorded later than it was
    /// taken — the spool the `command` hook writes when no daemon is running.
    pub fn at(mut self, at: Timestamp) -> Self {
        self.at = at;
        self
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

/// What the `command` hook hands back after it has enforced a verdict.
///
/// One shape for two journeys. Posted to `/vibeplane/decided` when a daemon is
/// listening, and appended to `~/.vibeplane/pending-decisions.jsonl` when one
/// is not — so a decision taken while the daemon was down is written down by
/// exactly the code that writes down every other decision, rather than by a
/// second path nobody exercises.
///
/// It carries the **verdict**, never the inputs to recompute one. The process
/// that enforced a rule is the authority for which rule it was; the daemon's
/// cached rules can be seconds behind the file the hook just read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecidedEnvelope {
    pub session: String,
    /// `allow`, `deny`, `ask` or `undecided`.
    pub verdict: String,
    /// The rule that produced it. `None` means no rule did — which is not a
    /// decision and is therefore not recorded, only observed.
    #[serde(default)]
    pub rule: Option<String>,
    /// What was asked for, in the words the audit log prints.
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub tool: String,
    /// When it was taken, which is not when it was filed.
    #[serde(default)]
    pub at: Option<Timestamp>,
    /// Set on a row that waited in the spool, so the log can say so rather than
    /// leaving a reader to wonder why the timestamps run backwards.
    #[serde(default)]
    pub late: bool,
    /// Whether the session is now waiting on a person.
    ///
    /// True for a `PermissionRequest` no rule answered: Claude Code is showing
    /// its own dialog and the run is blocked, which is how the inbox learns
    /// about it the instant it happens rather than six seconds later on a
    /// notification. False for `PreToolUse`, which fires on every call and
    /// says nothing about whether anybody is going to be asked.
    #[serde(default)]
    pub blocked: bool,
    /// The raw hook payload, for the observation half.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

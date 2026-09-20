//! The decision log: what Devplane decided, and on whose authority.
//!
//! Two questions have to be answerable months later, and neither is answerable
//! from the event log. *Why did this command run without anybody being asked?*
//! — because a rule in that repository allowed it, and the rule is named. *Why
//! is there a pull request on this branch?* — because these checks passed, and
//! they are named too.
//!
//! Observations are things that happened to Devplane. A decision is something
//! Devplane *did*, or allowed, and the difference matters: an observation can
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

/// **On whose authority something happened.**
///
/// This is the column the product is named for, and for five passes it could
/// not say what the design required. It had three values — `policy`, `human`,
/// `daemon` — and *daemon* was doing three unrelated jobs at once: Devplane
/// running a gate, a clock refusing a call nobody answered, and a question
/// dying unanswered when its run ended. The reason string told those apart and
/// the queryable column did not, in the one table whose whole purpose is being
/// queried by exactly that dimension.
///
/// # Why these five and not six
///
/// `classifier` is deliberately absent, and its absence is a measurement
/// rather than an oversight. Devplane has no channel that attributes an
/// individual call to a model's approval: across four concurrent sessions,
/// `PreToolUse` fired forty-nine times and `PermissionRequest` fired **zero**
/// times, so a call in an `auto` session is one nobody was asked about — which
/// is a fact about the *session's mode*, reported by `devplane modes`, and not
/// a fact about the call. A variant nothing can produce is a documented
/// trigger with no code behind it.
///
/// `unknown` is absent for the stronger reason: a row whose authority cannot be
/// established is not a row with another kind of authority, it is **a row
/// Devplane must not write**. A ledger that guesses is worse than one with
/// gaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum Authority {
    /// Somebody was asked and answered — through the inbox, the board or the
    /// CLI. The only value that needs no defence.
    Person,
    /// A rule matched: a `devplane.toml` prohibition or the machine-wide
    /// policy. Deterministic and re-derivable, and the rule text is in
    /// `reason`, which is the whole reason that field is not optional in
    /// practice.
    Rule,
    /// A clock decided, because nobody answered in time. **Distinct from
    /// `Daemon` on purpose**: Devplane ran the timer, but the *decision* is
    /// that time ran out, and a person auditing the week needs to find these
    /// without reading prose. The duration belongs in `reason`.
    Timer,
    /// Asked, never answered, and the moment passed — the run ended under the
    /// question, or the call was cancelled beneath it. **Nobody decided.**
    ///
    /// This is the value the whole product exists to be able to write. Devin
    /// ships the word *skipped* for it; nothing else records it at all.
    Nobody,
    /// Devplane itself, mechanically, carrying out something the project wrote
    /// down: a gate verdict, a pipeline advancing, a pull request opening.
    ///
    /// **Not a decision taken on the person's behalf** — it is the tool doing
    /// the job it was configured to do, and it is separated from `Rule` and
    /// `Timer` so that filtering for *what was decided for me* does not return
    /// every gate this machine has ever run.
    Daemon,
}

impl Authority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Authority::Person => "person",
            Authority::Rule => "rule",
            Authority::Timer => "timer",
            Authority::Nobody => "nobody",
            Authority::Daemon => "daemon",
        }
    }

    /// Parses the stored spelling. Unknown text is an error rather than a
    /// default: silently reading an unrecognised authority as `daemon` would
    /// put the most reassuring label on the least known row, which is the
    /// failure this enum was rebuilt to stop.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "person" => Authority::Person,
            "rule" => Authority::Rule,
            "timer" => Authority::Timer,
            "nobody" => Authority::Nobody,
            "daemon" => Authority::Daemon,
            _ => return None,
        })
    }

    /// Whether this row is something that was decided **instead of** the
    /// person — the filter the seat's surfaces default to.
    ///
    /// `Daemon` is excluded because it is the tool doing what it was told;
    /// `Person` is excluded because those are the ones you remember.
    pub fn was_taken_for_you(self) -> bool {
        matches!(self, Authority::Rule | Authority::Timer | Authority::Nobody)
    }
}

/// One thing that was decided.
///
/// `action` is the verb, in the same vocabulary the policy speaks:
/// `agent:tool.use`, `gate:run`, `git:push`, `gh:pr.create`, `work:advance`.
/// `subject` is what it was about — the command, the gate, the branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Decision {
    pub id: String,
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub at: Timestamp,
    pub authority: Authority,
    pub action: String,
    pub subject: String,
    /// `allow`, `deny`, `pass`, `fail`, `done`.
    pub outcome: String,
    /// The rule that decided, or the reason. This is the field the whole table
    /// exists for: "auto-approved" is not an answer, "auto-approved by
    /// `Bash(pnpm test *)`" is.
    pub reason: Option<String>,
    /// The tool this call was, where it was a tool call at all.
    ///
    /// `Bash`, `Read`, `Edit`, `mcp__…`. Empty for a gate verdict, a push or a
    /// pipeline advancing, which are decisions about something other than a
    /// tool.
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    #[serde(default)]
    pub run_id: Option<RunId>,
    #[serde(default)]
    pub work_id: Option<WorkId>,
}

impl Decision {
    pub fn new(
        authority: Authority,
        action: &str,
        subject: impl Into<String>,
        outcome: &str,
    ) -> Self {
        Self {
            id: crate::core::ids::new_event_id(),
            at: Timestamp::now(),
            authority,
            action: action.to_string(),
            subject: subject.into(),
            outcome: outcome.to_string(),
            reason: None,
            tool: None,
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

    /// Names the tool this decision was about.
    pub fn by_tool(mut self, tool: impl Into<String>) -> Self {
        let tool = tool.into();
        if !tool.is_empty() {
            self.tool = Some(tool);
        }
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

    /// One line, the way `devplane audit` prints it.
    pub fn line(&self) -> String {
        let reason = match &self.reason {
            Some(r) => format!(" — {r}"),
            None => String::new(),
        };
        format!(
            "{} {} {} {}{}",
            self.authority.as_str(),
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
            Authority::Rule,
            "agent:tool.use",
            "pnpm test -- --run",
            "allow",
        )
        .because("Bash(pnpm test *)")
        .for_run(&RunId::new("s1"));
        let line = d.line();
        assert!(line.contains("rule allow agent:tool.use"), "{line}");
        assert!(line.contains("pnpm test"));
        assert!(line.contains("Bash(pnpm test *)"));
        assert_eq!(d.run_id, Some(RunId::new("s1")));
    }

    #[test]
    fn the_three_things_daemon_used_to_mean_are_now_three_values() {
        // The defect this enum was rebuilt for. All three of these were
        // `Actor::Daemon`, distinguishable only by reading English prose, in
        // the one table whose entire purpose is being queried by this column.
        let gate = Decision::new(Authority::Daemon, "gate:run", "cargo test", "pass");
        let clock = Decision::new(Authority::Timer, "agent:tool.use", "req-1", "deny")
            .because("nobody answered within ten minutes");
        let lost = Decision::new(Authority::Nobody, "agent:question", "req-2", "unanswered")
            .because("the run ended before anybody answered");

        for (a, b) in [(&gate, &clock), (&gate, &lost), (&clock, &lost)] {
            assert_ne!(
                a.authority, b.authority,
                "these were the same value until 2026-09-19"
            );
        }

        // And the filter the seat's surfaces default to: what was decided
        // *instead of* the person. A gate the project asked for is not that.
        assert!(!Authority::Daemon.was_taken_for_you());
        assert!(!Authority::Person.was_taken_for_you());
        for a in [Authority::Rule, Authority::Timer, Authority::Nobody] {
            assert!(a.was_taken_for_you(), "{a:?}");
        }
    }

    #[test]
    fn an_unknown_authority_is_refused_rather_than_read_as_daemon() {
        // Reading an unrecognised value as `daemon` would put the most
        // reassuring label on the least known row. Every spelling this build
        // writes must round-trip; anything else is `None` and the row is
        // dropped by the reader.
        for a in [
            Authority::Person,
            Authority::Rule,
            Authority::Timer,
            Authority::Nobody,
            Authority::Daemon,
        ] {
            assert_eq!(Authority::parse(a.as_str()), Some(a), "{a:?}");
        }
        for unknown in ["policy", "human", "classifier", "unknown", ""] {
            assert_eq!(Authority::parse(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn a_long_subject_is_cut_rather_than_wrapped() {
        let d = Decision::new(Authority::Daemon, "gate:run", "x".repeat(500), "pass");
        assert!(d.line().chars().count() < 120);
    }
}

/// What the `command` hook hands back after it has enforced a verdict.
///
/// One shape for two journeys. Posted to `/devplane/decided` when a daemon is
/// listening, and appended to `~/.devplane/pending-decisions.jsonl` when one
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

/// Files a **shell** call in these decisions named for writing.
///
/// The one class Claude Code's own checkpointing documents that it does not
/// cover: *"files modified by bash commands are not tracked."* Everything
/// needed to answer it is already here — every tool call the gate saw, the
/// command text, and the tool it was — so this is a query rather than a
/// feature, with no snapshots and no second copy of anybody's files.
///
/// Three things it is careful about, and each is a way the answer could be a
/// confident lie:
///
/// * **A refused call wrote nothing.** A `deny` row is a call that did not
///   happen, so it is not in the list.
/// * **A path nothing can pin is not a filename.** A glob, a `~` or a variable
///   is left out rather than printed as though it named a file.
/// * **The gate saw the call, not the write.** `PreToolUse` fires *before* the
///   tool, so the honest sentence is *named for writing*, never *changed*.
///   Everything downstream of this function has to keep that wording.
pub fn files_a_shell_call_named_for_writing(decisions: &[Decision]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in decisions {
        if d.action != "agent:tool.use" || d.outcome == "deny" {
            continue;
        }
        let Some(tool) = d.tool.as_deref() else {
            continue;
        };
        if !crate::core::policy::is_shell(tool) {
            continue;
        }
        for t in crate::core::command::file_targets(&d.subject).list.iter() {
            if t.access == crate::core::command::Access::Write
                && !t.unresolvable
                && !out.iter().any(|p| p == &t.path)
            {
                out.push(t.path.clone());
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod rewind_tests {
    use super::*;
    use crate::core::ids::RunId;

    fn call(tool: &str, subject: &str, outcome: &str) -> Decision {
        Decision::new(Authority::Rule, "agent:tool.use", subject, outcome)
            .by_tool(tool)
            .for_run(&RunId::new("s1"))
    }

    #[test]
    fn a_shell_redirect_is_outside_the_vendors_checkpoint_and_an_edit_is_not() {
        let rows = vec![
            call("Bash", "echo x > notes.md", "allow"),
            // The vendor's own editing tools *are* checkpointed, so naming
            // them here would tell somebody to worry about a file that will
            // come back.
            call("Edit", "src/main.rs", "allow"),
        ];
        assert_eq!(
            files_a_shell_call_named_for_writing(&rows),
            vec!["notes.md".to_string()]
        );
    }

    #[test]
    fn a_refused_call_wrote_nothing_so_it_is_not_listed() {
        let rows = vec![call("Bash", "echo x > .env", "deny")];
        assert!(files_a_shell_call_named_for_writing(&rows).is_empty());
    }

    #[test]
    fn a_path_nothing_can_pin_is_not_printed_as_a_filename() {
        // A glob, a `~` or a variable names no single file. Printing one as
        // though it did would be a confident answer whose evidence does not
        // support it.
        let rows = vec![
            call("Bash", "echo x > $OUT", "allow"),
            call("Bash", "echo x > ~/notes.md", "allow"),
            call("Bash", "echo x > build/*.log", "allow"),
        ];
        assert!(files_a_shell_call_named_for_writing(&rows).is_empty());
    }

    #[test]
    fn a_decision_that_is_not_a_tool_call_is_ignored() {
        let rows = vec![Decision::new(
            Authority::Daemon,
            "gh:pr.create",
            "fix/login",
            "done",
        )];
        assert!(files_a_shell_call_named_for_writing(&rows).is_empty());
    }

    #[test]
    fn the_same_file_written_twice_is_named_once() {
        let rows = vec![
            call("Bash", "echo a > notes.md", "allow"),
            call("Bash", "echo b >> notes.md", "allow"),
        ];
        assert_eq!(files_a_shell_call_named_for_writing(&rows).len(), 1);
    }
}

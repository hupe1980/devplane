//! The decision log: what Devplane decided or allowed, and on whose authority —
//! the rule that allowed a command, the checks that opened a pull request.
//! Unlike observations, a decision cannot be re-derived, so it is append-only
//! and survives event-log pruning. Not a tamper-evident journal: the threat is
//! *"I cannot remember why that happened"*, not an adversary on the same machine.

use crate::core::ids::{ChangeId, ProjectId, RunId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// On whose authority something happened. There is no `classifier` (no channel
/// attributes a single call to a model's approval; an `auto` session is a mode,
/// reported by `devplane modes`) and no `unknown`: a row whose authority cannot
/// be established must not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum Authority {
    /// Somebody was asked and answered — inbox, board or CLI.
    Person,
    /// A rule matched: a `devplane.toml` prohibition or the machine-wide
    /// policy. The rule text goes in `reason`.
    Rule,
    /// Nobody answered in time and a clock decided; the duration goes in
    /// `reason`. Distinct from `Devplane` so these are findable without prose.
    Timer,
    /// Asked, never answered, and the moment passed — the run ended or the
    /// call was cancelled. Nobody decided.
    Nobody,
    /// Devplane mechanically carrying out what the project wrote down (a gate
    /// verdict, a pull request). Not a decision taken on the person's behalf,
    /// so *what was decided for me* excludes it.
    Devplane,
}

impl Authority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Authority::Person => "person",
            Authority::Rule => "rule",
            Authority::Timer => "timer",
            Authority::Nobody => "nobody",
            Authority::Devplane => "devplane",
        }
    }

    /// Parses the stored spelling. Unknown text is `None`, never a default:
    /// guessing would put the most reassuring label on the least known row.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "person" => Authority::Person,
            "rule" => Authority::Rule,
            "timer" => Authority::Timer,
            "nobody" => Authority::Nobody,
            "devplane" => Authority::Devplane,
            _ => return None,
        })
    }

    /// Whether this was decided instead of the person — the default filter of
    /// the seat's surfaces. Excludes `Devplane` (doing what it was told) and `Person`.
    pub fn was_taken_for_you(self) -> bool {
        matches!(self, Authority::Rule | Authority::Timer | Authority::Nobody)
    }
}

/// One thing that was decided. `action` is the verb in the policy's vocabulary
/// (`agent:tool.use`, `gate:run`, `git:push`, `gh:pr.create`, `change:stop`);
/// `subject` is what it was about.
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
    /// The rule that decided, or the reason: "auto-approved by
    /// `Bash(pnpm test *)`", not "auto-approved".
    pub reason: Option<String>,
    /// The tool, for a tool call (`Bash`, `Edit`, `mcp__…`); `None` otherwise.
    #[serde(default)]
    pub tool: Option<String>,
    /// Where the MCP server behind `tool` came from, as the vendor reported it:
    /// *what was acting, and who put it there*, beside `authority`. `None` for
    /// non-MCP tools and vendors or versions that do not report it — absent is
    /// not unknown.
    #[serde(default)]
    pub server_source: Option<String>,
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    #[serde(default)]
    pub run_id: Option<RunId>,
    #[serde(default)]
    pub change_id: Option<ChangeId>,
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
            server_source: None,
            project_id: None,
            run_id: None,
            change_id: None,
        }
    }

    /// Overrides the timestamp, for a decision filed later than it was taken
    /// (spooled by the `command` hook when the store would not open).
    pub fn at(mut self, at: Timestamp) -> Self {
        self.at = at;
        self
    }

    /// Records where the MCP server behind this call came from — transcribed
    /// from the vendor, never mapped; unrecognised values are kept as received.
    #[must_use]
    pub fn from_server(mut self, source: Option<String>) -> Self {
        self.server_source = source.filter(|s| !s.is_empty());
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

    pub fn for_change(mut self, change: &ChangeId) -> Self {
        self.change_id = Some(change.clone());
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
    fn the_three_things_one_value_used_to_mean_are_now_three_values() {
        let gate = Decision::new(Authority::Devplane, "gate:run", "cargo test", "pass");
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

        // What was decided instead of the person; a gate is not that.
        assert!(!Authority::Devplane.was_taken_for_you());
        assert!(!Authority::Person.was_taken_for_you());
        for a in [Authority::Rule, Authority::Timer, Authority::Nobody] {
            assert!(a.was_taken_for_you(), "{a:?}");
        }
    }

    #[test]
    fn an_unknown_authority_is_refused_rather_than_read_as_devplane() {
        // Every written spelling round-trips; anything else is `None`.
        for a in [
            Authority::Person,
            Authority::Rule,
            Authority::Timer,
            Authority::Nobody,
            Authority::Devplane,
        ] {
            assert_eq!(Authority::parse(a.as_str()), Some(a), "{a:?}");
        }
        for unknown in ["policy", "human", "classifier", "unknown", ""] {
            assert_eq!(Authority::parse(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn a_long_subject_is_cut_rather_than_wrapped() {
        let d = Decision::new(Authority::Devplane, "gate:run", "x".repeat(500), "pass");
        assert!(d.line().chars().count() < 120);
    }
}

/// What the `command` hook hands back after enforcing a verdict. Written to the
/// store, or appended to `~/.devplane/pending-decisions.jsonl` when the store
/// will not open, and filed by the same code either way. Carries the verdict,
/// not inputs to recompute one: the enforcing process is the authority for
/// which rule applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecidedEnvelope {
    pub session: String,
    #[serde(default)]
    pub source: crate::core::event::Source,
    /// `deny`, `ask`, `unresolved` or `undecided` — never `allow`, which
    /// [`crate::core::Verdict`] cannot express.
    pub verdict: String,
    /// The rule that produced it. `None` for `undecided` (observed only) and
    /// `unresolved` (see [`Self::why`]).
    #[serde(default)]
    pub rule: Option<String>,
    /// Where the MCP server behind the call came from, as the vendor reported it.
    #[serde(default)]
    pub server_source: Option<String>,
    /// Why the matcher could not decide, for an `unresolved` verdict — the only
    /// accounting for a decision with no rule behind it.
    #[serde(default)]
    pub why: Option<String>,
    /// What was asked for, in the words the audit log prints.
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub tool: String,
    /// When it was taken, not when it was filed.
    #[serde(default)]
    pub at: Option<Timestamp>,
    /// Set on a row that waited in the spool, so the log can explain backwards timestamps.
    #[serde(default)]
    pub late: bool,
    /// Whether the session is now waiting on a person: true for a
    /// `PermissionRequest` no rule answered (the vendor's dialog is up), false
    /// for `PreToolUse`, which fires on every call.
    #[serde(default)]
    pub blocked: bool,
    /// The raw hook payload, for the observation half.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

/// Files a shell call in these decisions named for writing — the class Claude
/// Code's checkpointing does not cover. Denied calls are skipped, paths that
/// pin no file (globs, `~`, variables) are left out, and because `PreToolUse`
/// fires before the tool, the wording must stay *named for writing*, never *changed*.
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
            // The vendor's editing tools are checkpointed, so they are not listed.
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
            Authority::Devplane,
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

/// Whether a tool call could carry server provenance (`mcp_server.source`) at
/// all. Lets a surface tell *not an MCP tool* from *MCP, but the vendor or
/// version reported no source*, rather than printing a blank for both.
#[must_use]
pub fn could_carry_provenance(tool: &str) -> bool {
    tool.starts_with("mcp__")
}

#[cfg(test)]
mod provenance_tests {
    use super::*;

    #[test]
    fn a_call_that_cannot_have_a_source_reads_differently_from_one_that_lost_it() {
        assert!(could_carry_provenance("mcp__linear__create_issue"));
        assert!(could_carry_provenance("mcp__github"));

        for ordinary in ["Bash", "Read", "Edit", "WebFetch", "Agent", ""] {
            assert!(
                !could_carry_provenance(ordinary),
                "{ordinary} has no server to come from"
            );
        }

        // `mcp__` anchors at the start.
        assert!(!could_carry_provenance("Bash(mcp__x)"));
        assert!(!could_carry_provenance("not_mcp__linear"));
    }

    #[test]
    fn a_source_this_build_has_never_seen_is_kept_as_received() {
        // Mapping an unknown to `unknown` would replace a fact with a guess.
        let d = Decision::new(Authority::Rule, "agent:tool.use", "x", "deny")
            .from_server(Some("something-nobody-has-shipped-yet".into()));
        assert_eq!(
            d.server_source.as_deref(),
            Some("something-nobody-has-shipped-yet")
        );
    }

    #[test]
    fn absent_is_not_unknown() {
        // Non-MCP tools, silent vendors and older agents all produce *nothing
        // said*, which must read differently from *said, and not understood*.
        let d = Decision::new(Authority::Rule, "agent:tool.use", "x", "deny");
        assert_eq!(d.server_source, None);
        // An empty string is an absence, not a value.
        let empty = d.clone().from_server(Some(String::new()));
        assert_eq!(empty.server_source, None);
    }

    #[test]
    fn two_servers_sharing_a_name_are_two_things() {
        // Same name, different software: never aggregated.
        let from_project = Decision::new(Authority::Rule, "agent:tool.use", "x", "deny")
            .by_tool("mcp__github__create_issue")
            .from_server(Some("project".into()));
        let from_user = Decision::new(Authority::Rule, "agent:tool.use", "x", "deny")
            .by_tool("mcp__github__create_issue")
            .from_server(Some("user".into()));
        assert_eq!(from_project.tool, from_user.tool);
        assert_ne!(
            from_project.server_source, from_user.server_source,
            "same name, different software"
        );
    }
}

//! The decision log: what was decided, and on whose authority — the rule that
//! refused a command, the person who allowed one, the checks that ran. The
//! policy never allows: an `allow` is always somebody's answer.
//! Unlike observations, a decision cannot be re-derived, so it is append-only
//! and survives event-log pruning. Not a tamper-evident journal: the threat is
//! *"I cannot remember why that happened"*, not an adversary on the same machine.

use crate::core::ids::{ChangeId, ProjectId, RunId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// On whose authority something happened. There is no `unknown`: a row whose
/// authority cannot be established must not be written; it is kept as an
/// [`Unattributed`] call with its reason instead.
///
/// There is no `model` and no `classifier`. No channel Devplane reads names a
/// model as the decider of a single call: Claude Code's `tool_decision` names
/// six sources, none of them a model (see [`attribute`]), and its auto mode
/// appears only as a permission mode; OpenAI documents Codex's auto-review as a
/// reviewer model deciding approvals, but no channel Devplane reads carries that
/// decision per call. A `model` authority lands only when such a channel does,
/// and only in the same change that amends the constitution's third principle,
/// this type, and the authorities guard in `scripts/concepts-check.sh`. Until
/// then a decision a model took reads *could not be attributed*, never
/// `person`, `rule` or `nobody`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
/// (`agent:tool.use`, `gate:run`, `git:push`, `github:pr.create`, `change:stop`);
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
    /// The rule that decided, or the reason: "refused by `Bash(rm -rf *)`",
    /// not "refused".
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
    /// The tool call this decides, by the agent's own id: the protocol's id
    /// for a driven call (the key its reported edit is found by), the vendor's
    /// `tool_use_id` for a watched one. `None` where no channel named the call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    /// The vendor whose own report this row was written from (`claude-code`).
    /// `None` on every decision Devplane itself took or enforced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// The vendor's own word for who decided, verbatim (`user_reject`,
    /// `config`). Kept beside `authority`, never instead of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_source: Option<String>,
}

impl Decision {
    /// A decision taken at `at`. [`Decision::new`] (outside the pure half)
    /// stamps the wall clock.
    pub fn new_at(
        authority: Authority,
        action: &str,
        subject: impl Into<String>,
        outcome: &str,
        at: Timestamp,
    ) -> Self {
        Self {
            id: crate::core::ids::new_event_id(),
            at,
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
            call_id: None,
            vendor: None,
            vendor_source: None,
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

    /// Names the driven tool call this decides.
    pub fn for_call(mut self, call: Option<&str>) -> Self {
        self.call_id = call.filter(|c| !c.is_empty()).map(str::to_string);
        self
    }

    /// Marks a row written from a vendor's own report of who decided,
    /// keeping its word verbatim.
    #[must_use]
    pub fn reported_by(mut self, vendor: &str, source: &str) -> Self {
        self.vendor = Some(vendor.to_string());
        self.vendor_source = Some(source.to_string());
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

// ---------------------------------------------------------------------------
// Who decided a call a vendor reported
// ---------------------------------------------------------------------------

/// What kind of session a vendor's report came from, as far as a documented
/// channel says. Claude Code reports a saved rule's match as the person's in
/// its Agent SDK and `-p` sessions, so the kind decides whether its `user_*`
/// sources name the person at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    /// The terminal CLI a person types into.
    Interactive,
    /// An Agent SDK or `-p` session, including every session Devplane drives.
    NotInteractive,
    /// Nothing documented said which.
    NotKnown,
}

impl SessionKind {
    /// From OpenTelemetry's `app.entrypoint` (*how the session was launched*)
    /// and whether Devplane drives the run. Only `cli` names the interactive
    /// terminal; `sdk-*` names the SDK, `-p` included. Every other value —
    /// an IDE, the desktop app, a chat integration — is not known, because
    /// the vendor does not say whether it reports rule matches as the person.
    #[must_use]
    pub fn of(entrypoint: Option<&str>, driven: bool) -> Self {
        if driven {
            return SessionKind::NotInteractive;
        }
        match entrypoint {
            Some("cli") => SessionKind::Interactive,
            Some(e) if e.starts_with("sdk-") => SessionKind::NotInteractive,
            _ => SessionKind::NotKnown,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Interactive => "interactive",
            SessionKind::NotInteractive => "not_interactive",
            SessionKind::NotKnown => "not_known",
        }
    }
}

/// A rule in the vendor's own settings that Devplane's reader found matching
/// a call: the rule as written, the file it is in, and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VendorRule {
    pub rule: String,
    pub file: String,
    /// `allow` or `deny`.
    pub effect: String,
}

impl VendorRule {
    /// The row's reason: the rule and where it is written.
    #[must_use]
    pub fn named(&self) -> String {
        format!("matched `{}` in {}", self.rule, self.file)
    }
}

/// Why a reported call has no decider Devplane can establish. Never an
/// authority, and never written as a decision row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "why", content = "value")]
pub enum Why {
    /// The report carried no `source`.
    NoSource,
    /// A `source` the pinned documentation does not define, as received.
    Unrecognised(String),
    /// Configuration decided while auto mode was on: a rule, the person's
    /// earlier grant or the vendor's classifier, and nothing says which.
    AutoMode,
    /// Configuration decided and no observation says which mode was in force.
    ModeNotKnown,
    /// The mode in force allows this call by itself.
    ModeAllowed,
    /// Configuration decided and Devplane's reader found no agreeing rule.
    ConfigUnsaid,
    /// A `user_*` source outside a session known to be interactive.
    NotInteractive,
    /// A hook decided and no hook decision Devplane recorded names the call.
    HookNotJoined,
}

impl Why {
    /// The sentence a surface prints, beside the vendor's own word.
    #[must_use]
    pub fn says(&self) -> String {
        match self {
            Why::NoSource => "not attributed — the vendor sent no source".into(),
            Why::Unrecognised(v) => format!(
                "not attributed — the vendor sent {v:?}, which its documentation of {} does not define",
                crate::core::vendors::CLAUDE_CODE_DECISION_SOURCES_CHECKED
            ),
            Why::AutoMode => "could not be attributed — auto mode was on".into(),
            Why::ModeNotKnown => {
                "could not be attributed — the permission mode was not known".into()
            }
            Why::ModeAllowed => "could not be attributed — the mode allowed it".into(),
            Why::ConfigUnsaid => "Claude Code's settings decided and do not say how — a rule, \
                                  a grant you made earlier in the session, or a failed prompt"
                .into(),
            Why::NotInteractive => "Claude Code reports a saved rule's match as the person's in \
                                    this kind of session"
                .into(),
            Why::HookNotJoined => {
                "a hook decided — no hook decision Devplane recorded names this call".into()
            }
        }
    }
}

/// What one vendor report establishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attributed {
    /// A decision row, by this authority — only ever `Person` or `Rule`.
    Row(Authority),
    /// A hook decided: Devplane's own row for the call, joined by its id when
    /// read, or [`Why::HookNotJoined`].
    Hook,
    /// No row; kept with the session's observations, with its reason.
    Unattributed(Why),
}

/// The tools Claude Code's `acceptEdits` mode allows by itself.
const EDIT_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// Whether the permission mode in force allows this call without any rule.
fn mode_allows(mode: &str, tool: &str) -> bool {
    match mode {
        "bypassPermissions" => true,
        "acceptEdits" => EDIT_TOOLS.contains(&tool),
        _ => false,
    }
}

/// Who decided a call Claude Code reported in its `tool_decision` event, as
/// far as the vendor's definition of `source` allows and no further.
///
/// - `user_permanent`, `user_temporary`, `user_abort`, `user_reject` are the
///   person **only in an interactive session**: elsewhere the vendor emits
///   them for a saved rule's match too.
/// - `config` covers rules, managed policy, flags, the active mode, an earlier
///   grant, inherently safe tools and a failed prompt, and the event does not
///   say which. It is `rule` only when the mode in force is known, is not
///   auto and does not allow the call by itself, **and** Devplane's own reader
///   of the vendor's settings found a rule matching the call whose effect
///   agrees — the row then names that rule.
/// - `hook` is joined to Devplane's own row when read.
/// - A missing or unrecognised source writes no row. Nothing is `unknown`.
///
/// Pinned to [`crate::core::vendors::CLAUDE_CODE_DECISION_SOURCES`].
#[must_use]
pub fn attribute(
    source: Option<&str>,
    decision: &str,
    tool: &str,
    mode: Option<&str>,
    session: SessionKind,
    matched: Option<&VendorRule>,
) -> Attributed {
    let Some(source) = source.filter(|s| !s.is_empty()) else {
        return Attributed::Unattributed(Why::NoSource);
    };
    match source {
        "user_permanent" | "user_temporary" | "user_abort" | "user_reject" => match session {
            SessionKind::Interactive => Attributed::Row(Authority::Person),
            _ => Attributed::Unattributed(Why::NotInteractive),
        },
        "hook" => Attributed::Hook,
        "config" => {
            let Some(mode) = mode.filter(|m| !m.is_empty()) else {
                return Attributed::Unattributed(Why::ModeNotKnown);
            };
            if mode == "auto" {
                return Attributed::Unattributed(Why::AutoMode);
            }
            let accepted = decision == "accept";
            if accepted && mode_allows(mode, tool) {
                return Attributed::Unattributed(Why::ModeAllowed);
            }
            let agrees = |r: &VendorRule| match accepted {
                true => r.effect == "allow",
                false => r.effect == "deny",
            };
            match matched {
                Some(r) if agrees(r) => Attributed::Row(Authority::Rule),
                _ => Attributed::Unattributed(Why::ConfigUnsaid),
            }
        }
        other => Attributed::Unattributed(Why::Unrecognised(other.to_string())),
    }
}

/// The permission rules one of the vendor's settings files holds, read at the
/// edge: the file, the directory its `/path` rules anchor at, and its lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VendorSettings {
    pub file: String,
    pub anchor: std::path::PathBuf,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
}

/// The vendor's rule that decided a call its configuration decided, if
/// Devplane's own reader can name one: for an accepted call, an allow rule
/// matching it while no deny or ask rule in any file does (either would have
/// stopped it); for a refused one, a deny rule matching it. Rules in flags,
/// in a session's grants or in files not read here are not seen, so `None`
/// means *not named*, never *no rule*.
#[must_use]
pub fn vendor_rule_for(
    files: &[VendorSettings],
    cwd: &std::path::Path,
    home: Option<&std::path::Path>,
    tool: &str,
    input: &serde_json::Value,
    accepted: bool,
) -> Option<VendorRule> {
    use crate::core::policy::{Class, Context, Rule};
    let hit = |class: Class, f: &VendorSettings, raw: &[String]| -> Option<VendorRule> {
        let ctx = Context::at(cwd).with_home(home).with_source(&f.anchor);
        raw.iter()
            .filter_map(|r| Rule::parse(r, class))
            .filter(|r| !r.is_negated())
            .find(|r| r.matches(&ctx, tool, input))
            .map(|r| VendorRule {
                rule: r.as_str().to_string(),
                file: f.file.clone(),
                effect: match class {
                    Class::Allow => "allow",
                    Class::Deny => "deny",
                    Class::Ask => "ask",
                }
                .into(),
            })
    };
    let first = |class: Class| {
        files.iter().find_map(|f| {
            let list = match class {
                Class::Allow => &f.allow,
                Class::Deny => &f.deny,
                Class::Ask => &f.ask,
            };
            hit(class, f, list)
        })
    };
    match accepted {
        true => match (first(Class::Deny), first(Class::Ask)) {
            (None, None) => first(Class::Allow),
            _ => None,
        },
        false => first(Class::Deny),
    }
}

/// One observation of the permission mode: when the vendor says it held
/// (its own timestamp, or the hook's), its per-process order where it sent
/// one, and the mode as spelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeMark {
    pub at: Timestamp,
    pub sequence: Option<u64>,
    pub mode: String,
}

/// The mode in force at `at`: the latest observation at or before it, by the
/// vendor's time and then its sequence — never by arrival. `None` when
/// nothing was observed by then, which is *not known*, never `default`.
#[must_use]
pub fn mode_at(marks: &[ModeMark], at: Timestamp, sequence: Option<u64>) -> Option<String> {
    marks
        .iter()
        .filter(|m| {
            m.at < at
                || (m.at == at
                    && match (m.sequence, sequence) {
                        (Some(a), Some(b)) => a <= b,
                        _ => true,
                    })
        })
        .max_by(|a, b| {
            a.at.cmp(&b.at)
                .then(a.sequence.unwrap_or(0).cmp(&b.sequence.unwrap_or(0)))
        })
        .map(|m| m.mode.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decision_says_who_decided_and_on_what_authority() {
        // "refused" is not an answer to "why did this not run".
        let d = Decision::new(Authority::Rule, "agent:tool.use", "rm -rf build", "deny")
            .because("Bash(rm -rf *)")
            .for_run(&RunId::new("s1"));
        let line = d.line();
        assert!(line.contains("rule deny agent:tool.use"), "{line}");
        assert!(line.contains("rm -rf build"));
        assert!(line.contains("Bash(rm -rf *)"));
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
mod shell_write_tests {
    use super::*;
    use crate::core::ids::RunId;

    /// A call a person answered: the policy never allows one itself.
    fn call(tool: &str, subject: &str, outcome: &str) -> Decision {
        Decision::new(Authority::Person, "agent:tool.use", subject, outcome)
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

#[cfg(test)]
mod attribute_tests {
    use super::*;

    fn rule(effect: &str) -> VendorRule {
        VendorRule {
            rule: "Read(src/**)".into(),
            file: "/p/.claude/settings.json".into(),
            effect: effect.into(),
        }
    }

    const I: SessionKind = SessionKind::Interactive;

    #[test]
    fn the_person_only_in_an_interactive_session() {
        for s in [
            "user_permanent",
            "user_temporary",
            "user_abort",
            "user_reject",
        ] {
            assert_eq!(
                attribute(Some(s), "accept", "Bash", Some("default"), I, None),
                Attributed::Row(Authority::Person),
                "{s}"
            );
            // The mode does not matter: the vendor said the person answered.
            assert_eq!(
                attribute(Some(s), "accept", "Bash", Some("auto"), I, None),
                Attributed::Row(Authority::Person)
            );
            for kind in [SessionKind::NotInteractive, SessionKind::NotKnown] {
                assert_eq!(
                    attribute(Some(s), "accept", "Bash", Some("default"), kind, None),
                    Attributed::Unattributed(Why::NotInteractive),
                    "{s} {kind:?}"
                );
            }
        }
    }

    #[test]
    fn config_is_a_rule_only_when_devplane_names_an_agreeing_rule() {
        let allow = rule("allow");
        let deny = rule("deny");
        let a = |d, mode, m| attribute(Some("config"), d, "Read", mode, I, m);
        assert_eq!(
            a("accept", Some("default"), Some(&allow)),
            Attributed::Row(Authority::Rule)
        );
        assert_eq!(
            a("reject", Some("default"), Some(&deny)),
            Attributed::Row(Authority::Rule)
        );
        // No rule, or one that disagrees: the settings did not say how.
        assert_eq!(
            a("accept", Some("default"), None),
            Attributed::Unattributed(Why::ConfigUnsaid)
        );
        assert_eq!(
            a("reject", Some("default"), Some(&allow)),
            Attributed::Unattributed(Why::ConfigUnsaid)
        );
        // Auto mode and an unknown mode win over a matching rule.
        assert_eq!(
            a("accept", Some("auto"), Some(&allow)),
            Attributed::Unattributed(Why::AutoMode)
        );
        assert_eq!(
            a("accept", None, Some(&allow)),
            Attributed::Unattributed(Why::ModeNotKnown)
        );
        // A mode that allows the call by itself decided nothing a rule did.
        assert_eq!(
            attribute(
                Some("config"),
                "accept",
                "Edit",
                Some("acceptEdits"),
                I,
                Some(&allow)
            ),
            Attributed::Unattributed(Why::ModeAllowed)
        );
        assert_eq!(
            attribute(
                Some("config"),
                "accept",
                "Bash",
                Some("bypassPermissions"),
                I,
                Some(&allow)
            ),
            Attributed::Unattributed(Why::ModeAllowed)
        );
        // `acceptEdits` does not allow a shell command by itself.
        assert_eq!(
            attribute(
                Some("config"),
                "accept",
                "Bash",
                Some("acceptEdits"),
                I,
                Some(&allow)
            ),
            Attributed::Row(Authority::Rule)
        );
    }

    #[test]
    fn a_missing_or_new_source_is_never_a_row_and_never_unknown() {
        assert_eq!(
            attribute(None, "accept", "Bash", Some("default"), I, None),
            Attributed::Unattributed(Why::NoSource)
        );
        assert_eq!(
            attribute(Some(""), "accept", "Bash", Some("default"), I, None),
            Attributed::Unattributed(Why::NoSource)
        );
        let new = attribute(Some("classifier"), "accept", "Bash", Some("auto"), I, None);
        assert_eq!(
            new,
            Attributed::Unattributed(Why::Unrecognised("classifier".into()))
        );
        assert_eq!(
            attribute(Some("hook"), "accept", "Bash", None, I, None),
            Attributed::Hook
        );
        for why in [
            Why::NoSource,
            Why::Unrecognised("x".into()),
            Why::AutoMode,
            Why::ModeNotKnown,
            Why::ModeAllowed,
            Why::ConfigUnsaid,
            Why::NotInteractive,
            Why::HookNotJoined,
        ] {
            assert!(!why.says().contains("unknown"), "{why:?}");
        }
    }

    /// Every result is a person, a rule, a hook to join, or no authority at
    /// all: telemetry never yields `timer`, `nobody`, `devplane` or a new one.
    #[test]
    fn only_a_person_or_a_rule_is_ever_written_from_a_report() {
        let sources = [
            None,
            Some("config"),
            Some("hook"),
            Some("user_permanent"),
            Some("user_temporary"),
            Some("user_abort"),
            Some("user_reject"),
            Some("model"),
        ];
        let modes = [
            None,
            Some("default"),
            Some("auto"),
            Some("bypassPermissions"),
        ];
        let kinds = [I, SessionKind::NotInteractive, SessionKind::NotKnown];
        let allow = rule("allow");
        for s in sources {
            for m in modes {
                for k in kinds {
                    for r in [None, Some(&allow)] {
                        if let Attributed::Row(a) = attribute(s, "accept", "Read", m, k, r) {
                            assert!(matches!(a, Authority::Person | Authority::Rule), "{a:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_session_kind_comes_from_the_entrypoint_and_driving_only() {
        assert_eq!(SessionKind::of(Some("cli"), false), I);
        assert_eq!(
            SessionKind::of(Some("cli"), true),
            SessionKind::NotInteractive
        );
        for sdk in ["sdk-cli", "sdk-ts", "sdk-py"] {
            assert_eq!(
                SessionKind::of(Some(sdk), false),
                SessionKind::NotInteractive
            );
        }
        for other in [
            None,
            Some("claude-vscode"),
            Some("claude-desktop"),
            Some(""),
        ] {
            assert_eq!(
                SessionKind::of(other, false),
                SessionKind::NotKnown,
                "{other:?}"
            );
        }
    }

    #[test]
    fn devplane_names_the_vendors_rule_only_when_nothing_overrides_it() {
        let dir = std::path::Path::new("/p");
        let project = VendorSettings {
            file: "/p/.claude/settings.json".into(),
            anchor: dir.into(),
            allow: vec!["Bash(npm test)".into(), "Read".into()],
            deny: vec!["Bash(rm *)".into()],
            ask: vec!["Bash(npm test --watch)".into()],
        };
        let files = [project];
        let bash = |c: &str| serde_json::json!({"command": c});
        let named = vendor_rule_for(&files, dir, None, "Bash", &bash("npm test"), true)
            .expect("an allow rule");
        assert_eq!(named.rule, "Bash(npm test)");
        assert_eq!(named.file, "/p/.claude/settings.json");
        assert_eq!(named.effect, "allow");
        assert_eq!(
            named.named(),
            "matched `Bash(npm test)` in /p/.claude/settings.json"
        );
        // Nothing matches: not named.
        assert_eq!(
            vendor_rule_for(&files, dir, None, "Bash", &bash("cargo build"), true),
            None
        );
        // A refusal is named by a deny rule, never by an allow.
        let deny = vendor_rule_for(&files, dir, None, "Bash", &bash("rm -rf x"), false)
            .expect("a deny rule");
        assert_eq!(deny.effect, "deny");
        assert_eq!(
            vendor_rule_for(&files, dir, None, "Bash", &bash("npm test"), false),
            None
        );
        // An accept a deny rule matches is not the allow rule's.
        assert_eq!(
            vendor_rule_for(&files, dir, None, "Bash", &bash("rm -rf x"), true),
            None
        );
    }

    fn t(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn the_mode_in_force_is_read_by_the_vendors_time_not_by_arrival() {
        // Given out of arrival order: `auto` arrived last but held first.
        let marks = vec![
            ModeMark {
                at: t("2026-10-03T10:05:00Z"),
                sequence: Some(9),
                mode: "default".into(),
            },
            ModeMark {
                at: t("2026-10-03T10:00:00Z"),
                sequence: Some(2),
                mode: "auto".into(),
            },
        ];
        assert_eq!(
            mode_at(&marks, t("2026-10-03T10:01:00Z"), Some(4)).as_deref(),
            Some("auto")
        );
        assert_eq!(
            mode_at(&marks, t("2026-10-03T10:06:00Z"), Some(12)).as_deref(),
            Some("default")
        );
        // Nothing observed by then is not known, never `default`.
        assert_eq!(mode_at(&marks, t("2026-10-03T09:00:00Z"), None), None);
        // One instant: the sequence breaks the tie.
        let same = vec![
            ModeMark {
                at: t("2026-10-03T10:00:00Z"),
                sequence: Some(3),
                mode: "auto".into(),
            },
            ModeMark {
                at: t("2026-10-03T10:00:00Z"),
                sequence: Some(5),
                mode: "default".into(),
            },
        ];
        assert_eq!(
            mode_at(&same, t("2026-10-03T10:00:00Z"), Some(4)).as_deref(),
            Some("auto")
        );
    }
}

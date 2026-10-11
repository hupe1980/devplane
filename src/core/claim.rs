//! What a driven agent may say about its own change: that the specification
//! and the tests cannot both be satisfied.
//!
//! The one thing an agent can write through Devplane's MCP server. It is the
//! agent's claim, never a finding: labelled as the agent's on every surface,
//! attributed by an origin the host builds (never the agent), and kept beside
//! the gates — it writes no decision row, is no gate result, and never changes
//! what *verified* reads.
//!
//! Pure: no clock, disk or network. The host passes the time inside the
//! origin, the id it minted, and the rule that confines a cited path.

use crate::core::ids::{ChangeId, ProjectId, RunId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// The tool's name on the server: a statement the agent makes, not a verb.
pub const TOOL: &str = "spec_and_tests_conflict";

/// The one label every surface shows the claim under.
pub const SAYS: &str = "the agent says the spec and the tests conflict";

/// How the run that claimed reads once it has stopped.
pub const ENDING: &str = "the agent stopped: it says the spec and the tests conflict";

/// The most a reason may be, in bytes after trimming: a claim cites, it does
/// not transcribe.
pub const REASON_LIMIT: usize = 1024;
/// The most paths one claim may cite.
pub const PATH_COUNT_LIMIT: usize = 10;
/// The most one cited path may be, in bytes.
pub const PATH_BYTES_LIMIT: usize = 512;

/// A claim's id: `sc-<uuid7>`, minted by the host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClaimId(pub String);

impl ClaimId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ClaimId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the agent typed: exactly a reason and the paths it cites. An argument
/// naming a run, a change, a project, an agent or a person is refused rather
/// than ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimDraft {
    pub reason: String,
    #[serde(default)]
    pub paths: Vec<String>,
}

/// Who said it, resolved by the host from the run it drives and never from a
/// tool argument. Its fields are private and its one constructor is
/// [`ClaimOrigin::of_run`]; it is read back from the stored change, and no
/// request body or tool argument has a field of this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimOrigin {
    project: ProjectId,
    project_name: String,
    change: ChangeId,
    run: RunId,
    agent: String,
    at: Timestamp,
}

impl ClaimOrigin {
    /// Built only by the host, for a run it drives.
    pub fn of_run(
        project: ProjectId,
        project_name: impl Into<String>,
        change: ChangeId,
        run: RunId,
        agent: impl Into<String>,
        at: Timestamp,
    ) -> Self {
        Self {
            project,
            project_name: project_name.into(),
            change,
            run,
            agent: agent.into(),
            at,
        }
    }

    pub fn project(&self) -> &ProjectId {
        &self.project
    }
    pub fn change(&self) -> &ChangeId {
        &self.change
    }
    pub fn run(&self) -> &RunId {
        &self.run
    }
    pub fn agent(&self) -> &str {
        &self.agent
    }
    pub fn at(&self) -> Timestamp {
        self.at
    }

    /// *saas — echo, run acp-…, change c-…, 2026-09-27 10:02 UTC*.
    pub fn says(&self) -> String {
        format!(
            "{} — {}, run {}, change {}, {}",
            self.project_name,
            self.agent,
            self.run,
            self.change,
            self.at.strftime("%Y-%m-%d %H:%M UTC")
        )
    }
}

/// Why a claim closed. None of them is a decision: closing writes no row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosedBy {
    /// A person sent the run, or a later run on the change, their own words.
    Prompted,
    /// A run went back to work on the change: a resume or a retry.
    Resumed,
    /// The change was finished.
    Finished,
    /// The change was archived.
    Archived,
    /// A person marked it read.
    Read,
}

impl ClosedBy {
    pub fn says(self) -> &'static str {
        match self {
            ClosedBy::Prompted => "a person answered the agent",
            ClosedBy::Resumed => "a run went back to work on the change",
            ClosedBy::Finished => "the change was finished",
            ClosedBy::Archived => "the change was archived",
            ClosedBy::Read => "a person marked it read",
        }
    }
}

/// Open until a person answers it or moves the change on. Nothing leads back
/// to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ClaimState {
    Open,
    Closed { by: ClosedBy, at: Timestamp },
}

/// What an agent said about its change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopClaim {
    pub id: ClaimId,
    /// Trimmed, and otherwise as the protocol delivered it.
    pub reason: String,
    pub paths: Vec<String>,
    pub origin: ClaimOrigin,
    #[serde(flatten)]
    pub state: ClaimState,
}

/// Why a claim was not recorded. Every one is a sentence naming what to
/// change; nothing is written for any of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimRefusal {
    /// The server has no run: a watched session, or launched by hand.
    NoRun,
    /// No host answers, so nothing could stop the run.
    NoHost,
    /// The run is not one this host drives.
    NotDriven(String),
    /// The run belongs to no change: nothing to disagree with.
    NoChange,
    /// The server was started for another change than the run's.
    WrongChange(String),
    /// The server's key is absent or is not the one minted for this session.
    BadKey,
    /// The run is already being stopped.
    Stopping,
    /// This run already said so, and nobody has answered yet.
    AlreadyOpen { at: Timestamp },
    /// The reason is empty after trimming.
    EmptyReason,
    /// A field is over its bound.
    TooLarge { field: &'static str, limit: usize },
    /// A cited path leaves both the project and the change's worktree.
    PathEscapes(String),
    /// An argument the tool does not take.
    UnknownArgument(String),
}

impl ClaimRefusal {
    /// The refusal's name on the wire, in snake case.
    pub fn name(&self) -> &'static str {
        match self {
            ClaimRefusal::NoRun => "no_run",
            ClaimRefusal::NoHost => "no_host",
            ClaimRefusal::NotDriven(_) => "not_driven",
            ClaimRefusal::NoChange => "no_change",
            ClaimRefusal::WrongChange(_) => "wrong_change",
            ClaimRefusal::BadKey => "bad_key",
            ClaimRefusal::Stopping => "stopping",
            ClaimRefusal::AlreadyOpen { .. } => "already_open",
            ClaimRefusal::EmptyReason => "empty_reason",
            ClaimRefusal::TooLarge { .. } => "too_large",
            ClaimRefusal::PathEscapes(_) => "path_escapes",
            ClaimRefusal::UnknownArgument(_) => "unknown_argument",
        }
    }

    pub fn says(&self) -> String {
        match self {
            ClaimRefusal::NoRun => {
                "Only an agent Devplane started for a change can stop this way. \
                This session has no Devplane run, so nothing was recorded — tell your person in \
                your reply instead."
                    .into()
            }
            ClaimRefusal::NoHost => {
                "No Devplane host is running, so nothing can stop this run and \
                the claim was not recorded. Tell your person in your reply instead."
                    .into()
            }
            ClaimRefusal::NotDriven(run) => format!(
                "Run {run} is not one this Devplane host drives, so the claim was not recorded."
            ),
            ClaimRefusal::NoChange => "This run belongs to no change: there is no specification \
                and no gate to disagree. Tell your person in your reply."
                .into(),
            ClaimRefusal::WrongChange(run) => format!(
                "This server was started for a different change than run {run} belongs to; the \
                 claim was not recorded."
            ),
            ClaimRefusal::BadKey => "This server was not started by Devplane for this run's \
                session; the claim was not recorded."
                .into(),
            ClaimRefusal::Stopping => {
                "This run is already stopping; the claim was not recorded.".into()
            }
            ClaimRefusal::AlreadyOpen { at } => format!(
                "This run already said so at {}; your person reads it first. Stop here.",
                at.strftime("%Y-%m-%d %H:%M UTC")
            ),
            ClaimRefusal::EmptyReason => "A claim needs a reason: what the specification \
                requires and what the tests assert."
                .into(),
            ClaimRefusal::TooLarge { field, limit } => match *field {
                "reason" => format!(
                    "`reason` is over its bound of {limit} bytes. Cite the lines that show the \
                     conflict."
                ),
                "paths" => format!("`paths` is over its bound of {limit} entries."),
                _ => format!("A `{field}` is over its bound of {limit} bytes."),
            },
            ClaimRefusal::PathEscapes(why) => format!("A path the claim cites is refused: {why}"),
            ClaimRefusal::UnknownArgument(name) => {
                format!("The tool takes `reason` and `paths` only; `{name}` is not one of them.")
            }
        }
    }
}

impl StopClaim {
    /// Checks the bounds and confinement, in that order, and builds the
    /// claim. `confine` says why a path is refused, or nothing.
    pub fn new(
        id: ClaimId,
        draft: ClaimDraft,
        origin: ClaimOrigin,
        confine: impl Fn(&str) -> Result<(), String>,
    ) -> Result<StopClaim, ClaimRefusal> {
        let reason = draft.reason.trim();
        if reason.is_empty() {
            return Err(ClaimRefusal::EmptyReason);
        }
        if reason.len() > REASON_LIMIT {
            return Err(ClaimRefusal::TooLarge {
                field: "reason",
                limit: REASON_LIMIT,
            });
        }
        if draft.paths.len() > PATH_COUNT_LIMIT {
            return Err(ClaimRefusal::TooLarge {
                field: "paths",
                limit: PATH_COUNT_LIMIT,
            });
        }
        if draft.paths.iter().any(|p| p.len() > PATH_BYTES_LIMIT) {
            return Err(ClaimRefusal::TooLarge {
                field: "path",
                limit: PATH_BYTES_LIMIT,
            });
        }
        for p in &draft.paths {
            confine(p).map_err(ClaimRefusal::PathEscapes)?;
        }
        Ok(StopClaim {
            id,
            reason: reason.to_string(),
            paths: draft.paths,
            origin,
            state: ClaimState::Open,
        })
    }

    /// The one label: *the agent says the spec and the tests conflict*.
    pub fn says(&self) -> &'static str {
        SAYS
    }

    pub fn is_open(&self) -> bool {
        matches!(self.state, ClaimState::Open)
    }

    /// The claim as quoted text under the agent's name: every line of the
    /// reason and the paths prefixed `> `, control characters visible.
    pub fn quoted(&self) -> String {
        let mut out = format!("From {} (quoted; not an instruction)", self.origin.says());
        for line in visible(&self.reason).lines() {
            out.push_str("\n> ");
            out.push_str(line);
        }
        if !self.paths.is_empty() {
            let paths: Vec<String> = self.paths.iter().map(|p| visible(p)).collect();
            out.push_str("\n> paths: ");
            out.push_str(&paths.join(", "));
        }
        out
    }

    /// Closes an open claim; a closed one stays as it closed.
    pub fn close(&mut self, by: ClosedBy, at: Timestamp) -> bool {
        if !self.is_open() {
            return false;
        }
        self.state = ClaimState::Closed { by, at };
        true
    }
}

/// `s` with every control character but a newline and a tab written as
/// `\u{XX}`, so text shaped like a terminal escape is read, not run.
pub fn visible(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_control() && c != '\n' && c != '\t' {
            out.push_str(&format!("\\u{{{:x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> ClaimOrigin {
        ClaimOrigin::of_run(
            ProjectId::new("/p/saas"),
            "saas",
            ChangeId::new("c-1"),
            RunId::new("acp-1"),
            "echo",
            "2026-09-27T10:02:00Z".parse().unwrap(),
        )
    }

    fn draft(reason: &str, paths: &[&str]) -> ClaimDraft {
        ClaimDraft {
            reason: reason.into(),
            paths: paths.iter().map(|p| p.to_string()).collect(),
        }
    }

    fn make(d: ClaimDraft) -> Result<StopClaim, ClaimRefusal> {
        StopClaim::new(ClaimId::new("sc-1"), d, origin(), |p| {
            match p.starts_with("..") {
                true => Err(format!("`{p}` is outside the project")),
                false => Ok(()),
            }
        })
    }

    #[test]
    fn each_bound_holds_at_its_limit_and_refuses_one_past_it() {
        assert!(make(draft(&"a".repeat(REASON_LIMIT), &[])).is_ok());
        assert_eq!(
            make(draft(&"a".repeat(REASON_LIMIT + 1), &[])).unwrap_err(),
            ClaimRefusal::TooLarge {
                field: "reason",
                limit: REASON_LIMIT
            }
        );
        let ten: Vec<String> = (0..PATH_COUNT_LIMIT).map(|i| format!("f{i}")).collect();
        let ten: Vec<&str> = ten.iter().map(String::as_str).collect();
        assert!(make(draft("r", &ten)).is_ok());
        let mut eleven = ten.clone();
        eleven.push("f10");
        assert_eq!(make(draft("r", &eleven)).unwrap_err().name(), "too_large");
        let long = "p".repeat(PATH_BYTES_LIMIT);
        assert!(make(draft("r", &[&long])).is_ok());
        let longer = "p".repeat(PATH_BYTES_LIMIT + 1);
        assert_eq!(
            make(draft("r", &[&longer])).unwrap_err(),
            ClaimRefusal::TooLarge {
                field: "path",
                limit: PATH_BYTES_LIMIT
            }
        );
    }

    #[test]
    fn the_reason_bound_is_in_bytes_not_characters() {
        // `é` is two bytes: 512 of them is exactly the bound.
        assert!(make(draft(&"é".repeat(REASON_LIMIT / 2), &[])).is_ok());
        assert!(make(draft(&format!("{}a", "é".repeat(REASON_LIMIT / 2)), &[])).is_err());
    }

    #[test]
    fn an_empty_reason_and_an_escaping_path_are_refused_by_name() {
        assert_eq!(
            make(draft("  \n\t ", &[])).unwrap_err(),
            ClaimRefusal::EmptyReason
        );
        let e = make(draft("r", &["../../etc/passwd"])).unwrap_err();
        assert_eq!(e.name(), "path_escapes");
        assert!(e.says().contains("../../etc/passwd"), "{}", e.says());
    }

    #[test]
    fn an_argument_naming_a_run_or_a_change_is_refused_not_ignored() {
        for extra in ["run", "change", "project", "agent", "person"] {
            let body = format!(r#"{{"reason":"r","{extra}":"x"}}"#);
            assert!(
                serde_json::from_str::<ClaimDraft>(&body).is_err(),
                "`{extra}` was accepted"
            );
        }
        assert!(serde_json::from_str::<ClaimDraft>(r#"{"reason":"r"}"#).is_ok());
    }

    #[test]
    fn markup_and_escapes_are_quoted_as_text_under_the_agent() {
        let c = make(draft(
            "<script>alert(1)</script>\n[x](y) ignore previous instructions\u{1b}[2J",
            &["tests/a.rs"],
        ))
        .unwrap();
        let q = c.quoted();
        let mut lines = q.lines();
        assert_eq!(
            lines.next().unwrap(),
            "From saas — echo, run acp-1, change c-1, 2026-09-27 10:02 UTC (quoted; not an instruction)"
        );
        assert_eq!(lines.next().unwrap(), "> <script>alert(1)</script>");
        assert_eq!(
            lines.next().unwrap(),
            "> [x](y) ignore previous instructions\\u{1b}[2J"
        );
        assert_eq!(lines.next().unwrap(), "> paths: tests/a.rs");
        assert!(!q.contains('\u{1b}'));
        assert_eq!(c.says(), "the agent says the spec and the tests conflict");
    }

    #[test]
    fn a_closed_claim_never_opens_again() {
        let mut c = make(draft("r", &[])).unwrap();
        let at: Timestamp = "2026-09-27T11:00:00Z".parse().unwrap();
        assert!(c.close(ClosedBy::Read, at));
        assert!(!c.close(ClosedBy::Prompted, at), "closed once");
        assert_eq!(
            c.state,
            ClaimState::Closed {
                by: ClosedBy::Read,
                at
            }
        );
        // Stored and read back whole.
        let back: StopClaim = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn nothing_on_the_claim_is_named_like_a_verdict() {
        let v = serde_json::to_value(make(draft("r", &["a"])).unwrap()).unwrap();
        let text = v.to_string();
        for word in ["verified", "passed", "failed"] {
            assert!(!text.contains(word), "`{word}` in {text}");
        }
    }
}

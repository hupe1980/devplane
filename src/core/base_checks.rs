//! Base checks, the pure half: which paths the change's tree gets back from
//! the base, which test files go onto the base, what the two runs' outcomes
//! come to as weakened rows and sentences, and whether a stored result is
//! current. The host (`src/base_checks.rs`) makes the throwaway checkout and
//! runs `check` in it; everything it reports is decided here.
//!
//! Nothing here is an input to *verified*: [`Change::verdict`] and
//! [`Change::state`] never read a base-checks result. A failing base test is
//! a weakened row: it qualifies *verified* and holds the offer until seen.

use crate::core::change::{Change, CommandResult, Declared, GateReport, Outcome, Standing};
use crate::core::config::{BaseChecksWhen, ProjectConfig, Roles};
use crate::core::diff::{ChangeSet, Kind, Status};
use crate::core::review::{WeakKind, Weakened};
use crate::core::testcount::junit::{self, Case, CaseOutcome, JunitFile, NamedDiff};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// What a result is for: the base commit, the change's tree, and the `check`
/// definition. A result whose key differs from the current one is stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub base: String,
    pub tree: String,
    pub definition: String,
}

/// The paths put back as the base has them for the base's tests on this change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Defining {
    /// The base has it; the change edited or deleted it.
    pub restored: Vec<String>,
    /// The change added it; the base lacks it.
    pub removed: Vec<String>,
    /// Files outside the tests that declare tests: their tests ran as the
    /// change wrote them, since they cannot be put back without the code.
    pub inline: Vec<String>,
}

impl Defining {
    pub fn is_empty(&self) -> bool {
        self.restored.is_empty() && self.removed.is_empty()
    }
}

/// One of the change's test files, placed on the base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacedFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
    /// The blob placed, from the verified tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// What goes onto the base for this change's new tests on the base.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub placed: Vec<PlacedFile>,
    /// Outside the `tests` role, with added test declarations: never moved.
    pub not_separable: Vec<String>,
    /// No `tests` role is declared, so nothing is guessed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_role: bool,
}

/// How one of the two runs ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ran", rename_all = "snake_case")]
pub enum RunOutcome {
    Passed,
    Failed,
    TimedOut {
        after_s: u64,
    },
    CouldNotRun {
        why: String,
    },
    /// Not run, and nothing spent: nothing to put back, nothing to place.
    Skipped {
        why: String,
    },
}

impl RunOutcome {
    /// From the `check` report of a run.
    pub fn of(report: &GateReport) -> Self {
        if report.passed() {
            return RunOutcome::Passed;
        }
        for c in &report.commands {
            match &c.outcome {
                Outcome::TimedOut { after_secs } => {
                    return RunOutcome::TimedOut {
                        after_s: *after_secs,
                    };
                }
                Outcome::NeverStarted { reason } | Outcome::Unknown { reason } => {
                    return RunOutcome::CouldNotRun {
                        why: format!("`{}` did not run: {reason}", c.command),
                    };
                }
                Outcome::Exited { .. } => {}
            }
        }
        match report.commands.is_empty() {
            true => RunOutcome::CouldNotRun {
                why: "no `check` command is declared".into(),
            },
            false => RunOutcome::Failed,
        }
    }
}

/// One run: how it ended, its commands with their counts, what JUnit said,
/// and the setup it needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub outcome: RunOutcome,
    #[serde(default)]
    pub commands: Vec<CommandResult>,
    #[serde(default)]
    pub junit: Vec<JunitFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<CommandResult>,
}

impl Run {
    /// A run that never started a process.
    pub fn not_run(outcome: RunOutcome) -> Self {
        Run {
            outcome,
            commands: Vec::new(),
            junit: Vec::new(),
            setup: None,
        }
    }

    /// Every case its JUnit files named.
    pub fn cases(&self) -> Vec<&Case> {
        self.junit.iter().flat_map(|j| j.read.cases()).collect()
    }

    /// Whether any declared JUnit file was read from this run.
    pub fn wrote_junit(&self) -> bool {
        self.junit
            .iter()
            .any(|j| matches!(j.read, junit::JunitRead::Read { .. }))
    }
}

/// The base's tests on this change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnChange {
    pub defining: Defining,
    pub run: Run,
    /// Against the change's own `check`, where both wrote JUnit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named: Option<NamedDiff>,
}

/// One new test, as JUnit named it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTest {
    pub suite: String,
    pub class: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

impl NewTest {
    fn of(c: &Case) -> Self {
        NewTest {
            suite: c.suite.clone(),
            class: c.class.clone(),
            name: c.name.clone(),
            file: c.file.clone(),
        }
    }

    pub fn says(&self) -> String {
        match self.class.trim().is_empty() {
            true => self.name.clone(),
            false => format!("{}::{}", self.class.trim(), self.name.trim()),
        }
    }
}

/// What this change's new tests did on the base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum NewTests {
    /// JUnit named them.
    Counted {
        passing: Vec<NewTest>,
        failing: Vec<NewTest>,
        skipped: u32,
        unattributed: u32,
    },
    /// No JUnit on this run: the gate's outcome, by file.
    ByFile {
        passed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        why: Option<String>,
    },
    None,
}

/// This change's new tests on the base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnBase {
    pub placement: Placement,
    pub run: Run,
    pub counted: NewTests,
}

/// One pair of runs, for one key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseChecks {
    pub key: Key,
    /// When the pair ended: a fact about the tool.
    pub at: Timestamp,
    pub on_change: OnChange,
    pub on_base: OnBase,
}

/// Why there is no current result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Absent {
    Off,
    NotRun,
    Running,
    Stale,
    CheckNotPassing,
    CheckNotRun,
    NoGates,
    NoHistory,
    /// `devplane verify --no-base-checks`.
    Declined,
}

impl Absent {
    pub fn says(self) -> &'static str {
        match self {
            Absent::Off => "base checks are turned off in devplane.toml",
            Absent::NotRun => {
                "base checks not run — they run when you offer, or press Run base checks"
            }
            Absent::Running => "base checks running",
            Absent::Stale => "base checks stale — the tree changed since they ran",
            Absent::CheckNotPassing => "not run — `check` does not pass with the change",
            Absent::CheckNotRun => "not run — `check` has not run against this tree",
            Absent::NoGates => "not run — no gates declared",
            Absent::NoHistory => "not run — this checkout shares no history with the base branch",
            Absent::Declined => "not run — `--no-base-checks` was passed",
        }
    }

    /// Whether a run may start from here.
    pub fn allows_a_run(self) -> bool {
        matches!(self, Absent::NotRun | Absent::Stale)
    }
}

/// The `check` definition a result is keyed by, from the governing config.
pub fn definition(config: &ProjectConfig) -> String {
    crate::core::testcount::definition_digest(
        &config.gates.check,
        config.gates.timeout.as_secs(),
        config.workspace.setup.as_deref(),
        &config.workspace.share,
        &config.gates.junit,
    )
}

/// The key a run would have now, or why no run may happen: first that
/// applies of *no gates*, *off*, *check not run*, *no history*, *check not
/// passing*. Reads the verdict; never feeds it.
pub fn key_now(change: &Change, config: &ProjectConfig) -> Result<Key, Absent> {
    if !config.has_gates() {
        return Err(Absent::NoGates);
    }
    if config.gates.base_checks == BaseChecksWhen::Off {
        return Err(Absent::Off);
    }
    let report = change.check_report().ok_or(Absent::CheckNotRun)?;
    if report.passed() && report.base_commit.is_none() {
        return Err(Absent::NoHistory);
    }
    match change.verdict(
        Declared::Checks(&config.gates.check),
        change.tree_now.as_ref(),
    ) {
        Standing::Verified => {}
        Standing::Failed { .. } => return Err(Absent::CheckNotPassing),
        Standing::NoGatesDeclared => return Err(Absent::NoGates),
        _ => return Err(Absent::CheckNotRun),
    }
    let tree = report
        .commit
        .as_ref()
        .and_then(|c| c.tree.clone())
        .ok_or(Absent::CheckNotRun)?;
    Ok(Key {
        base: report.base_commit.clone().ok_or(Absent::NoHistory)?,
        tree,
        definition: definition(config),
    })
}

/// The stored result, when it is for the tree, base and definition now.
pub fn current<'a>(change: &'a Change, config: &ProjectConfig) -> Option<&'a BaseChecks> {
    let key = key_now(change, config).ok()?;
    change.base_checks.as_ref().filter(|b| b.key == key)
}

/// The base-test rows of a current result; a stale one contributes none.
pub fn current_rows(change: &Change, config: &ProjectConfig) -> Vec<Weakened> {
    current(change, config)
        .map(|b| rows(&b.on_change))
        .unwrap_or_default()
}

/// Files outside the tests whose added lines declare a test.
fn declares_tests(set: &ChangeSet, tests: &crate::core::review::TestPaths) -> Vec<String> {
    let mut out: Vec<String> = set
        .uncut()
        .files
        .iter()
        .filter(|f| !matches!(f.status, Status::Deleted) && !tests.matches(&f.path))
        .filter(|f| {
            crate::core::review::hunks_of(&f.body)
                .iter()
                .flat_map(|h| h.lines.iter())
                .any(|(k, t)| *k == Kind::Added && crate::core::review::is_test_def(t.trim()))
        })
        .map(|f| f.path.clone())
        .collect();
    out.sort();
    out
}

/// The paths that define the checks and differ from the base: the `tests`
/// role (or the weakened scan's fallback without one), the scan's table of
/// check-defining files, every file a declared gate command names, and
/// `devplane.toml`. A rename is the old path put back and the new removed.
pub fn defining(set: &ChangeSet, roles: Option<&Roles>, gate_commands: &[String]) -> Defining {
    let tests = crate::core::review::TestPaths::of(roles);
    let gate_files = crate::core::review::files_gates_run(gate_commands);
    let defines = |p: &str| {
        tests.matches(p)
            || crate::core::review::defines_checks(p).is_some()
            || gate_files.iter().any(|g| g == p)
            || p == "devplane.toml"
    };
    let mut out = Defining::default();
    for f in &set.uncut().files {
        let sides: Vec<(&str, bool)> = match &f.status {
            Status::Added => vec![(&f.path, true)],
            Status::Modified | Status::Deleted => vec![(&f.path, false)],
            Status::Renamed { from } => vec![(from, false), (&f.path, true)],
        };
        for (path, added) in sides {
            if !defines(path) {
                continue;
            }
            let list = match added {
                true => &mut out.removed,
                false => &mut out.restored,
            };
            if !list.iter().any(|p| p == path) {
                list.push(path.to_string());
            }
        }
    }
    out.restored.sort();
    out.removed.sort();
    out.inline = declares_tests(set, &tests)
        .into_iter()
        .filter(|p| !defines(p))
        .collect();
    out
}

/// The change's added, edited or renamed files under the declared `tests`
/// role, and the files outside it that declare tests. Without a role nothing
/// is placed: a guessed file would change what runs for no declared reason.
pub fn placement(set: &ChangeSet, roles: Option<&Roles>) -> Placement {
    let Some(roles) = roles.filter(|r| !r.tests.is_empty()) else {
        return Placement {
            no_role: true,
            ..Placement::default()
        };
    };
    let tests = crate::core::review::TestPaths::of(Some(roles));
    let mut placed: Vec<PlacedFile> = set
        .uncut()
        .files
        .iter()
        .filter(|f| tests.matches(&f.path))
        .filter_map(|f| match &f.status {
            Status::Deleted => None,
            Status::Renamed { from } => Some(PlacedFile {
                path: f.path.clone(),
                renamed_from: Some(from.clone()),
                blob: None,
            }),
            Status::Added | Status::Modified => Some(PlacedFile {
                path: f.path.clone(),
                renamed_from: None,
                blob: None,
            }),
        })
        .collect();
    placed.sort_by(|a, b| a.path.cmp(&b.path));
    Placement {
        placed,
        not_separable: declares_tests(set, &tests),
        no_role: false,
    }
}

/// Why nothing was placed, when nothing was: the second run then starts no
/// process.
pub fn nothing_to_place(p: &Placement) -> Option<String> {
    if p.no_role {
        return Some("no tests role declared".into());
    }
    if !p.placed.is_empty() {
        return None;
    }
    Some(match p.not_separable.len() {
        0 => "the change adds or edits no tests".into(),
        n => not_separable(n),
    })
}

fn not_separable(n: usize) -> String {
    format!(
        "not separable — {n} {} tests and code together",
        match n {
            1 => "file holds",
            _ => "files hold",
        }
    )
}

/// What the base's tests on this change mean for the weakened group: none
/// for a pass or a skip; one per failing test otherwise, then one for the
/// failures not named, or one with the reason it did not pass.
pub fn rows(on_change: &OnChange) -> Vec<Weakened> {
    let run = &on_change.run;
    let row = |path: String, why: String, matched: String| Weakened {
        path,
        why,
        kind: WeakKind::BaseTests,
        matched,
    };
    const HERE: &str = "base checks";
    match &run.outcome {
        RunOutcome::Passed | RunOutcome::Skipped { .. } => Vec::new(),
        RunOutcome::TimedOut { after_s } => vec![row(
            HERE.into(),
            format!("timed out with the base's tests after {after_s}s"),
            "base tests timed out".into(),
        )],
        RunOutcome::CouldNotRun { why } => vec![row(
            HERE.into(),
            format!("could not be run with this change's files: {why}"),
            format!("base tests could not run: {why}"),
        )],
        RunOutcome::Failed => {
            let mut out: Vec<Weakened> = Vec::new();
            let failing: Vec<&Case> = run
                .cases()
                .into_iter()
                .filter(|c| matches!(c.outcome, CaseOutcome::Failed | CaseOutcome::Error))
                .collect();
            let failed_cmd = run.commands.iter().find(|c| !c.passed());
            if !failing.is_empty() {
                for c in &failing {
                    let id = c.id();
                    out.push(row(
                        c.file
                            .clone()
                            .unwrap_or_else(|| format!("{}::{}", c.suite, c.class)),
                        "fails as the base wrote it, on this change".into(),
                        match id.nth {
                            0 => format!("base test {}::{}::{}", c.suite, c.class, c.name),
                            n => format!("base test {}::{}::{} #{n}", c.suite, c.class, c.name),
                        },
                    ));
                }
            } else if let Some(cmd) = failed_cmd {
                for line in &cmd.failures {
                    out.push(row(
                        path_in(line).unwrap_or_else(|| HERE.into()),
                        "fails as the base wrote it, on this change".into(),
                        format!("base test {line}"),
                    ));
                }
            }
            let named = out.len() as u32;
            let counted: Option<u32> = failed_cmd.and_then(|c| {
                let t = c.tests.tallies();
                (!t.is_empty()).then(|| t.iter().map(|t| t.failed()).sum())
            });
            let (command, digest) = failed_cmd
                .map(|c| (c.command.clone(), c.output_digest.clone()))
                .unwrap_or_default();
            match counted {
                Some(n) if n > named => out.push(row(
                    HERE.into(),
                    format!(
                        "check failed with the base's tests; {} not named",
                        plural(n - named, "failure", "failures")
                    ),
                    format!("base tests failed: `{command}` {digest}"),
                )),
                None if named == 0 => out.push(row(
                    HERE.into(),
                    "check failed with the base's tests; which tests, and how many, the runner \
                     did not say"
                        .into(),
                    format!("base tests failed: `{command}` {digest}"),
                )),
                _ => {}
            }
            out
        }
    }
}

/// `tests/test_login.py` from `FAILED tests/test_login.py::test_lockout - …`.
fn path_in(line: &str) -> Option<String> {
    let t = line
        .trim()
        .trim_start_matches("FAILED ")
        .trim_start_matches("FAIL ");
    let first = t.split_whitespace().next()?;
    let (path, _) = first.split_once("::")?;
    (path.contains('/') || path.contains('.')).then(|| path.to_string())
}

fn plural(n: u32, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

/// How the new tests did on the base: by name when this run wrote JUnit, by
/// file otherwise. A new test is a case in a placed file, or, where the
/// runner writes no file, a case absent from the base's tests on this change.
pub fn new_tests(
    on_base: &Run,
    on_change: Option<&Run>,
    placed: &[PlacedFile],
    change_wrote_junit: bool,
) -> NewTests {
    if matches!(on_base.outcome, RunOutcome::Skipped { .. }) {
        return NewTests::None;
    }
    if !on_base.wrote_junit() {
        return NewTests::ByFile {
            passed: on_base.outcome == RunOutcome::Passed,
            why: change_wrote_junit.then(|| "this run wrote no JUnit".to_string()),
        };
    }
    let known: std::collections::HashSet<junit::CaseId> = on_change
        .map(|r| r.cases().into_iter().map(Case::id).collect())
        .unwrap_or_default();
    let base_ran = on_change.is_some_and(Run::wrote_junit);
    let in_placed = |file: &str| {
        placed
            .iter()
            .any(|p| p.path == file || p.path.ends_with(&format!("/{file}")))
    };
    let (mut passing, mut failing, mut skipped, mut unattributed) = (Vec::new(), Vec::new(), 0, 0);
    for c in on_base.cases() {
        let new = match &c.file {
            Some(f) => in_placed(f),
            None if base_ran => !known.contains(&c.id()),
            None => {
                unattributed += 1;
                continue;
            }
        };
        if !new {
            continue;
        }
        match c.outcome {
            CaseOutcome::Passed => passing.push(NewTest::of(c)),
            CaseOutcome::Failed | CaseOutcome::Error => failing.push(NewTest::of(c)),
            CaseOutcome::Skipped => skipped += 1,
        }
    }
    NewTests::Counted {
        passing,
        failing,
        skipped,
        unattributed,
    }
}

/// How a sentence is weighed on a surface: never the verified class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weight {
    Warn,
    Plain,
    Absent,
}

/// The base's tests on this change, in one sentence.
pub fn on_change_says(on_change: &OnChange) -> String {
    let base = match &on_change.run.outcome {
        RunOutcome::Passed => format!(
            "the base's tests pass on this change ({} put back)",
            plural(
                (on_change.defining.restored.len() + on_change.defining.removed.len()) as u32,
                "file",
                "files"
            )
        ),
        RunOutcome::Skipped { why } => why.clone(),
        RunOutcome::TimedOut { after_s } => {
            format!("timed out with the base's tests after {after_s}s")
        }
        RunOutcome::CouldNotRun { why } => {
            format!("could not be run with this change's files: {why}")
        }
        RunOutcome::Failed => {
            let rows = rows(on_change);
            let named = rows
                .iter()
                .filter(|r| r.matched.starts_with("base test "))
                .count() as u32;
            let counted: Option<u32> = on_change
                .run
                .commands
                .iter()
                .find(|c| !c.passed())
                .and_then(|c| {
                    let t = c.tests.tallies();
                    (!t.is_empty()).then(|| t.iter().map(|t| t.failed()).sum())
                });
            match counted.map(|n| n.max(named)).unwrap_or(named) {
                0 => "the base's tests fail on this change; how many is not known".into(),
                1 => "1 base test fails on this change".into(),
                n => format!("{n} base tests fail on this change"),
            }
        }
    };
    match on_change.defining.inline.len() {
        0 => base,
        n => format!(
            "{base} · {n} {} tests and code together; their tests ran as the change wrote them",
            match n {
                1 => "file holds",
                _ => "files hold",
            }
        ),
    }
}

pub fn on_change_weight(on_change: &OnChange) -> Weight {
    match on_change.run.outcome {
        RunOutcome::Passed => Weight::Plain,
        RunOutcome::Skipped { .. } => Weight::Absent,
        _ => Weight::Warn,
    }
}

/// This change's new tests on the base, in one sentence.
pub fn on_base_says(on_base: &OnBase) -> String {
    let files = on_base.placement.placed.len() as u32;
    let mut out = match (&on_base.run.outcome, &on_base.counted) {
        (RunOutcome::Skipped { why }, _) => return why.clone(),
        (RunOutcome::TimedOut { after_s }, _) => {
            format!("timed out without the change after {after_s}s")
        }
        (RunOutcome::CouldNotRun { why }, _) => {
            format!("could not be run without the change: {why}")
        }
        (
            _,
            NewTests::Counted {
                passing,
                failing,
                skipped,
                unattributed,
            },
        ) => {
            let mut parts: Vec<String> = Vec::new();
            match passing.len() {
                0 => {}
                1 => parts.push("1 new test passes without the change".into()),
                n => parts.push(format!("{n} new tests pass without the change")),
            }
            if !failing.is_empty() {
                parts.push(match parts.is_empty() {
                    true => format!(
                        "{} without the change",
                        match failing.len() {
                            1 => "1 new test fails".to_string(),
                            n => format!("{n} new tests fail"),
                        }
                    ),
                    false => format!("{} fail without it", failing.len()),
                });
            }
            if *skipped > 0 {
                parts.push(format!("{skipped} skipped"));
            }
            if *unattributed > 0 {
                parts.push(format!(
                    "{} could not be attributed",
                    plural(*unattributed, "test", "tests")
                ));
            }
            match parts.is_empty() {
                true => "no new test was named".into(),
                false => parts.join(" · "),
            }
        }
        (_, NewTests::ByFile { passed, why }) => {
            let s = match passed {
                true => match files {
                    1 => "1 new test file passes without the change".to_string(),
                    n => format!("{n} new test files pass without the change"),
                },
                false => format!(
                    "fails without the change ({}) — which tests, the runner wrote no JUnit to say",
                    plural(files, "test file", "test files")
                ),
            };
            match why {
                Some(w) if *passed => format!("{s} — {w}"),
                _ => s,
            }
        }
        _ => "no new test was named".into(),
    };
    for p in &on_base.placement.placed {
        if let Some(from) = &p.renamed_from {
            out.push_str(&format!(" · `{}` renamed from `{from}`", p.path));
        }
    }
    if !on_base.placement.not_separable.is_empty() {
        out.push_str(&format!(
            " · {}",
            not_separable(on_base.placement.not_separable.len())
        ));
    }
    out
}

pub fn on_base_weight(on_base: &OnBase) -> Weight {
    match (&on_base.run.outcome, &on_base.counted) {
        (RunOutcome::Skipped { .. }, _) if !on_base.placement.not_separable.is_empty() => {
            Weight::Warn
        }
        (RunOutcome::Skipped { .. }, _) => Weight::Absent,
        (_, NewTests::Counted { passing, .. }) if !passing.is_empty() => Weight::Warn,
        (_, NewTests::ByFile { passed: true, .. }) => Weight::Warn,
        (RunOutcome::TimedOut { .. } | RunOutcome::CouldNotRun { .. }, _) => Weight::Warn,
        _ => Weight::Plain,
    }
}

/// How many names a surface lists before folding.
pub const FOLD_NAMED: usize = 20;
pub const FOLD_NEW: usize = 50;

/// A list as a surface shows it: the first `at`, and how many more.
fn fold<T: Clone>(items: &[T], at: usize) -> (Vec<T>, usize) {
    (
        items.iter().take(at).cloned().collect(),
        items.len().saturating_sub(at),
    )
}

/// Why there is no current result, with its sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbsentView {
    pub absent: Absent,
    pub says: String,
}

/// The named differences, folded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedView {
    pub not_run_on_change: Vec<String>,
    pub not_run_on_change_more: usize,
    pub became_skipped: Vec<String>,
    pub became_skipped_more: usize,
    pub changed: Vec<String>,
    pub changed_more: usize,
    pub new: Vec<String>,
    pub new_more: usize,
    pub says: String,
}

/// The base's tests on this change, as every surface shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnChangeView {
    pub says: String,
    pub weight: Weight,
    pub outcome: RunOutcome,
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    pub inline: Vec<String>,
    pub commands: Vec<CommandResult>,
    /// The failing tests, as rows name them.
    pub failing: Vec<String>,
    pub failing_more: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named: Option<NamedView>,
    pub junit_notes: Vec<String>,
}

/// This change's new tests on the base, as every surface shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnBaseView {
    pub says: String,
    pub weight: Weight,
    pub outcome: RunOutcome,
    pub placed: Vec<PlacedFile>,
    pub not_separable: Vec<String>,
    pub commands: Vec<CommandResult>,
    pub passing: Vec<String>,
    pub passing_more: usize,
    pub failing: Vec<String>,
    pub failing_more: usize,
}

/// Base checks as the header, the Gates view, `change show` and the
/// certificate show them: one computation, so they cannot disagree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The header's line: *ran 12 → 3 · deselected 0 → 9 · pytest*, or the
    /// count beside why base checks have not run.
    pub counts_says: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absent: Option<AbsentView>,
    /// The runs shown describe an earlier tree, base or definition.
    pub stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_change: Option<OnChangeView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_base: Option<OnBaseView>,
    /// Whether *Run base checks* can do something now.
    pub can_run: bool,
    /// What the change's own `check` JUnit came to, when it did not name
    /// tests or git does not ignore it.
    pub junit_notes: Vec<String>,
    /// The certificate's counts sentence.
    pub certificate: String,
}

impl View {
    /// `None` when `check` has never run on the change: there is nothing to
    /// count and nothing to compare. `config` is the governing one (`None`
    /// when unreadable); `running` says a pair for this change is in flight.
    pub fn of(change: &Change, config: Option<&ProjectConfig>, running: bool) -> Option<Self> {
        let check = change.check_report()?;
        let key = match config {
            Some(c) => key_now(change, c),
            None => Err(Absent::NoGates),
        };
        Some(Self::compose(
            check,
            key,
            change.base_checks.as_ref(),
            running,
        ))
    }

    /// For a pair that just ran against `check`'s report, outside any
    /// change (`devplane verify`): current by construction.
    pub fn of_result(check: &GateReport, result: &BaseChecks) -> Self {
        Self::compose(check, Ok(result.key.clone()), Some(result), false)
    }

    /// For a `check` report no pair ran against, and why.
    pub fn absent(check: &GateReport, why: Absent) -> Self {
        Self::compose(check, Err(why), None, false)
    }

    fn compose(
        check: &GateReport,
        key: Result<Key, Absent>,
        stored: Option<&BaseChecks>,
        running: bool,
    ) -> Self {
        let is_current = matches!((&key, stored), (Ok(k), Some(b)) if b.key == *k);
        let absent = match (&key, is_current) {
            (_, true) => None,
            (Err(a), _) if running && a.allows_a_run() => Some(Absent::Running),
            (Err(a), _) => Some(*a),
            (Ok(_), false) if running => Some(Absent::Running),
            (Ok(_), false) if stored.is_some() => Some(Absent::Stale),
            (Ok(_), false) => Some(Absent::NotRun),
        };
        let stale = stored.is_some() && !is_current;
        let on_change_cmds = stored
            .filter(|_| is_current)
            .filter(|b| !matches!(b.on_change.run.outcome, RunOutcome::Skipped { .. }))
            .map(|b| b.on_change.run.commands.as_slice());
        let left = crate::core::testcount::compare::says(&check.commands, on_change_cmds);
        let counts_says = match absent {
            Some(a) => format!("{left} · {}", a.says()),
            None => left,
        };
        let certificate = crate::core::testcount::compare::certificate_sentence(
            &check.commands,
            stored
                .filter(|_| is_current)
                .zip(on_change_cmds)
                .map(|(b, cmds)| (cmds, b.key.base.as_str())),
        );
        View {
            counts_says,
            absent: absent.map(|a| AbsentView {
                absent: a,
                says: a.says().to_string(),
            }),
            stale,
            base: stored.map(|b| b.key.base.clone()),
            tree: stored.map(|b| b.key.tree.clone()),
            at: stored.map(|b| b.at),
            on_change: stored.map(|b| on_change_view(&b.on_change)),
            on_base: stored.map(|b| on_base_view(&b.on_base)),
            can_run: key.is_ok() && !is_current && !running,
            junit_notes: check.junit.iter().flat_map(JunitFile::notes).collect(),
            certificate,
        }
    }
}

fn on_change_view(o: &OnChange) -> OnChangeView {
    let failing: Vec<String> = rows(o)
        .iter()
        .filter_map(|r| r.matched.strip_prefix("base test ").map(str::to_string))
        .collect();
    let (failing, failing_more) = fold(&failing, FOLD_NAMED);
    let ids = |v: &[junit::CaseId]| v.iter().map(junit::CaseId::says).collect::<Vec<_>>();
    OnChangeView {
        says: on_change_says(o),
        weight: on_change_weight(o),
        outcome: o.run.outcome.clone(),
        restored: o.defining.restored.clone(),
        removed: o.defining.removed.clone(),
        inline: o.defining.inline.clone(),
        commands: o.run.commands.clone(),
        failing,
        failing_more,
        junit_notes: o.run.junit.iter().flat_map(JunitFile::notes).collect(),
        named: o.named.as_ref().map(|n| {
            let (not_run_on_change, not_run_on_change_more) =
                fold(&ids(&n.not_run_on_change), FOLD_NAMED);
            let (became_skipped, became_skipped_more) = fold(&ids(&n.became_skipped), FOLD_NAMED);
            let changed_all: Vec<String> = n
                .changed
                .iter()
                .map(|(id, a, b)| format!("{} ({} → {})", id.says(), a.as_str(), b.as_str()))
                .collect();
            let (changed, changed_more) = fold(&changed_all, FOLD_NAMED);
            let (new, new_more) = fold(&ids(&n.new), FOLD_NAMED);
            NamedView {
                not_run_on_change,
                not_run_on_change_more,
                became_skipped,
                became_skipped_more,
                changed,
                changed_more,
                new,
                new_more,
                says: n.says(),
            }
        }),
    }
}

fn on_base_view(o: &OnBase) -> OnBaseView {
    let names = |v: &[NewTest]| v.iter().map(NewTest::says).collect::<Vec<_>>();
    let (passing, failing) = match &o.counted {
        NewTests::Counted {
            passing, failing, ..
        } => (names(passing), names(failing)),
        _ => (Vec::new(), Vec::new()),
    };
    let (passing, passing_more) = fold(&passing, FOLD_NEW);
    let (failing, failing_more) = fold(&failing, FOLD_NEW);
    OnBaseView {
        says: on_base_says(o),
        weight: on_base_weight(o),
        outcome: o.run.outcome.clone(),
        placed: o.placement.placed.clone(),
        not_separable: o.placement.not_separable.clone(),
        commands: o.run.commands.clone(),
        passing,
        passing_more,
        failing,
        failing_more,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::{Body, FileChange, Hunk};
    use crate::core::testcount::junit::{JunitFile, JunitRead};
    use crate::core::testcount::{CommandCount, Runner, Tally};

    fn file(path: &str, status: Status, added: &[&str]) -> FileChange {
        FileChange {
            path: path.into(),
            status,
            added: added.len() as u32,
            removed: 0,
            body: Body::Hunks(vec![Hunk {
                header: "@@ -1 +1 @@".into(),
                lines: added.iter().map(|l| (Kind::Added, l.to_string())).collect(),
            }]),
        }
    }

    fn set(files: Vec<FileChange>) -> ChangeSet {
        ChangeSet {
            base: "main".into(),
            files,
            truncated: None,
            whole: None,
        }
    }

    fn roles(tests: &[&str]) -> Roles {
        Roles {
            tests: tests.iter().map(|s| s.to_string()).collect(),
            ..Roles::default()
        }
    }

    #[test]
    fn what_defines_the_checks_is_put_back_and_what_the_change_added_removed() {
        let s = set(vec![
            file("tests/test_a.py", Status::Modified, &["assert True"]),
            file("scripts/test.sh", Status::Modified, &["pytest -k a"]),
            file(
                "tests/test_new.py",
                Status::Added,
                &["def test_new(): pass"],
            ),
            file("tests/test_old.py", Status::Deleted, &[]),
            file(
                "tests/b_test.py",
                Status::Renamed {
                    from: "tests/b.py".into(),
                },
                &[],
            ),
            file("pyproject.toml", Status::Modified, &["x = 1"]),
            file("src/lib.rs", Status::Modified, &["#[test]", "fn t() {}"]),
            file("src/app.py", Status::Modified, &["x = 2"]),
        ]);
        let r = roles(&["tests/**"]);
        let d = defining(&s, Some(&r), &["sh ./scripts/test.sh".into()]);
        assert_eq!(
            d.restored,
            [
                "pyproject.toml",
                "scripts/test.sh",
                "tests/b.py",
                "tests/test_a.py",
                "tests/test_old.py"
            ]
        );
        assert_eq!(d.removed, ["tests/b_test.py", "tests/test_new.py"]);
        assert_eq!(
            d.inline,
            ["src/lib.rs"],
            "an inline test is listed, not put back"
        );
    }

    #[test]
    fn without_a_role_the_scans_own_fallback_decides() {
        let s = set(vec![
            file("tests/x_test.py", Status::Modified, &["assert 1"]),
            file("src/x.py", Status::Modified, &["y"]),
        ]);
        let d = defining(&s, None, &[]);
        assert_eq!(d.restored, ["tests/x_test.py"]);
    }

    #[test]
    fn no_differing_defining_path_is_an_empty_defining() {
        let s = set(vec![file("src/x.py", Status::Modified, &["y"])]);
        assert!(defining(&s, Some(&roles(&["tests/**"])), &[]).is_empty());
    }

    #[test]
    fn placement_takes_the_role_and_never_guesses() {
        let s = set(vec![
            file("tests/test_a.py", Status::Added, &["def test_a(): pass"]),
            file("tests/test_b.py", Status::Modified, &["x"]),
            file("tests/test_c.py", Status::Deleted, &[]),
            file(
                "tests/test_d.py",
                Status::Renamed {
                    from: "tests/d.py".into(),
                },
                &[],
            ),
            file(
                "src/routes/v1.ts",
                Status::Modified,
                &["test('drops v1', () => {})"],
            ),
            file("src/lib.rs", Status::Modified, &["#[cfg(test)]"]),
        ]);
        let p = placement(&s, Some(&roles(&["tests/**"])));
        let paths: Vec<&str> = p.placed.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            ["tests/test_a.py", "tests/test_b.py", "tests/test_d.py"]
        );
        assert_eq!(p.placed[2].renamed_from.as_deref(), Some("tests/d.py"));
        assert_eq!(
            p.not_separable,
            ["src/routes/v1.ts"],
            "#[cfg(test)] alone declares nothing"
        );
        assert!(placement(&s, None).no_role);
        assert!(placement(&s, Some(&roles(&[]))).no_role);
        assert_eq!(
            nothing_to_place(&placement(&s, None)).as_deref(),
            Some("no tests role declared")
        );
    }

    #[test]
    fn role_patterns_are_matched_whole() {
        let s = set(vec![
            file("spec/a_spec.rb", Status::Added, &["it('x')"]),
            file("tests/keep.py", Status::Added, &["x"]),
        ]);
        let p = placement(&s, Some(&roles(&["spec/**", "!spec/a_spec.rb"])));
        assert!(p.placed.is_empty(), "{p:?}");
        assert_eq!(
            nothing_to_place(&p).as_deref(),
            Some("not separable — 1 file holds tests and code together")
        );
    }

    fn failed_run(cases: Vec<Case>, failures: Vec<&str>, summary: Option<u32>) -> Run {
        let mut c = CommandResult::exited("sh ./scripts/test.sh", 1);
        c.failures = failures.into_iter().map(String::from).collect();
        c.output_digest = "d1".into();
        if let Some(n) = summary {
            c.tests = CommandCount::Read {
                tallies: vec![Tally {
                    runner: Runner::Pytest,
                    counts: vec![("failed".into(), n), ("passed".into(), 11)],
                    ran: 11 + n,
                    lines: 1,
                }],
            };
        }
        Run {
            outcome: RunOutcome::Failed,
            commands: vec![c],
            junit: match cases.is_empty() {
                true => vec![],
                false => vec![JunitFile {
                    read: JunitRead::Read {
                        path: "r.xml".into(),
                        cases,
                        dropped: false,
                    },
                    ignored: Some(true),
                }],
            },
            setup: None,
        }
    }

    fn case(name: &str, file: Option<&str>, outcome: CaseOutcome) -> Case {
        Case {
            suite: "pytest".into(),
            class: "tests.test_login".into(),
            name: name.into(),
            nth: 0,
            file: file.map(String::from),
            outcome,
        }
    }

    fn on(run: Run) -> OnChange {
        OnChange {
            defining: Defining {
                restored: vec!["scripts/test.sh".into()],
                ..Defining::default()
            },
            run,
            named: None,
        }
    }

    #[test]
    fn a_failing_case_is_a_row_with_or_without_its_file() {
        let r = rows(&on(failed_run(
            vec![
                case(
                    "test_lockout",
                    Some("tests/test_login.py"),
                    CaseOutcome::Failed,
                ),
                case("test_reset", None, CaseOutcome::Error),
                case("test_ok", None, CaseOutcome::Passed),
            ],
            vec![],
            Some(2),
        )));
        assert_eq!(r.len(), 2, "{r:?}");
        assert!(r.iter().all(|w| w.kind == WeakKind::BaseTests));
        assert_eq!(r[0].path, "tests/test_login.py");
        assert_eq!(r[1].path, "pytest::tests.test_login");
        assert_eq!(
            r[0].matched,
            "base test pytest::tests.test_login::test_lockout"
        );
    }

    #[test]
    fn failure_lines_name_what_junit_did_not_and_the_rest_is_counted() {
        let r = rows(&on(failed_run(
            vec![],
            vec!["FAILED tests/test_login.py::test_lockout - assert"],
            Some(3),
        )));
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(r[0].path, "tests/test_login.py");
        assert_eq!(
            r[1].why,
            "check failed with the base's tests; 2 failures not named"
        );
        let unknown = rows(&on(failed_run(vec![], vec![], None)));
        assert_eq!(unknown.len(), 1);
        assert!(unknown[0].why.contains("how many, the runner did not say"));
        assert!(!unknown[0].why.contains(" 0"));
    }

    #[test]
    fn a_timeout_and_a_run_that_could_not_start_are_rows_a_pass_and_a_skip_are_not() {
        let t = on(Run::not_run(RunOutcome::TimedOut { after_s: 60 }));
        assert_eq!(rows(&t)[0].why, "timed out with the base's tests after 60s");
        let c = on(Run::not_run(RunOutcome::CouldNotRun {
            why: "setup failed".into(),
        }));
        assert!(rows(&c)[0].why.contains("setup failed"));
        assert!(rows(&on(Run::not_run(RunOutcome::Passed))).is_empty());
        assert!(
            rows(&on(Run::not_run(RunOutcome::Skipped {
                why: "the change edits no file that defines the checks".into()
            })))
            .is_empty()
        );
    }

    #[test]
    fn a_named_row_keeps_its_seen_key_across_runs_with_different_output() {
        let a = failed_run(
            vec![case("test_lockout", None, CaseOutcome::Failed)],
            vec![],
            Some(1),
        );
        let mut b = a.clone();
        b.commands[0].output_digest = "d2".into();
        b.commands[0].output_tail = "other".into();
        assert_eq!(rows(&on(a))[0].matched, rows(&on(b))[0].matched);
    }

    #[test]
    fn the_sentences_say_what_happened_and_never_a_ratio() {
        let one = on(failed_run(
            vec![case("test_lockout", None, CaseOutcome::Failed)],
            vec![],
            Some(1),
        ));
        assert_eq!(on_change_says(&one), "1 base test fails on this change");
        assert_eq!(on_change_weight(&one), Weight::Warn);
        let pass = on(Run::not_run(RunOutcome::Passed));
        assert_eq!(
            on_change_says(&pass),
            "the base's tests pass on this change (1 file put back)"
        );
        let mut inline = pass.clone();
        inline.defining.inline = vec!["src/lib.rs".into(), "src/x.rs".into()];
        assert!(on_change_says(&inline).ends_with(
            "2 files hold tests and code together; their tests ran as the change wrote them"
        ));
    }

    fn base_run(outcome: RunOutcome, cases: Vec<Case>) -> Run {
        Run {
            outcome,
            commands: vec![],
            junit: match cases.is_empty() {
                true => vec![],
                false => vec![JunitFile {
                    read: JunitRead::Read {
                        path: "r.xml".into(),
                        cases,
                        dropped: false,
                    },
                    ignored: Some(true),
                }],
            },
            setup: None,
        }
    }

    fn placed(path: &str) -> PlacedFile {
        PlacedFile {
            path: path.into(),
            renamed_from: None,
            blob: Some("b1".into()),
        }
    }

    #[test]
    fn new_tests_are_attributed_by_file_else_by_absence_from_the_base_run() {
        let by_file = new_tests(
            &base_run(
                RunOutcome::Failed,
                vec![
                    case("test_new", Some("tests/test_rate.py"), CaseOutcome::Passed),
                    case(
                        "test_newer",
                        Some("tests/test_rate.py"),
                        CaseOutcome::Failed,
                    ),
                    case(
                        "test_skip",
                        Some("tests/test_rate.py"),
                        CaseOutcome::Skipped,
                    ),
                    case("test_old", Some("tests/test_login.py"), CaseOutcome::Passed),
                ],
            ),
            None,
            &[placed("tests/test_rate.py")],
            true,
        );
        let NewTests::Counted {
            passing,
            failing,
            skipped,
            unattributed,
        } = &by_file
        else {
            panic!("{by_file:?}")
        };
        assert_eq!(
            (passing.len(), failing.len(), *skipped, *unattributed),
            (1, 1, 1, 0)
        );

        let first = base_run(
            RunOutcome::Passed,
            vec![case("test_old", None, CaseOutcome::Passed)],
        );
        let by_absence = new_tests(
            &base_run(
                RunOutcome::Passed,
                vec![
                    case("test_old", None, CaseOutcome::Passed),
                    case("test_new", None, CaseOutcome::Passed),
                ],
            ),
            Some(&first),
            &[placed("tests/test_rate.py")],
            true,
        );
        assert!(
            matches!(&by_absence, NewTests::Counted { passing, .. } if passing.len() == 1 && passing[0].name == "test_new")
        );
        let none_to_compare = new_tests(
            &base_run(
                RunOutcome::Passed,
                vec![case("t", None, CaseOutcome::Passed)],
            ),
            None,
            &[placed("tests/test_rate.py")],
            true,
        );
        assert!(matches!(
            none_to_compare,
            NewTests::Counted {
                unattributed: 1,
                ..
            }
        ));
    }

    fn ob(outcome: RunOutcome, counted: NewTests, placed_files: Vec<PlacedFile>) -> OnBase {
        OnBase {
            placement: Placement {
                placed: placed_files,
                not_separable: vec![],
                no_role: false,
            },
            run: base_run(outcome, vec![]),
            counted,
        }
    }

    #[test]
    fn every_new_tests_sentence_omits_its_zero_parts() {
        let t = |n: &str| NewTest {
            suite: "s".into(),
            class: "c".into(),
            name: n.into(),
            file: None,
        };
        let counted = |p: usize, f: usize, s: u32, u: u32| NewTests::Counted {
            passing: (0..p).map(|i| t(&format!("p{i}"))).collect(),
            failing: (0..f).map(|i| t(&format!("f{i}"))).collect(),
            skipped: s,
            unattributed: u,
        };
        let says = |c: NewTests| on_base_says(&ob(RunOutcome::Failed, c, vec![placed("t.py")]));
        assert_eq!(
            says(counted(3, 2, 0, 0)),
            "3 new tests pass without the change · 2 fail without it"
        );
        assert_eq!(
            says(counted(1, 0, 0, 0)),
            "1 new test passes without the change"
        );
        assert_eq!(
            says(counted(0, 2, 0, 0)),
            "2 new tests fail without the change"
        );
        assert_eq!(
            says(counted(0, 1, 1, 2)),
            "1 new test fails without the change · 1 skipped · 2 tests could not be attributed"
        );
        let by_file = |passed, why: Option<&str>| NewTests::ByFile {
            passed,
            why: why.map(String::from),
        };
        assert_eq!(
            on_base_says(&ob(
                RunOutcome::Passed,
                by_file(true, None),
                vec![placed("t.py")]
            )),
            "1 new test file passes without the change"
        );
        assert_eq!(
            on_base_says(&ob(
                RunOutcome::Failed,
                by_file(false, None),
                vec![placed("a.py"), placed("b.py")]
            )),
            "fails without the change (2 test files) — which tests, the runner wrote no JUnit to say"
        );
        assert!(
            on_base_says(&ob(
                RunOutcome::Passed,
                by_file(true, Some("this run wrote no JUnit")),
                vec![placed("t.py")]
            ))
            .ends_with("this run wrote no JUnit")
        );
        let mut renamed = placed("tests/new.py");
        renamed.renamed_from = Some("tests/old.py".into());
        assert!(
            on_base_says(&ob(RunOutcome::Passed, by_file(true, None), vec![renamed]))
                .contains("`tests/new.py` renamed from `tests/old.py`")
        );
        assert_eq!(
            on_base_weight(&ob(
                RunOutcome::Passed,
                by_file(true, None),
                vec![placed("t.py")]
            )),
            Weight::Warn
        );
        assert_eq!(
            on_base_weight(&ob(
                RunOutcome::Failed,
                by_file(false, None),
                vec![placed("t.py")]
            )),
            Weight::Plain
        );
        for s in [
            says(counted(3, 2, 1, 1)),
            says(counted(12, 0, 0, 0)),
            on_change_says(&on(failed_run(vec![], vec![], Some(3)))),
        ] {
            assert!(!ratio(&s), "{s}");
        }
    }

    /// `/\d+\s*\/\s*\d+/`, `/\d\s*%/` or `/\d+ of \d+/`.
    fn ratio(s: &str) -> bool {
        let b = s.as_bytes();
        for i in 0..b.len() {
            if !b[i].is_ascii_digit() {
                continue;
            }
            let rest = s[i..].trim_start_matches(|c: char| c.is_ascii_digit());
            let t = rest.trim_start();
            if t.starts_with('%') {
                return true;
            }
            if let Some(after) = t.strip_prefix('/')
                && after.trim_start().starts_with(|c: char| c.is_ascii_digit())
            {
                return true;
            }
            if let Some(after) = rest.strip_prefix(" of ")
                && after.starts_with(|c: char| c.is_ascii_digit())
            {
                return true;
            }
        }
        false
    }
}

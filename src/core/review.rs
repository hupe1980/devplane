//! Reading a change: in the order being wrong costs most, with what is already
//! proven beside each file, and what nobody asked for under its own heading.
//! Every input is a declared or recorded fact (roles, test mapping, runs,
//! decisions); nothing is inferred from the code. No aggregate or score: the
//! one size sentence, [`Shape::says`], counts files, lines and roles only.

use crate::core::config::{Covers, Roles};
use crate::core::decision::Decision;
use crate::core::diff::{Body, ChangeSet, Hunk, Kind};
use crate::core::ids::RunId;
use crate::core::run::{Run, RunMode};
use crate::core::worktreeinclude::Patterns;
use crate::spec::SentTask;
use serde::{Deserialize, Serialize};

/// Where being wrong is expensive, most expensive first. The order is fixed;
/// which paths are which is declared in `[review.roles]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "wire/", rename = "ReviewRole")
)]
pub enum Role {
    Shared,
    Logic,
    Security,
    Integration,
    Wiring,
    Tests,
}

impl Role {
    pub const ALL: [Role; 6] = [
        Role::Shared,
        Role::Logic,
        Role::Security,
        Role::Integration,
        Role::Wiring,
        Role::Tests,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Shared => "shared",
            Role::Logic => "logic",
            Role::Security => "security",
            Role::Integration => "integration",
            Role::Wiring => "wiring",
            Role::Tests => "tests",
        }
    }

    pub fn says(self) -> &'static str {
        match self {
            Role::Shared => "shared surface",
            Role::Logic => "business logic",
            Role::Security => "security boundary",
            Role::Integration => "integration point",
            Role::Wiring => "wiring",
            Role::Tests => "tests and documentation",
        }
    }
}

/// A role's patterns are read as one gitignore file, so `!keep.rs` after
/// `src/**` excludes within the role.
fn compile(roles: &Roles) -> Vec<(Role, Patterns)> {
    Role::ALL
        .iter()
        .zip(roles.named())
        .map(|(role, (_, patterns))| (*role, Patterns::parse(&patterns.join("\n"))))
        .collect()
}

fn first_match(path: &str, compiled: &[(Role, Patterns)]) -> Option<Role> {
    compiled
        .iter()
        .find(|(_, p)| p.matches(path))
        .map(|(r, _)| *r)
}

/// The change's files in reading order: indices into `ChangeSet::files`,
/// grouped by role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ordered {
    /// Declared roles in order, then the undeclared (`None`); empty groups omitted.
    pub groups: Vec<(Option<Role>, Vec<usize>)>,
    /// No roles were declared, so this is path order and nothing more.
    pub unordered: bool,
}

impl Ordered {
    pub fn role(&self, file: usize) -> Option<Role> {
        self.groups
            .iter()
            .find(|(_, files)| files.contains(&file))
            .and_then(|(r, _)| *r)
    }
}

fn path_order(set: &ChangeSet) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..set.files.len()).collect();
    idx.sort_by(|a, b| set.files[*a].path.cmp(&set.files[*b].path));
    idx
}

/// Orders a change by the declared roles; path order when there are none.
/// A file matching no role is listed last rather than guessed into one.
pub fn order(set: &ChangeSet, roles: Option<&Roles>) -> Ordered {
    let by_path = path_order(set);
    let Some(roles) = roles else {
        return Ordered {
            groups: match by_path.is_empty() {
                true => Vec::new(),
                false => vec![(None, by_path)],
            },
            unordered: true,
        };
    };
    let compiled = compile(roles);
    let mut groups: Vec<(Option<Role>, Vec<usize>)> = Role::ALL
        .iter()
        .map(|r| (Some(*r), Vec::new()))
        .chain(std::iter::once((None, Vec::new())))
        .collect();
    for i in by_path {
        let role = first_match(&set.files[i].path, &compiled);
        if let Some(g) = groups.iter_mut().find(|(r, _)| *r == role) {
            g.1.push(i);
        }
    }
    groups.retain(|(_, files)| !files.is_empty());
    Ordered {
        groups,
        unordered: false,
    }
}

/// Whether a declared test covers a path. *Not covered* is a finding; *no
/// mapping* is the limit of what was declared, and must not render as either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "coverage", rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum Coverage {
    Covered { test: String },
    NotCovered,
    NoMapping,
}

impl Coverage {
    /// The per-file sentence; `None` for no mapping, which is said once per change.
    pub fn says(&self) -> Option<String> {
        match self {
            Coverage::Covered { test } => Some(format!("covered by `{test}`")),
            Coverage::NotCovered => Some("no declared test covers this path".into()),
            Coverage::NoMapping => None,
        }
    }
}

/// The first mapping whose paths match names the test.
pub fn coverage(path: &str, covers: Option<&[Covers]>) -> Coverage {
    let Some(covers) = covers.filter(|c| !c.is_empty()) else {
        return Coverage::NoMapping;
    };
    covers
        .iter()
        .find(|c| Patterns::parse(&c.paths.join("\n")).matches(path))
        .map_or(Coverage::NotCovered, |c| Coverage::Covered {
            test: c.test.clone(),
        })
}

/// Whether a hunk changes nothing but whitespace: removed and added lines,
/// whitespace stripped and blanks dropped, are the same sequence (so a
/// reordered import is a change). Where indentation is syntax (Python, YAML,
/// Makefile) a hunk whose leading whitespace changed is never collapsed.
pub fn formatter_only(path: &str, hunk: &Hunk) -> bool {
    let squeezed = |want: Kind| -> Vec<String> {
        hunk.lines
            .iter()
            .filter(|(k, _)| *k == want)
            .map(|(_, t)| t.chars().filter(|c| !c.is_whitespace()).collect::<String>())
            .filter(|t| !t.is_empty())
            .collect()
    };
    let touched = hunk.lines.iter().any(|(k, _)| *k != Kind::Context);
    if !(touched && squeezed(Kind::Removed) == squeezed(Kind::Added)) {
        return false;
    }
    !(indentation_is_syntax(path) && indentation_changed(hunk))
}

fn indentation_is_syntax(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "Makefile"
        || name == "GNUmakefile"
        || name.ends_with(".mk")
        || [".py", ".pyi", ".yaml", ".yml"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

fn indentation_changed(hunk: &Hunk) -> bool {
    let leads = |want: Kind| -> Vec<String> {
        hunk.lines
            .iter()
            .filter(|(k, t)| *k == want && !t.trim().is_empty())
            .map(|(_, t)| t.chars().take_while(|c| c.is_whitespace()).collect())
            .collect()
    };
    leads(Kind::Removed) != leads(Kind::Added)
}

pub fn hunks_of(body: &Body) -> &[Hunk] {
    match body {
        Body::Hunks(h) => h,
        _ => &[],
    }
}

/// One dispatched run's share of the change: the tasks it was sent and the
/// changed files it wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntentGroup {
    pub run: RunId,
    pub tasks: Vec<SentTask>,
    /// Indices into `ChangeSet::files`, in path order.
    pub files: Vec<usize>,
}

/// Why a changed file sits under *not asked for*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "why", rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum NotAskedFor {
    /// A hand edit, a watched session, a generated file.
    NoRunWrote,
    /// A dispatched run wrote it, and that run was sent no tasks.
    RunSentNoTasks { run: RunId },
}

impl NotAskedFor {
    pub fn says(&self) -> String {
        match self {
            NotAskedFor::NoRunWrote => "no run Devplane dispatched wrote this".into(),
            NotAskedFor::RunSentNoTasks { run } => {
                format!("written by run {run} which was sent no tasks")
            }
        }
    }
}

/// The change grouped by the run that wrote each file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Intent {
    pub groups: Vec<IntentGroup>,
    pub not_asked_for: Vec<(usize, NotAskedFor)>,
    /// Why grouping cannot be drawn at all, as a printable sentence.
    pub unavailable: Option<String>,
}

/// Groups a change by the run that wrote each file. `runs` is oldest first,
/// each with the files it wrote. A file belongs to the newest dispatched run
/// that wrote it and was sent tasks; otherwise it is *not asked for* with the
/// reason. With no dispatched run, grouping is unavailable rather than guessed.
pub fn by_intent(set: &ChangeSet, runs: &[(&Run, Vec<String>)]) -> Intent {
    let dispatched: Vec<&(&Run, Vec<String>)> = runs
        .iter()
        .filter(|(r, _)| r.mode != RunMode::Observed)
        .collect();
    let unavailable = match (runs.is_empty(), dispatched.is_empty()) {
        (true, _) => Some("no run has worked on this change, so grouping by task is unavailable"),
        (false, true) => Some(
            "every run on this change was watched, not dispatched by Devplane, so grouping by \
             task is unavailable",
        ),
        _ => None,
    };
    if let Some(why) = unavailable {
        return Intent {
            groups: Vec::new(),
            not_asked_for: Vec::new(),
            unavailable: Some(why.to_string()),
        };
    }

    let mut groups: Vec<IntentGroup> = dispatched
        .iter()
        .filter_map(|(r, _)| {
            r.sent.as_ref().map(|tasks| IntentGroup {
                run: r.id.clone(),
                tasks: tasks.clone(),
                files: Vec::new(),
            })
        })
        .collect();
    let mut not_asked_for = Vec::new();
    for i in path_order(set) {
        let path = &set.files[i].path;
        let writers: Vec<&Run> = dispatched
            .iter()
            .filter(|(_, wrote)| wrote.iter().any(|w| w == path))
            .map(|(r, _)| *r)
            .collect();
        let home = writers
            .iter()
            .rev()
            .find_map(|r| groups.iter().position(|g| g.run == r.id));
        match (home, writers.last()) {
            (Some(g), _) => groups[g].files.push(i),
            (None, Some(r)) => {
                not_asked_for.push((i, NotAskedFor::RunSentNoTasks { run: r.id.clone() }))
            }
            (None, None) => not_asked_for.push((i, NotAskedFor::NoRunWrote)),
        }
    }
    Intent {
        groups,
        not_asked_for,
        unavailable: None,
    }
}

/// The decisions whose subject names a path as a whole, not a substring:
/// `a.rs` is not named by `data.rs`, but is by `/abs/a.rs`.
pub fn decisions_for<'a>(path: &str, decisions: &'a [Decision]) -> Vec<&'a Decision> {
    if path.is_empty() {
        return Vec::new();
    }
    decisions
        .iter()
        .filter(|d| names_path(&d.subject, path))
        .collect()
}

fn names_path(subject: &str, path: &str) -> bool {
    let name_char = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.');
    subject.match_indices(path).any(|(at, _)| {
        let before = subject[..at].chars().next_back();
        let after = subject[at + path.len()..].chars().next();
        before.is_none_or(|c| !name_char(c)) && after.is_none_or(|c| !(name_char(c) || c == '/'))
    })
}

/// Which kind of alteration a weakened row is. Counted by kind, never scored:
/// a skipped test and an edited CI workflow ask different questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum WeakKind {
    /// A check turned off, loosened or silenced: a skip marker, an assertion
    /// removed or made to always pass, a tolerance changed, a suppression added.
    Skip,
    /// A test removed: its whole file, or a test inside a file that stays.
    Deleted,
    /// What *passing* means, edited: the gates, CI, a runner's, linter's or
    /// coverage tool's configuration, a script a gate runs, or a failure
    /// masked in one of those.
    Gate,
}

/// One hunk or file that alters a check, with why. A signal from the diff
/// text alone, never a verdict: it says what it matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Weakened {
    pub path: String,
    pub why: String,
    pub kind: WeakKind,
    /// What it matched: the added or removed line, trimmed; the deleted path;
    /// or, for an edit to a whole definition, a digest of its changed lines.
    /// Part of a seen mark's key, so a different text is a different row.
    pub matched: String,
}

/// What every surface says when a change's diff could not be read.
pub const UNREADABLE: &str = "the diff could not be read";

/// The rows a change's diff weakened, counted by kind, with how many no person
/// has marked seen. Beside *verified* on every surface; derived from the same
/// rows the review's first group shows, so the two cannot disagree.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Qualifier {
    /// Rows of kind [`WeakKind::Skip`].
    pub weakened: u32,
    /// Rows of kind [`WeakKind::Deleted`].
    pub deleted: u32,
    /// Rows of kind [`WeakKind::Gate`].
    pub gates_changed: u32,
    /// Rows with no current seen mark.
    pub unseen: u32,
    /// *1 check weakened · 1 gate changed*, or empty when nothing was.
    pub says: String,
}

impl Qualifier {
    /// Pure: `seen` holds `(path, matched)` for every row a person marked.
    pub fn of(rows: &[Weakened], seen: &std::collections::HashSet<(String, String)>) -> Self {
        let count = |k: WeakKind| rows.iter().filter(|w| w.kind == k).count() as u32;
        let mut q = Qualifier {
            weakened: count(WeakKind::Skip),
            deleted: count(WeakKind::Deleted),
            gates_changed: count(WeakKind::Gate),
            unseen: rows
                .iter()
                .filter(|w| !seen.contains(&(w.path.clone(), w.matched.clone())))
                .count() as u32,
            says: String::new(),
        };
        q.says = q.compose();
        q
    }

    /// For a checkout whose diff could not be read: no counts, and a sentence
    /// saying so, so *verified* never stands alone on an unread diff.
    pub fn unreadable() -> Self {
        Qualifier {
            says: UNREADABLE.to_string(),
            ..Qualifier::default()
        }
    }

    /// Nothing weakened, deleted or changed.
    pub fn is_empty(&self) -> bool {
        self.weakened + self.deleted + self.gates_changed == 0
    }

    fn compose(&self) -> String {
        let part = |n: u32, one: &str, many: &str| match n {
            0 => None,
            1 => Some(format!("1 {one}")),
            n => Some(format!("{n} {many}")),
        };
        [
            part(self.weakened, "check weakened", "checks weakened"),
            part(self.deleted, "test deleted", "tests deleted"),
            part(self.gates_changed, "gate changed", "gates changed"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
    }
}

/// What the review of a change *ready to decide* would lead with, carried on
/// its inbox row so the row never reads as calmer than the review. Hunks seen
/// are the window's own marks, so the host sends only the total.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct ReadyFacts {
    pub qualifier: Qualifier,
    /// Every hunk in the change.
    pub hunks: u32,
    /// Files no declared test covers; `None` when no mapping is declared.
    pub not_covered: Option<u32>,
    /// Hunks in files no dispatched task asked for; `None` when grouping by
    /// task is unavailable.
    pub not_asked_for: Option<u32>,
    /// The two counts in words, or why each is unknown: *1 file covered by
    /// nothing*, *no test mapping declared*.
    pub says: Vec<String>,
}

impl ReadyFacts {
    pub fn of(
        qualifier: Qualifier,
        hunks: u32,
        not_covered: Option<u32>,
        not_asked_for: Option<u32>,
    ) -> Self {
        let n = |n: u32, one: &str, many: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {many}"),
        };
        let says = vec![
            match not_covered {
                Some(c) => n(c, "file covered by nothing", "files covered by nothing"),
                None => "no test mapping declared".into(),
            },
            match not_asked_for {
                Some(c) => n(c, "hunk not asked for", "hunks not asked for"),
                None => "not grouped by task".into(),
            },
        ];
        ReadyFacts {
            qualifier,
            hunks,
            not_covered,
            not_asked_for,
            says,
        }
    }
}

/// Markers that turn a test off or narrow a run. Matched in added lines only.
pub const SKIP_MARKERS: &[&str] = &[
    "#[ignore",
    "@skip",
    "@pytest.mark.skip",
    "pytest.skip(",
    "xfail",
    // Whole spellings: a bare `.skip(` is also every Rust iterator.
    "it.skip(",
    "test.skip(",
    "describe.skip(",
    "it.only(",
    "test.only(",
    "describe.only(",
    "it.todo",
    "t.Skip(",
    "@Disabled",
    "@Ignore",
];

/// Comments that silence a linter or a type checker. Matched in added lines
/// of any file but prose, since documentation may name them.
pub const SUPPRESSIONS: &[&str] = &[
    "@ts-ignore",
    "@ts-expect-error",
    "@ts-nocheck",
    "eslint-disable",
    "# noqa",
    "# type: ignore",
    "#[allow(",
    "#![allow(",
    "//nolint",
    "// nolint",
    "pylint: disable",
];

/// What makes a failing step pass anyway. Matched in added lines of what
/// defines the checks: CI, the gates, a script a gate runs.
pub const MASKS: &[&str] = &[
    "|| true",
    "|| exit 0",
    "continue-on-error: true",
    "--skip",
    "-k \"not",
    "-k 'not",
];

/// Where a tolerance is written. A changed line carrying one is flagged; the
/// value's direction is not read.
const TOLERANCES: &[&str] = &[
    "approx(",
    "rel=",
    "abs=",
    "epsilon",
    "places=",
    "atol=",
    "rtol=",
    "delta=",
    "toBeCloseTo(",
    "assert_approx_eq",
    "assertAlmostEqual",
    "InDelta(",
    "InEpsilon(",
];

/// Where tests live when the project declared no `tests` role.
const TEST_DIRS: &[&str] = &["tests/", "test/", "__tests__/", "spec/"];

/// A test file by its name, when the project declared no `tests` role.
fn test_named(name: &str) -> bool {
    (name.starts_with("test_") && name.ends_with(".py"))
        || name.ends_with("_test.py")
        || name.ends_with("_test.go")
        || name.ends_with("_spec.rb")
        || name.contains(".test.")
        || name.contains(".spec.")
}

/// Documentation, where a marker is a word about a check, not a check.
fn is_prose(name: &str) -> bool {
    [".md", ".mdx", ".rst", ".txt", ".adoc"]
        .iter()
        .any(|e| name.ends_with(e))
}

/// What defines the checks: a hunk here changes what *passing* means.
fn defines_checks(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let s = |t: &str| Some(t.to_string());
    if path == "devplane.toml" {
        s("edits the gates' own definition (`devplane.toml`)")
    } else if path.starts_with(".github/workflows/") {
        s("edits a CI workflow")
    } else if path == ".gitlab-ci.yml" {
        s("edits the CI definition (`.gitlab-ci.yml`)")
    } else if name == "pyproject.toml" {
        s("edits `pyproject.toml`, which configures the test runner")
    } else if name == "conftest.py" {
        s("edits `conftest.py`, which configures the test run")
    } else if name.starts_with("jest.config.") || name.starts_with("vitest.config.") {
        s("edits the test runner's configuration")
    } else if matches!(name, "justfile" | "Justfile" | ".justfile") {
        Some(format!("edits `{name}`, whose recipes run the checks"))
    } else if matches!(name, "Makefile" | "makefile" | "GNUmakefile") {
        Some(format!("edits `{name}`, whose targets run the checks"))
    } else if name == ".pre-commit-config.yaml" {
        s("edits `.pre-commit-config.yaml`, which runs the checks before a commit")
    } else if matches!(name, "tox.ini" | "setup.cfg" | "pytest.ini") {
        Some(format!("edits `{name}`, which configures the test runner"))
    } else if matches!(
        name,
        "clippy.toml" | ".clippy.toml" | "deny.toml" | "rustfmt.toml" | ".rustfmt.toml"
    ) || name.starts_with(".eslintrc")
        || name.starts_with("eslint.config.")
    {
        Some(format!("edits `{name}`, which configures a linter"))
    } else if name.starts_with("tsconfig") && name.ends_with(".json") {
        Some(format!("edits `{name}`, which configures the type checker"))
    } else if matches!(name, "codecov.yml" | ".codecov.yml" | ".coveragerc") {
        Some(format!("edits `{name}`, which configures coverage"))
    } else {
        None
    }
}

/// The files a declared gate command names, relative to the project root:
/// `sh ./scripts/test.sh` names `scripts/test.sh`. Directories and flags are
/// not files.
fn files_gates_run(commands: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in commands {
        for t in c.split_whitespace() {
            let t = t.trim_matches(|c| c == '"' || c == '\'' || c == ';');
            let t = t.strip_prefix("./").unwrap_or(t);
            if t.is_empty()
                || t.starts_with('-')
                || t.ends_with('/')
                || t.contains('=')
                || t.contains('$')
                || !(t.contains('/') || t.contains('.'))
            {
                continue;
            }
            if !out.iter().any(|o| o == t) {
                out.push(t.to_string());
            }
        }
    }
    out
}

/// A line that is a comment, so a pattern in it asserts nothing.
fn commented(t: &str) -> bool {
    t.starts_with("//")
        || t.starts_with("/*")
        || t.starts_with('*')
        || (t.starts_with('#') && !t.starts_with("#["))
}

/// `needle` in `t` where it starts a word: `expect(` in `expect(x)`, not in
/// Rust's `.expect("…")`.
fn at_word(t: &str, needle: &str) -> bool {
    t.match_indices(needle).any(|(at, _)| {
        t[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.'))
    })
}

/// A line that asserts something, in any of the common spellings.
fn is_assertion(t: &str) -> bool {
    if commented(t) {
        return false;
    }
    t.starts_with("assert ")
        || [
            "assert!(",
            "assert_eq!(",
            "assert_ne!(",
            "self.assert",
        ]
        .iter()
        .any(|m| t.contains(m))
        // Go's `t.Error`, never `result.Error`.
        || [
            "t.Error",
            "t.Fatal",
            "expect(",
            "assert(",
            "assert.",
            "require.",
            "assertEquals(",
            "assertThat(",
        ]
        .iter()
        .any(|m| at_word(t, m))
}

/// An assertion that cannot fail.
fn is_trivial_assertion(t: &str) -> bool {
    if commented(t) {
        return false;
    }
    let squashed: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let python = squashed
        .strip_prefix("assertTrue")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with([',', ';', '#']));
    python
        || [
            "assert(true)",
            "assert!(true)",
            "expect(true)",
            "assertTrue(true)",
            "assertTrue(True)",
            "assert.ok(true)",
            "assert.True(t,true)",
            "assert1==1",
            "expect(1).toBe(1)",
        ]
        .iter()
        .any(|m| squashed.contains(m))
}

/// A line that declares a test.
fn is_test_def(t: &str) -> bool {
    [
        "def test",
        "async def test",
        "func Test",
        "#[test]",
        // Spelled in two halves so the purity check, which reads the text of
        // this module, does not take a string for a use of the runtime.
        concat!("#[tokio", "::test"),
        "@Test",
        "it(",
        "test(",
    ]
    .iter()
    .any(|m| t.starts_with(m))
}

/// Words that make a line an oracle for [`hollow_tests`]: an assertion in any
/// spelling, a helper named for one, or an expected failure. Broad on purpose —
/// a helper called `check_total` is taken at its word, so a row here is a test
/// with nothing in it that even looks like a check.
const ORACLE_WORDS: &[&str] = &[
    "assert", "expect", "verify", "should", "must", "check", "ensure", "require", "raise", "throw",
    "panic", "fail", "snapshot", "t.error", "t.fatal",
];

/// Tests the diff adds whole — the declaration and at least one body line, all
/// added — in which no line is an oracle. Returns each declaration, trimmed.
/// A declaration added over an existing body (a rename) has no added body and
/// is not read; a helper that asserts elsewhere is not followed, only named.
fn hollow_tests(hunks: &[Hunk]) -> Vec<String> {
    let oracle = |t: &str| {
        let t = t.to_ascii_lowercase();
        !commented(&t) && ORACLE_WORDS.iter().any(|w| t.contains(w))
    };
    let mut out = Vec::new();
    for h in hunks {
        let mut lines = h.lines.iter().peekable();
        while let Some((kind, text)) = lines.next() {
            let def = text.trim();
            if *kind != Kind::Added || !is_test_def(def) {
                continue;
            }
            // A Rust test is named by its `fn` line, not its attribute, so two in
            // one file are two rows.
            let mut name = def;
            let mut body = 0usize;
            let mut checks = oracle(def);
            while let Some((k, t)) = lines.peek() {
                if *k != Kind::Added || is_test_def(t.trim()) {
                    break;
                }
                let t = t.trim();
                if name.starts_with("#[") && !t.starts_with("#[") && t.contains("fn ") {
                    name = t;
                    // `fn opens() {}` and `fn opens() { run(); }` carry their
                    // whole body — empty counts — on the `fn` line itself.
                    if t.ends_with('}') {
                        body += 1;
                    }
                } else if !t.is_empty()
                    && !t.starts_with("#[")
                    && !matches!(t, "{" | "}" | "});" | "})" | ")")
                {
                    body += 1;
                }
                checks |= oracle(t);
                lines.next();
            }
            if body > 0 && !checks {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Removed lines matching `pred` that no added line matching it takes the
/// place of, across the whole file. A line moved verbatim is not removed; past
/// that, one added match replaces one removed match.
fn unreplaced(hunks: &[Hunk], pred: fn(&str) -> bool) -> Vec<String> {
    let lines = |k: Kind| -> Vec<String> {
        hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|(kind, _)| *kind == k)
            .map(|(_, t)| t.trim().to_string())
            .filter(|t| pred(t))
            .collect()
    };
    let mut removed = lines(Kind::Removed);
    let mut added = lines(Kind::Added);
    removed.retain(|r| match added.iter().position(|a| a == r) {
        Some(at) => {
            added.remove(at);
            false
        }
        None => true,
    });
    let excess = removed.len().saturating_sub(added.len());
    removed.truncate(excess);
    removed
}

/// A stable digest of a file's changed lines: an edit to a whole definition is
/// seen as that edit, and a different edit is a different row.
fn digest(hunks: &[Hunk]) -> String {
    let mut h = crate::core::hash::Rolling::new();
    for (k, t) in hunks.iter().flat_map(|h| h.lines.iter()) {
        if *k != Kind::Context {
            h.push_str(if *k == Kind::Added { "+" } else { "-" });
            h.push_str(t);
        }
    }
    format!("changed lines {}", h.hex())
}

/// The checks a change weakened or changed, in path order, each naming what it
/// matched. Every marker is read in the change's own diff, so one already in
/// the base never counts:
///
/// - **skip** — [`SKIP_MARKERS`] and [`SUPPRESSIONS`] added; in a test file, an
///   assertion removed with none in its place, one that cannot fail added, a
///   test added with no assertion in it, or a line carrying a tolerance changed;
/// - **deleted** — a test file deleted, or a test removed from one that stays;
/// - **gate** — an edit to what defines the checks (gates, CI, a runner's,
///   linter's or coverage configuration, a file a declared gate command names,
///   `package.json` `scripts`, `Cargo.toml` `[profile…]`), and [`MASKS`] added
///   there.
///
/// `gate_commands` are the declared gates' commands, for the files they name.
pub fn weakened(set: &ChangeSet, roles: Option<&Roles>, gate_commands: &[String]) -> Vec<Weakened> {
    let tests = roles
        .filter(|r| !r.tests.is_empty())
        .map(|r| Patterns::parse(&r.tests.join("\n")));
    let under_tests = |path: &str| match &tests {
        Some(p) => p.matches(path),
        None => {
            TEST_DIRS
                .iter()
                .any(|d| path.starts_with(d) || path.contains(&format!("/{d}")))
                || test_named(path.rsplit('/').next().unwrap_or(path))
        }
    };
    let gate_files = files_gates_run(gate_commands);
    let mut out: Vec<Weakened> = Vec::new();
    let mut push = |path: &str, kind: WeakKind, why: String, matched: String| {
        let w = Weakened {
            path: path.to_string(),
            why,
            kind,
            matched,
        };
        if !out.contains(&w) {
            out.push(w);
        }
    };
    for i in path_order(set) {
        let f = &set.files[i];
        let path = f.path.as_str();
        let name = path.rsplit('/').next().unwrap_or(path);
        let deleted = matches!(f.status, crate::core::diff::Status::Deleted);
        let test_file = under_tests(path);
        if deleted && test_file {
            push(
                path,
                WeakKind::Deleted,
                match tests {
                    Some(_) => "deletes a file under the declared `tests` role".into(),
                    None => "deletes a test file".into(),
                },
                path.to_string(),
            );
            continue;
        }
        // A rename is a deletion to whatever looked for the old name: a test
        // runner collecting `test_*.py`, or the file that configured the run.
        if let crate::core::diff::Status::Renamed { from } = &f.status {
            let from_name = from.rsplit('/').next().unwrap_or(from);
            if under_tests(from) && (!test_file || (test_named(from_name) && !test_named(name))) {
                push(
                    path,
                    WeakKind::Deleted,
                    format!("moves a test file out of the test suite (from `{from}`)"),
                    from.clone(),
                );
            }
            let defined = defines_checks(from).is_some() || gate_files.iter().any(|g| g == from);
            let defines_now =
                defines_checks(path).is_some() || gate_files.iter().any(|g| g == path);
            if defined && !defines_now {
                push(
                    path,
                    WeakKind::Gate,
                    format!(
                        "renames `{from}`, which defines the checks, to a name nothing reads it by"
                    ),
                    from.clone(),
                );
            }
        }
        let defines = defines_checks(path).or_else(|| {
            gate_files
                .iter()
                .any(|g| g == path)
                .then(|| format!("edits `{path}`, which a declared gate runs"))
        });
        let hunks = hunks_of(&f.body);
        for h in hunks {
            for (kind, text) in &h.lines {
                if *kind != Kind::Added {
                    continue;
                }
                let t = text.trim();
                // An ignored file is outside the working-tree digest and the
                // diff: nothing a person reviews or a gate is verified on.
                if name == ".gitignore"
                    && !t.is_empty()
                    && !t.starts_with('#')
                    && !t.starts_with('!')
                {
                    push(
                        path,
                        WeakKind::Gate,
                        "adds an ignore pattern — an ignored file is outside the digest and the \
                         review"
                            .into(),
                        t.to_string(),
                    );
                }
                if let Some(m) = SKIP_MARKERS.iter().find(|m| text.contains(**m)) {
                    push(
                        path,
                        WeakKind::Skip,
                        format!("adds a skip marker `{m}`"),
                        t.to_string(),
                    );
                }
                if !is_prose(name)
                    && let Some(m) = SUPPRESSIONS.iter().find(|m| text.contains(**m))
                {
                    push(
                        path,
                        WeakKind::Skip,
                        format!("adds a lint or type suppression `{m}`"),
                        t.to_string(),
                    );
                }
                if test_file && is_trivial_assertion(t) {
                    push(
                        path,
                        WeakKind::Skip,
                        "adds an assertion that cannot fail".into(),
                        t.to_string(),
                    );
                }
                if (defines.is_some() || name == "package.json")
                    && let Some(m) = MASKS.iter().find(|m| text.contains(**m))
                {
                    push(
                        path,
                        WeakKind::Gate,
                        format!("masks a failing step with `{m}`"),
                        t.to_string(),
                    );
                }
            }
            if test_file {
                for (_, t) in h.lines.iter().filter(|(k, _)| *k == Kind::Added) {
                    let changed = TOLERANCES.iter().find(|m| {
                        t.contains(**m)
                            && h.lines
                                .iter()
                                .any(|(k, r)| *k == Kind::Removed && r.contains(**m))
                    });
                    if let Some(m) = changed {
                        push(
                            path,
                            WeakKind::Skip,
                            format!("changes a tolerance `{m}`"),
                            t.trim().to_string(),
                        );
                    }
                }
            }
        }
        if test_file && !deleted {
            for t in unreplaced(hunks, is_assertion) {
                push(
                    path,
                    WeakKind::Skip,
                    "removes an assertion and adds none in its place".into(),
                    t,
                );
            }
            for t in unreplaced(hunks, is_test_def) {
                push(
                    path,
                    WeakKind::Deleted,
                    "removes a test and keeps the file".into(),
                    t,
                );
            }
            for t in hollow_tests(hunks) {
                push(
                    path,
                    WeakKind::Skip,
                    "adds a test with no assertion in it".into(),
                    t,
                );
            }
        }
        if hunks.is_empty() && !deleted {
            // Binary or past the bound: can still define the checks.
            if let Some(why) = defines {
                push(path, WeakKind::Gate, why, "not shown in the diff".into());
            }
            continue;
        }
        if let Some(why) = defines {
            push(path, WeakKind::Gate, why, digest(hunks));
        }
        if name == "package.json"
            && hunks.iter().any(|h| {
                h.lines.iter().any(|(k, t)| {
                    t.contains("\"scripts\"")
                        || (*k != Kind::Context
                            && ["\"test", "\"lint", "\"check", "\"typecheck"]
                                .iter()
                                .any(|key| t.trim_start().starts_with(key)))
                })
            })
        {
            push(
                path,
                WeakKind::Gate,
                "edits the `scripts` of `package.json`".into(),
                digest(hunks),
            );
        }
        if name == "Cargo.toml"
            && hunks.iter().any(|h| {
                h.header.contains("[profile")
                    || h.lines
                        .iter()
                        .any(|(_, t)| t.trim_start().starts_with("[profile"))
            })
        {
            push(
                path,
                WeakKind::Gate,
                "edits a `[profile]` table of `Cargo.toml`".into(),
                digest(hunks),
            );
        }
    }
    out
}

/// The certificate's sentence about the checks a change altered.
pub fn weakened_sentence(w: &[Weakened]) -> Option<String> {
    if w.is_empty() {
        return None;
    }
    let named: Vec<String> = w
        .iter()
        .take(6)
        .map(|w| format!("`{}` ({})", w.path, w.why))
        .collect();
    let more = match w.len().saturating_sub(6) {
        0 => String::new(),
        n => format!(" and {n} more"),
    };
    Some(format!(
        "This change itself altered the checks it was verified by: {}{more}.",
        named.join("; ")
    ))
}

/// How big a change is and where its weight sits, before anybody reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Shape {
    pub files: u32,
    pub added: u32,
    pub removed: u32,
    /// Files per declared role, in role order; empty roles omitted.
    pub by_role: Vec<(Role, u32)>,
    pub formatter_only: u32,
    pub not_read: u32,
    /// Whether roles were declared, so a count of none means none.
    pub ordered: bool,
}

/// Counts only, nothing derived from them.
pub fn shape(set: &ChangeSet, ordered: &Ordered) -> Shape {
    let (added, removed) = set.totals();
    let by_role = ordered
        .groups
        .iter()
        .filter_map(|(r, files)| r.map(|r| (r, files.len() as u32)))
        .collect();
    let formatter_only = set
        .files
        .iter()
        .flat_map(|f| hunks_of(&f.body).iter().map(move |h| (f.path.as_str(), h)))
        .filter(|(p, h)| formatter_only(p, h))
        .count() as u32;
    let not_read = set
        .truncated
        .as_ref()
        .map_or(0, |t| t.files_total.saturating_sub(t.files_shown) as u32);
    Shape {
        files: set.files.len() as u32,
        added,
        removed,
        by_role,
        formatter_only,
        not_read,
        ordered: !ordered.unordered,
    }
}

impl Shape {
    /// *8 files · 41+ 12− lines · 1 shared · 1 security · 1 formatter-only
    /// hunk collapsed*. Shared and security are named only when roles were declared.
    pub fn says(&self) -> String {
        let many = |n: u32, one: &str, more: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {more}"),
        };
        let mut out = format!(
            "{} · {}+ {}− lines",
            many(self.files, "file", "files"),
            self.added,
            self.removed
        );
        if self.ordered {
            for role in [Role::Shared, Role::Security] {
                let n = self
                    .by_role
                    .iter()
                    .find(|(r, _)| *r == role)
                    .map_or(0, |(_, n)| *n);
                out.push_str(&format!(" · {n} {}", role.as_str()));
            }
        }
        if self.formatter_only > 0 {
            out.push_str(&format!(
                " · {} collapsed",
                many(
                    self.formatter_only,
                    "formatter-only hunk",
                    "formatter-only hunks"
                )
            ));
        }
        if self.not_read > 0 {
            out.push_str(&format!(
                " · {} past the bound, not read",
                many(self.not_read, "file", "files")
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::{FileChange, Status};
    use crate::core::ids::SessionId;

    /// Go's `t.Error` asserts; a field called `Error` on anything else does not.
    #[test]
    fn a_go_error_field_is_not_an_assertion() {
        assert!(is_assertion("t.Errorf(\"got %v\", x)"));
        assert!(is_assertion("\tt.Fatal(err)"));
        assert!(!is_assertion("if result.Error != nil {"));
        assert!(!is_assertion("output.Fatal = true"));
    }

    fn hunk(lines: &[(Kind, &str)]) -> Hunk {
        Hunk {
            header: "@@ -1 +1 @@".into(),
            lines: lines.iter().map(|(k, t)| (*k, t.to_string())).collect(),
        }
    }

    fn file(path: &str, added: u32, removed: u32, hunks: Vec<Hunk>) -> FileChange {
        FileChange {
            path: path.into(),
            status: Status::Modified,
            added,
            removed,
            body: Body::Hunks(hunks),
        }
    }

    fn plain(path: &str) -> FileChange {
        file(
            path,
            1,
            0,
            vec![hunk(&[(Kind::Added, &format!("// {path}"))])],
        )
    }

    fn set(files: Vec<FileChange>) -> ChangeSet {
        ChangeSet {
            base: "main".into(),
            files,
            truncated: None,
        }
    }

    fn roles() -> Roles {
        Roles {
            shared: vec!["src/types/**".into()],
            logic: vec!["src/core/**".into()],
            security: vec!["src/auth/**".into()],
            integration: vec!["src/adapters/**".into()],
            wiring: vec!["src/routes/**".into(), "src/**/mod.rs".into()],
            tests: vec!["tests/**".into(), "*.md".into()],
        }
    }

    fn role_of(path: &str, roles: &Roles) -> Option<Role> {
        first_match(path, &compile(roles))
    }

    fn paths(s: &ChangeSet, idx: &[usize]) -> Vec<String> {
        idx.iter().map(|i| s.files[*i].path.clone()).collect()
    }

    #[test]
    fn the_first_matching_role_wins() {
        let r = Roles {
            logic: vec!["src/**".into()],
            security: vec!["src/auth/**".into()],
            ..Roles::default()
        };
        assert_eq!(role_of("src/auth/session.rs", &r), Some(Role::Logic));
        assert_eq!(role_of("README.md", &r), None);
        assert_eq!(role_of("src/types/user.rs", &roles()), Some(Role::Shared));
        assert_eq!(role_of("docs/README.md", &roles()), Some(Role::Tests));
    }

    #[test]
    fn undeclared_files_follow_the_declared_ones_in_path_order() {
        let s = set(vec![
            plain("zeta.rs"),
            plain("tests/auth.rs"),
            plain("src/auth/session.rs"),
            plain("alpha.rs"),
            plain("src/types/user.rs"),
            plain("src/core/rules.rs"),
        ]);
        let o = order(&s, Some(&roles()));
        assert!(!o.unordered);
        let flat: Vec<String> = o.groups.iter().flat_map(|(_, f)| paths(&s, f)).collect();
        assert_eq!(
            flat,
            [
                "src/types/user.rs",
                "src/core/rules.rs",
                "src/auth/session.rs",
                "tests/auth.rs",
                "alpha.rs",
                "zeta.rs"
            ]
        );
        assert_eq!(o.groups.last().unwrap().0, None, "undeclared is last");
        assert_eq!(o.role(2), Some(Role::Security));
        assert_eq!(o.role(0), None);
    }

    #[test]
    fn no_roles_is_path_order_and_says_so() {
        let s = set(vec![plain("b.rs"), plain("a.rs")]);
        let o = order(&s, None);
        assert!(o.unordered);
        assert_eq!(o.groups.len(), 1);
        assert_eq!(paths(&s, &o.groups[0].1), ["a.rs", "b.rs"]);
        assert!(order(&set(vec![]), None).groups.is_empty());
    }

    #[test]
    fn coverage_tells_covered_not_covered_and_no_mapping_apart() {
        let covers = vec![Covers {
            test: "tests/auth.rs".into(),
            paths: vec!["src/auth/**".into()],
        }];
        assert_eq!(
            coverage("src/auth/session.rs", Some(&covers)),
            Coverage::Covered {
                test: "tests/auth.rs".into()
            }
        );
        assert_eq!(
            coverage("src/routes/login.rs", Some(&covers)),
            Coverage::NotCovered
        );
        assert_eq!(coverage("src/auth/session.rs", None), Coverage::NoMapping);
        assert_eq!(Coverage::NoMapping.says(), None, "absent is not *no*");
        assert_eq!(
            Coverage::NotCovered.says().as_deref(),
            Some("no declared test covers this path")
        );
        assert_eq!(
            coverage("src/auth/session.rs", Some(&covers))
                .says()
                .as_deref(),
            Some("covered by `tests/auth.rs`")
        );
    }

    #[test]
    fn whitespace_is_formatting_and_a_reordered_import_is_not() {
        let spaces = hunk(&[
            (Kind::Context, "fn f() {"),
            (Kind::Removed, "    let x=1;"),
            (Kind::Added, "    let x = 1;"),
            (Kind::Removed, ""),
            (Kind::Context, "}"),
        ]);
        assert!(formatter_only("a.rs", &spaces));
        let reorder = hunk(&[
            (Kind::Removed, "use a;"),
            (Kind::Removed, "use b;"),
            (Kind::Added, "use b;"),
            (Kind::Added, "use a;"),
        ]);
        assert!(!formatter_only("a.rs", &reorder));
        let context_only = hunk(&[(Kind::Context, "x")]);
        assert!(
            !formatter_only("a.rs", &context_only),
            "a hunk that changes nothing"
        );
    }

    /// Re-indenting Python moves a line between blocks; it is never collapsed.
    #[test]
    fn reindenting_where_indentation_is_syntax_is_never_formatting() {
        let dedent = hunk(&[
            (Kind::Context, "if ok:"),
            (Kind::Removed, "    charge()"),
            (Kind::Added, "charge()"),
        ]);
        assert!(formatter_only("a.rs", &dedent));
        for path in ["pay.py", "ci/deploy.yml", "x.yaml", "Makefile"] {
            assert!(!formatter_only(path, &dedent), "{path}");
        }
        let spaced = hunk(&[(Kind::Removed, "    x=1"), (Kind::Added, "    x = 1")]);
        assert!(
            formatter_only("pay.py", &spaced),
            "same indent, spacing only"
        );
    }

    #[test]
    fn a_decision_names_a_whole_path_not_a_substring() {
        assert!(names_path("Edit: src/a.rs", "src/a.rs"));
        assert!(names_path("Edit: /abs/wt/src/a.rs", "src/a.rs"));
        assert!(names_path("Write(a.rs)", "a.rs"));
        assert!(!names_path("Edit: src/data.rs", "a.rs"));
        assert!(!names_path("Edit: src/a.rs.bak", "src/a.rs"));
        assert!(!names_path("Edit: src/a.rs/x", "src/a.rs"));
    }

    fn driven(id: &str, sent: Option<Vec<SentTask>>) -> Run {
        let mut r = Run::new(SessionId::new(id), "/w".into(), RunMode::Driven, "echo");
        r.sent = sent;
        r
    }

    fn task(text: &str) -> SentTask {
        SentTask {
            path: "specs/1/tasks.md".into(),
            text: text.into(),
            cites: Vec::new(),
            line: 1,
            occurrence: 1,
        }
    }

    #[test]
    fn a_file_is_grouped_under_the_run_that_wrote_it() {
        let s = set(vec![
            plain("src/asked.rs"),
            plain("src/by_hand.rs"),
            plain("src/untasked.rs"),
        ]);
        let tasked = driven("r-tasked", Some(vec![task("T001 do it")]));
        let untasked = driven("r-untasked", None);
        let runs = vec![
            (&tasked, vec!["src/asked.rs".to_string()]),
            (&untasked, vec!["src/untasked.rs".to_string()]),
        ];
        let i = by_intent(&s, &runs);
        assert_eq!(i.unavailable, None);
        assert_eq!(i.groups.len(), 1);
        assert_eq!(i.groups[0].run, tasked.id);
        assert_eq!(paths(&s, &i.groups[0].files), ["src/asked.rs"]);
        assert_eq!(
            i.not_asked_for,
            [
                (1, NotAskedFor::NoRunWrote),
                (
                    2,
                    NotAskedFor::RunSentNoTasks {
                        run: untasked.id.clone()
                    }
                )
            ]
        );
        assert_eq!(
            i.not_asked_for[0].1.says(),
            "no run Devplane dispatched wrote this"
        );
        assert!(
            i.not_asked_for[1]
                .1
                .says()
                .contains("which was sent no tasks")
        );
    }

    #[test]
    fn a_change_no_run_was_dispatched_for_cannot_be_grouped() {
        let s = set(vec![plain("a.rs")]);
        let mut watched = driven("w", None);
        watched.mode = RunMode::Observed;
        let i = by_intent(&s, &[(&watched, vec!["a.rs".into()])]);
        assert!(i.groups.is_empty() && i.not_asked_for.is_empty());
        assert_eq!(
            i.unavailable.as_deref(),
            Some(
                "every run on this change was watched, not dispatched by Devplane, so grouping \
                 by task is unavailable"
            )
        );
        assert!(by_intent(&s, &[]).unavailable.is_some());
    }

    #[test]
    fn decisions_are_found_by_the_path_they_name() {
        use crate::core::decision::Authority;
        let rows = vec![
            Decision::new(
                Authority::Rule,
                "agent:tool.use",
                "src/auth/session.rs",
                "allow",
            ),
            Decision::new(
                Authority::Rule,
                "agent:tool.use",
                "src/routes/login.rs",
                "allow",
            ),
        ];
        let hits = decisions_for("src/auth/session.rs", &rows);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].subject, "src/auth/session.rs");
        assert!(decisions_for("", &rows).is_empty());
    }

    #[test]
    fn the_shape_is_counts_and_nothing_else() {
        let fmt = hunk(&[(Kind::Removed, "a  =  1"), (Kind::Added, "a = 1")]);
        let s = set(vec![
            file(
                "src/types/user.rs",
                10,
                2,
                vec![hunk(&[(Kind::Added, "x")])],
            ),
            file(
                "src/core/rules.rs",
                10,
                2,
                vec![hunk(&[(Kind::Added, "x")])],
            ),
            file(
                "src/auth/session.rs",
                8,
                2,
                vec![hunk(&[(Kind::Added, "x")])],
            ),
            file(
                "src/routes/login.rs",
                4,
                1,
                vec![hunk(&[(Kind::Added, "x")])],
            ),
            file("tests/auth.rs", 5, 0, vec![hunk(&[(Kind::Added, "x")])]),
            file("README.md", 2, 1, vec![hunk(&[(Kind::Added, "x")])]),
            file("src/util/fmt.rs", 1, 1, vec![fmt]),
            file("src/unrelated.rs", 1, 3, vec![hunk(&[(Kind::Added, "x")])]),
        ]);
        let o = order(&s, Some(&roles()));
        let sh = shape(&s, &o);
        assert_eq!(
            sh.says(),
            "8 files · 41+ 12− lines · 1 shared · 1 security · 1 formatter-only hunk collapsed"
        );
        // Unordered names no role at all, rather than a zero nobody counted.
        let loose = shape(&s, &order(&s, None));
        assert_eq!(
            loose.says(),
            "8 files · 41+ 12− lines · 1 formatter-only hunk collapsed"
        );
        let words = sh.says();
        for tell in ["min", "estimated", "%", "score"] {
            assert!(!words.contains(tell), "{words}");
        }
    }
}

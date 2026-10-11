//! Reading a change: in the order being wrong costs most, with what is already
//! proven beside each file, and what nobody asked for under its own heading.
//! Every input is a declared or recorded fact (roles, test mapping, runs,
//! decisions); nothing is inferred from the code. No aggregate or score: the
//! one size sentence, [`Shape::says`], counts files, lines and roles only.

use crate::core::config::{Covers, Roles};
use crate::core::controls;
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

// ── Seen ─────────────────────────────────────────────────────────────────────
//
// A person's *seen* mark is on one unit of a change: a hunk, keyed by its
// content, so a hunk the agent rewrites is a new unit and unread again. A
// file with no hunks (binary, not shown, a bare rename) is one unit, and so
// is a weakened row whose file is not in the diff (a base test that fails on
// the change). Marks carry no time.

/// A hunk's identity for a seen mark: each line's kind and text, never the
/// `@@` header, so an edit above it that only shifts its line numbers keeps
/// it seen, and two hunks with the same text share one mark.
pub fn hunk_digest(h: &Hunk) -> String {
    let mut r = crate::core::hash::Rolling::new();
    for (k, t) in &h.lines {
        r.push_str(match k {
            Kind::Context => " ",
            Kind::Added => "+",
            Kind::Removed => "-",
        });
        r.push_str(t);
    }
    r.hex()
}

/// The one unit of a file with no hunks: its path, status and body.
pub fn file_digest(f: &crate::core::diff::FileChange) -> String {
    let mut r = crate::core::hash::Rolling::new();
    r.push_str("file");
    r.push_str(&f.path);
    r.push_str(&serde_json::to_string(&f.status).unwrap_or_default());
    match &f.body {
        Body::Binary { bytes } => r.push_str(&format!("binary {bytes:?}")),
        Body::Skipped { why } => r.push_str(why),
        Body::Hunks(h) => r.push_str(&format!("{} hunks", h.len())),
    }
    r.hex()
}

/// The units of one file, in diff order: its hunks' digests, or the file's
/// one unit when it has none.
pub fn file_units(f: &crate::core::diff::FileChange) -> Vec<String> {
    match hunks_of(&f.body) {
        [] => vec![file_digest(f)],
        h => h.iter().map(hunk_digest).collect(),
    }
}

/// The unit of a weakened row whose file is not in the diff.
fn row_digest(w: &Weakened) -> String {
    let mut r = crate::core::hash::Rolling::new();
    r.push_str("row");
    r.push_str(&format!("{:?}", w.kind));
    r.push_str(&w.matched);
    r.hex()
}

/// The units a weakened row is read in: the hunks at its path whose changed
/// lines carry what it matched; every unit of the file when none does (a
/// deleted file, an edited gate); its own unit when the file is not in the
/// diff. Read from the whole change, never the part a view was cut to.
pub fn row_units(w: &Weakened, set: &ChangeSet) -> Vec<(String, String)> {
    let set = set.uncut();
    let Some(f) = set.files.iter().find(|f| f.path == w.path) else {
        return vec![(w.path.clone(), row_digest(w))];
    };
    let hit: Vec<(String, String)> = hunks_of(&f.body)
        .iter()
        .filter(|h| {
            h.lines
                .iter()
                .any(|(k, t)| *k != Kind::Context && t.trim() == w.matched)
        })
        .map(|h| (f.path.clone(), hunk_digest(h)))
        .collect();
    match hit.is_empty() {
        true => file_units(f)
            .into_iter()
            .map(|d| (f.path.clone(), d))
            .collect(),
        false => hit,
    }
}

/// Every unit a mark of `path` covers: the file's units in the whole change,
/// and the units of any weakened row at that path. Empty when nothing there.
pub fn units_at(path: &str, set: &ChangeSet, rows: &[Weakened]) -> Vec<String> {
    let mut out: Vec<String> = set
        .uncut()
        .files
        .iter()
        .filter(|f| f.path == path)
        .flat_map(file_units)
        .collect();
    for w in rows.iter().filter(|w| w.path == path) {
        for (_, d) in row_units(w, set) {
            if !out.contains(&d) {
                out.push(d);
            }
        }
    }
    out
}

/// The rows a person has read, as `(path, matched)`: those every unit of
/// which carries a seen mark. A weakened row is seen exactly when its hunks
/// are; there is no mark of its own.
pub fn rows_seen(
    rows: &[Weakened],
    set: &ChangeSet,
    seen: &std::collections::HashSet<(String, String)>,
) -> std::collections::HashSet<(String, String)> {
    rows.iter()
        .filter(|w| row_units(w, set).iter().all(|u| seen.contains(u)))
        .map(|w| (w.path.clone(), w.matched.clone()))
        .collect()
}

/// The changed files none of whose units a person marked seen, in the order
/// given: what the offer names as *not opened*, by path, never as a share.
pub fn not_opened<'a>(
    paths: impl IntoIterator<Item = &'a str>,
    set: &ChangeSet,
    seen: &std::collections::HashSet<(String, String)>,
) -> Vec<String> {
    let set = set.uncut();
    paths
        .into_iter()
        .filter(|p| {
            set.files
                .iter()
                .filter(|f| f.path == *p)
                .flat_map(file_units)
                .all(|d| !seen.contains(&(p.to_string(), d)))
        })
        .map(str::to_string)
        .collect()
}

/// The changed files' paths in the order the review reads them: the files of
/// weakened rows first, then by declared role, then the rest.
pub fn reading_order(set: &ChangeSet, roles: Option<&Roles>, rows: &[Weakened]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in rows {
        if set.files.iter().any(|f| f.path == w.path) && !out.contains(&w.path) {
            out.push(w.path.clone());
        }
    }
    for (_, files) in order(set, roles).groups {
        for i in files {
            let p = &set.files[i].path;
            if !out.contains(p) {
                out.push(p.clone());
            }
        }
    }
    out
}

/// *2 files not opened: a.rs, b.rs*, or nothing when every file was. Names,
/// never a share of the change.
pub fn not_opened_says(files: &[String]) -> Option<String> {
    match files.len() {
        0 => None,
        1 => Some(format!("1 file not opened: {}", files[0])),
        n => Some(format!("{n} files not opened: {}", files.join(", "))),
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
pub enum NotAskedFor {
    /// A hand edit, a watched session, a generated file.
    NoRunWrote,
    /// A dispatched run wrote it, and that run was sent no tasks.
    RunSentNoTasks { run: RunId },
}

impl NotAskedFor {
    pub fn says(&self) -> String {
        match self {
            NotAskedFor::NoRunWrote => {
                "no dispatched run is recorded writing this, and no sent task names it".into()
            }
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
            // No recorded writer (a shell write, a hand edit): a sent task
            // that names the path by its full spelling is the ask it answers.
            (None, None) => match groups
                .iter()
                .rposition(|g| g.tasks.iter().any(|t| names_path(&t.text, path)))
            {
                Some(g) => groups[g].files.push(i),
                None => not_asked_for.push((i, NotAskedFor::NoRunWrote)),
            },
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
    /// What governs the agent, edited: its hooks, permissions, instructions,
    /// skills or MCP servers ([`crate::core::controls`]).
    AgentControls,
    /// Who must review a path, edited: `CODEOWNERS`, branch protection or a
    /// ruleset as code.
    Ownership,
    /// What the build trusts, edited: a lockfile, or a manifest's dependency
    /// declarations.
    SupplyChain,
    /// A test as the base wrote it fails on this change: the base's tests,
    /// put back on the change's tree, did not pass. Observed, not matched.
    BaseTests,
}

impl WeakKind {
    /// Every kind, in the one order every count list uses.
    pub const ALL: [WeakKind; 7] = [
        WeakKind::Skip,
        WeakKind::Deleted,
        WeakKind::Gate,
        WeakKind::AgentControls,
        WeakKind::Ownership,
        WeakKind::SupplyChain,
        WeakKind::BaseTests,
    ];

    /// The kind in words, shown beside a row's path and reason.
    pub fn says(self) -> &'static str {
        match self {
            WeakKind::Skip => "check weakened",
            WeakKind::Deleted => "test deleted",
            WeakKind::Gate => "gate changed",
            WeakKind::AgentControls => "agent controls",
            WeakKind::Ownership => "ownership",
            WeakKind::SupplyChain => "supply chain",
            WeakKind::BaseTests => "base tests",
        }
    }

    /// How a count of rows of this kind is said: one, then many.
    pub const fn nouns(self) -> (&'static str, &'static str) {
        match self {
            WeakKind::Skip => ("check weakened", "checks weakened"),
            WeakKind::Deleted => ("test deleted", "tests deleted"),
            WeakKind::Gate => ("gate changed", "gates changed"),
            WeakKind::AgentControls => ("agent control changed", "agent controls changed"),
            WeakKind::Ownership => ("ownership file changed", "ownership files changed"),
            WeakKind::SupplyChain => ("supply-chain file changed", "supply-chain files changed"),
            WeakKind::BaseTests => (
                "base test fails on this change",
                "base tests fail on this change",
            ),
        }
    }

    /// Whether a row of this kind alters the checks a change was verified by,
    /// rather than what governs the agent or what the build trusts.
    pub fn alters_checks(self) -> bool {
        !matches!(
            self,
            WeakKind::AgentControls | WeakKind::Ownership | WeakKind::SupplyChain
        )
    }
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
    /// Rows of kind [`WeakKind::AgentControls`]. Always set by the host;
    /// absent only in a row recorded before the kind was read.
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(as = "Option<u32>", optional))]
    pub agent_controls: u32,
    /// Rows of kind [`WeakKind::Ownership`]; absent as above.
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(as = "Option<u32>", optional))]
    pub ownership: u32,
    /// Rows of kind [`WeakKind::SupplyChain`]; absent as above.
    #[serde(default)]
    #[cfg_attr(feature = "typescript", ts(as = "Option<u32>", optional))]
    pub supply_chain: u32,
    /// Rows of kind [`WeakKind::BaseTests`].
    pub base_tests: u32,
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
            agent_controls: count(WeakKind::AgentControls),
            ownership: count(WeakKind::Ownership),
            supply_chain: count(WeakKind::SupplyChain),
            base_tests: count(WeakKind::BaseTests),
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

    /// The count of one kind.
    pub fn count(&self, kind: WeakKind) -> u32 {
        match kind {
            WeakKind::Skip => self.weakened,
            WeakKind::Deleted => self.deleted,
            WeakKind::Gate => self.gates_changed,
            WeakKind::AgentControls => self.agent_controls,
            WeakKind::Ownership => self.ownership,
            WeakKind::SupplyChain => self.supply_chain,
            WeakKind::BaseTests => self.base_tests,
        }
    }

    /// Nothing weakened, deleted or changed.
    pub fn is_empty(&self) -> bool {
        WeakKind::ALL.iter().all(|k| self.count(*k) == 0)
    }

    fn compose(&self) -> String {
        WeakKind::ALL
            .iter()
            .filter_map(|k| {
                let (one, many) = k.nouns();
                match self.count(*k) {
                    0 => None,
                    1 => Some(format!("1 {one}")),
                    n => Some(format!("{n} {many}")),
                }
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// What the review of a change *ready to decide* would lead with, carried on
/// its inbox row so the row never reads as calmer than the review. Hunks seen
/// are the host's marks, the same every surface counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct ReadyFacts {
    pub qualifier: Qualifier,
    /// Every hunk in the change.
    pub hunks: u32,
    /// Hunks a person marked seen, from the host's marks. Always set by the
    /// host; absent only in a row recorded before it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    pub hunks_seen: Option<u32>,
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
            hunks_seen: None,
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
    ".todo(",
    ".skip.each",
    ".skipIf",
    "t.Skip(",
    "t.Skipf(",
    "t.SkipNow()",
    "self.skipTest(",
    "pytest.importorskip",
    "@unittest.skip",
    "#[should_panic",
    "@Disabled",
    "@Ignore",
    // C#: NUnit and MSTest, and xUnit's `Skip =`.
    "[Ignore]",
    "[Ignore(",
    "[Fact(Skip",
    "[Theory(Skip",
];

/// Markers that must start a word: `xit(` is not in `exit(`.
const SKIP_WORDS: &[&str] = &["xit(", "xdescribe(", "xtest(", "xcontext("];

/// RSpec's `pending` and `skip`, a bare word at the start of a line. Read in
/// Ruby only, where the word is the call.
fn rspec_skip(name: &str, t: &str) -> bool {
    name.ends_with(".rb")
        && ["pending", "skip"].iter().any(|w| {
            t.strip_prefix(w)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '(', '"', '\'']))
        })
}

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

/// Which paths hold tests: the declared `tests` role, or, where none is
/// declared, [`TEST_DIRS`] and [`test_named`]. One definition, read by the
/// weakened scan and by base checks alike.
pub(crate) struct TestPaths(Option<Patterns>);

impl TestPaths {
    pub(crate) fn of(roles: Option<&Roles>) -> Self {
        TestPaths(
            roles
                .filter(|r| !r.tests.is_empty())
                .map(|r| Patterns::parse(&r.tests.join("\n"))),
        )
    }

    /// Whether a `tests` role is declared, rather than the fallback.
    pub(crate) fn declared(&self) -> bool {
        self.0.is_some()
    }

    pub(crate) fn matches(&self, path: &str) -> bool {
        match &self.0 {
            Some(p) => p.matches(path),
            None => {
                TEST_DIRS
                    .iter()
                    .any(|d| path.starts_with(d) || path.contains(&format!("/{d}")))
                    || test_named(path.rsplit('/').next().unwrap_or(path))
            }
        }
    }
}

/// Documentation, where a marker is a word about a check, not a check.
fn is_prose(name: &str) -> bool {
    [".md", ".mdx", ".rst", ".txt", ".adoc"]
        .iter()
        .any(|e| name.ends_with(e))
}

/// Keys that make a hunk of `package.json` about the checks: the scripts, and
/// the runners configured in place.
const PACKAGE_JSON_KEYS: &[&str] = &[
    "\"scripts\"",
    "\"test",
    "\"lint",
    "\"check",
    "\"typecheck",
    "\"jest\"",
    "\"vitest\"",
    "\"mocha\"",
    "\"ava\"",
];

/// Keys that make a hunk of `Cargo.toml` about the checks: which tests exist,
/// how they are run, what the lints are, and the profiles they build under.
const CARGO_TOML_KEYS: &[&str] = &[
    "[[test]]", "[lib]", "[lints", "harness", "test =", "[profile",
];

/// Keys that make a hunk of a Maven or Gradle build about the checks.
const JVM_BUILD_KEYS: &[&str] = &[
    "skipTests",
    "maven.test.skip",
    "testFailureIgnore",
    "surefire",
    "failsafe",
    "test {",
    "tasks.test",
    "tasks.named(\"test\"",
];

/// What defines the checks — the one table: `Some((keys, why))` for a file
/// whose edits change what *passing* means. `keys` empty is every edit; else
/// only a hunk naming one of them (in its header or any line) counts, for a
/// manifest that is mostly about something else.
fn check_file(path: &str) -> Option<(&'static [&'static str], String)> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let every = |t: &str| Some((&[][..], t.to_string()));
    if path == "devplane.toml" {
        every("edits the gates' own definition (`devplane.toml`)")
    } else if path.starts_with(".github/workflows/") {
        every("edits a CI workflow")
    } else if path == ".gitlab-ci.yml" {
        every("edits the CI definition (`.gitlab-ci.yml`)")
    } else if name == "package.json" {
        Some((
            PACKAGE_JSON_KEYS,
            "edits the `scripts` or a test runner's settings in `package.json`".into(),
        ))
    } else if name == "Cargo.toml" {
        Some((
            CARGO_TOML_KEYS,
            "edits the tests, lints or profiles `Cargo.toml` declares".into(),
        ))
    } else if name == "pom.xml" || name.starts_with("build.gradle") {
        Some((JVM_BUILD_KEYS, format!("edits how `{name}` runs the tests")))
    } else if path.ends_with(".cargo/config.toml") || path.ends_with(".cargo/config") {
        every("edits `.cargo/config`, which configures every cargo command, tests included")
    } else if name == "build.rs" {
        every("edits `build.rs`, which runs before every build and test")
    } else if name == ".gitattributes" {
        every("edits `.gitattributes`, which decides how git reads and diffs files")
    } else if name == "pyproject.toml" {
        every("edits `pyproject.toml`, which configures the test runner")
    } else if name == "conftest.py" {
        every("edits `conftest.py`, which configures the test run")
    } else if name == "noxfile.py" {
        every("edits `noxfile.py`, whose sessions run the checks")
    } else if name.starts_with("jest.config.")
        || name.starts_with("vitest.config.")
        || name.starts_with(".mocharc")
        || name.starts_with("playwright.config.")
        || name.starts_with("cypress.config.")
        || name.starts_with("karma.conf")
    {
        every("edits the test runner's configuration")
    } else if matches!(name, "justfile" | "Justfile" | ".justfile") {
        every(&format!("edits `{name}`, whose recipes run the checks"))
    } else if matches!(name, "Makefile" | "makefile" | "GNUmakefile") {
        every(&format!("edits `{name}`, whose targets run the checks"))
    } else if name == ".pre-commit-config.yaml" {
        every("edits `.pre-commit-config.yaml`, which runs the checks before a commit")
    } else if matches!(name, "tox.ini" | "setup.cfg" | "pytest.ini") {
        every(&format!("edits `{name}`, which configures the test runner"))
    } else if matches!(
        name,
        "clippy.toml" | ".clippy.toml" | "deny.toml" | "rustfmt.toml" | ".rustfmt.toml"
    ) || name.starts_with(".eslintrc")
        || name.starts_with("eslint.config.")
        || name.starts_with(".golangci.")
    {
        every(&format!("edits `{name}`, which configures a linter"))
    } else if name.starts_with("tsconfig") && name.ends_with(".json") {
        every(&format!(
            "edits `{name}`, which configures the type checker"
        ))
    } else if matches!(name, "codecov.yml" | ".codecov.yml" | ".coveragerc") {
        every(&format!("edits `{name}`, which configures coverage"))
    } else {
        None
    }
}

/// Whether a file is in [`check_file`]'s table at all, whatever it edits.
pub(crate) fn defines_checks(path: &str) -> Option<String> {
    check_file(path).map(|(_, why)| why)
}

/// A removed line that took a test away, wherever it was: Rust's inline test
/// modules live beside the code they test.
fn is_test_removal(t: &str) -> bool {
    is_test_def(t)
        || ["#[cfg(test)]", "describe("]
            .iter()
            .any(|m| t.starts_with(m))
}

/// The files a declared gate command names, relative to the project root:
/// `sh ./scripts/test.sh` names `scripts/test.sh`. Directories and flags are
/// not files.
pub(crate) fn files_gates_run(commands: &[String]) -> Vec<String> {
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
pub(crate) fn is_test_def(t: &str) -> bool {
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
fn digest<'a>(hunks: impl IntoIterator<Item = &'a Hunk>) -> String {
    let mut h = crate::core::hash::Rolling::new();
    for (k, t) in hunks.into_iter().flat_map(|h| h.lines.iter()) {
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
/// - **deleted** — a test file deleted, a test removed from one that stays, or
///   a test declaration removed from any file (inline test modules);
/// - **gate** — an edit to what defines the checks ([`check_file`]'s table and
///   a file a declared gate command names), and [`MASKS`] added there.
///
/// It reads the whole change, never the part a view was cut to
/// ([`ChangeSet::uncut`]). A test or check-defining file whose diff has no
/// lines to read (binary, or not shown) is a row of its own, and so is a set
/// cut with nothing whole beside it: what was not read is never "nothing
/// weakened".
///
/// `gate_commands` are the declared gates' commands, for the files they name.
pub fn weakened(set: &ChangeSet, roles: Option<&Roles>, gate_commands: &[String]) -> Vec<Weakened> {
    let set = set.uncut();
    let tests = TestPaths::of(roles);
    let under_tests = |path: &str| tests.matches(path);
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
    if let Some(t) = &set.truncated {
        push(
            "",
            WeakKind::Gate,
            "diff truncated — weakening not read".into(),
            format!("{} of {} files read", t.files_shown, t.files_total),
        );
    }
    for i in path_order(set) {
        let f = &set.files[i];
        let path = f.path.as_str();
        let name = path.rsplit('/').next().unwrap_or(path);
        let deleted = matches!(f.status, crate::core::diff::Status::Deleted);
        let test_file = under_tests(path);
        // A test file with no lines to read: git called it binary (a planted
        // `-diff` attribute does that too) or the view did not render it.
        if test_file && !deleted && !matches!(f.body, crate::core::diff::Body::Hunks(_)) {
            push(
                path,
                WeakKind::Skip,
                "a test file the diff shows no lines of — not readable, so what it changes is \
                 not read"
                    .into(),
                "not readable".into(),
            );
        }
        // What governs the agent or what the build trusts. Read before the
        // `continue`s below, so a deleted or unreadable control under the
        // `tests` role is still a row; a lockfile row says its tally.
        if let Some(c) = controls::control_file(path) {
            let hunks = hunks_of(&f.body);
            let what = match deleted {
                true => c.deletes(path),
                false => c.edits.clone(),
            };
            let why = |what: String| match c.lock {
                Some(lock) => controls::lockfile_says(&what, lock, hunks),
                None => what,
            };
            if hunks.is_empty() {
                push(path, c.kind, why(what), "not shown in the diff".into());
            } else if hunks.iter().any(|h| c.reads(h)) {
                let read = hunks.iter().filter(|h| c.reads(h));
                push(path, c.kind, why(what), digest(read));
            }
        }
        if deleted && test_file {
            push(
                path,
                WeakKind::Deleted,
                match tests.declared() {
                    true => "deletes a file under the declared `tests` role".into(),
                    false => "deletes a test file".into(),
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
            // A control renamed to a name its vendor does not read is the
            // control removed.
            if let Some(c) = controls::control_file(from)
                && controls::control_file(path).map(|now| now.kind) != Some(c.kind)
            {
                push(path, c.kind, c.renames_away(from), from.clone());
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
        let hunks = hunks_of(&f.body);
        // In the table, with the hunks it narrows to; else named by a gate.
        let (keys, defines) = match check_file(path) {
            Some((keys, why)) => (keys, Some(why)),
            None => (
                &[][..],
                gate_files
                    .iter()
                    .any(|g| g == path)
                    .then(|| format!("edits `{path}`, which a declared gate runs")),
            ),
        };
        let about_checks = keys.is_empty()
            || hunks.iter().any(|h| {
                keys.iter()
                    .any(|k| h.header.contains(k) || h.lines.iter().any(|(_, t)| t.contains(k)))
            });
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
                let marker = SKIP_MARKERS
                    .iter()
                    .find(|m| text.contains(**m))
                    .or_else(|| SKIP_WORDS.iter().find(|m| at_word(t, m)))
                    .copied()
                    .or_else(|| rspec_skip(name, t).then_some(t));
                if let Some(m) = marker {
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
                if defines.is_some()
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
            for t in unreplaced(hunks, is_test_removal) {
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
        } else if !test_file && !is_prose(name) {
            // Outside the test files a test is removed too: an inline
            // `#[cfg(test)]` module, or a spec beside its code.
            for t in unreplaced(hunks, is_test_removal) {
                push(path, WeakKind::Deleted, "a test removed".into(), t);
            }
        }
        if hunks.is_empty() && !deleted {
            // Binary or past the bound: can still define the checks.
            if let Some(why) = defines {
                push(path, WeakKind::Gate, why, "not shown in the diff".into());
            }
            continue;
        }
        if let Some(why) = defines.filter(|_| about_checks) {
            push(path, WeakKind::Gate, why, digest(hunks));
        }
    }
    out
}

/// The certificate's sentences about what a change altered: the checks it was
/// verified by, then what governs the agent or what the build trusts — each
/// written only when it has rows, each naming at most six and counting the rest.
pub fn weakened_sentence(w: &[Weakened]) -> Option<String> {
    let listed = |rows: &[&Weakened], name: &dyn Fn(&Weakened) -> String| {
        let named: Vec<String> = rows.iter().take(6).map(|w| name(w)).collect();
        let more = match rows.len().saturating_sub(6) {
            0 => String::new(),
            n => format!(" and {n} more"),
        };
        format!("{}{more}", named.join("; "))
    };
    let (checks, controls): (Vec<&Weakened>, Vec<&Weakened>) =
        w.iter().partition(|w| w.kind.alters_checks());
    let mut out = Vec::new();
    if !checks.is_empty() {
        out.push(format!(
            "This change itself altered the checks it was verified by: {}.",
            listed(&checks, &|w| format!("`{}` ({})", w.path, w.why))
        ));
    }
    if !controls.is_empty() {
        out.push(format!(
            "It also edited what governs the agent or what the build trusts: {}.",
            listed(&controls, &|w| format!(
                "`{}` ({} — {})",
                w.path,
                w.kind.says(),
                w.why
            ))
        ));
    }
    (!out.is_empty()).then(|| out.join(" "))
}

/// How big a change is and where its weight sits, before anybody reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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

    fn h(header: &str, lines: &[(Kind, &str)]) -> Hunk {
        Hunk {
            header: header.into(),
            lines: lines.iter().map(|(k, t)| (*k, t.to_string())).collect(),
        }
    }

    #[test]
    fn a_hunk_digest_is_its_content_not_its_place() {
        let a = h(
            "@@ -1,1 +1,2 @@",
            &[(Kind::Context, "a"), (Kind::Added, "b")],
        );
        let shifted = h(
            "@@ -9,1 +9,2 @@",
            &[(Kind::Context, "a"), (Kind::Added, "b")],
        );
        let rewritten = h(
            "@@ -1,1 +1,2 @@",
            &[(Kind::Context, "a"), (Kind::Added, "c")],
        );
        let flipped = h(
            "@@ -1,1 +1,2 @@",
            &[(Kind::Context, "a"), (Kind::Removed, "b")],
        );
        assert_eq!(hunk_digest(&a), hunk_digest(&shifted));
        assert_ne!(hunk_digest(&a), hunk_digest(&rewritten));
        assert_ne!(hunk_digest(&a), hunk_digest(&flipped));
        assert_eq!(hunk_digest(&a).len(), 16);
    }

    #[test]
    fn a_weakened_row_is_seen_exactly_when_its_hunks_are() {
        let skip = h(
            "@@ -1 +1,2 @@",
            &[(Kind::Context, "x"), (Kind::Added, "it.skip(\"a\")")],
        );
        let other = h("@@ -9 +9,2 @@", &[(Kind::Context, "y"), (Kind::Added, "z")]);
        let set = ChangeSet {
            base: "main".into(),
            files: vec![FileChange {
                path: "t.test.ts".into(),
                status: Status::Modified,
                added: 2,
                removed: 0,
                body: Body::Hunks(vec![skip.clone(), other.clone()]),
            }],
            ..ChangeSet::default()
        };
        let row = Weakened {
            path: "t.test.ts".into(),
            why: "skip".into(),
            kind: WeakKind::Skip,
            matched: "it.skip(\"a\")".into(),
        };
        let mut seen = std::collections::HashSet::new();
        assert!(rows_seen(std::slice::from_ref(&row), &set, &seen).is_empty());
        seen.insert(("t.test.ts".to_string(), hunk_digest(&other)));
        assert!(rows_seen(std::slice::from_ref(&row), &set, &seen).is_empty());
        seen.insert(("t.test.ts".to_string(), hunk_digest(&skip)));
        assert_eq!(rows_seen(std::slice::from_ref(&row), &set, &seen).len(), 1);
        // A row whose file is not in the diff is its own unit.
        let base = Weakened {
            path: "tests/old.rs".into(),
            why: "fails".into(),
            kind: WeakKind::BaseTests,
            matched: "old".into(),
        };
        let units = row_units(&base, &set);
        assert_eq!(units.len(), 1);
        assert_eq!(
            units_at("tests/old.rs", &set, &[base]),
            vec![units[0].1.clone()]
        );
    }

    #[test]
    fn files_not_opened_are_named_never_a_share() {
        let a = h("@@ -1 +1 @@", &[(Kind::Added, "a")]);
        let b = h("@@ -1 +1 @@", &[(Kind::Added, "b")]);
        let file = |p: &str, x: &Hunk| FileChange {
            path: p.into(),
            status: Status::Modified,
            added: 1,
            removed: 0,
            body: Body::Hunks(vec![x.clone()]),
        };
        let set = ChangeSet {
            base: "main".into(),
            files: vec![file("a.rs", &a), file("b.rs", &b)],
            ..ChangeSet::default()
        };
        let seen: std::collections::HashSet<_> = [("a.rs".to_string(), hunk_digest(&a))]
            .into_iter()
            .collect();
        let not = not_opened(["a.rs", "b.rs"], &set, &seen);
        assert_eq!(not, vec!["b.rs".to_string()]);
        let says = not_opened_says(&not).unwrap();
        assert_eq!(says, "1 file not opened: b.rs");
        assert!(!says.contains(" of ") && !says.contains('/') && !says.contains('%'));
    }

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
            whole: None,
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
            "no dispatched run is recorded writing this, and no sent task names it"
        );
        assert!(
            i.not_asked_for[1]
                .1
                .says()
                .contains("which was sent no tasks")
        );
    }

    #[test]
    fn a_file_no_writer_was_recorded_for_joins_the_task_that_names_it() {
        let s = set(vec![plain("tests/login.test.ts"), plain("tests/login.ts")]);
        let tasked = driven(
            "r-tasked",
            Some(vec![task("T003 Test the burst in tests/login.test.ts")]),
        );
        let i = by_intent(&s, &[(&tasked, Vec::new())]);
        assert_eq!(paths(&s, &i.groups[0].files), ["tests/login.test.ts"]);
        // A prefix of a named path is not the path.
        assert_eq!(i.not_asked_for, [(1, NotAskedFor::NoRunWrote)]);
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

    // ── Agent controls, ownership, supply chain ─────────────────────────────

    fn changed(path: &str, lines: &[(Kind, &str)]) -> FileChange {
        file(path, 1, 1, vec![hunk(lines)])
    }

    fn with(f: FileChange, status: Status) -> FileChange {
        FileChange { status, ..f }
    }

    fn binary(path: &str) -> FileChange {
        FileChange {
            body: Body::Binary { bytes: None },
            ..file(path, 0, 0, vec![])
        }
    }

    fn rows_of(files: Vec<FileChange>) -> Vec<Weakened> {
        weakened(&set(files), None, &[])
    }

    fn tests_role() -> Roles {
        Roles {
            tests: vec!["tests/**".into()],
            ..Roles::default()
        }
    }

    #[test]
    fn each_kind_says_itself_in_words_and_on_the_wire() {
        let words: Vec<&str> = WeakKind::ALL.iter().map(|k| k.says()).collect();
        assert_eq!(
            words,
            [
                "check weakened",
                "test deleted",
                "gate changed",
                "agent controls",
                "ownership",
                "supply chain",
                "base tests"
            ]
        );
        for (k, wire) in [
            (WeakKind::AgentControls, "\"agent_controls\""),
            (WeakKind::Ownership, "\"ownership\""),
            (WeakKind::SupplyChain, "\"supply_chain\""),
        ] {
            assert_eq!(serde_json::to_string(&k).unwrap(), wire);
            assert_eq!(serde_json::from_str::<WeakKind>(wire).unwrap(), k);
        }
    }

    #[test]
    fn the_qualifier_counts_each_new_kind_in_its_own_field() {
        let row = |path: &str, kind| Weakened {
            path: path.into(),
            why: "w".into(),
            kind,
            matched: path.into(),
        };
        let rows = [
            row("tests/a.rs", WeakKind::Skip),
            row(".claude/settings.json", WeakKind::AgentControls),
            row("Cargo.lock", WeakKind::SupplyChain),
        ];
        let q = Qualifier::of(&rows, &Default::default());
        assert_eq!(
            (q.weakened, q.agent_controls, q.ownership, q.supply_chain),
            (1, 1, 0, 1)
        );
        assert_eq!(
            q.says,
            "1 check weakened · 1 agent control changed · 1 supply-chain file changed"
        );
        assert_eq!(q.unseen, 3);
        let seen = [(
            ".claude/settings.json".to_string(),
            ".claude/settings.json".to_string(),
        )]
        .into_iter()
        .collect();
        assert_eq!(Qualifier::of(&rows, &seen).unseen, 2);
        let only = Qualifier::of(&rows[2..], &Default::default());
        assert!(!only.is_empty());
        assert_eq!(only.says, "1 supply-chain file changed");
        let two = Qualifier::of(
            &[
                row("CODEOWNERS", WeakKind::Ownership),
                row("docs/CODEOWNERS", WeakKind::Ownership),
            ],
            &Default::default(),
        );
        assert_eq!(two.says, "2 ownership files changed");
        for w in ["%", "score", "risk", "safe", "dangerous"] {
            assert!(!q.says.contains(w));
        }
    }

    #[test]
    fn an_edit_to_the_agents_controls_is_a_row_of_its_own_kind() {
        let lines = [(
            Kind::Added,
            "    \"PreToolUse\": [{ \"command\": \"x.sh\" }]",
        )];
        let r = rows_of(vec![changed(".claude/settings.json", &lines)]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].kind, WeakKind::AgentControls);
        assert_eq!(
            r[0].why,
            "edits `.claude/settings.json`, which sets the agent's hooks and permissions"
        );
        assert_eq!(r[0].matched, digest(&[hunk(&lines)]));
        // A further edit is a different row: a seen mark lapses.
        let again = rows_of(vec![changed(
            ".claude/settings.json",
            &[(
                Kind::Added,
                "    \"PreToolUse\": [{ \"command\": \"y.sh\" }]",
            )],
        )]);
        assert_ne!(again[0].matched, r[0].matched);

        let added = rows_of(vec![with(
            changed(".github/hooks/a.json", &[(Kind::Added, "{}")]),
            Status::Added,
        )]);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].kind, WeakKind::AgentControls);

        let gone = rows_of(vec![with(
            changed("AGENTS.md", &[(Kind::Removed, "Run the tests.")]),
            Status::Deleted,
        )]);
        assert_eq!(gone.len(), 1, "{gone:?}");
        assert_eq!(
            gone[0].why,
            "deletes `AGENTS.md`, the instructions the agent reads"
        );

        let renamed = rows_of(vec![with(
            file(".claude/settings.json.off", 0, 0, vec![]),
            Status::Renamed {
                from: ".claude/settings.json".into(),
            },
        )]);
        assert_eq!(renamed.len(), 1, "{renamed:?}");
        assert_eq!(
            renamed[0].why,
            "renames `.claude/settings.json`, which sets the agent's hooks and permissions, to \
             a name nothing reads it by"
        );
        assert_eq!(renamed[0].matched, ".claude/settings.json");

        let skill = rows_of(vec![binary(".claude/skills/x/logo.png")]);
        assert_eq!(skill.len(), 1);
        assert_eq!(skill[0].matched, "not shown in the diff");

        assert!(rows_of(vec![changed("docs/claude-settings.json", &lines)]).is_empty());
    }

    #[test]
    fn a_controls_row_stands_beside_the_rows_a_file_already_gave() {
        let r = rows_of(vec![changed(
            "CLAUDE.md",
            &[(
                Kind::Added,
                "Mark slow tests with it.skip( until CI is fast.",
            )],
        )]);
        let kinds: Vec<WeakKind> = r.iter().map(|w| w.kind).collect();
        assert!(kinds.contains(&WeakKind::AgentControls), "{r:?}");
        assert!(kinds.contains(&WeakKind::Skip), "{r:?}");

        // Under a declared `tests` role: a deleted AGENTS.md and a binary
        // lockfile are still read as what they govern.
        let s = set(vec![
            with(
                changed("tests/fixtures/AGENTS.md", &[(Kind::Removed, "x")]),
                Status::Deleted,
            ),
            binary("tests/fixtures/Cargo.lock"),
        ]);
        let r = weakened(&s, Some(&tests_role()), &[]);
        let at = |p: &str, k: WeakKind| r.iter().any(|w| w.path == p && w.kind == k);
        assert!(
            at("tests/fixtures/AGENTS.md", WeakKind::AgentControls),
            "{r:?}"
        );
        assert!(at("tests/fixtures/AGENTS.md", WeakKind::Deleted), "{r:?}");
        assert!(
            at("tests/fixtures/Cargo.lock", WeakKind::SupplyChain),
            "{r:?}"
        );
    }

    #[test]
    fn a_lockfile_row_says_its_tally() {
        let lock = changed(
            "Cargo.lock",
            &[
                (Kind::Added, "[[package]]"),
                (Kind::Added, "name = \"evil\""),
                (Kind::Added, "version = \"0.1.0\""),
            ],
        );
        let r = rows_of(vec![lock]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].kind, WeakKind::SupplyChain);
        assert_eq!(
            r[0].why,
            "edits the lockfile `Cargo.lock`: 1 package added (`evil` 0.1.0)"
        );
        for w in ["score", "risk", "%"] {
            assert!(!r[0].why.contains(w));
        }
        let unread = rows_of(vec![changed("composer.lock", &[(Kind::Added, "{")])]);
        assert_eq!(
            unread[0].why,
            "edits the lockfile `composer.lock`: package counts not read for this format"
        );
        let blind = rows_of(vec![binary("Cargo.lock")]);
        assert_eq!(
            blind[0].why,
            "edits the lockfile `Cargo.lock`: package counts not read: the diff shows no lines \
             of it"
        );
        let fixture = rows_of(vec![changed(
            "tests/fixtures/x/Cargo.lock",
            &[(Kind::Added, "")],
        )]);
        assert_eq!(fixture.len(), 1);
        assert_eq!(fixture[0].kind, WeakKind::SupplyChain);
    }

    #[test]
    fn ownership_and_dependency_declarations_are_rows() {
        for p in [
            "CODEOWNERS",
            ".github/CODEOWNERS",
            "docs/CODEOWNERS",
            ".github/settings.yml",
        ] {
            let r = rows_of(vec![changed(p, &[(Kind::Removed, "/src/auth/ @security")])]);
            assert_eq!(r.len(), 1, "{p}: {r:?}");
            assert_eq!(r[0].kind, WeakKind::Ownership);
        }
        let tf = |resource: &str| {
            rows_of(vec![changed(
                "infra/main.tf",
                &[
                    (
                        Kind::Context,
                        &format!("resource \"{resource}\" \"main\" {{"),
                    ),
                    (Kind::Removed, "  enforcement = \"active\""),
                    (Kind::Added, "  enforcement = \"disabled\""),
                ],
            )])
        };
        assert_eq!(tf("github_repository_ruleset").len(), 1);
        assert_eq!(tf("github_repository_ruleset")[0].kind, WeakKind::Ownership);
        assert!(tf("aws_s3_bucket").is_empty());

        // One manifest, two rows: the lints are the checks, the dependency
        // is the supply chain; each read and marked on its own.
        let lints = hunk(&[
            (Kind::Context, "[lints.rust]"),
            (Kind::Added, "unsafe_code = \"allow\""),
        ]);
        let deps = hunk(&[
            (Kind::Context, "[dependencies]"),
            (Kind::Added, "serde = \"1\""),
        ]);
        let r = rows_of(vec![file("Cargo.toml", 2, 0, vec![lints, deps.clone()])]);
        assert_eq!(r.len(), 2, "{r:?}");
        let gate = r.iter().find(|w| w.kind == WeakKind::Gate).unwrap();
        let supply = r.iter().find(|w| w.kind == WeakKind::SupplyChain).unwrap();
        assert_ne!(gate.matched, supply.matched);
        assert_eq!(supply.matched, digest(&[deps]));
        assert_eq!(supply.why, "edits the dependencies `Cargo.toml` declares");

        for p in ["requirements-dev.txt", "go.mod"] {
            let r = rows_of(vec![changed(p, &[(Kind::Added, "x")])]);
            assert_eq!(r.len(), 1, "{p}");
            assert_eq!(r[0].kind, WeakKind::SupplyChain);
        }
        let described = rows_of(vec![changed(
            "package.json",
            &[
                (Kind::Removed, "  \"description\": \"a\","),
                (Kind::Added, "  \"description\": \"b\","),
            ],
        )]);
        assert!(described.is_empty(), "{described:?}");
    }

    #[test]
    fn the_certificate_sentence_keeps_checks_and_controls_apart() {
        let row = |path: &str, kind, why: &str| Weakened {
            path: path.into(),
            why: why.into(),
            kind,
            matched: path.into(),
        };
        let check = row(
            "tests/a.rs",
            WeakKind::Skip,
            "adds a skip marker `#[ignore]`",
        );
        let lock = row(
            "Cargo.lock",
            WeakKind::SupplyChain,
            "edits the lockfile `Cargo.lock`: 1 package added (`a` 1.0)",
        );
        assert_eq!(
            weakened_sentence(std::slice::from_ref(&check)).unwrap(),
            "This change itself altered the checks it was verified by: `tests/a.rs` (adds a \
             skip marker `#[ignore]`)."
        );
        let only = weakened_sentence(std::slice::from_ref(&lock)).unwrap();
        assert_eq!(
            only,
            "It also edited what governs the agent or what the build trusts: `Cargo.lock` \
             (supply chain — edits the lockfile `Cargo.lock`: 1 package added (`a` 1.0))."
        );
        assert!(!only.contains("altered the checks"));
        let both = weakened_sentence(&[lock.clone(), check.clone()]).unwrap();
        assert!(both.starts_with("This change itself altered the checks it was verified by: "));
        assert!(both.contains(". It also edited what governs the agent"));
        let many: Vec<Weakened> = (0..9)
            .map(|i| row(&format!("a{i}/AGENTS.md"), WeakKind::AgentControls, "w"))
            .collect();
        let s = weakened_sentence(&many).unwrap();
        assert!(
            s.contains("a5/AGENTS.md") && !s.contains("a6/AGENTS.md"),
            "{s}"
        );
        assert!(s.ends_with(" and 3 more."), "{s}");
        assert_eq!(weakened_sentence(&[]), None);
    }
}

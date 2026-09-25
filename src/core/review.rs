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
use crate::core::spec::SentTask;
use crate::core::worktreeinclude::Patterns;
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

/// One hunk or file that alters a check, with why. A signal from the diff
/// text alone, never a verdict: it says what it matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Weakened {
    pub path: String,
    pub why: String,
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

/// Where tests live when the project declared no `tests` role.
const TEST_DIRS: &[&str] = &["tests/", "test/", "__tests__/", "spec/"];

/// What defines the checks: a hunk here changes what *passing* means.
fn defines_checks(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if path == "devplane.toml" {
        Some("edits the gates' own definition (`devplane.toml`)")
    } else if path.starts_with(".github/workflows/") {
        Some("edits a CI workflow")
    } else if path == ".gitlab-ci.yml" {
        Some("edits the CI definition (`.gitlab-ci.yml`)")
    } else if name == "pyproject.toml" {
        Some("edits `pyproject.toml`, which configures the test runner")
    } else if name == "conftest.py" {
        Some("edits `conftest.py`, which configures the test run")
    } else if name.starts_with("jest.config.") || name.starts_with("vitest.config.") {
        Some("edits the test runner's configuration")
    } else {
        None
    }
}

/// The checks a change weakened or changed, in path order: [`SKIP_MARKERS`]
/// added, test files deleted, edits to what defines the checks, `package.json`
/// `scripts`, `Cargo.toml` `[profile…]` tables.
pub fn weakened(set: &ChangeSet, roles: Option<&Roles>) -> Vec<Weakened> {
    let tests = roles
        .filter(|r| !r.tests.is_empty())
        .map(|r| Patterns::parse(&r.tests.join("\n")));
    let under_tests = |path: &str| match &tests {
        Some(p) => p.matches(path),
        None => TEST_DIRS
            .iter()
            .any(|d| path.starts_with(d) || path.contains(&format!("/{d}"))),
    };
    let mut out: Vec<Weakened> = Vec::new();
    let mut push = |path: &str, why: String| {
        let w = Weakened {
            path: path.to_string(),
            why,
        };
        if !out.contains(&w) {
            out.push(w);
        }
    };
    for i in path_order(set) {
        let f = &set.files[i];
        let path = f.path.as_str();
        if matches!(f.status, crate::core::diff::Status::Deleted) && under_tests(path) {
            push(
                path,
                match tests {
                    Some(_) => "deletes a file under the declared `tests` role".into(),
                    None => "deletes a test file".into(),
                },
            );
            continue;
        }
        let hunks = hunks_of(&f.body);
        for h in hunks {
            for (kind, text) in &h.lines {
                if *kind != Kind::Added {
                    continue;
                }
                if let Some(m) = SKIP_MARKERS.iter().find(|m| text.contains(**m)) {
                    push(path, format!("adds a skip marker `{m}`"));
                }
            }
        }
        if hunks.is_empty() && !matches!(f.status, crate::core::diff::Status::Deleted) {
            // Binary or past the bound: can still define the checks.
            if let Some(why) = defines_checks(path) {
                push(path, why.to_string());
            }
            continue;
        }
        if let Some(why) = defines_checks(path) {
            push(path, why.to_string());
        }
        let name = path.rsplit('/').next().unwrap_or(path);
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
            push(path, "edits the `scripts` of `package.json`".into());
        }
        if name == "Cargo.toml"
            && hunks.iter().any(|h| {
                h.header.contains("[profile")
                    || h.lines
                        .iter()
                        .any(|(_, t)| t.trim_start().starts_with("[profile"))
            })
        {
            push(path, "edits a `[profile]` table of `Cargo.toml`".into());
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

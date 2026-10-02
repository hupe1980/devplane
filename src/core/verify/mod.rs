//! `devplane verify`: the verdict where merges are decided. The base's own
//! `check` runs on the change; the checks the change weakened are read first;
//! the exit code says only whether `check` passed on a tree that stayed put.
//!
//! Pure: the edge (`src/verify.rs`) reads git, runs the gate and writes files,
//! and hands the results in as values. [`Verdict::of`] takes no weakened row —
//! a row is evidence shown first, never the verdict.

pub mod json;
pub mod summary;

use super::change::GateReport;
use super::review::{WeakKind, Weakened};
use serde::Serialize;

/// The file whose `check` governs, read at the merge base.
pub const CONFIG_FILE: &str = super::config::CONFIG_FILE;

/// How many moved paths a verdict names before counting the rest.
pub const MOVED_NAMED: usize = 20;

/// Where the name of the base came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseSource {
    /// `--base <ref>`.
    Flag,
    /// The branch `origin/HEAD` names.
    OriginHead,
    /// Neither: `main`.
    Fallback,
}

/// What the change is compared against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    /// What was asked: `main`, a tag, a commit.
    pub asked: String,
    pub asked_from: BaseSource,
    /// The spelling that resolved (`origin/main` before `main`).
    pub resolved: String,
    /// `git merge-base <resolved> HEAD`: whose `devplane.toml` governs.
    pub merge_base: String,
    /// `HEAD`'s commit.
    pub head: String,
    /// Wording only: the checkout is the forge's merge of a pull request.
    pub forge_merge: bool,
}

impl Base {
    /// The merge base as a reader types it.
    pub fn short(&self) -> &str {
        short(&self.merge_base)
    }
}

/// The first seven characters of an object id.
pub fn short(id: &str) -> &str {
    id.get(..7).unwrap_or(id)
}

/// The gate that ran, and what the checkout declares instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// From `devplane.toml` at the merge base. Never empty.
    pub check: Vec<String>,
    pub timeout_secs: u64,
    /// The base's `devplane.toml` as a git blob id.
    pub file_blob: Option<String>,
    pub change_declares: ChangeDeclares,
    /// The base declares `[workspace] setup`, which `verify` does not run.
    pub has_setup: bool,
    /// The base declares named gates, which `verify` does not run.
    pub has_named: bool,
}

/// What the checkout's own `devplane.toml` says `check` is. Shown; never run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "is", rename_all = "snake_case")]
pub enum ChangeDeclares {
    /// The same commands, in the same order.
    Same,
    /// Other commands, possibly none.
    Differs { check: Vec<String> },
    /// The checkout has no `devplane.toml`.
    Absent,
    /// The checkout's file does not parse.
    Unreadable { why: String },
}

impl ChangeDeclares {
    /// Compares the base's `check` with what the checkout's file says.
    /// `checkout` is `None` when the file is absent, `Err` when it does not parse.
    pub fn of(base: &[String], checkout: Option<Result<Vec<String>, String>>) -> Self {
        match checkout {
            None => ChangeDeclares::Absent,
            Some(Err(why)) => ChangeDeclares::Unreadable { why },
            Some(Ok(check)) if check == base => ChangeDeclares::Same,
            Some(Ok(check)) => ChangeDeclares::Differs { check },
        }
    }

    pub fn is_same(&self) -> bool {
        matches!(self, ChangeDeclares::Same)
    }
}

/// A change that rewrote what `check` means is a weakened row. The diff scan
/// already names an edit to `devplane.toml`; this adds the row only when it
/// did not, so the file is named once.
pub fn ensure_definition_row(rows: &mut Vec<Weakened>, declares: &ChangeDeclares) {
    if declares.is_same() || rows.iter().any(|w| w.path == CONFIG_FILE) {
        return;
    }
    let (why, matched) = match declares {
        ChangeDeclares::Same => return,
        ChangeDeclares::Differs { check } => (
            "rewrites `check`; the base's definition ran".to_string(),
            format!("check = {check:?}"),
        ),
        ChangeDeclares::Absent => (
            "deletes the file that defines `check`; the base's definition ran".to_string(),
            CONFIG_FILE.to_string(),
        ),
        ChangeDeclares::Unreadable { why } => (
            "leaves `devplane.toml` unreadable; the base's definition ran".to_string(),
            why.clone(),
        ),
    };
    rows.push(Weakened {
        path: CONFIG_FILE.to_string(),
        why,
        kind: WeakKind::Gate,
        matched,
    });
}

/// The two reads of the working tree around `check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trees {
    /// The digest before `check`, or why it could not be read.
    pub before: Result<String, String>,
    /// The digest after.
    pub after: Result<String, String>,
    /// The paths that differ between the two reads, sorted.
    pub moved: Vec<String>,
}

/// The one thing that sets the exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Every `check` command exited zero; the tree was the same before and after.
    Verified {
        tree: String,
    },
    Failed {
        why: Failed,
    },
    CouldNotRun {
        why: CouldNotRun,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failed {
    /// The first command that did not exit zero, and how.
    Command { command: String, says: String },
    /// Every command exited zero, and the tree moved: the result describes no tree.
    TreeMoved { paths: Vec<String>, more: u32 },
    /// Every command exited zero, and the tree could not be read around them.
    TreeUnreadable { why: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CouldNotRun {
    NotARepository,
    /// Neither spelling names a commit. `fallback` when nobody asked for it.
    BaseUnresolvable {
        tried: Vec<String>,
        fallback: bool,
    },
    /// The base and `HEAD` share no history here.
    NoSharedHistory {
        base: String,
        shallow: bool,
    },
    NoConfigAtBase {
        at: String,
    },
    ConfigUnreadableAtBase {
        at: String,
        why: String,
    },
    NoCheck {
        at: String,
    },
    DiffUnreadable {
        at: String,
        why: String,
    },
    /// `check` passed, and a file its evidence goes to could not be written.
    CouldNotWrite {
        path: String,
        why: String,
    },
}

impl CouldNotRun {
    /// The JSON `reason`.
    pub fn key(&self) -> &'static str {
        match self {
            CouldNotRun::NotARepository => "not_a_repository",
            CouldNotRun::BaseUnresolvable { .. } => "base_unresolvable",
            CouldNotRun::NoSharedHistory { .. } => "no_shared_history",
            CouldNotRun::NoConfigAtBase { .. } => "no_config_at_base",
            CouldNotRun::ConfigUnreadableAtBase { .. } => "config_unreadable_at_base",
            CouldNotRun::NoCheck { .. } => "no_check",
            CouldNotRun::DiffUnreadable { .. } => "diff_unreadable",
            CouldNotRun::CouldNotWrite { .. } => "could_not_write",
        }
    }

    fn says(&self) -> String {
        match self {
            CouldNotRun::NotARepository => {
                "this is not a git repository, so there is no base to compare with".into()
            }
            CouldNotRun::BaseUnresolvable {
                tried,
                fallback: true,
            } => format!(
                "no base was named, `origin/HEAD` names none, and {} are not commits here; \
                 pass `--base <ref>`",
                tried
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            CouldNotRun::BaseUnresolvable { tried, .. } => format!(
                "{} are not commits here; pass `--base <ref>` naming a fetched branch, tag or commit",
                tried
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            CouldNotRun::NoSharedHistory {
                base,
                shallow: true,
            } => format!(
                "the history of `{base}` is not in this checkout (a shallow clone); check out \
                 with `fetch-depth: 0`"
            ),
            CouldNotRun::NoSharedHistory { base, .. } => format!(
                "`{base}` and this checkout share no history here; fetch it, or check out with \
                 `fetch-depth: 0`"
            ),
            CouldNotRun::NoConfigAtBase { at } => format!(
                "the merge base {at} has no devplane.toml; a change cannot supply its own gate — \
                 its `check` runs from the next pull request after it merges"
            ),
            CouldNotRun::ConfigUnreadableAtBase { at, why } => {
                format!("devplane.toml at {at} could not be read: {why}")
            }
            CouldNotRun::NoCheck { at } => format!(
                "devplane.toml at {at} declares no `check`, so nothing is verified and nothing \
                 pretends to be"
            ),
            CouldNotRun::DiffUnreadable { at, why } => {
                format!("the diff against {at} could not be read: {why}")
            }
            CouldNotRun::CouldNotWrite { path, why } => {
                format!("`check` passed, and its evidence could not be written to {path}: {why}")
            }
        }
    }
}

impl Verdict {
    /// Derived from the gate's exit codes and the two reads of the tree alone.
    pub fn of(report: &GateReport, trees: &Trees) -> Self {
        if let Some(c) = report.commands.iter().find(|c| !c.passed()) {
            return Verdict::Failed {
                why: Failed::Command {
                    command: c.command.clone(),
                    says: super::certificate::outcome_sentence(c),
                },
            };
        }
        if report.commands.is_empty() {
            return Verdict::Failed {
                why: Failed::TreeUnreadable {
                    why: "no command ran".into(),
                },
            };
        }
        match (&trees.before, &trees.after) {
            (Ok(b), Ok(a)) if b == a => Verdict::Verified { tree: b.clone() },
            (Ok(_), Ok(_)) => {
                let mut paths = trees.moved.clone();
                paths.sort();
                let more = paths.len().saturating_sub(MOVED_NAMED) as u32;
                paths.truncate(MOVED_NAMED);
                Verdict::Failed {
                    why: Failed::TreeMoved { paths, more },
                }
            }
            (Err(why), _) | (_, Err(why)) => Verdict::Failed {
                why: Failed::TreeUnreadable { why: why.clone() },
            },
        }
    }

    /// 0 verified, 1 failed, 2 could not run.
    pub fn exit_code(&self) -> i32 {
        match self {
            Verdict::Verified { .. } => 0,
            Verdict::Failed { .. } => 1,
            Verdict::CouldNotRun { .. } => 2,
        }
    }

    /// The JSON `verdict`.
    pub fn key(&self) -> &'static str {
        match self {
            Verdict::Verified { .. } => "verified",
            Verdict::Failed { .. } => "failed",
            Verdict::CouldNotRun { .. } => "could_not_run",
        }
    }

    /// The word a heading uses.
    pub fn word(&self) -> &'static str {
        match self {
            Verdict::Verified { .. } => "verified",
            Verdict::Failed { .. } => "failed",
            Verdict::CouldNotRun { .. } => "could not run",
        }
    }

    /// One sentence; no two alike.
    pub fn says(&self) -> String {
        match self {
            Verdict::Verified { tree } => format!(
                "The base's `check` exited 0 on every command, on tree {}.",
                short(tree)
            ),
            Verdict::Failed {
                why: Failed::Command { command, says },
            } => format!("The base's `check` failed: `{command}` ended with {says}."),
            Verdict::Failed {
                why: Failed::TreeMoved { paths, more },
            } => format!(
                "Every command exited 0, but `check` changed the tree it ran on, so the result \
                 describes no tree: {}{}.",
                paths.join(", "),
                match more {
                    0 => String::new(),
                    n => format!(" and {n} more"),
                }
            ),
            Verdict::Failed {
                why: Failed::TreeUnreadable { why },
            } => format!(
                "Every command exited 0, but the tree could not be read around them, so the \
                 result describes no tree: {why}."
            ),
            Verdict::CouldNotRun { why } => format!("Could not run: {}.", why.says()),
        }
    }

    /// Whether `check` ran at all.
    pub fn ran(&self) -> bool {
        !matches!(self, Verdict::CouldNotRun { why } if !matches!(why, CouldNotRun::CouldNotWrite { .. }))
    }
}

/// `#base-checks` — the base's tests on this change, and this change's new
/// tests on the base — as `verify` ran them, or why it did not. Never an
/// input to [`Verdict`]: a failing base test is a weakened row, and a new
/// test that passes without the change is a line to read.
#[derive(Debug, Clone, PartialEq)]
pub enum BaseChecks {
    /// `check` did not run, so neither did they.
    NotReached,
    /// The shared view: the pair's result, or the absence and its sentence.
    Shown(Box<super::base_checks::View>),
}

impl BaseChecks {
    fn view(&self) -> Option<&super::base_checks::View> {
        match self {
            BaseChecks::NotReached => None,
            BaseChecks::Shown(v) => Some(v),
        }
    }

    /// Why no current result stands, as a sentence; `None` when the pair ran.
    pub fn absent_says(&self) -> Option<String> {
        match self.view() {
            None => {
                Some("The base's tests were not run on this change: `check` did not run.".into())
            }
            v => super::certificate::base_checks_absent(v),
        }
    }

    /// How many tests ran, beside the verdict: *ran 12 → 3 · pytest*, with
    /// why base checks did not run when they did not.
    pub fn counts(&self) -> Option<&str> {
        self.view().map(|v| v.counts_says.as_str())
    }

    /// The section's sentences: the absence, or one per run.
    pub fn says(&self) -> Vec<String> {
        if let Some(why) = self.absent_says() {
            return vec![why];
        }
        let Some(v) = self.view() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        // First: whether the evidence tells this change from its base.
        if let Some(h) = &v.headline {
            let mut chars = h.says.chars();
            let mut line: String = match chars.next() {
                Some(c) => c.to_uppercase().chain(chars).collect(),
                None => String::new(),
            };
            if !h.named.is_empty() {
                let more = match h.named_more {
                    0 => String::new(),
                    n => format!(" and {n} more"),
                };
                line.push_str(&format!(
                    " — failing on the base: {}{more}",
                    h.named.join(", ")
                ));
            }
            out.push(format!("{line}."));
        }
        if let Some(o) = &v.on_change {
            out.push(format!("The base's tests on this change: {}.", o.says));
        }
        if let Some(o) = &v.on_base {
            out.push(format!("This change's new tests on the base: {}.", o.says));
        }
        out
    }

    /// New tests that pass without the change, by name, and how many more.
    pub fn passing_without(&self) -> (&[String], usize) {
        match self.view().and_then(|v| v.on_base.as_ref()) {
            Some(o) => (&o.passing, o.passing_more),
            None => (&[], 0),
        }
    }

    /// `predicate.devplane.baseChecks`, as the certificate writes it.
    pub fn json(&self) -> serde_json::Value {
        super::certificate::base_checks_json(self.view(), self.absent_says())
    }
}

/// What a reader might assume ran, and did not.
pub fn not_done(definition: Option<&Definition>) -> Vec<String> {
    let mut out = Vec::new();
    if definition.is_some_and(|d| d.has_setup) {
        out.push("`[workspace] setup` was not run; this workflow prepared the runner.".into());
    }
    if definition.is_some_and(|d| d.has_named) {
        out.push("Named gates were not run; only `check` decides.".into());
    }
    out
}

/// Everything the composer needs.
#[derive(Debug, Clone, Copy)]
pub struct Run<'a> {
    /// `None` when `verify` stopped before the base resolved.
    pub base: Option<&'a Base>,
    pub definition: Option<&'a Definition>,
    pub rows: &'a [Weakened],
    pub truncated: Option<&'a super::diff::Truncation>,
    /// The merge base is `HEAD` and nothing is uncommitted.
    pub no_change: bool,
    /// `None` when `check` did not run.
    pub report: Option<&'a GateReport>,
    pub verdict: &'a Verdict,
    pub base_checks: &'a BaseChecks,
    pub not_done: &'a [String],
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::change::{CommandResult, Outcome};

    pub fn report(results: Vec<CommandResult>) -> GateReport {
        GateReport {
            gate: "check".into(),
            at: jiff::Timestamp::UNIX_EPOCH,
            duration_ms: 1,
            commands: results,
            attempt: 1,
            spec: None,
            commit: None,
            junit: Vec::new(),
            base_commit: None,
        }
    }

    pub fn same(tree: &str) -> Trees {
        Trees {
            before: Ok(tree.into()),
            after: Ok(tree.into()),
            moved: vec![],
        }
    }

    #[test]
    fn every_command_exiting_zero_on_a_still_tree_is_verified() {
        let r = report(vec![
            CommandResult::exited("a", 0),
            CommandResult::exited("b", 0),
        ]);
        let v = Verdict::of(&r, &same("abc"));
        assert_eq!(v, Verdict::Verified { tree: "abc".into() });
        assert_eq!(v.exit_code(), 0);
    }

    #[test]
    fn the_first_command_that_did_not_exit_zero_is_named() {
        for outcome in [
            Outcome::Exited { code: 1 },
            Outcome::TimedOut { after_secs: 3 },
            Outcome::NeverStarted {
                reason: "no shell".into(),
            },
            Outcome::Unknown {
                reason: "killed".into(),
            },
        ] {
            let r = report(vec![
                CommandResult::exited("ok", 0),
                CommandResult::without_verdict("bad", outcome),
            ]);
            let v = Verdict::of(&r, &same("abc"));
            assert!(
                matches!(&v, Verdict::Failed { why: Failed::Command { command, .. } } if command == "bad"),
                "{v:?}"
            );
            assert_eq!(v.exit_code(), 1);
        }
    }

    #[test]
    fn a_moved_tree_fails_and_names_at_most_twenty_paths() {
        let r = report(vec![CommandResult::exited("ok", 0)]);
        let moved: Vec<String> = (0..25).rev().map(|i| format!("f{i:02}")).collect();
        let v = Verdict::of(
            &r,
            &Trees {
                before: Ok("a".into()),
                after: Ok("b".into()),
                moved,
            },
        );
        let Verdict::Failed {
            why: Failed::TreeMoved { paths, more },
        } = &v
        else {
            panic!("{v:?}")
        };
        assert_eq!(paths.len(), 20);
        assert_eq!(paths[0], "f00");
        assert_eq!(*more, 5);
        assert_eq!(v.exit_code(), 1);
    }

    #[test]
    fn verdict_ignores_weakened_rows() {
        // The constructor takes no row: the same inputs give the same verdict,
        // whatever the diff weakened.
        let r = report(vec![CommandResult::exited("ok", 0)]);
        let mut rows = Vec::new();
        let without = Verdict::of(&r, &same("t"));
        ensure_definition_row(&mut rows, &ChangeDeclares::Absent);
        assert_eq!(rows.len(), 1);
        let with = Verdict::of(&r, &same("t"));
        assert_eq!(without, with);
        assert_eq!(with.exit_code(), 0);
    }

    #[test]
    fn could_not_run_exits_two_and_every_sentence_differs() {
        let all = vec![
            Verdict::Verified { tree: "t".into() },
            Verdict::Failed {
                why: Failed::Command {
                    command: "c".into(),
                    says: "exit 1".into(),
                },
            },
            Verdict::Failed {
                why: Failed::TreeMoved {
                    paths: vec!["p".into()],
                    more: 0,
                },
            },
            Verdict::Failed {
                why: Failed::TreeUnreadable { why: "w".into() },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::NotARepository,
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::BaseUnresolvable {
                    tried: vec!["origin/x".into(), "x".into()],
                    fallback: false,
                },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::BaseUnresolvable {
                    tried: vec!["origin/main".into(), "main".into()],
                    fallback: true,
                },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::NoSharedHistory {
                    base: "main".into(),
                    shallow: true,
                },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::NoSharedHistory {
                    base: "main".into(),
                    shallow: false,
                },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::NoConfigAtBase { at: "a".into() },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::ConfigUnreadableAtBase {
                    at: "a".into(),
                    why: "w".into(),
                },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::NoCheck { at: "a".into() },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::DiffUnreadable {
                    at: "a".into(),
                    why: "w".into(),
                },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::CouldNotWrite {
                    path: "p".into(),
                    why: "w".into(),
                },
            },
        ];
        let says: std::collections::HashSet<String> = all.iter().map(Verdict::says).collect();
        assert_eq!(says.len(), all.len());
        for v in &all {
            let want = match v {
                Verdict::Verified { .. } => 0,
                Verdict::Failed { .. } => 1,
                Verdict::CouldNotRun { .. } => 2,
            };
            assert_eq!(v.exit_code(), want);
        }
        assert!(
            all[7].says().contains("fetch-depth: 0"),
            "a shallow clone names the fix"
        );
    }

    #[test]
    fn a_definition_row_is_added_once() {
        let mut rows = vec![Weakened {
            path: CONFIG_FILE.into(),
            why: "edits what defines the checks".into(),
            kind: WeakKind::Gate,
            matched: "x".into(),
        }];
        ensure_definition_row(
            &mut rows,
            &ChangeDeclares::Differs {
                check: vec!["true".into()],
            },
        );
        assert_eq!(rows.len(), 1, "the scan's row is the one named");
        let mut none = Vec::new();
        ensure_definition_row(&mut none, &ChangeDeclares::Same);
        assert!(none.is_empty());
    }
}

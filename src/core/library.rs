//! The library: intent you reuse across projects, in the vendors' own formats.
//!
//! **This module owns verbs and no nouns.** `SKILL.md` has forty-six readers
//! and a six-field portable core; adding a forty-seventh format would be the
//! worst thing this product could ship. So an artefact here is whatever the
//! vendor wrote, byte for byte, and everything Devplane knows about it lives in
//! a separate file beside it.
//!
//! # What is pure and what is not
//!
//! Everything here is a total function over values. The digest takes an
//! iterator of `(path, bytes)` and never opens a file; drift takes three
//! digests and never looks at a disk; portability takes parsed frontmatter and
//! never asks a vendor anything. `tests/purity.rs` enforces it, and the split
//! is what makes the six drift outcomes a test table rather than six
//! filesystem fixtures.
//!
//! The half that touches a disk is `crate::library`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// The tree digest
// ---------------------------------------------------------------------------

/// Names an editor's noise makes, which are not drift.
///
/// **One constant, in one place, and short on purpose.** Without it a
/// `.DS_Store` reports six projects as drifted on a Mac, which is the fastest
/// way to teach somebody to stop reading this command. With too much in it, it
/// becomes a hiding place — so every suppression is *reported* (see
/// [`TreeDigest::ignored`]).
const IGNORED: &[&str] = &[".DS_Store", ".git/", "*.swp", "*~"];

/// Whether a relative path is suppressed by the ignore list.
///
/// Matching is deliberately literal: a leading-directory rule ends in `/`, a
/// suffix rule starts with `*`, and everything else is a file name. A glob
/// engine here would be a second pattern language in a product that already
/// argues against owning formats.
///
/// Guarded by `tests/purity.rs`, which globs this directory: nothing in it may
/// open a file, spawn a task or await anything. That is not incidental — it is
/// what makes the six drift outcomes a test table instead of six temporary
/// directories, and the portability rules a list instead of a fixture.
fn is_ignored(rel: &Path) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/");
    let name = rel.file_name().map(|n| n.to_string_lossy().to_string());
    IGNORED.iter().any(|pat| {
        if let Some(dir) = pat.strip_suffix('/') {
            s == dir || s.starts_with(&format!("{dir}/")) || s.contains(&format!("/{dir}/"))
        } else if let Some(suffix) = pat.strip_prefix('*') {
            s.ends_with(suffix)
        } else {
            name.as_deref() == Some(*pat)
        }
    })
}

/// A digest over an artefact, and what the ignore list suppressed getting there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct TreeDigest {
    pub digest: String,
    /// Relative paths the ignore list dropped, sorted.
    ///
    /// **Kept, and printed.** A suppressed difference that is not reported is
    /// the ignore list turning into somewhere to hide a change.
    pub ignored: Vec<String>,
}

/// Digests an artefact from its files.
///
/// Files are **sorted by relative path** so the result is independent of walk
/// order, and each contributes both its path and its content — a file renamed
/// with identical bytes is a change, and a digest that missed it would be
/// answering a different question from the one asked.
///
/// Lengths are folded between parts by [`crate::core::hash::Rolling`], so
/// `("ab", "c")` and `("a", "bc")` cannot collide. A folder of documents is
/// exactly the shape where that matters.
pub fn tree_digest<I, P>(files: I) -> TreeDigest
where
    I: IntoIterator<Item = (P, Vec<u8>)>,
    P: AsRef<Path>,
{
    let mut kept: Vec<(String, Vec<u8>)> = Vec::new();
    let mut ignored: Vec<String> = Vec::new();
    for (path, bytes) in files {
        let rel = path.as_ref();
        let shown = rel.to_string_lossy().replace('\\', "/");
        if is_ignored(rel) {
            ignored.push(shown);
        } else {
            kept.push((shown, bytes));
        }
    }
    kept.sort_by(|a, b| a.0.cmp(&b.0));
    ignored.sort();
    ignored.dedup();

    let mut h = crate::core::hash::Rolling::new();
    for (path, bytes) in &kept {
        h.push_str(path);
        h.push(bytes);
    }
    TreeDigest {
        digest: h.hex(),
        ignored,
    }
}

// ---------------------------------------------------------------------------
// The sidecar
// ---------------------------------------------------------------------------

/// `.devplane.toml`, written beside an artefact Devplane never touches.
///
/// **The two-file split is the whole trick**: the artefact stays byte-identical
/// to what its vendor expects, and the provenance lives next to it where `cat`
/// can read it. Putting this in SQLite would make the library's state invisible
/// to the editor that edits the library.
///
/// Four facts and nothing else. There is no `safe`, no grade and no score, and
/// [`crate::core::setup`] is what answers *what will this be allowed to do*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Sidecar {
    /// Where it came from — `github:acme/skills#review-findings`, a path, a URL.
    pub origin: String,
    /// The digest of what was fetched, so a later read can say whether the
    /// thing on disk is still the thing that arrived.
    pub digest: String,
    /// When, as RFC 3339.
    pub installed: String,
    /// Who asked for it.
    pub by: String,
}

impl Sidecar {
    pub const FILE: &'static str = ".devplane.toml";

    /// Parses a sidecar, reporting a **missing** field rather than defaulting
    /// one.
    ///
    /// A blank `origin` would answer *where did this come from* with silence
    /// dressed as an answer, which is worse than the question failing.
    pub fn parse(text: &str) -> Result<Self, String> {
        let v: toml::Value =
            toml::from_str(text).map_err(|e| format!("{} is not valid TOML: {e}", Self::FILE))?;
        let field = |k: &str| -> Result<String, String> {
            v.get(k)
                .and_then(|x| x.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| format!("{} has no `{k}`", Self::FILE))
        };
        Ok(Self {
            origin: field("origin")?,
            digest: field("digest")?,
            installed: field("installed")?,
            by: field("by")?,
        })
    }

    /// Renders it. Written by hand rather than by a serialiser so the file a
    /// person opens has the same shape as the one in the documentation.
    pub fn to_toml(&self) -> String {
        format!(
            "origin    = \"{}\"\ndigest    = \"{}\"\ninstalled = \"{}\"\nby        = \"{}\"\n",
            self.origin.escape_debug(),
            self.digest.escape_debug(),
            self.installed.escape_debug(),
            self.by.escape_debug(),
        )
    }
}

// ---------------------------------------------------------------------------
// Drift
// ---------------------------------------------------------------------------

/// What happened to one copy, relative to the library.
///
/// **The single most likely thing to be quietly reduced to a boolean.** Six
/// values because there are six situations, and collapsing any pair loses the
/// one fact a person needs: *which side moved*. `CopyMoved` and `LibraryMoved`
/// are the same boolean and opposite instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum Drift {
    /// current = installed = library.
    Unchanged,
    /// Somebody edited the project's copy.
    CopyMoved,
    /// The source moved. The copy is **stale, not wrong**.
    LibraryMoved,
    /// Both moved, and nothing here will pick between them.
    BothMoved,
    /// Installed, and no longer there. Deleted is not unchanged.
    Missing,
    /// A copy is here that Devplane did not install — no sidecar entry.
    ///
    /// Answers *which of my projects has a skill I did not put there*, which is
    /// the question nothing else on the machine can.
    Unrecorded,
}

impl Drift {
    /// The word the report prints. **No two are the same**, and a test asserts
    /// it: five outcomes a person cannot tell apart is the failure this feature
    /// is most likely to ship.
    pub fn label(self) -> &'static str {
        match self {
            Drift::Unchanged => "ok",
            Drift::CopyMoved => "DRIFT",
            Drift::LibraryMoved => "STALE",
            Drift::BothMoved => "CONFLICT",
            Drift::Missing => "MISSING",
            Drift::Unrecorded => "UNRECORDED",
        }
    }

    /// The sentence beside it, in the second person, saying what is true rather
    /// than what to do about it.
    pub fn says(self) -> &'static str {
        match self {
            Drift::Unchanged => "",
            Drift::CopyMoved => "the project's copy was edited",
            Drift::LibraryMoved => "the library moved; this copy is the one you installed",
            Drift::BothMoved => "both moved — nobody here will pick",
            Drift::Missing => "installed, and no longer there",
            Drift::Unrecorded => "a copy is here that Devplane did not install",
        }
    }

    /// Whether this is worth a person's eye. Used only for ordering the report:
    /// failures before successes, because a list that buries the one red line
    /// under five green ones is a list nobody reads twice.
    pub fn is_finding(self) -> bool {
        !matches!(self, Drift::Unchanged)
    }
}

/// What happened to one copy, from three digests.
///
/// `installed` is what the sidecar recorded, `current` is what is on disk now
/// (`None` when the copy is gone), and `library` is the source. Total, and pure
/// — which is what lets the six outcomes be a table rather than six temporary
/// directories.
pub fn drift(installed: Option<&str>, current: Option<&str>, library: &str) -> Drift {
    let Some(installed) = installed else {
        // No record that Devplane put it there. If there is no copy either,
        // there is nothing to say — but the caller only asks about copies that
        // exist, and `Unrecorded` is the interesting half.
        return match current {
            Some(_) => Drift::Unrecorded,
            None => Drift::Missing,
        };
    };
    let Some(current) = current else {
        return Drift::Missing;
    };
    match (current == installed, library == installed) {
        (true, true) => Drift::Unchanged,
        (false, true) => Drift::CopyMoved,
        (true, false) => Drift::LibraryMoved,
        (false, false) => Drift::BothMoved,
    }
}

// ---------------------------------------------------------------------------
// Portability
// ---------------------------------------------------------------------------

/// The six frontmatter fields the Agent Skills specification allows.
///
/// Held here and asserted against the fetched specification, so the code and
/// the corpus cannot drift apart without something failing.
pub const PORTABLE_FIELDS: &[&str] = &[
    "allowed-tools",
    "compatibility",
    "description",
    "license",
    "metadata",
    "name",
];

/// The distribution paths the vendor names as raising a hard error.
///
/// **Paths, never vendors.** The error is documented for Anthropic's own
/// distribution — claude.ai upload, the Skills API, `package_skill.py` — and a
/// third-party reader meeting an unknown key is under no such obligation.
/// `diff --to cursor` does not exist, and inventing it would be a compatibility
/// claim over somebody else's moving surface.
pub const DISTRIBUTION_PATHS: &[&str] = &["claude.ai", "the Skills API", "package_skill.py"];

/// Why one field will fail on those paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum Why {
    /// Outside the specification's six.
    UnexpectedKey,
    /// Over 64 characters.
    NameTooLong,
    /// Not lowercase letters, digits and single hyphens.
    NameNotSlug,
    /// Over 1024 characters, or empty.
    DescriptionTooLong,
    /// Over 500 characters.
    CompatibilityTooLong,
}

impl Why {
    pub fn says(&self) -> &'static str {
        match self {
            Why::UnexpectedKey => "unexpected key",
            Why::NameTooLong => "`name` is over 64 characters",
            Why::NameNotSlug => "`name` is not lowercase letters, digits and single hyphens",
            Why::DescriptionTooLong => "`description` is empty or over 1024 characters",
            Why::CompatibilityTooLong => "`compatibility` is over 500 characters",
        }
    }
}

/// One documented failure, for one field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
// **Named for its subject on the wire.** This and `batch::Finding` both
// exported `Finding.ts`; the fan-out's won and a page importing `Finding`
// believing it described a rejected frontmatter field would have got a
// preflight refusal instead. Two unrelated types, one filename, no error.
#[cfg_attr(
    feature = "typescript",
    ts(rename = "PortabilityFinding", export, export_to = "wire/")
)]
pub struct Finding {
    pub field: String,
    pub why: Why,
}

/// What a report can say about one artefact's frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Portability {
    pub findings: Vec<Finding>,
    /// **Set when the frontmatter could not be parsed at all**, so that an
    /// empty finding list never reads as *nothing will be lost*.
    ///
    /// The distinction is the whole reason this field exists: *no findings* and
    /// *not read* are the same output and opposite facts.
    pub unread: bool,
}

impl Portability {
    /// The sentence every report carries, whatever it found.
    ///
    /// Printed always, because the report's coverage is the thing a reader will
    /// otherwise assume: a tool that *silently ignores* a field raises nothing
    /// anywhere, and nothing here can see it.
    pub const CAVEAT: &'static str = "Documented failures only. Fields a tool silently ignores are not covered and are not absent.";
}

/// Whether a name is the slug shape the specification states.
fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The documented failures in one parsed frontmatter.
///
/// `fields` is the frontmatter as key/value pairs; `unread` says the reader
/// could not parse it. Nothing here grades anything, suggests a replacement, or
/// says a word about fields a reader ignores in silence.
pub fn portability(fields: &[(String, String)], unread: bool) -> Portability {
    if unread {
        return Portability {
            findings: Vec::new(),
            unread: true,
        };
    }
    let mut findings = Vec::new();
    for (k, v) in fields {
        if !PORTABLE_FIELDS.contains(&k.as_str()) {
            findings.push(Finding {
                field: k.clone(),
                why: Why::UnexpectedKey,
            });
            continue;
        }
        match k.as_str() {
            "name" if v.chars().count() > 64 => findings.push(Finding {
                field: k.clone(),
                why: Why::NameTooLong,
            }),
            "name" if !is_slug(v) => findings.push(Finding {
                field: k.clone(),
                why: Why::NameNotSlug,
            }),
            "description" if v.trim().is_empty() || v.chars().count() > 1024 => {
                findings.push(Finding {
                    field: k.clone(),
                    why: Why::DescriptionTooLong,
                })
            }
            "compatibility" if v.chars().count() > 500 => findings.push(Finding {
                field: k.clone(),
                why: Why::CompatibilityTooLong,
            }),
            _ => {}
        }
    }
    Portability {
        findings,
        unread: false,
    }
}

// ---------------------------------------------------------------------------
// Where an artefact may be written
// ---------------------------------------------------------------------------

/// A location some vendor documents, and the only kind Devplane writes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `.claude/skills/<name>/` — Claude Code, project.
    ClaudeProject,
    /// `~/.claude/skills/<name>/` — Claude Code, personal.
    ClaudePersonal,
    /// `~/.copilot/skills/<name>/` — Copilot CLI, personal.
    CopilotPersonal,
    /// `.devplane/prompts/<name>.md` — this product's own portable form.
    DevplanePrompt,
}

impl Scope {
    /// Where this artefact goes under `root`.
    ///
    /// **The only function that decides a write path**, so a path no vendor
    /// documents cannot be reached by construction rather than by a check
    /// somebody remembers to call.
    pub fn path(self, root: &Path, name: &str) -> PathBuf {
        match self {
            Scope::ClaudeProject => root.join(".claude/skills").join(name),
            Scope::ClaudePersonal => root.join(".claude/skills").join(name),
            Scope::CopilotPersonal => root.join(".copilot/skills").join(name),
            Scope::DevplanePrompt => root.join(".devplane/prompts").join(format!("{name}.md")),
        }
    }

    /// Who documents it, for the line that says why this path and no other.
    pub fn documented_by(self) -> &'static str {
        match self {
            Scope::ClaudeProject => "Claude Code, project scope",
            Scope::ClaudePersonal => "Claude Code, personal scope",
            Scope::CopilotPersonal => "Copilot CLI, personal scope",
            Scope::DevplanePrompt => "Devplane's own portable prompt",
        }
    }
}

// ---------------------------------------------------------------------------
// Preflight
// ---------------------------------------------------------------------------

/// Why one target cannot take this artefact.
///
/// Computed for **every** target before the first byte is written, so that a
/// refusal is never one failure after three successes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// Not trusted. Installing executable intent into a repository nobody has
    /// looked at is the thing `devplane trust` exists to precede.
    Untrusted,
    /// A different copy is already there.
    Collision,
    /// The path is not one any vendor documents.
    UndocumentedPath,
}

impl Refusal {
    pub fn label(self) -> &'static str {
        match self {
            Refusal::Untrusted => "refused",
            Refusal::Collision => "conflict",
            Refusal::UndocumentedPath => "refused",
        }
    }

    pub fn says(self, root: &Path) -> String {
        match self {
            Refusal::Untrusted => format!("not trusted — devplane trust {}", root.display()),
            Refusal::Collision => "a different copy is already there (--force to replace)".into(),
            Refusal::UndocumentedPath => "no vendor documents a path for this artefact here".into(),
        }
    }
}

/// What is known about one target, with no disk in sight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFacts {
    pub trusted: bool,
    /// The digest already at the destination, when something is there.
    pub existing: Option<String>,
    /// The digest being installed.
    pub incoming: String,
    pub documented_path: bool,
}

/// Whether one target can take this artefact, and why not.
///
/// Pure, so every refusal is a row in a test table rather than a scratch
/// repository somebody has to build. A collision with *identical* bytes is not
/// a collision: re-installing what is already there changes nothing and
/// refusing it would teach people to pass `--force` by reflex.
pub fn preflight(f: &TargetFacts) -> Option<Refusal> {
    if !f.documented_path {
        return Some(Refusal::UndocumentedPath);
    }
    if !f.trusted {
        return Some(Refusal::Untrusted);
    }
    match &f.existing {
        Some(d) if *d != f.incoming => Some(Refusal::Collision),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(v: &[(&str, &str)]) -> Vec<(PathBuf, Vec<u8>)> {
        v.iter()
            .map(|(p, c)| (PathBuf::from(p), c.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn the_digest_does_not_depend_on_the_order_files_were_walked_in() {
        let a = tree_digest(files(&[
            ("SKILL.md", "one"),
            ("scripts/run.sh", "two"),
            ("references/x.md", "three"),
        ]));
        let b = tree_digest(files(&[
            ("references/x.md", "three"),
            ("SKILL.md", "one"),
            ("scripts/run.sh", "two"),
        ]));
        assert_eq!(
            a.digest, b.digest,
            "sorted by path, so walk order is not a fact about the artefact"
        );
    }

    /// The path is part of the digest, not only the bytes.
    #[test]
    fn a_file_renamed_with_identical_bytes_is_a_change() {
        let a = tree_digest(files(&[("SKILL.md", "x")]));
        let b = tree_digest(files(&[("SKILL.MD", "x")]));
        assert_ne!(a.digest, b.digest);
    }

    /// And the lengths are folded in, so a shift across a boundary is not a
    /// collision.
    #[test]
    fn two_files_whose_contents_shift_across_the_boundary_do_not_collide() {
        let a = tree_digest(files(&[("a", "xy"), ("b", "z")]));
        let b = tree_digest(files(&[("a", "x"), ("b", "yz")]));
        assert_ne!(a.digest, b.digest);
    }

    #[test]
    fn a_suppressed_file_is_reported_and_does_not_move_the_digest() {
        let clean = tree_digest(files(&[("SKILL.md", "x")]));
        let noisy = tree_digest(files(&[
            ("SKILL.md", "x"),
            (".DS_Store", "junk"),
            ("SKILL.md.swp", "junk"),
            ("notes.md~", "junk"),
            (".git/HEAD", "ref: refs/heads/main"),
        ]));
        assert_eq!(
            clean.digest, noisy.digest,
            "an editor's noise is not drift, which is the whole reason the list exists"
        );
        assert_eq!(
            noisy.ignored,
            vec![
                ".DS_Store".to_string(),
                ".git/HEAD".to_string(),
                "SKILL.md.swp".to_string(),
                "notes.md~".to_string(),
            ],
            "and every suppression is reported, or the list is a hiding place"
        );
        assert!(clean.ignored.is_empty());
    }

    /// **The table this feature is most likely to lose.**
    #[test]
    fn the_six_outcomes_are_six_situations_and_no_two_read_the_same() {
        let cases = [
            (Some("a"), Some("a"), "a", Drift::Unchanged),
            (Some("a"), Some("b"), "a", Drift::CopyMoved),
            (Some("a"), Some("a"), "b", Drift::LibraryMoved),
            (Some("a"), Some("b"), "c", Drift::BothMoved),
            (Some("a"), None, "a", Drift::Missing),
            (None, Some("b"), "a", Drift::Unrecorded),
        ];
        for (installed, current, library, want) in cases {
            assert_eq!(
                drift(installed, current, library),
                want,
                "{installed:?} {current:?} {library}"
            );
        }

        // Five outcomes a person cannot tell apart is the same as one.
        let all = [
            Drift::Unchanged,
            Drift::CopyMoved,
            Drift::LibraryMoved,
            Drift::BothMoved,
            Drift::Missing,
            Drift::Unrecorded,
        ];
        let mut labels: Vec<_> = all.iter().map(|d| d.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), all.len(), "two outcomes share a label");
        let mut said: Vec<_> = all
            .iter()
            .filter(|d| d.is_finding())
            .map(|d| d.says())
            .collect();
        said.sort_unstable();
        said.dedup();
        assert_eq!(said.len(), all.len() - 1, "two findings read the same");
    }

    /// The pair that is one boolean and two opposite instructions.
    #[test]
    fn copy_moved_and_library_moved_are_never_collapsed() {
        assert_ne!(
            drift(Some("a"), Some("b"), "a"),
            drift(Some("a"), Some("a"), "b")
        );
        assert_ne!(Drift::CopyMoved.says(), Drift::LibraryMoved.says());
    }

    #[test]
    fn a_sidecar_round_trips_and_a_missing_field_is_reported_rather_than_defaulted() {
        let s = Sidecar {
            origin: "github:acme/skills#review-findings".into(),
            digest: "c3f1".into(),
            installed: "2026-09-19T14:02:11Z".into(),
            by: "hupe".into(),
        };
        assert_eq!(Sidecar::parse(&s.to_toml()).unwrap(), s);

        for missing in ["origin", "digest", "installed", "by"] {
            let mut lines: Vec<String> = s
                .to_toml()
                .lines()
                .filter(|l| !l.starts_with(missing))
                .map(str::to_string)
                .collect();
            lines.push(String::new());
            let err = Sidecar::parse(&lines.join("\n")).unwrap_err();
            assert!(err.contains(missing), "{missing}: {err}");
        }
        // Present but blank is missing, not an answer.
        let blank = s.to_toml().replace("\"hupe\"", "\"\"");
        assert!(Sidecar::parse(&blank).is_err());
    }

    #[test]
    fn an_unexpected_key_is_a_documented_failure_and_the_six_are_not() {
        let p = portability(
            &[
                ("name".into(), "review-findings".into()),
                ("description".into(), "Reviews findings".into()),
                ("argument-hint".into(), "<file>".into()),
            ],
            false,
        );
        assert_eq!(p.findings.len(), 1, "{p:?}");
        assert_eq!(p.findings[0].field, "argument-hint");
        assert_eq!(p.findings[0].why, Why::UnexpectedKey);
        assert!(!p.unread);
    }

    #[test]
    fn the_documented_constraints_are_counted_rather_than_claimed() {
        let long = "a".repeat(65);
        let p = portability(&[("name".into(), long)], false);
        assert_eq!(p.findings[0].why, Why::NameTooLong);

        for bad in ["Review-Findings", "-lead", "trail-", "a--b", ""] {
            let p = portability(&[("name".into(), bad.into())], false);
            assert!(
                p.findings
                    .iter()
                    .any(|f| matches!(f.why, Why::NameNotSlug | Why::NameTooLong)),
                "{bad} was accepted"
            );
        }
        assert!(
            portability(&[("name".into(), "review-findings-2".into())], false)
                .findings
                .is_empty()
        );

        assert_eq!(
            portability(&[("description".into(), "a".repeat(1025))], false).findings[0].why,
            Why::DescriptionTooLong
        );
        assert_eq!(
            portability(&[("description".into(), "  ".into())], false).findings[0].why,
            Why::DescriptionTooLong
        );
        assert_eq!(
            portability(&[("compatibility".into(), "a".repeat(501))], false).findings[0].why,
            Why::CompatibilityTooLong
        );
    }

    /// **The distinction the whole report rests on.**
    #[test]
    fn a_frontmatter_nobody_could_read_is_unread_and_not_clean() {
        let p = portability(&[], true);
        assert!(p.findings.is_empty());
        assert!(
            p.unread,
            "no findings and not read are the same output and opposite facts"
        );
        assert!(!portability(&[], false).unread);
    }

    #[test]
    fn a_write_path_is_one_a_vendor_documents_and_nothing_else() {
        let root = Path::new("/repo");
        assert_eq!(
            Scope::ClaudeProject.path(root, "review"),
            PathBuf::from("/repo/.claude/skills/review")
        );
        assert_eq!(
            Scope::CopilotPersonal.path(root, "review"),
            PathBuf::from("/repo/.copilot/skills/review")
        );
        assert_eq!(
            Scope::DevplanePrompt.path(root, "bump-deps"),
            PathBuf::from("/repo/.devplane/prompts/bump-deps.md")
        );
        for s in [
            Scope::ClaudeProject,
            Scope::ClaudePersonal,
            Scope::CopilotPersonal,
            Scope::DevplanePrompt,
        ] {
            assert!(!s.documented_by().is_empty());
        }
    }

    #[test]
    fn every_refusal_is_known_before_anything_is_written() {
        let ok = TargetFacts {
            trusted: true,
            existing: None,
            incoming: "a".into(),
            documented_path: true,
        };
        assert_eq!(preflight(&ok), None);

        assert_eq!(
            preflight(&TargetFacts {
                trusted: false,
                ..ok.clone()
            }),
            Some(Refusal::Untrusted)
        );
        assert_eq!(
            preflight(&TargetFacts {
                existing: Some("b".into()),
                ..ok.clone()
            }),
            Some(Refusal::Collision)
        );
        assert_eq!(
            preflight(&TargetFacts {
                documented_path: false,
                ..ok.clone()
            }),
            Some(Refusal::UndocumentedPath),
            "an undocumented path is refused before trust is even consulted"
        );
        // Re-installing exactly what is already there is not a collision.
        assert_eq!(
            preflight(&TargetFacts {
                existing: Some("a".into()),
                ..ok.clone()
            }),
            None,
            "refusing an identical re-install teaches people to pass --force by reflex"
        );
    }

    /// The no-grading refusal, asserted on the vocabulary rather than left
    /// as a review note.
    #[test]
    fn nothing_this_module_can_say_is_a_grade() {
        let mut vocabulary: Vec<String> = Vec::new();
        for d in [
            Drift::Unchanged,
            Drift::CopyMoved,
            Drift::LibraryMoved,
            Drift::BothMoved,
            Drift::Missing,
            Drift::Unrecorded,
        ] {
            vocabulary.push(d.label().into());
            vocabulary.push(d.says().into());
        }
        for w in [
            Why::UnexpectedKey,
            Why::NameTooLong,
            Why::NameNotSlug,
            Why::DescriptionTooLong,
            Why::CompatibilityTooLong,
        ] {
            vocabulary.push(w.says().into());
        }
        for r in [
            Refusal::Untrusted,
            Refusal::Collision,
            Refusal::UndocumentedPath,
        ] {
            vocabulary.push(r.label().into());
            vocabulary.push(r.says(Path::new("/repo")));
        }
        vocabulary.push(Portability::CAVEAT.into());

        let banned = ["safe", "unsafe", "risky", "trusted", "score", "grade", "✓"];
        for word in &vocabulary {
            let lower = word.to_lowercase();
            for b in banned {
                // `trusted` appears only as the *refusal* "not trusted", which
                // is a fact about a repository rather than a verdict about an
                // artefact — so it is matched on the standalone word.
                if b == "trusted" && lower.contains("not trusted") {
                    continue;
                }
                assert!(!lower.contains(b), "`{b}` in {word:?}");
            }
        }
    }
}

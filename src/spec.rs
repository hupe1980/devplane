//! The specification a change answers, read back.
//!
//! No methodology is modelled: Spec Kit, Kiro and OpenSpec agree only that a
//! specification is a committed Markdown folder structured by headings, with
//! progress as `- [ ]` / `- [x]` boxes in `tasks.md`. So the outline is the
//! headings and progress is the boxes; nothing here knows what a requirement
//! is. The boxes matter because they can contradict an agent's self-report.

use crate::core::run::Run;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MARKDOWN: &[&str] = &["md", "markdown"];

/// How deep a specification folder is read: known layouts need two, and the
/// bound limits what a committed path can make the host walk.
const MAX_DEPTH: usize = 3;

/// One markdown file of a specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    /// Relative to the project root, so it reads the same on any machine.
    pub path: String,
    pub fingerprint: String,
    /// The headings, in order. No tool's section names are recognised.
    pub headings: Vec<Heading>,
    /// `- [x]`.
    pub done: u32,
    /// `- [ ]`.
    pub open: u32,
    /// Lines carrying one of the project's unresolved-question markers.
    pub questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

/// A specification as it is on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// What the change named, relative to the project root.
    pub path: String,
    /// Every markdown file under it, in path order. Empty when the path names
    /// nothing readable — a finding, not a blank.
    pub docs: Vec<Doc>,
}

impl Spec {
    /// Reads the specification at `named`, a file or a folder. Missing is not
    /// an error: it comes back with no documents and the caller reports it.
    pub fn read(root: &Path, named: &str, markers: &[String]) -> Self {
        let mut docs = Vec::new();
        // A refused path reads as absent ([`confine`] gives the reason). The
        // metadata is the link's own, so a symlink is never a document.
        if let Ok(joined) = confine(root, named) {
            let meta = std::fs::symlink_metadata(&joined).ok();
            if meta.as_ref().is_some_and(|m| m.is_file()) {
                if let Some(d) = Doc::read(root, &joined, markers) {
                    docs.push(d);
                }
            } else if meta.is_some_and(|m| m.is_dir()) {
                docs.extend(
                    markdown_files(&joined)
                        .iter()
                        .filter_map(|p| Doc::read(root, p, markers)),
                );
                // Path order, so the folder fingerprint is stable across machines.
                docs.sort_by(|a, b| a.path.cmp(&b.path));
            }
        }
        Self {
            path: named.to_string(),
            docs,
        }
    }

    /// `(checked, total)` boxes, from `tasks.md` only — the one filename all
    /// three layouts share. Other boxes (e.g. `checklists/`) grade the
    /// specification, not the feature. With no `tasks.md` (a one-file
    /// specification), every box counts rather than reporting zero.
    pub fn tasks(&self) -> (u32, u32) {
        let named = self.docs.iter().any(|d| is_task_list(&d.path));
        self.docs
            .iter()
            .filter(|d| !named || is_task_list(&d.path))
            .fold((0, 0), |(d, t), doc| {
                (d + doc.done, t + doc.done + doc.open)
            })
    }

    /// How many lines are marked as unresolved questions, never counting
    /// `checklists/`: its rows are assertions *about* markers ("No [NEEDS
    /// CLARIFICATION] markers remain"), not questions.
    pub fn questions(&self) -> usize {
        self.question_lines().count()
    }

    /// Every heading under the folder, in reading order: what (`spec.md`,
    /// `requirements.md`, `proposal.md`), how (`plan.md`, `design.md`), the
    /// task list, everything else, and `checklists/` last.
    pub fn outline(&self) -> Vec<Heading> {
        let mut docs: Vec<&Doc> = self.docs.iter().collect();
        docs.sort_by_key(|d| (reading_rank(&self.path, &d.path), d.path.clone()));
        docs.iter().flat_map(|d| d.headings.clone()).collect()
    }

    /// Every unresolved question, with its document. The single filtered
    /// source for both the count and the list, so they cannot disagree.
    pub fn question_lines(&self) -> impl Iterator<Item = (&str, &str)> {
        self.docs
            .iter()
            .filter(|d| !is_checklist(&d.path))
            .flat_map(|d| {
                d.questions
                    .iter()
                    .map(move |q| (d.path.as_str(), q.as_str()))
            })
    }

    /// One fingerprint over every document, in path order; `None` when there
    /// are none, so *absent* differs from *empty*. Stable across toolchains
    /// (not `DefaultHasher`), because it is stored and compared later.
    pub fn fingerprint(&self) -> Option<String> {
        if self.docs.is_empty() {
            return None;
        }
        let mut h = crate::core::hash::Rolling::new();
        for d in &self.docs {
            h.push_str(&d.path);
            h.push_str(&d.fingerprint);
        }
        Some(h.hex())
    }
}

/// Where a document comes in reading order within the folder `base`.
fn reading_rank(base: &str, path: &str) -> u8 {
    let rel = path
        .strip_prefix(base.trim_end_matches('/'))
        .map_or(path, |r| r.trim_start_matches('/'));
    if is_checklist(path) {
        return 5;
    }
    match rel {
        "spec.md" | "requirements.md" | "proposal.md" => 0,
        "plan.md" | "design.md" => 1,
        "tasks.md" => 2,
        r if !r.contains('/') => 3,
        _ => 4,
    }
}

/// Every markdown file under a folder, in path order, to [`MAX_DEPTH`]. Shared
/// by the document reader and the trace so their bounds cannot drift.
fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, 0, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Symlinks are not followed: a link out of the repository is the
        // containment hole `confine` closes at the top level.
        if entry.file_type().is_ok_and(|t| t.is_symlink()) {
            continue;
        }
        if path.is_dir() {
            walk(&path, depth + 1, out);
        } else if is_markdown(&path) {
            out.push(path);
        }
    }
}

/// A path as it is written into a record: relative to the root, with `/`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Whether a document is a feature's task list: file name `tasks.md` at any
/// depth, case-insensitively.
fn is_task_list(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case("tasks.md"))
}

fn is_markdown(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MARKDOWN.contains(&e.to_ascii_lowercase().as_str()))
}

impl Doc {
    fn read(root: &Path, path: &Path, markers: &[String]) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let fingerprint = crate::core::hash::hex(&bytes);
        // Lossy: a stray non-UTF-8 byte must not erase the specification.
        let text = String::from_utf8_lossy(&bytes);

        let mut headings = Vec::new();
        let (mut done, mut open) = (0u32, 0u32);
        let mut questions = Vec::new();
        let mut fenced = false;
        for line in text.lines() {
            let t = line.trim_start();
            // Fenced examples (common in templates) are not structure.
            if t.starts_with("```") || t.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            if let Some(rest) = t.strip_prefix('#') {
                let extra = rest.chars().take_while(|c| *c == '#').count();
                let level = 1 + extra;
                let text = rest[extra..].trim();
                // `#hashtag` is not a heading; ATX requires the space.
                if level <= 6 && rest[extra..].starts_with(' ') && !text.is_empty() {
                    headings.push(Heading {
                        level: level as u8,
                        text: text.trim_end_matches('#').trim().to_string(),
                    });
                }
            }
            match task_box(t) {
                Some(true) => done += 1,
                Some(false) => open += 1,
                None => {}
            }
            // The markers are the project's words, matched case-insensitively
            // per line. A ticked box is a finished check, not a question, and a
            // marker inside backticks is a mention, not a marker.
            if task_box(t) != Some(true)
                && markers.iter().any(|m| {
                    outside_code_spans(line)
                        .to_lowercase()
                        .contains(&m.to_lowercase())
                })
            {
                questions.push(line.trim().to_string());
            }
        }

        Some(Self {
            path: relative(root, path),
            fingerprint,
            headings,
            done,
            open,
            questions,
        })
    }
}

/// How far a task list has moved. `0 of 0` is unrepresentable: with no boxes a
/// caller gets `None`, never a full or empty bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Progress {
    pub done: u32,
    /// Always greater than zero.
    pub total: u32,
}

impl Progress {
    pub fn of(done: u32, total: u32) -> Option<Self> {
        (total > 0).then_some(Self { done, total })
    }

    pub fn open(&self) -> u32 {
        self.total.saturating_sub(self.done)
    }

    pub fn complete(&self) -> bool {
        self.done >= self.total
    }
}

/// One unresolved question, with the document it is in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Question {
    /// Relative to the project root.
    pub path: String,
    /// The line, in the project's own words.
    pub text: String,
}

/// A specification as a surface needs it: outline, progress, open questions.
/// Composed here, from the same [`Spec`] as `SpecStamp`, so the certificate and
/// the surfaces cannot count differently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Plan {
    /// As the change named it, relative to the project root.
    pub path: String,
    /// False when the named specification is not there — a finding.
    pub present: bool,
    /// How many markdown documents it covers.
    pub files: u32,
    /// `None` where there is no task list.
    pub progress: Option<Progress>,
    /// Which bound this reading hit, if any; `None` means all of it was read.
    pub truncated: Option<String>,
    /// Bounded; the full count is [`Self::open_questions`].
    pub questions: Vec<Question>,
    pub open_questions: u32,
    /// The headings, in reading order.
    pub outline: Vec<Heading>,
    pub fingerprint: Option<String>,
}

/// The most question lines one plan hands to a surface (the inbox shows one
/// item with the count regardless).
const MAX_QUESTIONS: usize = 20;

impl Plan {
    /// Reads the specification a change names.
    pub fn read(root: &Path, named: &str, markers: &[String]) -> Self {
        let spec = Spec::read(root, named, markers);
        let (done, total) = spec.tasks();
        let questions: Vec<Question> = spec
            .question_lines()
            .take(MAX_QUESTIONS)
            .map(|(path, text)| Question {
                path: path.to_string(),
                text: text.to_string(),
            })
            .collect();
        let all_questions = spec.questions();
        Self {
            path: named.to_string(),
            present: !spec.docs.is_empty(),
            files: spec.docs.len() as u32,
            progress: Progress::of(done, total),
            truncated: (all_questions > MAX_QUESTIONS).then(|| {
                format!(
                    "{all_questions} lines are marked unresolved; the first \
                     {MAX_QUESTIONS} are here"
                )
            }),
            open_questions: all_questions as u32,
            questions,
            outline: spec.outline(),
            fingerprint: spec.fingerprint(),
        }
    }

    /// Whether a *done* verdict against this plan is worth a second look.
    /// Informs an approval; never blocks one.
    pub fn contradicts_done(&self) -> bool {
        !self.present || self.open_questions > 0 || self.progress.is_some_and(|p| !p.complete())
    }
}

/// Whether this document is under `checklists/`, which grades the
/// specification: its boxes are not progress and its markers not questions.
fn is_checklist(path: &str) -> bool {
    path.split('/').any(|seg| seg == "checklists")
}

/// The line with backtick spans removed (N backticks close on exactly N). An
/// unclosed run keeps the rest of the line, so a typo cannot hide a marker.
fn outside_code_spans(line: &str) -> String {
    let b: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != '`' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let ticks = b[i..].iter().take_while(|&&c| c == '`').count();
        let body = i + ticks;
        let mut j = body;
        let close = loop {
            while j < b.len() && b[j] != '`' {
                j += 1;
            }
            if j >= b.len() {
                break None;
            }
            let run = b[j..].iter().take_while(|&&c| c == '`').count();
            if run == ticks {
                break Some(j);
            }
            j += run;
        };
        match close {
            Some(c) => i = c + ticks,
            None => {
                out.extend(&b[i..]);
                break;
            }
        }
    }
    out
}

/// `- [ ]` / `- [x]`, in the spellings GitHub renders, including numbered
/// lists (`1. [ ]`, as OpenSpec and Kiro write).
fn task_box(line: &str) -> Option<bool> {
    let (marker, rest) = line.split_once(' ')?;
    if !is_list_marker(marker) {
        return None;
    }
    // The box has to close, or `[NEEDS CLARIFICATION]` at the head of a bullet
    // counts as an unticked task.
    match rest.trim_start().as_bytes() {
        [b'[', b' ', b']', ..] => Some(false),
        [b'[', b'x' | b'X', b']', ..] => Some(true),
        _ => None,
    }
}

/// `-`, `*`, `+`, or `12.` / `12)`: the word before a list item's text.
fn is_list_marker(marker: &str) -> bool {
    let bullet = matches!(marker, "-" | "*" | "+");
    let ordered = marker.len() >= 2
        && marker.ends_with(['.', ')'])
        && marker[..marker.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit());
    bullet || ordered
}

// ---------------------------------------------------------------------------
// Where a specification may be read from
// ---------------------------------------------------------------------------

/// A path under the project root, or the sentence saying why it may not be
/// read. Paths here are untrusted text. Refused: leaving the root as written
/// (`../`, absolute, `~`); resolving outside it via a linked directory; and a
/// symbolic link at the named path itself. A nonexistent path is not refused:
/// absence is a finding the caller reports.
pub fn confine(root: &Path, named: &str) -> Result<PathBuf, String> {
    // Checked before joining: `within` reads a leading `~` as home.
    if !crate::core::policy::within(root, Path::new(named)) {
        return Err(format!("`{named}` is outside the project"));
    }
    let joined = root.join(named);
    let Ok(meta) = std::fs::symlink_metadata(&joined) else {
        return Ok(joined);
    };
    if meta.is_symlink() {
        return Err(format!(
            "`{named}` is a symbolic link, and a link is not followed"
        ));
    }
    // Resolved on both sides: the root itself may be behind a link (macOS tmp).
    match (root.canonicalize(), joined.canonicalize()) {
        (Ok(r), Ok(j)) if j.starts_with(&r) => Ok(joined),
        _ => Err(format!(
            "`{named}` resolves to somewhere outside the project"
        )),
    }
}

// ---------------------------------------------------------------------------
// The four facts a dialect is
// ---------------------------------------------------------------------------

/// What the folders under a dialect's root look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// `NNN-slug/` (three or more digits) or `YYYYMMDD-HHMMSS-slug/`. Other
    /// folders under the root are the tool's own material.
    Numbered,
    /// `<id>/`: any folder except the tool's archive folder, if it has one.
    Named { archive: Option<&'static str> },
}

impl Shape {
    fn admits(self, name: &str) -> bool {
        match self {
            Self::Numbered => {
                let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
                let lead = digits(name);
                let rest = &name[lead..];
                let Some(rest) = rest.strip_prefix('-') else {
                    return false;
                };
                if lead == 8 {
                    // `20260925-101500-slug`: the time, then the name.
                    let time = digits(rest);
                    if time == 6 && rest[6..].starts_with('-') {
                        return rest.len() > 7;
                    }
                }
                lead >= 3 && !rest.is_empty()
            }
            Self::Named { archive } => !name.starts_with('.') && Some(name) != archive,
        }
    }
}

/// A specification layout, as four facts about its paths — and nothing else:
/// not a schema, section list or requirement notation, which the tools
/// disagree on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dialect {
    pub name: &'static str,
    /// Where the change folders are, relative to the project root.
    pub root: &'static str,
    pub change_folder: Shape,
    /// The one file whose boxes are progress on the feature.
    pub task_file: &'static str,
    /// The tool's own marker (trailing `/` for a directory). Detection is by
    /// its presence only.
    pub detected_by: &'static str,
}

/// The layouts recognised without configuration; `[spec] plans` declares
/// another.
pub const DIALECTS: &[Dialect] = &[
    Dialect {
        name: "Spec Kit",
        root: "specs",
        change_folder: Shape::Numbered,
        task_file: "tasks.md",
        detected_by: ".specify/",
    },
    Dialect {
        name: "OpenSpec",
        root: "openspec/changes",
        change_folder: Shape::Named {
            archive: Some("archive"),
        },
        task_file: "tasks.md",
        detected_by: "openspec/config.yaml",
    },
    Dialect {
        name: "Kiro",
        root: ".kiro/specs",
        change_folder: Shape::Named { archive: None },
        task_file: "tasks.md",
        detected_by: ".kiro/",
    },
];

/// The name the configured layout carries.
pub const PLAIN: &str = "Plain";

/// What a surface says for a repository with no marker and no `[spec] plans`:
/// it names what was looked for, so the reader can fix it. A test keeps it in
/// step with [`DIALECTS`].
pub const NO_LAYOUT: &str = "this project has no specification layout; Devplane recognises \
     Spec Kit (`.specify/`), OpenSpec (`openspec/config.yaml`) and Kiro (`.kiro/`); \
     `[spec] plans` declares another";

/// A layout present in one repository: a [`Dialect`], owned, since a
/// configured root is not a constant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detected {
    pub name: String,
    pub root: String,
    pub change_folder: Shape,
    pub task_file: String,
    /// What was found, or the key that declared it: `.specify/`,
    /// `openspec/config.yaml`, `.kiro/`, `[spec] plans`.
    pub detected_by: String,
}

impl Detected {
    fn of(d: &Dialect) -> Self {
        Self {
            name: d.name.to_string(),
            root: d.root.to_string(),
            change_folder: d.change_folder,
            task_file: d.task_file.to_string(),
            detected_by: d.detected_by.to_string(),
        }
    }

    /// The layout `[spec] plans` declares: every folder under it is a change,
    /// with its task list in `tasks.md`.
    pub fn plain(plans: &str) -> Self {
        Self {
            name: PLAIN.to_string(),
            root: plans.trim_end_matches('/').to_string(),
            change_folder: Shape::Named { archive: None },
            task_file: "tasks.md".to_string(),
            detected_by: "[spec] plans".to_string(),
        }
    }
}

/// Every recognised layout whose marker is present at `root`. Several at once
/// is normal, not a conflict.
pub fn detect(root: &Path) -> Vec<Detected> {
    DIALECTS
        .iter()
        .filter_map(|d| {
            let found = markers_of(d)
                .into_iter()
                .find(|m| marker_present(root, m))?;
            let mut det = Detected::of(d);
            det.detected_by = found.to_string();
            Some(det)
        })
        .collect()
}

/// OpenSpec's older marker, still the only one in some repositories.
pub const OPENSPEC_LEGACY_MARKER: &str = "openspec/project.md";

/// A dialect's marker, then any older marker the same tool wrote.
fn markers_of(d: &Dialect) -> Vec<&'static str> {
    match d.name {
        "OpenSpec" => vec![d.detected_by, OPENSPEC_LEGACY_MARKER],
        _ => vec![d.detected_by],
    }
}

/// The dialect a folder is a change of, when its path is one.
fn dialect_of(root: &Path, path: &str) -> Option<&'static Dialect> {
    DIALECTS.iter().find(|d| {
        path.strip_prefix(d.root)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|name| !name.contains('/') && d.change_folder.admits(name))
            && markers_of(d).iter().any(|m| marker_present(root, m))
    })
}

fn marker_present(root: &Path, marker: &str) -> bool {
    let want_dir = marker.ends_with('/');
    let Ok(path) = confine(root, marker.trim_end_matches('/')) else {
        return false;
    };
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() == want_dir)
}

/// One change: the folder a change answers, and where its task list is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeFolder {
    /// The root this was confined to, so the trace reads the same folder.
    #[serde(skip)]
    root: PathBuf,
    /// The layout this was listed under, if any.
    pub dialect: Option<String>,
    /// Relative to the project root.
    pub path: String,
    /// The folder's own name: `001-password-reset`, `add-rate-limit`.
    pub name: String,
    /// Relative to the project root; not checked for existence here.
    pub task_file: String,
}

impl ChangeFolder {
    /// A folder a change named, confined to the project. Unlike `Spec::read`
    /// this refuses, so *nothing there* and *not allowed to look* differ.
    pub fn at(root: &Path, path: &str) -> Result<Self, String> {
        let joined = confine(root, path)?;
        if !std::fs::symlink_metadata(&joined).is_ok_and(|m| m.is_dir()) {
            return Err(format!("`{path}` is not a folder in this project"));
        }
        let path = path.trim_end_matches('/').replace('\\', "/");
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        // A folder under a dialect's root reads citations that dialect's way.
        let dialect = dialect_of(root, &path);
        Ok(Self {
            root: root.to_path_buf(),
            dialect: dialect.map(|d| d.name.to_string()),
            task_file: format!("{path}/{}", dialect.map_or("tasks.md", |d| d.task_file)),
            name,
            path,
        })
    }
}

/// The change folders of one layout, in path order. Links are not listed, for
/// the reason [`confine`] gives.
pub fn changes(root: &Path, layout: &Detected) -> Vec<ChangeFolder> {
    let Ok(dir) = confine(root, &layout.root) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<ChangeFolder> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| layout.change_folder.admits(name))
        .filter_map(|name| {
            let mut c = ChangeFolder::at(root, &format!("{}/{name}", layout.root)).ok()?;
            c.dialect = Some(layout.name.clone());
            c.task_file = format!("{}/{}", c.path, layout.task_file);
            Some(c)
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

// ---------------------------------------------------------------------------
// Edges: a token in a heading and again in a task line
// ---------------------------------------------------------------------------

/// The shape a requirement identifier takes: a non-numeric prefix, then digits.
/// A prefix with a digit could match versions and timeouts, so bare-number
/// notations must declare a word prefix (`Req ` matches `Req 12`).
///
/// A token is the prefix at a word boundary, digits, then any word characters
/// (`FR-001`, `FR-004a`); `NFR-001` is not an `FR-001`. Case-sensitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TokenShape {
    prefix: String,
}

impl TokenShape {
    /// The shapes that cover the three recognised layouts' own templates.
    pub const DEFAULTS: &[&str] = &["FR-", "NFR-", "SC-", "REQ-", "US-", "AC-"];

    /// A shape from its prefix, or why the prefix is not one.
    pub fn new(prefix: &str) -> Result<Self, String> {
        if prefix.trim().is_empty() {
            return Err(
                "is empty: a shape is a prefix before the digits, and no prefix \
                        is a bare number"
                    .to_string(),
            );
        }
        if prefix.chars().any(char::is_numeric) {
            return Err(format!(
                "`{prefix}` contains a digit: a shape is a non-numeric prefix before \
                 the digits, so it cannot match a version or a timeout by accident"
            ));
        }
        Ok(Self {
            prefix: prefix.to_string(),
        })
    }

    pub fn defaults() -> Vec<Self> {
        Self::DEFAULTS
            .iter()
            .filter_map(|p| Self::new(p).ok())
            .collect()
    }
}

/// A token as the repository wrote it. Opaque: nothing here interprets it.
pub type Token = String;

/// One task line, where it is, so a surface can point at it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskAnchor {
    /// Relative to the project root.
    pub path: String,
    /// One-based, as an editor counts.
    pub line: u32,
    /// The line, trimmed.
    pub text: String,
    /// `- [x]`. What an agent wrote about its own work, never evidence.
    pub done: bool,
    /// Every token on the line (in Kiro, also its nested `_Requirements:` line).
    pub cites: Vec<Token>,
    /// Which occurrence of this text in its file, from one: two identical
    /// lines are two tasks.
    #[serde(skip_serializing_if = "is_first")]
    pub occurrence: u32,
}

fn is_first(n: &u32) -> bool {
    *n <= 1
}

/// What co-occurrence found in one change folder. No percentage is derived
/// from it: the tokens' meaning is unknown, so the lists are the finding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Trace {
    /// Each headed token cited by a task, with the tasks, in heading order.
    pub edges: Vec<(Token, Vec<TaskAnchor>)>,
    /// Tokens heading something no task cites.
    pub orphan_requirements: Vec<Token>,
    /// Tasks whose line carries no headed token.
    pub tasks_citing_nothing: Vec<TaskAnchor>,
    /// True when no token of any shape appears anywhere in the folder; the
    /// surface then shows [`UNRECOGNISED`] instead of the lists.
    pub unrecognised: bool,
}

/// What a surface says when [`Trace::unrecognised`] is set.
pub const UNRECOGNISED: &str = "no requirement identifier of a recognised shape appears in this \
     change, so no edges were drawn; `[spec] tokens` declares the notation this repository uses";

/// The edges of one change folder. A heading position is a markdown heading or
/// a list item's leading bold label (`- **FR-001**: …`); a task position is a
/// box line in the task file only. `checklists/` and fenced blocks are skipped,
/// and the scope is this folder.
pub fn edges(change: &ChangeFolder, shapes: &[TokenShape]) -> Trace {
    let dir = change.root.join(&change.path);
    let dialect = change.dialect.as_deref();
    let mut headed: Vec<Token> = Vec::new();
    let mut tasks: Vec<TaskAnchor> = Vec::new();
    let mut any = false;
    let head = |tok: Token, headed: &mut Vec<Token>| {
        if !headed.contains(&tok) {
            headed.push(tok);
        }
    };

    for file in markdown_files(&dir) {
        let rel = relative(&change.root, &file);
        if is_checklist(&rel) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let is_tasks = rel == change.task_file;
        let mut fenced = false;
        // Kiro: the current requirement, and the open tasks by indentation.
        let mut requirement: Option<String> = None;
        let mut open: Vec<(usize, usize)> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("```") || t.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            let indent = line.len() - t.len();
            let mut found = tokens_in(line, shapes);
            if let Some(label) = heading_label(t) {
                for tok in tokens_in(label, shapes) {
                    head(tok, &mut headed);
                }
                if dialect == Some("Spec Kit")
                    && let Some(us) = user_story(label)
                {
                    any = true;
                    head(us, &mut headed);
                }
                // A heading at requirement level or above opens a requirement
                // or ends one; `#### Acceptance Criteria` sits inside it.
                if dialect == Some("Kiro")
                    && t.starts_with('#')
                    && t.chars().take_while(|c| *c == '#').count() <= 3
                {
                    requirement = kiro_requirement(label);
                    if let Some(r) = &requirement {
                        any = true;
                        head(format!("Requirement {r}"), &mut headed);
                    }
                }
            } else if dialect == Some("Kiro")
                && !is_tasks
                && indent == 0
                && let Some(r) = &requirement
                && let Some(n) = numbered_item(t)
            {
                // `1. WHEN … THEN …` under `### Requirement 2` is criterion
                // `2.1`, which is how Kiro's tasks cite it.
                head(format!("{r}.{n}"), &mut headed);
            }
            if is_tasks && dialect == Some("Spec Kit") {
                for us in story_tags(t) {
                    if !found.contains(&us) {
                        found.push(us);
                    }
                }
            }
            if is_tasks
                && dialect == Some("Kiro")
                && let Some(cited) = kiro_citations(t)
            {
                // The citations belong to the nearest enclosing task above.
                while open.last().is_some_and(|(at, _)| *at >= indent) {
                    open.pop();
                }
                if let Some((_, task)) = open.last() {
                    let task = &mut tasks[*task];
                    for c in cited {
                        any = true;
                        let major = c.split('.').next().unwrap_or(&c).to_string();
                        for tok in [c, format!("Requirement {major}")] {
                            if !task.cites.contains(&tok) {
                                task.cites.push(tok);
                            }
                        }
                    }
                }
                continue;
            }
            any |= !found.is_empty();
            if is_tasks && let Some(done) = task_box(t) {
                let text = t.trim_end().to_string();
                let key = task_key(&text);
                let occurrence = 1 + tasks
                    .iter()
                    .filter(|a| a.path == rel && task_key(&a.text) == key)
                    .count() as u32;
                while open.last().is_some_and(|(at, _)| *at >= indent) {
                    open.pop();
                }
                open.push((indent, tasks.len()));
                tasks.push(TaskAnchor {
                    path: rel.clone(),
                    line: i as u32 + 1,
                    text,
                    done,
                    cites: found,
                    occurrence,
                });
            }
        }
    }

    // Notation not recognised: the tasks stay selectable by `file:line`.
    if !any {
        return Trace {
            tasks_citing_nothing: tasks,
            unrecognised: true,
            ..Trace::default()
        };
    }

    let mut edges = Vec::new();
    let mut orphan_requirements = Vec::new();
    for tok in &headed {
        let hits: Vec<TaskAnchor> = tasks
            .iter()
            .filter(|t| t.cites.contains(tok))
            .cloned()
            .collect();
        if hits.is_empty() {
            orphan_requirements.push(tok.clone());
        } else {
            edges.push((tok.clone(), hits));
        }
    }
    let tasks_citing_nothing = tasks
        .into_iter()
        .filter(|t| !t.cites.iter().any(|c| headed.contains(c)))
        .collect();
    Trace {
        edges,
        orphan_requirements,
        tasks_citing_nothing,
        unrecognised: false,
    }
}

/// Spec Kit's `User Story 1 - Title (Priority: P1)` heading, as the `US1`
/// its tasks cite.
fn user_story(label: &str) -> Option<Token> {
    let at = label.find("User Story ")?;
    let n: String = label[at + "User Story ".len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (!n.is_empty()).then(|| format!("US{n}"))
}

/// Spec Kit's story labels on a task line: `[US1]`.
fn story_tags(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = line[from..].find("[US") {
        let at = from + i + 3;
        let n: String = line[at..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if !n.is_empty() && line[at + n.len()..].starts_with(']') {
            out.push(format!("US{n}"));
        }
        from = at;
    }
    out
}

/// Kiro's `### Requirement 2` heading, as `2`.
fn kiro_requirement(label: &str) -> Option<String> {
    let rest = label.trim().strip_prefix("Requirement ")?;
    let n: String = rest.chars().take_while(char::is_ascii_digit).collect();
    (!n.is_empty() && rest[n.len()..].trim().is_empty()).then_some(n)
}

/// `3. WHEN …` — a numbered list item's number.
fn numbered_item(t: &str) -> Option<String> {
    let n: String = t.chars().take_while(char::is_ascii_digit).collect();
    (!n.is_empty() && t[n.len()..].starts_with(". ")).then_some(n)
}

/// Kiro's `- _Requirements: 1.1, 2.3_` line, as `["1.1", "2.3"]`.
fn kiro_citations(t: &str) -> Option<Vec<Token>> {
    let t = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .unwrap_or(t);
    let body = t
        .strip_prefix("_Requirements:")
        .or_else(|| t.strip_prefix("*Requirements:"))?;
    let body = body.trim().trim_end_matches(['_', '*']).trim();
    Some(
        body.split(',')
            .map(str::trim)
            .filter(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit() || ch == '.'))
            .map(str::to_string)
            .collect(),
    )
}

/// The text in heading position on a line, if the line has one.
fn heading_label(t: &str) -> Option<&str> {
    if let Some(rest) = t.strip_prefix('#') {
        let extra = rest.chars().take_while(|c| *c == '#').count();
        let body = &rest[extra..];
        // As the outline: ATX needs the space; at most six levels.
        return (extra < 6 && body.starts_with(' '))
            .then(|| body.trim().trim_end_matches('#').trim());
    }
    let (marker, rest) = t.split_once(' ')?;
    if !is_list_marker(marker) {
        return None;
    }
    let rest = rest.trim_start();
    for delim in ["**", "__"] {
        if let Some(inner) = rest.strip_prefix(delim)
            && let Some(end) = inner.find(delim)
        {
            return Some(&inner[..end]);
        }
    }
    None
}

/// Every token of any shape on a line, in the order written, each once.
fn tokens_in(text: &str, shapes: &[TokenShape]) -> Vec<Token> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut found: Vec<(usize, Token)> = Vec::new();
    for shape in shapes {
        let p = shape.prefix.as_str();
        let mut from = 0;
        while let Some(i) = text[from..].find(p) {
            let at = from + i;
            let after_prefix = at + p.len();
            let bounded = text[..at].chars().next_back().is_none_or(|c| !is_word(c));
            let digits: usize = text[after_prefix..]
                .chars()
                .take_while(char::is_ascii_digit)
                .map(char::len_utf8)
                .sum();
            if !bounded || digits == 0 {
                from = after_prefix;
                continue;
            }
            let tail: usize = text[after_prefix + digits..]
                .chars()
                .take_while(|c| is_word(*c))
                .map(char::len_utf8)
                .sum();
            let end = after_prefix + digits + tail;
            let tok = &text[at..end];
            if !found.iter().any(|(_, f)| f == tok) {
                found.push((at, tok.to_string()));
            }
            from = end;
        }
    }
    found.sort_by_key(|(at, _)| *at);
    found.into_iter().map(|(_, t)| t).collect()
}

// ---------------------------------------------------------------------------
// Task → run: what was sent, and what the boxes said when it closed
// ---------------------------------------------------------------------------

/// A task's key: the line without its box, trimmed. Text, not a line number,
/// so it survives edits above it, a tick and a reordering.
pub fn task_key(line: &str) -> String {
    let t = line.trim();
    if let Some((marker, rest)) = t.split_once(' ')
        && is_list_marker(marker)
    {
        let rest = rest.trim_start();
        for bx in ["[ ]", "[x]", "[X]"] {
            if let Some(body) = rest.strip_prefix(bx) {
                return body.trim().to_string();
            }
        }
    }
    t.to_string()
}

impl TaskAnchor {
    /// The identity of this task: its text (see [`task_key`]), and which
    /// occurrence of that text in its file it is when it is not the first.
    pub fn key(&self) -> String {
        keyed(task_key(&self.text), self.occurrence)
    }
}

fn keyed(text: String, occurrence: u32) -> String {
    match occurrence {
        0 | 1 => text,
        n => format!("{text} #{n}"),
    }
}

/// One task sent to a run, as the run's record carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct SentTask {
    /// The task file, relative to the project root.
    pub path: String,
    /// The key — the line without its box.
    pub text: String,
    /// The tokens the line cited when sent.
    pub cites: Vec<Token>,
    /// Where it was when sent; for the editor link, never for identity.
    pub line: u32,
    /// Which occurrence of `text` in its file, from one; part of the identity.
    #[serde(default = "one")]
    pub occurrence: u32,
}

fn one() -> u32 {
    1
}

/// The tasks as a prompt carries them: one `- <text>` line each, which is
/// what `{tasks}` resolves to and what the appended *Tasks:* block holds.
pub fn tasks_block(sent: &[SentTask]) -> String {
    sent.iter()
        .map(|t| format!("- {}", t.text))
        .collect::<Vec<_>>()
        .join("\n")
}

impl SentTask {
    fn of(a: &TaskAnchor) -> Self {
        Self {
            path: a.path.clone(),
            text: task_key(&a.text),
            cites: a.cites.clone(),
            line: a.line,
            occurrence: a.occurrence.max(1),
        }
    }

    /// The same identity as [`TaskAnchor::key`].
    pub fn key(&self) -> String {
        keyed(self.text.clone(), self.occurrence)
    }
}

/// Every task the trace saw, once each by place (not text), in file order —
/// without the doubling of a task under several edges.
fn every_task(trace: &Trace) -> Vec<&TaskAnchor> {
    let mut out: Vec<&TaskAnchor> = Vec::new();
    for a in trace
        .edges
        .iter()
        .flat_map(|(_, tasks)| tasks.iter())
        .chain(trace.tasks_citing_nothing.iter())
    {
        if !out.iter().any(|o| o.path == a.path && o.line == a.line) {
            out.push(a);
        }
    }
    out.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    out
}

/// The keys of every ticked task the trace saw — what a run's close records.
pub fn ticked_keys(trace: &Trace) -> Vec<String> {
    every_task(trace)
        .into_iter()
        .filter(|t| t.done)
        .map(TaskAnchor::key)
        .collect()
}

/// How many task texts a refusal names before it says *and more*.
const MOST_NAMED: usize = 20;

/// The tasks a selection names, resolved against the folder's trace. A selector
/// is a token (every task citing it) or `file:line` relative to the change
/// folder; duplicates collapse by key. A selector matching nothing refuses,
/// naming the tasks that exist, rather than silently sending fewer.
pub fn select_tasks(
    trace: &Trace,
    change_path: &str,
    selectors: &[String],
) -> Result<Vec<SentTask>, String> {
    let all = every_task(trace);
    let mut out: Vec<SentTask> = Vec::new();
    for sel in selectors {
        let sel = sel.trim();
        let by_line = sel
            .rsplit_once(':')
            .filter(|(_, n)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            .map(|(file, n)| {
                // Relative to the change folder, or to the project root.
                let file = file.trim_start_matches("./");
                let base = change_path.trim_end_matches('/');
                let path = match file.strip_prefix(base).and_then(|r| r.strip_prefix('/')) {
                    Some(_) => file.to_string(),
                    None => format!("{base}/{file}"),
                };
                (path, n.parse::<u32>().unwrap_or(0))
            });
        let hits: Vec<&TaskAnchor> = match &by_line {
            Some((path, line)) => all
                .iter()
                .copied()
                .filter(|t| t.path == *path && t.line == *line)
                .collect(),
            None => all
                .iter()
                .copied()
                .filter(|t| t.cites.iter().any(|c| c == sel))
                .collect(),
        };
        if hits.is_empty() && by_line.is_none() && trace.unrecognised {
            return Err(format!(
                "no task in {change_path} cites `{sel}`: {UNRECOGNISED}. Select a task by \
                 `file:line` instead, e.g. `tasks.md:{}`",
                all.first().map_or(1, |t| t.line)
            ));
        }
        if hits.is_empty() {
            let named: Vec<String> = all
                .iter()
                .take(MOST_NAMED)
                .map(|t| format!("{}:{} {}", t.path, t.line, t.key()))
                .collect();
            let more = all.len().saturating_sub(MOST_NAMED);
            return Err(format!(
                "no task in {change_path} matches `{sel}`; the tasks are: {}{}",
                if named.is_empty() {
                    "none — the folder has no task line of a recognised shape".to_string()
                } else {
                    named.join(", ")
                },
                if more > 0 {
                    format!(" (and {more} more)")
                } else {
                    String::new()
                }
            ));
        }
        for h in hits {
            if !out.iter().any(|s| s.key() == h.key()) {
                out.push(SentTask::of(h));
            }
        }
    }
    Ok(out)
}

/// Ticked, and seen by a passing check, as two counts that are never one.
///
/// Ticked is the agent's own word. *Seen by a passing check* means the task was
/// sent to a run that closed before a `check` that passed, so that check ran
/// over its work. It is never called *verified*: the check may have failed
/// since, and only the change is ever verified. Computed on every read and
/// `Serialize` only. Never divided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Counts {
    pub tasks: u32,
    pub ticked: u32,
    /// `None` when no gates are declared — not zero.
    pub seen_by_pass: Option<u32>,
    /// Ticked in the specification, sent to no run of this change — by key.
    pub ticked_unsent: Vec<String>,
    /// Sent to a run and not ticked when that run closed — by key.
    pub sent_unticked: Vec<String>,
}

impl Counts {
    /// *11 ticked · 9 seen by a passing check*, *… · no gates declared*, or
    /// *… · stale* when that check ran against a tree that has since moved.
    pub fn says(&self, stale: bool) -> String {
        match self.seen_by_pass {
            None => format!("{} ticked · no gates declared", self.ticked),
            Some(v) if stale => format!(
                "{} ticked · {v} seen by a passing check · stale",
                self.ticked
            ),
            Some(v) => format!("{} ticked · {v} seen by a passing check", self.ticked),
        }
    }
}

/// One requirement token's row: `REQ-3 · 3 tasks · 2 ticked · 0 seen by a passing check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct TokenRow {
    pub token: Token,
    pub tasks: u32,
    pub ticked: u32,
    /// `None` when no gates are declared; see [`Counts`].
    pub seen_by_pass: Option<u32>,
}

impl TokenRow {
    pub fn says(&self) -> String {
        format!(
            "{} tasks · {} ticked · {}",
            self.tasks,
            self.ticked,
            match self.seen_by_pass {
                Some(v) => format!("{v} seen by a passing check"),
                None => "no gates declared".to_string(),
            }
        )
    }
}

/// Whether this run was sent the task with this key.
fn was_sent(run: &Run, key: &str) -> bool {
    run.sent
        .as_ref()
        .is_some_and(|sent| sent.iter().any(|t| t.key() == key))
}

/// Whether the run closed before the change's last passing gate ran, so the
/// gate saw its work. An open run, or no passing gate, sees nothing.
fn closed_before(run: &Run, last_pass: Option<Timestamp>) -> bool {
    matches!((run.observed.as_ref(), last_pass), (Some(o), Some(p)) if o.at < p)
}

fn seen_by_pass(runs: &[&Run], key: &str, last_pass: Option<Timestamp>) -> bool {
    runs.iter()
        .any(|r| was_sent(r, key) && closed_before(r, last_pass))
}

/// The two counts over one change folder. `last_pass` is when the latest
/// passing gate report was taken; `gates_declared` false makes the second count absent.
pub fn counts(
    trace: &Trace,
    runs: &[&Run],
    gates_declared: bool,
    last_pass: Option<Timestamp>,
) -> Counts {
    let all = every_task(trace);
    let ticked: Vec<&TaskAnchor> = all.iter().copied().filter(|t| t.done).collect();
    let passed = ticked
        .iter()
        .filter(|t| seen_by_pass(runs, &t.key(), last_pass))
        .count();
    let ticked_unsent = ticked
        .iter()
        .map(|t| t.key())
        .filter(|k| !runs.iter().any(|r| was_sent(r, k)))
        .collect();
    // Judged by the last run that closed holding the task.
    let sent_unticked = all
        .iter()
        .map(|t| t.key())
        .filter(|k| {
            runs.iter()
                .filter(|r| was_sent(r, k))
                .filter_map(|r| r.observed.as_ref())
                .max_by_key(|o| o.at)
                .is_some_and(|o| !o.ticked.contains(k))
        })
        .collect();
    Counts {
        tasks: all.len() as u32,
        ticked: ticked.len() as u32,
        seen_by_pass: gates_declared.then_some(passed as u32),
        ticked_unsent,
        sent_unticked,
    }
}

/// The same fold, per requirement token, in the order the headings appear.
pub fn token_rows(
    trace: &Trace,
    runs: &[&Run],
    gates_declared: bool,
    last_pass: Option<Timestamp>,
) -> Vec<TokenRow> {
    trace
        .edges
        .iter()
        .map(|(token, tasks)| {
            let mut keys: Vec<(String, bool)> = Vec::new();
            for t in tasks {
                let k = t.key();
                if !keys.iter().any(|(seen, _)| *seen == k) {
                    keys.push((k, t.done));
                }
            }
            let ticked = keys.iter().filter(|(_, done)| *done).count();
            let passed = keys
                .iter()
                .filter(|(k, done)| *done && seen_by_pass(runs, k, last_pass))
                .count();
            TokenRow {
                token: token.clone(),
                tasks: keys.len() as u32,
                ticked: ticked as u32,
                seen_by_pass: gates_declared.then_some(passed as u32),
            }
        })
        .collect()
}

impl Spec {
    /// When the newest document under `named` was last written, by filesystem
    /// mtime — not git, so uncommitted edits count.
    pub fn changed_at(root: &Path, named: &str) -> Option<Timestamp> {
        Self::documents_touched(root, named)
            .into_iter()
            .map(|(_, at)| at)
            .max()
    }

    /// The documents under `named` written after `since`, relative to the root.
    pub fn changed_since(root: &Path, named: &str, since: Timestamp) -> Vec<String> {
        Self::documents_touched(root, named)
            .into_iter()
            .filter(|(_, at)| *at > since)
            .map(|(path, _)| path)
            .collect()
    }

    fn documents_touched(root: &Path, named: &str) -> Vec<(String, Timestamp)> {
        let Ok(joined) = confine(root, named) else {
            return Vec::new();
        };
        let files = match std::fs::symlink_metadata(&joined) {
            Ok(m) if m.is_file() => vec![joined],
            Ok(m) if m.is_dir() => markdown_files(&joined),
            _ => Vec::new(),
        };
        files
            .into_iter()
            .filter_map(|p| {
                let at = std::fs::metadata(&p).ok()?.modified().ok()?;
                let at = Timestamp::try_from(at).ok()?;
                Some((relative(root, &p), at))
            })
            .collect()
    }
}

impl crate::core::change::SpecStamp {
    /// Reads and fingerprints the specification a change names. A missing one
    /// is kept unfingerprinted rather than dropped. Built from the same
    /// [`Plan`] surfaces are served, so the two cannot disagree.
    ///
    /// [`Plan`]: crate::spec::Plan
    pub fn of(path: &str, root: &std::path::Path, markers: &[String]) -> Self {
        Self::from_plan(&crate::spec::Plan::read(root, path, markers))
    }
}

impl crate::core::change::Change {
    /// Validates the specification (a file or a folder) this change answers.
    /// Untrusted input: refused unless it exists inside the project — an error,
    /// never a silently dropped field.
    pub fn with_spec(root: &std::path::Path, spec: &str) -> Result<String, String> {
        let joined = root.join(spec);
        if !crate::core::policy::within(root, &joined) {
            return Err(format!("{spec} is outside the project"));
        }
        if !joined.exists() {
            return Err(format!("{spec} is not in this project"));
        }
        // A folder with no Markdown is a typo; catch it now, not at the gate.
        if joined.is_dir() && crate::spec::Spec::read(root, spec, &[]).docs.is_empty() {
            return Err(format!("{spec} holds no markdown"));
        }
        // Stored relative, so the certificate reads the same on any machine.
        Ok(joined
            .strip_prefix(root)
            .unwrap_or(&joined)
            .to_string_lossy()
            .replace('\\', "/"))
    }
}

/// The `[spec]` section's questions that are answered by reading the
/// repository.
impl crate::core::config::SpecSection {
    /// The immediate children of the plans directory, in path order. Which
    /// one is being worked to is answered only by a change naming it.
    pub fn plan_paths(&self, root: &std::path::Path) -> Vec<String> {
        // Every layout present — the recognised dialects by their own marker,
        // and `[spec] plans` when declared. A Spec Kit repository with no
        // `devplane.toml` is read by its `.specify/`, not skipped.
        // `layouts` drops `plans` when a dialect already covers its root, but a
        // declared folder counts every subfolder — numbered or not — so it is
        // enumerated in its own right here.
        let mut layouts = crate::spec::detect(root);
        if let Some(dir) = self.plans.as_deref() {
            layouts.push(crate::spec::Detected::plain(dir));
        }
        let mut out: Vec<String> = layouts
            .iter()
            .flat_map(|layout| crate::spec::changes(root, layout))
            .map(|c| c.path)
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The recognised layouts present, plus the one `plans` declares unless a
    /// recognised layout already covers that root. Empty means
    /// [`crate::spec::NO_LAYOUT`].
    pub fn layouts(&self, root: &std::path::Path) -> Vec<crate::spec::Detected> {
        let mut out = crate::spec::detect(root);
        if let Some(dir) = self.plans.as_deref() {
            let plain = crate::spec::Detected::plain(dir);
            if !out.iter().any(|d| d.root == plain.root) {
                out.push(plain);
            }
        }
        out
    }

    /// The edges of one change, under this repository's own shapes.
    pub fn trace(&self, change: &crate::spec::ChangeFolder) -> crate::spec::Trace {
        crate::spec::edges(change, &self.token_shapes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_kit_repository_with_no_devplane_toml_lists_its_specs() {
        let root = tmp();
        std::fs::create_dir_all(root.join(".specify")).unwrap();
        for f in ["001-replicas-honest", "002-stream-assertions"] {
            std::fs::create_dir_all(root.join("specs").join(f)).unwrap();
            std::fs::write(root.join("specs").join(f).join("spec.md"), "# S\n").unwrap();
        }
        let config = crate::core::config::SpecSection::default();
        let paths = config.plan_paths(&root);
        assert_eq!(
            paths,
            ["specs/001-replicas-honest", "specs/002-stream-assertions"],
            "a detected Spec Kit layout was not enumerated"
        );
    }

    fn tmp() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-spec-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_task_list_is_counted_in_every_shape_the_field_writes_one() {
        for (line, want) in [
            ("- [ ] T001 Create project structure", Some(false)),
            ("- [x] T002 Initialize the project", Some(true)),
            ("- [X] done, shouting", Some(true)),
            ("* [ ] a star bullet", Some(false)),
            ("+ [x] a plus bullet", Some(true)),
            ("1. [ ] 1.1 Add theme context provider", Some(false)),
            ("12) [x] an ordered marker with a paren", Some(true)),
            // Not tasks.
            ("- an ordinary bullet", None),
            ("- [NEEDS CLARIFICATION] which timeout?", None),
            ("-[ ] no space after the marker", None),
            ("- [~] not a box this renders", None),
            ("some prose - [ ] mid-sentence", None),
        ] {
            assert_eq!(task_box(line.trim_start()), want, "{line}");
        }
    }

    #[test]
    fn a_specification_is_a_folder_and_its_progress_is_the_sum_of_its_boxes() {
        let root = tmp();
        let feature = root.join("specs/001-password-reset");
        std::fs::create_dir_all(feature.join("contracts")).unwrap();
        std::fs::write(
            feature.join("spec.md"),
            "# Password reset\n\n## Requirements\n\n- **FR-001** The system MUST send one mail\n\
             - **FR-002** [NEEDS CLARIFICATION: which timeout?]\n",
        )
        .unwrap();
        std::fs::write(
            feature.join("tasks.md"),
            "# Tasks\n\n## Phase 1\n\n- [x] T001 Scaffold\n- [x] T002 Wire the route\n\
             - [ ] T003 Send the mail\n\n```\n- [ ] an example in a fence\n```\n",
        )
        .unwrap();
        std::fs::write(
            feature.join("contracts/api.md"),
            "# API\n\n- [ ] T004 Draft\n",
        )
        .unwrap();
        std::fs::write(feature.join("data.json"), "{}").unwrap();

        let markers = vec!["NEEDS CLARIFICATION".to_string()];
        let spec = Spec::read(&root, "specs/001-password-reset", &markers);

        assert_eq!(
            spec.docs
                .iter()
                .map(|d| d.path.as_str())
                .collect::<Vec<_>>(),
            [
                "specs/001-password-reset/contracts/api.md",
                "specs/001-password-reset/spec.md",
                "specs/001-password-reset/tasks.md",
            ],
            "every markdown file under it, in path order, and nothing else"
        );
        assert_eq!(
            spec.tasks(),
            (2, 3),
            "the fenced example is a template's illustration, and the box in \
             contracts/ is not the task list: where a tasks.md exists, it is"
        );
        assert_eq!(spec.questions(), 1);
        assert!(
            spec.docs[1].questions[0].contains("which timeout"),
            "the line, so a person can read it"
        );

        let outline: Vec<_> = spec.docs[2]
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect();
        assert_eq!(outline, [(1, "Tasks"), (2, "Phase 1")]);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_fingerprint_moves_when_any_document_in_the_folder_does() {
        let root = tmp();
        let feature = root.join("specs/f");
        std::fs::create_dir_all(&feature).unwrap();
        std::fs::write(feature.join("spec.md"), "# One\n").unwrap();
        std::fs::write(feature.join("tasks.md"), "- [ ] T001\n").unwrap();
        let before = Spec::read(&root, "specs/f", &[]).fingerprint().unwrap();

        std::fs::write(feature.join("tasks.md"), "- [x] T001\n").unwrap();
        let after = Spec::read(&root, "specs/f", &[]).fingerprint().unwrap();
        assert_ne!(before, after, "a ticked box is a changed specification");

        // Absent, not empty.
        let gone = Spec::read(&root, "specs/nothing-here", &[]);
        assert!(gone.fingerprint().is_none());
        assert_eq!(gone.tasks(), (0, 0));
        assert_eq!(gone.path, "specs/nothing-here");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn one_markdown_file_is_a_specification_too() {
        let root = tmp();
        std::fs::create_dir_all(root.join("specs")).unwrap();
        std::fs::write(
            root.join("specs/reset.md"),
            "# Reset\n\n- [x] one\n- [ ] two\n",
        )
        .unwrap();
        let spec = Spec::read(&root, "specs/reset.md", &[]);
        assert_eq!(spec.docs.len(), 1);
        assert_eq!(spec.tasks(), (1, 2));
        std::fs::remove_dir_all(&root).ok();
    }

    /// A mention of a marker is not a marker: in a code span, a fence, or a
    /// checklist.
    #[test]
    fn a_document_that_mentions_a_marker_has_no_question() {
        let root = tmp();
        let dir = root.join("specs/001-x");
        std::fs::create_dir_all(dir.join("checklists")).unwrap();
        std::fs::write(
            dir.join("spec.md"),
            // A specification documenting this very mechanism.
            "# X\n\nIt collects `[NEEDS CLARIFICATION]` lines by the project's own words.\n\n             ```\nA fenced [NEEDS CLARIFICATION] example.\n```\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("checklists/requirements.md"),
            // A checklist asserting the absence, in prose rather than a box.
            "# Checklist\n\n**\"No [NEEDS CLARIFICATION] markers\" — passes.**\n             - [x] No [NEEDS CLARIFICATION] markers remain\n",
        )
        .unwrap();

        let markers = vec!["NEEDS CLARIFICATION".to_string()];
        let spec = Spec::read(&root, "specs/001-x", &markers);
        assert_eq!(
            spec.questions(),
            0,
            "a mention was counted as a question: {:?}",
            spec.question_lines().collect::<Vec<_>>()
        );

        // A real one still counts.
        std::fs::write(
            dir.join("plan.md"),
            "# Plan\n\nHow long may a hold last? [NEEDS CLARIFICATION]\n",
        )
        .unwrap();
        let spec = Spec::read(&root, "specs/001-x", &markers);
        assert_eq!(spec.questions(), 1, "a real marker stopped being seen");
        assert_eq!(
            spec.question_lines().next().unwrap().0,
            "specs/001-x/plan.md"
        );
    }

    #[test]
    fn the_no_layout_sentence_names_every_dialect_and_its_marker() {
        for d in DIALECTS {
            assert!(
                NO_LAYOUT.contains(d.name),
                "{} is not in: {NO_LAYOUT}",
                d.name
            );
            assert!(
                NO_LAYOUT.contains(d.detected_by),
                "{} is not in: {NO_LAYOUT}",
                d.detected_by
            );
        }
        assert!(NO_LAYOUT.contains("[spec] plans"));
        assert!(UNRECOGNISED.contains("[spec] tokens"));
    }

    #[test]
    fn every_default_shape_is_a_non_numeric_prefix() {
        assert_eq!(TokenShape::defaults().len(), TokenShape::DEFAULTS.len());
        assert!(TokenShape::new("1.").is_err());
        assert!(TokenShape::new("").is_err());
        assert!(TokenShape::new("  ").is_err());
        assert!(
            TokenShape::new("Req ").is_ok(),
            "a word before bare numbers is a prefix"
        );
    }

    #[test]
    fn a_token_is_bounded_on_both_sides() {
        let shapes = TokenShape::defaults();
        assert_eq!(
            tokens_in("- **NFR-001**: fast (FR-002)", &shapes),
            ["NFR-001", "FR-002"]
        );
        assert_eq!(
            tokens_in("FR-004a and FR-004a again, FR- alone, xFR-9", &shapes),
            ["FR-004a"]
        );
        assert!(tokens_in("bump to 0.8.4, 1.5s, 9.9 and v2.1.273", &shapes).is_empty());
        assert!(
            tokens_in(
                "fr-001 is not FR-001's spelling",
                &[TokenShape::new("FR-").unwrap()]
            ) == ["FR-001"]
        );
    }

    #[test]
    fn a_heading_position_is_a_heading_or_a_leading_label() {
        assert_eq!(
            heading_label("## Requirements (REQ-1)"),
            Some("Requirements (REQ-1)")
        );
        assert_eq!(heading_label("- **FR-001**: the mail"), Some("FR-001"));
        assert_eq!(heading_label("1. __FR-002__ the link"), Some("FR-002"));
        assert_eq!(heading_label("- the mail (FR-001)"), None, "not leading");
        assert_eq!(
            heading_label("- [ ] **T001** a task"),
            None,
            "a box, not a label"
        );
        assert_eq!(heading_label("#hashtag"), None);
    }

    #[test]
    fn an_unclosed_code_span_does_not_swallow_a_marker() {
        assert!(outside_code_spans("a `b [NEEDS CLARIFICATION]").contains("NEEDS CLARIFICATION"));
        assert!(!outside_code_spans("a `[NEEDS CLARIFICATION]` b").contains("NEEDS CLARIFICATION"));
        assert!(
            outside_code_spans("``a`b`` [NEEDS CLARIFICATION]").contains("NEEDS CLARIFICATION")
        );
    }
}

#[cfg(test)]
mod edge_counts {
    use super::*;
    use crate::core::SessionId;
    use crate::core::run::{Observed, RunMode};

    /// The counts fixture: fifteen lines, eleven ticked, four labels.
    fn fixture() -> Trace {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/spec/counts");
        let change = ChangeFolder::at(&root, "specs/007-counts").unwrap();
        edges(&change, &TokenShape::defaults())
    }

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn run(sent: Vec<SentTask>, closed: &str, ticked: Vec<String>) -> Run {
        let mut r = Run::new(
            SessionId::new("s"),
            PathBuf::from("/repo"),
            RunMode::Driven,
            "echo",
        );
        r.sent = Some(sent);
        r.observed = Some(Observed {
            fingerprint: Some("f".into()),
            ticked,
            changed_at: None,
            at: at(closed),
        });
        r
    }

    #[test]
    fn a_task_is_keyed_by_its_text_without_the_box() {
        for (line, want) in [
            ("- [ ] T001 Do it (FR-001)", "T001 Do it (FR-001)"),
            ("  - [x] T002 Done", "T002 Done"),
            ("- [X] T003 Shouted", "T003 Shouted"),
            ("1. [ ] 1.1 numbered", "1.1 numbered"),
            ("- a plain bullet", "- a plain bullet"),
            ("prose", "prose"),
        ] {
            assert_eq!(task_key(line), want, "{line}");
        }
        // Ticking a box does not change its key.
        assert_eq!(task_key("- [ ] T001 x"), task_key("- [x] T001 x"));
    }

    /// Two runs were sent the lines citing the first three requirements, the gate passed after
    /// both closed, and two ticked lines were sent to nobody.
    #[test]
    fn eleven_ticked_nine_seen_by_a_passing_check_over_two_runs() {
        let t = fixture();
        assert!(!t.unrecognised);
        let first =
            select_tasks(&t, "specs/007-counts", &["FR-001".into(), "FR-002".into()]).unwrap();
        let second = select_tasks(&t, "specs/007-counts", &["FR-003".into()]).unwrap();
        assert_eq!(first.len(), 7, "four cite FR-001 and three cite FR-002");
        assert_eq!(second.len(), 4);

        let a = run(first, "2026-09-25T10:00:00Z", vec![]);
        let b = run(second, "2026-09-25T10:05:00Z", vec![]);
        let pass = Some(at("2026-09-25T10:10:00Z"));
        let c = counts(&t, &[&a, &b], true, pass);
        assert_eq!((c.tasks, c.ticked, c.seen_by_pass), (15, 11, Some(9)));
        assert_eq!(
            c.ticked_unsent,
            [
                "T010 Write the docs (FR-004)",
                "T011 Announce the release (FR-004)"
            ],
            "ticked by hand, sent to nobody"
        );
        assert_eq!(c.says(false), "11 ticked · 9 seen by a passing check");
        assert_eq!(
            c.says(true),
            "11 ticked · 9 seen by a passing check · stale"
        );

        // The gate passed before the second run closed: it saw none of their four.
        let early = Some(at("2026-09-25T10:02:00Z"));
        let c = counts(&t, &[&a, &b], true, early);
        assert_eq!((c.ticked, c.seen_by_pass), (11, Some(6)));

        // No gate has passed: zero.
        let c = counts(&t, &[&a, &b], true, None);
        assert_eq!(c.seen_by_pass, Some(0));
        assert_eq!(c.says(false), "11 ticked · 0 seen by a passing check");

        // No gates declared: absent, not zero.
        let c = counts(&t, &[&a, &b], false, pass);
        assert_eq!(c.seen_by_pass, None);
        assert_eq!(c.says(false), "11 ticked · no gates declared");

        let rows = token_rows(&t, &[&a, &b], true, pass);
        let flat: Vec<(String, u32, u32, Option<u32>)> = rows
            .iter()
            .map(|r| (r.token.clone(), r.tasks, r.ticked, r.seen_by_pass))
            .collect();
        assert_eq!(
            flat,
            [
                ("FR-001".to_string(), 4, 3, Some(3)),
                ("FR-002".to_string(), 3, 3, Some(3)),
                ("FR-003".to_string(), 4, 3, Some(3)),
                ("FR-004".to_string(), 4, 2, Some(0)),
            ]
        );
        assert_eq!(
            rows[3].says(),
            "4 tasks · 2 ticked · 0 seen by a passing check"
        );
        assert_eq!(
            token_rows(&t, &[], false, None)[0].says(),
            "4 tasks · 3 ticked · no gates declared"
        );
    }

    /// Sent and unticked at close is listed; the later close wins.
    #[test]
    fn a_sent_task_unticked_at_close_is_listed() {
        let t = fixture();
        let sent = select_tasks(&t, "specs/007-counts", &["FR-001".into()]).unwrap();
        let ticked_two: Vec<String> = sent.iter().take(2).map(|s| s.text.clone()).collect();
        let a = run(sent.clone(), "2026-09-25T10:00:00Z", ticked_two);
        let c = counts(&t, &[&a], true, None);
        assert_eq!(
            c.sent_unticked,
            [
                "T003 Validate the input (FR-001)",
                "T015 Remove the flag (FR-001)"
            ]
        );
        // A later close with them ticked clears it.
        let all: Vec<String> = sent.iter().map(|s| s.text.clone()).collect();
        let b = run(sent, "2026-09-25T10:30:00Z", all);
        assert!(counts(&t, &[&a, &b], true, None).sent_unticked.is_empty());
        // A run that has not closed says nothing about its boxes.
        let mut open = a.clone();
        open.observed = None;
        assert!(counts(&t, &[&open], true, None).sent_unticked.is_empty());
    }

    #[test]
    fn a_selector_is_a_token_or_a_line_and_unknown_names_the_tasks() {
        let t = fixture();
        let by_line = select_tasks(&t, "specs/007-counts", &["tasks.md:5".into()]).unwrap();
        assert_eq!(by_line.len(), 1);
        assert_eq!(by_line[0].text, "T001 Scaffold the module (FR-001)");
        assert_eq!(by_line[0].path, "specs/007-counts/tasks.md");
        assert_eq!(by_line[0].line, 5);
        assert_eq!(by_line[0].cites, ["FR-001"]);

        // Duplicates collapse by key.
        let twice = select_tasks(
            &t,
            "specs/007-counts/",
            &["FR-001".into(), "./tasks.md:5".into()],
        )
        .unwrap();
        assert_eq!(twice.len(), 4);

        let err = select_tasks(&t, "specs/007-counts", &["FR-099".into()]).unwrap_err();
        assert!(
            err.starts_with("no task in specs/007-counts matches `FR-099`; the tasks are: "),
            "{err}"
        );
        assert!(err.contains("T001 Scaffold the module (FR-001)"), "{err}");
        assert!(!err.contains("more)"), "fifteen fit in twenty: {err}");
        let err = select_tasks(&t, "specs/007-counts", &["tasks.md:4".into()]).unwrap_err();
        assert!(
            err.contains("`tasks.md:4`"),
            "a heading line is not a task: {err}"
        );
        assert!(
            select_tasks(&t, "specs/007-counts", &[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn the_folder_is_dated_by_its_newest_document() {
        let root =
            std::env::temp_dir().join(format!("vp-spec-at-{}", uuid::Uuid::new_v4().simple()));
        let dir = root.join("specs/f");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("spec.md"), "# One\n").unwrap();
        let first = Spec::changed_at(&root, "specs/f").expect("dated");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("tasks.md"), "- [ ] T001\n").unwrap();
        let second = Spec::changed_at(&root, "specs/f").expect("dated");
        assert!(second >= first);
        assert_eq!(
            Spec::changed_since(&root, "specs/f", first),
            ["specs/f/tasks.md"]
        );
        assert_eq!(Spec::changed_at(&root, "specs/nothing"), None);
        std::fs::remove_dir_all(&root).ok();
    }
}

#[cfg(test)]
mod dogfood {
    use super::*;

    /// The reader, pointed at whatever real specifications this checkout has.
    /// `specs/` is unpublished, so finding nothing is allowed. Where a feature
    /// exists, the reader is checked against `tasks.md` counted independently.
    #[test]
    fn a_real_specification_reads_and_its_progress_is_the_task_list() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let Ok(entries) = std::fs::read_dir(root.join("specs")) else {
            return;
        };
        let mut looked_at = 0;
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() || !dir.join("tasks.md").is_file() {
                continue;
            }
            looked_at += 1;
            let feature = format!("specs/{}", entry.file_name().to_string_lossy());
            let markers = vec!["NEEDS CLARIFICATION".to_string()];
            let spec = Spec::read(root, &feature, &markers);

            let names: Vec<&str> = spec.docs.iter().map(|d| d.path.as_str()).collect();
            for want in ["spec.md", "plan.md", "tasks.md"] {
                assert!(
                    names.iter().any(|n| n.ends_with(want)),
                    "{feature}: {want} is part of the feature and the reader missed it: {names:?}"
                );
            }

            // Counted independently: every box in `tasks.md`, outside fences.
            let text = std::fs::read_to_string(dir.join("tasks.md")).expect("tasks.md reads");
            let mut fenced = false;
            let (mut done, mut total) = (0u32, 0u32);
            for line in text.lines() {
                if line.trim_start().starts_with("```") {
                    fenced = !fenced;
                    continue;
                }
                if fenced {
                    continue;
                }
                match task_box(line) {
                    Some(true) => {
                        done += 1;
                        total += 1;
                    }
                    Some(false) => total += 1,
                    None => {}
                }
            }
            assert!(total > 0, "{feature}: a task list with no tasks in it");
            assert_eq!(
                spec.tasks(),
                (done, total),
                "{feature}: the reader disagrees with its own task list —                  `checklists/` is not progress on the feature"
            );
        }
        if looked_at == 0 {
            eprintln!("no specifications in this checkout — nothing read");
        }
    }
}

// ---------------------------------------------------------------------------
// The extension hook, which is somebody else's contract
// ---------------------------------------------------------------------------

/// The Spec Kit release everything below was read against (from an installed
/// copy). Like `policy::SYNTAX_MODELLED_ON`, it records provenance, not a
/// supported-versions floor.
pub const SPEC_KIT_READ_AGAINST: &str = "1.0.7";

/// Where Spec Kit looks for extension hooks, relative to the repository root.
pub const EXTENSIONS_FILE: &str = ".specify/extensions.yml";

/// The hook points Spec Kit defines — `before_` and `after_` for each of its
/// ten commands. A name not listed here is one no command looks for.
pub const HOOK_EVENTS: &[&str] = &[
    "before_analyze",
    "after_analyze",
    "before_checklist",
    "after_checklist",
    "before_clarify",
    "after_clarify",
    "before_constitution",
    "after_constitution",
    "before_converge",
    "after_converge",
    "before_implement",
    "after_implement",
    "before_plan",
    "after_plan",
    "before_specify",
    "after_specify",
    "before_tasks",
    "after_tasks",
    "before_taskstoissues",
    "after_taskstoissues",
];

/// Where Devplane registers by default: after implement, where code is written
/// (converge only appends to a task list).
pub const DEFAULT_HOOK_EVENT: &str = "after_implement";

/// The command name the entry carries. The agent turns dots into hyphens, so
/// this is invoked as `/devplane-gate`.
pub const HOOK_COMMAND: &str = "devplane.gate";

/// The entry Devplane registers, as YAML, under `hooks.<event>`.
///
/// No `condition` key: Spec Kit commands skip a hook with a non-empty
/// condition unless a `HookExecutor` evaluates it, which not every installed
/// version has — and Devplane has no condition to express. No `priority`
/// either: the executor does not sort by it. `optional: false` because an
/// optional hook is merely offered to the agent, which is no gate.
pub fn hook_entry() -> String {
    format!(
        "    - extension: devplane\n\
         \x20     command: {HOOK_COMMAND}\n\
         \x20     description: Run this project's own gates and report what they said\n\
         \x20     prompt: Run the project's gates and report the verdict verbatim.\n\
         \x20     optional: false\n\
         \x20     enabled: true\n"
    )
}

/// The skill [`HOOK_COMMAND`] resolves to: agents turn dots into hyphens.
pub const HOOK_SKILL: &str = "devplane-gate";

/// Where a skill of that name can be defined. Pure: `core` may not look up a
/// home directory. The plugin's own copy is not listed, because a
/// marketplace install's directory layout is not a documented interface.
#[must_use]
pub fn skill_locations(root: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let leaf = format!(".claude/skills/{HOOK_SKILL}/SKILL.md");
    let mut out = vec![root.join(&leaf)];
    if let Some(h) = home {
        let user = h.join(&leaf);
        if !out.contains(&user) {
            out.push(user);
        }
    }
    out
}

/// Why registering the hook here would produce a gate that cannot run: a
/// mandatory hook naming an undefined command is a step the workflow may not
/// skip and cannot perform. `None` when a skill file exists at one of
/// [`skill_locations`].
#[must_use]
pub fn unreachable_skill(root: &Path, home: Option<&Path>) -> Option<String> {
    let places = skill_locations(root, home);
    if places.iter().any(|p| p.is_file()) {
        return None;
    }
    Some(format!(
        "nothing here defines `/{HOOK_SKILL}`, which is what `{HOOK_COMMAND}` resolves to"
    ))
}

/// A whole `extensions.yml`, for a repository that has none.
pub fn extensions_file(event: &str) -> String {
    format!("hooks:\n  {event}:\n{}", hook_entry())
}

#[cfg(test)]
mod hook_tests {
    use super::*;

    #[test]
    fn the_command_and_the_skill_are_two_spellings_of_one_thing() {
        // If these drift, the hook registers cleanly and nothing can run it.
        assert_eq!(HOOK_COMMAND.replace('.', "-"), HOOK_SKILL);
    }

    #[test]
    fn a_skill_nothing_defines_is_reported_as_unreachable() {
        let tmp = std::env::temp_dir().join(format!("dp-spec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");

        assert!(unreachable_skill(&tmp, None).is_some());

        let skill = tmp.join(".claude/skills").join(HOOK_SKILL);
        std::fs::create_dir_all(&skill).expect("skill dir");
        std::fs::write(skill.join("SKILL.md"), "---\nname: x\n---\n").expect("skill file");
        assert!(unreachable_skill(&tmp, None).is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn the_places_looked_at_are_the_project_then_the_user() {
        let root = Path::new("/repo");
        let home = Path::new("/home/dev");
        let places = skill_locations(root, Some(home));
        assert_eq!(places.len(), 2);
        assert!(
            places[0].starts_with(root),
            "the project is looked at first"
        );
        assert!(places[1].starts_with(home));
        assert_eq!(skill_locations(root, None).len(), 1);
    }

    #[test]
    fn the_entry_carries_no_condition_and_is_not_optional() {
        let entry = hook_entry();
        assert!(
            !entry.contains("condition"),
            "a condition makes every agent skip the hook, silently, for ever:\n{entry}"
        );
        assert!(entry.contains("optional: false"), "{entry}");
        assert!(entry.contains(HOOK_COMMAND), "{entry}");
    }

    #[test]
    fn the_command_becomes_the_skill_the_plugin_ships() {
        assert_eq!(HOOK_COMMAND.replace('.', "-"), "devplane-gate");
    }

    #[test]
    fn the_default_event_is_one_spec_kit_defines() {
        assert!(HOOK_EVENTS.contains(&DEFAULT_HOOK_EVENT));
        assert_eq!(HOOK_EVENTS.len(), 20, "ten commands, before and after each");
    }

    #[test]
    fn a_fresh_file_is_a_whole_document() {
        let f = extensions_file(DEFAULT_HOOK_EVENT);
        assert!(f.starts_with("hooks:\n"));
        assert!(f.contains("  after_implement:\n"));
        assert!(f.contains("    - extension: devplane"));
    }
}

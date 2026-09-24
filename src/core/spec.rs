//! The specification a piece of work answers, read back.
//!
//! **No methodology is modelled here, and that is a finding rather than a
//! shortcut.** Three tools in this space document a layout — Spec Kit's
//! `specs/NNN-name/`, Kiro's `.kiro/specs/<feature>/`, OpenSpec's
//! `openspec/changes/<id>/` — and they agree on almost nothing else. A
//! requirement is `FR-001`, an EARS sentence or a `### Requirement:` heading;
//! the document names differ; all three are young and still moving. Holding a
//! table of somebody else's nouns is the mistake this project refuses
//! everywhere else.
//!
//! What they *do* share is small, stable and enough:
//!
//! - it is **Markdown**, committed to the repository;
//! - the structure is **headings**;
//! - the unit is a **folder**, not a file;
//! - progress is a **task list** in a `tasks.md`, written with `- [ ]` and
//!   `- [x]` (verified against Spec Kit's and OpenSpec's own templates), and
//!   **only** that file: a `checklists/` folder validating the specification's
//!   own quality is not progress on the feature.
//!
//! So that is what is read. The outline is the headings. The progress is the
//! boxes. Nothing here knows what a requirement is.
//!
//! **Why the boxes are worth reading at all.** An agent's end-of-task report
//! references about one action in eleven and drifts toward its plan as the run
//! leaves it, so it is worth very little on its own and is the whole point
//! beside something that can contradict it. A task list in the repository is
//! exactly that second thing: *the agent says it is finished and the
//! specification it named has eleven boxes unticked* is a sentence no exit code
//! and no self-report can produce alone.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Markdown, in the extensions these tools actually write.
const MARKDOWN: &[&str] = &["md", "markdown"];

/// How deep a specification folder is read.
///
/// Spec Kit's deepest is `contracts/`, one level under the feature; OpenSpec's
/// is `specs/<capability>/spec.md`, two. Three is room to spare and a bound on
/// what a path in a committed file can make the daemon walk.
const MAX_DEPTH: usize = 3;

/// One markdown file of a specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    /// Relative to the project root, so it reads the same on any machine.
    pub path: String,
    pub fingerprint: String,
    /// The headings, in order, with their level. The outline and nothing else:
    /// no tool's section names are recognised, because they disagree.
    pub headings: Vec<Heading>,
    /// `- [x]`.
    pub done: u32,
    /// `- [ ]`.
    pub open: u32,
    /// Lines carrying one of the words the *project* said mark an unresolved
    /// question. Empty unless it said some.
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
    /// What the work named, relative to the project root.
    pub path: String,
    /// Every markdown file under it, in path order. Empty when the path names
    /// nothing readable — which is a finding, not a blank.
    pub docs: Vec<Doc>,
}

impl Spec {
    /// Reads the specification at `named`, which may be a file or a folder.
    ///
    /// A folder because that is what every tool in this space produces: one
    /// document is the exception. Missing is not an error here — a work that
    /// names a specification nobody wrote is exactly the thing a certificate
    /// should say out loud, so it comes back with no documents and the caller
    /// says so.
    pub fn read(root: &Path, named: &str, markers: &[String]) -> Self {
        let joined = root.join(named);
        let mut docs = Vec::new();
        if joined.is_file() {
            if let Some(d) = Doc::read(root, &joined, markers) {
                docs.push(d);
            }
        } else if joined.is_dir() {
            collect(root, &joined, markers, 0, &mut docs);
            // Path order, so a fingerprint over the folder is stable and two
            // machines listing the same directory agree.
            docs.sort_by(|a, b| a.path.cmp(&b.path));
        }
        Self {
            path: named.to_string(),
            docs,
        }
    }

    /// Checked boxes, and how many there are — **from `tasks.md` only**.
    ///
    /// The figure sits beside a gate verdict answering *how much of this work
    /// is done*, so it may only count boxes that mean that. It counted every
    /// box under the path until this project pointed the reader at its own
    /// first specification and got **47** where the task list has 31: Spec Kit
    /// writes a `checklists/` folder whose boxes validate the *specification's*
    /// quality, and those are not progress on the feature.
    ///
    /// `tasks.md` is not a guess at a methodology — it is the one filename all
    /// three documented layouts share, and Spec Kit's and OpenSpec's are pinned
    /// in the claim ledger. Where a specification has none, every box counts:
    /// narrowing unconditionally made a one-file specification report no
    /// progress at all, which is a worse answer than the one it was fixing.
    pub fn tasks(&self) -> (u32, u32) {
        // Where there is a `tasks.md`, it is the task list and the other
        // documents are not. Where there is none — a one-file specification, or
        // a layout nobody here has seen — every box counts, because there is
        // nothing to tell them apart and reporting zero is a worse answer than
        // the one that was being fixed.
        let named = self.docs.iter().any(|d| is_task_list(&d.path));
        self.docs
            .iter()
            .filter(|d| !named || is_task_list(&d.path))
            .fold((0, 0), |(d, t), doc| {
                (d + doc.done, t + doc.done + doc.open)
            })
    }

    /// Lines the project's own words mark as an unresolved question — and
    /// **never from `checklists/`, for the same reason progress is not counted
    /// there.**
    ///
    /// A checklist validates the *specification's* quality, so its rows are
    /// assertions **about** markers rather than markers: Spec Kit's own row is
    /// "No [NEEDS CLARIFICATION] markers remain". A ticked box of that shape was
    /// already skipped; the prose beside it was not, and running this reader
    /// over this repository's thirteen specifications reported an unanswered
    /// question in two of them — both a checklist discussing the marker, neither
    /// a question anybody has.
    ///
    /// The same argument decides both counts, so it is applied in both places:
    /// a folder that grades the plan is not the plan.
    pub fn questions(&self) -> usize {
        self.question_lines().count()
    }

    /// Every heading under the folder, in path order.
    ///
    /// The outline and nothing else: no tool's section names are recognised,
    /// because the three documented layouts disagree about all of them.
    pub fn outline(&self) -> Vec<Heading> {
        self.docs.iter().flat_map(|d| d.headings.clone()).collect()
    }

    /// Every unresolved question, with the document it is in.
    ///
    /// **The count and the list come from one place on purpose.** `questions`
    /// used to fold over `docs` itself, which left any surface wanting the lines
    /// to fold over `docs` too — and a surface that re-derived the filter would
    /// show two questions under a heading that said none. One producer, one
    /// filter, and a caller that cannot reach the unfiltered field without
    /// meaning to.
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

    /// One fingerprint over every document, in path order.
    ///
    /// `None` when there is nothing to fingerprint, which is how *the
    /// specification was not there* stays distinguishable from *it was empty*.
    /// **Stable across toolchains**, which the hasher this used to reach for
    /// was not. `DefaultHasher`'s own documentation says its hashes *"should
    /// not be relied upon over releases"* — fine for a hash map, wrong for a
    /// value written into a record somebody reads later. The failure was
    /// precise: two gates either side of a compiler upgrade would report a
    /// specification change nobody made, and a test comparing two values from
    /// one binary cannot see it.
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

fn collect(root: &Path, dir: &Path, markers: &[String], depth: usize, out: &mut Vec<Doc>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Symlinks are not followed: the path arrives from a command line or a
        // committed file, and a link out of the repository is the containment
        // hole `with_spec` closes at the top level.
        if entry.file_type().is_ok_and(|t| t.is_symlink()) {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, markers, depth + 1, out);
        } else if is_markdown(&path)
            && let Some(d) = Doc::read(root, &path, markers)
        {
            out.push(d);
        }
    }
}

/// Whether a document is the one that carries a feature's task list.
///
/// Matched on the file name so a nested layout still works — OpenSpec's
/// `changes/<id>/tasks.md` and Spec Kit's `specs/NNN-name/tasks.md` are both
/// this — and case-insensitively, because a filename's case is the
/// filesystem's business rather than the specification's.
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
        // Lossy on purpose: a specification is text somebody wrote, and a byte
        // that is not UTF-8 is a reason to render a replacement character
        // rather than to report no specification at all.
        let text = String::from_utf8_lossy(&bytes);

        let mut headings = Vec::new();
        let (mut done, mut open) = (0u32, 0u32);
        let mut questions = Vec::new();
        let mut fenced = false;
        for line in text.lines() {
            let t = line.trim_start();
            // A fence toggles, so a `# comment` or a `- [ ]` inside an example
            // block is not counted as structure. These tools ship templates
            // with fenced examples in them.
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
            // The words are the project's, never this tool's: `[NEEDS
            // CLARIFICATION]` is Spec Kit's spelling and the next tool will
            // have another. Case-insensitive and per line, exactly as a
            // reviewer's findings are matched.
            // A **ticked** box is a finished check, not an open question, and
            // saying so is the difference between a true count and a plausible
            // one. Spec Kit's own quality checklist carries the line "No
            // [NEEDS CLARIFICATION] markers remain" — asserting the absence of
            // the very thing the marker names — and counting it reported one
            // unanswered question about a specification that had none.
            //
            // **And a marker inside backticks is a mention, not a marker.** A
            // specification that documents this very mechanism writes
            // `` `[NEEDS CLARIFICATION]` `` in a sentence about it, and the
            // reader counted its own documentation as an open question. The
            // fence rule above says the same thing about a block; a span is the
            // same argument one delimiter smaller.
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
            path: path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/"),
            fingerprint,
            headings,
            done,
            open,
            questions,
        })
    }
}

/// How far a task list has moved, when there **is** one.
///
/// **`0 of 0` is unrepresentable, and that is the whole reason this is a type.**
/// A specification with no task list is not a specification with no progress: a
/// full bar over a plan that has no boxes is the most confident wrong thing a
/// surface can say, and rendering it was one component away for as long as the
/// counts travelled as two bare integers. A caller receives `None` and has
/// nothing to divide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Progress {
    pub done: u32,
    /// Always greater than zero. A [`Progress`] only exists where boxes do.
    pub total: u32,
}

impl Progress {
    /// `None` where the specification has no boxes at all.
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

/// A specification as a surface needs it: the outline, how far it has moved,
/// and what it says it has not answered.
///
/// **Composed here rather than by each caller**, because the two counts on it
/// are also the two the done certificate stamps, and a second place deriving
/// them is a second place to get the `checklists/` rule or the `tasks.md` rule
/// wrong. `SpecStamp` and this both read one [`Spec`]; a test asserts they
/// agree for one folder at one commit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Plan {
    /// As the work named it, relative to the project root.
    pub path: String,
    /// **False when the work names a specification that is not there**, which
    /// is a finding rather than a blank — the certificate has recorded it since
    /// the certificate shipped, and nothing has ever shown it.
    pub present: bool,
    /// How many markdown documents it covers.
    pub files: u32,
    /// `None` where there is no task list. See [`Progress`].
    pub progress: Option<Progress>,
    /// **Which bound this reading hit**, where it hit one.
    ///
    /// The walk stops at three levels and the question lines are capped. A
    /// caller that reaches either gets a shorter answer, and a shorter answer
    /// that does not say it is short is a figure nobody can check. `None` means
    /// the whole specification was read.
    pub truncated: Option<String>,
    /// Bounded, because a folder can carry any number and a ranked inbox may
    /// not be flooded by one project. The count is on [`Self::open_questions`].
    pub questions: Vec<Question>,
    pub open_questions: u32,
    /// The headings, in path order. No tool's section names are recognised.
    pub outline: Vec<Heading>,
    /// Of every document under the folder. `None` when there is nothing to
    /// fingerprint.
    pub fingerprint: Option<String>,
}

/// The most question lines one plan hands to a surface.
///
/// A project gets **one** attention item naming its count, so this bound is
/// about the detail view rather than about the inbox. Forty markers in one
/// folder is a fact about that folder; four hundred lines on the wire is a
/// surface nobody can read.
const MAX_QUESTIONS: usize = 20;

impl Plan {
    /// Reads the specification a work names.
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
            // **Which bound this reading hit**, where it hit one. A shorter
            // answer that does not say it is short is a figure nobody can
            // check.
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
    ///
    /// **The sentence this feature exists for**, and it is a sentence rather
    /// than a verdict: an incomplete plan beside a passing gate informs an
    /// approval and may never block one.
    pub fn contradicts_done(&self) -> bool {
        !self.present || self.open_questions > 0 || self.progress.is_some_and(|p| !p.complete())
    }
}

/// Whether this document is a checklist rather than the plan.
/// Whether this document is a checklist rather than the plan.
///
/// `checklists/` is Spec Kit's folder for validating the specification itself.
/// Its boxes are not progress and its markers are not questions; both
/// exclusions rest on one sentence — **a folder that grades the plan is not the
/// plan** — and so both read it here.
fn is_checklist(path: &str) -> bool {
    path.split('/').any(|seg| seg == "checklists")
}

/// The line with everything inside backtick spans removed.
///
/// Markdown's rule is that a run of N backticks opens a span that the next run
/// of exactly N closes. That is enough for the case this exists for — prose
/// quoting a marker — and deliberately no more: an unclosed backtick leaves the
/// rest of the line **kept**, because dropping it would hide a real marker
/// behind a typo.
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
            // Unclosed: keep the rest verbatim rather than swallowing it.
            None => {
                out.extend(&b[i..]);
                break;
            }
        }
    }
    out
}

/// `- [ ]` / `- [x]`, in the spellings GitHub renders.
///
/// Numbered lists too: OpenSpec and Kiro both write `1. [ ] …`, and a task list
/// that is half-counted is worse than one nobody counted.
fn task_box(line: &str) -> Option<bool> {
    let (marker, rest) = line.split_once(' ')?;
    let bullet = matches!(marker, "-" | "*" | "+");
    let ordered = marker.len() >= 2
        && marker.ends_with(['.', ')'])
        && marker[..marker.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit());
    if !bullet && !ordered {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-spec-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The boxes, in the spellings these tools write.
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

    /// A folder, because that is what every one of these tools produces.
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
        // Not markdown: it is in the folder and it is not a document.
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

        // The outline is the headings and nothing is interpreted.
        let outline: Vec<_> = spec.docs[2]
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect();
        assert_eq!(outline, [(1, "Tasks"), (2, "Phase 1")]);

        std::fs::remove_dir_all(&root).ok();
    }

    /// The fingerprint covers the folder, so a change anywhere in it shows.
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

        // And a specification nobody wrote is absent rather than empty.
        let gone = Spec::read(&root, "specs/nothing-here", &[]);
        assert!(gone.fingerprint().is_none());
        assert_eq!(gone.tasks(), (0, 0));
        assert_eq!(gone.path, "specs/nothing-here");

        std::fs::remove_dir_all(&root).ok();
    }

    /// A single file still works: it is the exception, not the shape.
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

    /// **A mention of a marker is not a marker, in three shapes.**
    ///
    /// Found by pointing this reader at this repository's own thirteen
    /// specifications, which is a thing nothing had done: it reported an
    /// unanswered question in **three** of them and every one was prose about
    /// the mechanism. The reader already skipped a *ticked* checklist box —
    /// Spec Kit's "No [NEEDS CLARIFICATION] markers remain" — and that was the
    /// narrowest possible version of the rule.
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

        // And a real one still counts, or the fix is a mute button.
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

    /// An unclosed backtick keeps the rest of the line, because dropping it
    /// would hide a real marker behind a typo.
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
mod dogfood {
    use super::*;

    /// The reader, pointed at whatever real specifications this checkout has.
    ///
    /// The one test in the suite that reads something on disk rather than a
    /// fixture. `specs/` is not published — it is the maintainer's working
    /// material — so this is **allowed to find nothing** and does nothing when
    /// it does. That is not a weakened test: it
    /// is the only honest shape for one whose subject is a folder a clean
    /// checkout has no reason to contain.
    ///
    /// Where a feature *is* there, the reader is checked against a count taken
    /// a different way — `tasks.md` read directly, by name — because the defect
    /// this caught was the reader counting boxes from `checklists/` as progress
    /// and returning 47 for a list of 31. Two computations that agree is
    /// evidence; a computation compared with itself is not.
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

            // Counted the other way: one named file, every checkbox in it, and
            // nothing else in the tree. Fenced blocks are excluded here as the
            // reader excludes them, which is the one thing the two share.
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
        // Nothing to say when there is nothing there, and saying it out loud so
        // a silent pass is not mistaken for a real one.
        if looked_at == 0 {
            eprintln!("no specifications in this checkout — nothing read");
        }
    }
}

// ---------------------------------------------------------------------------
// The extension hook, which is somebody else's contract
// ---------------------------------------------------------------------------

/// Where Spec Kit looks for extension hooks, relative to the repository root.
/// The Spec Kit release this integration was read against.
///
/// **A dated fact about somebody else's tool, in the tree rather than in a
/// memory.** Everything below — the extensions file, the hook points, the
/// `optional: false` contract, the silent `condition` skip — was read from an
/// installed copy at this version. None of it is announced in a changelog this
/// product watches, so the only honest form of the claim is a version with a
/// mechanism that notices when the copy on this machine has moved.
///
/// It is the same shape as `policy::SYNTAX_MODELLED_ON`: not a compatibility
/// floor and not a supported-versions list, because nothing here agrees to
/// anything. It is the answer to *what was this read against*, which is the
/// question a person asks when the shape upstream changes.
pub const SPEC_KIT_READ_AGAINST: &str = "1.0.7";

pub const EXTENSIONS_FILE: &str = ".specify/extensions.yml";

/// The hook points Spec Kit defines — `before_` and `after_` for each of its
/// ten commands.
///
/// **Read from the installed copy rather than from documentation**, because the
/// thing that has to be true is what the agent in front of the user does. A
/// name not on this list is one no command will ever look for, so registering
/// under it would write a hook that can never fire.
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

/// Where Devplane registers by default.
///
/// `after_implement` rather than `after_converge`: `/speckit-implement` is where
/// code is written, so it is where a gate has something to check. Converge
/// assesses a codebase and appends unbuilt work to a task list — running a
/// suite after an append answers a question nobody asked in that turn.
pub const DEFAULT_HOOK_EVENT: &str = "after_implement";

/// The command name the entry carries. The agent turns dots into hyphens, so
/// this is invoked as `/devplane-gate`.
pub const HOOK_COMMAND: &str = "devplane.gate";

/// The entry Devplane registers, as YAML, under `hooks.<event>`.
///
/// **No `condition` key, and its absence is the load-bearing part.**
///
/// Every Spec Kit command skips a hook whose condition is non-empty, deferring
/// evaluation to a `HookExecutor`. When this was written there was no such
/// class, so an entry carrying a condition would look correct, be committed and
/// never fire.
///
/// **Re-checked 2026-09-21: upstream now documents a `HookExecutor`, and the
/// copy installed here still has none.** The conclusion is unchanged and the
/// reason is now a different one, which is worth writing down rather than
/// quietly leaving the old sentence to rot: Devplane has no condition to
/// express. A gate either runs after the agent writes code or it is not a gate.
/// A key whose behaviour depends on which version of somebody else's tool is
/// installed is a key to leave out — the entry then means the same thing on
/// every version, which is the property a committed file needs.
///
/// The upstream entry also gained a `priority` field, ordering hooks low-first.
/// It is omitted for the same reason and one more: the documentation says the
/// executor **does not sort by priority**, so writing one would express an
/// intention nothing acts on.
///
/// `optional: false` because an optional hook is merely *offered* to the agent,
/// and a workflow that offers a gate has no gate.
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

/// The skill the hook's command resolves to, as an agent spells it.
///
/// The entry carries `devplane.gate`; every Spec Kit command turns dots into
/// hyphens before invoking, so what the agent looks for is a skill of this name.
pub const HOOK_SKILL: &str = "devplane-gate";

/// Where a skill of that name can be defined, given a project root and a home
/// directory.
///
/// **Pure, and takes its roots as arguments**, because `core` may not go and
/// look up a home directory — the caller knows where it is and a test should not
/// have to own one.
///
/// The plugin's own copy is deliberately **not** on this list. A plugin
/// installed from the marketplace lands in a vendor-managed directory whose
/// layout is not documented as an interface, and guessing it would produce a
/// check that passes on one machine and fails on the next. What this answers is
/// the question with an exact answer: *is there a skill file here that defines
/// it?*
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

/// Why registering the hook here would produce a gate that cannot run.
///
/// **A mandatory hook is one the agent *must* invoke and wait for.** Registered
/// against a command nothing defines, it does not degrade to no gate — the
/// workflow reaches a step it is told it may not skip and cannot perform. That
/// is the same failure as a deny rule that matches nothing, one layer out: it
/// reads as a gate and is none.
///
/// `None` when a skill file exists at one of [`skill_locations`].
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
        // The agent turns dots into hyphens before invoking, so a hook naming
        // `devplane.gate` looks for a skill called `devplane-gate`. If these
        // two drift, the hook registers cleanly and nothing can ever run it.
        assert_eq!(HOOK_COMMAND.replace('.', "-"), HOOK_SKILL);
    }

    #[test]
    fn a_skill_nothing_defines_is_reported_as_unreachable() {
        let tmp = std::env::temp_dir().join(format!("dp-spec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");

        // Nothing there: a mandatory hook here would be a step the workflow may
        // not skip and cannot perform.
        assert!(unreachable_skill(&tmp, None).is_some());

        // A skill file in the project makes it reachable.
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
        // No home to look in is a value, not a failure: `core` may not go and
        // find one.
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

    /// The command name is what the agent turns into a slash command, so the
    /// dot form and the hyphen form have to be the two spellings of one thing.
    #[test]
    fn the_command_becomes_the_skill_the_plugin_ships() {
        assert_eq!(HOOK_COMMAND.replace('.', "-"), "devplane-gate");
    }

    #[test]
    fn the_default_event_is_one_spec_kit_defines() {
        assert!(HOOK_EVENTS.contains(&DEFAULT_HOOK_EVENT));
        assert_eq!(HOOK_EVENTS.len(), 20, "ten commands, before and after each");
    }

    /// A file written for a repository that has none must be the whole file,
    /// not a fragment that happens to look like one.
    #[test]
    fn a_fresh_file_is_a_whole_document() {
        let f = extensions_file(DEFAULT_HOOK_EVENT);
        assert!(f.starts_with("hooks:\n"));
        assert!(f.contains("  after_implement:\n"));
        assert!(f.contains("    - extension: devplane"));
    }
}

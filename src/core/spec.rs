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
//!   `- [x]` (verified against Spec Kit's and OpenSpec's own templates).
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
use std::path::Path;

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

    /// Checked boxes, and how many there are.
    pub fn tasks(&self) -> (u32, u32) {
        self.docs.iter().fold((0, 0), |(d, t), doc| {
            (d + doc.done, t + doc.done + doc.open)
        })
    }

    pub fn questions(&self) -> usize {
        self.docs.iter().map(|d| d.questions.len()).sum()
    }

    /// One fingerprint over every document, in path order.
    ///
    /// `None` when there is nothing to fingerprint, which is how *the
    /// specification was not there* stays distinguishable from *it was empty*.
    pub fn fingerprint(&self) -> Option<String> {
        use std::hash::{Hash, Hasher};
        if self.docs.is_empty() {
            return None;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for d in &self.docs {
            d.path.hash(&mut h);
            d.fingerprint.hash(&mut h);
        }
        Some(format!("{:016x}", h.finish()))
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

fn is_markdown(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MARKDOWN.contains(&e.to_ascii_lowercase().as_str()))
}

impl Doc {
    fn read(root: &Path, path: &Path, markers: &[String]) -> Option<Self> {
        use std::hash::{Hash, Hasher};
        let bytes = std::fs::read(path).ok()?;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut h);
        let fingerprint = format!("{:016x}", h.finish());
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
            if markers
                .iter()
                .any(|m| line.to_lowercase().contains(&m.to_lowercase()))
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
            (2, 4),
            "the fenced example is a template's illustration, not a task"
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
}

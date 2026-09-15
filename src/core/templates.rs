//! The prompts a project already has.
//!
//! A dispatch surface that makes you retype the thing you do every Tuesday is
//! a dispatch surface you stop opening, so the launcher lists what is already
//! written down: the project's portable prompts, and its Claude Code Skills.
//!
//! Listing is a read of two directories and the first few lines of each file.
//! It is deliberately **not** a YAML parser — three scalar fields are read off
//! the frontmatter by line, and anything more elaborate is ignored rather than
//! half-understood. A picker that renders a wrong description is worse than one
//! that renders none, and a real parser here would be a dependency bought for
//! a hint.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where a template came from, which is also how much of it Vibeplane can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// `.vibeplane/prompts/<name>.md` — the portable form. Used whole.
    Portable,
    /// `.claude/skills/<name>/SKILL.md` in the repository.
    ProjectSkill,
    /// The same, under the user's home directory.
    PersonalSkill,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Portable => "portable",
            Source::ProjectSkill => "skill",
            Source::PersonalSkill => "personal skill",
        }
    }
}

/// One prompt a step or a dispatch can name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub source: Source,
    /// What it is for. A skill's `description`; a portable prompt's first
    /// non-empty line, which is the closest thing it has to one.
    pub description: Option<String>,
    /// A skill's `argument-hint`, so the picker can show what it expects.
    pub hint: Option<String>,
}

/// Everything the project offers, in the order a dispatch would pick it.
///
/// Deduplicated by name, first wins — so a portable prompt shadowing a skill of
/// the same name appears once, as the one that will actually run. A picker that
/// lists both would be offering a choice that does not exist.
pub fn list(dir: &Path) -> Vec<Template> {
    let mut out: Vec<Template> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    let mut add = |t: Template| {
        if seen.insert(t.name.clone()) {
            out.push(t);
        }
    };

    for (name, path) in entries(&dir.join(".vibeplane/prompts"), false) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        add(Template {
            description: first_line(&text),
            name,
            source: Source::Portable,
            hint: None,
        });
    }
    for (root, source) in [
        (Some(dir.to_path_buf()), Source::ProjectSkill),
        (dirs::home_dir(), Source::PersonalSkill),
    ] {
        let Some(root) = root else { continue };
        for (name, path) in entries(&root.join(".claude/skills"), true) {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let (front, _) = crate::core::text::split_frontmatter(&text);
            add(Template {
                name,
                source,
                description: front.and_then(|f| field(f, "description")),
                hint: front.and_then(|f| field(f, "argument-hint")),
            });
        }
    }
    out
}

/// `(name, file)` for each entry of a template directory.
///
/// `nested` distinguishes the two shapes: a portable prompt is `<name>.md`, a
/// skill is `<name>/SKILL.md`.
fn entries(dir: &Path, nested: bool) -> Vec<(String, std::path::PathBuf)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<_> = read
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            match nested {
                true => {
                    let file = path.join("SKILL.md");
                    if !file.is_file() {
                        return None;
                    }
                    Some((path.file_name()?.to_str()?.to_string(), file))
                }
                false => {
                    if path.extension()? != "md" {
                        return None;
                    }
                    Some((path.file_stem()?.to_str()?.to_string(), path.clone()))
                }
            }
        })
        .collect();
    // Stable order: a picker whose rows move between openings is one nobody
    // builds muscle memory for.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Reads one scalar off frontmatter, by line.
///
/// Not YAML: `key: value`, quotes trimmed, anything folded or nested ignored.
/// The three fields this needs are written as scalars in every example the
/// provider documents, and guessing at the rest would render a hint that is
/// wrong — which is worse than no hint.
fn field(front: &str, key: &str) -> Option<String> {
    front
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            (k.trim() == key).then(|| v.trim().trim_matches(['"', '\'']).to_string())
        })
        // A block scalar (`>`, `|`, and the chomping variants) puts the value
        // on the following lines. Returning the indicator would render `>` as
        // the description, which is the "wrong hint" this module refuses.
        .filter(|v| !v.is_empty() && !v.starts_with('>') && !v.starts_with('|'))
}

/// The first line worth showing, for a file with no frontmatter to ask.
fn first_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("---"))
        .map(|l| crate::core::text::clip(l.trim_start_matches('#').trim(), 120))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("vp-tpl-{tag}-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_project_offers_its_prompts_and_its_skills() {
        let d = scratch("both");
        std::fs::create_dir_all(d.join(".vibeplane/prompts")).unwrap();
        std::fs::write(
            d.join(".vibeplane/prompts/implement.md"),
            "# Implement\n\nDo the thing.",
        )
        .unwrap();
        std::fs::create_dir_all(d.join(".claude/skills/review")).unwrap();
        std::fs::write(
            d.join(".claude/skills/review/SKILL.md"),
            "---\nname: review\ndescription: Review the diff and write findings\nargument-hint: <since>\n---\n\nReview it.",
        )
        .unwrap();

        let got = list(&d);
        let by = |n: &str| got.iter().find(|t| t.name == n).cloned();

        let implement = by("implement").expect("the portable prompt");
        assert_eq!(implement.source, Source::Portable);
        // No frontmatter to ask, so the first real line is the description —
        // with the heading marker stripped, because `# Implement` is a title.
        assert_eq!(implement.description.as_deref(), Some("Implement"));

        let review = by("review").expect("the skill");
        assert_eq!(review.source, Source::ProjectSkill);
        assert_eq!(
            review.description.as_deref(),
            Some("Review the diff and write findings")
        );
        assert_eq!(review.hint.as_deref(), Some("<since>"));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_shadowed_skill_is_listed_once_as_what_will_run() {
        // Resolution prefers the portable file, so listing both would offer a
        // choice that does not exist — the picker would show two rows and one
        // of them would be a lie about what happens when you press enter.
        let d = scratch("shadow");
        std::fs::create_dir_all(d.join(".vibeplane/prompts")).unwrap();
        std::fs::write(d.join(".vibeplane/prompts/review.md"), "Portable review.").unwrap();
        std::fs::create_dir_all(d.join(".claude/skills/review")).unwrap();
        std::fs::write(
            d.join(".claude/skills/review/SKILL.md"),
            "---\ndescription: skill version\n---\nx",
        )
        .unwrap();

        let got = list(&d);
        let reviews: Vec<_> = got.iter().filter(|t| t.name == "review").collect();
        assert_eq!(reviews.len(), 1, "{got:?}");
        assert_eq!(reviews[0].source, Source::Portable);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_project_with_nothing_written_down_offers_nothing() {
        let d = scratch("empty");
        assert!(list(&d).is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn frontmatter_this_reader_does_not_understand_is_skipped_not_guessed() {
        // A folded or nested value is ignored rather than rendered wrong. A
        // picker showing a wrong description is worse than one showing none.
        assert_eq!(field("description: >\n  folded text", "description"), None);
        assert_eq!(
            field("description: plain", "description").as_deref(),
            Some("plain")
        );
        assert_eq!(field("name: x", "description"), None);
    }
}

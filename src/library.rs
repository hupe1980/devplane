//! The library, where it meets a disk.
//!
//! Everything that decides anything lives in [`crate::core::library`] and is
//! pure. This module is the walk, the copy and the sidecar write — and it is
//! the **only** place in the feature that opens a file.
//!
//! # Two refusals hold the whole thing up
//!
//! **Nothing here rewrites an artefact.** A `SKILL.md` installed from the
//! library is byte-identical to the one in it, and the digest is what proves
//! it. The moment this starts templating somebody's skill it owns a format,
//! and owning a format is the thing the library exists not to do.
//!
//! **Nothing here translates between vendors.** The tempting feature — *one
//! skill, correct on Claude Code and Cursor and Copilot* — requires deciding
//! what `context: fork` means on a product with no subagents, and the honest
//! answer is that it does not mean anything. What is offered instead is a
//! report of what a target will reject.
//!
//! # And it is not a daemon
//!
//! No task, no timer, no watcher. A background process that edits files in six
//! repositories is a process that produces a commit nobody wrote in a
//! repository nobody was looking at. `sync` is a command and a button; it
//! prints what it will do and writes nothing without `--apply`.

use crate::core::library::{
    Drift, Portability, Refusal, Scope, Sidecar, TargetFacts, TreeDigest, drift, portability,
    preflight, tree_digest,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// `~/.devplane/library`.
pub fn root() -> Result<PathBuf> {
    Ok(crate::config::home()?.join("library"))
}

/// What an artefact is, which decides how it is read and where it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A directory whose entry file is `SKILL.md`, plus whatever it carries —
    /// `scripts/`, `references/`, `assets/` are conventional and their contents
    /// changing with nothing saying so is the silent failure this ends.
    Skill,
    /// A single portable markdown file, for agents that are not Claude Code.
    Prompt,
}

/// One artefact in the library.
#[derive(Debug, Clone)]
pub struct Artefact {
    pub name: String,
    pub kind: Kind,
    pub path: PathBuf,
    pub digest: TreeDigest,
    /// `None` when nobody recorded where it came from — which is a fact worth
    /// printing rather than a blank to fill in.
    pub sidecar: Option<Sidecar>,
}

/// Reads every file under `path` as `(relative path, bytes)`.
///
/// Symlinks are **not followed**: an artefact that reaches outside itself is a
/// thing this feature has no story for, and following one would make the digest
/// a statement about somebody else's directory.
fn walk(path: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut out = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?;
        for e in entries.flatten() {
            let p = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
            } else if meta.is_file() {
                let rel = p.strip_prefix(path).unwrap_or(&p).to_path_buf();
                out.push((rel, std::fs::read(&p)?));
            }
        }
    }
    Ok(out)
}

/// Digests one artefact on disk, directory or file.
///
/// **The sidecar never reaches the digest.** It is Devplane's file, not the
/// vendor's, and records facts *about* the copy rather than part of it —
/// `install` writes it inside the destination, so counting it would make every
/// freshly installed copy digest differently from its source and report as
/// drifted the moment it arrived.
///
/// Excluded here rather than added to the ignore list: that list suppresses an
/// editor's noise and *reports* each suppression, while this was never part of
/// the artefact and has nothing to report.
pub fn digest_at(path: &Path) -> Result<TreeDigest> {
    if path.is_dir() {
        let files = walk(path)?
            .into_iter()
            .filter(|(rel, _)| rel.file_name().is_none_or(|n| n != Sidecar::FILE));
        Ok(tree_digest(files))
    } else {
        // A single file is digested under its own name, so a prompt renamed is
        // a change for the same reason a file renamed inside a skill is.
        let name = path
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("artefact"));
        Ok(tree_digest(vec![(name, std::fs::read(path)?)]))
    }
}

/// Every artefact the library holds.
pub fn list() -> Result<Vec<Artefact>> {
    let root = root()?;
    let mut out = Vec::new();
    for (sub, kind) in [("skills", Kind::Skill), ("prompts", Kind::Prompt)] {
        let dir = root.join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let is_dir = p.is_dir();
            if (kind == Kind::Skill) != is_dir {
                continue;
            }
            let name = match kind {
                Kind::Skill => e.file_name().to_string_lossy().to_string(),
                Kind::Prompt => p
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
            };
            if name.is_empty() || name.starts_with('.') {
                continue;
            }
            let sidecar = read_sidecar(&p, kind);
            out.push(Artefact {
                name,
                kind,
                digest: digest_at(&p)?,
                path: p,
                sidecar,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The sidecar beside an artefact: inside it for a skill, alongside it for a
/// prompt, because a prompt is one file and has no inside.
fn sidecar_path(path: &Path, kind: Kind) -> PathBuf {
    match kind {
        Kind::Skill => path.join(Sidecar::FILE),
        Kind::Prompt => path.with_extension("md.devplane.toml"),
    }
}

fn read_sidecar(path: &Path, kind: Kind) -> Option<Sidecar> {
    let text = std::fs::read_to_string(sidecar_path(path, kind)).ok()?;
    Sidecar::parse(&text).ok()
}

/// One project's copy of one artefact.
#[derive(Debug, Clone)]
pub struct Copy {
    pub project: String,
    pub root: PathBuf,
    pub path: PathBuf,
    pub drift: Drift,
    /// Whether this project holds a copy at all.
    ///
    /// **Separate from `drift`, and found by running the command.** A project
    /// that has never had the artefact was reported as `Unchanged` — which
    /// rendered as `ok`, exactly like a project holding an identical copy. Two
    /// opposite facts, one word: *you have this and it is fine* and *you do not
    /// have this*. Coverage is the second half of what `diff` is for — *which
    /// of my six lack it* — so it cannot be spelled the same as the first.
    pub present: bool,
    /// What the ignore list suppressed getting to the copy's digest, so a
    /// suppressed difference is reportable rather than hidden.
    pub ignored: Vec<String>,
}

/// Where this artefact would live in a project, by kind.
///
/// One scope per kind for now, and deliberately so: Copilot documents project
/// skills taking precedence over personal ones with the same name, and Devplane
/// does not resolve that precedence — which one wins is the vendor's business
/// and guessing it is the kind of claim this feature refuses.
pub fn scope_for(kind: Kind) -> Scope {
    match kind {
        Kind::Skill => Scope::ClaudeProject,
        Kind::Prompt => Scope::DevplanePrompt,
    }
}

/// Coverage: every registered project, and what it holds.
///
/// Includes projects with **no** copy, because *which of my six lack this* is
/// half of what the verb is for, and a list of the ones that have it cannot
/// answer it.
pub fn coverage(artefact: &Artefact, projects: &[(String, PathBuf)]) -> Vec<Copy> {
    let scope = scope_for(artefact.kind);
    let mut out = Vec::new();
    for (name, root) in projects {
        let path = scope.path(root, &artefact.name);
        let exists = path.exists();
        let current = exists.then(|| digest_at(&path).ok()).flatten();
        // The sidecar in the *project* records what was installed there. Its
        // absence beside a copy that exists is `Unrecorded` — a skill somebody
        // put there by hand, which is the question nothing else answers.
        let installed = read_sidecar(&path, artefact.kind).map(|s| s.digest);
        // A project that never had it is not a *drift* finding — nothing moved
        // — but it is the coverage answer, so it is carried with `present`
        // false rather than dropped or dressed as `Unchanged`.
        if !exists && installed.is_none() {
            out.push(Copy {
                project: name.clone(),
                root: root.clone(),
                path,
                drift: Drift::Unchanged,
                present: false,
                ignored: Vec::new(),
            });
            continue;
        }
        let ignored = current
            .as_ref()
            .map(|_| digest_at(&path).map(|d| d.ignored).unwrap_or_default())
            .unwrap_or_default();
        out.push(Copy {
            project: name.clone(),
            root: root.clone(),
            path,
            present: true,
            drift: drift(
                installed.as_deref(),
                current.as_ref().map(|d| d.digest.as_str()),
                &artefact.digest.digest,
            ),
            ignored,
        });
    }
    out
}

/// The documented portability failures for one artefact.
///
/// Reads the entry file's frontmatter by line, exactly as the rest of this
/// product does, and reports `unread` when there is none to read — so that an
/// empty finding list never reads as *nothing will be lost*.
pub fn portability_of(artefact: &Artefact) -> Portability {
    let entry = match artefact.kind {
        Kind::Skill => artefact.path.join("SKILL.md"),
        Kind::Prompt => artefact.path.clone(),
    };
    let Ok(text) = std::fs::read_to_string(&entry) else {
        return portability(&[], true);
    };
    let (front, _) = crate::core::text::split_frontmatter(&text);
    let Some(front) = front else {
        return portability(&[], true);
    };
    let fields: Vec<(String, String)> = front
        .lines()
        .filter_map(|l| {
            // Only top-level scalars. An indented line belongs to a nested
            // value this reader does not claim to understand, and guessing at
            // it would be inventing a parser for somebody else's format.
            if l.starts_with(char::is_whitespace) || l.trim().is_empty() {
                return None;
            }
            let (k, v) = l.split_once(':')?;
            Some((
                k.trim().to_string(),
                v.trim().trim_matches(['"', '\'']).to_string(),
            ))
        })
        .collect();
    if fields.is_empty() {
        return portability(&[], true);
    }
    portability(&fields, false)
}

/// What one target would refuse, computed for every target before any write.
pub fn preflight_all(
    artefact: &Artefact,
    targets: &[(String, PathBuf, bool)],
) -> Vec<(String, PathBuf, Option<Refusal>)> {
    let scope = scope_for(artefact.kind);
    targets
        .iter()
        .map(|(name, root, trusted)| {
            let dest = scope.path(root, &artefact.name);
            let existing = dest.exists().then(|| digest_at(&dest).ok()).flatten();
            let facts = TargetFacts {
                trusted: *trusted,
                existing: existing.map(|d| d.digest),
                incoming: artefact.digest.digest.clone(),
                documented_path: true,
            };
            (name.clone(), dest, preflight(&facts))
        })
        .collect()
}

/// Copies one artefact into one target, byte for byte, and writes the sidecar.
///
/// Returns what was replaced, when anything was. **Every overwrite is reported
/// individually**: five quiet successes and one lost file is how people learn
/// to stop reading output.
pub fn install_one(artefact: &Artefact, root: &Path, by: &str) -> Result<Option<String>> {
    let dest = scope_for(artefact.kind).path(root, &artefact.name);
    let replaced = dest
        .exists()
        .then(|| digest_at(&dest).ok())
        .flatten()
        .map(|d| d.digest);

    match artefact.kind {
        Kind::Skill => {
            copy_tree(&artefact.path, &dest)?;
        }
        Kind::Prompt => {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&artefact.path, &dest)?;
        }
    }

    let side = Sidecar {
        origin: artefact
            .sidecar
            .as_ref()
            .map(|s| s.origin.clone())
            .unwrap_or_else(|| format!("library:{}", artefact.name)),
        digest: artefact.digest.digest.clone(),
        installed: jiff::Timestamp::now().to_string(),
        by: by.to_string(),
    };
    let sp = sidecar_path(&dest, artefact.kind);
    if let Some(parent) = sp.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&sp, side.to_toml())?;
    Ok(replaced)
}

/// Copies one artefact over another, byte for byte, with no sidecar written.
///
/// The `project → library` half of `sync`. It writes **no** sidecar, and that
/// asymmetry is deliberate: a sidecar records *where this came from and who
/// installed it*, and a copy taken back out of a project did not come from
/// anywhere new. Overwriting the library's provenance with the moment somebody
/// ran `sync` would destroy the only record of the artefact's actual origin.
pub fn install_into(source: &Artefact, dest: &Path) -> Result<()> {
    match source.kind {
        Kind::Skill => copy_tree(&source.path, dest),
        Kind::Prompt => {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source.path, dest)?;
            Ok(())
        }
    }
}

/// Copies a directory, skipping the sidecar so a source's provenance does not
/// become a target's.
///
/// The destination is **emptied of tracked files first**: a copy that left a
/// file the source no longer has would make the digests differ for ever, and
/// the verb would report drift it caused itself.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        std::fs::remove_dir_all(to).with_context(|| format!("replacing {}", to.display()))?;
    }
    std::fs::create_dir_all(to)?;
    for (rel, bytes) in walk(from)? {
        if rel.file_name().is_some_and(|n| n == Sidecar::FILE) {
            continue;
        }
        let dest = to.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, bytes)?;
    }
    Ok(())
}

/// Everything installed from one origin.
///
/// One of the three questions provenance exists for, and the only one the other
/// verbs do not already answer: *what else did I install from there*.
pub fn from_origin(origin: &str, artefacts: &[Artefact]) -> Vec<String> {
    artefacts
        .iter()
        .filter(|a| {
            a.sidecar
                .as_ref()
                .is_some_and(|s| s.origin.contains(origin))
        })
        .map(|a| a.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "devplane-lib-{}-{}-{}",
            tag,
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn skill_at(dir: &Path, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: review-findings\ndescription: Reviews findings\n---\n\n{body}\n"),
        )
        .unwrap();
    }

    #[test]
    fn a_directory_is_digested_from_its_files_and_a_symlink_is_not_followed() {
        let d = scratch("walk");
        skill_at(&d.join("s"), "one");
        std::fs::create_dir_all(d.join("s/scripts")).unwrap();
        std::fs::write(d.join("s/scripts/run.sh"), "echo hi").unwrap();
        let before = digest_at(&d.join("s")).unwrap();

        std::fs::write(d.join("s/scripts/run.sh"), "echo bye").unwrap();
        let after = digest_at(&d.join("s")).unwrap();
        assert_ne!(
            before.digest, after.digest,
            "a script changing under a skill is the silent failure this ends"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_installed_copy_is_byte_identical_to_its_source() {
        let lib = scratch("src");
        let proj = scratch("dst");
        skill_at(&lib.join("review-findings"), "body");
        std::fs::create_dir_all(lib.join("review-findings/references")).unwrap();
        std::fs::write(lib.join("review-findings/references/x.md"), "ref").unwrap();

        let a = Artefact {
            name: "review-findings".into(),
            kind: Kind::Skill,
            digest: digest_at(&lib.join("review-findings")).unwrap(),
            path: lib.join("review-findings"),
            sidecar: None,
        };
        assert!(install_one(&a, &proj, "tester").unwrap().is_none());

        let dest = proj.join(".claude/skills/review-findings");
        // Compared by **bytes**, not by digest: the digest is the thing under
        // test, so using it here would be the test asserting itself.
        for rel in ["SKILL.md", "references/x.md"] {
            assert_eq!(
                std::fs::read(lib.join("review-findings").join(rel)).unwrap(),
                std::fs::read(dest.join(rel)).unwrap(),
                "{rel} was not copied byte for byte"
            );
        }
        let side = read_sidecar(&dest, Kind::Skill).expect("a sidecar was written");
        assert_eq!(side.by, "tester");
        assert_eq!(side.digest, a.digest.digest);

        // And a second install reports what it replaced.
        std::fs::write(dest.join("SKILL.md"), "edited").unwrap();
        assert!(
            install_one(&a, &proj, "tester").unwrap().is_some(),
            "an overwrite is reported rather than silent"
        );
        let _ = std::fs::remove_dir_all(&lib);
        let _ = std::fs::remove_dir_all(&proj);
    }

    /// The sidecar of the source must not become the sidecar of the copy by
    /// being copied along with it — the copy's sidecar says when *it* was
    /// installed, which is a different fact.
    #[test]
    fn the_sources_own_sidecar_is_not_copied_into_the_target() {
        let lib = scratch("side-src");
        let proj = scratch("side-dst");
        let src = lib.join("s");
        skill_at(&src, "body");
        std::fs::write(
            src.join(Sidecar::FILE),
            Sidecar {
                origin: "github:acme/skills#s".into(),
                digest: "old".into(),
                installed: "2020-01-01T00:00:00Z".into(),
                by: "somebody-else".into(),
            }
            .to_toml(),
        )
        .unwrap();

        let a = Artefact {
            name: "s".into(),
            kind: Kind::Skill,
            digest: digest_at(&src).unwrap(),
            path: src,
            sidecar: read_sidecar(&lib.join("s"), Kind::Skill),
        };
        install_one(&a, &proj, "me").unwrap();
        let side = read_sidecar(&proj.join(".claude/skills/s"), Kind::Skill).unwrap();
        assert_eq!(side.by, "me", "the copy records who installed the copy");
        assert_eq!(
            side.origin, "github:acme/skills#s",
            "and carries the origin forward, which is the fact that travels"
        );
        let _ = std::fs::remove_dir_all(&lib);
        let _ = std::fs::remove_dir_all(&proj);
    }

    /// A frontmatter this reader cannot parse is `unread`, never clean.
    #[test]
    fn an_artefact_with_no_frontmatter_reports_unread_rather_than_no_findings() {
        let d = scratch("front");
        let s = d.join("s");
        std::fs::create_dir_all(&s).unwrap();
        std::fs::write(s.join("SKILL.md"), "no frontmatter here\n").unwrap();
        let a = Artefact {
            name: "s".into(),
            kind: Kind::Skill,
            digest: digest_at(&s).unwrap(),
            path: s,
            sidecar: None,
        };
        let p = portability_of(&a);
        assert!(
            p.unread,
            "absence of findings must not read as nothing to lose"
        );
        assert!(p.findings.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn what_else_came_from_one_origin_is_one_question() {
        let a = |name: &str, origin: Option<&str>| Artefact {
            name: name.into(),
            kind: Kind::Skill,
            path: PathBuf::from("/x"),
            digest: TreeDigest {
                digest: "d".into(),
                ignored: Vec::new(),
            },
            sidecar: origin.map(|o| Sidecar {
                origin: o.into(),
                digest: "d".into(),
                installed: "t".into(),
                by: "me".into(),
            }),
        };
        let all = [
            a("one", Some("github:acme/skills#one")),
            a("two", Some("github:acme/skills#two")),
            a("three", Some("github:other/pack#three")),
            a("four", None),
        ];
        assert_eq!(from_origin("github:acme/skills", &all), ["one", "two"]);
        assert!(from_origin("nothing", &all).is_empty());
    }
}

//! What governs the agent and what the build trusts — the one table.
//!
//! The weakened group reads edits to what *passing* means
//! ([`crate::core::review`]'s `check_file`). The agent can equally edit what
//! governs it — its hooks, permissions, instructions, skills and MCP servers —
//! and what the build trusts — who must review a path, which packages resolve.
//! [`control_file`] is that table, over paths and, for a manifest, over the
//! hunk's own keys and line shapes; [`Tally`] reads a lockfile's diff into
//! counts with names. Pure: diff text in, rows' words out. No file, clock,
//! network or model; nothing here says an edit is good or bad.

use std::collections::BTreeMap;

use crate::core::diff::{Hunk, Kind};
use crate::core::review::WeakKind;

/// How a manifest's changed line is recognised as a dependency declaration
/// when the table or object it sits in is not in the hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// `name = "<version req>"` or `name = { version|git|path|registry|workspace … }`.
    CargoToml,
    /// `"name": "<range | url | workspace: | file: | npm: …>"`.
    PackageJson,
}

/// The lockfile format whose entries [`Tally::of`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lock {
    /// `[[package]]` blocks: `Cargo.lock`, `poetry.lock`, `uv.lock`.
    Toml,
    /// `"node_modules/…": {` entries: `package-lock.json`, `npm-shrinkwrap.json` (v2, v3).
    Npm,
    /// `name@version:` keys: `pnpm-lock.yaml` (v6–v9).
    Pnpm,
    /// Unindented `"name@range", …:` keys: `yarn.lock` (v1 and berry).
    Yarn,
    /// One line per module and version: `go.sum`.
    GoSum,
    /// `    name (version)` under `specs:`: `Gemfile.lock`.
    Gemfile,
    /// A lockfile whose entries are not read here.
    Unread,
}

/// One entry of the table: the kind of row, which hunks count, and the words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub kind: WeakKind,
    /// Empty: every edit. Else only a changed line naming one, or governed by
    /// a table or object naming one, counts.
    pub keys: &'static [&'static str],
    pub shape: Option<Shape>,
    /// `Some` for a lockfile.
    pub lock: Option<Lock>,
    /// The sentence for an edit: *edits `p`, which …*.
    pub edits: String,
    /// What the file is, after its path: *which …*, *a skill the agent loads*.
    pub about: String,
}

impl Control {
    /// The sentence for a deletion.
    pub fn deletes(&self, path: &str) -> String {
        format!("deletes `{path}`, {}", self.about)
    }

    /// The sentence for a rename to a name the table does not hold.
    pub fn renames_away(&self, from: &str) -> String {
        format!(
            "renames `{from}`, {}, to a name nothing reads it by",
            self.about
        )
    }

    /// Whether a hunk is one this row is about.
    pub fn reads(&self, h: &Hunk) -> bool {
        self.keys.is_empty() || declares(h, self.keys, self.shape)
    }
}

const CARGO_KEYS: &[&str] = &["dependencies", "[patch", "[replace"];
const PACKAGE_JSON_KEYS: &[&str] = &[
    "\"dependencies\"",
    "\"devDependencies\"",
    "\"peerDependencies\"",
    "\"optionalDependencies\"",
    "\"overrides\"",
    "\"resolutions\"",
];
const PYPROJECT_KEYS: &[&str] = &[
    "dependencies",
    "optional-dependencies",
    "dependency-groups",
    "[tool.poetry.dependencies",
    "[tool.poetry.group",
];
const COMPOSER_KEYS: &[&str] = &["\"require\"", "\"require-dev\""];
const TERRAFORM_KEYS: &[&str] = &[
    "github_branch_protection",
    "github_branch_protection_v3",
    "github_repository_ruleset",
];

/// The lockfiles, by file name at any depth, and whether their entries are read.
const LOCKFILES: &[(&str, Lock)] = &[
    ("Cargo.lock", Lock::Toml),
    ("poetry.lock", Lock::Toml),
    ("uv.lock", Lock::Toml),
    ("package-lock.json", Lock::Npm),
    ("npm-shrinkwrap.json", Lock::Npm),
    ("pnpm-lock.yaml", Lock::Pnpm),
    ("yarn.lock", Lock::Yarn),
    ("go.sum", Lock::GoSum),
    ("Gemfile.lock", Lock::Gemfile),
    ("bun.lock", Lock::Unread),
    ("Pipfile.lock", Lock::Unread),
    ("pdm.lock", Lock::Unread),
    ("composer.lock", Lock::Unread),
    ("flake.lock", Lock::Unread),
];

/// What governs the agent or what the build trusts — the one table:
/// `Some` for a file whose edits are a row of kind *agent controls*,
/// *ownership* or *supply chain*. Each vendor path is the one its vendor
/// documents for a project. `devplane.toml` is not here: it is a *gate* row.
pub fn control_file(path: &str) -> Option<Control> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let row = |kind, edits: String, about: &str| Control {
        kind,
        keys: &[],
        shape: None,
        lock: None,
        edits,
        about: about.to_string(),
    };
    let which = |kind, about: &str| row(kind, format!("edits `{path}`, {about}"), about);
    let agent = |about: &str| which(WeakKind::AgentControls, about);
    let keyed = |keys, shape, edits: String, about: &str| Control {
        keys,
        shape,
        ..row(WeakKind::SupplyChain, edits, about)
    };
    let manifest = |keys, shape| {
        keyed(
            keys,
            shape,
            format!("edits the dependencies `{path}` declares"),
            "which declares the build's dependencies",
        )
    };
    match path {
        ".claude/settings.json" | ".claude/settings.local.json" => {
            return Some(agent("which sets the agent's hooks and permissions"));
        }
        ".codex/hooks.json" => return Some(agent("which sets the agent's hooks")),
        ".codex/config.toml" | ".gemini/settings.json" => {
            return Some(agent(
                "which configures the agent, its tools and MCP servers",
            ));
        }
        ".mcp.json" => return Some(agent("which gives the agent MCP servers")),
        "CODEOWNERS" | ".github/CODEOWNERS" | "docs/CODEOWNERS" => {
            return Some(which(
                WeakKind::Ownership,
                "which decides who must review a path",
            ));
        }
        ".github/settings.yml" => {
            return Some(which(
                WeakKind::Ownership,
                "which can set branch protection",
            ));
        }
        _ => {}
    }
    if path.starts_with(".github/hooks/") {
        return Some(agent("which sets the agent's hooks"));
    }
    if path.starts_with(".claude/skills/") || path.starts_with(".agents/skills/") {
        return Some(agent("a skill the agent loads"));
    }
    if name == "CLAUDE.md" || name == "AGENTS.md" {
        return Some(agent("the instructions the agent reads"));
    }
    if let Some((_, lock)) = LOCKFILES.iter().find(|(n, _)| *n == name) {
        return Some(Control {
            lock: Some(*lock),
            ..row(
                WeakKind::SupplyChain,
                format!("edits the lockfile `{path}`"),
                "which pins the packages the build resolves",
            )
        });
    }
    if name.ends_with(".tf") {
        return Some(Control {
            kind: WeakKind::Ownership,
            ..keyed(
                TERRAFORM_KEYS,
                None,
                format!("edits branch protection or a ruleset in `{path}`"),
                "which sets branch protection or a ruleset",
            )
        });
    }
    let requirements =
        name.starts_with("requirements") && (name.ends_with(".txt") || name.ends_with(".in"));
    match name {
        "Cargo.toml" => Some(manifest(CARGO_KEYS, Some(Shape::CargoToml))),
        "package.json" => Some(manifest(PACKAGE_JSON_KEYS, Some(Shape::PackageJson))),
        "pyproject.toml" => Some(manifest(PYPROJECT_KEYS, None)),
        "composer.json" => Some(manifest(COMPOSER_KEYS, None)),
        "go.mod" | "Gemfile" | "Pipfile" => Some(manifest(&[], None)),
        _ if requirements => Some(manifest(&[], None)),
        _ => None,
    }
}

// ── Manifests ────────────────────────────────────────────────────────────────

fn names_key(t: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| t.contains(k))
}

/// A line that opens a table, array or object: what the lines after it sit in.
fn opens(t: &str) -> bool {
    let t = t.trim_end();
    t.ends_with('{') || (t.ends_with('[') && !t.starts_with('['))
}

/// A line that closes the array or object a line opened.
fn closes(t: &str) -> bool {
    matches!(t.trim(), "}" | "}," | "]" | "],")
}

/// Whether a manifest hunk edits a dependency declaration: a changed line
/// names a key; or the table or object it sits in, as the hunk shows it, names
/// one; or, where the hunk does not show what it sits in, the hunk's header
/// names a key or the line is shaped as a declaration. A key in a context line
/// that governs no changed line does not count.
fn declares(h: &Hunk, keys: &[&str], shape: Option<Shape>) -> bool {
    // `None`: what the line sits in is not in the hunk.
    let mut table: Option<bool> = None;
    let mut object: Option<bool> = None;
    for (k, text) in &h.lines {
        let t = text.trim();
        let changed = *k != Kind::Context;
        let governed = object.or(table);
        if changed {
            let shaped = shape.is_some_and(|s| shaped(s, t));
            let hit = names_key(t, keys)
                || governed == Some(true)
                || (governed.is_none() && (names_key(&h.header, keys) || shaped));
            if hit && !t.is_empty() {
                return true;
            }
        }
        if t.starts_with('[') && t.ends_with(']') && !t.contains(',') {
            table = Some(names_key(t, keys));
            object = None;
        } else if opens(t) {
            object = Some(names_key(t, keys));
        } else if closes(t) {
            object = None;
        }
    }
    false
}

/// Keys of a manifest's own metadata whose values look like versions.
const NOT_A_DEPENDENCY: &[&str] = &[
    "name",
    "version",
    "rust-version",
    "edition",
    "resolver",
    "description",
    "main",
    "module",
    "types",
    "typings",
    "type",
    "license",
    "node",
    "npm",
    "pnpm",
    "yarn",
    "bun",
    "packageManager",
];

/// Whether a changed line reads as one dependency declared in that format.
fn shaped(shape: Shape, t: &str) -> bool {
    match shape {
        Shape::CargoToml => {
            let Some((name, value)) = t.split_once('=') else {
                return false;
            };
            let name = name.trim().trim_matches('"');
            let value = value.trim();
            let named = !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
                && !NOT_A_DEPENDENCY.contains(&name);
            named
                && ((value.starts_with('"') && version_like(&value[1..]))
                    || (value.starts_with('{')
                        && ["version", "git", "path", "registry", "workspace"]
                            .iter()
                            .any(|k| {
                                value.contains(&format!("{k} ="))
                                    || value.contains(&format!("{k}="))
                            })))
        }
        Shape::PackageJson => {
            let Some(rest) = t.strip_prefix('"') else {
                return false;
            };
            let Some((name, value)) = rest.split_once("\":") else {
                return false;
            };
            let value = value.trim().trim_end_matches(',');
            let Some(value) = value.strip_prefix('"') else {
                return false;
            };
            !name.is_empty()
                && !NOT_A_DEPENDENCY.contains(&name)
                && (version_like(value)
                    || [
                        "workspace:",
                        "file:",
                        "npm:",
                        "link:",
                        "git+",
                        "git:",
                        "github:",
                        "http:",
                        "https:",
                        "latest\"",
                    ]
                    .iter()
                    .any(|p| value.starts_with(p)))
        }
    }
}

fn version_like(v: &str) -> bool {
    v.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || "^~<>=*".contains(c))
}

// ── Lockfiles ────────────────────────────────────────────────────────────────

/// How many names a class shows before it counts the rest.
pub const NAMES_SHOWN: usize = 8;

/// The bound on one package name or version in a row's words.
const NAME_CHARS: usize = 80;

/// A lockfile's diff read as packages: added, removed, changed version, and
/// changed only checksum or source (*resealed*), each with names; and edits
/// to an entry whose name is in no line of its hunk. Counts and names only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    pub added: Vec<(String, String)>,
    pub removed: Vec<(String, String)>,
    pub changed: Vec<(String, String, String)>,
    pub resealed: Vec<(String, String)>,
    pub unnamed: u32,
}

/// What one line of a lockfile is to the entry it sits in.
enum Read {
    /// A new entry starts here; `single` when the line is the whole entry.
    Begin {
        name: Option<String>,
        version: Option<String>,
        other: bool,
        single: bool,
    },
    /// Leaves any entry: a section header, a non-package table.
    End,
    Name(String),
    Version(String),
    /// A checksum, integrity, resolved URL or source line of the entry.
    Other,
    /// Anything else: a dependency list, metadata.
    Skip,
}

fn indent(t: &str) -> usize {
    t.len() - t.trim_start().len()
}

fn quoted(v: &str) -> String {
    v.trim()
        .trim_end_matches(',')
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

/// `name@range` → `name`, keeping a scope's leading `@`.
fn before_at(spec: &str) -> Option<String> {
    let spec = spec.trim().trim_matches('"').trim_matches('\'');
    let spec = spec.strip_prefix('/').unwrap_or(spec);
    let first = spec.chars().next()?.len_utf8();
    let at = spec[first..].find('@')? + first;
    Some(spec[..at].to_string())
}

fn read(lock: Lock, line: &str) -> Read {
    let t = line.trim();
    if t.is_empty() {
        return Read::Skip;
    }
    match lock {
        Lock::Toml => {
            if t == "[[package]]" {
                return Read::Begin {
                    name: None,
                    version: None,
                    other: false,
                    single: false,
                };
            }
            if t.starts_with('[') && indent(line) == 0 {
                return Read::End;
            }
            if indent(line) == 0 {
                if let Some(v) = t.strip_prefix("name = ") {
                    return Read::Name(quoted(v));
                }
                if let Some(v) = t.strip_prefix("version = ") {
                    return Read::Version(quoted(v));
                }
                if t.starts_with("source") || t.starts_with("checksum") {
                    return Read::Other;
                }
            }
            match t.contains("hash") || t.contains("checksum") {
                true => Read::Other,
                false => Read::Skip,
            }
        }
        Lock::Npm => {
            let i = indent(line);
            if i <= 4 && t.ends_with('{') {
                let key = t.trim_end_matches('{').trim().trim_end_matches(':');
                let key = key.trim_matches('"');
                return match key.rfind("node_modules/") {
                    Some(at) if i == 4 => Read::Begin {
                        name: Some(key[at + "node_modules/".len()..].to_string()),
                        version: None,
                        other: false,
                        single: false,
                    },
                    _ => Read::End,
                };
            }
            if i == 6 {
                if let Some(v) = t.strip_prefix("\"version\":") {
                    return Read::Version(quoted(v));
                }
                if t.starts_with("\"resolved\"") || t.starts_with("\"integrity\"") {
                    return Read::Other;
                }
            }
            Read::Skip
        }
        Lock::Pnpm => {
            let i = indent(line);
            if i == 0 {
                return Read::End;
            }
            if i == 2 && (t.ends_with(':') || t.contains(": {")) {
                let key = t.split(": {").next().unwrap_or(t).trim_end_matches(':');
                let key = key.trim_matches('\'').trim_matches('"');
                let key = key.strip_prefix('/').unwrap_or(key);
                let key = key.split('(').next().unwrap_or(key);
                return match before_at(key) {
                    Some(name) => Read::Begin {
                        version: Some(key[name.len() + 1..].to_string()),
                        name: Some(name),
                        other: false,
                        single: false,
                    },
                    None => Read::End,
                };
            }
            match i == 4 && t.starts_with("resolution:") {
                true => Read::Other,
                false => Read::Skip,
            }
        }
        Lock::Yarn => {
            let i = indent(line);
            if t.starts_with('#') {
                return Read::Skip;
            }
            if i == 0 {
                return match t.ends_with(':') {
                    true => {
                        let first = t.trim_end_matches(':').split(',').next().unwrap_or("");
                        match before_at(first) {
                            Some(name) => Read::Begin {
                                name: Some(name),
                                version: None,
                                other: false,
                                single: false,
                            },
                            None => Read::End,
                        }
                    }
                    false => Read::End,
                };
            }
            if i == 2 {
                if let Some(v) = t
                    .strip_prefix("version: ")
                    .or_else(|| t.strip_prefix("version "))
                {
                    return Read::Version(quoted(v));
                }
                if ["resolved ", "integrity ", "checksum:", "resolution:"]
                    .iter()
                    .any(|p| t.starts_with(p))
                {
                    return Read::Other;
                }
            }
            Read::Skip
        }
        Lock::GoSum => {
            let mut parts = t.split_whitespace();
            match (parts.next(), parts.next(), parts.next()) {
                (Some(module), Some(version), Some(_)) => Read::Begin {
                    name: Some(module.to_string()),
                    version: Some(version.trim_end_matches("/go.mod").to_string()),
                    other: true,
                    single: true,
                },
                _ => Read::Skip,
            }
        }
        Lock::Gemfile => {
            let i = indent(line);
            let entry = |t: &str| {
                let (name, rest) = t.split_once(" (")?;
                let (version, _) = rest.split_once(')')?;
                let ranged = version.chars().next().is_some_and(|c| "~<>=!".contains(c));
                (!name.contains(' ') && !ranged).then(|| (name.to_string(), version.to_string()))
            };
            match (i, entry(t)) {
                (4, Some((name, version))) => Read::Begin {
                    name: Some(name),
                    version: Some(version),
                    other: false,
                    single: true,
                },
                (2, Some((name, version))) if t.contains("sha256=") => Read::Begin {
                    name: Some(name),
                    version: Some(version),
                    other: true,
                    single: true,
                },
                (0, _) => Read::End,
                _ => Read::Skip,
            }
        }
        Lock::Unread => Read::Skip,
    }
}

#[derive(Default)]
struct Entry {
    name: Option<String>,
    version: Option<String>,
    others: Vec<String>,
    touched: bool,
}

/// One side of a hunk (the old file's lines or the new's), read as entries.
/// The first entry is the one the hunk opens inside, its start out of view.
fn entries(lock: Lock, side: &[(bool, &str)]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut cur: Option<Entry> = Some(Entry::default());
    for (changed, line) in side {
        match read(lock, line) {
            Read::Begin {
                name,
                version,
                other,
                single,
            } => {
                out.extend(cur.take());
                let e = Entry {
                    name,
                    version,
                    others: match other {
                        true => vec![line.trim().to_string()],
                        false => Vec::new(),
                    },
                    touched: *changed,
                };
                match single {
                    true => out.push(e),
                    false => cur = Some(e),
                }
            }
            Read::End => out.extend(cur.take()),
            Read::Name(n) => {
                if let Some(e) = cur.as_mut() {
                    e.name = Some(n);
                    e.touched |= *changed;
                }
            }
            Read::Version(v) => {
                if let Some(e) = cur.as_mut() {
                    e.version = Some(v);
                    e.touched |= *changed;
                }
            }
            Read::Other => {
                if let Some(e) = cur.as_mut() {
                    e.others.push(line.trim().to_string());
                    e.touched |= *changed;
                }
            }
            Read::Skip => {}
        }
    }
    out.extend(cur);
    out
}

/// Per name, per version (or none in view), the entry's other lines.
type Side = BTreeMap<String, BTreeMap<Option<String>, Vec<String>>>;

impl Tally {
    /// Read from the diff text alone: each hunk's old and new lines, parsed as
    /// the format's entries, compared by name. Linear in the diff.
    pub fn of(lock: Lock, hunks: &[Hunk]) -> Tally {
        let mut old: Side = BTreeMap::new();
        let mut new: Side = BTreeMap::new();
        let mut t = Tally::default();
        if lock == Lock::Unread {
            return t;
        }
        for h in hunks {
            let side = |skip: Kind| -> Vec<(bool, &str)> {
                h.lines
                    .iter()
                    .filter(|(k, _)| *k != skip)
                    .map(|(k, s)| (*k != Kind::Context, s.as_str()))
                    .collect()
            };
            let mut unnamed = [0u32; 2];
            for (i, (entries, into)) in [
                (entries(lock, &side(Kind::Added)), &mut old),
                (entries(lock, &side(Kind::Removed)), &mut new),
            ]
            .into_iter()
            .enumerate()
            {
                for e in entries {
                    match e.name {
                        Some(n) => into
                            .entry(n)
                            .or_default()
                            .entry(e.version)
                            .or_default()
                            .extend(e.others),
                        None if e.touched => unnamed[i] += 1,
                        None => {}
                    }
                }
            }
            t.unnamed += unnamed[0].max(unnamed[1]);
        }
        let shown = |v: &Option<String>| v.clone().unwrap_or_default();
        let names: std::collections::BTreeSet<&String> = old.keys().chain(new.keys()).collect();
        for name in names {
            let empty = BTreeMap::new();
            let o = old.get(name).unwrap_or(&empty);
            let n = new.get(name).unwrap_or(&empty);
            let mut gone: Vec<&Option<String>> = Vec::new();
            for (v, others) in o {
                match n.get(v) {
                    Some(now) => {
                        let (mut a, mut b) = (others.clone(), now.clone());
                        a.sort();
                        b.sort();
                        if a != b {
                            t.resealed.push((name.clone(), shown(v)));
                        }
                    }
                    None => gone.push(v),
                }
            }
            let came: Vec<&Option<String>> = n.keys().filter(|v| !o.contains_key(*v)).collect();
            let pairs = gone.len().min(came.len());
            for i in 0..pairs {
                t.changed
                    .push((name.clone(), shown(gone[i]), shown(came[i])));
            }
            for v in &gone[pairs..] {
                t.removed.push((name.clone(), shown(v)));
            }
            for v in &came[pairs..] {
                t.added.push((name.clone(), shown(v)));
            }
        }
        t
    }

    /// Whether nothing was counted.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.resealed.is_empty()
            && self.unnamed == 0
    }

    /// *2 packages added (`a` 1.0, `b` 2.0) · 1 removed (`c` 0.3) · …*; a class
    /// that is zero is omitted; each names at most [`NAMES_SHOWN`] and counts
    /// the rest. Nothing counted is a sentence, never a zero.
    pub fn says(&self) -> String {
        if self.is_empty() {
            return "no package added, removed or changed — the file's lines changed".into();
        }
        let name = |n: &str| format!("`{}`", crate::core::text::clip(n, NAME_CHARS));
        let ver = |v: &str| crate::core::text::clip(v, NAME_CHARS);
        let pinned = |(n, v): &(String, String)| match v.is_empty() {
            true => name(n),
            false => format!("{} {}", name(n), ver(v)),
        };
        let list = |items: Vec<String>| {
            let more = items.len().saturating_sub(NAMES_SHOWN);
            let mut shown: Vec<String> = items.into_iter().take(NAMES_SHOWN).collect();
            if more > 0 {
                shown.push(format!("and {more} more"));
            }
            format!(" ({})", shown.join(", "))
        };
        let mut parts: Vec<String> = Vec::new();
        let mut class = |n: usize, what: &str, names: String| {
            if n == 0 {
                return;
            }
            let noun = match (parts.is_empty(), n) {
                (true, 1) => "package ",
                (true, _) => "packages ",
                (false, _) => "",
            };
            parts.push(format!("{n} {noun}{what}{names}"));
        };
        class(
            self.added.len(),
            "added",
            list(self.added.iter().map(pinned).collect()),
        );
        class(
            self.removed.len(),
            "removed",
            list(self.removed.iter().map(pinned).collect()),
        );
        class(
            self.changed.len(),
            "changed version",
            list(
                self.changed
                    .iter()
                    .map(|(n, a, b)| format!("{} {} → {}", name(n), ver(a), ver(b)))
                    .collect(),
            ),
        );
        class(
            self.resealed.len(),
            "changed checksum or source with the same version",
            list(self.resealed.iter().map(pinned).collect()),
        );
        class(
            self.unnamed as usize,
            "changed, name not in the diff",
            String::new(),
        );
        parts.join(" · ")
    }
}

/// The reason of a lockfile's row: what it edits, then the tally, or why the
/// package counts were not read. Never *0 packages changed*.
pub fn lockfile_says(lead: &str, lock: Lock, hunks: &[Hunk]) -> String {
    match (lock, hunks.is_empty()) {
        (_, true) => format!("{lead}: package counts not read: the diff shows no lines of it"),
        (Lock::Unread, false) => format!("{lead}: package counts not read for this format"),
        (lock, false) => format!("{lead}: {}", Tally::of(lock, hunks).says()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unified diff's hunks, from the text git prints.
    fn hunks(diff: &str) -> Vec<Hunk> {
        let mut out: Vec<Hunk> = Vec::new();
        for line in diff.lines() {
            if line.starts_with("@@") {
                out.push(Hunk {
                    header: line.to_string(),
                    lines: Vec::new(),
                });
                continue;
            }
            let Some(h) = out.last_mut() else { continue };
            let (k, t) = match line.chars().next() {
                Some('+') => (Kind::Added, &line[1..]),
                Some('-') => (Kind::Removed, &line[1..]),
                Some(' ') => (Kind::Context, &line[1..]),
                None => (Kind::Context, ""),
                _ => continue,
            };
            h.lines.push((k, t.to_string()));
        }
        out
    }

    fn kind(path: &str) -> Option<WeakKind> {
        control_file(path).map(|c| c.kind)
    }

    #[test]
    fn the_table_holds_every_documented_path_and_kind() {
        use WeakKind::*;
        for p in [
            ".claude/settings.json",
            ".claude/settings.local.json",
            ".codex/hooks.json",
            ".codex/config.toml",
            ".gemini/settings.json",
            ".mcp.json",
            ".github/hooks/a.json",
            ".claude/skills/review/SKILL.md",
            ".agents/skills/x/SKILL.md",
            "CLAUDE.md",
            "AGENTS.md",
            "a/b/c/CLAUDE.md",
            "a/b/c/AGENTS.md",
        ] {
            assert_eq!(kind(p), Some(AgentControls), "{p}");
        }
        for p in [
            "CODEOWNERS",
            ".github/CODEOWNERS",
            "docs/CODEOWNERS",
            ".github/settings.yml",
            "infra/github.tf",
        ] {
            assert_eq!(kind(p), Some(Ownership), "{p}");
        }
        for (name, _) in LOCKFILES {
            assert_eq!(kind(name), Some(SupplyChain), "{name}");
            let deep = format!("a/b/{name}");
            assert_eq!(kind(&deep), Some(SupplyChain), "{deep}");
            assert!(control_file(name).unwrap().lock.is_some());
        }
        for p in [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "composer.json",
            "go.mod",
            "Gemfile",
            "Pipfile",
            "requirements.txt",
            "requirements-dev.txt",
            "requirements.in",
            "svc/requirements-test.in",
        ] {
            assert_eq!(kind(p), Some(SupplyChain), "{p}");
            assert!(control_file(p).unwrap().lock.is_none(), "{p}");
        }
    }

    #[test]
    fn a_name_that_resembles_a_control_is_not_one() {
        for p in [
            "docs/claude-settings.json",
            "notes/AGENTS.md.bak",
            "packages/x/.claude/skills/a/SKILL.md",
            "src/CODEOWNERS",
            "Cargo.lock.bak",
            "devplane.toml",
            "x/.claude/settings.json",
            ".copilot/hooks/a.json",
            "src/settings.yml",
            "requirements.md",
        ] {
            assert_eq!(kind(p), None, "{p}");
        }
    }

    #[test]
    fn every_reason_names_the_file_and_no_judgement() {
        for p in [
            ".claude/settings.json",
            ".mcp.json",
            "AGENTS.md",
            ".claude/skills/a.md",
            "CODEOWNERS",
            "main.tf",
            "Cargo.lock",
            "Cargo.toml",
        ] {
            let c = control_file(p).unwrap();
            assert!(c.edits.contains(&format!("`{p}`")), "{}", c.edits);
            for s in [&c.edits, &c.deletes(p), &c.renames_away(p)] {
                for w in ["risk", "safe", "dangerous", "suspicious", "score", "%"] {
                    assert!(!s.to_lowercase().contains(w), "{s}");
                }
            }
        }
        assert_eq!(
            control_file(".claude/settings.json").unwrap().edits,
            "edits `.claude/settings.json`, which sets the agent's hooks and permissions"
        );
        assert_eq!(
            control_file(".claude/settings.json")
                .unwrap()
                .renames_away(".claude/settings.json"),
            "renames `.claude/settings.json`, which sets the agent's hooks and permissions, \
             to a name nothing reads it by"
        );
    }

    fn reads(path: &str, diff: &str) -> bool {
        let c = control_file(path).unwrap();
        hunks(diff).iter().any(|h| c.reads(h))
    }

    #[test]
    fn a_manifest_hunk_counts_when_it_declares_a_dependency() {
        // A dependency added under a `[dependencies]` header out of view.
        assert!(reads(
            "Cargo.toml",
            "@@ -8,3 +8,4 @@\n anyhow = \"1\"\n+serde = \"1\"\n tokio = { version = \"1\" }\n"
        ));
        assert!(reads(
            "Cargo.toml",
            "@@ -8,2 +8,3 @@\n anyhow = \"1\"\n+serde = { version = \"1\", features = [\"derive\"] }\n"
        ));
        assert!(reads(
            "Cargo.toml",
            "@@ -1,3 +1,3 @@\n [patch.crates-io]\n-ring = { git = \"https://x\" }\n+ring = { git = \"https://y\" }\n"
        ));
        // Package metadata, with the dependencies table below it in view.
        assert!(!reads(
            "Cargo.toml",
            "@@ -1,6 +1,6 @@\n [package]\n name = \"x\"\n-description = \"old\"\n+description = \"new\"\n \n [dependencies]\n"
        ));
        assert!(!reads(
            "Cargo.toml",
            "@@ -1,3 +1,3 @@\n name = \"x\"\n-version = \"0.1.0\"\n+version = \"0.2.0\"\n"
        ));
        assert!(!reads(
            "Cargo.toml",
            "@@ -1,3 +1,3 @@\n [lints.rust]\n-unsafe_code = \"forbid\"\n+unsafe_code = \"allow\"\n"
        ));
        assert!(reads(
            "package.json",
            "@@ -1,4 +1,5 @@\n   \"dependencies\": {\n+    \"left-pad\": \"^1.3.0\",\n     \"lodash\": \"4.17.21\"\n   }\n"
        ));
        assert!(reads(
            "package.json",
            "@@ -5,2 +5,3 @@\n     \"lodash\": \"4.17.21\",\n+    \"left-pad\": \"^1.3.0\"\n"
        ));
        assert!(!reads(
            "package.json",
            "@@ -1,4 +1,4 @@\n {\n   \"name\": \"x\",\n-  \"version\": \"1.2.0\",\n+  \"version\": \"1.3.0\",\n"
        ));
        assert!(!reads(
            "package.json",
            "@@ -1,3 +1,3 @@\n   \"engines\": {\n-    \"node\": \">=18\"\n+    \"node\": \">=20\"\n"
        ));
        assert!(!reads(
            "package.json",
            "@@ -1,3 +1,3 @@\n-  \"description\": \"a\",\n+  \"description\": \"b\",\n   \"dependencies\": {\n"
        ));
        assert!(reads(
            "pyproject.toml",
            "@@ -3,3 +3,4 @@\n dependencies = [\n     \"requests>=2\",\n+    \"httpx>=0.27\",\n ]\n"
        ));
        assert!(!reads(
            "pyproject.toml",
            "@@ -1,3 +1,3 @@\n [tool.ruff]\n-line-length = 88\n+line-length = 100\n"
        ));
        assert!(reads(
            "composer.json",
            "@@ -1,3 +1,4 @@\n   \"require\": {\n+    \"monolog/monolog\": \"^3.0\",\n"
        ));
        assert!(reads("go.mod", "@@ -1 +1 @@\n-go 1.21\n+go 1.22\n"));
        assert!(reads(
            "main.tf",
            "@@ -1,3 +1,3 @@\n resource \"github_repository_ruleset\" \"main\" {\n-  enforcement = \"active\"\n+  enforcement = \"disabled\"\n"
        ));
        assert!(!reads(
            "main.tf",
            "@@ -1,3 +1,3 @@\n resource \"aws_s3_bucket\" \"b\" {\n-  acl = \"private\"\n+  acl = \"public\"\n"
        ));
    }

    fn tally(lock: Lock, diff: &str) -> Tally {
        Tally::of(lock, &hunks(diff))
    }

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    fn changes(v: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
        v.iter()
            .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
            .collect()
    }

    #[test]
    fn each_lockfile_format_tallies_its_own_tools_diff() {
        use fixtures::*;
        let t = tally(Lock::Toml, CARGO);
        assert_eq!(t.added, pairs(&[("ryu", "1.0.23")]));
        assert_eq!(t.removed, pairs(&[("memchr", "2.8.3")]));
        assert_eq!(t.changed, changes(&[("itoa", "1.0.10", "1.0.11")]));
        assert!(t.resealed.is_empty() && t.unnamed == 0, "{t:?}");

        for (lock, diff) in [(Lock::Toml, UV), (Lock::Toml, POETRY)] {
            let t = tally(lock, diff);
            assert_eq!(t.added, pairs(&[("certifi", "2024.2.2")]), "{diff}");
            assert_eq!(t.removed, pairs(&[("idna", "3.6")]));
            assert_eq!(t.changed, changes(&[("six", "1.16.0", "1.17.0")]));
            assert!(t.resealed.is_empty() && t.unnamed == 0, "{t:?}");
        }

        for (lock, diff) in [
            (Lock::Npm, NPM),
            (Lock::Pnpm, PNPM),
            (Lock::Yarn, YARN),
            (Lock::Yarn, BERRY),
        ] {
            let t = tally(lock, diff);
            assert_eq!(t.added, pairs(&[("ms", "2.1.3")]), "{lock:?}");
            assert_eq!(t.removed, pairs(&[("isarray", "2.0.5")]), "{lock:?}");
            assert_eq!(
                t.changed,
                changes(&[("is-number", "7.0.0", "6.0.0")]),
                "{lock:?}"
            );
            assert!(t.resealed.is_empty() && t.unnamed == 0, "{lock:?} {t:?}");
        }

        let t = tally(Lock::GoSum, GO);
        assert_eq!(t.added, pairs(&[("github.com/pkg/errors", "v0.9.1")]));
        assert_eq!(t.removed, pairs(&[("golang.org/x/text", "v0.14.0")]));
        assert_eq!(
            t.changed,
            changes(&[("github.com/google/uuid", "v1.5.0", "v1.6.0")])
        );

        let t = tally(Lock::Gemfile, GEM);
        assert_eq!(t.added, pairs(&[("rake", "13.1.0")]));
        assert_eq!(t.removed, pairs(&[("json", "2.7.1")]));
        assert_eq!(t.changed, changes(&[("rack", "2.2.8", "2.2.9")]));
    }

    /// The five classes of the settled example: 2 added, 1 removed, 1 version
    /// changed, 1 checksum changed alone.
    #[test]
    fn a_cargo_lock_says_each_class_with_its_names() {
        let diff = "@@ -1,30 +1,34 @@
 [[package]]
-name = \"h2\"
-version = \"0.3.26\"
-source = \"registry+https://github.com/rust-lang/crates.io-index\"
-checksum = \"aaaa\"
+name = \"hyper-util\"
+version = \"0.1.10\"
+source = \"registry+https://github.com/rust-lang/crates.io-index\"
+checksum = \"bbbb\"

 [[package]]
 name = \"ring\"
 version = \"0.17.8\"
 source = \"registry+https://github.com/rust-lang/crates.io-index\"
-checksum = \"cccc\"
+checksum = \"dddd\"

 [[package]]
 name = \"serde\"
-version = \"1.0.210\"
+version = \"1.0.215\"
 source = \"registry+https://github.com/rust-lang/crates.io-index\"
-checksum = \"eeee\"
+checksum = \"ffff\"
+
+[[package]]
+name = \"tower\"
+version = \"0.5.2\"
+source = \"registry+https://github.com/rust-lang/crates.io-index\"
+checksum = \"gggg\"
";
        let t = tally(Lock::Toml, diff);
        assert_eq!(
            t.says(),
            "2 packages added (`hyper-util` 0.1.10, `tower` 0.5.2) · 1 removed (`h2` 0.3.26) · \
             1 changed version (`serde` 1.0.210 → 1.0.215) · 1 changed checksum or source with \
             the same version (`ring` 0.17.8)"
        );
    }

    #[test]
    fn an_edit_whose_entry_name_is_out_of_view_is_counted_as_such() {
        let diff = "@@ -40,4 +40,4 @@ version = 4
 version = \"1.0.0\"
 source = \"registry+https://github.com/rust-lang/crates.io-index\"
-checksum = \"aaaa\"
+checksum = \"bbbb\"
";
        let t = tally(Lock::Toml, diff);
        assert_eq!(t.unnamed, 1, "{t:?}");
        assert_eq!(t.says(), "1 package changed, name not in the diff");
        // A dependency list edited inside a named entry is not a package change.
        let diff = "@@ -1,6 +1,6 @@
 name = \"x\"
 version = \"0.1.0\"
 dependencies = [
- \"memchr\",
+ \"ryu\",
 ]
";
        assert!(tally(Lock::Toml, diff).is_empty());
    }

    #[test]
    fn a_class_names_eight_and_counts_the_rest() {
        let mut diff = String::from("@@ -1 +1,40 @@\n");
        for i in 0..11 {
            diff.push_str(&format!(
                "+[[package]]\n+name = \"p{i:02}\"\n+version = \"1.0.{i}\"\n"
            ));
        }
        let says = tally(Lock::Toml, &diff).says();
        assert!(
            says.starts_with("11 packages added (`p00` 1.0.0, "),
            "{says}"
        );
        assert!(says.contains("`p07` 1.0.7, and 3 more)"), "{says}");
        assert!(!says.contains("p08"), "{says}");
    }

    #[test]
    fn lines_that_moved_without_a_package_change_are_a_sentence_not_a_zero() {
        let diff = "@@ -1,3 +1,4 @@
 [[package]]
 name = \"a\"
+
 version = \"1.0.0\"
";
        let t = tally(Lock::Toml, diff);
        assert!(t.is_empty());
        assert_eq!(
            t.says(),
            "no package added, removed or changed — the file's lines changed"
        );
        let h = hunks(diff);
        assert_eq!(
            lockfile_says("edits the lockfile `composer.lock`", Lock::Unread, &h),
            "edits the lockfile `composer.lock`: package counts not read for this format"
        );
        assert_eq!(
            lockfile_says("edits the lockfile `Cargo.lock`", Lock::Toml, &[]),
            "edits the lockfile `Cargo.lock`: package counts not read: the diff shows no lines \
             of it"
        );
    }

    #[test]
    fn a_large_generated_lockfile_tallies_quickly() {
        let mut diff = String::from("@@ -1,10000 +1,10000 @@\n");
        for i in 0..2_500 {
            diff.push_str(&format!(
                " [[package]]\n name = \"crate{i}\"\n-version = \"1.0.0\"\n+version = \"1.0.1\"\n"
            ));
        }
        assert!(diff.lines().count() >= 10_000);
        let h = hunks(&diff);
        let lines: usize = h.iter().map(|h| h.lines.len()).sum();
        let mut h = h;
        // Twice the lines: 20 000.
        h.push(h[0].clone());
        assert!(lines * 2 >= 20_000);
        let start = std::time::Instant::now();
        let t = Tally::of(Lock::Toml, &h);
        let took = start.elapsed();
        assert_eq!(t.changed.len(), 2_500);
        // Generous in debug builds; the bound is for the release build.
        let bound = match cfg!(debug_assertions) {
            true => std::time::Duration::from_millis(500),
            false => std::time::Duration::from_millis(50),
        };
        assert!(took < bound, "{took:?}");
    }

    /// Diffs produced by each format's own tool: lock, commit, change the
    /// manifest, lock again, `git diff`.
    mod fixtures {
        pub const CARGO: &str = r#"@@ -4,20 +4,20 @@ version = 4

 [[package]]
 name = "itoa"
-version = "1.0.10"
+version = "1.0.11"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "b1a46d1a171d865aa5f83f92695765caa047a9b4cbae2cbf37dbd613a793fd4c"
+checksum = "49f1f14873335454500d59611f1cf4a4b0f786f9ac11f4312a78e4cf2566695b"

 [[package]]
-name = "memchr"
-version = "2.8.3"
+name = "ryu"
+version = "1.0.23"
 source = "registry+https://github.com/rust-lang/crates.io-index"
-checksum = "cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98"
+checksum = "9774ba4a74de5f7b1c1451ed6cd5285a32eddb5cccb8cc655a4e50009e06477f"

 [[package]]
 name = "x"
 version = "0.1.0"
 dependencies = [
  "itoa",
- "memchr",
+ "ryu",
 ]
"#;

        pub const NPM: &str = r#"@@ -8,23 +8,23 @@
       "name": "x",
       "version": "1.0.0",
       "dependencies": {
-        "is-number": "7.0.0",
-        "isarray": "2.0.5"
+        "is-number": "6.0.0",
+        "ms": "2.1.3"
       }
     },
     "node_modules/is-number": {
-      "version": "7.0.0",
-      "resolved": "https://registry.npmjs.org/is-number/-/is-number-7.0.0.tgz",
-      "integrity": "sha512-41Cifkg6e8TylSpdtTpeLVMqvSBEVzTttHvERD741+pnZ8ANv0004MRL43QKPDlK9cGvNp6NZWZUBlbGXYxxng==",
+      "version": "6.0.0",
+      "resolved": "https://registry.npmjs.org/is-number/-/is-number-6.0.0.tgz",
+      "integrity": "sha512-Wu1VHeILBK8KAWJUAiSZQX94GmOE45Rg6/538fKwiloUu21KncEkYGPqob2oSZ5mUT73vLGrHQjKw3KMPwfDzg==",
       "license": "MIT",
       "engines": {
-        "node": ">=0.12.0"
+        "node": ">=0.10.0"
       }
     },
-    "node_modules/isarray": {
-      "version": "2.0.5",
-      "resolved": "https://registry.npmjs.org/isarray/-/isarray-2.0.5.tgz",
-      "integrity": "sha512-xHjhDr3cNBK0BzdUJSPXZntQUx/mwMS5Rw4A7lPJ90XGAO6ISP/ePDNuo0vhqOZU+UD5JoodwCAAoZQd3FeAKw==",
+    "node_modules/ms": {
+      "version": "2.1.3",
+      "resolved": "https://registry.npmjs.org/ms/-/ms-2.1.3.tgz",
+      "integrity": "sha512-6FlzubTLZG3J2a/NVCAleEhjzq5oxgHyaCU9yYXvcLsvoVaHJq/s5xXI6/XXP6tz7R9xAOtHnSO/tXtF3WRTlA==",
       "license": "MIT"
     }
   }
"#;

        pub const PNPM: &str = r#"@@ -9,23 +9,23 @@ importers:
   .:
     dependencies:
       is-number:
-        specifier: 7.0.0
-        version: 7.0.0
-      isarray:
-        specifier: 2.0.5
-        version: 2.0.5
+        specifier: 6.0.0
+        version: 6.0.0
+      ms:
+        specifier: 2.1.3
+        version: 2.1.3

 packages:

-  is-number@7.0.0:
-    resolution: {integrity: sha512-41Cifkg6e8TylSpdtTpeLVMqvSBEVzTttHvERD741+pnZ8ANv0004MRL43QKPDlK9cGvNp6NZWZUBlbGXYxxng==}
-    engines: {node: '>=0.12.0'}
+  is-number@6.0.0:
+    resolution: {integrity: sha512-Wu1VHeILBK8KAWJUAiSZQX94GmOE45Rg6/538fKwiloUu21KncEkYGPqob2oSZ5mUT73vLGrHQjKw3KMPwfDzg==}
+    engines: {node: '>=0.10.0'}

-  isarray@2.0.5:
-    resolution: {integrity: sha512-xHjhDr3cNBK0BzdUJSPXZntQUx/mwMS5Rw4A7lPJ90XGAO6ISP/ePDNuo0vhqOZU+UD5JoodwCAAoZQd3FeAKw==}
+  ms@2.1.3:
+    resolution: {integrity: sha512-6FlzubTLZG3J2a/NVCAleEhjzq5oxgHyaCU9yYXvcLsvoVaHJq/s5xXI6/XXP6tz7R9xAOtHnSO/tXtF3WRTlA==}

 snapshots:

-  is-number@7.0.0: {}
+  is-number@6.0.0: {}

-  isarray@2.0.5: {}
+  ms@2.1.3: {}
"#;

        pub const YARN: &str = r#"@@ -2,12 +2,12 @@
 # yarn lockfile v1


-is-number@7.0.0:
-  version "7.0.0"
-  resolved "https://registry.yarnpkg.com/is-number/-/is-number-7.0.0.tgz#7535345b896734d5f80c4d06c50955527a14f12b"
-  integrity sha512-41Cifkg6e8TylSpdtTpeLVMqvSBEVzTttHvERD741+pnZ8ANv0004MRL43QKPDlK9cGvNp6NZWZUBlbGXYxxng==
+is-number@6.0.0:
+  version "6.0.0"
+  resolved "https://registry.yarnpkg.com/is-number/-/is-number-6.0.0.tgz#e6d15ad31fc262887cccf217ae5f9316f81b1995"
+  integrity sha512-Wu1VHeILBK8KAWJUAiSZQX94GmOE45Rg6/538fKwiloUu21KncEkYGPqob2oSZ5mUT73vLGrHQjKw3KMPwfDzg==

-isarray@2.0.5:
-  version "2.0.5"
-  resolved "https://registry.yarnpkg.com/isarray/-/isarray-2.0.5.tgz#8af1e4c1221244cc62459faf38940d4e644a5723"
-  integrity sha512-xHjhDr3cNBK0BzdUJSPXZntQUx/mwMS5Rw4A7lPJ90XGAO6ISP/ePDNuo0vhqOZU+UD5JoodwCAAoZQd3FeAKw==
+ms@2.1.3:
+  version "2.1.3"
+  resolved "https://registry.yarnpkg.com/ms/-/ms-2.1.3.tgz#574c8138ce1d2b5861f0b44579dbadd60c6615b2"
+  integrity sha512-6FlzubTLZG3J2a/NVCAleEhjzq5oxgHyaCU9yYXvcLsvoVaHJq/s5xXI6/XXP6tz7R9xAOtHnSO/tXtF3WRTlA==
"#;

        pub const BERRY: &str = r#"@@ -5,17 +5,17 @@ __metadata:
   version: 8
   cacheKey: 10c0

-"is-number@npm:7.0.0":
-  version: 7.0.0
-  resolution: "is-number@npm:7.0.0"
-  checksum: 10c0/b4686d0d3053146095ccd45346461bc8e53b80aeb7671cc52a4de02dbbf7dc0d1d2a986e2fe4ae206984b4d34ef37e8b795ebc4f4295c978373e6575e295d811
+"is-number@npm:6.0.0":
+  version: 6.0.0
+  resolution: "is-number@npm:6.0.0"
+  checksum: 10c0/5da4c68401529675c575878d2760d66f18eaef4b014858577f6003daf66488d7fe4eae684b1e8574e3fa1bb447c6c6c56b8491d2b4b3239da2d32e5f6f218008
   languageName: node
   linkType: hard

-"isarray@npm:2.0.5":
-  version: 2.0.5
-  resolution: "isarray@npm:2.0.5"
-  checksum: 10c0/4199f14a7a13da2177c66c31080008b7124331956f47bca57dd0b6ea9f11687aa25e565a2c7a2b519bc86988d10398e3049a1f5df13c9f6b7664154690ae79fd
+"ms@npm:2.1.3":
+  version: 2.1.3
+  resolution: "ms@npm:2.1.3"
+  checksum: 10c0/d924b57e7312b3b63ad21fc5b3dc0af5e78d61a1fc7cfb5457edaf26326bf62be5307cc87ffb6862ef1c2b33b0233cdb5d4f01c4c958cc0d660948b65a287a48
   languageName: node
   linkType: hard

@@ -23,7 +23,7 @@ __metadata:
   version: 0.0.0-use.local
   resolution: "x@workspace:."
   dependencies:
-    is-number: "npm:7.0.0"
-    isarray: "npm:2.0.5"
+    is-number: "npm:6.0.0"
+    ms: "npm:2.1.3"
   languageName: unknown
   linkType: soft
"#;

        pub const GO: &str = r#"@@ -1,4 +1,4 @@
-github.com/google/uuid v1.5.0 h1:1p67kYwdtXjb0gL0BPiP1Av9wiZPo5A8z2cWkTZ+eyU=
-github.com/google/uuid v1.5.0/go.mod h1:TIyPZe4MgqvfeYDBFedMoGGpEw/LqOeaOT+nhxU+yHo=
-golang.org/x/text v0.14.0 h1:ScX5w1eTa3QqT8oi6+ziP7dTV1S2+ALU0bI+0zXKWiQ=
-golang.org/x/text v0.14.0/go.mod h1:18ZOQIKpY8NJVqYksKHtTdi31H5itFRjB5/qKTNYzSU=
+github.com/google/uuid v1.6.0 h1:NIvaJDMOsjHA8n1jAhLSgzrAzy1Hgr+hNrb57e+94F0=
+github.com/google/uuid v1.6.0/go.mod h1:TIyPZe4MgqvfeYDBFedMoGGpEw/LqOeaOT+nhxU+yHo=
+github.com/pkg/errors v0.9.1 h1:FEBLx1zS214owpjy7qsBeixbURkuhQAwrK5UwLGTwt4=
+github.com/pkg/errors v0.9.1/go.mod h1:bwawxfHBFNV+L2hUp1rHADufV3IMtnDRdf1r5NINEl0=
"#;

        pub const UV: &str = r#"@@ -3,21 +3,21 @@ revision = 3
 requires-python = ">=3.9"

 [[package]]
-name = "idna"
-version = "3.6"
+name = "certifi"
+version = "2024.2.2"
 source = { registry = "https://pypi.org/simple" }
-sdist = { url = "https://files.pythonhosted.org/packages/bf/3f/ea4b9117521a1e9c50344b909be7886dd00a519552724809bb1f486986c2/idna-3.6.tar.gz", hash = "sha256:9ecdbbd083b06798ae1e86adcbfe8ab1479cf864e4ee30fe4e46a003d12491ca", size = 175426, upload-time = "2023-11-25T15:40:54.902Z" }
+sdist = { url = "https://files.pythonhosted.org/packages/71/da/e94e26401b62acd6d91df2b52954aceb7f561743aa5ccc32152886c76c96/certifi-2024.2.2.tar.gz", hash = "sha256:0569859f95fc761b18b45ef421b1290a0f65f147e92a1e5eb3e635f9a5e4e66f", size = 164886, upload-time = "2024-02-02T01:22:17.364Z" }
 wheels = [
-    { url = "https://files.pythonhosted.org/packages/c2/e7/a82b05cf63a603df6e68d59ae6a68bf5064484a0718ea5033660af4b54a9/idna-3.6-py3-none-any.whl", hash = "sha256:c05567e9c24a6b9faaa835c4821bad0590fbb9d5779e7caa6e1cc4978e7eb24f", size = 61567, upload-time = "2023-11-25T15:40:52.604Z" },
+    { url = "https://files.pythonhosted.org/packages/ba/06/a07f096c664aeb9f01624f858c3add0a4e913d6c96257acb4fce61e7de14/certifi-2024.2.2-py3-none-any.whl", hash = "sha256:dc383c07b76109f368f6106eee2b593b04a011ea4d55f652c6ca24a754d1cdd1", size = 163774, upload-time = "2024-02-02T01:22:14.86Z" },
 ]

 [[package]]
 name = "six"
-version = "1.16.0"
+version = "1.17.0"
 source = { registry = "https://pypi.org/simple" }
-sdist = { url = "https://files.pythonhosted.org/packages/71/39/171f1c67cd00715f190ba0b100d606d440a28c93c7714febeca8b79af85e/six-1.16.0.tar.gz", hash = "sha256:1e61c37477a1626458e36f7b1d82aa5c9b094fa4802892072e49de9c60c4c926", size = 34041, upload-time = "2021-05-05T14:18:18.379Z" }
+sdist = { url = "https://files.pythonhosted.org/packages/94/e7/b2c673351809dca68a0e064b6af791aa332cf192da575fd474ed7d6f16a2/six-1.17.0.tar.gz", hash = "sha256:ff70335d468e7eb6ec65b95b99d3a2836546063f63acc5171de367e834932a81", size = 34031, upload-time = "2024-12-04T17:35:28.174Z" }
 wheels = [
-    { url = "https://files.pythonhosted.org/packages/d9/5a/e7c31adbe875f2abbb91bd84cf2dc52d792b5a01506781dbcf25c91daf11/six-1.16.0-py2.py3-none-any.whl", hash = "sha256:8abb2f1d86890a2dfb989f9a77cfcfd3e47c2a354b01111771326f8aa26e0254", size = 11053, upload-time = "2021-05-05T14:18:17.237Z" },
+    { url = "https://files.pythonhosted.org/packages/b7/ce/149a00dd41f10bc29e5921b496af8b574d8413afcd5e30dfa0ed46c2cc5e/six-1.17.0-py2.py3-none-any.whl", hash = "sha256:4721f391ed90541fddacab5acf947aa0d3dc7d27b2e1e8eda2be8970586c3274", size = 11050, upload-time = "2024-12-04T17:35:26.475Z" },
 ]

 [[package]]
@@ -25,12 +25,12 @@ name = "x"
 version = "0.1.0"
 source = { virtual = "." }
 dependencies = [
-    { name = "idna" },
+    { name = "certifi" },
     { name = "six" },
 ]

 [package.metadata]
 requires-dist = [
-    { name = "idna", specifier = "==3.6" },
-    { name = "six", specifier = "==1.16.0" },
+    { name = "certifi", specifier = "==2024.2.2" },
+    { name = "six", specifier = "==1.17.0" },
 ]
"#;

        pub const POETRY: &str = r#"@@ -1,30 +1,30 @@
 # This file is automatically @generated by Poetry 2.5.1 and should not be changed by hand.

 [[package]]
-name = "idna"
-version = "3.6"
-description = "Internationalized Domain Names in Applications (IDNA)"
+name = "certifi"
+version = "2024.2.2"
+description = "Python package for providing Mozilla's CA Bundle."
 optional = false
-python-versions = ">=3.5"
+python-versions = ">=3.6"
 groups = ["main"]
 files = [
-    {file = "idna-3.6-py3-none-any.whl", hash = "sha256:c05567e9c24a6b9faaa835c4821bad0590fbb9d5779e7caa6e1cc4978e7eb24f"},
-    {file = "idna-3.6.tar.gz", hash = "sha256:9ecdbbd083b06798ae1e86adcbfe8ab1479cf864e4ee30fe4e46a003d12491ca"},
+    {file = "certifi-2024.2.2-py3-none-any.whl", hash = "sha256:dc383c07b76109f368f6106eee2b593b04a011ea4d55f652c6ca24a754d1cdd1"},
+    {file = "certifi-2024.2.2.tar.gz", hash = "sha256:0569859f95fc761b18b45ef421b1290a0f65f147e92a1e5eb3e635f9a5e4e66f"},
 ]

 [[package]]
 name = "six"
-version = "1.16.0"
+version = "1.17.0"
 description = "Python 2 and 3 compatibility utilities"
 optional = false
-python-versions = ">=2.7, !=3.0.*, !=3.1.*, !=3.2.*"
+python-versions = "!=3.0.*,!=3.1.*,!=3.2.*,>=2.7"
 groups = ["main"]
 files = [
-    {file = "six-1.16.0-py2.py3-none-any.whl", hash = "sha256:8abb2f1d86890a2dfb989f9a77cfcfd3e47c2a354b01111771326f8aa26e0254"},
-    {file = "six-1.16.0.tar.gz", hash = "sha256:1e61c37477a1626458e36f7b1d82aa5c9b094fa4802892072e49de9c60c4c926"},
+    {file = "six-1.17.0-py2.py3-none-any.whl", hash = "sha256:4721f391ed90541fddacab5acf947aa0d3dc7d27b2e1e8eda2be8970586c3274"},
+    {file = "six-1.17.0.tar.gz", hash = "sha256:ff70335d468e7eb6ec65b95b99d3a2836546063f63acc5171de367e834932a81"},
 ]

 [metadata]
 lock-version = "2.1"
 python-versions = "^3.9"
-content-hash = "b58b3d027734a1e2b536214a542d2e22298e2a2760b8a4cd2d0e00bfebe97bc4"
+content-hash = "d8206cbd8b08f53506c001bdb1fbcf41041836208624de4c8d4811331fac069b"
"#;

        pub const GEM: &str = r#"@@ -1,15 +1,15 @@
 GEM
   remote: https://rubygems.org/
   specs:
-    json (2.7.1)
-    rack (2.2.8)
+    rack (2.2.9)
+    rake (13.1.0)

 PLATFORMS
   ruby

 DEPENDENCIES
-  json (= 2.7.1)
-  rack (= 2.2.8)
+  rack (= 2.2.9)
+  rake (= 13.1.0)

 BUNDLED WITH
    1.17.2
"#;
    }
}

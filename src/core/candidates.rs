//! Commands a repository already has that could be its `check` gate.
//!
//! Detection reads at most a few named files and offers text: it never runs
//! anything, never rewrites a command, and never judges one a good check. The
//! person picks, edits or types, and only the exact text they confirm becomes
//! a gate. Every candidate carries the file (and line) it was found in, so the
//! window can show it as quoted, attributed repository text.

use serde::Serialize;

/// What the I/O half read from a repository root: only these paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoFiles {
    /// A `Cargo.toml` is present.
    pub cargo_toml: bool,
    /// `package.json`'s text, when present.
    pub package_json: Option<String>,
    /// Which of [`LOCKFILES`] are present, by name.
    pub lockfiles: Vec<String>,
    /// `justfile`'s text, when present (also `Justfile`, `.justfile`).
    pub justfile: Option<(String, String)>,
    /// Each `.github/workflows/*.yml` / `*.yaml`: its path from the root and text.
    pub workflows: Vec<(String, String)>,
    /// An `openspec/` directory is present.
    pub openspec: bool,
}

/// The lockfiles that name a package manager, in the order they are believed.
pub const LOCKFILES: &[(&str, &str)] = &[
    ("pnpm-lock.yaml", "pnpm"),
    ("yarn.lock", "yarn"),
    ("bun.lockb", "bun"),
    ("bun.lock", "bun"),
    ("package-lock.json", "npm"),
];

/// Where a candidate would go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Joins {
    /// The definition of done.
    Check,
    /// A gate run by name: evidence on the change, never part of *verified*.
    Named(String),
}

impl Serialize for Joins {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Joins::Check => s.serialize_str("check"),
            Joins::Named(n) => s.serialize_str(&format!("named:{n}")),
        }
    }
}

/// One command offered from the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// In full, exactly as found or as the one fixed rule makes it.
    pub command: String,
    /// The file it came from, relative to the root (`openspec/` for the directory).
    pub source: String,
    /// Where in that file, counting from one.
    pub line: Option<u32>,
    pub joins: Joins,
}

/// Everything offered, and what was looked for — so an empty offer can say
/// what it looked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidates {
    pub candidates: Vec<Candidate>,
    pub looked_for: Vec<&'static str>,
}

/// What detection reads, in words the window shows.
pub const LOOKED_FOR: &[&str] = &[
    "Cargo.toml",
    "package.json scripts.test",
    "justfile recipes named test, check, verify, lint or ci",
    ".github/workflows/*.yml run: steps",
    "openspec/",
];

/// The name of the gate the OpenSpec validation is offered as.
pub const OPENSPEC_GATE: &str = "openspec";
/// The command offered for it.
pub const OPENSPEC_COMMAND: &str = "openspec validate --all --strict";

/// Every candidate in `files`, in source order; none is selected.
pub fn candidates(files: &RepoFiles) -> Candidates {
    let mut out = Vec::new();
    if files.cargo_toml {
        out.push(Candidate {
            command: "cargo test".into(),
            source: "Cargo.toml".into(),
            line: None,
            joins: Joins::Check,
        });
    }
    if let Some(text) = &files.package_json
        && let Some(line) = package_test(text)
    {
        let manager = LOCKFILES
            .iter()
            .find(|(f, _)| files.lockfiles.iter().any(|l| l == f))
            .map(|(_, m)| *m)
            .unwrap_or("npm");
        out.push(Candidate {
            command: format!("{manager} test"),
            source: "package.json".into(),
            line,
            joins: Joins::Check,
        });
    }
    if let Some((name, text)) = &files.justfile {
        for (recipe, line) in just_recipes(text) {
            out.push(Candidate {
                command: format!("just {recipe}"),
                source: name.clone(),
                line: Some(line),
                joins: Joins::Check,
            });
        }
    }
    for (path, text) in &files.workflows {
        for (command, line) in workflow_runs(text) {
            out.push(Candidate {
                command,
                source: path.clone(),
                line: Some(line),
                joins: Joins::Check,
            });
        }
    }
    if files.openspec {
        out.push(Candidate {
            command: OPENSPEC_COMMAND.into(),
            source: "openspec/".into(),
            line: None,
            joins: Joins::Named(OPENSPEC_GATE.into()),
        });
    }
    Candidates {
        candidates: out,
        looked_for: LOOKED_FOR.to_vec(),
    }
}

/// `Some(line)` when `scripts.test` is declared (the line where it is, if
/// found); `None` when there is none, or npm's own placeholder that fails.
fn package_test(text: &str) -> Option<Option<u32>> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let test = v.get("scripts")?.get("test")?.as_str()?;
    // `npm init`'s placeholder exits 1 by design: not a test suite.
    if test.contains("no test specified") {
        return None;
    }
    let line = text
        .lines()
        .position(|l| l.trim_start().starts_with("\"test\""))
        .map(|i| i as u32 + 1);
    Some(line)
}

/// Recipe headers whose name suggests a check, with their line.
fn just_recipes(text: &str) -> Vec<(String, u32)> {
    const PREFIXES: &[&str] = &["test", "check", "verify", "lint", "ci"];
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        // A recipe header starts in column zero; attributes and comments do not count.
        if line.starts_with([' ', '\t', '#', '[', '@']) {
            continue;
        }
        let Some((head, rest)) = line.split_once(':') else {
            continue;
        };
        // `name := value` is an assignment, not a recipe.
        if rest.starts_with('=') {
            continue;
        }
        let name = head.split_whitespace().next().unwrap_or("");
        let ok = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if ok && PREFIXES.iter().any(|p| name.starts_with(p)) {
            out.push((name.to_string(), i as u32 + 1));
        }
    }
    out
}

/// Each `run:` value in a workflow, verbatim, with the line it starts on: a
/// single-line value, or an indented `|`/`>` block joined by newlines.
fn workflow_runs(text: &str) -> Vec<(String, u32)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        let body = trimmed
            .strip_prefix("- run:")
            .or_else(|| trimmed.strip_prefix("run:"));
        let Some(value) = body else {
            i += 1;
            continue;
        };
        let value = value.trim();
        let start = i as u32 + 1;
        if value.starts_with('|') || value.starts_with('>') {
            let indent_of = |l: &str| l.len() - l.trim_start().len();
            let key_indent = indent_of(line);
            let mut block: Vec<&str> = Vec::new();
            let mut block_indent: Option<usize> = None;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    block.push("");
                    j += 1;
                    continue;
                }
                let ind = indent_of(l);
                if ind <= key_indent {
                    break;
                }
                let bi = *block_indent.get_or_insert(ind);
                block.push(if l.len() >= bi {
                    &l[bi..]
                } else {
                    l.trim_start()
                });
                j += 1;
            }
            while block.last() == Some(&"") {
                block.pop();
            }
            if !block.is_empty() {
                out.push((block.join("\n"), start));
            }
            i = j;
            continue;
        }
        let value = unquote(value);
        if !value.is_empty() {
            out.push((value.to_string(), start));
        }
        i += 1;
    }
    out
}

/// A YAML scalar's quotes off, nothing else changed.
fn unquote(v: &str) -> &str {
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return &v[1..v.len() - 1];
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands(c: &Candidates) -> Vec<(&str, &str, Option<u32>)> {
        c.candidates
            .iter()
            .map(|c| (c.command.as_str(), c.source.as_str(), c.line))
            .collect()
    }

    #[test]
    fn a_cargo_manifest_offers_cargo_test_with_where_it_was_found() {
        let c = candidates(&RepoFiles {
            cargo_toml: true,
            ..Default::default()
        });
        assert_eq!(commands(&c), vec![("cargo test", "Cargo.toml", None)]);
        assert_eq!(c.candidates[0].joins, Joins::Check);
    }

    #[test]
    fn the_test_script_is_offered_with_the_projects_own_package_manager() {
        let pkg = "{\n  \"name\": \"x\",\n  \"scripts\": {\n    \"test\": \"vitest run\"\n  }\n}\n";
        let pnpm = candidates(&RepoFiles {
            package_json: Some(pkg.into()),
            lockfiles: vec!["pnpm-lock.yaml".into()],
            ..Default::default()
        });
        assert_eq!(
            commands(&pnpm),
            vec![("pnpm test", "package.json", Some(4))]
        );
        let none = candidates(&RepoFiles {
            package_json: Some(pkg.into()),
            ..Default::default()
        });
        assert_eq!(none.candidates[0].command, "npm test");
        // No test script, or npm's failing placeholder: nothing offered.
        let placeholder = r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"}}"#;
        for text in ["{}", placeholder, "not json"] {
            let c = candidates(&RepoFiles {
                package_json: Some(text.into()),
                ..Default::default()
            });
            assert!(c.candidates.is_empty(), "{text}");
        }
    }

    #[test]
    fn only_justfile_recipes_named_like_a_check_are_offered_with_their_line() {
        let just = "set shell := [\"bash\", \"-c\"]\nversion := \"1\"\n\n# the tests\ntest:\n    cargo test\n\ndeploy: test\n    ./deploy\n\ncheck-all arg='x':\n    true\n";
        let c = candidates(&RepoFiles {
            justfile: Some(("justfile".into(), just.into())),
            ..Default::default()
        });
        assert_eq!(
            commands(&c),
            vec![
                ("just test", "justfile", Some(5)),
                ("just check-all", "justfile", Some(11))
            ]
        );
    }

    #[test]
    fn workflow_steps_are_offered_verbatim_single_line_and_block() {
        let wf = [
            "name: ci",
            "on: push",
            "jobs:",
            "  test:",
            "    runs-on: ubuntu-latest",
            "    steps:",
            "      - uses: actions/checkout@v4",
            "      - run: cargo clippy -- -D warnings",
            "      - name: tests",
            "        run: |",
            "          cargo test --locked",
            "          echo \"${{ secrets.TOKEN }}\"",
            "      - run: \"npm ci\"",
            "",
        ]
        .join("\n");
        let c = candidates(&RepoFiles {
            workflows: vec![(".github/workflows/ci.yml".into(), wf)],
            ..Default::default()
        });
        assert_eq!(
            commands(&c),
            vec![
                (
                    "cargo clippy -- -D warnings",
                    ".github/workflows/ci.yml",
                    Some(8)
                ),
                (
                    "cargo test --locked\necho \"${{ secrets.TOKEN }}\"",
                    ".github/workflows/ci.yml",
                    Some(10)
                ),
                ("npm ci", ".github/workflows/ci.yml", Some(13)),
            ]
        );
    }

    #[test]
    fn openspec_is_offered_only_as_a_named_gate() {
        let c = candidates(&RepoFiles {
            openspec: true,
            ..Default::default()
        });
        assert_eq!(c.candidates.len(), 1);
        assert_eq!(c.candidates[0].command, OPENSPEC_COMMAND);
        assert_eq!(c.candidates[0].source, "openspec/");
        assert_eq!(c.candidates[0].joins, Joins::Named("openspec".into()));
        assert_eq!(
            serde_json::to_value(&c.candidates[0].joins).unwrap(),
            serde_json::json!("named:openspec")
        );
    }

    #[test]
    fn a_monorepo_offers_every_candidate_and_nothing_offers_nothing_but_says_what_it_read() {
        let c = candidates(&RepoFiles {
            cargo_toml: true,
            package_json: Some(r#"{"scripts":{"test":"jest"}}"#.into()),
            lockfiles: vec!["yarn.lock".into()],
            openspec: true,
            ..Default::default()
        });
        let all: Vec<&str> = c.candidates.iter().map(|c| c.command.as_str()).collect();
        assert_eq!(all, vec!["cargo test", "yarn test", OPENSPEC_COMMAND]);
        let none = candidates(&RepoFiles::default());
        assert!(none.candidates.is_empty());
        assert_eq!(none.looked_for.len(), 5);
    }
}

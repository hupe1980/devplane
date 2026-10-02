//! Writing the gates a person chose into `devplane.toml` without rewriting it.
//!
//! The file is the person's: every line outside the assignment being replaced
//! stays byte-for-byte as it was, the written lines carry a comment naming
//! where they came from, and the result is parsed again and refused if it
//! would change anything but the gates being declared. A file that does not
//! parse is never touched; the caller shows where it fails.

use crate::core::config::ProjectConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The comment above every assignment written from the window.
pub const ORIGIN: &str = "# gates declared from the Devplane window; edit freely";

/// What the person confirmed: `check`'s commands in order, and gates run by
/// name (`named.openspec`), each written as its own `[gates.<name>]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Declared {
    pub check: Vec<String>,
    pub named: BTreeMap<String, Vec<String>>,
}

impl Declared {
    pub fn is_empty(&self) -> bool {
        self.check.iter().all(|c| c.trim().is_empty())
            && self
                .named
                .values()
                .all(|v| v.iter().all(|c| c.trim().is_empty()))
    }

    /// Blank commands dropped; a named gate with no commands dropped.
    pub fn cleaned(&self) -> Declared {
        let keep = |v: &Vec<String>| -> Vec<String> {
            v.iter().filter(|c| !c.trim().is_empty()).cloned().collect()
        };
        Declared {
            check: keep(&self.check),
            named: self
                .named
                .iter()
                .map(|(k, v)| (k.clone(), keep(v)))
                .filter(|(_, v)| !v.is_empty())
                .collect(),
        }
    }
}

/// Why nothing would be written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "refused", rename_all = "snake_case")]
pub enum Refused {
    /// The file on disk does not parse; `line` counts from one.
    Unparseable { line: Option<u32>, error: String },
    /// Nothing was chosen.
    NothingDeclared,
    /// A gate name that cannot be a table under `[gates]`.
    BadName { name: String },
    /// The splice would have changed something else; never written.
    WouldChangeOther { what: String },
}

impl Refused {
    pub fn says(&self) -> String {
        match self {
            Refused::Unparseable {
                line: Some(l),
                error,
            } => format!("devplane.toml does not parse at line {l}: {error}"),
            Refused::Unparseable { line: None, error } => {
                format!("devplane.toml does not parse: {error}")
            }
            Refused::NothingDeclared => "no command was chosen".into(),
            Refused::BadName { name } => format!("`{name}` cannot name a gate"),
            Refused::WouldChangeOther { what } => {
                format!("writing the gates would also have changed {what}; nothing was written")
            }
        }
    }
}

/// Whether `existing` parses, and where it does not.
pub fn check_parses(existing: &str) -> Result<ProjectConfig, Refused> {
    if let Err(e) = existing.parse::<toml::Table>() {
        let line = e.span().map(|s| {
            existing[..s.start.min(existing.len())]
                .lines()
                .count()
                .max(1) as u32
        });
        return Err(Refused::Unparseable {
            line,
            error: e.message().to_string(),
        });
    }
    ProjectConfig::parse(existing).map_err(|error| Refused::Unparseable { line: None, error })
}

/// The new text of `devplane.toml`: `existing` (or nothing) with `declared`'s
/// gates set, every other line unchanged.
pub fn splice(existing: Option<&str>, declared: &Declared) -> Result<String, Refused> {
    let declared = declared.cleaned();
    if declared.is_empty() {
        return Err(Refused::NothingDeclared);
    }
    for name in declared.named.keys() {
        let ok = !name.is_empty()
            && name != "check"
            && name != "named"
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !ok {
            return Err(Refused::BadName { name: name.clone() });
        }
    }
    let text = existing.unwrap_or("");
    if existing.is_some() {
        check_parses(text)?;
    }
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let trailing_newline = text.is_empty() || text.ends_with('\n');

    if !declared.check.is_empty() {
        let assignment = assign("check", &declared.check);
        match table_range(&lines, "gates") {
            Some((header, end)) => match key_range(&lines, header + 1, end, "check") {
                Some((from, to)) => {
                    let from = if from > 0 && lines[from - 1].trim() == ORIGIN {
                        from - 1
                    } else {
                        from
                    };
                    let mut new = vec![ORIGIN.to_string()];
                    new.extend(assignment);
                    lines.splice(from..to, new);
                }
                None => {
                    let mut new = vec![ORIGIN.to_string()];
                    new.extend(assignment);
                    lines.splice(header + 1..header + 1, new);
                }
            },
            None => {
                if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.push(ORIGIN.to_string());
                lines.push("[gates]".into());
                lines.extend(assignment);
            }
        }
    }
    for (name, run) in &declared.named {
        let assignment = assign("run", run);
        let table = format!("gates.{name}");
        match table_range(&lines, &table) {
            Some((header, end)) => match key_range(&lines, header + 1, end, "run") {
                Some((from, to)) => {
                    lines.splice(from..to, assignment);
                }
                None => {
                    lines.splice(header + 1..header + 1, assignment);
                }
            },
            None => {
                if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.push(format!(
                    "{ORIGIN} — a gate run by name: evidence on the change, never part of check"
                ));
                lines.push(format!("[{table}]"));
                lines.extend(assignment);
            }
        }
    }
    let mut out = lines.join("\n");
    if trailing_newline || existing.is_none() {
        out.push('\n');
    }
    verify(text, &out, &declared)?;
    Ok(out)
}

/// The re-parse: the result loads, holds exactly the declared gates, and
/// differs from the original in nothing else.
fn verify(before: &str, after: &str, declared: &Declared) -> Result<(), Refused> {
    let cfg = ProjectConfig::parse(after).map_err(|what| Refused::WouldChangeOther {
        what: format!("whether the file parses ({what})"),
    })?;
    if !declared.check.is_empty() && cfg.gates.check != declared.check {
        return Err(Refused::WouldChangeOther {
            what: "`check` to something other than what was chosen".into(),
        });
    }
    for (name, run) in &declared.named {
        if cfg.gates.named.get(name).map(|g| &g.run) != Some(run) {
            return Err(Refused::WouldChangeOther {
                what: format!("`[gates.{name}]` to something other than what was chosen"),
            });
        }
    }
    let strip = |text: &str| -> toml::Table {
        let mut t: toml::Table = text.parse().unwrap_or_default();
        if let Some(toml::Value::Table(g)) = t.get_mut("gates") {
            if !declared.check.is_empty() {
                g.remove("check");
            }
            for name in declared.named.keys() {
                if let Some(toml::Value::Table(n)) = g.get_mut(name) {
                    n.remove("run");
                    if n.is_empty() {
                        g.remove(name);
                    }
                }
            }
            if g.is_empty() {
                t.remove("gates");
            }
        }
        t
    };
    if strip(before) != strip(after) {
        return Err(Refused::WouldChangeOther {
            what: "another key".into(),
        });
    }
    Ok(())
}

/// `key = [...]`, on one line when it is short, else one command a line.
fn assign(key: &str, commands: &[String]) -> Vec<String> {
    let quoted: Vec<String> = commands
        .iter()
        .map(|c| toml::Value::String(c.clone()).to_string())
        .collect();
    let one = format!("{key} = [{}]", quoted.join(", "));
    if one.len() <= 80 && !one.contains('\n') {
        return vec![one];
    }
    let mut out = vec![format!("{key} = [")];
    out.extend(quoted.into_iter().map(|q| format!("  {q},")));
    out.push("]".into());
    out
}

/// The header's name, when `line` is a table header (`[x]`, not `[[x]]`).
fn header_name(line: &str) -> Option<String> {
    let t = line.trim();
    let t = t.split_once('#').map_or(t, |(h, _)| h).trim();
    if t.starts_with("[[") {
        return Some(String::new());
    }
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    Some(
        inner
            .split('.')
            .map(|p| p.trim().trim_matches('"'))
            .collect::<Vec<_>>()
            .join("."),
    )
}

/// The header line of table `name` and the line its body ends before.
fn table_range(lines: &[String], name: &str) -> Option<(usize, usize)> {
    let header = lines
        .iter()
        .position(|l| header_name(l).as_deref() == Some(name))?;
    let end = lines[header + 1..]
        .iter()
        .position(|l| header_name(l).is_some())
        .map_or(lines.len(), |i| header + 1 + i);
    // Blank lines and comments before the next header belong to what follows.
    let mut end = end;
    while end > header + 1 && {
        let t = lines[end - 1].trim();
        t.is_empty() || t.starts_with('#')
    } {
        end -= 1;
    }
    Some((header, end))
}

/// The lines `key = …` spans within `[from, to)`, a multi-line array whole.
fn key_range(lines: &[String], from: usize, to: usize, key: &str) -> Option<(usize, usize)> {
    let start = (from..to).find(|&i| {
        let t = lines[i].trim_start();
        t.strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })?;
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for (i, line) in lines.iter().enumerate().skip(start) {
        let mut chars = line.chars().peekable();
        let mut seen_eq = i > start;
        while let Some(c) = chars.next() {
            match quote {
                Some(q) => {
                    if c == '\\' && q == '"' {
                        chars.next();
                    } else if c == q {
                        quote = None;
                    }
                }
                None => match c {
                    '=' => seen_eq = true,
                    '"' | '\'' if seen_eq => quote = Some(c),
                    '[' if seen_eq => depth += 1,
                    ']' if seen_eq => depth -= 1,
                    '#' => break,
                    _ => {}
                },
            }
        }
        if depth <= 0 {
            return Some((start, i + 1));
        }
    }
    Some((start, lines.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(cmds: &[&str]) -> Declared {
        Declared {
            check: cmds.iter().map(|s| s.to_string()).collect(),
            named: BTreeMap::new(),
        }
    }

    #[test]
    fn no_file_becomes_the_origin_comment_and_the_check() {
        let out = splice(None, &check(&["npm test"])).unwrap();
        assert_eq!(out, format!("{ORIGIN}\n[gates]\ncheck = [\"npm test\"]\n"));
        assert_eq!(
            ProjectConfig::parse(&out).unwrap().gates.check,
            vec!["npm test"]
        );
    }

    #[test]
    fn an_existing_file_keeps_every_other_byte() {
        let before = "# my project\n[project]\nname = \"x\"  # inline\n\n[gates]\n# the suite\ncheck = [\"old\"]\ntimeout = \"5m\"\n\n[policy]\ndeny = [\"Bash(rm:*)\"]\n";
        let out = splice(Some(before), &check(&["cargo test", "cargo clippy"])).unwrap();
        let expected = format!(
            "# my project\n[project]\nname = \"x\"  # inline\n\n[gates]\n# the suite\n{ORIGIN}\ncheck = [\"cargo test\", \"cargo clippy\"]\ntimeout = \"5m\"\n\n[policy]\ndeny = [\"Bash(rm:*)\"]\n"
        );
        assert_eq!(out, expected);
        // Declaring again replaces the comment too, never doubling it.
        let again = splice(Some(&out), &check(&["cargo test"])).unwrap();
        assert_eq!(again.matches(ORIGIN).count(), 1);
    }

    #[test]
    fn a_gates_table_without_check_gets_it_under_the_header_and_none_at_all_is_appended() {
        let before = "[gates]\ntimeout = \"5m\"\n";
        let out = splice(Some(before), &check(&["just test"])).unwrap();
        assert_eq!(
            out,
            format!("[gates]\n{ORIGIN}\ncheck = [\"just test\"]\ntimeout = \"5m\"\n")
        );
        let before = "[project]\nname = \"x\"\n";
        let out = splice(Some(before), &check(&["just test"])).unwrap();
        assert!(out.starts_with(before), "{out}");
        assert!(out.ends_with(&format!("\n{ORIGIN}\n[gates]\ncheck = [\"just test\"]\n")));
    }

    #[test]
    fn a_multi_line_check_array_is_replaced_whole() {
        let before =
            "[gates]\ncheck = [\n  \"a\",  # first\n  \"b ]\",\n]\nmax_feedback_rounds = 1\n";
        let out = splice(Some(before), &check(&["c"])).unwrap();
        assert_eq!(
            out,
            format!("[gates]\n{ORIGIN}\ncheck = [\"c\"]\nmax_feedback_rounds = 1\n")
        );
    }

    #[test]
    fn openspec_is_its_own_table_and_check_is_untouched() {
        let before = "[gates]\ncheck = [\"npm test\"]\n";
        let mut d = Declared::default();
        d.named.insert(
            "openspec".into(),
            vec!["openspec validate --all --strict".into()],
        );
        let out = splice(Some(before), &d).unwrap();
        assert!(out.starts_with(before));
        assert!(out.contains("\n[gates.openspec]\nrun = [\"openspec validate --all --strict\"]\n"));
        assert!(!out.contains("gates.named"));
        let cfg = ProjectConfig::parse(&out).unwrap();
        assert_eq!(cfg.gates.check, vec!["npm test"]);
        assert_eq!(
            cfg.gates.named["openspec"].run,
            vec!["openspec validate --all --strict"]
        );
    }

    #[test]
    fn an_unparseable_file_is_refused_with_its_line() {
        let before = "[project]\nname = \"x\"\n[gates\ncheck = []\n";
        match splice(Some(before), &check(&["x"])) {
            Err(Refused::Unparseable { line, .. }) => assert_eq!(line, Some(3)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_dotted_check_elsewhere_is_refused_rather_than_doubled() {
        // `check` is set without a `[gates]` header: appending one would be a
        // second definition, which the re-parse refuses.
        let before = "gates.check = [\"a\"]\n";
        assert!(splice(Some(before), &check(&["b"])).is_err());
    }

    #[test]
    fn nothing_chosen_and_bad_names_are_refused() {
        assert_eq!(splice(None, &check(&["  "])), Err(Refused::NothingDeclared));
        let mut d = Declared::default();
        d.named.insert("a.b".into(), vec!["x".into()]);
        assert!(matches!(splice(None, &d), Err(Refused::BadName { .. })));
    }

    #[test]
    fn commands_are_quoted_as_toml_never_altered() {
        let cmd = "sh -c 'echo \"hi\" \\ done'";
        let out = splice(None, &check(&[cmd])).unwrap();
        assert_eq!(ProjectConfig::parse(&out).unwrap().gates.check, vec![cmd]);
    }
}

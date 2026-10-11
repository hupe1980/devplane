//! The weakened rows and the failing named tests, as GitHub workflow-command
//! annotations and as a SARIF 2.1.0 log. Pure: the edge says which paths are
//! files and prints or writes what this returns. Nothing here is uploaded or
//! posted; uploading the SARIF is the person's own workflow step.
//!
//! An annotation is evidence placed on a line, never a verdict: printing one
//! changes no exit code.

use crate::core::change::GateReport;
use crate::core::diff::{Body, ChangeSet, Kind, Status};
use crate::core::review::{WeakKind, Weakened};
use crate::core::testcount::junit::CaseOutcome;
use serde_json::{Value, json};

/// GitHub shows at most this many warning annotations, and as many error
/// annotations, per step; the rest are counted, never silently dropped.
pub const PER_STEP: usize = 10;

/// Bounds on repository text in one annotation, in characters.
const WHY_CHARS: usize = 400;
const MATCHED_CHARS: usize = 160;
const PATH_CHARS: usize = 240;

/// How a finding is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// A weakened row: read first, never a failure.
    Warning,
    /// A named test that failed in `check`.
    Error,
}

impl Level {
    fn command(self) -> &'static str {
        match self {
            Level::Warning => "warning",
            Level::Error => "error",
        }
    }
}

/// One row or failing test, placed on a file and line where one is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    /// `weakened/skip`, `test/failed`, …: the SARIF rule.
    pub rule: &'static str,
    /// A path in the repository, relative to its root; `None` when the row
    /// names no file (the diff truncated, the base checks as a whole).
    pub file: Option<String>,
    /// From one, on the change's side; `None` gives a file-level annotation.
    pub line: Option<u32>,
    pub title: String,
    pub message: String,
}

/// How many annotations were printed and how many GitHub would not show.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counted {
    pub printed: usize,
    pub left_out: usize,
}

fn rule_of(kind: WeakKind) -> &'static str {
    match kind {
        WeakKind::Skip => "weakened/skip",
        WeakKind::Deleted => "weakened/deleted",
        WeakKind::Gate => "weakened/gate",
        WeakKind::AgentControls => "weakened/agent-controls",
        WeakKind::Ownership => "weakened/ownership",
        WeakKind::SupplyChain => "weakened/supply-chain",
        WeakKind::BaseTests => "weakened/base-tests",
    }
}

const FAILED_TEST: &str = "test/failed";

/// Repository text on one line: control characters dropped, bounded.
fn flat(s: &str, max: usize) -> String {
    let one: String = s
        .chars()
        .map(|c| {
            if matches!(c, '\n' | '\r' | '\t') {
                ' '
            } else {
                c
            }
        })
        .filter(|c| !c.is_control())
        .collect();
    crate::core::text::clip(one.trim(), max)
}

/// A path a reader can open in the repository: relative, no `..`, no
/// backslash, no empty part. Anything else is named in the message instead.
fn repository_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

/// The line on the change's side that a row matched: the added line it
/// quotes, or where a removed line it quotes stood. `None` when the row
/// matched no single line (a digest of hunks, a deleted file, a path).
pub fn line_of(set: &ChangeSet, row: &Weakened) -> Option<u32> {
    let want = row.matched.trim();
    if want.is_empty() {
        return None;
    }
    let f = set.uncut().files.iter().find(|f| f.path == row.path)?;
    if matches!(f.status, Status::Deleted) {
        return None;
    }
    let Body::Hunks(hunks) = &f.body else {
        return None;
    };
    for h in hunks {
        let mut new = new_start(&h.header)?;
        for (kind, text) in &h.lines {
            let hit = text.trim() == want;
            match kind {
                Kind::Added if hit => return Some(new.max(1)),
                Kind::Removed if hit => return Some(new.max(1)),
                Kind::Added | Kind::Context => new += 1,
                Kind::Removed => {}
            }
        }
    }
    None
}

/// `c` of an `@@ -a,b +c,d @@` header.
fn new_start(header: &str) -> Option<u32> {
    let plus = header.split_whitespace().nth(2)?.strip_prefix('+')?;
    plus.split(',').next()?.parse().ok()
}

/// Every weakened row, then every named test that failed or errored in
/// `check`. `is_file` says whether a path is a file in the checkout; a row's
/// path that is neither that nor in the change is named in the message.
pub fn findings(
    rows: &[Weakened],
    set: Option<&ChangeSet>,
    report: Option<&GateReport>,
    is_file: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let in_change = |p: &str| set.is_some_and(|s| s.uncut().files.iter().any(|f| f.path == p));
    let placed = |p: &str| repository_path(p) && (in_change(p) || is_file(p));
    let mut out = Vec::new();
    for kind in WeakKind::ALL {
        for w in rows.iter().filter(|w| w.kind == kind) {
            let file = placed(&w.path).then(|| w.path.clone());
            let line = match (&file, set) {
                (Some(_), Some(s)) => line_of(s, w),
                _ => None,
            };
            let why = flat(&w.why, WHY_CHARS);
            let matched = flat(&w.matched, MATCHED_CHARS);
            let said = match matched.is_empty() {
                true => why,
                false => format!("{why} — {matched}"),
            };
            let message = match (&file, w.path.is_empty()) {
                (None, false) => format!("{}: {said}", flat(&w.path, PATH_CHARS)),
                _ => said,
            };
            out.push(Finding {
                level: Level::Warning,
                rule: rule_of(kind),
                file,
                line,
                title: format!("Devplane verify — {}", kind.says()),
                message,
            });
        }
    }
    for f in report.map(|r| r.junit.as_slice()).unwrap_or_default() {
        for c in f.read.cases() {
            let outcome = match c.outcome {
                CaseOutcome::Failed => "failed",
                CaseOutcome::Error => "errored",
                CaseOutcome::Passed | CaseOutcome::Skipped => continue,
            };
            let file = c.file.as_deref().filter(|p| placed(p)).map(str::to_string);
            let name = flat(&c.id().says(), PATH_CHARS);
            let message = match c.nth {
                0 => format!("{name} {outcome} in the base's check"),
                n => format!("{name} #{n} {outcome} in the base's check"),
            };
            out.push(Finding {
                level: Level::Error,
                rule: FAILED_TEST,
                file,
                line: None,
                title: "Devplane verify — test failed".into(),
                message,
            });
        }
    }
    out
}

/// GitHub's escaping for a command's message: `%`, CR and LF.
pub fn escape_data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// GitHub's escaping for a command's property: the message's, and `:` and `,`.
pub fn escape_property(s: &str) -> String {
    escape_data(s).replace(':', "%3A").replace(',', "%2C")
}

/// One finding as a workflow command:
/// `::warning file=…,line=…,title=…::message`.
pub fn command(f: &Finding) -> String {
    let mut props: Vec<String> = Vec::new();
    if let Some(file) = &f.file {
        props.push(format!("file={}", escape_property(file)));
        if let Some(line) = f.line {
            props.push(format!("line={line}"));
        }
    }
    props.push(format!("title={}", escape_property(&f.title)));
    format!(
        "::{} {}::{}",
        f.level.command(),
        props.join(","),
        escape_data(&f.message)
    )
}

/// The commands GitHub will show — at most [`PER_STEP`] of each level, in
/// order — and how many were left out.
pub fn commands(findings: &[Finding]) -> (Vec<String>, Counted) {
    let mut out = Vec::new();
    let mut left_out = 0;
    for level in [Level::Warning, Level::Error] {
        let of: Vec<&Finding> = findings.iter().filter(|f| f.level == level).collect();
        out.extend(of.iter().take(PER_STEP).map(|f| command(f)));
        left_out += of.len().saturating_sub(PER_STEP);
    }
    let printed = out.len();
    (out, Counted { printed, left_out })
}

/// A relative path as a URI reference: unreserved characters and `/` kept,
/// everything else percent-encoded byte by byte.
fn uri(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Where a finding with no file is placed in SARIF, which a code-scanning
/// upload needs a location for: the file whose `check` governs.
const NO_FILE: &str = super::CONFIG_FILE;

fn rule_says(rule: &str) -> &'static str {
    match rule {
        "weakened/skip" => "A check turned off, loosened or silenced",
        "weakened/deleted" => "A test removed",
        "weakened/gate" => "What passing means, edited",
        "weakened/agent-controls" => "What governs the agent, edited",
        "weakened/ownership" => "Who must review a path, edited",
        "weakened/supply-chain" => "What the build trusts, edited",
        "weakened/base-tests" => "A test as the base wrote it fails on this change",
        _ => "A named test failed in the base's check",
    }
}

/// Every finding — none capped — as a SARIF 2.1.0 log with one run.
pub fn sarif(findings: &[Finding], version: &str) -> Value {
    let mut rules: Vec<&'static str> = Vec::new();
    for f in findings {
        if !rules.contains(&f.rule) {
            rules.push(f.rule);
        }
    }
    let results: Vec<Value> = findings
        .iter()
        .map(|f| {
            let index = rules.iter().position(|r| *r == f.rule).unwrap_or(0);
            let mut region = Value::Null;
            if let (Some(_), Some(line)) = (&f.file, f.line) {
                region = json!({ "startLine": line });
            }
            let mut physical = json!({
                "artifactLocation": {
                    "uri": uri(f.file.as_deref().unwrap_or(NO_FILE)),
                    "uriBaseId": "%SRCROOT%",
                },
            });
            if !region.is_null() {
                physical["region"] = region;
            }
            json!({
                "ruleId": f.rule,
                "ruleIndex": index,
                "level": match f.level {
                    Level::Warning => "warning",
                    Level::Error => "error",
                },
                "message": { "text": f.message },
                "locations": [{ "physicalLocation": physical }],
            })
        })
        .collect();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "devplane verify",
                    "version": version,
                    "rules": rules.iter().map(|r| json!({
                        "id": r,
                        "shortDescription": { "text": rule_says(r) },
                        "defaultConfiguration": {
                            "level": if *r == FAILED_TEST { "error" } else { "warning" },
                        },
                    })).collect::<Vec<_>>(),
                },
            },
            "results": results,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::{FileChange, Hunk};

    fn row(path: &str, kind: WeakKind, matched: &str) -> Weakened {
        Weakened {
            path: path.into(),
            why: "adds a skip marker".into(),
            kind,
            matched: matched.into(),
        }
    }

    fn set(path: &str, header: &str, lines: &[(Kind, &str)]) -> ChangeSet {
        ChangeSet {
            base: "b".into(),
            files: vec![FileChange {
                path: path.into(),
                status: Status::Modified,
                added: 0,
                removed: 0,
                body: Body::Hunks(vec![Hunk {
                    header: header.into(),
                    lines: lines.iter().map(|(k, t)| (*k, t.to_string())).collect(),
                }]),
            }],
            truncated: None,
            whole: None,
        }
    }

    #[test]
    fn the_line_is_the_added_line_on_the_changes_side() {
        let s = set(
            "t.js",
            "@@ -7,3 +7,4 @@ describe",
            &[
                (Kind::Context, "a"),
                (Kind::Removed, "  it(\"x\")"),
                (Kind::Added, "  it.skip(\"x\")"),
                (Kind::Context, "b"),
                (Kind::Removed, "  expect(y)"),
                (Kind::Context, "c"),
            ],
        );
        assert_eq!(
            line_of(&s, &row("t.js", WeakKind::Skip, "it.skip(\"x\")")),
            Some(8)
        );
        // A removed line: where it stood, on the change's side.
        assert_eq!(
            line_of(&s, &row("t.js", WeakKind::Skip, "expect(y)")),
            Some(10)
        );
        assert_eq!(line_of(&s, &row("t.js", WeakKind::Gate, "digest")), None);
        assert_eq!(line_of(&s, &row("u.js", WeakKind::Skip, "a")), None);
    }

    #[test]
    fn properties_and_data_are_escaped_as_github_reads_them() {
        assert_eq!(escape_data("a%b\r\nc: d, e"), "a%25b%0D%0Ac: d, e");
        assert_eq!(escape_property("a%b\r\nc: d, e"), "a%25b%0D%0Ac%3A d%2C e");
        let f = Finding {
            level: Level::Warning,
            rule: "weakened/skip",
            file: Some("dir,x/a:b.js".into()),
            line: Some(3),
            title: "T: t".into(),
            message: "50% off\n::error::forged".into(),
        };
        assert_eq!(
            command(&f),
            "::warning file=dir%2Cx/a%3Ab.js,line=3,title=T%3A t::50%25 off%0A::error::forged"
        );
        assert!(!command(&f).contains('\n'));
    }

    #[test]
    fn a_row_with_no_line_is_file_level_and_one_with_no_file_names_its_path() {
        let rows = [
            row(".github/workflows/ci.yml", WeakKind::Gate, "digest"),
            row("base checks", WeakKind::BaseTests, "base tests timed out"),
            row("../../etc/passwd", WeakKind::BaseTests, "x"),
        ];
        let f = findings(&rows, None, None, &|p| {
            p.ends_with("ci.yml") || p.contains("etc")
        });
        let cmds: Vec<String> = f.iter().map(command).collect();
        assert!(
            cmds[0].starts_with("::warning file=.github/workflows/ci.yml,title="),
            "{cmds:?}"
        );
        assert!(cmds[1].starts_with("::warning title="), "{cmds:?}");
        assert!(cmds[1].ends_with("::base checks: adds a skip marker — base tests timed out"));
        assert!(cmds[2].starts_with("::warning title="), "{cmds:?}");
    }

    #[test]
    fn at_most_ten_of_each_level_and_the_rest_counted() {
        let rows: Vec<Weakened> = (0..13)
            .map(|i| row(&format!("t{i}.js"), WeakKind::Skip, "x"))
            .collect();
        let f = findings(&rows, None, None, &|_| true);
        let (cmds, n) = commands(&f);
        assert_eq!(cmds.len(), PER_STEP);
        assert_eq!(
            n,
            Counted {
                printed: 10,
                left_out: 3
            }
        );
        // SARIF carries every one.
        let s = sarif(&f, "0");
        assert_eq!(s["runs"][0]["results"].as_array().unwrap().len(), 13);
    }

    #[test]
    fn a_path_is_a_uri_reference() {
        assert_eq!(uri("a b/c#d.js"), "a%20b/c%23d.js");
        assert_eq!(uri("tests/ü.js"), "tests/%C3%BC.js");
    }
}

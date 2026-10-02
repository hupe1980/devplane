//! The job-page Markdown and the terminal text: one section list, rendered
//! twice. The checks the change weakened come first after the heading; every
//! piece of repository text is inert; the whole is bounded, and every fold
//! says how much it left out.

use super::{ChangeDeclares, CouldNotRun, Failed, Run, Verdict, short};
use crate::core::change::{CommandResult, GateReport};
use crate::core::review::{WeakKind, Weakened};

/// The step summary's bound, under GitHub's 1 MiB per step.
pub const LIMIT_BYTES: usize = 900 * 1024;

/// Bounds on one piece of repository text, in characters.
pub const PATH_CHARS: usize = 240;
pub const MATCHED_CHARS: usize = 160;
pub const COMMAND_CHARS: usize = 200;
const LINE_CHARS: usize = 400;

/// Failure lines listed per command before the rest are counted.
pub const FAILURE_LINES: usize = 20;

/// What the Limits section says, in every rendering.
pub const LIMITS: &str = "This is evidence that these commands ended as recorded against this \
tree. It is not evidence that the work is correct, that the commands check the right things, or \
that the specification was satisfied. `check` ran code in this tree, including files this change \
edited. This workflow's own definition is part of the pull request; branch protection and code \
owners on `.github/workflows/` are what guard it. Whether this job must pass to merge is set in \
branch protection.";

/// Repository text as a code span nothing inside can close: control
/// characters dropped, newlines flattened, bounded at `max` characters with
/// the remainder counted after the span.
pub fn inert(text: &str, max: usize) -> String {
    let flat: String = text
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
    let n = flat.chars().count();
    let (body, more) = match n > max {
        true => (flat.chars().take(max).collect::<String>(), n - max),
        false => (flat, 0),
    };
    let fence = "`".repeat(longest_backticks(&body) + 1);
    // Padded on both sides: CommonMark strips exactly one space from each, so
    // a backtick at either end cannot fuse with the delimiter.
    let span = format!("{fence} {body} {fence}");
    match more {
        0 => span,
        n => format!("{span} … ({n} more characters)"),
    }
}

/// [`inert`] for a table cell, where a pipe would end the cell.
fn cell(text: &str, max: usize) -> String {
    inert(text, max).replace('|', "\\|")
}

/// Our own sentence that may quote repository text: nothing in it links,
/// mentions, or opens markup.
fn prose(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| {
            if matches!(c, '\n' | '\r' | '\t') {
                ' '
            } else {
                c
            }
        })
        .filter(|c| !c.is_control())
        .take(max)
        .collect();
    flat.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('@', "&#64;")
}

fn longest_backticks(s: &str) -> usize {
    s.as_bytes()
        .split(|b| *b != b'`')
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0)
}

/// A fenced block whose fence is longer than any backtick run inside it.
fn fenced(content: &str) -> String {
    let clean: String = content
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect();
    let fence = "`".repeat(longest_backticks(&clean).max(2) + 1);
    format!("{fence}text\n{clean}\n{fence}")
}

fn size(n: u64) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{:.1} MiB", n as f64 / (1024.0 * 1024.0)),
        n if n >= 1024 => format!("{} KiB", n.div_ceil(1024)),
        n => format!("{n} bytes"),
    }
}

fn duration(secs: u64) -> String {
    match secs {
        s if s >= 3600 && s % 3600 == 0 => format!("{}h", s / 3600),
        s if s >= 60 && s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

const fn nouned(k: WeakKind) -> (WeakKind, &'static str, &'static str) {
    let (one, many) = k.nouns();
    (k, one, many)
}

/// The kinds in the order the first section lists them, with their nouns:
/// [`WeakKind::ALL`]'s order and [`WeakKind::nouns`]' words.
const KINDS: [(WeakKind, &str, &str); 7] = [
    nouned(WeakKind::ALL[0]),
    nouned(WeakKind::ALL[1]),
    nouned(WeakKind::ALL[2]),
    nouned(WeakKind::ALL[3]),
    nouned(WeakKind::ALL[4]),
    nouned(WeakKind::ALL[5]),
    nouned(WeakKind::ALL[6]),
];

fn count(n: usize, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

/// *1 check weakened · 1 gate changed*, or *nothing weakened*.
pub fn counts_says(rows: &[Weakened]) -> String {
    let q = crate::core::review::Qualifier::of(rows, &std::collections::HashSet::new());
    match q.is_empty() {
        true => "nothing weakened".into(),
        false => q.says,
    }
}

/// The rows counted by kind, for `--json`.
pub fn counts(rows: &[Weakened]) -> serde_json::Value {
    let q = crate::core::review::Qualifier::of(rows, &std::collections::HashSet::new());
    serde_json::json!({
        "weakened": q.weakened,
        "deleted": q.deleted,
        "gatesChanged": q.gates_changed,
        "agentControls": q.agent_controls,
        "ownership": q.ownership,
        "supplyChain": q.supply_chain,
        "baseTests": q.base_tests,
        "says": counts_says(rows),
    })
}

/// How much the Markdown may show; shrunk until it fits.
#[derive(Debug, Clone, Copy)]
struct Room {
    tails: bool,
    rows_per_kind: usize,
}

/// The Markdown for the job page, under `limit` bytes.
pub fn markdown(run: &Run<'_>, limit: usize) -> String {
    let mut room = Room {
        tails: true,
        rows_per_kind: run.rows.len(),
    };
    loop {
        let out = compose_markdown(run, room);
        if out.len() <= limit {
            return out;
        }
        if room.tails {
            room.tails = false;
        } else if room.rows_per_kind > 0 {
            room.rows_per_kind = room.rows_per_kind * 3 / 4;
        } else {
            return out;
        }
    }
}

fn heading_sentence(run: &Run<'_>) -> String {
    let Some(base) = run.base else {
        return prose(&run.verdict.says(), 2000);
    };
    let against = format!(
        "`check` as declared at the merge base with {} ({})",
        inert(&base.asked, PATH_CHARS),
        inert(base.short(), 40)
    );
    let what = match run.verdict {
        Verdict::Verified { tree } => format!(
            "{against} exited 0 on every command, on tree {}.",
            inert(short(tree), 40)
        ),
        Verdict::Failed {
            why: Failed::Command { command, says },
        } => format!(
            "{against} failed: {} ended with {}.",
            inert(command, COMMAND_CHARS),
            prose(says, 400)
        ),
        Verdict::Failed {
            why: Failed::TreeMoved { .. },
        } => format!(
            "{against}: every command exited 0, but `check` changed the tree it ran on, so the \
             result describes no tree."
        ),
        Verdict::Failed {
            why: Failed::TreeUnreadable { why },
        } => format!(
            "{against}: every command exited 0, but the tree could not be read around them, so \
             the result describes no tree: {}.",
            prose(why, 400)
        ),
        Verdict::CouldNotRun { why } => {
            prose(&Verdict::CouldNotRun { why: why.clone() }.says(), 2000)
        }
    };
    let merged = match base.forge_merge {
        true => format!(
            " The checked-out commit {} is the pull request merged into {}.",
            inert(short(&base.head), 40),
            inert(&base.asked, PATH_CHARS)
        ),
        false => String::new(),
    };
    format!("{what}{merged}")
}

fn compose_markdown(run: &Run<'_>, room: Room) -> String {
    let mut o = String::new();
    o.push_str(&format!("## Devplane verify — {}\n\n", run.verdict.word()));
    o.push_str(&heading_sentence(run));
    o.push_str("\n\n");
    if let Some(c) = run.base_checks.counts().filter(|_| run.report.is_some()) {
        o.push_str(&format!("Tests: {}.\n\n", prose(c, 400)));
    }

    let full = run.report.is_some();
    if !full {
        o.push_str("### Limits\n\n");
        o.push_str(LIMITS);
        o.push('\n');
        return o;
    }

    // 1. Weakened — always first.
    let at = run.base.map(|b| b.short().to_string()).unwrap_or_default();
    o.push_str(&format!(
        "### Weakened checks — {}\n\n",
        counts_says(run.rows)
    ));
    if run.no_change {
        let asked = run.base.map(|b| b.asked.as_str()).unwrap_or("the base");
        o.push_str(&format!(
            "No change against {}: the merge base is this commit.\n\n",
            inert(asked, PATH_CHARS)
        ));
    } else if run.rows.is_empty() {
        o.push_str("This change weakens no check the diff scan recognises.\n\n");
    } else {
        o.push_str(&format!(
            "Against {}, this change weakens or changes these checks. They do not fail \
             this job; they are what to read first.\n\n",
            inert(&at, 40)
        ));
        for (kind, one, many) in KINDS {
            let rows: Vec<&Weakened> = run.rows.iter().filter(|w| w.kind == kind).collect();
            if rows.is_empty() {
                continue;
            }
            o.push_str(&format!("#### {}\n\n", count(rows.len(), one, many)));
            let shown = rows.len().min(room.rows_per_kind);
            if shown > 0 {
                o.push_str("| Path | Why | Matched |\n|---|---|---|\n");
                for w in &rows[..shown] {
                    o.push_str(&format!(
                        "| {} | {} | {} |\n",
                        cell(&w.path, PATH_CHARS),
                        prose(&w.why, 400),
                        cell(&w.matched, MATCHED_CHARS)
                    ));
                }
                o.push('\n');
            }
            if rows.len() > shown {
                o.push_str(&format!(
                    "And {} more {} — `devplane verify --json` lists them all.\n\n",
                    rows.len() - shown,
                    match rows.len() - shown {
                        1 => "row",
                        _ => "rows",
                    }
                ));
            }
        }
    }
    if let Some(t) = run.truncated {
        o.push_str(&format!(
            "The diff was truncated: {} of {} files shown. Rows were read from the whole diff.\n\n",
            t.files_shown, t.files_total
        ));
    }
    if let Some(line) = annotations_say(run) {
        o.push_str(&line);
        o.push_str("\n\n");
    }

    // 2. Gate.
    o.push_str("### Gate\n\n");
    if let Some(d) = run.definition {
        o.push_str(&format!(
            "Ran `check` from `devplane.toml` at {} (timeout {}):\n\n",
            inert(&at, 40),
            duration(d.timeout_secs)
        ));
        o.push_str("| Command | Outcome |\n|---|---|\n");
        for (i, c) in d.check.iter().enumerate() {
            let outcome = run
                .report
                .and_then(|r| r.commands.get(i))
                .map(|r| prose(&crate::core::certificate::outcome_sentence(r), 400))
                .unwrap_or_else(|| "never started — an earlier command failed".into());
            o.push_str(&format!("| {} | {} |\n", cell(c, COMMAND_CHARS), outcome));
        }
        o.push('\n');
        match &d.change_declares {
            ChangeDeclares::Same => {}
            ChangeDeclares::Differs { check } => {
                o.push_str(
                    "This change declares a different `check`, which did not run and takes \
                     effect only after it merges.\n\n",
                );
                o.push_str("| Base (ran) | This change (not run) |\n|---|---|\n");
                for i in 0..d.check.len().max(check.len()).max(1) {
                    let side = |v: &[String]| {
                        v.get(i)
                            .map(|c| cell(c, COMMAND_CHARS))
                            .unwrap_or_else(|| "—".into())
                    };
                    o.push_str(&format!("| {} | {} |\n", side(&d.check), side(check)));
                }
                o.push('\n');
            }
            ChangeDeclares::Absent => o.push_str(
                "This change deletes `devplane.toml`; the base's `check` ran, and the deletion \
                 takes effect only after it merges.\n\n",
            ),
            ChangeDeclares::Unreadable { why } => o.push_str(&format!(
                "This change's `devplane.toml` does not parse: {}. The base's `check` ran.\n\n",
                inert(why, 400)
            )),
        }
    }
    if let Some(r) = run.report {
        for c in r.commands.iter().filter(|c| !c.passed()) {
            failed_block(&mut o, c, room.tails);
        }
    }
    if let Verdict::Failed {
        why: Failed::TreeMoved { paths, more },
    } = run.verdict
    {
        o.push_str(
            "Every command exited 0, but `check` changed the tree it ran on, so the result \
             describes no tree. Moved:\n\n",
        );
        for p in paths {
            o.push_str(&format!("- {}\n", inert(p, PATH_CHARS)));
        }
        if *more > 0 {
            o.push_str(&format!("- and {more} more\n"));
        }
        o.push('\n');
    }

    // 3. Base checks.
    o.push_str("### Base checks\n\n");
    for line in run.base_checks.says() {
        o.push_str(&prose(&line, 2000));
        o.push_str("\n\n");
    }
    let (passing, more) = run.base_checks.passing_without();
    if !passing.is_empty() {
        o.push_str("New tests that pass without the change — they may not test what it adds:\n\n");
        for t in passing {
            o.push_str(&format!("- {}\n", inert(t, PATH_CHARS)));
        }
        if more > 0 {
            o.push_str(&format!(
                "- and {more} more — `devplane verify --json` lists them all\n"
            ));
        }
        o.push('\n');
    }

    // 4. Not done.
    o.push_str("### Not done by verify\n\n");
    if run.not_done.is_empty() {
        o.push_str("The base declares no `[workspace] setup` and no named gates; `check` is all it defines.\n\n");
    } else {
        for line in run.not_done {
            o.push_str(&format!("- {line}\n"));
        }
        o.push('\n');
    }

    // 5. Limits.
    o.push_str("### Limits\n\n");
    o.push_str(LIMITS);
    o.push('\n');
    o
}

/// How many annotations were printed, and how many GitHub would not show.
fn annotations_say(run: &Run<'_>) -> Option<String> {
    let c = run.annotations?;
    match (c.printed, c.left_out) {
        (0, 0) => None,
        (n, 0) => Some(format!(
            "Annotations: {n} written, each on its file and line where one is known."
        )),
        (n, more) => Some(format!(
            "Annotations: {n} written, each on its file and line where one is known; {more} left out, because GitHub shows at most {} warnings \
             and {} errors per step. `--sarif` writes every one.",
            super::annotate::PER_STEP,
            super::annotate::PER_STEP,
        )),
    }
}

fn failed_block(o: &mut String, c: &CommandResult, tails: bool) {
    o.push_str(&format!(
        "**{}** — {}\n\n",
        inert(&c.command, COMMAND_CHARS),
        prose(&crate::core::certificate::outcome_sentence(c), 400)
    ));
    if !c.failures.is_empty() {
        for f in c.failures.iter().take(FAILURE_LINES) {
            o.push_str(&format!("- {}\n", inert(f, LINE_CHARS)));
        }
        if c.failures.len() > FAILURE_LINES {
            o.push_str(&format!(
                "- and {} more failure lines\n",
                c.failures.len() - FAILURE_LINES
            ));
        }
        o.push('\n');
    }
    if c.output_tail.is_empty() {
        return;
    }
    let label = format!(
        "Output, last {} of {}",
        size(c.output_tail.len() as u64),
        size(c.output_bytes)
    );
    match tails {
        true => o.push_str(&format!(
            "<details><summary>{label}</summary>\n\n{}\n\n</details>\n\n",
            fenced(&c.output_tail)
        )),
        false => o.push_str(&format!(
            "{label}: left out to fit the summary; `devplane verify --json` carries it.\n\n"
        )),
    }
}

/// The same sections, as terminal text.
pub fn text(run: &Run<'_>) -> String {
    let mut o = String::new();
    let against = match run.base {
        Some(b) => format!("  against {} (merge base {})", b.asked, b.short()),
        None => String::new(),
    };
    match run.verdict {
        Verdict::Verified { tree } => {
            o.push_str(&format!("verified{against} · tree {}\n", short(tree)))
        }
        Verdict::Failed { .. } => {
            o.push_str(&format!("failed{against}\n  {}\n", run.verdict.says()))
        }
        Verdict::CouldNotRun { why } => {
            let says = run.verdict.says();
            let says = says
                .strip_prefix("Could not run: ")
                .unwrap_or(&says)
                .trim_end_matches('.');
            o.push_str(&format!("could not run  {says}\n"));
            if !matches!(why, CouldNotRun::CouldNotWrite { .. }) || run.report.is_none() {
                return o;
            }
        }
    }
    let Some(report) = run.report else {
        return o;
    };
    o.push('\n');
    if let Some(c) = run.base_checks.counts() {
        o.push_str(&format!("tests     {}\n", plain(c, 400)));
    }
    o.push_str(&format!("weakened  {}\n", counts_says(run.rows)));
    if run.no_change {
        o.push_str("  no change against the base: the merge base is this commit\n");
    } else if run.rows.is_empty() {
        o.push_str("  this change weakens no check the diff scan recognises\n");
    }
    for (kind, _, _) in KINDS {
        for w in run.rows.iter().filter(|w| w.kind == kind) {
            o.push_str(&format!(
                "  {}  {}\n",
                plain(&w.path, PATH_CHARS),
                plain(&w.why, 400)
            ));
            o.push_str(&format!("    {}\n", plain(&w.matched, MATCHED_CHARS)));
        }
    }
    if let Some(t) = run.truncated {
        o.push_str(&format!(
            "  the diff was truncated: {} of {} files shown\n",
            t.files_shown, t.files_total
        ));
    }
    if let Some(line) = annotations_say(run) {
        o.push_str(&format!("  {line}\n"));
    }
    o.push('\n');
    if let Some(d) = run.definition {
        let at = run.base.map(|b| b.short()).unwrap_or("");
        o.push_str(&format!(
            "{:<10}check, as declared at {at} (devplane.toml)\n",
            "gate"
        ));
        gate_lines(&mut o, &d.check, report);
        match &d.change_declares {
            ChangeDeclares::Same => {}
            ChangeDeclares::Differs { check } => o.push_str(&format!(
                "  this change declares check = {:?}, which did not run\n",
                check
                    .iter()
                    .map(|c| plain(c, COMMAND_CHARS))
                    .collect::<Vec<_>>()
            )),
            ChangeDeclares::Absent => o.push_str("  this change deletes devplane.toml\n"),
            ChangeDeclares::Unreadable { why } => o.push_str(&format!(
                "  this change's devplane.toml does not parse: {}\n",
                plain(why, 400)
            )),
        }
    }
    o.push('\n');
    for (i, line) in run.base_checks.says().iter().enumerate() {
        let label = if i == 0 { "base checks" } else { "" };
        o.push_str(&format!("{label:<13}{}\n", plain(line, 400)));
    }
    let (passing, more) = run.base_checks.passing_without();
    for t in passing {
        o.push_str(&format!(
            "  passes without the change  {}\n",
            plain(t, PATH_CHARS)
        ));
    }
    if more > 0 {
        o.push_str(&format!("  and {more} more\n"));
    }
    if !run.not_done.is_empty() {
        o.push_str(&format!("not done     {}\n", run.not_done.join(" ")));
    }
    o
}

fn gate_lines(o: &mut String, check: &[String], report: &GateReport) {
    for (i, c) in check.iter().enumerate() {
        match report.commands.get(i) {
            Some(r) => {
                o.push_str(&format!(
                    "  {:<8} {}\n",
                    r.outcome.headline(),
                    plain(c, COMMAND_CHARS)
                ));
                if !r.passed() {
                    for f in r.failures.iter().take(FAILURE_LINES) {
                        o.push_str(&format!("    {}\n", plain(f, LINE_CHARS)));
                    }
                }
            }
            None => o.push_str(&format!("  never started  {}\n", plain(c, COMMAND_CHARS))),
        }
    }
}

/// Repository text on a terminal: no control characters (no escape sequence
/// reaches the terminal), one line, bounded.
fn plain(s: &str, max: usize) -> String {
    let flat: String = s
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
    crate::core::text::clip(&flat, max)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{report, same};
    use super::super::*;
    use super::*;
    use crate::core::change::{CommandResult, Outcome};

    fn base() -> Base {
        Base {
            asked: "main".into(),
            asked_from: BaseSource::Flag,
            resolved: "origin/main".into(),
            merge_base: "4f2a91c0000000000000000000000000000000000".into(),
            head: "b71c0de000000000000000000000000000000000".into(),
            forge_merge: false,
        }
    }

    fn definition(declares: ChangeDeclares) -> Definition {
        Definition {
            check: vec!["npm test".into(), "npm run lint".into()],
            timeout_secs: 600,
            file_blob: None,
            change_declares: declares,
            has_setup: true,
            has_named: false,
        }
    }

    fn row(path: &str, kind: WeakKind, matched: &str) -> Weakened {
        Weakened {
            path: path.into(),
            why: "adds a skip marker `it.skip(`".into(),
            kind,
            matched: matched.into(),
        }
    }

    fn render(
        verdict: &Verdict,
        rep: Option<&GateReport>,
        rows: &[Weakened],
        d: &Definition,
    ) -> String {
        let b = base();
        let nd = not_done(Some(d));
        let bc = BaseChecks::NotReached;
        markdown(
            &Run {
                base: Some(&b),
                definition: Some(d),
                rows,
                truncated: None,
                no_change: false,
                report: rep,
                verdict,
                base_checks: &bc,
                not_done: &nd,
                annotations: None,
            },
            LIMIT_BYTES,
        )
    }

    fn headings(md: &str) -> Vec<&str> {
        md.lines().filter(|l| l.starts_with("### ")).collect()
    }

    #[test]
    fn the_sections_come_in_order_with_weakened_first() {
        let r = report(vec![
            CommandResult::exited("npm test", 0),
            CommandResult::exited("npm run lint", 0),
        ]);
        let v = Verdict::of(&r, &same("9e1b03d"));
        let rows = [row("tests/a.test.js", WeakKind::Skip, "it.skip(\"x\")")];
        let md = render(&v, Some(&r), &rows, &definition(ChangeDeclares::Same));
        assert!(md.starts_with("## Devplane verify — verified\n"), "{md}");
        let h = headings(&md);
        assert!(
            h[0].starts_with("### Weakened checks — 1 check weakened"),
            "{h:?}"
        );
        assert_eq!(
            &h[1..],
            &[
                "### Gate",
                "### Base checks",
                "### Not done by verify",
                "### Limits"
            ]
        );
        assert!(md.contains("The base's tests were not run on this change"));
        assert!(md.contains("`[workspace] setup` was not run"));
    }

    #[test]
    fn nothing_weakened_is_a_sentence() {
        let r = report(vec![
            CommandResult::exited("npm test", 0),
            CommandResult::exited("npm run lint", 0),
        ]);
        let v = Verdict::of(&r, &same("t"));
        let md = render(&v, Some(&r), &[], &definition(ChangeDeclares::Same));
        assert!(md.contains("This change weakens no check the diff scan recognises."));
    }

    /// Agent controls, ownership and the supply chain are counted on their
    /// own, after the gate and before the base's tests, each its own `####`.
    #[test]
    fn the_controls_kinds_have_their_own_sections_and_counts() {
        assert_eq!(
            KINDS.map(|(k, _, _)| k),
            [
                WeakKind::Skip,
                WeakKind::Deleted,
                WeakKind::Gate,
                WeakKind::AgentControls,
                WeakKind::Ownership,
                WeakKind::SupplyChain,
                WeakKind::BaseTests,
            ]
        );
        let r = report(vec![CommandResult::exited("npm test", 0)]);
        let v = Verdict::of(&r, &same("t"));
        let rows = [
            row(
                ".claude/settings.json",
                WeakKind::AgentControls,
                "changed lines a",
            ),
            row("CODEOWNERS", WeakKind::Ownership, "changed lines b"),
            row("Cargo.lock", WeakKind::SupplyChain, "changed lines c"),
            row("package.json", WeakKind::SupplyChain, "changed lines d"),
        ];
        let md = render(&v, Some(&r), &rows, &definition(ChangeDeclares::Same));
        assert!(
            md.contains(
                "### Weakened checks — 1 agent control changed · 1 ownership file changed · 2 \
                 supply-chain files changed"
            ),
            "{md}"
        );
        let sections: Vec<&str> = md.lines().filter(|l| l.starts_with("#### ")).collect();
        assert_eq!(
            sections,
            [
                "#### 1 agent control changed",
                "#### 1 ownership file changed",
                "#### 2 supply-chain files changed",
            ]
        );
        assert!(md.find("Cargo.lock").unwrap() < md.find("### Gate").unwrap());
        let c = counts(&rows);
        assert_eq!(c["agentControls"], 1);
        assert_eq!(c["ownership"], 1);
        assert_eq!(c["supplyChain"], 2);
        assert_eq!(c["gatesChanged"], 0);
        // The rows never reach the verdict.
        assert_eq!(v.exit_code(), 0);
    }

    #[test]
    fn a_failed_command_is_named_with_its_failures_and_a_folded_tail() {
        let mut bad = CommandResult::exited("npm test", 1);
        bad.failures = vec!["FAIL tests/login.test.js".into()];
        bad.output_tail = "lots of output\n```\n</details>".into();
        bad.output_bytes = 42_000;
        let r = report(vec![bad]);
        let v = Verdict::of(&r, &same("t"));
        let md = render(&v, Some(&r), &[], &definition(ChangeDeclares::Same));
        assert!(md.starts_with("## Devplane verify — failed"));
        assert!(md.contains("**` npm test `** — exit 1"), "{md}");
        assert!(md.contains("- ` FAIL tests/login.test.js `"));
        assert!(
            md.contains("<details><summary>Output, last 29 bytes of 42 KiB</summary>"),
            "{md}"
        );
        assert!(
            md.contains("````text\n"),
            "the fence outgrows the tail's own: {md}"
        );
        assert!(md.contains("never started — an earlier command failed"));
    }

    #[test]
    fn a_moved_tree_names_its_paths() {
        let r = report(vec![CommandResult::exited("npm test", 0)]);
        let v = Verdict::of(
            &r,
            &Trees {
                before: Ok("a".into()),
                after: Ok("b".into()),
                moved: vec!["generated.txt".into()],
            },
        );
        let md = render(&v, Some(&r), &[], &definition(ChangeDeclares::Same));
        assert!(md.contains("- ` generated.txt `"), "{md}");
    }

    #[test]
    fn could_not_run_has_only_its_sentence_and_limits() {
        let v = Verdict::CouldNotRun {
            why: CouldNotRun::NoCheck {
                at: "4f2a91c".into(),
            },
        };
        let md = render(&v, None, &[], &definition(ChangeDeclares::Same));
        assert!(md.starts_with("## Devplane verify — could not run"));
        assert_eq!(headings(&md), vec!["### Limits"]);
        assert!(md.contains("declares no `check`"));
    }

    #[test]
    fn a_differing_definition_is_shown_beside_the_base() {
        let r = report(vec![
            CommandResult::exited("npm test", 0),
            CommandResult::exited("npm run lint", 0),
        ]);
        let v = Verdict::of(&r, &same("t"));
        let md = render(
            &v,
            Some(&r),
            &[],
            &definition(ChangeDeclares::Differs {
                check: vec!["true".into()],
            }),
        );
        assert!(md.contains("| Base (ran) | This change (not run) |"));
        assert!(md.contains("| ` npm test ` | ` true ` |"), "{md}");
        let md = render(&v, Some(&r), &[], &definition(ChangeDeclares::Absent));
        assert!(md.contains("This change deletes `devplane.toml`"));
        let md = render(
            &v,
            Some(&r),
            &[],
            &definition(ChangeDeclares::Unreadable {
                why: "<b>bad</b>".into(),
            }),
        );
        assert!(md.contains("does not parse: ` <b>bad</b> `"));
    }

    #[test]
    fn planted_text_renders_inert() {
        let nasty = "<img src=x> [a](http://x) @someone ```` \u{0}end";
        let s = inert(nasty, MATCHED_CHARS);
        assert!(!s.contains('\u{0}'));
        assert!(s.starts_with("````` "), "{s}");
        assert!(s.ends_with(" `````"), "{s}");
        let long = "x".repeat(10_000);
        assert!(inert(&long, MATCHED_CHARS).ends_with("… (9840 more characters)"));
        // `why` is prose and may quote repository text: nothing in it links.
        let p = prose("see [a](http://x) <img> @someone", 400);
        assert!(!p.contains("[a]"));
        assert!(!p.contains('<'));
        assert!(!p.contains("@someone"));
    }

    #[test]
    fn folds_count_every_row() {
        let rows: Vec<Weakened> = (0..5000usize)
            .map(|i| {
                row(
                    &format!("tests/{i:05}/{}.test.js", "p".repeat(200)),
                    if i.is_multiple_of(2) {
                        WeakKind::Skip
                    } else {
                        WeakKind::Deleted
                    },
                    &"m".repeat(300),
                )
            })
            .collect();
        let r = report(vec![
            CommandResult::exited("npm test", 0),
            CommandResult::exited("npm run lint", 0),
        ]);
        let v = Verdict::of(&r, &same("t"));
        let md = render(&v, Some(&r), &rows, &definition(ChangeDeclares::Same));
        assert!(md.len() <= LIMIT_BYTES, "{}", md.len());
        for (kind, _, _) in KINDS {
            let total = rows.iter().filter(|w| w.kind == kind).count();
            if total == 0 {
                continue;
            }
            let rendered = md
                .lines()
                .filter(|l| l.starts_with("| ` tests/"))
                .filter(|l| {
                    let n: usize = l[10..15].parse().unwrap();
                    n.is_multiple_of(2) == (kind == WeakKind::Skip)
                })
                .count();
            let counted: usize = md
                .lines()
                .filter(|l| l.starts_with("And "))
                .map(|l| l.split(' ').nth(1).unwrap().parse::<usize>().unwrap())
                .sum::<usize>()
                / 2;
            assert_eq!(rendered + counted, total, "{kind:?}");
        }
    }

    #[test]
    fn a_timeout_is_said_in_words() {
        let r = report(vec![CommandResult::without_verdict(
            "npm test",
            Outcome::TimedOut { after_secs: 600 },
        )]);
        let v = Verdict::of(&r, &same("t"));
        let md = render(&v, Some(&r), &[], &definition(ChangeDeclares::Same));
        assert!(md.contains("killed after 600s"));
    }
}

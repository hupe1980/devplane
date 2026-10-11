//! One matcher per runner's summary line, each pinned by a recorded output of
//! the real runner in `tests/fixtures/runners/`. Colour escapes are stripped
//! first. A count that does not fit in `u32` makes the line unrecognised.

use super::{Runner, Tally};

/// Every runner's tally so far, for one pipe.
#[derive(Debug, Default)]
pub struct Acc {
    /// Summed: one `test result:` line per test binary.
    cargo: Option<Tally>,
    /// Summed: one `--- PASS:` line per test and subtest.
    go_v: Option<Tally>,
    /// The last complete summary of each single-summary runner.
    last: std::collections::BTreeMap<Runner, Tally>,
}

impl Acc {
    /// Reads one line (without its newline).
    pub fn line(&mut self, raw: &str) {
        let clean = strip_ansi(raw);
        let t = clean.trim();
        if t.is_empty() {
            return;
        }
        if let Some(rest) = t.strip_prefix("test result: ") {
            if let Some(counts) = cargo(rest) {
                sum(
                    &mut self.cargo,
                    Runner::CargoTest,
                    counts,
                    &["passed", "failed"],
                );
            }
        } else if let Some((noun, _)) = [
            ("pass", "--- PASS: "),
            ("fail", "--- FAIL: "),
            ("skip", "--- SKIP: "),
        ]
        .iter()
        .find(|(_, p)| t.starts_with(p))
        {
            let counts = ["pass", "fail", "skip"]
                .iter()
                .map(|n| (n.to_string(), u32::from(n == noun)))
                .collect();
            sum(
                &mut self.go_v,
                Runner::GoTestVerbose,
                counts,
                &["pass", "fail"],
            );
        } else if let Some(tally) = nextest(t)
            .or_else(|| gotestsum(t))
            .or_else(|| jest(t))
            .or_else(|| vitest(t))
            .or_else(|| pytest(t))
        {
            self.last.insert(tally.runner, tally);
        }
    }

    /// Every tally, in [`Runner`] order. A nextest summary makes the libtest
    /// lines it echoes from a failing test not a second runner, and a
    /// gotestsum summary the `--- PASS` lines it echoes.
    pub fn finish(self) -> Vec<Tally> {
        let mut out: Vec<Tally> = Vec::new();
        if !self.last.contains_key(&Runner::Nextest) {
            out.extend(self.cargo);
        }
        out.extend(self.last.values().cloned());
        if !self.last.contains_key(&Runner::Gotestsum) {
            out.extend(self.go_v);
        }
        out.sort_by_key(|t| t.runner);
        out
    }
}

/// Adds one summary line into a summed tally; `ran` is the sum of `ran_nouns`.
fn sum(into: &mut Option<Tally>, runner: Runner, counts: Vec<(String, u32)>, ran_nouns: &[&str]) {
    let t = into.get_or_insert_with(|| Tally {
        runner,
        counts: Vec::new(),
        ran: 0,
        lines: 0,
    });
    for (noun, n) in counts {
        match t.counts.iter_mut().find(|(k, _)| *k == noun) {
            Some((_, c)) => *c = c.saturating_add(n),
            None => t.counts.push((noun, n)),
        }
    }
    t.lines += 1;
    t.ran = ran_nouns
        .iter()
        .map(|n| t.count(n))
        .fold(0u32, u32::saturating_add);
}

/// `ESC [ … final` and two-byte escapes removed.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                // An operating-system command, to BEL or ST.
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// `12` as a count, refusing anything past `u32`.
fn num(s: &str) -> Option<u32> {
    let n: u64 = s.trim().parse().ok()?;
    u32::try_from(n).ok()
}

/// `3 passed` as `("passed", 3)`.
fn pair(s: &str) -> Option<(String, u32)> {
    let (n, noun) = s.trim().split_once(' ')?;
    let noun = noun.trim();
    if noun.is_empty() {
        return None;
    }
    Some((noun.to_string(), num(n)?))
}

/// `ok. 12 passed; 0 failed; 1 ignored; 0 measured; 3 filtered out; finished in 0.02s`
fn cargo(rest: &str) -> Option<Vec<(String, u32)>> {
    let rest = rest
        .strip_prefix("ok.")
        .or_else(|| rest.strip_prefix("FAILED."))?;
    let mut counts = Vec::new();
    for part in rest.split(';') {
        let part = part.trim();
        if part.starts_with("finished in") || part.is_empty() {
            continue;
        }
        let (noun, n) = pair(part)?;
        if !["passed", "failed", "ignored", "measured", "filtered out"].contains(&noun.as_str()) {
            return None;
        }
        counts.push((noun, n));
    }
    let has = |n: &str| counts.iter().any(|(k, _)| k == n);
    (has("passed") && has("failed")).then_some(counts)
}

/// `Summary [   0.015s] 12 tests run: 11 passed, 1 failed, 2 skipped`, or
/// `3/12 tests run: …` when cancelled.
fn nextest(t: &str) -> Option<Tally> {
    let rest = t.strip_prefix("Summary [")?;
    let (_, rest) = rest.split_once(']')?;
    let (head, tail) = rest.split_once(':')?;
    let head = head.trim();
    let first = head.split_whitespace().next()?;
    if !(head.ends_with("tests run") || head.ends_with("test run")) {
        return None;
    }
    let run = num(first.split('/').next()?)?;
    let mut counts = vec![("run".to_string(), run)];
    for part in tail.split(',') {
        // `7 passed (1 slow)`: the noun, not the aside.
        let part = part.split(" (").next().unwrap_or(part);
        let (noun, n) = pair(part)?;
        counts.push((noun, n));
    }
    Some(Tally {
        runner: Runner::Nextest,
        counts,
        ran: run,
        lines: 1,
    })
}

/// `DONE 12 tests, 1 skipped, 2 failures in 1.2s`
fn gotestsum(t: &str) -> Option<Tally> {
    let rest = t.strip_prefix("DONE ")?;
    let (rest, _) = rest.rsplit_once(" in ")?;
    let mut counts = Vec::new();
    for part in rest.split(',') {
        let (noun, n) = pair(part)?;
        let noun = match noun.as_str() {
            "test" => "tests",
            "failure" => "failures",
            "error" => "errors",
            other => other,
        }
        .to_string();
        counts.push((noun, n));
    }
    if counts.first().map(|(n, _)| n.as_str()) != Some("tests") {
        return None;
    }
    let mut tally = Tally {
        runner: Runner::Gotestsum,
        counts,
        ran: 0,
        lines: 1,
    };
    tally.ran = tally.count("tests").saturating_sub(tally.count("skipped"));
    Some(tally)
}

/// `Tests:       1 failed, 2 skipped, 1 todo, 9 passed, 13 total`
fn jest(t: &str) -> Option<Tally> {
    let rest = t.strip_prefix("Tests:")?;
    let mut counts = Vec::new();
    for part in rest.split(',') {
        counts.push(pair(part)?);
    }
    let mut tally = Tally {
        runner: Runner::Jest,
        counts,
        ran: 0,
        lines: 1,
    };
    if !tally.counts.iter().any(|(n, _)| n == "total") {
        return None;
    }
    tally.ran = tally
        .count("total")
        .saturating_sub(tally.count("skipped"))
        .saturating_sub(tally.count("todo"));
    Some(tally)
}

/// `Tests  1 failed | 9 passed | 2 skipped | 1 todo (13)`
fn vitest(t: &str) -> Option<Tally> {
    let rest = t.strip_prefix("Tests ")?.trim();
    let (parts, total) = rest.rsplit_once('(')?;
    let total = num(total.strip_suffix(')')?)?;
    let mut counts = Vec::new();
    for part in parts.split('|') {
        counts.push(pair(part)?);
    }
    counts.push(("total".into(), total));
    let mut tally = Tally {
        runner: Runner::Vitest,
        counts,
        ran: 0,
        lines: 1,
    };
    tally.ran = total
        .saturating_sub(tally.count("skipped"))
        .saturating_sub(tally.count("todo"));
    Some(tally)
}

/// Nouns pytest prints in its summary.
const PYTEST_NOUNS: &[&str] = &[
    "passed",
    "failed",
    "error",
    "errors",
    "skipped",
    "xfailed",
    "xpassed",
    "deselected",
    "warning",
    "warnings",
    "rerun",
];

/// `===== 3 passed, 9 deselected in 0.05s =====`, the same without the rule
/// (`-q`), and `no tests ran in 0.01s`.
fn pytest(t: &str) -> Option<Tally> {
    let t = t.trim_matches(|c| c == '=' || c == ' ');
    let (head, timing) = t.rsplit_once(" in ")?;
    if !timing.starts_with(|c: char| c.is_ascii_digit()) || !timing.contains('s') {
        return None;
    }
    let mut tally = Tally {
        runner: Runner::Pytest,
        counts: Vec::new(),
        ran: 0,
        lines: 1,
    };
    if head == "no tests ran" {
        return Some(tally);
    }
    for part in head.split(',') {
        let (noun, n) = pair(part)?;
        if !PYTEST_NOUNS.contains(&noun.as_str()) {
            return None;
        }
        match noun.as_str() {
            "warning" | "warnings" | "rerun" => {}
            "error" => tally.counts.push(("errors".into(), n)),
            _ => tally.counts.push((noun, n)),
        }
    }
    tally.ran = ["passed", "failed", "xfailed", "xpassed"]
        .iter()
        .map(|n| tally.count(n))
        .fold(0u32, u32::saturating_add);
    Some(tally)
}

#[cfg(test)]
mod tests {
    use super::super::Scanner;
    use super::*;

    /// Every recorded output, with what it reads to.
    const RECORDED: &[(&str, &str, &str)] = &[
        (
            "cargo/two_binaries_and_doctests",
            include_str!("../../../tests/fixtures/runners/cargo/two_binaries_and_doctests.txt"),
            include_str!(
                "../../../tests/fixtures/runners/cargo/two_binaries_and_doctests.expected"
            ),
        ),
        (
            "cargo/one_failing",
            include_str!("../../../tests/fixtures/runners/cargo/one_failing.txt"),
            include_str!("../../../tests/fixtures/runners/cargo/one_failing.expected"),
        ),
        (
            "cargo/skip",
            include_str!("../../../tests/fixtures/runners/cargo/skip.txt"),
            include_str!("../../../tests/fixtures/runners/cargo/skip.expected"),
        ),
        (
            "nextest/pass",
            include_str!("../../../tests/fixtures/runners/nextest/pass.txt"),
            include_str!("../../../tests/fixtures/runners/nextest/pass.expected"),
        ),
        (
            "nextest/fail",
            include_str!("../../../tests/fixtures/runners/nextest/fail.txt"),
            include_str!("../../../tests/fixtures/runners/nextest/fail.expected"),
        ),
        (
            "nextest/skipped",
            include_str!("../../../tests/fixtures/runners/nextest/skipped.txt"),
            include_str!("../../../tests/fixtures/runners/nextest/skipped.expected"),
        ),
        (
            "nextest/cancelled",
            include_str!("../../../tests/fixtures/runners/nextest/cancelled.txt"),
            include_str!("../../../tests/fixtures/runners/nextest/cancelled.expected"),
        ),
        (
            "pytest/pass",
            include_str!("../../../tests/fixtures/runners/pytest/pass.txt"),
            include_str!("../../../tests/fixtures/runners/pytest/pass.expected"),
        ),
        (
            "pytest/deselected",
            include_str!("../../../tests/fixtures/runners/pytest/deselected.txt"),
            include_str!("../../../tests/fixtures/runners/pytest/deselected.expected"),
        ),
        (
            "pytest/failed_and_error",
            include_str!("../../../tests/fixtures/runners/pytest/failed_and_error.txt"),
            include_str!("../../../tests/fixtures/runners/pytest/failed_and_error.expected"),
        ),
        (
            "pytest/no_tests_ran",
            include_str!("../../../tests/fixtures/runners/pytest/no_tests_ran.txt"),
            include_str!("../../../tests/fixtures/runners/pytest/no_tests_ran.expected"),
        ),
        (
            "pytest/quiet",
            include_str!("../../../tests/fixtures/runners/pytest/quiet.txt"),
            include_str!("../../../tests/fixtures/runners/pytest/quiet.expected"),
        ),
        (
            "jest/pass",
            include_str!("../../../tests/fixtures/runners/jest/pass.txt"),
            include_str!("../../../tests/fixtures/runners/jest/pass.expected"),
        ),
        (
            "jest/failed_skipped_todo",
            include_str!("../../../tests/fixtures/runners/jest/failed_skipped_todo.txt"),
            include_str!("../../../tests/fixtures/runners/jest/failed_skipped_todo.expected"),
        ),
        (
            "vitest/pass",
            include_str!("../../../tests/fixtures/runners/vitest/pass.txt"),
            include_str!("../../../tests/fixtures/runners/vitest/pass.expected"),
        ),
        (
            "vitest/failed_skipped_todo",
            include_str!("../../../tests/fixtures/runners/vitest/failed_skipped_todo.txt"),
            include_str!("../../../tests/fixtures/runners/vitest/failed_skipped_todo.expected"),
        ),
        (
            "gotestsum/pass",
            include_str!("../../../tests/fixtures/runners/gotestsum/pass.txt"),
            include_str!("../../../tests/fixtures/runners/gotestsum/pass.expected"),
        ),
        (
            "gotestsum/fail",
            include_str!("../../../tests/fixtures/runners/gotestsum/fail.txt"),
            include_str!("../../../tests/fixtures/runners/gotestsum/fail.expected"),
        ),
        (
            "go/verbose_subtests_skip",
            include_str!("../../../tests/fixtures/runners/go/verbose_subtests_skip.txt"),
            include_str!("../../../tests/fixtures/runners/go/verbose_subtests_skip.expected"),
        ),
        (
            "go/verbose_fail",
            include_str!("../../../tests/fixtures/runners/go/verbose_fail.txt"),
            include_str!("../../../tests/fixtures/runners/go/verbose_fail.expected"),
        ),
        (
            "go/without_v",
            include_str!("../../../tests/fixtures/runners/go/without_v.txt"),
            include_str!("../../../tests/fixtures/runners/go/without_v.expected"),
        ),
    ];

    fn scan(text: &str) -> Vec<Tally> {
        let mut s = Scanner::new();
        // In uneven pieces, as a pipe delivers them.
        for chunk in text.as_bytes().chunks(37) {
            s.feed(chunk);
        }
        s.finish()
    }

    #[test]
    fn every_recorded_output_reads_to_what_the_runner_printed() {
        for (name, output, expected) in RECORDED {
            let got = scan(output);
            let want: Vec<(&str, &str)> = expected
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.rsplit_once(' ').unwrap_or((l, "")))
                .collect();
            if want == [("none", "")] {
                assert!(got.is_empty(), "{name}: read {got:?} from no summary");
                continue;
            }
            assert_eq!(got.len(), 1, "{name}: {got:?}");
            let t = &got[0];
            for (key, value) in want {
                match key {
                    "runner" => assert_eq!(
                        t.runner.name().replace(" -v", "_verbose").replace(' ', "_"),
                        value,
                        "{name}"
                    ),
                    "ran" => assert_eq!(t.ran.to_string(), value, "{name}: ran"),
                    "lines" => assert_eq!(t.lines.to_string(), value, "{name}: lines"),
                    noun => assert_eq!(t.count(noun).to_string(), value, "{name}: {noun}"),
                }
            }
        }
    }

    #[test]
    fn no_summary_is_never_ran_zero_and_no_tests_ran_is() {
        assert!(scan("compiling\nall good\n").is_empty());
        let none = scan(include_str!(
            "../../../tests/fixtures/runners/pytest/no_tests_ran.txt"
        ));
        assert_eq!(none.len(), 1);
        assert_eq!(none[0].ran, 0);
        assert_eq!(none[0].says(), "pytest ran 0");
    }

    #[test]
    fn colour_is_stripped_before_matching() {
        assert_eq!(strip_ansi("\u{1b}[32;1mok\u{1b}[0m"), "ok");
        assert_eq!(strip_ansi("a\u{1b}]8;;x\u{7}b"), "ab");
    }

    #[test]
    fn prose_that_looks_like_a_summary_is_not_one() {
        for line in [
            "Tests: none today",
            "DONE with it in 3s",
            "we passed the check in 2s",
            "Summary [ok] nothing",
            "test result: maybe",
        ] {
            assert!(scan(line).is_empty(), "{line}");
        }
    }
}

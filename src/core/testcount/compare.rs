//! The counts line, composed once: how many ran on this change, beside how
//! many the base's tests ran on it. Two numbers with an arrow, runner by
//! runner, never a ratio, a percentage or a sum across runners.

use super::{CommandCount, Tally};
use crate::core::change::CommandResult;

/// Nouns that restate *ran* and are never compared on their own.
const RESTATES: &[&str] = &["run", "total", "tests"];

/// Every tally the commands read, in command order.
pub fn tallies(commands: &[CommandResult]) -> Vec<&Tally> {
    commands.iter().flat_map(|c| c.tests.tallies()).collect()
}

/// Why no count was read from these commands, naming the runner when one
/// was named: never a zero.
fn not_read(commands: &[CommandResult]) -> String {
    commands
        .iter()
        .find_map(|c| match &c.tests {
            CommandCount::NotRecognised { says, .. } => Some(says.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "no test runner's summary was read".into())
}

/// The left side of the header's line: *ran 3*, or *ran 12 → 3 · deselected
/// 0 → 9 · pytest* when `base` (the base's tests on this change) is given.
/// With more than one runner, each is named.
pub fn says(check: &[CommandResult], base: Option<&[CommandResult]>) -> String {
    let here = tallies(check);
    if here.is_empty() {
        return not_read(check);
    }
    let there: Vec<&Tally> = base.map(tallies).unwrap_or_default();
    let several = here.len() > 1;
    here.iter()
        .map(|t| match there.iter().find(|b| b.runner == t.runner) {
            Some(b) => arrow(b, t),
            None if several => format!("{} ran {}", t.runner.name(), t.ran),
            None => format!("ran {}", t.ran),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// *ran 12 → 3 · deselected 0 → 9 · pytest*: only the nouns whose counts
/// differ follow *ran*.
fn arrow(base: &Tally, here: &Tally) -> String {
    let mut out = format!("ran {} → {}", base.ran, here.ran);
    let mut nouns: Vec<&str> = Vec::new();
    for (n, _) in base.counts.iter().chain(here.counts.iter()) {
        if !nouns.contains(&n.as_str()) && !RESTATES.contains(&n.as_str()) {
            nouns.push(n);
        }
    }
    for n in nouns {
        let (b, h) = (base.count(n), here.count(n));
        if b != h {
            out.push_str(&format!(" · {n} {b} → {h}"));
        }
    }
    out.push_str(&format!(" · {}", here.runner.name()));
    out
}

/// The certificate's one sentence about counts. `base` is the base's tests on
/// this change with the base commit they ran at.
pub fn certificate_sentence(
    check: &[CommandResult],
    base: Option<(&[CommandResult], &str)>,
) -> String {
    let here = tallies(check);
    if here.is_empty() {
        return "No test runner's summary was recognised in this run's output, so how many tests \
                ran is not known."
            .into();
    }
    let parts: Vec<String> = here
        .iter()
        .map(|t| {
            let other = base.and_then(|(cmds, _)| {
                tallies(cmds)
                    .into_iter()
                    .find(|b| b.runner == t.runner)
                    .cloned()
            });
            match (other, base) {
                (Some(b), Some((_, at))) => {
                    let mut s = format!(
                        "{} ran {} on this change against {} for the base's tests on it (base \
                         commit `{}`, the same `check`)",
                        t.runner.name(),
                        t.ran,
                        b.ran,
                        crate::core::text::clip(at, 12)
                    );
                    for (n, h) in &t.counts {
                        let was = b.count(n);
                        if RESTATES.contains(&n.as_str()) || was == *h {
                            continue;
                        }
                        s.push_str(&format!("; {n} {h} against {was}"));
                    }
                    s
                }
                _ => format!("{} ran {} on this change", t.runner.name(), t.ran),
            }
        })
        .collect();
    format!("The test runner printed: {}.", parts.join(" · "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::testcount::Runner;

    fn cmd(tallies: Vec<Tally>) -> CommandResult {
        let mut c = CommandResult::exited("sh ./scripts/test.sh", 0);
        c.tests = CommandCount::Read { tallies };
        c
    }

    fn py(counts: &[(&str, u32)], ran: u32) -> Tally {
        Tally {
            runner: Runner::Pytest,
            counts: counts.iter().map(|(n, c)| (n.to_string(), *c)).collect(),
            ran,
            lines: 1,
        }
    }

    #[test]
    fn one_runner_alone_is_its_count() {
        let c = [cmd(vec![py(&[("passed", 3), ("deselected", 9)], 3)])];
        assert_eq!(says(&c, None), "ran 3");
    }

    #[test]
    fn the_arrow_reads_base_then_change_with_only_the_nouns_that_moved() {
        let here = [cmd(vec![py(&[("passed", 3), ("deselected", 9)], 3)])];
        let base = [cmd(vec![py(&[("passed", 11), ("failed", 1)], 12)])];
        assert_eq!(
            says(&here, Some(&base)),
            "ran 12 → 3 · passed 11 → 3 · failed 1 → 0 · deselected 0 → 9 · pytest"
        );
    }

    #[test]
    fn two_runners_are_each_named_and_never_summed() {
        let jest = Tally {
            runner: Runner::Jest,
            counts: vec![("passed".into(), 5), ("total".into(), 5)],
            ran: 5,
            lines: 1,
        };
        let here = [cmd(vec![py(&[("passed", 3)], 3), jest.clone()])];
        let s = says(&here, None);
        assert_eq!(s, "pytest ran 3 · jest ran 5");
        let base = [cmd(vec![py(&[("passed", 3)], 3)])];
        assert_eq!(says(&here, Some(&base)), "ran 3 → 3 · pytest · jest ran 5");
    }

    #[test]
    fn a_side_not_read_is_said_and_never_zero() {
        let mut c = CommandResult::exited("pytest -q", 0);
        c.tests = CommandCount::NotRecognised {
            runner: Runner::Pytest,
            says: "count not read — pytest summary not recognised".into(),
        };
        assert_eq!(
            says(std::slice::from_ref(&c), None),
            "count not read — pytest summary not recognised"
        );
        let base = [cmd(vec![py(&[("passed", 3)], 3)])];
        assert!(!says(&[c], Some(&base)).contains('0'));
        let lint = [CommandResult::exited("cargo clippy", 0)];
        assert_eq!(says(&lint, None), "no test runner's summary was read");
    }

    #[test]
    fn differing_command_lists_still_compare_by_runner() {
        let here = [
            CommandResult::exited("lint", 0),
            cmd(vec![py(&[("passed", 3)], 3)]),
        ];
        let base = [cmd(vec![py(&[("passed", 12)], 12)])];
        assert_eq!(
            says(&here, Some(&base)),
            "ran 12 → 3 · passed 12 → 3 · pytest"
        );
    }

    #[test]
    fn the_certificate_says_both_numbers_or_why_there_are_none() {
        let here = [cmd(vec![py(&[("passed", 3), ("deselected", 9)], 3)])];
        let base = [cmd(vec![py(&[("passed", 12)], 12)])];
        let s = certificate_sentence(&here, Some((&base, "5940cb87c7f9aa")));
        assert!(s.contains("pytest ran 3 on this change against 12"), "{s}");
        assert!(s.contains("deselected 9 against 0"), "{s}");
        let none = certificate_sentence(&[CommandResult::exited("x", 0)], None);
        assert!(none.contains("not known"));
        assert!(!none.contains(" 0 "));
    }
}

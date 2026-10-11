//! How many tests ran, as the runner itself printed it. A pure line scanner is
//! fed every byte a gate command writes, one scanner per pipe, so a summary
//! that scrolled out of the kept tail is still read. Each runner keeps its own
//! nouns; the one word added is *ran*, defined per runner. Counts from two
//! runners are never added together, and a summary that was not read is never
//! a zero.

pub mod compare;
pub mod junit;
pub mod runners;

use serde::{Deserialize, Serialize};

/// A runner whose printed summary is read.
/// Serialised as the name every surface prints, so a surface never maps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Runner {
    #[serde(rename = "cargo test")]
    CargoTest,
    #[serde(rename = "nextest")]
    Nextest,
    #[serde(rename = "pytest")]
    Pytest,
    #[serde(rename = "jest")]
    Jest,
    #[serde(rename = "vitest")]
    Vitest,
    #[serde(rename = "gotestsum")]
    Gotestsum,
    #[serde(rename = "go test -v")]
    GoTestVerbose,
}

impl Runner {
    /// The name every surface prints.
    pub fn name(self) -> &'static str {
        match self {
            Runner::CargoTest => "cargo test",
            Runner::Nextest => "nextest",
            Runner::Pytest => "pytest",
            Runner::Jest => "jest",
            Runner::Vitest => "vitest",
            Runner::Gotestsum => "gotestsum",
            Runner::GoTestVerbose => "go test -v",
        }
    }

    /// The runner a command names, for the *not recognised* sentence only:
    /// the output's shape decides recognition, so `just check` running cargo
    /// is still read.
    pub fn named_by(command: &str) -> Option<Runner> {
        let c = command;
        if c.contains("nextest") {
            Some(Runner::Nextest)
        } else if c.contains("cargo test") {
            Some(Runner::CargoTest)
        } else if c.contains("pytest") || c.contains("py.test") {
            Some(Runner::Pytest)
        } else if c.contains("vitest") {
            Some(Runner::Vitest)
        } else if c.contains("jest") {
            Some(Runner::Jest)
        } else if c.contains("gotestsum") {
            Some(Runner::Gotestsum)
        } else if c.contains("go test") {
            Some(Runner::GoTestVerbose)
        } else {
            None
        }
    }
}

/// One runner's summary from one command: its nouns in its order, and *ran*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub runner: Runner,
    /// The runner's nouns and counts, as it printed them: `[("passed", 3)]`.
    pub counts: Vec<(String, u32)>,
    /// How many ran, in this runner's own terms (research-pinned per runner).
    pub ran: u32,
    /// Summary lines read; cargo and `go test -v` print one per binary or test.
    pub lines: u32,
}

impl Tally {
    /// The count under one noun; an omitted noun is zero, since these runners
    /// omit zero counts.
    pub fn count(&self, noun: &str) -> u32 {
        self.counts
            .iter()
            .find(|(n, _)| n == noun)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    }

    /// Failures the runner counted: failed, errors, failures.
    pub fn failed(&self) -> u32 {
        ["failed", "errors", "failures", "fail"]
            .iter()
            .map(|n| self.count(n))
            .sum()
    }

    /// *pytest ran 12 · failed 1 · passed 11*: every noun it printed that is
    /// not zero and not a restatement of *ran*.
    pub fn says(&self) -> String {
        let mut out = format!("{} ran {}", self.runner.name(), self.ran);
        for (noun, n) in &self.counts {
            if *n == 0 || matches!(noun.as_str(), "run" | "total" | "tests") {
                continue;
            }
            out.push_str(&format!(" · {noun} {n}"));
        }
        out
    }
}

/// How many tests one command ran, or why that is not known. Never renders as
/// zero unless a runner printed zero.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "count", rename_all = "snake_case")]
pub enum CommandCount {
    /// One tally per runner recognised; never empty.
    Read { tallies: Vec<Tally> },
    /// The command names a runner and no summary of it was read.
    NotRecognised { runner: Runner, says: String },
    /// No runner named and no summary printed: a linter, a build.
    #[default]
    NoRunner,
    /// The command did not reach an exit code; a partial summary is not a count.
    NoExit,
}

impl CommandCount {
    /// The count for a command that ended with `outcome`, from what each pipe's
    /// scanner read. The two pipes are never summed: where both carry one
    /// runner, the one with more summary lines stands.
    pub fn of(
        command: &str,
        outcome: &crate::core::change::Outcome,
        stdout: Vec<Tally>,
        stderr: Vec<Tally>,
    ) -> Self {
        if !matches!(outcome, crate::core::change::Outcome::Exited { .. }) {
            return CommandCount::NoExit;
        }
        let mut merged: Vec<Tally> = stdout;
        for t in stderr {
            match merged.iter_mut().find(|m| m.runner == t.runner) {
                Some(m) if t.lines > m.lines => *m = t,
                Some(_) => {}
                None => merged.push(t),
            }
        }
        merged.sort_by_key(|t| t.runner);
        if !merged.is_empty() {
            return CommandCount::Read { tallies: merged };
        }
        match Runner::named_by(command) {
            Some(Runner::GoTestVerbose) if !command.contains("-v") => CommandCount::NotRecognised {
                runner: Runner::GoTestVerbose,
                says: "go test prints no count without -v; gotestsum or -v does".into(),
            },
            Some(runner) => CommandCount::NotRecognised {
                runner,
                says: format!("count not read — {} summary not recognised", runner.name()),
            },
            None => CommandCount::NoRunner,
        }
    }

    /// The tallies read, empty when none were.
    pub fn tallies(&self) -> &[Tally] {
        match self {
            CommandCount::Read { tallies } => tallies,
            _ => &[],
        }
    }

    /// One sentence for this command's count.
    pub fn says(&self) -> String {
        match self {
            CommandCount::Read { tallies } => tallies
                .iter()
                .map(Tally::says)
                .collect::<Vec<_>>()
                .join(" · "),
            CommandCount::NotRecognised { says, .. } => says.clone(),
            CommandCount::NoRunner => "no test runner's summary in this output".into(),
            CommandCount::NoExit => "did not finish, so no count was read".into(),
        }
    }
}

/// The longest line a summary can be; a longer one is dropped whole.
pub const MAX_LINE: usize = 64 * 1024;

/// Reads one pipe line by line as bytes arrive, keeping only a partial line
/// and the tallies so far.
#[derive(Debug, Default)]
pub struct Scanner {
    partial: Vec<u8>,
    /// The current line passed [`MAX_LINE`]; skipped to its end.
    overlong: bool,
    acc: runners::Acc,
}

impl Scanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the next bytes of the pipe, in order.
    pub fn feed(&mut self, bytes: &[u8]) {
        let mut rest = bytes;
        while let Some(at) = rest.iter().position(|b| *b == b'\n') {
            self.push(&rest[..at]);
            self.end_line();
            rest = &rest[at + 1..];
        }
        self.push(rest);
    }

    fn push(&mut self, bytes: &[u8]) {
        if self.overlong {
            return;
        }
        if self.partial.len() + bytes.len() > MAX_LINE {
            self.partial.clear();
            self.overlong = true;
            return;
        }
        self.partial.extend_from_slice(bytes);
    }

    fn end_line(&mut self) {
        if !self.overlong {
            let line = String::from_utf8_lossy(&self.partial);
            self.acc.line(&line);
        }
        self.partial.clear();
        self.overlong = false;
    }

    /// The pipe closed: every runner's tally, in a fixed order.
    pub fn finish(mut self) -> Vec<Tally> {
        if !self.partial.is_empty() || self.overlong {
            self.end_line();
        }
        self.acc.finish()
    }
}

/// What `check` means for base checks, as one digest: its commands, the gate
/// timeout, the setup command, the shared caches and the declared JUnit paths.
/// A result for another definition is stale.
pub fn definition_digest(
    check: &[String],
    timeout_secs: u64,
    setup: Option<&str>,
    share: &[String],
    junit: &[String],
) -> String {
    let mut share = share.to_vec();
    share.sort();
    let mut junit = junit.to_vec();
    junit.sort();
    let mut h = crate::core::hash::Rolling::new();
    for c in check {
        h.push_str("check\0");
        h.push_str(c);
    }
    h.push_str(&format!("\0timeout\0{timeout_secs}"));
    h.push_str("\0setup\0");
    h.push_str(setup.unwrap_or(""));
    for s in &share {
        h.push_str("\0share\0");
        h.push_str(s);
    }
    for j in &junit {
        h.push_str("\0junit\0");
        h.push_str(j);
    }
    h.hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Vec<Tally> {
        let mut s = Scanner::new();
        s.feed(text.as_bytes());
        s.finish()
    }

    #[test]
    fn a_summary_split_across_two_feeds_reads_once() {
        let mut s = Scanner::new();
        s.feed(b"== 3 passed, 9 dese");
        s.feed(b"lected in 0.05s ==\n");
        let t = s.finish();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].ran, 3);
        assert_eq!(t[0].count("deselected"), 9);
    }

    #[test]
    fn two_runners_are_two_tallies_never_one_sum() {
        let t = read(
            "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n\
             ===== 3 passed in 0.05s =====\n",
        );
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].runner, Runner::CargoTest);
        assert_eq!(t[0].ran, 4);
        assert_eq!(t[1].runner, Runner::Pytest);
        assert_eq!(t[1].ran, 3);
    }

    #[test]
    fn a_line_over_the_bound_is_dropped_without_a_panic() {
        let mut s = Scanner::new();
        let long = vec![b'x'; MAX_LINE + 10];
        s.feed(&long);
        s.feed(b" 3 passed in 0.1s\n== 2 passed in 0.1s ==\n");
        let t = s.finish();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].ran, 2, "the overlong line was skipped, the next read");
    }

    #[test]
    fn a_count_past_u32_is_not_read() {
        assert!(read("== 4294967296 passed in 0.1s ==\n").is_empty());
        assert_eq!(read("== 4294967295 passed in 0.1s ==\n")[0].ran, u32::MAX);
    }

    #[test]
    fn a_command_that_did_not_exit_carries_no_count() {
        let t = read("== 3 passed in 0.1s ==\n");
        let c = CommandCount::of(
            "pytest",
            &crate::core::change::Outcome::TimedOut { after_secs: 5 },
            t,
            vec![],
        );
        assert_eq!(c, CommandCount::NoExit);
    }

    #[test]
    fn a_named_runner_without_a_summary_is_not_read_never_zero() {
        let c = CommandCount::of(
            "sh -c 'pytest -q'",
            &crate::core::change::Outcome::Exited { code: 0 },
            vec![],
            vec![],
        );
        assert_eq!(c.says(), "count not read — pytest summary not recognised");
        assert!(!c.says().contains(" 0"));
        let lint = CommandCount::of(
            "cargo clippy",
            &crate::core::change::Outcome::Exited { code: 0 },
            vec![],
            vec![],
        );
        assert_eq!(lint, CommandCount::NoRunner);
        let go = CommandCount::of(
            "go test ./...",
            &crate::core::change::Outcome::Exited { code: 0 },
            vec![],
            vec![],
        );
        assert!(go.says().contains("without -v"), "{}", go.says());
    }

    #[test]
    fn the_pipe_with_more_summary_lines_stands() {
        let one = read("test result: ok. 1 passed; 0 failed; finished in 0s\n");
        let two = read(
            "test result: ok. 1 passed; 0 failed; finished in 0s\n\
             test result: ok. 2 passed; 0 failed; finished in 0s\n",
        );
        let c = CommandCount::of(
            "cargo test",
            &crate::core::change::Outcome::Exited { code: 0 },
            one,
            two,
        );
        assert_eq!(c.tallies()[0].ran, 3);
    }

    #[test]
    fn the_definition_digest_moves_with_each_input() {
        let base = definition_digest(&["a".into()], 60, None, &[], &[]);
        assert_eq!(base, definition_digest(&["a".into()], 60, None, &[], &[]));
        assert_ne!(base, definition_digest(&["b".into()], 60, None, &[], &[]));
        assert_ne!(base, definition_digest(&["a".into()], 61, None, &[], &[]));
        assert_ne!(
            base,
            definition_digest(&["a".into()], 60, Some("npm ci"), &[], &[])
        );
        assert_ne!(
            base,
            definition_digest(&["a".into()], 60, None, &[], &["r.xml".into()])
        );
    }
}

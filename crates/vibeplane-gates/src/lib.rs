//! Verification gates: turning "the agent says it is done" into "the project's
//! own checks agree".
//!
//! This is the smallest idea in the product and the one that earns it. An agent
//! reporting success is a claim; the repository's test command is evidence. The
//! gate runs the commands the project committed, in the worktree the work
//! happened in, as a child of the daemon — never through the agent, which would
//! let the thing being checked choose the check.
//!
//! Output is bounded and failures are extracted where the runner is recognised,
//! because what goes back to the agent has to fit in the context window it
//! needs to do the fixing.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use vibeplane_domain::work::{CommandResult, GateReport};

/// How much of a command's output is kept. A failing suite can print megabytes;
/// the end is where the summary lives.
const OUTPUT_TAIL_BYTES: usize = 8 * 1024;

/// Runs a gate's commands in order, stopping at the first failure.
///
/// Stopping early is deliberate: if the type check fails, the test run that
/// follows will fail for the same reason, and reporting both makes the cause
/// harder to see, not easier.
pub async fn run(
    gate: &str,
    commands: &[String],
    dir: &Path,
    timeout: Duration,
    attempt: u32,
) -> GateReport {
    run_expecting(gate, commands, dir, timeout, attempt, false).await
}

/// The same, for a gate that is supposed to fail.
///
/// A reproduction is the one check whose success is a failure: if the command
/// that demonstrates a bug passes, the bug has not been demonstrated. Stopping
/// at the first failure is therefore wrong here — the failure is the result —
/// so every command runs.
pub async fn run_expecting(
    gate: &str,
    commands: &[String],
    dir: &Path,
    timeout: Duration,
    attempt: u32,
    expect_fail: bool,
) -> GateReport {
    let started = Instant::now();
    let deadline = Instant::now() + timeout;
    let mut results = Vec::new();

    for command in commands {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let result = run_one(command, dir, remaining).await;
        let passed = result.passed();
        results.push(result);
        if !passed && !expect_fail {
            break;
        }
    }

    GateReport {
        expect_fail,
        gate: gate.to_string(),
        at: jiff::Timestamp::now(),
        duration_ms: started.elapsed().as_millis() as u64,
        commands: results,
        attempt,
    }
}

async fn run_one(command: &str, dir: &Path, timeout: Duration) -> CommandResult {
    let started = Instant::now();

    if timeout.is_zero() {
        return CommandResult {
            command: command.to_string(),
            exit_code: None,
            duration_ms: 0,
            output_tail: "the gate ran out of time before this command started".into(),
            failures: Vec::new(),
            timed_out: true,
        };
    }

    // Through a shell, because the commands are written by a person in a TOML
    // file and they expect `&&`, pipes and their own `$PATH`.
    let child = tokio::process::Command::new(shell())
        .arg(shell_flag())
        .arg(command)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return CommandResult {
                command: command.to_string(),
                exit_code: None,
                duration_ms: started.elapsed().as_millis() as u64,
                output_tail: format!("could not start: {e}"),
                failures: Vec::new(),
                timed_out: false,
            };
        }
    };

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut buf = String::new();

    let collect = async {
        let mut a = String::new();
        let mut b = String::new();
        if let Some(o) = stdout.as_mut() {
            o.read_to_string(&mut a).await.ok();
        }
        if let Some(e) = stderr.as_mut() {
            e.read_to_string(&mut b).await.ok();
        }
        (a, b)
    };

    let status = tokio::time::timeout(timeout, async {
        let (a, b) = collect.await;
        buf.push_str(&a);
        buf.push_str(&b);
        child.wait().await
    })
    .await;

    let (exit_code, timed_out) = match status {
        Ok(Ok(s)) => (s.code(), false),
        Ok(Err(_)) => (None, false),
        Err(_) => {
            // Kill the process tree's leader; a gate that leaves a test runner
            // behind quietly eats the machine.
            let _ = child.kill().await;
            (None, true)
        }
    };

    CommandResult {
        command: command.to_string(),
        exit_code,
        duration_ms: started.elapsed().as_millis() as u64,
        failures: extract_failures(&buf),
        output_tail: tail(&buf, OUTPUT_TAIL_BYTES),
        timed_out,
    }
}

fn shell() -> &'static str {
    if cfg!(windows) { "cmd" } else { "sh" }
}
fn shell_flag() -> &'static str {
    if cfg!(windows) { "/C" } else { "-c" }
}

/// Keeps the last `n` bytes, on a character boundary.
fn tail(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let start = s.len() - n;
    let start = (start..s.len())
        .find(|i| s.is_char_boundary(*i))
        .unwrap_or(s.len());
    format!("…\n{}", &s[start..])
}

/// Pulls out the lines that name what failed.
///
/// Deliberately a handful of well-known shapes rather than a clever heuristic.
/// A wrong guess is worse than none: it sends the agent after the wrong line
/// and hides the real one. When nothing matches, the caller falls back to the
/// output tail and says so.
pub fn extract_failures(output: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in output.lines() {
        let t = line.trim();
        let interesting = t.starts_with("error[")
            || t.starts_with("error:")
            // tsc puts the file and position first: `src/x.ts(4,2): error TS2345: …`
            || t.contains(": error TS")
            || (t.starts_with("test ") && t.contains("FAILED"))
            || t.starts_with("FAIL ")
            || t.starts_with("✗ ")
            || t.starts_with("× ")
            || t.starts_with("FAILED ")
            || t.starts_with("AssertionError")
            || (t.starts_with("assert") && t.contains("failed"))
            || t.contains("panicked at");
        if interesting {
            out.push(t.to_string());
        }
        if out.len() >= 40 {
            break;
        }
    }
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> std::path::PathBuf {
        std::env::temp_dir()
    }

    #[tokio::test]
    async fn a_passing_gate_passes() {
        let report = run(
            "check",
            &["true".into(), "echo hello".into()],
            &dir(),
            Duration::from_secs(10),
            1,
        )
        .await;
        assert!(report.passed());
        assert_eq!(report.commands.len(), 2);
        assert!(report.summary().contains("passed"));
    }

    #[tokio::test]
    async fn an_empty_gate_is_not_a_pass() {
        // A Definition of Done with nothing in it proves nothing, and must
        // never read as success.
        let report = run("check", &[], &dir(), Duration::from_secs(10), 1).await;
        assert!(!report.passed());
    }

    #[tokio::test]
    async fn the_first_failure_stops_the_gate() {
        // The second command would fail for the same reason; reporting both
        // buries the cause.
        let report = run(
            "check",
            &["false".into(), "echo should-not-run".into()],
            &dir(),
            Duration::from_secs(10),
            1,
        )
        .await;
        assert!(!report.passed());
        assert_eq!(report.commands.len(), 1);
    }

    #[tokio::test]
    async fn output_reaches_the_report() {
        let report = run(
            "check",
            &["echo 'test auth::login ... FAILED'; exit 1".into()],
            &dir(),
            Duration::from_secs(10),
            1,
        )
        .await;
        assert!(!report.passed());
        assert!(
            report.commands[0]
                .failures
                .iter()
                .any(|f| f.contains("auth::login"))
        );
        assert!(report.feedback().contains("auth::login"));
    }

    #[tokio::test]
    async fn a_command_that_hangs_is_killed() {
        let report = run(
            "check",
            &["sleep 30".into()],
            &dir(),
            Duration::from_millis(300),
            1,
        )
        .await;
        assert!(report.commands[0].timed_out);
        assert!(!report.passed());
        assert!(report.summary().contains("timed out"));
    }

    #[tokio::test]
    async fn a_gate_that_runs_out_of_time_does_not_start_the_rest() {
        let report = run(
            "check",
            &["sleep 5".into(), "echo later".into()],
            &dir(),
            Duration::from_millis(200),
            1,
        )
        .await;
        assert_eq!(report.commands.len(), 1, "the gate stops at the timeout");
        assert!(report.commands[0].timed_out);
    }

    #[tokio::test]
    async fn a_command_that_cannot_start_is_reported_not_panicked() {
        let report = run(
            "check",
            &["definitely-not-a-command-xyz".into()],
            &dir(),
            Duration::from_secs(5),
            1,
        )
        .await;
        assert!(!report.passed());
    }

    #[test]
    fn failures_are_extracted_from_the_shapes_that_matter() {
        let cases = [
            ("error[E0308]: mismatched types", true),
            ("src/x.ts(4,2): error TS2345: no", true),
            ("test auth::login ... FAILED", true),
            ("FAIL src/auth.test.ts", true),
            ("thread 'x' panicked at src/lib.rs:4", true),
            ("   Compiling vibeplane v0.1.0", false),
            ("warning: unused variable", false),
        ];
        for (line, expected) in cases {
            assert_eq!(
                !extract_failures(line).is_empty(),
                expected,
                "misjudged: {line}"
            );
        }
    }

    #[test]
    fn an_unrecognised_runner_yields_nothing_rather_than_a_guess() {
        // Sending the agent after the wrong line is worse than sending the log.
        let out = "some bespoke build tool said things\nand then more things\n";
        assert!(extract_failures(out).is_empty());
    }

    #[test]
    fn the_tail_is_cut_on_a_character_boundary() {
        let s = "ä".repeat(10_000);
        let t = tail(&s, 1024);
        assert!(t.len() <= 1100);
        assert!(t.starts_with('…'));
    }
}

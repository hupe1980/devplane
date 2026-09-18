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

use crate::core::text::tail;
use crate::core::work::{CommandResult, GateReport};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

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
        // Out of time: record it rather than starting a process only to kill
        // it in the same breath. A reproduction gate runs *every* command, so
        // this is reachable — and spawning `pnpm test` with nothing left on the
        // clock costs a real process, can leave real side effects, and reports
        // a timeout for a command that was never given a chance to run.
        if remaining.is_zero() {
            results.push(CommandResult {
                command: command.clone(),
                exit_code: None,
                duration_ms: 0,
                output_tail: format!(
                    "not run: `{gate}` was already past its {}s timeout",
                    timeout.as_secs()
                ),
                failures: Vec::new(),
                timed_out: true,
            });
            continue;
        }
        let result = run_one(command, dir, remaining).await;
        let passed = result.passed();
        results.push(result);
        if !passed && !expect_fail {
            break;
        }
    }

    GateReport {
        expect_fail,
        // Stamped by the caller, which knows the work and its project root.
        // The gate runner is given commands and a directory and deliberately
        // knows nothing about what the work is answering.
        spec: None,
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
    let mut cmd = tokio::process::Command::new(shell());
    cmd.arg(shell_flag())
        .arg(command)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        // The shell leads its own process group, so a timeout can reach the
        // whole tree. `sh -c "cargo test"` execs cargo and killing the child is
        // enough; `sh -c "a && b"` does not, and a `vitest --watch` somebody
        // wrote by accident would otherwise survive the gate and eat the
        // machine — which is exactly what the timeout exists to prevent.
        cmd.process_group(0);
    }
    let child = cmd.spawn();

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

    let pid = child.id();

    // Both pipes are drained **at the same time**, into a buffer that outlives
    // the read. Two bugs, one shape.
    //
    // Reading stdout to EOF first and stderr afterwards deadlocks the moment a
    // command fills the 64 KiB stderr buffer while still writing to stdout —
    // which `cargo test` does on any real failure. The symptom was a gate that
    // always timed out, and the cause was invisible in the report.
    //
    // And the buffer is shared rather than owned by the future, because a
    // timeout drops that future: a gate that ran out of time reported "killed
    // after 600s" and not one line of what the command had printed, which is
    // exactly the failure where the output is the only clue. A hung test suite
    // names the test it hung in.
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let status = tokio::time::timeout(timeout, async {
        let (_, _) = tokio::join!(
            drain(child.stdout.take(), captured.clone()),
            drain(child.stderr.take(), captured.clone())
        );
        child.wait().await
    })
    .await;

    let (exit_code, timed_out) = match status {
        Ok(Ok(s)) => (s.code(), false),
        Ok(Err(_)) => (None, false),
        Err(_) => {
            // The whole group, not just the shell: a test runner left behind by
            // a timed-out gate quietly eats the machine.
            terminate_group(pid);
            let _ = child.kill().await;
            (None, true)
        }
    };
    // Decoded once, at the end. Decoding each chunk as it arrived would put a
    // replacement character wherever an 8 KiB read happened to land in the
    // middle of a multi-byte one — and the place that shows up is the failing
    // line handed back to the agent, which is the one string here that has to
    // be exact.
    let buf = captured
        .lock()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();

    CommandResult {
        command: command.to_string(),
        exit_code,
        duration_ms: started.elapsed().as_millis() as u64,
        failures: extract_failures(&buf),
        output_tail: match timed_out {
            // What it printed before it was killed, which is the only evidence
            // there is about where it got stuck.
            true => format!(
                "killed after {}s\n{}",
                timeout.as_secs(),
                tail(&buf, OUTPUT_TAIL_BYTES)
            ),
            false => tail(&buf, OUTPUT_TAIL_BYTES),
        },
        timed_out,
    }
}

/// Reads one pipe into the shared buffer until it closes.
///
/// Generic over the pipe, because stdout and stderr are different types and the
/// only alternative is the same twenty lines written twice — which is how the
/// two of them end up drifting apart, and the drift would be invisible.
///
/// Bounded as it arrives: a runaway command must not be able to fill memory
/// faster than the timeout can stop it. Bytes rather than text, so the bound
/// never cuts a character in half.
async fn drain<R>(pipe: Option<R>, into: std::sync::Arc<std::sync::Mutex<Vec<u8>>>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut pipe) = pipe else { return };
    let mut chunk = vec![0u8; 8 * 1024];
    while let Ok(n) = pipe.read(&mut chunk).await {
        if n == 0 {
            return;
        }
        let Ok(mut buf) = into.lock() else { return };
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 4 * OUTPUT_TAIL_BYTES {
            let keep = buf.len() - 2 * OUTPUT_TAIL_BYTES;
            buf.drain(..keep);
        }
    }
}

/// SIGKILLs a process group led by `pid`. A no-op where the platform has no
/// group semantics, and harmless when the group is already gone.
fn terminate_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        // SAFETY: `kill` with a negative pid signals the process group; the
        // group was created by `process_group(0)` above and contains only this
        // gate's children. ESRCH just means it has already exited.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = pid;
}

fn shell() -> &'static str {
    if cfg!(windows) { "cmd" } else { "sh" }
}
fn shell_flag() -> &'static str {
    if cfg!(windows) { "/C" } else { "-c" }
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
    async fn a_command_that_floods_both_pipes_still_finishes() {
        // Reading stdout to EOF before touching stderr deadlocks as soon as a
        // command fills the 64 KiB stderr buffer — which every real failing
        // test suite does. The gate then "timed out" and said nothing useful.
        let report = run(
            "check",
            // stderr *first*: the old reader waited on stdout to EOF, so the
            // child blocked on a full stderr buffer and neither side moved.
            &["yes stderr | head -c 400000 >&2; yes stdout | head -c 400000; echo DRAINED".into()],
            &dir(),
            Duration::from_secs(20),
            1,
        )
        .await;
        assert!(!report.commands[0].timed_out, "the gate deadlocked");
        assert!(report.passed());
        assert!(
            report.commands[0].output_tail.contains("DRAINED"),
            "the command ran to the end, so both pipes were drained"
        );
    }

    #[tokio::test]
    async fn both_streams_reach_the_report() {
        // Interleaved as they arrive, which is not the order they were written
        // in — a pipe is block-buffered and a terminal is not, so stderr
        // routinely lands first. Both being *there* is the property; claiming
        // an order would be claiming something the operating system does not
        // promise, and a test that asserts it passes until it does not.
        let report = run(
            "check",
            &["echo on-stdout; echo on-stderr >&2".into()],
            &dir(),
            Duration::from_secs(10),
            1,
        )
        .await;
        let out = &report.commands[0].output_tail;
        assert!(out.contains("on-stdout"), "{out}");
        assert!(out.contains("on-stderr"), "{out}");
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
    async fn a_gate_that_times_out_still_says_what_it_saw() {
        // The failure where the output matters most produced none of it: the
        // reading future was dropped with the timeout, so a suite that hung in
        // one test reported "killed after 600s" and nothing else.
        let report = run(
            "check",
            &["echo 'running test auth::login'; sleep 30".into()],
            &dir(),
            Duration::from_millis(400),
            1,
        )
        .await;
        assert!(report.commands[0].timed_out);
        assert!(
            report.commands[0].output_tail.contains("auth::login"),
            "the last thing it printed is the only clue there is: {}",
            report.commands[0].output_tail
        );
    }

    #[tokio::test]
    async fn output_that_is_not_ascii_survives_the_chunk_boundary() {
        // The pipe is read in 8 KiB chunks and a multi-byte character does not
        // care where those land. Decoding per chunk put a replacement character
        // in the middle of one — in the failing line handed back to the agent,
        // which is the one string here that has to be exact.
        let padding = "x".repeat(8 * 1024 - 1);
        let report = run(
            "check",
            &[format!("printf '{padding}äöü — café ✓\n'; exit 1")],
            &dir(),
            Duration::from_secs(10),
            1,
        )
        .await;
        let out = &report.commands[0].output_tail;
        assert!(
            out.contains("äöü — café ✓"),
            "mangled: {}",
            &out[out.len().saturating_sub(60)..]
        );
        assert!(!out.contains('\u{FFFD}'), "a character was split");
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
            ("   Compiling devplane v0.1.0", false),
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
}

//! Verification gates: the project's own committed checks, run in the
//! change's worktree as a child of Devplane — never through the agent, which
//! would let the thing being checked choose the check. Output is bounded and
//! failure lines are extracted where the runner is recognised, so feedback
//! fits the agent's context.

use crate::core::change::{CommandResult, CommitStamp, GateReport, Outcome};
use crate::core::text::tail;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;

/// How much of a command's output is kept; a failing suite's summary is at the end.
const OUTPUT_TAIL_BYTES: usize = 8 * 1024;

/// Runs a gate's commands in order, stopping at the first failure (a later
/// command would usually fail for the same reason and bury the cause).
///
/// `env` (e.g. a shared cache variable from `core::caches`) is set on each
/// command over the host's environment; empty in a person's own checkout.
pub async fn run(
    gate: &str,
    commands: &[String],
    dir: &Path,
    timeout: Duration,
    attempt: u32,
    env: &[(String, PathBuf)],
) -> GateReport {
    let started = Instant::now();
    let deadline = Instant::now() + timeout;
    let mut results = Vec::new();
    // Stamped before and after: a pass describes the tree only if it did not
    // move while the gate ran.
    let before = commit_stamp(dir).await;

    for command in commands {
        let remaining = deadline.saturating_duration_since(Instant::now());
        // Out of time: record it rather than spawning a process only to kill
        // it and report a timeout for a command that never ran.
        if remaining.is_zero() {
            results.push(CommandResult {
                command: command.clone(),
                outcome: Outcome::NeverStarted {
                    reason: format!(
                        "`{gate}` was already past its {}s timeout",
                        timeout.as_secs()
                    ),
                },
                duration_ms: 0,
                output_tail: String::new(),
                output_bytes: 0,
                output_digest: crate::core::hash::hex(b""),
                failures: Vec::new(),
            });
            continue;
        }
        let result = run_one(command, dir, remaining, env).await;
        let passed = result.passed();
        results.push(result);
        if !passed {
            break;
        }
    }

    let after = commit_stamp(dir).await;
    GateReport {
        // Stamped by the caller, which knows the change and its project root.
        spec: None,
        commit: settled(before, after),
        gate: gate.to_string(),
        at: jiff::Timestamp::now(),
        duration_ms: started.elapsed().as_millis() as u64,
        commands: results,
        attempt,
    }
}

/// The stamp a report keeps: the post-run one, with its digest only if the
/// pre-run digest matches. A command that writes into the tree (a formatter,
/// a generated file) or a concurrent edit leaves no digest, and a report
/// without one can never make a change verified.
fn settled(before: Option<CommitStamp>, after: Option<CommitStamp>) -> Option<CommitStamp> {
    let mut after = after?;
    let same = matches!(
        (before.as_ref().and_then(|b| b.tree.as_ref()), after.tree.as_ref()),
        (Some(a), Some(b)) if a == b
    );
    if !same {
        after.tree = None;
    }
    Some(after)
}

/// What the tree was when this gate ran; `None` outside a repository, which a
/// certificate states explicitly.
async fn commit_stamp(dir: &Path) -> Option<CommitStamp> {
    crate::git::commit_stamp(dir).await
}

/// The report for a gate whose worktree is gone (removed by hand).
///
/// Every command is recorded as never started, with the reason, and no commit
/// is stamped. Not a failure of the work, and never a pass.
pub fn absent_worktree(gate: &str, commands: &[String], dir: &Path, attempt: u32) -> GateReport {
    let reason = format!("the worktree {} is gone, so nothing ran", dir.display());
    GateReport {
        gate: gate.to_string(),
        at: jiff::Timestamp::now(),
        duration_ms: 0,
        commands: commands
            .iter()
            .map(|c| {
                CommandResult::without_verdict(
                    c,
                    Outcome::NeverStarted {
                        reason: reason.clone(),
                    },
                )
            })
            .collect(),
        attempt,
        spec: None,
        commit: None,
    }
}

async fn run_one(
    command: &str,
    dir: &Path,
    timeout: Duration,
    env: &[(String, PathBuf)],
) -> CommandResult {
    let started = Instant::now();

    if timeout.is_zero() {
        // Use the constructor so every no-verdict result has identical empty
        // values, digest included.
        return CommandResult::without_verdict(
            command,
            Outcome::NeverStarted {
                reason: "the gate ran out of time before this command started".into(),
            },
        );
    }

    // Through a shell: commands come from a TOML file and expect `&&`, pipes
    // and the person's `$PATH`.
    let mut cmd = tokio::process::Command::new(shell());
    cmd.arg(shell_flag())
        .arg(command)
        .current_dir(dir)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_os_str())))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        // Lead a process group so a timeout kills the whole tree: `sh -c "a && b"`
        // does not exec, and an accidental `--watch` would otherwise survive.
        cmd.process_group(0);
    }
    let child = cmd.spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            // A structured outcome, not prose: a missing binary is a broken
            // gate, not a broken change.
            return CommandResult::without_verdict(
                command,
                Outcome::NeverStarted {
                    reason: e.to_string(),
                },
            );
        }
    };

    let pid = child.id();
    // The group dies with this call however it ends: normally, on a timeout,
    // or because the caller stopped waiting and dropped the future. Without
    // it a `cmd &` in a check, or a whole suite whose gate was cancelled,
    // outlives the gate unowned.
    let mut group = Group(pid);

    // Both pipes are drained concurrently into a buffer that outlives the
    // read. Sequential reads deadlock once a command fills the 64 KiB stderr
    // buffer while still writing stdout. The buffer is shared rather than owned
    // by the future because a timeout drops the future, and a hung suite's
    // output is exactly what names the test it hung in.
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Captured::default()));
    let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
    let drains = async {
        tokio::join!(
            drain(stdout, captured.clone()),
            drain(stderr, captured.clone())
        )
    };
    tokio::pin!(drains);
    let mut drained = false;
    // The verdict is the shell's exit, not the pipes closing: a background
    // child still holding stdout must not turn a finished check into a
    // timeout.
    let status = tokio::time::timeout(timeout, async {
        loop {
            tokio::select! {
                s = child.wait() => break s,
                _ = &mut drains, if !drained => drained = true,
            }
        }
    })
    .await;
    // Whatever the leader left running goes now; that closes their ends of
    // the pipes, so what they already wrote is read to the end.
    group.kill();
    if !drained {
        let _ = tokio::time::timeout(Duration::from_secs(2), &mut drains).await;
    }

    let outcome = match status {
        Ok(Ok(s)) => match s.code() {
            Some(code) => Outcome::Exited { code },
            // Killed by a signal: it ran, but there is no verdict.
            None => Outcome::Unknown {
                reason: "killed by a signal".into(),
            },
        },
        Ok(Err(e)) => Outcome::Unknown {
            reason: e.to_string(),
        },
        Err(_) => {
            // The group is already gone; the shell is reaped here.
            let _ = child.kill().await;
            Outcome::TimedOut {
                after_secs: timeout.as_secs(),
            }
        }
    };
    // Decoded once at the end, so an 8 KiB read boundary never splits a
    // multi-byte character in the failing line handed to the agent. The byte
    // count and digest cover the whole output, not the tail; the digest binds
    // the record to this run but does not reproduce, since the two pipes
    // interleave nondeterministically.
    let (buf, output_bytes, output_digest) = captured
        .lock()
        .map(|c| {
            (
                String::from_utf8_lossy(&c.tail).into_owned(),
                c.bytes,
                c.digest.hex(),
            )
        })
        .unwrap_or_else(|_| (String::new(), 0, crate::core::hash::hex(b"")));

    CommandResult {
        command: command.to_string(),
        outcome,
        duration_ms: started.elapsed().as_millis() as u64,
        failures: extract_failures(&buf),
        output_tail: tail(&buf, OUTPUT_TAIL_BYTES),
        output_bytes,
        output_digest,
    }
}

/// What a command printed: the kept tail, plus the count and digest of
/// everything.
#[derive(Default)]
struct Captured {
    tail: Vec<u8>,
    bytes: u64,
    digest: crate::core::hash::Stream,
}

/// Reads one pipe into the shared buffer until it closes.
///
/// Bounded as it arrives, so a runaway command cannot fill memory before the
/// timeout; in bytes, so the bound never cuts a character. Count and digest
/// see every byte first.
async fn drain<R>(pipe: Option<R>, into: std::sync::Arc<std::sync::Mutex<Captured>>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut pipe) = pipe else { return };
    let mut chunk = vec![0u8; 8 * 1024];
    while let Ok(n) = pipe.read(&mut chunk).await {
        if n == 0 {
            return;
        }
        let Ok(mut c) = into.lock() else { return };
        c.bytes += n as u64;
        c.digest.update(&chunk[..n]);
        c.tail.extend_from_slice(&chunk[..n]);
        if c.tail.len() > 4 * OUTPUT_TAIL_BYTES {
            let keep = c.tail.len() - 2 * OUTPUT_TAIL_BYTES;
            c.tail.drain(..keep);
        }
    }
}

/// A gate command's process group, killed once: explicitly when the command
/// ends, or on drop when the future was cancelled first. Once, so a pid the
/// system has since reused is never signalled.
struct Group(Option<u32>);

impl Group {
    fn kill(&mut self) {
        terminate_group(self.0.take());
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        self.kill();
    }
}

/// SIGKILLs a process group led by `pid`. A no-op where the platform has no
/// group semantics, and harmless when the group is already gone.
fn terminate_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        // SAFETY: a negative pid signals the group created by
        // `process_group(0)`, which holds only this gate's children. ESRCH just
        // means it already exited.
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
/// Only a handful of well-known shapes: a wrong guess sends the agent after the
/// wrong line. When nothing matches, the caller falls back to the output tail.
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
            &[],
        )
        .await;
        assert!(report.passed());
        assert_eq!(report.commands.len(), 2);
        assert!(report.summary().contains("passed"));
    }

    #[tokio::test]
    async fn an_empty_gate_is_not_a_pass() {
        // An empty Definition of Done proves nothing.
        let report = run("check", &[], &dir(), Duration::from_secs(10), 1, &[]).await;
        assert!(!report.passed());
    }

    #[tokio::test]
    async fn the_first_failure_stops_the_gate() {
        let report = run(
            "check",
            &["false".into(), "echo should-not-run".into()],
            &dir(),
            Duration::from_secs(10),
            1,
            &[],
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
            &[],
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
        // Sequential pipe reads deadlock once stderr's 64 KiB buffer fills.
        let report = run(
            "check",
            // stderr first, to fill its buffer before stdout closes.
            &["yes stderr | head -c 400000 >&2; yes stdout | head -c 400000; echo DRAINED".into()],
            &dir(),
            Duration::from_secs(20),
            1,
            &[],
        )
        .await;
        assert!(!report.commands[0].timed_out(), "the gate deadlocked");
        assert!(report.passed());
        assert!(
            report.commands[0].output_tail.contains("DRAINED"),
            "the command ran to the end, so both pipes were drained"
        );
    }

    /// The byte count and digest cover everything printed, not the kept tail.
    #[tokio::test]
    async fn output_over_the_tail_counts_whole() {
        let report = run(
            "check",
            &["yes 0123456789 | head -c 200000".into()],
            &dir(),
            Duration::from_secs(20),
            1,
            &[],
        )
        .await;
        let c = &report.commands[0];
        assert_eq!(c.output_bytes, 200_000, "counted over the trimmed buffer");
        let whole: Vec<u8> = "0123456789\n".bytes().cycle().take(200_000).collect();
        assert_eq!(c.output_digest, crate::core::hash::hex(&whole));
        assert!(c.output_tail.len() < 200_000);
    }

    /// A gate that writes into the tree it checks keeps its stamp but loses its
    /// digest.
    #[tokio::test]
    async fn a_tree_that_moves_while_the_gate_runs_has_no_digest() {
        let root = std::env::temp_dir().join(format!(
            "devplane-gate-moves-{}-{}",
            std::process::id(),
            jiff::Timestamp::now().as_nanosecond()
        ));
        std::fs::create_dir_all(&root).unwrap();
        for args in [
            &["init", "-q", "-b", "main"][..],
            &[
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "i",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let still = run(
            "check",
            &["true".into()],
            &root,
            Duration::from_secs(10),
            1,
            &[],
        )
        .await;
        assert!(still.commit.as_ref().unwrap().tree.is_some());
        let moved = run(
            "check",
            &["echo generated > out.txt".into()],
            &root,
            Duration::from_secs(10),
            1,
            &[],
        )
        .await;
        assert!(moved.passed());
        let stamp = moved.commit.expect("still stamped");
        assert!(
            stamp.tree.is_none(),
            "a pass over a moving tree kept a digest"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn both_streams_reach_the_report() {
        // Both present; the order is not asserted, since the OS does not
        // promise one.
        let report = run(
            "check",
            &["echo on-stdout; echo on-stderr >&2".into()],
            &dir(),
            Duration::from_secs(10),
            1,
            &[],
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
            &[],
        )
        .await;
        assert!(report.commands[0].timed_out());
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
            &[],
        )
        .await;
        assert_eq!(report.commands.len(), 1, "the gate stops at the timeout");
        assert!(report.commands[0].timed_out());
    }

    #[tokio::test]
    async fn a_gate_that_times_out_still_says_what_it_saw() {
        // A timed-out gate still reports what the command printed.
        let report = run(
            "check",
            &["echo 'running test auth::login'; sleep 30".into()],
            &dir(),
            Duration::from_millis(400),
            1,
            &[],
        )
        .await;
        assert!(report.commands[0].timed_out());
        assert!(
            report.commands[0].output_tail.contains("auth::login"),
            "the last thing it printed is the only clue there is: {}",
            report.commands[0].output_tail
        );
    }

    #[tokio::test]
    async fn output_that_is_not_ascii_survives_the_chunk_boundary() {
        // A multi-byte character straddling an 8 KiB read boundary must survive.
        let padding = "x".repeat(8 * 1024 - 1);
        let report = run(
            "check",
            &[format!("printf '{padding}äöü — café ✓\n'; exit 1")],
            &dir(),
            Duration::from_secs(10),
            1,
            &[],
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
            &[],
        )
        .await;
        assert!(!report.passed());
    }

    #[tokio::test]
    async fn a_declared_variable_reaches_every_command() {
        // The shared cache variable must reach the gate, or it builds cold.
        let report = run(
            "check",
            &["echo \"seen=$CARGO_TARGET_DIR\"".into()],
            &dir(),
            Duration::from_secs(10),
            1,
            &[("CARGO_TARGET_DIR".into(), PathBuf::from("/shared/cargo"))],
        )
        .await;
        assert!(report.passed());
        assert!(
            report.commands[0]
                .output_tail
                .contains("seen=/shared/cargo"),
            "{}",
            report.commands[0].output_tail
        );
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

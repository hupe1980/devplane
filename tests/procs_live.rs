//! The process-table reader, against this machine rather than against a
//! fixture.
//!
//! `src/observe/procs.rs` unit-tests the parsing and the identity rules over
//! canned rows, which is the right place for the logic. What no fixture can
//! check is whether `ps` on *this* platform prints the columns this code asks
//! for, in the order it expects — and that is the half that breaks silently,
//! because a parser fed nothing returns an empty list and an empty list reads
//! as *no leaked agents* rather than as *the reader is broken*.
//!
//! Unix only. There is no process-group semantics to reason about elsewhere,
//! and the guard this whole mechanism backs up is `#[cfg(unix)]` in the
//! protocol crate too.

#![cfg(unix)]

use devplane::observe::procs;
use std::os::unix::process::CommandExt as _;

/// The snapshot is real: it contains the process asking for it.
#[test]
fn the_process_table_contains_the_process_that_asked_for_it() {
    let snap = procs::snapshot();
    assert!(
        !snap.is_empty(),
        "`ps -eo pid=,ppid=,pgid=,command=` produced nothing this parser could read, \
         which would make every leak check answer \"no leak\" for ever"
    );
    let me = std::process::id();
    let found = snap
        .iter()
        .find(|p| p.pid == me)
        .expect("this test's own pid is missing from the snapshot");
    assert_eq!(found.pid, me);
    assert!(
        !found.command.is_empty(),
        "the command column is what tells a recycled pid from our agent, and it is empty"
    );
}

/// **The whole mechanism, against a process shaped like the thing it hunts.**
///
/// A leaked agent is a group leader that outlived its parent. This spawns one —
/// `setsid` makes it lead its own group exactly as the protocol crate's
/// `process_group(0)` does — confirms it is found by pid, command and
/// group-leadership together, and then confirms the two ways it must *not* be
/// found: a pid that is not a leader, and a command that no longer matches,
/// which is the pid-reuse case.
#[test]
fn a_group_leading_process_is_found_and_a_mismatched_one_is_not() {
    // `sleep` is everywhere and takes an argument distinctive enough to match
    // on without matching this test binary or the harness around it.
    let marker = format!("devplane-leak-probe-{}", std::process::id());
    let Ok(child) = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("exec -a {marker} sleep 30")])
        .process_group(0)
        .spawn()
    else {
        // A machine that will not spawn a process has nothing to tell us about
        // a reader of the process table.
        return;
    };
    let pid = child.id();

    // Give the exec a moment to replace the shell, so the command column shows
    // the marker rather than `/bin/sh`.
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Everything below runs against a real process, and the result is computed
    // before the probe is killed so that a failure cannot leave it behind.
    let snap = procs::snapshot();
    let outcome = (|| {
        let p = snap
            .iter()
            .find(|p| p.pid == pid)
            .ok_or_else(|| format!("the probe (pid {pid}) is not in the snapshot"))?;
        if !p.is_group_leader() {
            return Err(format!(
                "spawned with process_group(0) and not leading its own group: {p:?}"
            ));
        }
        // Found when the recorded command still matches.
        let hits = procs::leaked(&[(pid, marker.clone())]);
        if hits.len() != 1 || hits[0].pid != pid {
            return Err(format!("the probe was not recognised: {p:?} -> {hits:?}"));
        }
        // **Not** found when it does not — the pid-reuse guard. Without this a
        // recycled pid would be reported as somebody's leaked agent.
        if !procs::leaked(&[(pid, "a-command-this-is-not".to_string())]).is_empty() {
            return Err("a recorded command that no longer matches was reported".to_string());
        }
        Ok(())
    })();

    // Nothing is left behind, whatever the assertions said: the negative group
    // id ends the probe and anything it started, which is the same signal the
    // inbox row tells a person to send.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    let mut child = child;
    let _ = child.wait();

    // **Asserted after the cleanup and never skipped.** The first version of
    // this test wrapped every assertion in `if let Some(p) = found`, so a
    // snapshot that did not contain the probe passed silently — a vacuous test
    // of a reader whose failure mode is returning nothing.
    outcome.expect("the live process-table check");
}

/// A pid nothing is using is not reported, whatever command is recorded for it.
#[test]
fn a_pid_that_is_not_running_is_never_reported_as_leaked() {
    // Zero is never a process: `kill(0, …)` addresses the caller's own group,
    // which is the bug this guards against in the other direction.
    assert!(procs::leaked(&[(0, "sleep".to_string())]).is_empty());
    // And a pid far above the system maximum cannot be allocated.
    assert!(procs::leaked(&[(u32::MAX, "sleep".to_string())]).is_empty());
}

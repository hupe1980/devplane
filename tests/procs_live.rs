//! The process-table reader against this machine: checks that `ps` here prints
//! the columns the parser expects, since a broken reader returns an empty list
//! that reads as "no leaked agents". Unix only.

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

/// A leaked agent is a group leader that outlived its parent. A `setsid` probe is
/// found by pid, command and leadership together, and not found as a non-leader
/// pid or with a changed command (pid reuse).
#[test]
fn a_group_leading_process_is_found_and_a_mismatched_one_is_not() {
    // A symlink gives the probe a distinctive `argv[0]` without `exec -a`, which
    // dash (`/bin/sh` on Debian/Ubuntu) does not support.
    let marker = format!("devplane-leak-probe-{}", std::process::id());
    let dir = std::env::temp_dir().join(format!("devplane-probe-{}", std::process::id()));
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let probe = dir.join(&marker);
    let Some(sleep) = ["/bin/sleep", "/usr/bin/sleep"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
    else {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    if std::os::unix::fs::symlink(sleep, &probe).is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let Ok(child) = std::process::Command::new(&probe)
        .arg("30")
        .process_group(0)
        .spawn()
    else {
        // A machine that cannot spawn a process says nothing about the reader.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let pid = child.id();

    // A moment for the process table to show it.
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Computed before the probe is killed, so a failure cannot leave it behind.
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
        // Not found when it does not: the pid-reuse guard.
        if !procs::leaked(&[(pid, "a-command-this-is-not".to_string())]).is_empty() {
            return Err("a recorded command that no longer matches was reported".to_string());
        }
        Ok(())
    })();

    // The negative group id ends the probe and anything it started.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    let mut child = child;
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);

    // Asserted after cleanup and never skipped: an empty snapshot must fail.
    outcome.expect("the live process-table check");
}

/// A pid nothing is using is not reported, whatever command is recorded for it.
#[test]
fn a_pid_that_is_not_running_is_never_reported_as_leaked() {
    // Zero is never a process: `kill(0, …)` addresses the caller's own group.
    assert!(procs::leaked(&[(0, "sleep".to_string())]).is_empty());
    // And a pid far above the system maximum cannot be allocated.
    assert!(procs::leaked(&[(u32::MAX, "sleep".to_string())]).is_empty());
}

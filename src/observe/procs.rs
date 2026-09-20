//! The process table, read once, for the one question nothing else answers:
//! **is an agent Devplane started still running with nobody talking to it?**
//!
//! A graceful stop tears every agent's process group down. A `SIGKILL`, a
//! crash, an OOM or a power cut gives the daemon no chance: the agent
//! re-parents to pid 1, keeps its process group, and blocks on a stdout nobody
//! is reading — holding a worktree, and still spending if it was mid-turn. For
//! a product whose sentence is *what happened without you*, a model burning
//! money with no row anywhere is the worst bug available.
//!
//! One `ps`, run at most twice per daemon lifetime. `/proc` is Linux-only and
//! would need a second path for macOS anyway.
//!
//! **It never kills anything**, and not out of squeamishness: the agent may be
//! *mid-write*, finishing a long command whose output it is about to put in the
//! worktree. A tool that killed it on sight would destroy work to tidy up after
//! itself. The row says what is running, where, and how to end it.

use std::collections::HashSet;

/// One row of the process table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    /// The process group. An agent is spawned as its own group leader, so for
    /// one of ours this equals `pid` — which is most of what identifies it.
    pub pgid: u32,
    pub command: String,
}

impl Proc {
    /// Whether this process leads its own group, which every agent the
    /// protocol crate spawns does and almost nothing else on a developer's
    /// machine does by accident.
    pub fn is_group_leader(&self) -> bool {
        self.pid == self.pgid
    }

    /// Whether this looks like the agent that was recorded.
    ///
    /// Pid reuse is the risk and this is the mitigation. A pid on a busy
    /// machine comes round again, and acting on a stale one would mean naming
    /// somebody's shell as a leaked agent. Three things have to agree: the pid,
    /// that it still leads its own group, and that its command line still
    /// contains the agent's own command. Two of the three are cheap
    /// coincidences; all three together are not.
    pub fn looks_like_agent(&self, agent_command: &str) -> bool {
        self.is_group_leader() && !agent_command.is_empty() && self.command.contains(agent_command)
    }
}

/// Whether `pid` is a Devplane daemon, as far as the process table can say.
///
/// `Some(true)` it is, `Some(false)` the pid belongs to something else, and
/// `None` the process table could not be read — which is *not* the same as
/// *no*, and the caller has to decide which way to be wrong.
///
/// **Liveness alone is the wrong question.** `daemon.json` outlives a daemon
/// that was killed, and the pid gets reused — so a guard that asks only whether
/// *something* is alive at that number refuses to start for ever, naming
/// somebody else's process. [`Proc::looks_like_agent`] takes the same care for
/// the same reason.
///
/// Matched on the command rather than on group leadership: a daemon started
/// from a shell is not its own group leader.
pub fn is_daemon(pid: u32) -> Option<bool> {
    let table = snapshot();
    if table.is_empty() {
        return None;
    }
    let Some(p) = table.iter().find(|p| p.pid == pid) else {
        // The table was read and the pid is not in it: gone, definitively.
        return Some(false);
    };
    // The binary's own name, so a rename cannot leave this matching nothing.
    Some(p.command.contains(env!("CARGO_PKG_NAME")))
}

/// Every process on the machine, or an empty list when it cannot be read.
///
/// **Absence is not emptiness, and the caller is the one that knows what to do
/// about it** — but there is exactly one caller and for it the two coincide: a
/// `ps` that will not run means *no leak can be reported*, never *there is no
/// leak*, and reporting nothing is the honest outcome either way.
#[cfg(unix)]
pub fn snapshot() -> Vec<Proc> {
    let out = std::process::Command::new("ps")
        .args(["-eo", "pid=,ppid=,pgid=,command="])
        .output();
    let Ok(out) = out else {
        tracing::debug!("could not read the process table");
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(unix))]
pub fn snapshot() -> Vec<Proc> {
    // No process-group semantics to reason about, and the guard this backs up
    // is `#[cfg(unix)]` in the protocol crate too. Reporting nothing is
    // correct rather than unimplemented.
    Vec::new()
}

/// Splits `ps` output into rows.
///
/// Separated from [`snapshot`] so the parsing is testable without a machine
/// state: the three numeric columns are fixed-width-ish and the command is
/// everything after them, spaces and all.
fn parse(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let pid = it.next()?.parse().ok()?;
            let ppid = it.next()?.parse().ok()?;
            let pgid = it.next()?.parse().ok()?;
            // The rest of the line, with its original spacing thrown away —
            // this is matched with `contains`, never re-executed.
            let command = it.collect::<Vec<_>>().join(" ");
            Some(Proc {
                pid,
                ppid,
                pgid,
                command,
            })
        })
        .collect()
}

/// The pids of this process's own children that lead their own groups.
///
/// Used twice around a spawn, and the difference is the agent that was just
/// started. Restricted to group leaders because that is what the protocol
/// crate makes an agent and because it drops the ordinary short-lived children
/// this daemon shells out for — `git`, `gh`, a gate command — which inherit
/// this process's group and would otherwise show up as candidates.
pub fn own_group_leading_children() -> HashSet<u32> {
    let me = std::process::id();
    snapshot()
        .into_iter()
        .filter(|p| p.ppid == me && p.is_group_leader())
        .map(|p| p.pid)
        .collect()
}

/// The single child that appeared between two readings, if it is unambiguous.
///
/// **`None` when two appeared**, which is the honest answer rather than a
/// guess: two dispatches racing would otherwise have a one-in-two chance of
/// attributing each other's agent, and a leaked-process row naming the wrong
/// worktree is worse than no row. The cost of `None` is that this daemon
/// cannot report a leak for that one run, which is exactly where it was
/// before this module existed.
pub fn one_new_child(before: &HashSet<u32>, after: &HashSet<u32>) -> Option<u32> {
    let mut new = after.difference(before);
    let first = new.next().copied()?;
    new.next().is_none().then_some(first)
}

/// Agents this daemon recorded that are still running with nobody attached.
///
/// Every one of these is a process that was started by a previous daemon,
/// survived its death, and cannot be reached: its stdio was that daemon's
/// pipes. It will never make progress and will never be answered.
pub fn leaked(recorded: &[(u32, String)]) -> Vec<Proc> {
    if recorded.is_empty() {
        return Vec::new();
    }
    let snap = snapshot();
    recorded
        .iter()
        .filter_map(|(pid, command)| {
            snap.iter()
                .find(|p| p.pid == *pid && p.looks_like_agent(command))
                .cloned()
        })
        .collect()
}

#[cfg(test)]
mod tests {

    /// **A live pid is not a running daemon**, and the guard that believed it
    /// could refuse to start for ever after a `SIGKILL` left `daemon.json`
    /// behind and the pid came round again.
    #[cfg(unix)]
    #[test]
    fn a_pid_that_is_not_ours_is_not_a_daemon() {
        // pid 1 is launchd or init: always alive, never Devplane. It is also
        // the case that proves this cannot be answered with `kill(pid, 0)`.
        match is_daemon(1) {
            Some(false) => {}
            Some(true) => panic!("pid 1 is init, not a Devplane daemon"),
            // A machine whose process table cannot be read answers `None`, and
            // the caller refuses rather than guessing. Nothing to assert.
            None => {}
        }
        // This test binary's own command carries the crate name, so it is the
        // positive control. Guarded on actually finding ourselves in the table:
        // `ps` is a snapshot taken under whatever load the suite is running, and
        // asserting on a row that may not have been captured would trade a real
        // check for an intermittent one.
        let me = std::process::id();
        if snapshot().iter().any(|p| p.pid == me) {
            assert_eq!(
                is_daemon(me),
                Some(true),
                "our own process should match the crate name"
            );
        }
    }
    use super::*;

    fn rows() -> Vec<Proc> {
        parse(
            "    1     0     1 /sbin/launchd\n\
             \x20 910   1   910 node /usr/local/bin/claude-agent-acp --stdio\n\
             \x20 911 910   910 /bin/sh -c cargo test\n\
             \x20 912   1   912 npx codex acp\n",
        )
    }

    #[test]
    fn the_process_table_parses_into_rows_with_the_command_intact() {
        let r = rows();
        assert_eq!(r.len(), 4);
        assert_eq!(r[1].pid, 910);
        assert_eq!(r[1].ppid, 1);
        assert_eq!(r[1].pgid, 910);
        assert_eq!(
            r[1].command, "node /usr/local/bin/claude-agent-acp --stdio",
            "the command keeps its arguments, which is what identifies it"
        );
    }

    /// The three-way identity check, and the pid-reuse case it exists for.
    #[test]
    fn a_recorded_pid_is_only_ours_if_it_still_leads_its_group_and_still_matches() {
        let r = rows();
        let agent = &r[1];
        assert!(agent.looks_like_agent("claude-agent-acp"));

        // Same pid, but it is no longer a group leader: a child inside somebody
        // else's group, which an agent this crate spawned never is.
        let child = &r[2];
        assert!(!child.is_group_leader());
        assert!(!child.looks_like_agent("cargo"));

        // Pid came round again and is now something else entirely. This is the
        // failure the command match prevents, and without it this function
        // would name a stranger's process as a leaked agent.
        assert!(!agent.looks_like_agent("codex"));
        // And an empty recorded command matches nothing rather than everything.
        assert!(!agent.looks_like_agent(""));
    }

    #[test]
    fn one_new_child_is_reported_and_two_are_not() {
        let before: HashSet<u32> = [1, 2].into_iter().collect();
        assert_eq!(
            one_new_child(&before, &[1, 2, 9].into_iter().collect()),
            Some(9)
        );
        assert_eq!(one_new_child(&before, &before), None, "nothing appeared");
        assert_eq!(
            one_new_child(&before, &[1, 2, 9, 10].into_iter().collect()),
            None,
            "two dispatches raced, so neither is attributed rather than one being guessed"
        );
        // A child that went away between the readings does not make the
        // difference negative or the answer wrong.
        assert_eq!(
            one_new_child(&before, &[2, 9].into_iter().collect()),
            Some(9)
        );
    }

    #[test]
    fn nothing_recorded_means_no_process_table_is_read_at_all() {
        assert!(leaked(&[]).is_empty());
    }

    /// The parser survives what `ps` actually does at the edges.
    #[test]
    fn rows_that_cannot_be_read_are_dropped_rather_than_guessed_at() {
        let r = parse("not a row\n\n  7\n  7 8\n  7 8 9\n  7 8 9 sleep 1\n");
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(
            r[0].command, "",
            "three columns and no command is still a row"
        );
        assert_eq!(r[1].command, "sleep 1");
    }
}

/// Whether a process is a command an agent started for a tool call.
///
/// **Deliberately narrow, and the direction of the narrowness is the point.**
/// Every provider spawns a tool-call command as a shell with `-c`; almost
/// nothing else a session owns looks like that. A false positive here is the
/// expensive mistake — it would mark a session that genuinely wants a prompt as
/// busy, and a person who is the blocker would never be told — so anything this
/// cannot recognise is counted as *not a job*, which leaves the board saying
/// exactly what it says today.
fn is_tool_command(command: &str) -> bool {
    // The shell-snapshot path is the provider's own fingerprint and needs no
    // guessing. The generic form catches the same call under a config
    // directory this does not know the name of.
    if command.contains("shell-snapshots/") {
        return true;
    }
    let mut words = command.split_whitespace();
    let Some(program) = words.next() else {
        return false;
    };
    let shell = matches!(
        program.rsplit('/').next().unwrap_or(program),
        "sh" | "bash" | "zsh" | "fish" | "dash"
    );
    shell && words.next() == Some("-c")
}

/// How many commands each of the given sessions is running right now.
///
/// One `ps`, answering for every session at once, because the question is
/// asked about all of them on the same poll. Sessions with nothing running are
/// present with a zero — **a checked zero is a different fact from an absent
/// one**, and the caller distinguishes them.
#[cfg(unix)]
pub fn running_jobs(session_pids: &[u32]) -> std::collections::HashMap<u32, u32> {
    jobs_from(&snapshot(), session_pids)
}

#[cfg(not(unix))]
pub fn running_jobs(session_pids: &[u32]) -> std::collections::HashMap<u32, u32> {
    let _ = session_pids;
    std::collections::HashMap::new()
}

/// Separated from [`running_jobs`] so the counting is testable against a fixed
/// process table rather than whatever this machine happens to be doing.
fn jobs_from(procs: &[Proc], session_pids: &[u32]) -> std::collections::HashMap<u32, u32> {
    let wanted: HashSet<u32> = session_pids.iter().copied().collect();
    let mut out: std::collections::HashMap<u32, u32> = wanted.iter().map(|p| (*p, 0)).collect();
    for p in procs {
        if wanted.contains(&p.ppid) && is_tool_command(&p.command) {
            *out.entry(p.ppid).or_insert(0) += 1;
        }
    }
    out
}

#[cfg(test)]
mod job_tests {
    use super::*;

    fn proc(pid: u32, ppid: u32, command: &str) -> Proc {
        Proc {
            pid,
            ppid,
            pgid: pid,
            command: command.to_string(),
        }
    }

    /// The bug this exists for, as the process table showed it: a session the
    /// roster called `idle` with a test suite running under it for 37 minutes,
    /// beside four genuinely idle sessions with no children at all.
    #[test]
    fn a_session_running_a_test_suite_is_told_apart_from_one_that_wants_a_prompt() {
        let table = vec![
            proc(17144, 1, "claude --output-format stream-json"),
            proc(
                12282,
                17144,
                "/bin/zsh -c source /Users/x/.claude/shell-snapshots/snapshot-zsh-1.sh && cargo test",
            ),
            proc(93172, 1, "claude --output-format stream-json"),
        ];
        let jobs = jobs_from(&table, &[17144, 93172]);
        assert_eq!(jobs.get(&17144), Some(&1), "the suite is running");
        assert_eq!(
            jobs.get(&93172),
            Some(&0),
            "and a session with no children is a checked zero, not a gap"
        );
    }

    /// **A session nobody asked about is absent, not zero.** The caller turns
    /// a zero into *the job finished*; inventing one for a session that was
    /// never in the list would clear a block on no evidence.
    #[test]
    fn only_the_sessions_asked_about_are_answered_for() {
        let table = vec![proc(200, 100, "/bin/sh -c make test")];
        let jobs = jobs_from(&table, &[999]);
        assert_eq!(jobs.get(&100), None);
        assert_eq!(jobs.get(&999), Some(&0));
    }

    /// Everything that is not recognisably a tool call is counted as none of
    /// them, because the cost of guessing wrong runs the other way: a person
    /// who is the blocker would stop being told so.
    #[test]
    fn nothing_unrecognised_is_counted_as_a_job() {
        assert!(is_tool_command(
            "/bin/zsh -c source /h/.claude/shell-snapshots/s.sh && just ci"
        ));
        assert!(is_tool_command("/bin/sh -c cargo test"));
        assert!(is_tool_command("bash -c make"));
        // A language server, an MCP server, an editor helper: all plausible
        // children of a session, none of them a command it is waiting on.
        assert!(!is_tool_command("node /path/to/mcp-server.js"));
        assert!(!is_tool_command("/usr/bin/python3 -m something"));
        assert!(!is_tool_command("/bin/zsh -i"));
        assert!(!is_tool_command("rust-analyzer"));
        assert!(!is_tool_command(""));
    }
}

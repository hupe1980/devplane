//! The process table, read for one question: is an agent Devplane started
//! still running with nobody talking to it?
//!
//! After a `SIGKILL`, crash or power cut the agent re-parents to pid 1, keeps
//! its process group and blocks on an unread stdout, holding a worktree and
//! possibly spending. One `ps` (portable, unlike `/proc`), at most twice per
//! host lifetime. It never kills: the agent may be mid-write. The row says what
//! is running, where, and how to end it.

use std::collections::HashSet;

/// One row of the process table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    /// The process group. An agent is spawned as its own group leader, so for
    /// one of ours this equals `pid`.
    pub pgid: u32,
    pub command: String,
}

impl Proc {
    /// Whether this process leads its own group, which every spawned agent
    /// does and almost nothing else does by accident.
    pub fn is_group_leader(&self) -> bool {
        self.pid == self.pgid
    }

    /// Whether this looks like the agent that was recorded.
    ///
    /// Guards against pid reuse: the pid, group leadership and the agent's
    /// command in the command line must all agree.
    pub fn looks_like_agent(&self, agent_command: &str) -> bool {
        self.is_group_leader() && !agent_command.is_empty() && self.command.contains(agent_command)
    }
}

/// Every process on the machine, or an empty list when it cannot be read.
///
/// A `ps` that will not run means no leak can be reported, never that there
/// is none; reporting nothing is honest either way.
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
    // No process-group semantics here, and the guard this backs is
    // `#[cfg(unix)]` in the protocol crate too.
    Vec::new()
}

/// Splits `ps` output into rows.
///
/// Separate from [`snapshot`] for testing: three numeric columns, then the
/// command with its spaces.
// Only the Unix process-table reader calls this; tests exercise it everywhere.
#[cfg(any(unix, test))]
fn parse(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let pid = it.next()?.parse().ok()?;
            let ppid = it.next()?.parse().ok()?;
            let pgid = it.next()?.parse().ok()?;
            // Spacing is lost; this is only matched with `contains`.
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
/// Diffed around a spawn to find the new agent. Group leaders only, which
/// drops the short-lived `git` and gate children that share our group.
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
/// `None` when two appeared: racing starts could otherwise swap agents, and a
/// row naming the wrong worktree is worse than none.
pub fn one_new_child(before: &HashSet<u32>, after: &HashSet<u32>) -> Option<u32> {
    let mut new = after.difference(before);
    let first = new.next().copied()?;
    new.next().is_none().then_some(first)
}

/// Agents this host recorded that are still running with nobody attached.
///
/// Each was started by a previous host and survived it; its stdio was that
/// host's pipes, so it will never be answered.
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

        // Same pid, no longer a group leader: not one of ours.
        let child = &r[2];
        assert!(!child.is_group_leader());
        assert!(!child.looks_like_agent("cargo"));

        // The pid came round again as something else; the command match
        // stops this naming a stranger's process.
        assert!(!agent.looks_like_agent("codex"));
        // An empty recorded command matches nothing rather than everything.
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
        // A child gone between readings does not skew the difference.
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
/// Deliberately narrow: providers spawn tool calls as a shell with `-c`. A
/// false positive would mark a session that needs a person as busy, so
/// anything unrecognised is not a job.
// Only the Unix process-table reader calls this; tests exercise it everywhere.
#[cfg(any(unix, test))]
fn is_tool_command(command: &str) -> bool {
    // The shell-snapshot path is the provider's own fingerprint; the generic
    // form catches it under an unknown config directory.
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
/// One `ps` for every session on the poll. Sessions with nothing running get
/// a zero: a checked zero differs from an absent one. Only for sessions no
/// hook speaks for — a connected session reports `background_tasks` itself.
#[cfg(unix)]
pub fn running_jobs(session_pids: &[u32]) -> std::collections::HashMap<u32, u32> {
    jobs_from(&snapshot(), session_pids)
}

#[cfg(not(unix))]
pub fn running_jobs(session_pids: &[u32]) -> std::collections::HashMap<u32, u32> {
    let _ = session_pids;
    std::collections::HashMap::new()
}

/// Separate from [`running_jobs`] so counting is testable on a fixed table.
// Only the Unix process-table reader calls this; tests exercise it everywhere.
#[cfg(any(unix, test))]
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

    /// A session reported `idle` with a test suite running under it, beside
    /// genuinely idle sessions with no children.
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

    /// A session nobody asked about is absent, not zero: a zero clears a
    /// block.
    #[test]
    fn only_the_sessions_asked_about_are_answered_for() {
        let table = vec![proc(200, 100, "/bin/sh -c make test")];
        let jobs = jobs_from(&table, &[999]);
        assert_eq!(jobs.get(&100), None);
        assert_eq!(jobs.get(&999), Some(&0));
    }

    /// Anything not recognisably a tool call counts as none: guessing wrong
    /// would hide that a person is the blocker.
    #[test]
    fn nothing_unrecognised_is_counted_as_a_job() {
        assert!(is_tool_command(
            "/bin/zsh -c source /h/.claude/shell-snapshots/s.sh && just ci"
        ));
        assert!(is_tool_command("/bin/sh -c cargo test"));
        assert!(is_tool_command("bash -c make"));
        // Language servers, MCP servers, editor helpers: not awaited commands.
        assert!(!is_tool_command("node /path/to/mcp-server.js"));
        assert!(!is_tool_command("/usr/bin/python3 -m something"));
        assert!(!is_tool_command("/bin/zsh -i"));
        assert!(!is_tool_command("rust-analyzer"));
        assert!(!is_tool_command(""));
    }
}

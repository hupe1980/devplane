//! Shared test scaffolding.

/// Serialises every test that drives a real agent process, across test binaries.
///
/// `async-process` has a process-global reactor and child reaper; with tokio
/// runtimes created and dropped per test, an agent's pipes can land on an
/// undriven reactor and its turn never completes. A static mutex is per binary,
/// and `cargo test` runs binaries in parallel, so this uses `flock`, which the
/// kernel releases on process exit.
pub struct AgentSerial(std::fs::File);

pub fn one_agent_at_a_time() -> AgentSerial {
    let path = std::env::temp_dir().join("devplane-test-agents.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .expect("the suite needs a lock file it can create");
    // SAFETY: `file` owns the descriptor and outlives the call.
    let rc = unsafe { libc::flock(std::os::unix::io::AsRawFd::as_raw_fd(&file), libc::LOCK_EX) };
    assert_eq!(rc, 0, "could not take the agent lock");
    AgentSerial(file)
}

impl Drop for AgentSerial {
    fn drop(&mut self) {
        // SAFETY: same descriptor, still owned by `self`.
        unsafe {
            libc::flock(
                std::os::unix::io::AsRawFd::as_raw_fd(&self.0),
                libc::LOCK_UN,
            );
        }
    }
}

/// Kills a spawned host if the test ends (e.g. panics) before reaping it.
/// Holds the pid so the child can be handed to a reaper thread; `disarm` once
/// the process is seen to exit, so a reused pid is never signalled.
#[allow(dead_code)] // only the binaries that spawn a host use it
pub struct HostGuard(Option<u32>);

#[allow(dead_code)]
impl HostGuard {
    pub fn new(pid: u32) -> Self {
        Self(Some(pid))
    }

    pub fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for HostGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            // SAFETY: a signal to a pid this test spawned and has not seen exit.
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
    }
}

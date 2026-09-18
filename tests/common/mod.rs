//! Shared test scaffolding.
//!
//! One thing lives here, and it is the fix for a flake this project has now
//! diagnosed twice.

/// Serialises every test that drives a real agent process — across test
/// *binaries*, not only within one.
///
/// The failure: `#[tokio::test]` builds and drops a runtime per test, while the
/// protocol crate spawns agents through `async-process`, whose reactor and
/// child reaper are process-global. With runtimes coming and going while
/// process groups are killed, an agent's pipes end up on a reactor nothing is
/// driving and its turn never completes. Measured at roughly one run in six.
///
/// The first fix was a `tokio::sync::Mutex` static, which is correct and
/// incomplete: a static is per binary, and `cargo test` runs test binaries
/// **in parallel**. `tests/work.rs` held it and `tests/observer.rs` never had
/// one, so the two raced each other for the machine's global reaper and the
/// flake survived in a form that only appears in a full-suite run — which is
/// exactly where it is hardest to read and the only place CI looks.
///
/// A file lock is the smallest thing that spans processes. `flock` is released
/// by the kernel when the process exits, so a panicking test cannot wedge the
/// suite the way a lock file left on disk would.
pub struct AgentSerial(std::fs::File);

pub fn one_agent_at_a_time() -> AgentSerial {
    let path = std::env::temp_dir().join("devplane-test-agents.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .expect("the suite needs a lock file it can create");
    // SAFETY: `fd` is owned by `file`, which outlives the call, and `flock` on
    // a valid descriptor has no other precondition.
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

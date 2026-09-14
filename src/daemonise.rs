//! Starting the daemon in the background.
//!
//! The daemon has to outlive the terminal that started it: a hook that arrives
//! while no window is open is precisely the observation Vibeplane exists to
//! catch.

use anyhow::{Context, Result};
use std::process::{Command, Stdio};

/// Re-executes this binary as `vibeplane serve`, detached.
pub fn spawn_detached() -> Result<()> {
    let exe = std::env::current_exe().context("finding the vibeplane binary")?;
    let log = crate::config::home()?.join("daemon.log");
    std::fs::create_dir_all(log.parent().unwrap()).ok();
    let out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .with_context(|| format!("opening {}", log.display()))?;
    let err = out.try_clone()?;

    let mut cmd = Command::new(exe);
    cmd.arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A new session, so closing the terminal does not take the daemon with
        // it.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    cmd.spawn().context("starting the daemon")?;
    Ok(())
}

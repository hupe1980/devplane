//! Where Vibeplane keeps its own state, and how a client finds the daemon.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The default port. Chosen to sit next to the other local agent tools rather
/// than in the ephemeral range, so it is recognisable in `lsof`.
pub const DEFAULT_PORT: u16 = 47831;

/// `~/.vibeplane`, or `$VIBEPLANE_HOME`.
pub fn home() -> Result<PathBuf> {
    if let Some(h) = std::env::var_os("VIBEPLANE_HOME") {
        return Ok(PathBuf::from(h));
    }
    let base = dirs::home_dir().context("cannot determine the home directory")?;
    Ok(base.join(".vibeplane"))
}

pub fn db_path() -> Result<PathBuf> {
    Ok(home()?.join("vibeplane.db"))
}

pub fn token_path() -> Result<PathBuf> {
    Ok(home()?.join("token"))
}

pub fn daemon_file() -> Result<PathBuf> {
    Ok(home()?.join("daemon.json"))
}

/// What a running daemon publishes so clients can reach it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonInfo {
    pub pid: u32,
    pub port: u16,
    pub version: String,
    pub started_at: String,
}

impl DaemonInfo {
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

pub fn read_daemon_info() -> Result<Option<DaemonInfo>> {
    let p = daemon_file()?;
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(serde_json::from_str(&s).ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn write_daemon_info(info: &DaemonInfo) -> Result<()> {
    let p = daemon_file()?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(info)?)?;
    Ok(())
}

pub fn clear_daemon_info() -> Result<()> {
    let p = daemon_file()?;
    std::fs::remove_file(p).ok();
    Ok(())
}

/// Reads the shared secret, creating it on first use.
///
/// The token gates the loopback API. Loopback alone is not an access control:
/// any process on the machine can reach it, so the token file's permissions are
/// what actually separate Vibeplane from everything else running as the user.
pub fn load_or_create_token() -> Result<String> {
    let p = token_path()?;
    if let Ok(t) = std::fs::read_to_string(&p) {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    let token = generate_token();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, &token)?;
    restrict_permissions(&p)?;
    Ok(token)
}

#[cfg(unix)]
fn restrict_permissions(p: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_p: &std::path::Path) -> Result<()> {
    Ok(())
}

/// 256 bits from the OS RNG, hex-encoded. Two v4 UUIDs: the specification
/// requires a cryptographic source for the random bits, which is exactly what
/// is wanted here and one dependency fewer than pulling in `rand`.
fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_and_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}

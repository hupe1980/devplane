//! Where Devplane keeps its own state, and how a client finds the daemon.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The default port. Chosen to sit next to the other local agent tools rather
/// than in the ephemeral range, so it is recognisable in `lsof`.
pub const DEFAULT_PORT: u16 = 47831;

/// `~/.devplane`, or `$DEVPLANE_HOME`.
pub fn home() -> Result<PathBuf> {
    if let Some(h) = std::env::var_os("DEVPLANE_HOME") {
        return Ok(PathBuf::from(h));
    }
    let base = dirs::home_dir().context("cannot determine the home directory")?;
    Ok(base.join(".devplane"))
}

pub fn db_path() -> Result<PathBuf> {
    Ok(home()?.join("devplane.db"))
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
    /// The binary this daemon was started from.
    ///
    /// **For one question, asked at one moment**: a person who tried `npx` and
    /// then installed properly has two copies and one daemon, and *which one is
    /// running* matters exactly when they wonder why a change did nothing.
    ///
    /// `None` when the process could not name its own executable, which is a
    /// real state on some platforms — reported as unknown rather than guessed
    /// at from `argv[0]`, which a caller controls.
    #[serde(default)]
    pub exe: Option<String>,
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
/// what actually separate Devplane from everything else running as the user.
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

/// Where a decision goes when it was made with no daemon to record it.
///
/// The `command` hook decides in its own process (see `cli::cmd_hook`), so a
/// stopped daemon no longer means an unenforced rule — but it would mean an
/// unrecorded one, and an audit trail with invisible holes is the same class of
/// failure as a rule that quietly does not fire.
pub fn spool_path() -> Result<PathBuf> {
    Ok(home()?.join("pending-decisions.jsonl"))
}

/// The most decisions held for a daemon that never comes. Twenty thousand lines
/// is a few megabytes and weeks of ordinary use.
///
/// **A bound rather than none**, because the alternative is a file that grows
/// for as long as somebody runs agents without ever starting the daemon — which
/// is a supported way to use this, since the gate no longer needs one. The
/// oldest rows go first: a decision from three weeks ago explains less than the
/// one taken a minute ago, and the drain says how many were dropped rather than
/// leaving the count to be inferred from a gap.
const SPOOL_MAX_LINES: usize = 20_000;

/// Appends one decision. `O_APPEND` with a single short write is atomic enough
/// for the only concurrency there is: several hook processes, one line each.
///
/// Failure is deliberately ignored by the caller. This runs while a session is
/// blocked, and a full disk must not turn into a refused tool call.
pub fn spool_decision(line: &serde_json::Value) -> Result<()> {
    use std::io::Write;
    let path = spool_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Checked before the write rather than after, so the file cannot exceed the
    // bound even briefly — and by size first, because `metadata` is one syscall
    // and counting lines is a read of the whole file on every tool call.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > (SPOOL_MAX_LINES * 512) as u64) {
        trim_spool(&path);
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// Keeps the newest `SPOOL_MAX_LINES` and records how many went.
///
/// Rewritten through a temporary file and renamed, so a crash mid-trim leaves
/// either the old spool or the new one and never a half-written log of
/// decisions.
fn trim_spool(path: &std::path::Path) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= SPOOL_MAX_LINES {
        return;
    }
    let dropped = lines.len() - SPOOL_MAX_LINES;
    let marker = serde_json::json!({
        "session": "devplane",
        "verdict": "note",
        "rule": null,
        "subject": format!("{dropped} spooled decisions were dropped: no daemon had started in a very long time"),
        "late": true,
    });
    let tmp = path.with_extension("jsonl.tmp");
    let kept = format!("{marker}\n{}\n", lines[dropped..].join("\n"));
    if std::fs::write(&tmp, kept).is_ok() {
        std::fs::rename(&tmp, path).ok();
    }
}

/// How many decisions are waiting to be filed, without consuming them.
///
/// `doctor` reports it: an enforced decision that is not yet written down is a
/// true statement about the machine, and the person reading diagnostics is the
/// one who would want to know their audit trail is behind.
pub fn drain_spool_count() -> usize {
    spool_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

/// Reads and removes the spool, for the daemon to ingest at startup.
///
/// Read-then-remove rather than truncate: a hook appending between the two
/// loses a line, and losing the *newest* line is better than the alternative of
/// holding a lock on the path every hook process needs.
pub fn drain_spool() -> Vec<serde_json::Value> {
    let Ok(path) = spool_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    std::fs::remove_file(&path).ok();
    text.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
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

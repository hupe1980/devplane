//! Where Devplane keeps its own state, and how a client finds the host.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The default port: next to other local agent tools, outside the ephemeral
/// range.
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

pub fn host_file() -> Result<PathBuf> {
    Ok(home()?.join("host.json"))
}

/// What a running host publishes so commands can reach it: where it listens
/// and which process it is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostRecord {
    pub pid: u32,
    pub port: u16,
    pub version: String,
    pub started_at: String,
    /// The binary this host was started from, so a person with two installs
    /// can tell which one is running. `None` when the process cannot name its
    /// own executable; never guessed from `argv[0]`.
    #[serde(default)]
    pub exe: Option<String>,
}

impl HostRecord {
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

pub fn read_host() -> Result<Option<HostRecord>> {
    let p = host_file()?;
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(serde_json::from_str(&s).ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Written to a temporary file and renamed over, so a client never reads half
/// a record.
pub fn write_host(info: &HostRecord) -> Result<()> {
    let p = host_file()?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(info)?)?;
    std::fs::rename(&tmp, &p).inspect_err(|_| {
        std::fs::remove_file(&tmp).ok();
    })?;
    Ok(())
}

/// Removes `host.json` unconditionally — for a record nothing answers on.
pub fn clear_host() -> Result<()> {
    let p = host_file()?;
    std::fs::remove_file(p).ok();
    Ok(())
}

/// Removes `host.json` only when it names this process: a host shutting down
/// must not delete the record of one that started after it.
pub fn clear_host_if_ours(pid: u32, port: u16) -> Result<()> {
    if let Some(r) = read_host()?
        && r.pid == pid
        && r.port == port
    {
        clear_host()?;
    }
    Ok(())
}

/// `~/.devplane/host.lock`, held exclusively for a host's whole life.
///
/// One host per home is structural: check-then-start races, while the lock is
/// taken before anything boots and released by the OS however the process ends.
#[derive(Debug)]
pub struct HostLock {
    _file: std::fs::File,
}

/// Why the lock was not taken.
#[derive(Debug)]
pub enum LockRefused {
    /// Another process holds it: a host is running or starting.
    Held,
    Io(std::io::Error),
}

/// Takes the host lock under `home`.
pub fn lock_host_at(home: &std::path::Path) -> std::result::Result<HostLock, LockRefused> {
    std::fs::create_dir_all(home).map_err(LockRefused::Io)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join("host.lock"))
        .map_err(LockRefused::Io)?;
    match file.try_lock() {
        Ok(()) => Ok(HostLock { _file: file }),
        Err(std::fs::TryLockError::WouldBlock) => Err(LockRefused::Held),
        Err(std::fs::TryLockError::Error(e)) => Err(LockRefused::Io(e)),
    }
}

/// Reads the shared secret, creating it on first use. Loopback is reachable by
/// any local process, so the token file's permissions are the access control.
pub fn load_or_create_token() -> Result<String> {
    let token = load_or_create_token_at(&token_path()?)?;
    write_ingest_token(&home()?.join(INGEST_TOKEN_FILE), &token);
    Ok(token)
}

/// Where the telemetry-only token is written for exporters a person
/// configures by hand: `$(cat ~/.devplane/telemetry-token)`.
pub const INGEST_TOKEN_FILE: &str = "telemetry-token";

/// Writes the telemetry-only token for `control` at `p`, owner-only, when it
/// is missing or differs. Best effort: `connect` writes it where it is needed.
fn write_ingest_token(p: &std::path::Path, control: &str) {
    let want = ingest_token(control);
    if std::fs::read_to_string(p).is_ok_and(|t| t.trim() == want) {
        return;
    }
    std::fs::remove_file(p).ok();
    if let Ok(mut f) = owner_only().open(p) {
        use std::io::Write;
        let _ = f.write_all(want.as_bytes());
    }
}

/// [`load_or_create_token`] at a path.
///
/// Created owner-only (`0600`) under a temporary name, filled, then linked
/// into place (which fails if another process won), so a reader sees no token
/// or a whole one and two first runs agree.
pub fn load_or_create_token_at(p: &std::path::Path) -> Result<String> {
    let read = |p: &std::path::Path| -> Option<String> {
        let t = std::fs::read_to_string(p).ok()?;
        let t = t.trim().to_string();
        (!t.is_empty()).then_some(t)
    };
    if let Some(t) = read(p) {
        return Ok(t);
    }
    let token = generate_token();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    {
        use std::io::Write;
        let mut f = owner_only().open(&tmp)?;
        f.write_all(token.as_bytes())?;
        f.sync_all()?;
    }
    let linked = std::fs::hard_link(&tmp, p);
    std::fs::remove_file(&tmp).ok();
    match linked {
        Ok(()) => Ok(token),
        // Somebody else created it in the meantime: theirs is the token.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            read(p).context("the token file exists and is empty")
        }
        Err(e) => Err(e.into()),
    }
}

/// A new file, created owner-only.
fn owner_only() -> std::fs::OpenOptions {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o
}

/// The token vendor settings carry, derived one way from the control token.
///
/// Telemetry configuration lives where the agent reads it — Claude Code
/// exports its settings' `env` block to every tool call — so what it carries
/// may only write telemetry: the host accepts it on the telemetry routes and
/// nowhere else, and it cannot be turned back into the token that answers a
/// permission.
pub fn ingest_token(control: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"devplane telemetry ingest\0");
    h.update(control.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 244 random bits from the OS RNG, hex-encoded: two v4 UUIDs (122 random
/// bits each), which require a cryptographic source, without adding `rand`.
fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// The app's own file: `~/.devplane/app.toml`, section `[app]`.
///
/// The shortcut is the one global key the app takes, so it is the person's to
/// change; the port is the in-process host's.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    /// One global shortcut, in the shortcut plugin's own notation.
    pub shortcut: String,
    /// The port the in-process host binds: the CLI host's default unless set;
    /// `0` picks one, and then agent telemetry does not reach it.
    pub port: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            shortcut: "CmdOrCtrl+Shift+Space".into(),
            // The port `connect` writes into the vendor's telemetry settings,
            // so an agent's telemetry reaches the window's host too. `0`
            // (pick one) left every exporter pointing at nothing.
            port: DEFAULT_PORT,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AppFile {
    app: AppConfig,
    github: GitHubConfig,
}

/// The client id of the GitHub OAuth App this build was made with, if any.
/// Public by design (the device flow has no secret); set at build time. It is
/// github.com's: an Enterprise server registers its own apps.
pub const BUILD_CLIENT_ID: Option<&str> = option_env!("DEVPLANE_GITHUB_CLIENT_ID");

/// `[github]` in `~/.devplane/app.toml`: which GitHub host this machine signs
/// in to, and the OAuth App's client id on each host that has one. Nothing
/// secret.
///
/// ```toml
/// [github]
/// host = "ghe.corp"            # the configured host; github.com unless set
/// client_id = "Iv1.…"          # that host's app
///
/// [github.hosts."ghe.other"]
/// client_id = "Iv1.…"          # another Enterprise server's app
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitHubConfig {
    /// `github.com` unless set.
    pub host: String,
    /// The configured host's app.
    pub client_id: Option<String>,
    /// Any other host's app, keyed by host name.
    pub hosts: std::collections::BTreeMap<String, GitHubHostConfig>,
}

/// `[github.hosts."<host>"]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitHubHostConfig {
    pub client_id: Option<String>,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            host: "github.com".into(),
            client_id: None,
            hosts: Default::default(),
        }
    }
}

impl GitHubConfig {
    /// The client id for `host`: its own `[github.hosts]` entry, else
    /// `[github] client_id` when it is the configured host, else the build's
    /// when it is github.com, else none (then only `--with-token` signs in).
    pub fn client_id_for(&self, host: &str) -> Option<String> {
        let host = host.trim().trim_end_matches('/').to_ascii_lowercase();
        let set = |c: &Option<String>| c.clone().filter(|c| !c.trim().is_empty());
        self.hosts
            .iter()
            .find(|(h, _)| h.trim().trim_end_matches('/').eq_ignore_ascii_case(&host))
            .and_then(|(_, c)| set(&c.client_id))
            .or_else(|| {
                let configured = self.host.trim().trim_end_matches('/');
                configured
                    .eq_ignore_ascii_case(&host)
                    .then(|| set(&self.client_id))
                    .flatten()
            })
            .or_else(|| {
                (host == "github.com")
                    .then(|| BUILD_CLIENT_ID.map(str::to_string))
                    .flatten()
            })
            .filter(|c| !c.trim().is_empty())
    }
}

/// `[github]` from an `app.toml`; the defaults where it is missing or does
/// not parse (`app_config_from` reports the latter).
pub fn github_config_from(path: &std::path::Path) -> GitHubConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| toml::from_str::<AppFile>(&t).ok())
        .map(|f| f.github)
        .unwrap_or_default()
}

pub fn app_config_path() -> Result<PathBuf> {
    Ok(home()?.join("app.toml"))
}

/// Reads the app's configuration: defaults where the file is missing; on a
/// parse error the app still starts with defaults and `doctor` names the path.
pub fn app_config() -> Result<(AppConfig, Vec<crate::core::config::Problem>)> {
    Ok(app_config_from(&app_config_path()?))
}

pub fn app_config_from(path: &std::path::Path) -> (AppConfig, Vec<crate::core::config::Problem>) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (AppConfig::default(), Vec::new());
        }
        Err(e) => {
            return (
                AppConfig::default(),
                vec![crate::core::config::Problem {
                    where_: path.display().to_string(),
                    what: format!("could not be read: {e}"),
                    fatal: true,
                }],
            );
        }
    };
    match toml::from_str::<AppFile>(&text) {
        Ok(f) => (f.app, Vec::new()),
        Err(e) => (
            AppConfig::default(),
            vec![crate::core::config::Problem {
                where_: path.display().to_string(),
                what: format!("does not parse, so the defaults apply: {e}"),
                fatal: true,
            }],
        ),
    }
}

/// Where a decision goes when the store could not be opened to record it.
///
/// The `command` hook decides in its own process (see `cli::cmd_hook`); an
/// unopenable store must not leave a hole in the audit trail.
pub fn spool_path() -> Result<PathBuf> {
    Ok(home()?.join("pending-decisions.jsonl"))
}

/// The most decisions held for a store that stays unreadable (a few MB).
/// Nothing need be running to drain it, so it is bounded: the oldest rows go
/// first, and the drain reports how many were dropped.
const SPOOL_MAX_LINES: usize = 20_000;

/// Appends one decision. `O_APPEND` with one short write is atomic enough for
/// several hook processes writing a line each. The caller ignores failure: a
/// full disk must not turn into a refused tool call.
pub fn spool_decision(line: &serde_json::Value) -> Result<()> {
    use std::io::Write;
    let path = spool_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Checked before the write so the file never exceeds the bound, and by
    // size first: `metadata` is one syscall, counting lines reads the file.
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

/// Keeps the newest `SPOOL_MAX_LINES` and records how many went. Rewritten via
/// a temporary file and rename, so a crash leaves the old spool or the new one.
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
        "subject": format!("{dropped} spooled decisions were dropped: no host had started in a very long time"),
        "late": true,
    });
    let tmp = path.with_extension("jsonl.tmp");
    let kept = format!("{marker}\n{}\n", lines[dropped..].join("\n"));
    if std::fs::write(&tmp, kept).is_ok() {
        std::fs::rename(&tmp, path).ok();
    }
}

/// How many decisions are waiting to be filed, without consuming them.
/// `doctor` reports it, so a person can see their audit trail is behind.
pub fn drain_spool_count() -> usize {
    spool_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

/// Reads and removes the spool, for the host to ingest at startup.
///
/// Read-then-remove rather than truncate: a hook appending in between loses
/// its line, which beats a lock on the path every hook process needs.
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

// Reading the configuration files: the pure half parses text it is given
// (`ProjectConfig::parse`); opening the file happens here.
impl crate::core::GlobalConfig {
    /// Reads `<home>/policy.toml`. Missing means no rules; malformed is an
    /// error, because a typo in a deny rule must never read as permission.
    pub fn load(home: &std::path::Path) -> Result<Self, crate::core::ConfigError> {
        let path = home.join("policy.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(crate::core::ConfigError::Io(
                    path.display().to_string(),
                    e.to_string(),
                ));
            }
        };
        toml::from_str(&text)
            .map_err(|e| crate::core::ConfigError::Parse(path.display().to_string(), e.to_string()))
    }
}

impl crate::core::ProjectConfig {
    /// Reads the configuration for a repository root. Missing is the default;
    /// malformed is an error, so a typo never silently removes a deny rule.
    pub fn load(root: &std::path::Path) -> Result<Self, crate::core::ConfigError> {
        let path = root.join(crate::core::config::CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(crate::core::ConfigError::Io(
                    path.display().to_string(),
                    e.to_string(),
                ));
            }
        };
        Self::parse(&text)
            .map_err(|e| crate::core::ConfigError::Parse(path.display().to_string(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No file is the ordinary state and means the defaults; a file that will
    /// not parse is said by path rather than silently replaced by them.
    #[test]
    fn the_app_config_defaults_and_names_a_file_it_cannot_read() {
        let dir = std::env::temp_dir().join(format!(
            "dp-app-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.toml");

        let (cfg, problems) = app_config_from(&path);
        assert_eq!(cfg, AppConfig::default());
        assert_eq!(cfg.shortcut, "CmdOrCtrl+Shift+Space");
        // The port `connect` writes into the telemetry settings.
        assert_eq!(cfg.port, DEFAULT_PORT);
        assert!(problems.is_empty());

        std::fs::write(&path, "[app]\nshortcut = \"Alt+Space\"\nport = 47000\n").unwrap();
        let (cfg, problems) = app_config_from(&path);
        assert_eq!(cfg.shortcut, "Alt+Space");
        assert_eq!(cfg.port, 47000);
        assert!(problems.is_empty());

        std::fs::write(&path, "[app\nshortcut = ").unwrap();
        let (cfg, problems) = app_config_from(&path);
        assert_eq!(
            cfg,
            AppConfig::default(),
            "a broken file falls back to the defaults"
        );
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].where_.ends_with("app.toml") && problems[0].fatal,
            "{problems:?}"
        );

        // A key this file does not have is a typo, not a preference.
        std::fs::write(&path, "[app]\nshortcutt = \"Alt+Space\"\n").unwrap();
        let (_, problems) = app_config_from(&path);
        assert_eq!(
            problems.len(),
            1,
            "an unknown key is a problem: {problems:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_github_section_names_a_host_and_a_client_id() {
        let dir = std::env::temp_dir().join(format!(
            "dp-app-gh-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.toml");
        assert_eq!(github_config_from(&path), GitHubConfig::default());
        assert_eq!(GitHubConfig::default().host, "github.com");

        std::fs::write(
            &path,
            "[app]\nport = 47000\n[github]\nhost = \"ghe.corp\"\nclient_id = \"Iv1.abc\"\n",
        )
        .unwrap();
        let g = github_config_from(&path);
        assert_eq!(g.host, "ghe.corp");
        assert_eq!(g.client_id_for("ghe.corp").as_deref(), Some("Iv1.abc"));
        assert_eq!(
            g.client_id_for("ghe.other"),
            None,
            "one host's app is not another's"
        );
        assert_eq!(
            g.client_id_for("github.com").as_deref(),
            BUILD_CLIENT_ID,
            "github.com takes the build's app, never the Enterprise one"
        );
        let (_, problems) = app_config_from(&path);
        assert!(problems.is_empty(), "{problems:?}");

        std::fs::write(
            &path,
            "[github]\nclient_id = \"Iv1.com\"\n[github.hosts.\"GHE.corp\"]\nclient_id = \"Iv1.ghe\"\n",
        )
        .unwrap();
        let g = github_config_from(&path);
        assert_eq!(g.client_id_for("github.com").as_deref(), Some("Iv1.com"));
        assert_eq!(g.client_id_for("ghe.corp").as_deref(), Some("Iv1.ghe"));
        let (_, problems) = app_config_from(&path);
        assert!(problems.is_empty(), "{problems:?}");

        // A typo is a problem, not a silently ignored setting.
        std::fs::write(&path, "[github]\nhots = \"ghe.corp\"\n").unwrap();
        let (_, problems) = app_config_from(&path);
        assert_eq!(problems.len(), 1, "{problems:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tokens_are_long_and_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    /// The spool bound holds for a machine where Devplane is never opened but
    /// agents keep spooling refusals.
    #[test]
    fn a_long_dormancy_cannot_grow_the_spool_without_bound() {
        let dir = std::env::temp_dir().join(format!(
            "dp-spool-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pending-decisions.jsonl");

        // Far more than the bound, as a dormant machine would accumulate.
        let line = serde_json::json!({"session": "s", "verdict": "deny", "rule": "r"});
        let many = format!("{line}\n").repeat(SPOOL_MAX_LINES + 5_000);
        std::fs::write(&path, &many).unwrap();

        super::trim_spool(&path);

        let after: Vec<String> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        assert!(
            after.len() <= SPOOL_MAX_LINES + 1,
            "the spool kept {} lines, over the bound",
            after.len()
        );

        // Nothing is dropped without a count.
        assert!(
            after[0].contains("5000 spooled decisions were dropped"),
            "the trim must say how many went: {}",
            &after[0][..after[0].len().min(120)]
        );

        // A crash mid-trim leaves the old spool or the new one, and no stray
        // temporary file.
        assert!(
            !path.with_extension("jsonl.tmp").exists(),
            "a temporary file was left beside the spool"
        );

        // The count survives the drain: `drain_spool` drops unparseable lines,
        // so the marker must be valid JSON.
        let parsed: Vec<serde_json::Value> = after
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        assert_eq!(
            parsed.len(),
            after.len(),
            "a drain would discard {} of the kept lines",
            after.len() - parsed.len()
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("vp-config-{tag}-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Two hosts booting in the same instant: exactly one takes the lock, and
    /// it is free again once that host is gone.
    #[test]
    fn only_one_host_holds_the_lock() {
        let home = scratch("lock");
        let (a, b) = std::thread::scope(|s| {
            let a = s.spawn(|| lock_host_at(&home));
            let b = s.spawn(|| lock_host_at(&home));
            (a.join().unwrap(), b.join().unwrap())
        });
        let held = [&a, &b].iter().filter(|r| r.is_ok()).count();
        assert_eq!(held, 1, "{a:?} {b:?}");
        assert!(
            [&a, &b].iter().any(|r| matches!(r, Err(LockRefused::Held))),
            "the other one is refused as held"
        );
        drop((a, b));
        assert!(lock_host_at(&home).is_ok(), "released with its holder");
        std::fs::remove_dir_all(&home).ok();
    }

    /// Created owner-only rather than narrowed afterwards, and two first runs
    /// agree on one token.
    #[test]
    fn the_token_is_created_owner_only_and_once() {
        let home = scratch("token");
        let p = home.join("token");
        let (a, b) = std::thread::scope(|s| {
            let a = s.spawn(|| load_or_create_token_at(&p).unwrap());
            let b = s.spawn(|| load_or_create_token_at(&p).unwrap());
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(a, b, "two first runs made two tokens");
        assert_eq!(load_or_create_token_at(&p).unwrap(), a);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let leftovers = std::fs::read_dir(&home).unwrap().count();
        assert_eq!(leftovers, 1, "a temporary file was left behind");
        std::fs::remove_dir_all(&home).ok();
    }
}

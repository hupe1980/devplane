//! Connecting and disconnecting Claude Code.
//!
//! The only part of Devplane that writes a file the user owns, under five
//! rules: merge, never replace; remove exactly what was added (every entry
//! runs this binary's shim); never take over a setting already doing a job
//! (someone else's telemetry export stays theirs); never install a hook that
//! replaces behaviour (`WorktreeCreate` would break `claude --worktree`,
//! subagent isolation and background sessions); and show the diff first,
//! confirming on a terminal.

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// The marker in every URL Devplane writes (the telemetry endpoint), by which
/// `connect` and `disconnect` recognise their own.
pub const URL_MARKER: &str = "/devplane/";

/// The environment variables Devplane sets for telemetry, plus the metrics
/// exporter pair, so `disconnect` also takes those back.
const OTEL_VARS: &[&str] = &[
    "CLAUDE_CODE_ENABLE_TELEMETRY",
    "OTEL_LOGS_EXPORTER",
    "OTEL_METRICS_EXPORTER",
    "OTEL_EXPORTER_OTLP_PROTOCOL",
    "OTEL_EXPORTER_OTLP_ENDPOINT",
    // Taken with the endpoint: a stale bearer is a credential left behind.
    "OTEL_EXPORTER_OTLP_HEADERS",
    "OTEL_LOGS_EXPORT_INTERVAL",
    "OTEL_METRIC_EXPORT_INTERVAL",
    "OTEL_METRICS_INCLUDE_ENTRYPOINT",
    "OTEL_METRICS_INCLUDE_REPOSITORY",
];

/// The hook events Devplane subscribes to.
///
/// Two are synchronous. `PermissionRequest` is the instant blocked signal and
/// carries the full verdict. `PreToolUse` fires before every tool call in
/// every mode, the only way a prohibition reaches auto mode; it answers with a
/// prohibition or nothing. Everything else is `async`, so no hook slows
/// Claude. `WorktreeCreate` is excluded; see the module docs.
const HOOKS: &[(&str, Option<&str>, bool)] = &[
    ("UserPromptSubmit", None, true),
    ("PreToolUse", None, false),
    ("PostToolUse", None, true),
    ("PostToolUseFailure", None, true),
    // Auto mode refusing a permission: otherwise the run reads "working"
    // while the agent is being stopped.
    ("PermissionDenied", None, true),
    // An MCP server asking the user, at once; the `elicitation_dialog`
    // notification below is the late backstop.
    ("Elicitation", None, true),
    // Fires when a person answers the elicitation in their terminal, so the
    // inbox stops asking.
    ("ElicitationResult", None, true),
    // The hook settings were edited, or a managed policy blocks loopback;
    // otherwise that only shows as a quiet channel.
    ("ConfigChange", None, true),
    // An observed session's own task list. No matcher support.
    ("TaskCreated", None, true),
    ("TaskCompleted", None, true),
    (
        "Notification",
        Some("permission_prompt|idle_prompt|elicitation_dialog|elicitation_url_dialog"),
        true,
    ),
    ("Stop", None, true),
    ("StopFailure", None, true),
    ("SubagentStart", None, true),
    ("SubagentStop", None, true),
    ("CwdChanged", None, true),
    // `PostCompact`, not `PreCompact`: resetting the gauge before compaction
    // would flicker `context_high` while the window is still full.
    ("PostCompact", None, true),
    // A `/model` switch changes the window the gauge is a percentage of.
    ("PostModelSwitch", None, true),
    // Names the model before the switch, so the next turn is priced right.
    ("PreModelSwitch", None, true),
    ("SessionEnd", None, true),
    ("PermissionRequest", None, false),
];

/// What a connect or disconnect did, so the CLI can report it honestly.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct ConnectReport {
    pub hooks_added: usize,
    pub hooks_removed: usize,
    pub telemetry: TelemetryStatus,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryStatus {
    /// Devplane owns the export.
    Configured,
    /// Someone else configured an exporter; Devplane left it alone.
    External,
    /// Removed by disconnect.
    Removed,
    #[default]
    Untouched,
}

/// Where Claude Code keeps its user settings.
pub fn settings_path() -> Result<PathBuf> {
    let base = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs_home().map(|h| h.join(".claude")))
        .context("cannot determine the Claude Code config directory")?;
    Ok(base.join("settings.json"))
}

/// Where this platform's **managed** settings live, as the vendor documents it.
///
/// The documented locations only, one per platform (the vendor removed the
/// Windows `ProgramData` fallback). Drop-ins, policy helpers, the Windows
/// registry and SDK hosts are not resolved, so an absent timer means "nothing
/// in the files read", not "none".
pub fn managed_settings_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/ClaudeCode/managed-settings.json")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\Program Files\ClaudeCode\managed-settings.json")
    } else {
        PathBuf::from("/etc/claude-code/managed-settings.json")
    }
}

/// What can answer a question on this machine without the person.
///
///
/// Absent where nothing sets it, and where a file will not parse: the vendor
/// says such a document has none of its settings in effect.
pub fn question_clock() -> Option<crate::core::clock::QuestionClock> {
    let user_path = settings_path().ok()?;
    let user = read_settings(&user_path).unwrap_or_default();
    let managed_path = managed_settings_path();
    let managed = read_settings(&managed_path).unwrap_or_default();
    crate::core::clock::question_clock(
        &user,
        &user_path.display().to_string(),
        &managed,
        &managed_path.display().to_string(),
    )
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Reads user settings, tolerating an absent file but not a malformed one:
/// discarding what we could not parse would delete the user's configuration.
pub fn read_settings(path: &Path) -> Result<Map<String, Value>> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(Map::new()),
        Ok(s) => serde_json::from_str(&s)
            .with_context(|| format!("{} is not valid JSON; not touching it", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// The settings as they would be written: what a diff is taken over.
pub fn render_settings(settings: &Map<String, Value>) -> String {
    serde_json::to_string_pretty(settings).unwrap_or_default() + "\n"
}

/// Writes settings back, preserving a backup of what was there before.
pub fn write_settings(path: &Path, settings: &Map<String, Value>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    // Keep the file's mode on everything written in its place, backup
    // included: a `0600` settings file can hold an API key.
    let mode = std::fs::metadata(path).ok().map(|m| m.permissions());
    if path.exists() {
        let backup = path.with_extension("json.devplane-backup");
        std::fs::copy(path, &backup)
            .with_context(|| format!("backing up {} first", path.display()))?;
        if let Some(p) = &mode {
            std::fs::set_permissions(&backup, p.clone()).ok();
        }
    }
    let body = render_settings(settings);
    // Temporary file and rename: an interrupted write never leaves half a
    // settings file.
    let tmp = path.with_extension("json.devplane-tmp");
    // A file that carries the bearer token is owner-only whatever it was:
    // the token opens the host's API to whoever reads it.
    let secret = body.contains("Authorization=Bearer");
    // Created owner-only when it carries the token, so it is never readable
    // by others even for the moment before the mode below is applied.
    let _ = std::fs::remove_file(&tmp);
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    if secret {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    {
        use std::io::Write;
        open.open(&tmp)
            .and_then(|mut f| f.write_all(body.as_bytes()))
            .with_context(|| format!("writing {}", tmp.display()))?;
    }
    if let Some(p) = mode {
        std::fs::set_permissions(&tmp, p)
            .with_context(|| format!("keeping the mode of {}", path.display()))?;
    }
    #[cfg(unix)]
    if secret {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("making {} owner-only", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = secret;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

/// A line diff of two renderings, in the unified style, with the unchanged
/// lines around a change kept for orientation.
///
/// A small hand-rolled LCS; settings files are a few hundred lines. Empty when
/// the two are equal, so a caller can say *nothing to change*.
pub fn diff(before: &str, after: &str) -> String {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    // lcs[i][j]: the longest common subsequence of a[i..] and b[j..].
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // Walk it into a line-by-line script: ' ' kept, '-' removed, '+' added.
    let mut script: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            script.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            script.push(('-', a[i]));
            i += 1;
        } else {
            script.push(('+', b[j]));
            j += 1;
        }
    }
    script.extend(a[i..].iter().map(|l| ('-', *l)));
    script.extend(b[j..].iter().map(|l| ('+', *l)));

    if script.iter().all(|(op, _)| *op == ' ') {
        return String::new();
    }
    // Only the neighbourhood of a change is printed: three lines of context
    // either side, and a `…` where unchanged lines were left out.
    const CONTEXT: usize = 3;
    let changed: Vec<usize> = script
        .iter()
        .enumerate()
        .filter(|(_, (op, _))| *op != ' ')
        .map(|(k, _)| k)
        .collect();
    let mut out = String::new();
    let mut last_printed: Option<usize> = None;
    for (k, (op, line)) in script.iter().enumerate() {
        let near = changed.iter().any(|c| k.abs_diff(*c) <= CONTEXT);
        if !near {
            continue;
        }
        if let Some(p) = last_printed
            && k > p + 1
        {
            out.push_str("…\n");
        }
        out.push(*op);
        out.push(' ');
        out.push_str(line);
        out.push('\n');
        last_printed = Some(k);
    }
    out
}

/// Adds Devplane's hooks and telemetry configuration.
///
/// `exe` is the absolute path of the `devplane` binary every hook runs.
pub fn connect(
    settings: &mut Map<String, Value>,
    base_url: &str,
    token: &str,
    exe: &Path,
) -> ConnectReport {
    let mut report = ConnectReport::default();

    // ---- hooks -------------------------------------------------------------
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut();
    let Some(hooks) = hooks else {
        report
            .notes
            .push("`hooks` in settings.json is not an object; left alone".into());
        return report;
    };

    // `SessionStart` only accepts `command` and `mcp_tool` hooks. Without it
    // a session is invisible until it does something.
    {
        let entry = json!({
            "hooks": [{
                "type": "command",
                "command": format!("{} hook", shell_quote(exe)),
                "async": true,
            }]
        });
        let list = hooks
            .entry("SessionStart".to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut();
        if let Some(list) = list {
            list.retain(|e| !is_ours(e));
            list.push(entry);
            report.hooks_added += 1;
        }
    }

    for (event, matcher, is_async) in HOOKS {
        // Every event runs the binary, none uses HTTP: an HTTP hook needs a
        // listening host, and `async` exists only for `command` hooks. The
        // binary writes to the store; a host folds it in when there is one.
        let entry = if *event == "PermissionRequest" {
            gate_entry(exe, *matcher, crate::observe::hook::HOLD_TIMEOUT_SECS)
        } else if *event == "PreToolUse" {
            gate_entry(exe, *matcher, crate::observe::hook::GATE_TIMEOUT_SECS)
        } else {
            observe_entry(exe, *matcher, *is_async)
        };
        let list = hooks
            .entry(event.to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut();
        let Some(list) = list else { continue };
        // Replace ours if present, so reconnecting updates the path.
        list.retain(|e| !is_ours(e));
        list.push(entry);
        report.hooks_added += 1;
    }

    // ---- telemetry ---------------------------------------------------------
    let env = settings
        .entry("env")
        .or_insert_with(|| json!({}))
        .as_object_mut();
    let Some(env) = env else {
        report
            .notes
            .push("`env` in settings.json is not an object; telemetry not configured".into());
        return report;
    };

    // Somebody else's collector is any endpoint without our marker — a
    // collector on loopback included. In the file and in this process's
    // environment alike.
    // The logs-only endpoint takes precedence over the general one for the
    // records this sends, so it is somebody else's collector too.
    let is_foreign = |endpoint: &str| !endpoint.is_empty() && !endpoint.contains(URL_MARKER);
    let foreign = [
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
    ]
    .iter()
    .any(|key| {
        env.get(*key)
            .and_then(|v| v.as_str())
            .is_some_and(is_foreign)
            || std::env::var(key).ok().as_deref().is_some_and(is_foreign)
    });

    if foreign {
        report.telemetry = TelemetryStatus::External;
        report.notes.push(
            "telemetry already exports to another collector; left alone. Cost and context \
             figures will be unavailable."
                .into(),
        );
    } else {
        // Log records only; metrics would repeat them. The two
        // `OTEL_METRICS_INCLUDE_*` variables stay: despite the name, the vendor
        // gates `app.entrypoint` and repository attributes on events with them.
        for (k, v) in [
            ("CLAUDE_CODE_ENABLE_TELEMETRY", "1"),
            ("OTEL_LOGS_EXPORTER", "otlp"),
            ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/json"),
            ("OTEL_LOGS_EXPORT_INTERVAL", "2000"),
            ("OTEL_METRICS_INCLUDE_ENTRYPOINT", "true"),
            ("OTEL_METRICS_INCLUDE_REPOSITORY", "true"),
        ] {
            env.insert(k.into(), json!(v));
        }
        // A metrics exporter switched on earlier goes with it.
        env.remove("OTEL_METRICS_EXPORTER");
        env.remove("OTEL_METRIC_EXPORT_INTERVAL");
        env.insert(
            "OTEL_EXPORTER_OTLP_ENDPOINT".into(),
            json!(format!("{base_url}/devplane/otel")),
        );
        // The telemetry endpoints are authenticated. This branch runs only
        // when no foreign collector is configured, so the bearer header
        // reaches exactly one collector: ours. The vendor exports this block
        // to every tool call, so `token` is the telemetry-only one — it
        // cannot answer a permission or reach any other route.
        env.insert(
            "OTEL_EXPORTER_OTLP_HEADERS".into(),
            json!(format!("Authorization=Bearer {token}")),
        );
        report.telemetry = TelemetryStatus::Configured;
    }

    report
}

/// Wraps the user's status line so rate limits reach the store.
///
/// Optional and off by default: it takes over a command the user configured.
/// The wrapper forwards the sample, then runs the original with the same
/// input, so the status line on screen is unchanged.
pub fn wrap_status_line(settings: &mut Map<String, Value>, exe: &Path) -> String {
    let existing = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    if existing.as_deref().map(is_shim_command).unwrap_or(false) {
        return "status line already wrapped".into();
    }

    let command = match &existing {
        Some(orig) => format!("{} statusline --then {}", shell_quote(exe), shell_arg(orig)),
        None => format!("{} statusline", shell_quote(exe)),
    };
    settings.insert(
        "statusLine".into(),
        json!({ "type": "command", "command": command }),
    );
    match existing {
        Some(_) => "wrapped your existing status line".into(),
        None => "installed a status line (it prints nothing of its own)".into(),
    }
}

/// Quotes an arbitrary string as one shell argument.
fn shell_arg(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Undoes [`shell_arg`]: exactly one quote off each end, then every `'\''`
/// back to `'`. Not `trim_matches`, which would eat a quote that belongs to the
/// original (`echo 'hi'`). Anything not shaped like our quoting is returned
/// as it is.
fn unshell_arg(s: &str) -> String {
    match s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        Some(inner) => inner.replace("'\\''", "'"),
        None => s.to_string(),
    }
}

/// Removes everything `connect` added, and nothing else.
pub fn disconnect(settings: &mut Map<String, Value>) -> ConnectReport {
    let mut report = ConnectReport::default();

    if let Some(Value::Object(hooks)) = settings.get_mut("hooks") {
        let events: Vec<String> = hooks.keys().cloned().collect();
        for event in events {
            if let Some(Value::Array(list)) = hooks.get_mut(&event) {
                let before = list.len();
                list.retain(|e| !is_ours(e));
                report.hooks_removed += before - list.len();
                if list.is_empty() {
                    hooks.remove(&event);
                }
            }
        }
        if hooks.is_empty() {
            settings.remove("hooks");
        }
    }

    // Unwrap the status line, restoring the original: a shim pointing at a
    // removed binary would silently stop it updating.
    if let Some(cmd) = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        && is_shim_command(&cmd)
    {
        match cmd.split_once(" --then ") {
            Some((_, original)) => {
                let original = unshell_arg(original.trim());
                settings.insert(
                    "statusLine".into(),
                    json!({ "type": "command", "command": original }),
                );
                report
                    .notes
                    .push("restored your original status line".into());
            }
            None => {
                settings.remove("statusLine");
            }
        }
    }

    if let Some(Value::Object(env)) = settings.get_mut("env") {
        // Only remove telemetry we pointed at ourselves.
        let ours = env
            .get("OTEL_EXPORTER_OTLP_ENDPOINT")
            .and_then(|v| v.as_str())
            .map(|v| v.contains(URL_MARKER))
            .unwrap_or(false);
        if ours {
            for k in OTEL_VARS {
                env.remove(*k);
            }
            report.telemetry = TelemetryStatus::Removed;
        }
        if env.is_empty() {
            settings.remove("env");
        }
    }

    report
}

/// Whether a hook entry is one Devplane wrote.
///
///
/// A command entry by the shim it runs, an HTTP entry by the URL marker in it.
/// Anything else belongs to the user and is never touched.
pub(crate) fn is_ours(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hs| {
            hs.iter().any(|h| {
                let url_is_ours = h
                    .get("url")
                    .and_then(|u| u.as_str())
                    .map(|u| u.contains(URL_MARKER))
                    .unwrap_or(false);
                let cmd_is_ours = h
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(is_shim_command)
                    .unwrap_or(false);
                url_is_ours || cmd_is_ours
            })
        })
        .unwrap_or(false)
}

/// Recognises one of our shims regardless of where the binary lives, without
/// matching a user's own script that merely mentions the word.
fn is_shim_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    ["hook", "statusline"].iter().any(|verb| {
        trimmed
            .split_once(&format!(" {verb}"))
            .map(|(head, tail)| {
                let head = head.trim().trim_matches('\'').trim_matches('"');
                let is_ours = std::path::Path::new(head)
                    .file_name()
                    .map(|f| f == "devplane" || f == "devplane.exe")
                    .unwrap_or(false);
                // `statusline --then ...` has a tail; `hook` must not, or a
                // script called `hook-something` would match.
                is_ours && (tail.is_empty() || tail.starts_with(' ') || tail.starts_with(" --"))
            })
            .unwrap_or(false)
    })
}

/// Quotes a path for a shell command line.
pub(crate) fn shell_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    if s.chars().all(|c| c.is_alphanumeric() || "/._-".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// The deciding hook: this binary, reading the payload on stdin and answering
/// on stdout, with no host in the path.
fn gate_entry(exe: &Path, matcher: Option<&str>, timeout: u64) -> Value {
    let mut hook = json!({
        "type": "command",
        "command": format!("{} hook", shell_quote(exe)),
        // See `GATE_TIMEOUT_SECS` and `HOLD_TIMEOUT_SECS`: short where the
        // hook only decides, longer than the longest hold where it may hold.
        "timeout": timeout,
    });
    if let Some(m) = matcher {
        hook["matcher"] = json!(m);
    }
    json!({ "hooks": [hook] })
}

/// An observation hook: this binary, writing what it saw to the store and
/// answering nothing. `async`, so no observation can make Claude feel slower.
fn observe_entry(exe: &Path, matcher: Option<&str>, is_async: bool) -> Value {
    let mut hook = json!({
        "type": "command",
        "command": format!("{} hook", shell_quote(exe)),
        "timeout": crate::observe::hook::GATE_TIMEOUT_SECS,
    });
    if is_async {
        hook["async"] = json!(true);
    }
    let mut entry = json!({ "hooks": [hook] });
    if let Some(m) = matcher {
        entry["matcher"] = json!(m);
    }
    entry
}

/// Reports what is currently installed, for `doctor`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ConnectState {
    pub settings_path: PathBuf,
    pub hooks_installed: Vec<String>,
    pub telemetry_endpoint: Option<String>,
    pub telemetry_is_ours: bool,
    /// Hooks of ours are installed, but the deciding ones are not the shape
    /// this build writes: a blocking `command` hook on `PreToolUse` and
    /// `PermissionRequest`, the latter outlasting the longest hold. "Installed"
    /// alone would be false reassurance; `doctor` says to reconnect.
    pub gate_is_stale: bool,
    /// Hook events of ours whose command names a binary that no longer
    /// exists, with that path. The vendor runs nothing for them, so the gate
    /// is off for that event however the settings read.
    pub gate_off: Vec<(String, String)>,
}

/// The binary one of our shim commands runs, unquoted as [`shell_quote`]
/// wrote it.
pub fn shim_binary(cmd: &str) -> Option<PathBuf> {
    let trimmed = cmd.trim();
    let head = ["hook", "statusline"]
        .iter()
        .find_map(|verb| trimmed.split_once(&format!(" {verb}")).map(|(h, _)| h))?
        .trim();
    Some(PathBuf::from(unshell_arg(head)))
}

/// Our hook events whose binary is gone, as `(event, path)`.
fn missing_binaries(settings: &Map<String, Value>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) else {
        return out;
    };
    for (event, entries) in hooks {
        let commands = entries
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| is_ours(e))
            .filter_map(|e| e.get("hooks").and_then(|h| h.as_array()))
            .flatten()
            .filter_map(|h| h.get("command").and_then(|c| c.as_str()))
            .filter(|c| is_shim_command(c));
        for cmd in commands {
            if let Some(bin) = shim_binary(cmd)
                && !bin.exists()
            {
                out.push((event.clone(), bin.display().to_string()));
                break;
            }
        }
    }
    out
}

/// Whether the entries for a deciding event contain a gate that can decide.
///
/// Not an `async` entry (cannot answer), not HTTP (a connection failure lets
/// the call through, so a stopped host means no prohibitions), and, for the
/// event a permission is held on, a timeout outlasting `outlast` seconds, or
/// the vendor kills the hold mid-question.
fn is_live_gate(entries: &Value, outlast: u64) -> bool {
    entries
        .as_array()
        .map(|list| {
            list.iter().filter(|e| is_ours(e)).any(|e| {
                e.get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hs| {
                        hs.iter().any(|h| {
                            h.get("async").is_none()
                                && h.get("timeout")
                                    .and_then(|t| t.as_u64())
                                    .is_none_or(|t| t > outlast)
                                && h.get("type").and_then(|t| t.as_str()) == Some("command")
                                && h.get("command")
                                    .and_then(|c| c.as_str())
                                    .map(is_shim_command)
                                    .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// The result of actually running the gate the way the provider runs it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct GateProbe {
    /// The command the settings file says to run.
    pub command: Option<String>,
    /// Whether it answered with a verdict for a call a rule covers.
    pub answered: bool,
    /// How long it took, in milliseconds.
    pub millis: u128,
    pub error: Option<String>,
}

/// Runs the installed gate against a call its own rules must refuse, and
/// reports whether it answered.
///
///
/// No hook can enforce its own presence, so detection has to run the thing: a
/// `Read` deny on a path nothing holds, in a temporary project, exercising
/// rule loading, path matching and the reply shape.
pub fn probe_gate(settings: &Map<String, Value>) -> GateProbe {
    let started = std::time::Instant::now();
    let command = settings
        .get("hooks")
        .and_then(|h| h.get("PreToolUse"))
        .and_then(|e| e.as_array())
        .and_then(|list| {
            list.iter().filter(|e| is_ours(e)).find_map(|e| {
                e.get("hooks")?
                    .as_array()?
                    .iter()
                    .find_map(|h| h.get("command")?.as_str().map(str::to_string))
            })
        });
    let Some(command) = command else {
        return GateProbe {
            command: None,
            answered: false,
            millis: 0,
            error: Some("no command gate is installed for PreToolUse".into()),
        };
    };

    let probe = match probe_dir() {
        Ok(d) => d,
        Err(e) => {
            return GateProbe {
                command: Some(command),
                answered: false,
                millis: 0,
                error: Some(e.to_string()),
            };
        }
    };
    let payload = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": crate::observe::hook::PROBE_SESSION,
        "cwd": probe.display().to_string(),
        "tool_name": "Bash",
        "tool_input": { "command": "cat .devplane-probe" },
    })
    .to_string();

    let out = run_gate(&command, &payload);
    std::fs::remove_dir_all(&probe).ok();
    let millis = started.elapsed().as_millis();

    match out {
        Err(e) => GateProbe {
            command: Some(command),
            answered: false,
            millis,
            error: Some(e),
        },
        Ok(text) => {
            let decided = serde_json::from_str::<Value>(text.trim())
                .ok()
                .and_then(|v| {
                    v.get("hookSpecificOutput")?
                        .get("permissionDecision")?
                        .as_str()
                        .map(str::to_string)
                })
                .is_some_and(|d| d == "deny");
            GateProbe {
                command: Some(command),
                answered: decided,
                millis,
                error: (!decided)
                    .then(|| format!("it ran and did not refuse a denied read; it said {text:?}")),
            }
        }
    }
}

/// A throwaway project whose only rule is the one the probe exercises.
fn probe_dir() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("devplane-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join(crate::core::config::CONFIG_FILE),
        "[project]\nname = \"devplane-probe\"\n\n[policy]\nnever_auto = [\"Read(.devplane-probe)\"]\n",
    )?;
    Ok(dir)
}

/// Runs the configured command exactly as the provider does: through a shell,
/// with the payload on stdin.
fn run_gate(command: &str, payload: &str) -> Result<String, String> {
    use std::io::Write;
    let mut child = shell_command(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("it will not start: {e}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("no stdin")?
        .write_all(payload.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "it exited {} — {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("no output")
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(windows)]
fn shell_command(command: &str) -> std::process::Command {
    let mut c = std::process::Command::new("cmd");
    c.arg("/C").arg(command);
    c
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> std::process::Command {
    let mut c = std::process::Command::new("sh");
    c.arg("-c").arg(command);
    c
}

pub fn inspect(settings: &Map<String, Value>, path: &Path) -> ConnectState {
    let hooks_installed: Vec<String> = settings
        .get("hooks")
        .and_then(|h| h.as_object())
        .map(|hooks| {
            hooks
                .iter()
                .filter(|(_, v)| v.as_array().map(|a| a.iter().any(is_ours)).unwrap_or(false))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();

    let telemetry_endpoint = settings
        .get("env")
        .and_then(|e| e.get("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let hooks = settings.get("hooks");
    let gate_is_stale = !hooks_installed.is_empty()
        && [
            ("PreToolUse", 0),
            (
                "PermissionRequest",
                crate::core::config::Hold::CEILING.as_secs(),
            ),
        ]
        .iter()
        .any(|(e, outlast)| {
            !hooks
                .and_then(|h| h.get(*e))
                .is_some_and(|entries| is_live_gate(entries, *outlast))
        });

    ConnectState {
        gate_off: missing_binaries(settings),
        gate_is_stale,
        settings_path: path.to_path_buf(),
        telemetry_is_ours: telemetry_endpoint
            .as_deref()
            .map(|e| e.contains(URL_MARKER))
            .unwrap_or(false),
        hooks_installed,
        telemetry_endpoint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> Map<String, Value> {
        let mut s = Map::new();
        connect(
            &mut s,
            "http://127.0.0.1:47831",
            "tok",
            Path::new("/usr/local/bin/devplane"),
        );
        s
    }

    fn connect_test(s: &mut Map<String, Value>, url: &str, token: &str) -> ConnectReport {
        connect(s, url, token, Path::new("/usr/local/bin/devplane"))
    }

    /// A hook whose binary was removed (an npx cache cleared, a moved
    /// install) runs nothing: `doctor` names the event as gate off.
    #[test]
    fn a_hook_whose_binary_is_gone_is_reported_as_gate_off() {
        let mut gone = Map::new();
        connect(
            &mut gone,
            "http://127.0.0.1:47831",
            "tok",
            Path::new("/nonexistent/it's gone/devplane"),
        );
        let state = inspect(&gone, Path::new("/tmp/settings.json"));
        let events: Vec<&str> = state.gate_off.iter().map(|(e, _)| e.as_str()).collect();
        assert!(events.contains(&"PreToolUse"), "{events:?}");
        assert!(events.contains(&"PermissionRequest"), "{events:?}");
        assert!(
            state
                .gate_off
                .iter()
                .all(|(_, p)| p == "/nonexistent/it's gone/devplane"),
            "the path is unquoted back: {:?}",
            state.gate_off
        );
        // One that exists is not reported.
        let exe = std::env::current_exe().unwrap();
        let mut here = Map::new();
        connect(&mut here, "http://127.0.0.1:47831", "tok", &exe);
        assert!(inspect(&here, Path::new("/tmp/s.json")).gate_off.is_empty());
    }

    #[test]
    fn connect_installs_every_hook_once() {
        let s = connected();
        let hooks = s["hooks"].as_object().unwrap();
        assert_eq!(
            hooks.len(),
            HOOKS.len() + 1,
            "the listed hooks plus SessionStart"
        );
        assert!(hooks.contains_key("PermissionRequest"));
    }

    #[test]
    fn session_start_uses_a_command_hook_because_http_is_ignored_there() {
        // An HTTP `SessionStart` entry is written and then never runs.
        let s = connected();
        let hook = &s["hooks"]["SessionStart"][0]["hooks"][0];
        assert_eq!(hook["type"], json!("command"));
        assert!(hook["command"].as_str().unwrap().ends_with(" hook"));
        assert_eq!(hook["async"], json!(true), "and must not delay startup");
    }

    #[test]
    fn the_command_hook_is_recognised_as_ours_on_disconnect() {
        let mut s = connected();
        let report = disconnect(&mut s);
        assert_eq!(report.hooks_removed, HOOKS.len() + 1);
        assert!(s.get("hooks").is_none());
    }

    #[test]
    fn a_users_own_command_hook_is_not_mistaken_for_ours() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"hooks": {"SessionStart": [{"hooks": [
                 {"type":"command","command":"/opt/mine/notify-on-hook"}]}]}}"#,
        )
        .unwrap();
        connect_test(&mut s, "http://127.0.0.1:1", "t");
        disconnect(&mut s);
        let list = s["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0]["hooks"][0]["command"],
            json!("/opt/mine/notify-on-hook")
        );
    }

    #[test]
    fn worktree_create_is_never_installed() {
        // It replaces Claude Code's own `git worktree` logic machine-wide.
        let s = connected();
        assert!(
            !s["hooks"]
                .as_object()
                .unwrap()
                .contains_key("WorktreeCreate")
        );
    }

    #[test]
    fn a_hook_set_from_before_the_second_gate_is_reported_as_stale() {
        // An async entry cannot decide, so every prohibition is inert in auto
        // mode; "installed" must not hide that.
        let mut old: Map<String, Value> = serde_json::from_str(
            r#"{"hooks": {"PreToolUse": [{"hooks": [{"type":"http",
                 "url":"http://127.0.0.1:47831/devplane/hook","async":true}]}]}}"#,
        )
        .unwrap();
        let state = inspect(&old, std::path::Path::new("/tmp/settings.json"));
        assert!(
            state.gate_is_stale,
            "an async observation hook is not a gate"
        );

        // An HTTP gate decides only while a host listens; a connection
        // failure is a silent non-blocking error.
        let http_gate: Map<String, Value> = serde_json::from_str(
            r#"{"hooks": {"PreToolUse": [{"hooks": [{"type":"http",
                 "url":"http://127.0.0.1:47831/devplane/policy","timeout":5}]}]},
                "PermissionRequest": [{"hooks": [{"type":"http",
                 "url":"http://127.0.0.1:47831/devplane/policy","timeout":5}]}]}"#,
        )
        .unwrap();
        assert!(
            inspect(&http_gate, std::path::Path::new("/tmp/settings.json")).gate_is_stale,
            "an HTTP gate is absent whenever the host is"
        );

        connect_test(&mut old, "http://127.0.0.1:47831", "t");
        let state = inspect(&old, std::path::Path::new("/tmp/settings.json"));
        assert!(!state.gate_is_stale, "reconnecting has to actually fix it");
    }

    #[test]
    fn exactly_the_two_gate_hooks_block_and_they_reach_the_policy() {
        // `PreToolUse` is the only event that fires in auto mode; without it a
        // `never_auto` rule never runs there. Everything else is async.
        let s = connected();
        let mut blocking = Vec::new();
        for (event, entries) in s["hooks"].as_object().unwrap() {
            let hook = &entries[0]["hooks"][0];
            if matches!(event.as_str(), "PermissionRequest" | "PreToolUse") {
                assert!(hook.get("async").is_none(), "{event} must block");
                assert!(hook.get("timeout").is_some(), "{event} needs a deadline");
                // Not HTTP: a connection failure is non-blocking, so an HTTP
                // gate is silently off whenever the host is.
                assert_eq!(
                    hook["type"],
                    json!("command"),
                    "{event} must not need a host"
                );
                assert!(
                    hook["command"].as_str().unwrap().ends_with(" hook"),
                    "{event} must run the binary that decides"
                );
                blocking.push(event.clone());
            } else {
                assert_eq!(hook["async"], json!(true), "{event} would slow Claude down");
            }
        }
        blocking.sort();
        assert_eq!(blocking, vec!["PermissionRequest", "PreToolUse"]);
    }

    /// The vendor's timeout for the event a permission is held on exceeds
    /// the longest hold, and a five-second hold gate reads as stale.
    #[test]
    fn the_holding_hook_outlasts_the_longest_hold() {
        let s = connected();
        let timeout = |e: &str| s["hooks"][e][0]["hooks"][0]["timeout"].as_u64().unwrap();
        let ceiling = crate::core::config::Hold::CEILING.as_secs();
        assert!(
            timeout("PermissionRequest") > ceiling,
            "a hold the vendor kills part-way is a question that never ends"
        );
        assert!(
            timeout("PreToolUse") < ceiling,
            "the hook that only decides stays short"
        );
        let mut old = s.clone();
        old["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"] = json!(5);
        assert!(
            inspect(&old, std::path::Path::new("/tmp/settings.json")).gate_is_stale,
            "a holding gate the vendor would kill mid-hold is not a live gate"
        );
    }

    #[test]
    fn existing_hooks_are_preserved() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"hooks": {"PreToolUse": [{"hooks": [{"type":"command","command":"mine.sh"}]}]}}"#,
        )
        .unwrap();
        connect_test(&mut s, "http://127.0.0.1:1", "t");
        let list = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(list.len(), 2, "the user's own hook must survive");
        assert_eq!(list[0]["hooks"][0]["command"], json!("mine.sh"));
    }

    #[test]
    fn reconnecting_replaces_rather_than_duplicates() {
        let mut s = connected();
        connect_test(&mut s, "http://127.0.0.1:99999", "t2");
        let list = s["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["hooks"][0]["type"], json!("command"));
        assert!(
            list[0]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .ends_with("devplane hook")
        );
    }

    #[test]
    fn disconnect_removes_ours_and_leaves_theirs() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"hooks": {"PreToolUse": [{"hooks": [{"type":"command","command":"mine.sh"}]}]}}"#,
        )
        .unwrap();
        connect_test(&mut s, "http://127.0.0.1:1", "t");
        let report = disconnect(&mut s);
        assert_eq!(report.hooks_removed, HOOKS.len() + 1);
        let list = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["hooks"][0]["command"], json!("mine.sh"));
        assert!(s.get("env").is_none(), "our telemetry vars are gone");
    }

    #[test]
    fn an_absent_allowlist_is_not_created() {
        // Creating it would restrict every other HTTP hook on the machine.
        let s = connected();
        assert!(s.get("allowedHttpHookUrls").is_none());
    }

    #[test]
    fn a_foreign_collector_is_never_hijacked() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://collector.corp:4317"}}"#,
        )
        .unwrap();
        let report = connect_test(&mut s, "http://127.0.0.1:1", "t");
        assert_eq!(report.telemetry, TelemetryStatus::External);
        assert_eq!(
            s["env"]["OTEL_EXPORTER_OTLP_ENDPOINT"],
            json!("http://collector.corp:4317")
        );
        assert!(!report.notes.is_empty(), "and the user is told why");
    }

    /// A collector on this machine is somebody else's too: only the endpoint
    /// with the marker is ours.
    #[test]
    fn a_local_collector_that_is_not_ours_is_left_alone() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:4318"}}"#,
        )
        .unwrap();
        let report = connect_test(&mut s, "http://127.0.0.1:1", "t");
        assert_eq!(report.telemetry, TelemetryStatus::External);
        assert_eq!(
            s["env"]["OTEL_EXPORTER_OTLP_ENDPOINT"],
            json!("http://localhost:4318"),
            "a collector on loopback was taken over"
        );

        // Ours, from an earlier connect, is ours to replace.
        let mut ours: Map<String, Value> = serde_json::from_str(
            r#"{"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://127.0.0.1:47831/devplane/otel"}}"#,
        )
        .unwrap();
        let report = connect_test(&mut ours, "http://127.0.0.1:1", "t");
        assert_eq!(report.telemetry, TelemetryStatus::Configured);
        assert_eq!(
            ours["env"]["OTEL_EXPORTER_OTLP_ENDPOINT"],
            json!("http://127.0.0.1:1/devplane/otel")
        );
    }

    /// Log records only; a metrics exporter enabled earlier is taken back.
    #[test]
    fn the_metrics_exporter_is_not_enabled_and_an_old_one_is_removed() {
        let s = connected();
        let env = s["env"].as_object().unwrap();
        assert!(!env.contains_key("OTEL_METRICS_EXPORTER"));
        assert!(!env.contains_key("OTEL_METRIC_EXPORT_INTERVAL"));
        assert_eq!(env["OTEL_LOGS_EXPORTER"], json!("otlp"));
        // The attribute gates, which the vendor applies to events as well.
        assert_eq!(env["OTEL_METRICS_INCLUDE_ENTRYPOINT"], json!("true"));

        let mut old: Map<String, Value> = serde_json::from_str(
            r#"{"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://127.0.0.1:1/devplane/otel",
                        "OTEL_METRICS_EXPORTER": "otlp", "OTEL_METRIC_EXPORT_INTERVAL": "10000"}}"#,
        )
        .unwrap();
        connect_test(&mut old, "http://127.0.0.1:1", "t");
        assert!(
            !old["env"]
                .as_object()
                .unwrap()
                .contains_key("OTEL_METRICS_EXPORTER")
        );
    }

    /// The diff a person is shown before their settings file changes.
    #[test]
    fn the_diff_shows_exactly_the_changed_keys() {
        let mut s: Map<String, Value> =
            serde_json::from_str(r#"{"model": "opus", "theme": "dark"}"#).unwrap();
        let before = render_settings(&s);
        assert_eq!(diff(&before, &before), "", "nothing changed, nothing shown");

        connect_test(&mut s, "http://127.0.0.1:1", "t");
        let after = render_settings(&s);
        let d = diff(&before, &after);
        assert!(d.contains("+   \"hooks\": {"), "{d}");
        assert!(d.contains("+     \"OTEL_LOGS_EXPORTER\": \"otlp\","), "{d}");
        // `theme` legitimately gains a trailing comma; `model` is untouched
        // and must not appear as removed.
        assert!(
            !d.lines()
                .any(|l| l.starts_with("- ") && l.contains("model")),
            "an untouched key was shown as removed:\n{d}"
        );
        // Removed lines read as removed, on the way back out.
        disconnect(&mut s);
        let back = diff(&after, &render_settings(&s));
        assert!(back.contains("-   \"hooks\": {"), "{back}");
        assert!(!back.contains("+   \"hooks\""), "{back}");
    }

    /// The file keeps its mode, and so does its backup: a `0600` settings
    /// file can hold an API key.
    #[cfg(unix)]
    #[test]
    fn writing_keeps_the_files_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vp-test-mode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, r#"{"model":"opus"}"#).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();

        let mut s = read_settings(&p).unwrap();
        connect_test(&mut s, "http://127.0.0.1:1", "t");
        write_settings(&p, &s).unwrap();

        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&p), 0o600, "the rewritten file lost its mode");
        assert_eq!(
            mode(&p.with_extension("json.devplane-backup")),
            0o600,
            "the backup is world-readable"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disconnect_leaves_a_foreign_collector_alone() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"env": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://collector.corp:4317",
                        "CLAUDE_CODE_ENABLE_TELEMETRY": "1"}}"#,
        )
        .unwrap();
        disconnect(&mut s);
        assert_eq!(s["env"]["CLAUDE_CODE_ENABLE_TELEMETRY"], json!("1"));
    }

    /// The exporter carries the bearer; with no foreign collector there is
    /// exactly one place the header can go.
    #[test]
    fn telemetry_is_configured_with_the_bearer_and_disconnect_takes_it_back() {
        let mut settings = Map::new();
        let report = connect(
            &mut settings,
            "http://127.0.0.1:7777",
            "tok-abc",
            Path::new("/usr/local/bin/devplane"),
        );
        assert_eq!(report.telemetry, TelemetryStatus::Configured);
        assert_eq!(
            settings["env"]["OTEL_EXPORTER_OTLP_HEADERS"], "Authorization=Bearer tok-abc",
            "the exporter was pointed at an authenticated endpoint with no credential"
        );

        disconnect(&mut settings);
        let env = settings.get("env").and_then(|v| v.as_object());
        assert!(
            env.is_none_or(|e| !e.contains_key("OTEL_EXPORTER_OTLP_HEADERS")),
            "a bearer for a host that is gone was left in the settings file"
        );
    }

    #[test]
    fn telemetry_never_carries_prompt_text() {
        // The flags that would include prompts and responses must never be set.
        let s = connected();
        let env = s["env"].as_object().unwrap();
        for forbidden in [
            "OTEL_LOG_USER_PROMPTS",
            "OTEL_LOG_ASSISTANT_RESPONSES",
            "OTEL_LOG_TOOL_CONTENT",
            "OTEL_LOG_TOOL_DETAILS",
        ] {
            assert!(!env.contains_key(forbidden), "{forbidden} must stay unset");
        }
    }

    #[test]
    fn wrapping_a_status_line_preserves_the_original() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"statusLine": {"type": "command", "command": "~/bin/my-line.sh --fancy"}}"#,
        )
        .unwrap();
        wrap_status_line(&mut s, Path::new("/usr/local/bin/devplane"));
        let cmd = s["statusLine"]["command"].as_str().unwrap();
        assert!(cmd.starts_with("/usr/local/bin/devplane statusline --then "));
        assert!(cmd.contains("my-line.sh --fancy"));

        // And disconnecting gives it back exactly.
        disconnect(&mut s);
        assert_eq!(
            s["statusLine"]["command"],
            json!("~/bin/my-line.sh --fancy")
        );
    }

    #[test]
    fn wrapping_twice_is_not_nesting() {
        let mut s = Map::new();
        wrap_status_line(&mut s, Path::new("/usr/local/bin/devplane"));
        let once = s["statusLine"]["command"].as_str().unwrap().to_string();
        wrap_status_line(&mut s, Path::new("/usr/local/bin/devplane"));
        assert_eq!(s["statusLine"]["command"].as_str().unwrap(), once);
    }

    #[test]
    fn a_quoted_status_line_survives_connect_and_disconnect() {
        for original in [
            "echo 'hi'",
            "jq -r '.model.display_name'",
            "'quoted at both ends'",
            "it's",
        ] {
            let mut s: Map<String, Value> = serde_json::from_value(
                json!({"statusLine": {"type": "command", "command": original}}),
            )
            .unwrap();
            wrap_status_line(&mut s, Path::new("/usr/local/bin/devplane"));
            disconnect(&mut s);
            assert_eq!(s["statusLine"]["command"], json!(original), "{original}");
        }
    }

    #[test]
    fn a_status_line_we_did_not_install_is_left_alone() {
        let mut s: Map<String, Value> = serde_json::from_str(
            r#"{"statusLine": {"type": "command", "command": "starship prompt"}}"#,
        )
        .unwrap();
        disconnect(&mut s);
        assert_eq!(s["statusLine"]["command"], json!("starship prompt"));
    }

    #[test]
    fn malformed_settings_are_refused_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("vp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, "{ this is not json").unwrap();
        assert!(read_settings(&p).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writing_keeps_a_backup_and_is_atomic() {
        let dir = std::env::temp_dir().join(format!("vp-test-w-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, r#"{"model":"opus"}"#).unwrap();

        let mut s = read_settings(&p).unwrap();
        connect_test(&mut s, "http://127.0.0.1:1", "t");
        write_settings(&p, &s).unwrap();

        let back = read_settings(&p).unwrap();
        assert_eq!(back["model"], json!("opus"), "unrelated settings survive");
        assert!(back.contains_key("hooks"));
        assert!(p.with_extension("json.devplane-backup").exists());
        assert!(!p.with_extension("json.devplane-tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}

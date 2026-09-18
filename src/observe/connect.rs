//! Connecting and disconnecting Claude Code.
//!
//! The only part of Devplane that writes to a file the user owns, so the one
//! that has to be most careful. Five rules:
//!
//! 1. **Merge, never replace.** Hooks the user already configured keep working,
//!    and Devplane's entries are appended alongside them.
//! 2. **Remove exactly what was added.** Every entry carries a recognisable
//!    loopback URL, so disconnecting is subtraction rather than a guess.
//! 3. **Never take over a setting that is already doing a job.** If telemetry
//!    is already exported somewhere, Devplane does not redirect it; it reports
//!    that the channel belongs to someone else.
//! 4. **Never install a hook that replaces behaviour.** `WorktreeCreate`
//!    replaces Claude Code's own `git worktree` logic when configured, so an
//!    observer installing it would break `claude --worktree`, subagent
//!    isolation and background sessions everywhere.
//! 5. **Use the hook type each event actually supports.** `SessionStart`
//!    accepts only `command` and `mcp_tool` hooks: an HTTP entry there is
//!    accepted by the settings file and then silently never runs, which looks
//!    exactly like a session that started without telling anyone.

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// The marker in every URL Devplane writes. Disconnect removes entries whose
/// URL contains it and nothing else.
pub const URL_MARKER: &str = "/devplane/";

/// The environment variables Devplane sets for telemetry.
const OTEL_VARS: &[&str] = &[
    "CLAUDE_CODE_ENABLE_TELEMETRY",
    "OTEL_LOGS_EXPORTER",
    "OTEL_METRICS_EXPORTER",
    "OTEL_EXPORTER_OTLP_PROTOCOL",
    "OTEL_EXPORTER_OTLP_ENDPOINT",
    "OTEL_LOGS_EXPORT_INTERVAL",
    "OTEL_METRIC_EXPORT_INTERVAL",
    "OTEL_METRICS_INCLUDE_ENTRYPOINT",
    "OTEL_METRICS_INCLUDE_REPOSITORY",
];

/// The hook events Devplane subscribes to.
///
/// **Two are synchronous, and they answer different questions.**
///
/// `PermissionRequest` is the instant signal that a session is blocked — the
/// `permission_prompt` notification is six seconds late and, in a terminal,
/// deferred by every keystroke — and it carries the full verdict, because it
/// fires only when a human was going to be asked anyway.
///
/// `PreToolUse` fires before every tool call in every mode, which makes it the
/// only way a prohibition reaches a session in auto mode, where a classifier
/// approves routine calls and no prompt happens. It answers with a prohibition
/// or nothing; an allow there would skip the classifier too.
///
/// Everything else is `async`, so no hook can make Claude feel slower.
///
/// The list deliberately excludes `WorktreeCreate`; see the module docs.
const HOOKS: &[(&str, Option<&str>, bool)] = &[
    ("UserPromptSubmit", None, true),
    ("PreToolUse", None, false),
    ("PostToolUse", None, true),
    ("PostToolUseFailure", None, true),
    // The permission Claude Code's own auto mode refused, which is a decision
    // about this session that Devplane would otherwise never see: the run
    // stays "working" while the agent is being stopped from doing things. The
    // receiver has always known how to read it; nothing subscribed to it.
    ("PermissionDenied", None, true),
    // An MCP server asking the user something, at the moment it asks. The
    // `elicitation_dialog` notification below says the same thing about six
    // seconds later and, in a terminal, defers again on every keystroke — the
    // identical argument that makes `PermissionRequest` the hinge of the
    // design. It is kept as the backstop, not as the signal.
    ("Elicitation", None, true),
    // The other half of it, and the reason an answered question stops being
    // asked. It fires when a person answers the elicitation — in their own
    // terminal, where Devplane has no other way to learn the dialog closed.
    // Without it the inbox went on showing a decision that had been made,
    // which is what teaches somebody to skim the list.
    ("ElicitationResult", None, true),
    // Somebody edited the settings Devplane writes its hooks into, or a
    // managed policy arrived that blocks loopback. Every other symptom of that
    // is a channel going quiet, which is indistinguishable from a quiet
    // machine.
    ("ConfigChange", None, true),
    // An observed session's own task list — the plan a driven run gets over
    // the protocol and a watched one never had. No matcher support: these fire
    // on every occurrence.
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
    // `PostCompact`, not `PreCompact`: the context gauge is reset by this, and
    // resetting it *before* compaction meant the gauge dropped to zero while
    // the window was still full, then filled again from the summarisation
    // request — a `context_high` item that cleared itself and came back.
    ("PostCompact", None, true),
    // The model decides the size of the context window, so without this a
    // `/model` switch mid-session leaves the gauge a percentage of the window
    // the session started with.
    ("PostModelSwitch", None, true),
    // And its sequential sibling, which names the model *before* the switch —
    // so the next turn is not priced against the previous model's window.
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

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Reads user settings, tolerating an absent file but not a malformed one:
/// silently discarding a file we could not parse would delete the user's
/// configuration.
pub fn read_settings(path: &Path) -> Result<Map<String, Value>> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(Map::new()),
        Ok(s) => serde_json::from_str(&s)
            .with_context(|| format!("{} is not valid JSON; not touching it", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Writes settings back, preserving a backup of what was there before.
pub fn write_settings(path: &Path, settings: &Map<String, Value>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if path.exists() {
        let backup = path.with_extension("json.devplane-backup");
        std::fs::copy(path, &backup)
            .with_context(|| format!("backing up {} first", path.display()))?;
    }
    let body = serde_json::to_string_pretty(settings)? + "\n";
    // Write to a temporary file and rename, so an interrupted write cannot
    // leave the user with half a settings file and no Claude Code.
    let tmp = path.with_extension("json.devplane-tmp");
    std::fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

/// Adds Devplane's hooks and telemetry configuration.
///
/// `exe` is the absolute path of the `devplane` binary, used for the one hook
/// that cannot be delivered over HTTP.
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

    // `SessionStart` only accepts `command` and `mcp_tool` hooks, so it gets
    // the binary's own shim, which forwards the payload to the daemon. Without
    // it a session is invisible until it does something.
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
        // **The two deciding events ride a `command` hook; everything else
        // rides HTTP.** Claude Code walks past an unreachable HTTP hook
        // (*"Connection failure: non-blocking error, execution continues"*), so
        // an HTTP gate is off whenever the daemon is — announced only as a
        // generic hook error, once per tool call, naming no rule. A daemon is
        // often not running; a binary at a fixed path is not.
        //
        // The cost is a spawn per tool call, measured before it was chosen:
        // about 26 ms cold, against the ~50 ms this path already had over
        // loopback. Observation stays on HTTP — it is `async`, and a dropped
        // event costs history rather than a verdict.
        let entry = if matches!(*event, "PermissionRequest" | "PreToolUse") {
            gate_entry(exe, *matcher)
        } else {
            hook_entry(base_url, "hook", token, *matcher, *is_async)
        };
        let list = hooks
            .entry(event.to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut();
        let Some(list) = list else { continue };
        // Replace ours if it is already there, so reconnecting after a port
        // change updates the URL instead of adding a second copy.
        list.retain(|e| !is_ours(e));
        list.push(entry);
        report.hooks_added += 1;
    }

    // ---- the HTTP hook allowlist -------------------------------------------
    //
    // Defining this key restricts *every* HTTP hook on the machine to the
    // patterns it lists. Creating it would silently disable hooks the user
    // already has, so it is only extended when it already exists.
    match settings.get_mut("allowedHttpHookUrls") {
        Some(Value::Array(list)) => {
            let pattern = json!("http://127.0.0.1:*");
            if !list.contains(&pattern) {
                list.push(pattern);
                report
                    .notes
                    .push("added http://127.0.0.1:* to the existing allowedHttpHookUrls".into());
            }
        }
        Some(_) => report
            .notes
            .push("allowedHttpHookUrls is not an array; hooks may be blocked".into()),
        None => {} // Absent means every HTTP hook is allowed. Leave it that way.
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

    let foreign = env
        .get("OTEL_EXPORTER_OTLP_ENDPOINT")
        .and_then(|v| v.as_str())
        .map(|v| !v.contains("127.0.0.1") && !v.contains("localhost"))
        .unwrap_or(false)
        || std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok();

    if foreign {
        report.telemetry = TelemetryStatus::External;
        report.notes.push(
            "telemetry already exports to another collector; left alone. Cost and context \
             figures will be unavailable."
                .into(),
        );
    } else {
        for (k, v) in [
            ("CLAUDE_CODE_ENABLE_TELEMETRY", "1"),
            ("OTEL_LOGS_EXPORTER", "otlp"),
            ("OTEL_METRICS_EXPORTER", "otlp"),
            ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/json"),
            ("OTEL_LOGS_EXPORT_INTERVAL", "2000"),
            ("OTEL_METRIC_EXPORT_INTERVAL", "10000"),
            ("OTEL_METRICS_INCLUDE_ENTRYPOINT", "true"),
            ("OTEL_METRICS_INCLUDE_REPOSITORY", "true"),
        ] {
            env.insert(k.into(), json!(v));
        }
        env.insert(
            "OTEL_EXPORTER_OTLP_ENDPOINT".into(),
            json!(format!("{base_url}/devplane/otel")),
        );
        report.telemetry = TelemetryStatus::Configured;
    }

    report
}

/// Wraps the user's status line so rate limits reach the daemon.
///
/// Optional, and off by default, because it takes over a command the user
/// configured. The wrapper forwards the sample and then runs the original with
/// the same input, so the status line on screen is unchanged — if the shim
/// dies, the worst case is a status line that stops updating, which is why it
/// is not installed unless asked for.
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

    if let Some(Value::Array(list)) = settings.get_mut("allowedHttpHookUrls") {
        list.retain(|v| v.as_str() != Some("http://127.0.0.1:*"));
    }

    // Unwrap the status line, restoring whatever it was wrapping. Leaving a
    // shim behind that points at a daemon the user just removed would stop
    // their status line updating and give no clue why.
    if let Some(cmd) = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        && is_shim_command(&cmd)
    {
        match cmd.split_once(" --then ") {
            Some((_, original)) => {
                let original = original.trim().trim_matches('\'').replace("'\\''", "'");
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
        // Only remove telemetry we pointed at ourselves. A user who exports
        // elsewhere keeps their configuration.
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
/// HTTP entries are recognised by the marker in their URL; the one command
/// entry by the shim it runs. Anything else in the file belongs to the user and
/// is never touched.
fn is_ours(entry: &Value) -> bool {
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
fn shell_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    if s.chars().all(|c| c.is_alphanumeric() || "/._-".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// The deciding hook: this binary, reading the payload on stdin and answering
/// on stdout, with no daemon in the path.
fn gate_entry(exe: &Path, matcher: Option<&str>) -> Value {
    let mut hook = json!({
        "type": "command",
        "command": format!("{} hook", shell_quote(exe)),
        // Generous against a cold page cache on a spinning disk, and still a
        // bound. A gate slower than this is one nobody is waiting for: Claude
        // Code cancels it and prompts the person itself, which is the same
        // outcome as `undecided` and the right one.
        "timeout": 5,
    });
    if let Some(m) = matcher {
        hook["matcher"] = json!(m);
    }
    json!({ "hooks": [hook] })
}

fn hook_entry(
    base_url: &str,
    path: &str,
    token: &str,
    matcher: Option<&str>,
    is_async: bool,
) -> Value {
    let mut hook = json!({
        "type": "http",
        "url": format!("{base_url}/devplane/{path}"),
        "headers": { "Authorization": format!("Bearer {token}") },
    });
    if is_async {
        hook["async"] = json!(true);
    } else {
        // A policy decision that takes longer than this is a decision nobody is
        // making; Claude Code carries on and prompts the human itself.
        hook["timeout"] = json!(5);
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
    pub allowlist_blocks_us: bool,
    /// A hook set installed before the gate became two hooks, so prohibitions
    /// do not reach a session in auto mode.
    ///
    /// Worth its own field rather than a log line: "PreToolUse: installed" is
    /// the answer that reads as reassurance and provides none, which is the
    /// exact failure this whole layer is built to avoid. `doctor` says to
    /// reconnect.
    pub gate_is_stale: bool,
}

/// Whether an installed entry for `event` is the blocking, policy-routed shape
/// the gate needs. An `async` entry, or one pointing at the observation
/// endpoint, cannot decide anything.
/// Whether the entries for a deciding event contain a gate that can actually
/// decide.
///
/// Two ways this has been false while reading as installed, and both are here
/// because each was shipped. An **async** entry cannot answer at all. An
/// **HTTP** entry answers only while the daemon is up, and Claude Code
/// documents a connection failure as a non-blocking error that lets the call
/// through — so a stopped daemon is a machine with no prohibitions, announced
/// as a generic hook error once per tool call. A gate installed before the move
/// is stale in exactly the sense this field means: present, and not deciding.
fn is_live_gate(entries: &Value) -> bool {
    entries
        .as_array()
        .map(|list| {
            list.iter().filter(|e| is_ours(e)).any(|e| {
                e.get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hs| {
                        hs.iter().any(|h| {
                            h.get("async").is_none()
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
/// **No hook can enforce its own presence** — a timed-out one does not block,
/// and the vendor's reference says not to count on a stalled one to act as a
/// gate — so detection is the defence, and it has to *run the thing*. A
/// settings file containing the right line is evidence about a settings file.
///
/// The probe is a `Read` deny on a path nothing will hold, in a temporary
/// project of its own, so it exercises rule loading, path matching and the
/// reply shape without depending on what the user has written.
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

    // An allowlist that exists but does not cover loopback silently disables
    // every hook we installed, which otherwise looks like "Claude is quiet".
    let allowlist_blocks_us = match settings.get("allowedHttpHookUrls") {
        Some(Value::Array(list)) => !list.iter().any(|v| {
            v.as_str()
                .map(|s| s.contains("127.0.0.1") || s == "*")
                .unwrap_or(false)
        }),
        _ => false,
    };

    let hooks = settings.get("hooks");
    let gate_is_stale = !hooks_installed.is_empty()
        && ["PreToolUse", "PermissionRequest"].iter().any(|e| {
            !hooks
                .and_then(|h| h.get(*e))
                .map(is_live_gate)
                .unwrap_or(false)
        });

    ConnectState {
        gate_is_stale,
        settings_path: path.to_path_buf(),
        telemetry_is_ours: telemetry_endpoint
            .as_deref()
            .map(|e| e.contains(URL_MARKER))
            .unwrap_or(false),
        hooks_installed,
        telemetry_endpoint,
        allowlist_blocks_us,
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

    #[test]
    fn connect_installs_every_hook_once() {
        let s = connected();
        let hooks = s["hooks"].as_object().unwrap();
        assert_eq!(
            hooks.len(),
            HOOKS.len() + 1,
            "the HTTP hooks plus SessionStart"
        );
        assert!(hooks.contains_key("PermissionRequest"));
    }

    #[test]
    fn session_start_uses_a_command_hook_because_http_is_ignored_there() {
        // `SessionStart` accepts only `command` and `mcp_tool` hooks. An HTTP
        // entry is written happily and then never runs, which is how a session
        // can start without anyone hearing about it.
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
        // Configuring it replaces Claude Code's own `git worktree` logic, which
        // would break `claude --worktree` for every repository on the machine.
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
        // "PreToolUse: installed" is the answer that reads as reassurance and
        // provides none: an async entry pointing at the observation endpoint
        // cannot decide anything, so every prohibition is inert in auto mode
        // and nothing says so.
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

        // And an HTTP gate is stale too, for a subtler reason: it decides only
        // while the daemon is listening, and a connection failure is a
        // non-blocking error the session never shows. Installed, and off
        // whenever the daemon is.
        let http_gate: Map<String, Value> = serde_json::from_str(
            r#"{"hooks": {"PreToolUse": [{"hooks": [{"type":"http",
                 "url":"http://127.0.0.1:47831/devplane/policy","timeout":5}]}]},
                "PermissionRequest": [{"hooks": [{"type":"http",
                 "url":"http://127.0.0.1:47831/devplane/policy","timeout":5}]}]}"#,
        )
        .unwrap();
        assert!(
            inspect(&http_gate, std::path::Path::new("/tmp/settings.json")).gate_is_stale,
            "an HTTP gate is absent whenever the daemon is"
        );

        connect_test(&mut old, "http://127.0.0.1:47831", "t");
        let state = inspect(&old, std::path::Path::new("/tmp/settings.json"));
        assert!(!state.gate_is_stale, "reconnecting has to actually fix it");
    }

    #[test]
    fn exactly_the_two_gate_hooks_block_and_they_reach_the_policy() {
        // `PermissionRequest` answers a prompt that was going to be shown.
        // `PreToolUse` is the only event that fires in auto mode, where a
        // classifier approves silently and no prompt ever happens — without it
        // a project's `never_auto` rule does not run at all in that mode.
        // Everything else is async, because an observer has no business making
        // the work it observes feel slower.
        let s = connected();
        let mut blocking = Vec::new();
        for (event, entries) in s["hooks"].as_object().unwrap() {
            let hook = &entries[0]["hooks"][0];
            if matches!(event.as_str(), "PermissionRequest" | "PreToolUse") {
                assert!(hook.get("async").is_none(), "{event} must block");
                assert!(hook.get("timeout").is_some(), "{event} needs a deadline");
                // **Not HTTP.** Claude Code treats a connection failure as a
                // non-blocking error and carries on, so an HTTP gate is off
                // whenever the daemon is — silently. The binary is on disk
                // either way and decides in its own process.
                assert_eq!(
                    hook["type"],
                    json!("command"),
                    "{event} must not need a daemon"
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
        assert!(
            list[0]["hooks"][0]["url"]
                .as_str()
                .unwrap()
                .contains("99999")
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
        // Creating it would restrict every other HTTP hook on the machine to
        // the patterns we happened to list.
        let s = connected();
        assert!(s.get("allowedHttpHookUrls").is_none());
    }

    #[test]
    fn an_existing_allowlist_is_extended() {
        let mut s: Map<String, Value> =
            serde_json::from_str(r#"{"allowedHttpHookUrls": ["https://hooks.example.com/*"]}"#)
                .unwrap();
        connect_test(&mut s, "http://127.0.0.1:1", "t");
        let list = s["allowedHttpHookUrls"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|v| v == "https://hooks.example.com/*"));
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
    fn inspect_spots_an_allowlist_that_blocks_us() {
        let mut s: Map<String, Value> =
            serde_json::from_str(r#"{"allowedHttpHookUrls": ["https://hooks.example.com/*"]}"#)
                .unwrap();
        // Simulate a managed allowlist: connect cannot widen it.
        let state = inspect(&s, Path::new("/x"));
        assert!(state.allowlist_blocks_us);
        connect_test(&mut s, "http://127.0.0.1:1", "t");
        assert!(!inspect(&s, Path::new("/x")).allowlist_blocks_us);
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

//! Terminal output.
//!
//! Dense, aligned, and colour only where it carries meaning. The rule is the
//! same as the board's: a line the eye can scan for what needs a human, not a
//! line that repeats everything the daemon knows.

use serde::Deserialize;

pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[2m";
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";

/// Whether to emit escape codes at all. Honours `NO_COLOR`, and turns itself
/// off when stdout is not a terminal so that `vibeplane ls > file` is readable.
pub fn colour() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::IsTerminal::is_terminal(&std::io::stdout())
}

pub fn paint(code: &str, s: &str) -> String {
    if colour() {
        format!("{code}{s}{RESET}")
    } else {
        s.to_string()
    }
}

/// The API responses, mirrored for the terminal.
///
/// Some fields are not printed by the current table. They are kept because they
/// are part of the API contract this client is checked against: a field that
/// silently disappears from the struct is a field nobody notices the server
/// stopped sending.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BoardResponse {
    pub summary: Summary,
    pub runs: Vec<RunView>,
    #[serde(default)]
    pub projects: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Summary {
    pub projects: usize,
    pub runs: usize,
    pub live: usize,
    pub working: usize,
    pub needs_you: usize,
    pub idle: usize,
    pub failed: usize,
    #[serde(default)]
    pub dormant: usize,
    pub cost_usd: f64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RunView {
    pub id: String,
    pub project_name: Option<String>,
    pub agent: String,
    pub mode: String,
    pub state: String,
    pub waiting_for: Option<String>,
    pub cwd: String,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub entrypoint: Option<String>,
    pub name: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub reporting: bool,
    pub cost_usd: f64,
    pub context_percent: Option<f64>,
    pub idle_seconds: i64,
}

#[derive(Debug, Deserialize)]
pub struct InboxItem {
    pub kind: String,
    pub level: String,
    pub run_id: String,
    pub title: String,
    pub detail: Option<String>,
    #[serde(default)]
    pub options: Vec<crate::core::event::Choice>,
    #[serde(default)]
    pub actions: Vec<String>,
    /// Present when the item can be answered from here.
    #[serde(default)]
    pub request_id: Option<String>,
    /// Where `open_pr` goes.
    #[serde(default)]
    pub url: Option<String>,
    /// Set when the item is about a piece of work rather than a session.
    #[serde(default)]
    pub work_id: Option<String>,
    /// The rule that would have answered this call, on a permission item.
    #[serde(default)]
    pub suggested_rule: Option<String>,
}

/// One fragment of a driven run's conversation, as the API serves it.
#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub text: String,
}

/// The glyph and colour for a run state. The shape carries the meaning for
/// anyone who cannot see the colour.
pub fn state_marker(state: &str) -> String {
    match state {
        "working" | "starting" => paint(BLUE, "●"),
        "waiting" => paint(YELLOW, "◆"),
        "idle" => paint(DIM, "○"),
        "completed" => paint(GREEN, "✓"),
        "failed" => paint(RED, "✗"),
        "lost" => paint(RED, "?"),
        _ => paint(DIM, "·"),
    }
}

pub fn level_marker(level: &str) -> String {
    match level {
        "critical" => paint(RED, "!!"),
        "high" => paint(YELLOW, " !"),
        _ => paint(DIM, "  "),
    }
}

/// A short, human duration: `4s`, `12m`, `3h`.
pub fn ago(seconds: i64) -> String {
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86400),
    }
}

/// Truncates to a display width, with an ellipsis when it had to cut.
pub use crate::core::text::clip;

/// The line shown for what a run is doing.
///
/// A missing summary is not a session with nothing to say — it is one we have
/// nothing about, usually because the roster found it before any hook did. An
/// empty column reads as the first; these words read as the second.
pub fn summary_line(run: &RunView) -> &str {
    if let Some(s) = run.summary.as_deref() {
        return s;
    }
    match run.state.as_str() {
        "waiting" => "needs you",
        "idle" => "waiting for a prompt",
        // The roster says the process is busy and nothing else has spoken.
        "working" | "starting" => "busy",
        "failed" => "failed",
        "lost" => "gone",
        _ => "",
    }
}

/// The label shown for where a session lives.
pub fn surface(run: &RunView) -> &str {
    match run.entrypoint.as_deref() {
        Some("claude-vscode") => "vscode",
        Some("cli") => "cli",
        Some("sdk-ts") | Some("sdk-py") => "sdk",
        Some(other) => other,
        None => match run.mode.as_str() {
            "background" => "bg",
            _ => "-",
        },
    }
}

/// A duration as a person would say it: `45m`, `2h10m`, `90s`.
///
/// Used where a bound is reported back — "running for 2h10m" reads as a fact,
/// where `7800s` reads as a field.
pub fn duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    match secs {
        0..=90 => format!("{secs}s"),
        91..=3599 => format!("{}m", secs / 60),
        _ => {
            let (h, m) = (secs / 3600, (secs % 3600) / 60);
            if m == 0 {
                format!("{h}h")
            } else {
                format!("{h}h{m}m")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(state: &str, summary: Option<&str>) -> RunView {
        let mut r: RunView = serde_json::from_value(serde_json::json!({
            "id": "r1", "project_name": null, "agent": "claude", "mode": "observed",
            "state": state, "waiting_for": null, "cwd": "/repo", "worktree": null,
            "model": null, "entrypoint": null, "name": null, "summary": null,
            "cost_usd": 0.0, "context_percent": null, "idle_seconds": 0,
        }))
        .unwrap();
        r.summary = summary.map(str::to_string);
        r
    }

    #[test]
    fn a_working_run_that_has_said_nothing_still_says_something() {
        // The roster found it before any hook did. An empty column would read
        // as a session with nothing to report.
        assert_eq!(summary_line(&view("working", None)), "busy");
        assert_eq!(summary_line(&view("idle", None)), "waiting for a prompt");
    }

    #[test]
    fn what_the_run_says_wins() {
        assert_eq!(
            summary_line(&view("working", Some("Bash: cargo test"))),
            "Bash: cargo test"
        );
    }

    #[test]
    fn durations_are_short() {
        assert_eq!(ago(5), "5s");
        assert_eq!(ago(125), "2m");
        assert_eq!(ago(7200), "2h");
        assert_eq!(ago(200_000), "2d");
    }
}

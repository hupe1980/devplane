//! Terminal output: dense, aligned, and colour only where it carries meaning.

use serde::Deserialize;

pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[2m";
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";

/// Whether to emit escape codes at all.
///
/// Honours `NO_COLOR`, is off when stdout is not a terminal, and honours
/// `CLICOLOR_FORCE` so `less -R` — and the alignment tests — can force colour.
pub fn colour() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var_os("CLICOLOR_FORCE").is_some_and(|v| v != "0") {
        return true;
    }
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

pub fn paint(code: &str, s: &str) -> String {
    if colour() {
        format!("{code}{s}{RESET}")
    } else {
        s.to_string()
    }
}

/// How wide a string is on screen: characters, not bytes, and escape codes
/// count as nothing. An East Asian wide character under-counts by one; exact
/// widths need Unicode tables no dependency here carries.
#[must_use]
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // A CSI sequence runs to a letter; other escapes are not emitted
            // here, so skipping one character is the closer approximation.
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        width += 1;
    }
    width
}

/// Left-aligns `s` in a column `width` wide, measured by what is visible.
///
/// `format!("{:<10}", …)` pads by character count, so escape codes break it.
/// Always pads at least one space: a column is a minimum, not a truncation.
#[must_use]
pub fn pad(s: &str, width: usize) -> String {
    let visible = visible_width(s);
    let spaces = width.saturating_sub(visible).max(1);
    format!("{s}{}", " ".repeat(spaces))
}

/// Right-aligns `s` in a column `width` wide, measured by what is visible.
/// The mirror of [`pad`]; always at least one space.
#[must_use]
pub fn pad_left(s: &str, width: usize) -> String {
    let visible = visible_width(s);
    let spaces = width.saturating_sub(visible).max(1);
    format!("{}{s}", " ".repeat(spaces))
}

/// The board, as the host serves it and as a host-less command composes it —
/// one type, so the terminal cannot drift from the page.
pub use crate::core::BoardSummary as Summary;
pub use crate::core::ForgeCounts;
pub use crate::view::{BoardResponse, RunView};

/// `/api/inbox`, as the terminal reads it. Deserialised whole, so a reply
/// this build cannot read is an error rather than a silently empty inbox.
#[derive(Debug, Deserialize)]
pub struct InboxResponse {
    pub items: Vec<InboxItem>,
    #[serde(default)]
    pub folded: Vec<InboxSummary>,
    #[serde(default)]
    pub inhibited: Vec<InboxInhibited>,
    #[serde(default)]
    pub narrowed: Option<crate::core::attention::Narrowed>,
    #[serde(default)]
    pub snoozed: u64,
    /// The close ("since you last looked" and the summary under it).
    #[serde(default)]
    pub close: serde_json::Value,
}

/// A group of items shown as one row. Counted, never hidden: the ids are here
/// so the group can be expanded.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct InboxSummary {
    pub kind: String,
    pub project: Option<String>,
    pub count: usize,
    pub level: String,
    pub ids: Vec<String>,
}

/// Consequences counted on the row that explains them.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct InboxInhibited {
    pub cause: String,
    pub count: usize,
    pub because: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InboxItem {
    pub kind: String,
    pub level: String,
    /// Raised since this person last read the inbox. Decided by the host so
    /// two surfaces never draw the boundary differently; a missing field
    /// reads as not new.
    #[serde(default)]
    pub new_to_you: bool,
    /// `None` for an item not about a session: a change whose runs have all
    /// ended, or `gate_down`, which is about the machine.
    #[serde(default)]
    pub run_id: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    #[serde(default)]
    pub options: Vec<crate::core::event::Choice>,
    #[serde(default)]
    pub actions: Vec<String>,
    /// Present when the item can be answered from here.
    #[serde(default)]
    pub request_id: Option<String>,
    /// The durable ask this item is about, which is what an answer names.
    #[serde(default)]
    pub ask: Option<String>,
    /// Where `open_pr` goes.
    #[serde(default)]
    pub url: Option<String>,
    /// The report a report row is about, which is what its commands name.
    #[serde(default)]
    pub report: Option<String>,
    /// Why there is no yes-or-no, when there is not.
    #[serde(default)]
    pub answer_in: Option<String>,
    /// The project this came from, as the host names it; `None` for an item
    /// about the machine (`gate_down`).
    #[serde(default)]
    pub project_name: Option<String>,
    /// When this started asking; the list is ordered by band, then by age.
    #[serde(default)]
    pub since: Option<String>,
    /// Set when the item is about a change rather than a session.
    #[serde(default)]
    pub change_id: Option<String>,
    /// The rule to paste so this is never asked again, on a permission item.
    #[serde(default)]
    pub offer: Option<crate::core::offer::RuleOffer>,
    /// Why there is none. A blank where an offer belongs reads as broken.
    #[serde(default)]
    pub no_offer: Option<crate::core::offer::NoOfferView>,
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
        // Cut off rather than finished or broken, so it gets its own glyph.
        "interrupted" => paint(DIM, "⊘"),
        "stopped" => paint(DIM, "◌"),
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

/// How long until a moment in the future, or how long since one in the past;
/// a rate-limit window resets ahead of now, which `ago` would print as `0s`.
pub fn until(at: jiff::Timestamp) -> String {
    let seconds = (at - jiff::Timestamp::now()).get_seconds();
    if seconds <= 0 {
        return "now".into();
    }
    format!("in {}", ago(seconds))
}

/// Truncates to a display width, with an ellipsis when it had to cut.
pub use crate::core::text::clip;

/// The line shown for what a run is doing.
///
/// A missing summary means we know nothing about the session yet (usually the
/// roster found it before any hook did), not that it has nothing to say.
pub fn summary_line(run: &RunView) -> &str {
    if let Some(s) = run.summary.as_deref() {
        return s;
    }
    match run.state.as_str() {
        // Waiting on a command it started is not a person's turn: the provider
        // reports idle, the process table says busy.
        "waiting" if run.waiting_for.as_deref() == Some("job") => "running a command it started",
        "waiting" => "needs you",
        "idle" => "waiting for a prompt",
        // The roster says the process is busy and nothing else has spoken.
        "working" | "starting" => "busy",
        "failed" => "failed",
        "lost" => "gone",
        // Never an empty label: a state needs words as well as a glyph.
        "interrupted" => "interrupted — the host stopped this",
        "stopped" => "stopped",
        "completed" => "done",
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
/// Used where a bound is reported back — "running for 2h10m", not `7800s`.
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

    /// Every run state has a glyph and words, and neither is the fallback:
    /// colour never carries state alone, and a `&str` match cannot be
    /// exhaustive.
    #[test]
    fn every_run_state_has_a_marker_and_a_label() {
        use crate::core::RunState;
        let all = [
            RunState::Starting,
            RunState::Working,
            RunState::Waiting(crate::core::WaitingFor::Question),
            RunState::Idle,
            RunState::Completed,
            RunState::Failed,
            RunState::Stopped,
            RunState::Interrupted,
            RunState::Lost,
        ];
        let fallback = paint(DIM, "·");
        for st in all {
            let name = st.as_str();
            assert_ne!(
                state_marker(name),
                fallback,
                "{name} renders as the anonymous fallback dot"
            );
            let v = view(name, None);
            assert!(
                !summary_line(&v).is_empty(),
                "{name} renders with no words beside it"
            );
        }
    }

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
        // The roster found it before any hook did.
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

#[cfg(test)]
mod width_tests {
    use super::*;

    #[test]
    fn escape_codes_occupy_no_columns() {
        assert_eq!(visible_width("failed"), 6);
        assert_eq!(visible_width(&format!("{RED}failed{RESET}")), 6);
        assert_eq!(visible_width(&format!("{DIM}{BOLD}a{RESET}")), 1);
        // A character is a column, not a byte.
        assert_eq!(visible_width("✓"), 1);
    }

    #[test]
    fn a_painted_field_pads_to_the_same_width_as_a_bare_one() {
        // `format!("{:<10}", paint(RED, "failed"))` measures the escape codes
        // too, and captured stdout never shows the coloured version.
        let bare = pad("failed", 10);
        let painted = pad(&format!("{RED}failed{RESET}"), 10);
        assert_eq!(visible_width(&bare), visible_width(&painted));
        assert_eq!(visible_width(&bare), 10);
    }

    #[test]
    fn a_field_wider_than_its_column_still_separates() {
        // 23 characters in a 22-wide column still gets a separating space.
        let over = pad("cargo fmt --all --check", 22);
        assert!(over.ends_with(' '), "{over:?}");
        assert_eq!(visible_width(&over), 24);
    }

    #[test]
    fn a_column_is_a_minimum_and_never_a_truncation() {
        // Truncating would hide the end of a command, which says what it ran.
        let long = "cargo clippy --all-targets --all-features -- -D warnings";
        assert!(pad(long, 10).starts_with(long));
    }
}

#[cfg(test)]
mod pad_left_tests {
    use super::*;

    /// The same defect `pad` fixes, on the other side of the column.
    #[test]
    fn a_painted_number_is_right_aligned_by_what_is_visible() {
        unsafe { std::env::set_var("CLICOLOR_FORCE", "1") };
        let painted = paint(YELLOW, "3");
        assert!(painted.len() > 1, "the fixture is not actually coloured");

        let padded = pad_left(&painted, 8);
        assert_eq!(
            visible_width(&padded),
            8,
            "a coloured cell is not eight columns wide: {padded:?}"
        );
        // `format!` is what this replaces, and it is wrong here.
        assert_ne!(
            visible_width(&format!("{painted:>8}")),
            8,
            "format! aligned a painted string correctly, so this helper is unnecessary"
        );
        unsafe { std::env::remove_var("CLICOLOR_FORCE") };
    }

    #[test]
    fn a_field_wider_than_its_column_still_separates() {
        assert_eq!(pad_left("1234567890", 4), " 1234567890");
    }
}

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

/// Whether to emit escape codes at all.
///
/// Honours `NO_COLOR`, turns itself off when stdout is not a terminal so that
/// `devplane ls > file` is readable, and honours `CLICOLOR_FORCE` so that a
/// person piping into `less -R` — and a test — can ask for colour anyway.
///
/// **`CLICOLOR_FORCE` is why the alignment is testable at all.** Every test in
/// this repository captures stdout, which is not a terminal, so every test has
/// only ever seen the uncoloured output. The columns collapsed the moment colour
/// was on and nothing could see it — every test here captures stdout, which is not a terminal.
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

/// How wide a string is on screen: its characters, not its bytes, and not the
/// escape codes that carry no width.
#[must_use]
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // A CSI sequence runs to a letter. Anything else beginning `ESC` is
            // not something this module emits, and skipping one character of it
            // is closer than counting the escape as a column.
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

/// Left-aligns `s` in a column `width` wide, measured by what is **visible**.
///
/// **`format!("{:<10}", paint(…))` does not do this**, and the difference is
/// invisible in every test this repository has. Rust pads to the string's
/// character count, and a painted string carries nine characters of escape code
/// that occupy no columns — so a ten-wide column holding a coloured `failed`
/// measures sixteen, pads to nothing, and the next field begins immediately
/// after it: `failedcargo clippy …`. Uncoloured, the same code is correct, and
/// tests capture stdout, which is not a terminal — every test here captures stdout, which is not a terminal.
///
/// **Always at least one space**, so a field wider than its column separates
/// from the next one instead of running into it. A column is a minimum, not a
/// promise that everything fits.
#[must_use]
pub fn pad(s: &str, width: usize) -> String {
    let visible = visible_width(s);
    let spaces = width.saturating_sub(visible).max(1);
    format!("{s}{}", " ".repeat(spaces))
}

/// Right-aligns `s` in a column `width` wide, measured by what is **visible**.
///
/// The mirror of [`pad`], and it exists for the same reason: `format!("{:>8}",
/// paint(…))` pads to the string's character count, and a painted string
/// carries escape codes that occupy no columns — so a right-aligned coloured
/// number lands eight characters left of where it belongs and every test sees
/// the uncoloured rendering that worked.
///
/// **Always at least one space**, so a number wider than its column still
/// separates from the field before it.
#[must_use]
pub fn pad_left(s: &str, width: usize) -> String {
    let visible = visible_width(s);
    let spaces = width.saturating_sub(visible).max(1);
    format!("{}{s}", " ".repeat(spaces))
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
    /// Per project id: what GitHub says is open there.
    #[serde(default)]
    pub forge: std::collections::BTreeMap<String, ForgeCounts>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ForgeCounts {
    pub issues: usize,
    pub pull_requests: usize,
    pub needs_you: usize,
    /// Why this project's last poll failed. The counts are the last good ones.
    #[serde(default)]
    pub stale: Option<String>,
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
    #[serde(default)]
    pub open_issues: usize,
    #[serde(default)]
    pub open_prs: usize,
    #[serde(default)]
    pub forge_needs_you: usize,
    /// Asks still waiting whose sessions have ended. Its own number, because
    /// the columns above are a breakdown of sessions and this is not one.
    #[serde(default)]
    pub asks_waiting: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RunView {
    pub id: String,
    /// The project's id, which is what the forge counts are keyed by.
    #[serde(default)]
    pub project: Option<String>,
    pub project_name: Option<String>,
    pub agent: String,
    pub mode: String,
    /// The permission mode the session's own vendor reports, when one of the
    /// eleven hook events that carry it has arrived. `None` is *nothing has
    /// said yet*, which is not the same as *nobody is asked* and must not
    /// render as though it were.
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// Whether a person is in the loop for an ordinary call. Three-valued:
    /// `None` means the mode is one this build does not recognise, or none has
    /// been reported.
    #[serde(default)]
    pub asks_a_person: Option<bool>,
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

/// A group of items shown as one row.
///
/// **Counted, never hidden.** The row names the kind, the project and how
/// many, and the ids are here so it can be opened — a summary that could not
/// be expanded would be a cap wearing a feature's clothes.
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
    /// **Raised since this person last read the inbox.**
    ///
    /// Decided by the daemon, never here: two surfaces each comparing an item's
    /// timestamp to the mark is the second place a boundary gets drawn, and the
    /// two would disagree the first time one of them rounded.
    ///
    /// `#[serde(default)]` because a machine with no previous look sends
    /// nothing to be new to, and a missing field must read as *not new* rather
    /// than fail the decode — the defect this struct's `run_id` field already
    /// records.
    #[serde(default)]
    pub new_to_you: bool,
    /// `None` for an item that is not about a session: a piece of work whose
    /// runs have all ended, or the `gate_down` item, which is about the
    /// machine.
    ///
    /// **This was `String`, and the API has always sent `null` here**, so
    /// `devplane inbox` failed to decode the whole response the moment one
    /// such item existed — which `AttentionItem` calls "the ordinary case, not
    /// an edge one". The renderer below already had a branch for a missing run;
    /// the type stopped it ever being reached.
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
    /// Set when the item is about a piece of work rather than a session.
    #[serde(default)]
    pub work_id: Option<String>,
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
        // Cut off rather than finished or broken, and it needs a glyph of its
        // own: it fell through to the anonymous dot for as long as the state
        // existed, which is colour carrying meaning alone with the colour
        // removed too.
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

/// How long until a moment in the future, or how long since one in the past.
///
/// `ago` answers "how long since"; a rate-limit window resets *ahead* of now,
/// and rendering that with `ago` prints `0s` for every window on the machine.
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
/// A missing summary is not a session with nothing to say — it is one we have
/// nothing about, usually because the roster found it before any hook did. An
/// empty column reads as the first; these words read as the second.
pub fn summary_line(run: &RunView) -> &str {
    if let Some(s) = run.summary.as_deref() {
        return s;
    }
    match run.state.as_str() {
        // **Waiting on a command it started is not a person's turn.** The
        // provider reports such a session as idle, because no tokens are being
        // generated; the process table says otherwise. Saying "needs you" here
        // for the forty minutes of a test suite is how a board teaches people
        // to stop reading it.
        "waiting" if run.waiting_for.as_deref() == Some("job") => "running a command it started",
        "waiting" => "needs you",
        "idle" => "waiting for a prompt",
        // The roster says the process is busy and nothing else has spoken.
        "working" | "starting" => "busy",
        "failed" => "failed",
        "lost" => "gone",
        // **Not "" .** An interrupted run rendered as a dim dot with no words
        // beside it, which is the one thing a state language may not do.
        "interrupted" => "interrupted — the daemon stopped this",
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

    /// **Every run state has a glyph and words, and neither is the fallback.**
    ///
    /// Bought by `RunState::Interrupted`, which was added to stop the daemon
    /// writing `completed` over interrupted work — and then rendered as the
    /// anonymous dim dot with an empty label, because both matches take `&str`
    /// and a `&str` match cannot be exhaustive. The state existed, was correct
    /// in the store, and was invisible on the one surface a person reads.
    ///
    /// [`DESIGN.md`]'s rule is that colour never carries state alone. A state
    /// with no glyph *and* no words fails it twice.
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

// ── Markup, and the reason it is a type rather than a String ────────────────

/// Markup that is already safe to insert into the page.
///
/// **This exists so that "untrusted text is data" stops being a discipline.**
/// The page currently holds that line itself: a test scans it for `${a.bare}`
/// interpolations and fails on any that skips `esc()`. That guard held at 149
/// sites *by luck rather than by design* until somebody wrote it, and it works
/// only because all the interpolation is in one file it can read.
///
/// Rendering moves into this module, so the guarantee has to move with it — and
/// a grep over Rust would be a weaker guard than the one it replaces. So it is
/// a type: the only way to get a runtime value into markup is
/// [`Html::text`], which escapes, and the only way to bypass that is
/// [`Html::raw`], which takes `&'static str` — a value read off the API cannot
/// be `'static`, so untrusted data **cannot reach it**.
///
/// Session names, pull-request titles and branch names come from repositories
/// and from models. Neither is a place to be optimistic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Html(String);

impl Html {
    /// A value from outside, escaped.
    pub fn text(s: impl AsRef<str>) -> Self {
        let s = s.as_ref();
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&#39;"),
                _ => out.push(c),
            }
        }
        Html(out)
    }

    /// Markup this module wrote itself.
    ///
    /// `&'static str` is the whole guarantee: a string that came off the API,
    /// out of a repository or out of a model is allocated at runtime and cannot
    /// satisfy it. There is no `raw(&str)` on purpose, and adding one would
    /// delete the property this type exists for.
    pub fn raw(s: &'static str) -> Self {
        Html(s.to_string())
    }

    pub fn push(&mut self, other: Html) {
        self.0.push_str(&other.0);
    }

    pub fn concat(parts: impl IntoIterator<Item = Html>) -> Self {
        let mut out = Html::default();
        for p in parts {
            out.push(p);
        }
        out
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What may be interpolated by [`markup!`].
///
/// `Html` passes through because it is already safe; everything else is
/// escaped on the way in. There is no implementation that passes a `&str`
/// through unescaped, which is the point.
pub trait IntoHtml {
    fn into_html(self) -> Html;
}

impl IntoHtml for Html {
    fn into_html(self) -> Html {
        self
    }
}

macro_rules! escaped_into_html {
    ($($t:ty),*) => { $(
        impl IntoHtml for $t {
            fn into_html(self) -> Html { Html::text(self.to_string()) }
        }
    )* };
}
escaped_into_html!(&str, String, u64, i64, usize, f64, u32, i32);

/// Build markup from a literal template and escaped arguments.
///
/// The literal parts are trusted because they are written here; every `{}` is
/// escaped unless it is already [`Html`]. It reads like `format!` and cannot be
/// used to smuggle a value past the escaper, which `format!` can.
#[macro_export]
macro_rules! markup {
    ($lit:literal $(, $arg:expr)* $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::render::IntoHtml as _;
        $crate::render::Html::raw_fmt(
            format!($lit $(, $crate::render::IntoHtml::into_html($arg).as_str())*)
        )
    }};
}

impl Html {
    /// Only for [`markup!`], which has already escaped every argument.
    #[doc(hidden)]
    pub fn raw_fmt(s: String) -> Self {
        Html(s)
    }
}

/// The reason pane: what was decided about one piece of work, and on whose
/// authority.
///
/// **The first renderer moved out of the page**, and the one the rest of the
/// move is measured against. It is a fair sample of the shape most of the board
/// is: fetch some rows, render each, say something honest when there are none.
///
/// Rendered here rather than in the page for the reason in the type above —
/// `reason` is a rule somebody wrote, `authority` names who decided, and `action` can
/// be a command an agent composed. None of those is a place to be optimistic
/// about a template literal.
pub fn reason_pane(head: Option<(&str, &str, Option<&str>)>, rows: &[DecisionRow]) -> Html {
    let mut out = Html::default();

    if let Some((title, kind_level, detail)) = head {
        let sub = match detail {
            Some(d) => markup!("{} · {}", kind_level, d),
            None => markup!("{}", kind_level),
        };
        out.push(markup!(
            r#"<div style="margin-bottom:.6rem"><b>{}</b><div style="color:var(--dim)">{}</div></div>"#,
            title,
            sub
        ));
    }

    if rows.is_empty() {
        // Absence is distinguishable from zero: nothing *has been* decided is a
        // different sentence from nothing *happened*, and this is the first.
        out.push(Html::raw(
            r#"<div class="empty">Nothing has been decided about this yet.</div>"#,
        ));
        return out;
    }

    for d in rows {
        out.push(markup!(
            r#"<div class="e"><span class="t">{}</span><span class="who">{}</span><span>{} {}</span></div>"#,
            d.at.as_str(),
            d.authority.as_str(),
            d.outcome.as_str(),
            d.action.as_str()
        ));
        if let Some(reason) = &d.reason {
            out.push(markup!(r#"<div class="r">↳ {}</div>"#, reason.as_str()));
        }
    }
    out
}

/// One row of the reason pane, as the pane needs it.
pub struct DecisionRow {
    /// `HH:MM`, already narrowed — the pane shows a time, not a timestamp.
    pub at: String,
    /// `person`, `rule`, `timer`, `nobody` or `daemon` — the column the pane
    /// exists to show.
    pub authority: String,
    pub action: String,
    pub outcome: String,
    pub reason: Option<String>,
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
        // This is the whole bug. `format!("{:<10}", paint(RED, "failed"))` pads
        // to nothing, because the string it measures is sixteen characters of
        // which ten are invisible — and every test in this repository captures
        // stdout, which is not a terminal, so every test saw the six-character
        // version and passed.
        let bare = pad("failed", 10);
        let painted = pad(&format!("{RED}failed{RESET}"), 10);
        assert_eq!(visible_width(&bare), visible_width(&painted));
        assert_eq!(visible_width(&bare), 10);
    }

    #[test]
    fn a_field_wider_than_its_column_still_separates() {
        // `cargo fmt --all --check` is 23 characters in a 22-wide column, and
        // the field after it used to begin immediately: `…--checkexit 1`.
        let over = pad("cargo fmt --all --check", 22);
        assert!(over.ends_with(' '), "{over:?}");
        assert_eq!(visible_width(&over), 24);
    }

    #[test]
    fn a_column_is_a_minimum_and_never_a_truncation() {
        // A field that does not fit is still printed in full. Truncating here
        // would hide the end of a command, which is the one part that says what
        // it actually ran.
        let long = "cargo clippy --all-targets --all-features -- -D warnings";
        assert!(pad(long, 10).starts_with(long));
    }
}

#[cfg(test)]
mod pad_left_tests {
    use super::*;

    /// The same defect `pad` was written for, on the other side of the column.
    ///
    /// Every test in this repository captures stdout, which is not a terminal,
    /// so every test sees the uncoloured rendering — which is correct. The
    /// coloured one is the only one anybody looks at.
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

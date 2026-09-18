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
/// off when stdout is not a terminal so that `devplane ls > file` is readable.
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
/// `reason` is a rule somebody wrote, `actor` can be a model, and `action` can
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
            d.actor.as_str(),
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
    pub actor: String,
    pub action: String,
    pub outcome: String,
    pub reason: Option<String>,
}

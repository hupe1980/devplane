//! The inbox: what needs a human, ranked.
//!
//! Attention items are *derived* from run state, never stored as authoritative
//! facts — a rebuild from events produces the same inbox. From M2 the open
//! human tasks of the runtime are merged into the same list.

use crate::event::WaitingFor;
use crate::ids::{AttentionId, ProjectId, RunId};
use crate::run::{Run, RunState};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// How loudly an item asks for the human. Only `High` and `Critical` are
/// allowed to raise an OS notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Low,
    Normal,
    High,
    Critical,
}

/// What kind of decision is being asked for. The kinds are deliberately about
/// *what the human must do*, not about which subsystem produced them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    /// A tool wants permission and no policy rule matched.
    Permission,
    /// The agent asked a question.
    Question,
    /// A run ended with an error.
    RunFailed,
    /// A live run has produced no activity for longer than the stall timeout.
    Stalled,
    /// Reconciliation lost the process.
    Lost,
    /// The context window is nearly full; quality drops and compaction looms.
    ContextHigh,
    /// A subscription rate limit is nearly exhausted.
    RateLimit,
}

impl AttentionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AttentionKind::Permission => "permission",
            AttentionKind::Question => "question",
            AttentionKind::RunFailed => "run_failed",
            AttentionKind::Stalled => "stalled",
            AttentionKind::Lost => "lost",
            AttentionKind::ContextHigh => "context_high",
            AttentionKind::RateLimit => "rate_limit",
        }
    }

    pub fn default_level(&self) -> Level {
        match self {
            AttentionKind::Permission | AttentionKind::Question | AttentionKind::RunFailed => {
                Level::High
            }
            AttentionKind::Lost => Level::Critical,
            AttentionKind::Stalled | AttentionKind::ContextHigh | AttentionKind::RateLimit => {
                Level::Normal
            }
        }
    }
}

/// An action offered on an item. Every item carries at least one: an inbox
/// entry with nothing to do about it is a notification, and notifications
/// belong somewhere else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Raise the window that owns this session. The only way to answer an
    /// observed session, and named honestly for that reason.
    Focus,
    /// Attach a terminal to the session.
    Attach,
    /// Open the run in the UI.
    Open,
    /// Copy the command that resumes this session.
    CopyResume,
    /// Dismiss the item until something changes.
    Snooze,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Focus => "focus",
            Action::Attach => "attach",
            Action::Open => "open",
            Action::CopyResume => "copy_resume",
            Action::Snooze => "snooze",
        }
    }
}

/// One entry in the inbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttentionItem {
    pub id: AttentionId,
    pub kind: AttentionKind,
    pub level: Level,
    pub run_id: RunId,
    pub project_id: Option<ProjectId>,
    /// One line naming the decision, not the subsystem.
    pub title: String,
    /// The detail a human needs to decide: the tool input, the question, the
    /// error. Rendered as untrusted text.
    pub detail: Option<String>,
    pub options: Vec<String>,
    pub actions: Vec<Action>,
    pub since: Timestamp,
}

impl AttentionItem {
    /// The stable id of an item, so that the same open question keeps its
    /// place in the list across rebuilds and can be snoozed.
    pub fn make_id(run: &RunId, kind: &AttentionKind) -> AttentionId {
        AttentionId::new(format!("{}:{}", run.as_str(), kind.as_str()))
    }

    /// Sort key: level first, then age. Reverse-sorted, so the most urgent and
    /// oldest is first.
    pub fn rank(&self) -> (Level, i64) {
        (self.level, -self.since.as_second())
    }
}

/// Thresholds the inbox uses. Kept in one struct so the daemon can expose them
/// and the tests can set them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub stall_seconds: i64,
    pub context_high_percent: f64,
    pub rate_limit_percent: f64,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            stall_seconds: 600,
            context_high_percent: 85.0,
            rate_limit_percent: 90.0,
        }
    }
}

/// Derives the inbox for one run. Pure, so the whole inbox is a map over runs
/// and a rebuild after a restart produces exactly the same list.
pub fn items_for_run(run: &Run, cfg: &AttentionConfig) -> Vec<AttentionItem> {
    let mut out = Vec::new();
    // A snoozed run is still on the board — it is only out of the queue of
    // things being asked of the human right now.
    if run.is_snoozed() {
        return out;
    }
    let mut push = |kind: AttentionKind,
                    title: String,
                    detail: Option<String>,
                    options: Vec<String>,
                    actions: Vec<Action>,
                    since: Timestamp| {
        out.push(AttentionItem {
            id: AttentionItem::make_id(&run.id, &kind),
            level: kind.default_level(),
            kind,
            run_id: run.id.clone(),
            project_id: run.project_id.clone(),
            title,
            detail,
            options,
            actions,
            since,
        });
    };

    match &run.state {
        RunState::Waiting(WaitingFor::Permission) => {
            let b = run.blocked_on.as_ref();
            let tool = b.and_then(|b| b.tool.clone()).unwrap_or_default();
            let title = if tool.is_empty() {
                "Permission needed".to_string()
            } else {
                format!("Permission: {tool}")
            };
            push(
                AttentionKind::Permission,
                title,
                b.and_then(|b| b.message.clone())
                    .or_else(|| b.and_then(|b| b.input.as_ref().map(summarise_input))),
                Vec::new(),
                vec![Action::Focus, Action::Attach, Action::Open],
                b.map(|b| b.since).unwrap_or(run.last_event_at),
            );
        }
        RunState::Waiting(WaitingFor::Question) => {
            let b = run.blocked_on.as_ref();
            push(
                AttentionKind::Question,
                b.and_then(|b| b.message.clone())
                    .unwrap_or_else(|| "Agent asked a question".to_string()),
                None,
                b.map(|b| b.options.clone()).unwrap_or_default(),
                vec![Action::Focus, Action::Attach, Action::Open],
                b.map(|b| b.since).unwrap_or(run.last_event_at),
            );
        }
        RunState::Failed => push(
            AttentionKind::RunFailed,
            "Run failed".to_string(),
            run.summary.clone(),
            Vec::new(),
            vec![Action::Open, Action::CopyResume, Action::Snooze],
            run.last_event_at,
        ),
        RunState::Lost => push(
            AttentionKind::Lost,
            "Session lost".to_string(),
            run.summary.clone(),
            Vec::new(),
            vec![Action::CopyResume, Action::Open],
            run.last_event_at,
        ),
        RunState::Working if run.idle_seconds() > cfg.stall_seconds => push(
            AttentionKind::Stalled,
            format!("No activity for {} min", run.idle_seconds() / 60),
            run.summary.clone(),
            Vec::new(),
            vec![Action::Focus, Action::Attach, Action::Open],
            run.last_activity_at,
        ),
        _ => {}
    }

    if run.state.is_live()
        && let Some(pct) = run.totals.context_percent()
        && pct >= cfg.context_high_percent
    {
        push(
            AttentionKind::ContextHigh,
            format!("Context {pct:.0}% full"),
            Some(
                "Compaction is close; ask the agent to commit or hand off to a fresh session."
                    .into(),
            ),
            Vec::new(),
            vec![Action::Open, Action::Focus, Action::Snooze],
            run.last_event_at,
        );
    }

    out
}

/// Builds and ranks the whole inbox.
pub fn rank(mut items: Vec<AttentionItem>) -> Vec<AttentionItem> {
    items.sort_by_key(|i| std::cmp::Reverse(i.rank()));
    items
}

fn summarise_input(v: &serde_json::Value) -> String {
    if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
        return cmd.to_string();
    }
    if let Some(p) = v.get("file_path").and_then(|c| c.as_str()) {
        return p.to_string();
    }
    let s = v.to_string();
    if s.len() > 200 {
        format!("{}…", &s[..200])
    } else {
        s
    }
}

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
    /// A check on a pull request Vibeplane opened is red.
    CiRed,
    /// A pull request is green and waiting for a person.
    PrReady,
    /// Work was mid-flight when the daemon stopped, and its agent is gone.
    Interrupted,
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
            AttentionKind::CiRed => "ci_red",
            AttentionKind::PrReady => "pr_ready",
            AttentionKind::Interrupted => "interrupted",
        }
    }

    pub fn default_level(&self) -> Level {
        match self {
            AttentionKind::Permission
            | AttentionKind::Question
            | AttentionKind::RunFailed
            | AttentionKind::CiRed => Level::High,
            AttentionKind::Lost => Level::Critical,
            AttentionKind::Interrupted => Level::High,
            AttentionKind::Stalled
            | AttentionKind::ContextHigh
            | AttentionKind::RateLimit
            | AttentionKind::PrReady => Level::Normal,
        }
    }
}

/// An action offered on an item. Every item carries at least one: an inbox
/// entry with nothing to do about it is a notification, and notifications
/// belong somewhere else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Grant the outstanding request. Offered only when Vibeplane can actually
    /// answer it — a driven run — never for a session it merely watches.
    Allow,
    /// Refuse it.
    Deny,
    /// Raise the window that owns this session. The only way to answer an
    /// observed session, and named honestly for that reason.
    Focus,
    /// Attach a terminal to the session.
    Attach,
    /// Open the run in the UI.
    Open,
    /// Copy the command that resumes this session.
    CopyResume,
    /// Open the pull request in a browser.
    OpenPr,
    /// Hand the failing check's log back to the agent that wrote the code.
    SendToAgent,
    /// Dismiss the item until something changes.
    Snooze,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Allow => "allow",
            Action::Deny => "deny",
            Action::Focus => "focus",
            Action::Attach => "attach",
            Action::Open => "open",
            Action::CopyResume => "copy_resume",
            Action::OpenPr => "open_pr",
            Action::SendToAgent => "send_to_agent",
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
    /// The protocol request this item answers, when it can be answered.
    #[serde(default)]
    pub request_id: Option<String>,
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
            request_id: run.blocked_on.as_ref().and_then(|b| b.request_id.clone()),
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
            // Answerable only when Vibeplane owns the session. Offering
            // "allow" for a run it cannot reach would be a button that lies.
            let answerable = b.and_then(|b| b.request_id.as_ref()).is_some();
            let actions = if answerable {
                vec![Action::Allow, Action::Deny, Action::Open]
            } else {
                vec![Action::Focus, Action::Attach, Action::Open]
            };
            push(
                AttentionKind::Permission,
                title,
                b.and_then(|b| b.message.clone())
                    .or_else(|| b.and_then(|b| b.input.as_ref().map(summarise_input))),
                Vec::new(),
                actions,
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

/// Derives the inbox entries for a piece of work.
///
/// Work produces items that no run can: a pull request going red hours after
/// the agent stopped is the clearest example of why Work is the durable unit
/// and the session is not.
pub fn items_for_work(work: &crate::work::Work, has_live_run: bool) -> Vec<AttentionItem> {
    let mut out = Vec::new();
    let mk = |kind: AttentionKind, title: String, detail: Option<String>, actions: Vec<Action>| {
        AttentionItem {
            id: AttentionId::new(format!("{}:{}", work.id.as_str(), kind.as_str())),
            level: kind.default_level(),
            kind,
            run_id: crate::ids::RunId::new(
                work.current_run()
                    .map(|r| r.to_string())
                    .unwrap_or_default(),
            ),
            project_id: Some(work.project_id.clone()),
            title,
            detail,
            options: Vec::new(),
            actions,
            request_id: None,
            since: work.updated_at,
        }
    };

    // Work that was mid-flight when the daemon stopped. The Work came back from
    // the store; the agent process did not, and nothing will ever move it on
    // its own — so it has to be said rather than left looking busy for ever.
    if !has_live_run
        && matches!(
            work.phase,
            crate::work::Phase::Implement | crate::work::Phase::Verify
        )
    {
        out.push(mk(
            AttentionKind::Interrupted,
            format!("Interrupted: {}", work.title),
            Some(
                "The agent is gone and this was still in progress. Its branch and \
                 worktree are untouched."
                    .into(),
            ),
            vec![Action::Open, Action::Snooze],
        ));
    }

    let Some(pr) = &work.pull_request else {
        return out;
    };

    out.extend(match pr.status.as_str() {
        "failing" => vec![mk(
            AttentionKind::CiRed,
            format!("#{} is red: {}", pr.number, work.title),
            Some(if pr.failing_checks.is_empty() {
                "a check failed".to_string()
            } else {
                pr.failing_checks.join(", ")
            }),
            vec![Action::SendToAgent, Action::OpenPr, Action::Snooze],
        )],
        // `ready_to_merge` is approved *and* green, so nobody is being asked
        // for anything; it does not belong in a queue of decisions.
        "ready_for_review" => vec![mk(
            AttentionKind::PrReady,
            format!("#{} is ready: {}", pr.number, work.title),
            None,
            vec![Action::OpenPr, Action::Snooze],
        )],
        "changes_requested" => vec![mk(
            AttentionKind::CiRed,
            format!("#{} has review comments: {}", pr.number, work.title),
            None,
            vec![Action::SendToAgent, Action::OpenPr, Action::Snooze],
        )],
        _ => Vec::new(),
    });
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

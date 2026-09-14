//! The status line — the rate-limit channel.
//!
//! Claude Code runs a status-line command on every update, debounced at 300 ms,
//! and hands it a documented JSON payload on stdin. Most of what it carries is
//! available elsewhere, but two things are not: the subscription rate limits,
//! and the provider's own context percentage rather than one derived from
//! token counts.
//!
//! The shim is optional, and off by default, because installing it means
//! wrapping a command the user already configured.

use crate::core::event::Event;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct StatusPayload {
    pub session_id: String,
    #[serde(default)]
    pub session_name: Option<String>,
    #[serde(default)]
    pub context_window: Option<ContextWindow>,
    #[serde(default)]
    pub rate_limits: Option<RateLimits>,
    #[serde(default)]
    pub worktree: Option<Worktree>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextWindow {
    /// Pre-calculated by Claude Code from input tokens only; the derived figure
    /// must use the same formula or the two gauges disagree.
    #[serde(default)]
    pub used_percentage: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimits {
    #[serde(default)]
    pub five_hour: Option<Window>,
    #[serde(default)]
    pub seven_day: Option<Window>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Window {
    #[serde(default)]
    pub used_percentage: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Worktree {
    #[serde(default)]
    pub path: Option<std::path::PathBuf>,
    #[serde(default)]
    pub branch: Option<String>,
}

/// Turns one status-line sample into events.
pub fn to_events(p: &StatusPayload) -> Vec<Event> {
    let mut out = vec![Event::StatusSample {
        context_used_percent: p.context_window.as_ref().and_then(|c| c.used_percentage),
        rate_limit_five_hour: p
            .rate_limits
            .as_ref()
            .and_then(|r| r.five_hour.as_ref())
            .and_then(|w| w.used_percentage),
        rate_limit_seven_day: p
            .rate_limits
            .as_ref()
            .and_then(|r| r.seven_day.as_ref())
            .and_then(|w| w.used_percentage),
        session_name: p.session_name.clone(),
    }];
    // The status line is the one channel that names the worktree branch, which
    // a directory change cannot tell us.
    if let Some(w) = &p.worktree
        && let Some(path) = &w.path
    {
        out.push(Event::WorktreeEntered {
            path: path.clone(),
            branch: w.branch.clone(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sample_carries_context_and_rate_limits() {
        let p: StatusPayload = serde_json::from_str(
            r#"{
              "session_id": "s1",
              "session_name": "auth-refactor",
              "context_window": {"used_percentage": 62.5},
              "rate_limits": {"five_hour": {"used_percentage": 91.2}},
              "worktree": {"path": "/repo/.claude/worktrees/x", "branch": "worktree-x"}
            }"#,
        )
        .unwrap();
        let evs = to_events(&p);
        match &evs[0] {
            Event::StatusSample {
                context_used_percent,
                rate_limit_five_hour,
                session_name,
                ..
            } => {
                assert_eq!(*context_used_percent, Some(62.5));
                assert_eq!(*rate_limit_five_hour, Some(91.2));
                assert_eq!(session_name.as_deref(), Some("auth-refactor"));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(evs[1], Event::WorktreeEntered { .. }));
    }

    #[test]
    fn absent_blocks_are_absent_not_zero() {
        // Rate limits appear only for subscription accounts and only after the
        // first response. Reporting a missing window as 0 % would put a false
        // "plenty left" on the board.
        let p: StatusPayload = serde_json::from_str(r#"{"session_id": "s1"}"#).unwrap();
        match &to_events(&p)[0] {
            Event::StatusSample {
                rate_limit_five_hour,
                context_used_percent,
                ..
            } => {
                assert_eq!(*rate_limit_five_hour, None);
                assert_eq!(*context_used_percent, None);
            }
            other => panic!("{other:?}"),
        }
    }
}

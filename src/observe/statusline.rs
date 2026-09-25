//! The status line — the richest per-session channel the vendor publishes.
//!
//! Claude Code pipes a documented JSON payload to the status-line command on
//! each (debounced) update. Only this channel carries rate limits and resets,
//! the provider's context percentage and window, and the session's release.
//!
//! Nothing may depend on it: the shim is optional, off by default and only
//! runs in interactive sessions. Every field degrades to another channel or to
//! absent.
use crate::core::event::{Event, RateWindow, StatusSample};
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
    #[serde(default)]
    pub model: Option<Model>,
    /// The Claude Code release this session is running.
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub cost: Option<Cost>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cost {
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    #[serde(default)]
    pub total_lines_added: Option<u64>,
    #[serde(default)]
    pub total_lines_removed: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextWindow {
    /// Pre-calculated by Claude Code from input tokens only; the derived figure
    /// must use the same formula or the two gauges disagree.
    #[serde(default)]
    pub used_percentage: Option<f64>,
    /// 200 000, or 1 000 000 for an extended-context model.
    #[serde(default)]
    pub context_window_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimits {
    #[serde(default)]
    pub five_hour: Option<Window>,
    #[serde(default)]
    pub seven_day: Option<Window>,
    /// Behind a Claude apps gateway. Runs past 100 once the limit is exceeded,
    /// and is the only spending signal the gateway persona has.
    #[serde(default)]
    pub spend_limit: Option<Window>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Window {
    #[serde(default)]
    pub used_percentage: Option<f64>,
    /// Unix epoch seconds.
    #[serde(default)]
    pub resets_at: Option<i64>,
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
    let ctx = p.context_window.as_ref();
    let mut windows = Vec::new();
    if let Some(r) = &p.rate_limits {
        for (name, w) in [
            ("five_hour", &r.five_hour),
            ("seven_day", &r.seven_day),
            ("spend_limit", &r.spend_limit),
        ] {
            // No percentage means the account lacks that window; reporting
            // 0 % would show a false "plenty left".
            if let Some(w) = w
                && let Some(used) = w.used_percentage
            {
                windows.push(RateWindow {
                    name: name.to_string(),
                    used_percent: used,
                    resets_at: w.resets_at,
                });
            }
        }
    }
    let cost = p.cost.as_ref();
    let mut out = vec![Event::StatusSample(StatusSample {
        context_used_percent: ctx.and_then(|c| c.used_percentage),
        context_window_size: ctx.and_then(|c| c.context_window_size),
        rate_limits: windows,
        session_name: p.session_name.clone(),
        // The id, not the display name: everywhere else names models by id.
        model: p
            .model
            .as_ref()
            .and_then(|m| m.id.clone().or_else(|| m.display_name.clone())),
        claude_version: p.version.clone(),
        cost_usd: cost.and_then(|c| c.total_cost_usd),
        lines_added: cost.and_then(|c| c.total_lines_added),
        lines_removed: cost.and_then(|c| c.total_lines_removed),
    })];
    // The only channel that names the worktree branch.
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

    fn sample(json: &str) -> StatusSample {
        let p: StatusPayload = serde_json::from_str(json).unwrap();
        match to_events(&p).remove(0) {
            Event::StatusSample(s) => s,
            other => panic!("{other:?}"),
        }
    }

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
            Event::StatusSample(s) => {
                assert_eq!(s.context_used_percent, Some(62.5));
                assert_eq!(s.rate_limits.len(), 1);
                assert_eq!(s.rate_limits[0].name, "five_hour");
                assert_eq!(s.rate_limits[0].used_percent, 91.2);
                assert_eq!(s.session_name.as_deref(), Some("auth-refactor"));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(evs[1], Event::WorktreeEntered { .. }));
    }

    #[test]
    fn absent_blocks_are_absent_not_zero() {
        // Rate limits appear only for subscription accounts, after the first
        // response; a missing window must not read as 0 %.
        let s = sample(r#"{"session_id": "s1"}"#);
        assert!(s.rate_limits.is_empty());
        assert_eq!(s.context_used_percent, None);
        assert_eq!(s.model, None);
        assert_eq!(s.claude_version, None);
        assert_eq!(s.cost_usd, None);
    }

    #[test]
    fn a_window_the_account_does_not_have_is_not_a_window_at_zero() {
        // The block is present and the percentage is not: an account without
        // that limit, which must not be reported as a window under pressure.
        let s = sample(
            r#"{"session_id":"s1","rate_limits":{"five_hour":{},"seven_day":{"used_percentage":12.0}}}"#,
        );
        assert_eq!(s.rate_limits.len(), 1);
        assert_eq!(s.rate_limits[0].name, "seven_day");
    }

    #[test]
    fn the_spend_limit_window_is_read_and_may_exceed_a_hundred() {
        // Gateway spend runs past 100 once exceeded, so it is not clamped.
        let s = sample(
            r#"{"session_id":"s1","rate_limits":{"spend_limit":{"used_percentage":118.5,"resets_at":1738429200}}}"#,
        );
        assert_eq!(s.rate_limits[0].name, "spend_limit");
        assert_eq!(s.rate_limits[0].used_percent, 118.5);
        assert_eq!(s.rate_limits[0].resets_at, Some(1738429200));
    }

    #[test]
    fn the_payload_carries_the_model_the_version_the_window_and_the_cost() {
        // Facts no other channel reports; `version` is the release this
        // session runs, which the gate's matcher depends on.
        let s = sample(
            r#"{
              "session_id": "s1",
              "version": "2.1.272",
              "model": {"id": "claude-opus-5", "display_name": "Opus"},
              "context_window": {"used_percentage": 8.0, "context_window_size": 1000000},
              "cost": {"total_cost_usd": 1.25, "total_lines_added": 156, "total_lines_removed": 23}
            }"#,
        );
        assert_eq!(s.claude_version.as_deref(), Some("2.1.272"));
        // The id, not the display name.
        assert_eq!(s.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(s.context_window_size, Some(1_000_000));
        assert_eq!(s.cost_usd, Some(1.25));
        assert_eq!(s.lines_added, Some(156));
        assert_eq!(s.lines_removed, Some(23));
    }

    #[test]
    fn the_display_name_is_the_fallback_and_never_the_preference() {
        let s = sample(r#"{"session_id":"s1","model":{"display_name":"Opus"}}"#);
        assert_eq!(s.model.as_deref(), Some("Opus"));
    }

    #[test]
    fn an_unknown_field_does_not_lose_the_sample() {
        // The vendor adds fields between releases; an unknown key must not
        // turn the channel off.
        let s = sample(
            r#"{"session_id":"s1","vim":{"mode":"NORMAL"},"prompt_cache":{"hit_ratio":0.91},"version":"2.1.272"}"#,
        );
        assert_eq!(s.claude_version.as_deref(), Some("2.1.272"));
    }
}

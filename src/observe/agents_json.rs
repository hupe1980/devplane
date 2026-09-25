//! The session roster, from `claude agents --json`.
//!
//! Authoritative for background sessions, which Claude's own daemon
//! supervises; hooks go quiet exactly when a session is blocked. It is also
//! the discovery channel (every live session, interactive included, appears
//! before any hook fires) and the startup reconciliation source: a "working"
//! run that is unlisted and whose process is gone is lost.

use crate::core::event::Event;
use serde::Deserialize;
use std::path::PathBuf;

/// One row of `claude agents --json`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AgentRow {
    pub cwd: PathBuf,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default, rename = "startedAt")]
    pub started_at: Option<i64>,
    /// The short id used by `claude attach`, `logs` and `stop`.
    #[serde(default)]
    pub id: Option<String>,
    /// The full session UUID, used by `claude --resume`; absent until the
    /// session has one.
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, rename = "waitingFor")]
    pub waiting_for: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// Where the session is running, when the roster reports it.
    #[serde(default)]
    pub entrypoint: Option<String>,
}

impl AgentRow {
    /// The id to key a run on: the session UUID when present (what hooks and
    /// telemetry report), else the short id.
    pub fn run_key(&self) -> Option<String> {
        self.session_id.clone().or_else(|| self.id.clone())
    }

    pub fn to_event(&self) -> Event {
        Event::RosterSeen {
            // Filled from the process table by the poller before reducing.
            jobs: None,
            kind: self.kind.clone().unwrap_or_else(|| "interactive".into()),
            // Only a background row carries a state the provider owns; an
            // invented state would overwrite what hooks reported.
            state: if self.is_background() {
                self.state.clone()
            } else {
                None
            },
            status: self.status.clone(),
            waiting_for: self.waiting_for.clone(),
            pid: self.pid,
            name: self.name.clone(),
            entrypoint: self.entrypoint.clone(),
            started_at_ms: self.started_at,
        }
    }

    /// Whether the provider's own daemon supervises this session.
    ///
    /// Decides who is authoritative about state: the Claude daemon for a
    /// background session, the hooks for an interactive one.
    pub fn is_background(&self) -> bool {
        self.kind.as_deref() != Some("interactive")
    }
}

/// Parses the roster. A malformed row is skipped rather than failing the poll:
/// one unparseable session must not blank the board.
pub fn parse(body: &[u8]) -> Vec<AgentRow> {
    // The command prints an array; tolerate an object wrapper too.
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match &v {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Object(o) => o
            .get("agents")
            .or_else(|| o.get("sessions"))
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    arr.into_iter()
        .filter_map(|row| serde_json::from_value(row).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::Event;

    const SAMPLE: &str = r#"[
      {"cwd":"/repo","kind":"background","startedAt":1704067200000,"id":"7c5dcf5d",
       "sessionId":"1f0e2c9a-6d0b-4c11-9f39-2a77c1d4e8b5","state":"working","pid":12345,
       "status":"busy","name":"flaky-test-fix"},
      {"cwd":"/other","kind":"background","id":"aa11","state":"blocked",
       "status":"waiting","waitingFor":"permission prompt"},
      {"cwd":"/third","kind":"interactive","startedAt":1704067200000}
    ]"#;

    #[test]
    fn the_roster_parses() {
        let rows = parse(SAMPLE.as_bytes());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name.as_deref(), Some("flaky-test-fix"));
        assert_eq!(rows[0].pid, Some(12345));
    }

    #[test]
    fn a_row_keys_on_the_session_uuid_when_it_has_one() {
        // Keying on the short id would create a second row for the session.
        let rows = parse(SAMPLE.as_bytes());
        assert_eq!(
            rows[0].run_key().as_deref(),
            Some("1f0e2c9a-6d0b-4c11-9f39-2a77c1d4e8b5")
        );
        assert_eq!(rows[1].run_key().as_deref(), Some("aa11"));
    }

    #[test]
    fn interactive_rows_are_listed_but_not_owned_by_the_provider() {
        let rows = parse(SAMPLE.as_bytes());
        assert!(rows[0].is_background());
        assert!(!rows[2].is_background());
        // An interactive row still becomes an event, carrying no state.
        match rows[2].to_event() {
            Event::RosterSeen { state, kind, .. } => {
                assert_eq!(state, None);
                assert_eq!(kind, "interactive");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_real_roster_row_parses() {
        // Captured from `claude agents --json` (Claude Code 2.1.270),
        // including the `status` field only some sessions carry.
        let rows = parse(
            br#"[{"pid":10274,"cwd":"/Users/x/matter-kit","kind":"interactive",
                  "startedAt":1789290021018,
                  "sessionId":"fd247e6f-b3fa-4711-b3d0-83689154449e",
                  "name":"matter-kit-f7","status":"busy"}]"#,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status.as_deref(), Some("busy"));
        assert_eq!(rows[0].pid, Some(10274));
        assert_eq!(
            rows[0].run_key().as_deref(),
            Some("fd247e6f-b3fa-4711-b3d0-83689154449e")
        );
    }

    /// A background row with no `state` reports none, never `working`.
    #[test]
    fn a_background_row_with_no_state_invents_none() {
        let rows = parse(br#"[{"cwd":"/a","kind":"background","id":"x1"}]"#);
        match rows[0].to_event() {
            Event::RosterSeen { state, .. } => assert_eq!(state, None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_blocked_row_becomes_a_blocked_event() {
        let rows = parse(SAMPLE.as_bytes());
        match rows[1].to_event() {
            Event::RosterSeen {
                state, waiting_for, ..
            } => {
                assert_eq!(state.as_deref(), Some("blocked"));
                assert_eq!(waiting_for.as_deref(), Some("permission prompt"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_malformed_row_does_not_blank_the_board() {
        let rows = parse(br#"[{"cwd":"/a"},{"nonsense":true},{"cwd":"/b"}]"#);
        assert_eq!(rows.len(), 2, "good rows survive a bad neighbour");
    }

    #[test]
    fn garbage_yields_nothing() {
        assert!(parse(b"not json").is_empty());
        assert!(parse(b"").is_empty());
    }
}

//! What an agent actually said, for the runs Devplane drives (hooks and
//! telemetry carry no prose). Kept out of the event log: run state reduces over
//! that log and a sentence reduces to nothing, so transcripts get their own
//! table, retention and live-stream frame.
use crate::core::ids::RunId;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Who said it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// What Devplane sent: a prompt, another turn, a gate's failures.
    User,
    Agent,
    /// The agent's reasoning, kept apart so a reader can skip it.
    Thought,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Agent => "agent",
            Role::Thought => "thought",
        }
    }

    /// `None` for an unknown role, so the row is dropped rather than misattributed.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "user" => Role::User,
            "agent" => Role::Agent,
            "thought" => Role::Thought,
            _ => return None,
        })
    }
}

/// One coalesced fragment of a conversation (see [`Coalescer`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub run_id: RunId,
    pub at: Timestamp,
    pub role: Role,
    pub text: String,
}

impl Message {
    pub fn new(run_id: RunId, role: Role, text: impl Into<String>) -> Self {
        Self {
            // Uuid v7: ordering by id is ordering by time.
            id: crate::core::ids::new_event_id(),
            run_id,
            at: Timestamp::now(),
            role,
            text: text.into(),
        }
    }
}

/// What the live stream carries, tagged so a subscriber never inspects the
/// payload to tell an `event` from a `message`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum Frame {
    /// Something changed about a run, a project or a change. Boxed: an envelope
    /// is far larger than the message fragments that make up most traffic.
    Event(Box<crate::core::event::EventEnvelope>),
    Message(Message),
}

impl Frame {
    pub fn run_id(&self) -> &RunId {
        match self {
            Frame::Event(e) => &e.run_id,
            Frame::Message(m) => &m.run_id,
        }
    }
}

/// Joins streamed chunks into fragments: flushed when the speaker changes, when
/// something else happens, or when long enough to show progress.
#[derive(Debug, Default)]
pub struct Coalescer {
    role: Option<Role>,
    buffer: String,
}

/// About a paragraph.
const FRAGMENT_LIMIT: usize = 1024;

impl Coalescer {
    pub fn push(&mut self, role: Role, chunk: &str) -> Option<(Role, String)> {
        let flushed = match self.role {
            Some(current) if current != role => self.take(),
            _ => None,
        };
        self.role = Some(role);
        self.buffer.push_str(chunk);
        // Flushed after appending, so the crossing chunk is in the fragment.
        match flushed {
            Some(f) => Some(f),
            None if self.buffer.len() >= FRAGMENT_LIMIT => self.take(),
            None => None,
        }
    }

    /// Ends the current fragment; called when a tool call arrives or the turn ends.
    pub fn take(&mut self) -> Option<(Role, String)> {
        let role = self.role.take()?;
        let text = std::mem::take(&mut self.buffer);
        match text.trim().is_empty() {
            true => None,
            false => Some((role, text)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(c: &mut Coalescer, role: Role, chunks: &[&str]) -> Vec<(Role, String)> {
        let mut out: Vec<_> = chunks.iter().filter_map(|s| c.push(role, s)).collect();
        out.extend(c.take());
        out
    }

    #[test]
    fn chunks_become_one_fragment_rather_than_one_row_each() {
        let mut c = Coalescer::default();
        let out = drain(
            &mut c,
            Role::Agent,
            &["Look", "ing ", "at ", "the ", "tests"],
        );
        assert_eq!(out, vec![(Role::Agent, "Looking at the tests".to_string())]);
    }

    #[test]
    fn a_change_of_speaker_ends_the_fragment() {
        let mut c = Coalescer::default();
        assert_eq!(c.push(Role::Thought, "I should read auth.rs"), None);
        assert_eq!(
            c.push(Role::Agent, "Reading auth.rs"),
            Some((Role::Thought, "I should read auth.rs".to_string()))
        );
        assert_eq!(c.take(), Some((Role::Agent, "Reading auth.rs".to_string())));
    }

    #[test]
    fn a_long_answer_is_flushed_as_it_arrives() {
        let mut c = Coalescer::default();
        let mut fragments = 0;
        for _ in 0..10 {
            if c.push(Role::Agent, &"x".repeat(200)).is_some() {
                fragments += 1;
            }
        }
        assert!(fragments >= 1, "a long answer must appear before it ends");
        assert!(fragments <= 2, "and must not become a row per chunk");
    }

    #[test]
    fn whitespace_between_tool_calls_is_not_something_somebody_said() {
        let mut c = Coalescer::default();
        c.push(Role::Agent, "\n\n  ");
        assert_eq!(c.take(), None);
        assert_eq!(c.take(), None, "and an empty buffer stays empty");
    }

    #[test]
    fn the_stream_tells_the_two_kinds_of_frame_apart() {
        let m = Frame::Message(Message::new(RunId::new("r1"), Role::Agent, "hello"));
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["frame"], "message");
        assert_eq!(v["role"], "agent");
        assert_eq!(serde_json::from_value::<Frame>(v).unwrap(), m);
    }
}

#[cfg(test)]
mod role_tests {
    use super::Role;

    #[test]
    fn a_role_round_trips_by_name_and_an_unknown_one_is_none() {
        for r in [Role::User, Role::Agent, Role::Thought] {
            assert_eq!(Role::parse(r.as_str()), Some(r));
        }
        assert_eq!(Role::parse("oracle"), None);
    }
}

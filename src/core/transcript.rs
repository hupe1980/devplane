//! What an agent actually said, for the runs Devplane drives.
//!
//! Only driven runs have one. A session somebody started themselves already has
//! a window showing its transcript — `focus` raises it — and the documented
//! channels carry no prose anyway: hooks report lifecycle and tool inputs, and
//! telemetry redacts prompts and responses.
//!
//! Kept out of the event log on purpose. Run state is a pure reduction over
//! that log and a sentence reduces to nothing, so a transcript there would be
//! most of the rows and would slow every replay. Its own table, its own
//! retention, and a separate frame on the live stream.
use crate::core::ids::RunId;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Who said it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// What Devplane sent: a dispatch, another turn, a gate's failures handed
    /// back. Recorded because a transcript that starts with the answer is half
    /// a conversation.
    User,
    /// The agent's answer.
    Agent,
    /// The agent's reasoning, where it streams any. Kept apart from the answer
    /// because a reader wants to skip it far more often than read it.
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

    pub fn parse(s: &str) -> Self {
        match s {
            "user" => Role::User,
            "thought" => Role::Thought,
            _ => Role::Agent,
        }
    }
}

/// One coalesced fragment of a conversation.
///
/// A fragment rather than a message: the protocol streams chunks a few
/// characters at a time, and one row per chunk would be a database of
/// syllables. The pump joins them until the speaker changes, something happens,
/// or the fragment gets long enough to be worth keeping on its own.
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
            // Uuid v7, so ordering by id is ordering by time and reading a
            // transcript back needs no join on the clock.
            id: crate::core::ids::new_event_id(),
            run_id,
            at: Timestamp::now(),
            role,
            text: text.into(),
        }
    }
}

/// What the live stream carries.
///
/// Two kinds of thing, told apart on the wire. A subscriber that wants the
/// board re-read on any change watches for `event`; one showing a transcript
/// watches for `message`; neither has to inspect the payload to find out which
/// it was handed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum Frame {
    /// Something changed about a run, a project or a piece of work.
    Event(crate::core::event::EventEnvelope),
    /// A fragment of what a driven agent said.
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

/// Joins streamed chunks into fragments worth storing.
///
/// The protocol sends the answer a few characters at a time. Storing each chunk
/// would be a row per syllable; waiting for the turn to end would mean nothing
/// to show while the agent is working, which is exactly when somebody is
/// watching. So: flush when the speaker changes, when something else happens,
/// or when the fragment is long enough to stand alone.
#[derive(Debug, Default)]
pub struct Coalescer {
    role: Option<Role>,
    buffer: String,
}

/// How long a fragment may get before it is flushed on its own. About a
/// paragraph: long enough that a chatty agent does not produce a row a second,
/// short enough that a reader sees progress.
const FRAGMENT_LIMIT: usize = 1024;

impl Coalescer {
    /// Adds a chunk, returning a fragment if this one completed it.
    pub fn push(&mut self, role: Role, chunk: &str) -> Option<(Role, String)> {
        let flushed = match self.role {
            Some(current) if current != role => self.take(),
            _ => None,
        };
        self.role = Some(role);
        self.buffer.push_str(chunk);
        // A long buffer is flushed *after* appending, so the fragment that
        // crosses the line carries the chunk that crossed it.
        match flushed {
            Some(f) => Some(f),
            None if self.buffer.len() >= FRAGMENT_LIMIT => self.take(),
            None => None,
        }
    }

    /// Ends the current fragment, if there is one. Called when a tool call
    /// arrives or the turn ends: both mean the sentence is over.
    pub fn take(&mut self) -> Option<(Role, String)> {
        let role = self.role.take()?;
        let text = std::mem::take(&mut self.buffer);
        // Whitespace between two tool calls is not a thing anybody said.
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
        // Reasoning and answer arrive interleaved; running them together would
        // put the agent's thinking inside its reply.
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
        // Waiting for the turn to end would mean nothing on screen while the
        // agent is working, which is when somebody is watching.
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
        // A reader must never have to inspect the payload to find out what it
        // is holding.
        let m = Frame::Message(Message::new(RunId::new("r1"), Role::Agent, "hello"));
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["frame"], "message");
        assert_eq!(v["role"], "agent");
        assert_eq!(serde_json::from_value::<Frame>(v).unwrap(), m);
    }
}

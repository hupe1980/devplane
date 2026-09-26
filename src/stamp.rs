//! The wall clock, read at the edge. The pure half (`src/core/`) never reads
//! it: its constructors take the instant as a value (`*_at`). These are the
//! same constructors stamped with *now*, for callers that observe something
//! as it happens.

use crate::core::change::Change;
use crate::core::decision::{Authority, Decision};
use crate::core::event::{Event, EventEnvelope, Source};
use crate::core::ids::{ProjectId, RunId, SessionId};
use crate::core::run::{Run, RunMode};
use crate::core::transcript::{Message, Role};
use jiff::Timestamp;
use std::path::PathBuf;

impl EventEnvelope {
    /// An envelope observed now.
    pub fn new(run_id: RunId, source: Source, event: Event) -> Self {
        Self::at(run_id, source, event, Timestamp::now())
    }
}

impl Decision {
    /// A decision taken now.
    pub fn new(
        authority: Authority,
        action: &str,
        subject: impl Into<String>,
        outcome: &str,
    ) -> Self {
        Self::new_at(authority, action, subject, outcome, Timestamp::now())
    }
}

impl Run {
    /// A run first seen now.
    pub fn new(session_id: SessionId, cwd: PathBuf, mode: RunMode, agent: &str) -> Self {
        Self::new_at(session_id, cwd, mode, agent, Timestamp::now())
    }
}

impl Change {
    /// A change created now.
    pub fn new(project_id: ProjectId, title: String, prompt: String) -> Self {
        Self::new_at(project_id, title, prompt, Timestamp::now())
    }
}

impl Message {
    /// A message said now.
    pub fn new(run_id: RunId, role: Role, text: impl Into<String>) -> Self {
        Self::new_at(run_id, role, text, Timestamp::now())
    }
}

impl crate::core::change::Completion {
    /// What a change rests on, stamped now.
    pub fn of(
        change: &Change,
        declared: crate::core::change::Declared<'_>,
        now: Option<&crate::core::change::CommitStamp>,
    ) -> Self {
        Self::of_at(change, declared, now, Timestamp::now())
    }
}

impl crate::core::change::Waiting {
    /// One phrase, for the word beside the state, as of now.
    pub fn says(&self) -> String {
        self.says_at(Timestamp::now())
    }
}

impl Run {
    /// Whether this run belongs in the working set as of now.
    pub fn is_active(&self) -> bool {
        self.is_active_at(Timestamp::now())
    }
}

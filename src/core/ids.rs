//! Identifiers. Every id is a newtype so that a project id can never be passed
//! where a run id is meant.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        // `transparent` on the wire, so it is a string on the wire, and the
        // generated TypeScript says so rather than inventing a wrapper object
        // the interface would then have to unwrap.
        #[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
        #[cfg_attr(
            feature = "typescript",
            ts(type = "string", export, export_to = "wire/")
        )]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
    };
}

string_id!(
    ProjectId,
    "A registered repository root. Derived from the canonical path, so the same \
     checkout always resolves to the same project across restarts."
);
string_id!(
    SessionId,
    "A provider's own session identifier — Claude Code's `session_id`, an ACP \
     session id. Stable across resume, which is why runs are keyed by it."
);
string_id!(
    RunId,
    "A Devplane run. For observed sessions this mirrors the `SessionId`: a \
     session we did not start has no other identity we can correlate on."
);
string_id!(AttentionId, "One item in the inbox.");
string_id!(
    AskId,
    "One thing an agent asked a person. **An opaque token, and that is the \
     point**: it is what an answer is addressed to, from any surface, however \
     long afterwards and whatever process is alive by then. Addressing an \
     answer by session is what made an answer undeliverable the moment the \
     session was gone."
);
string_id!(
    BatchId,
    "One person's intent, sent once, to many targets. The reviewable unit of a \
     fan-out: six runs from one prompt are not six things to review, they are \
     one thing that happened six times."
);
string_id!(
    WorkId,
    "One unit of work: durable across sessions, and the thing a branch, a \
     worktree and a set of gates belong to."
);

impl ProjectId {
    /// Derives a project id from a filesystem path.
    ///
    /// The path is used as given, minus a trailing separator. No normalisation
    /// and no symlink resolution happen here — a caller that needs two spellings
    /// of the same directory to agree must canonicalise first, which every
    /// caller that registers a project does.
    pub fn from_path(path: &std::path::Path) -> Self {
        Self(path.to_string_lossy().trim_end_matches('/').to_string())
    }
}

impl RunId {
    pub fn from_session(session: &SessionId) -> Self {
        Self(session.0.clone())
    }
}

/// A monotonically increasing event id. Uuid v7 so that ordering by id is
/// ordering by time, which is what every query wants.
pub fn new_event_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

//! Identifiers. Every id is a newtype so that a project id can never be passed
//! where a run id is meant.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
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
    "A Vibeplane run. For observed sessions this mirrors the `SessionId`: a \
     session we did not start has no other identity we can correlate on."
);
string_id!(AttentionId, "One item in the inbox.");

impl ProjectId {
    /// Derives a project id from a filesystem path. The path is used verbatim
    /// after lexical normalisation; callers that care about symlinks should
    /// canonicalise first.
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

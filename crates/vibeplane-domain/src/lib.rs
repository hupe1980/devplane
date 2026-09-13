//! Vibeplane domain types.
//!
//! No I/O lives here: everything in this crate is data plus the pure functions
//! that derive one shape from another. That is what lets the reducer be
//! replay-tested and the inbox be rebuilt from scratch after a restart.

pub mod attention;
pub mod event;
pub mod ids;
pub mod project;
pub mod run;

pub use attention::{Action, AttentionConfig, AttentionItem, AttentionKind, Level};
pub use event::{ApiUsage, Event, EventEnvelope, Source, WaitingFor};
pub use ids::{AttentionId, ProjectId, RunId, SessionId};
pub use project::Project;
pub use run::{BlockedOn, Run, RunMode, RunState, RunTotals, ToolCall};

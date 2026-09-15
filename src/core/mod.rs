//! Vibeplane's types and the pure logic over them.
//!
//! One rule defines this module: **nothing here may reach the outside world.**
//! No `async fn`, no `.await`, no `tokio`, no `sqlx`, no `reqwest`, no `axum`.
//! Everything is data, plus functions that derive one shape from another; the
//! only I/O is a synchronous read of a small local file in
//! [`config`](crate::core::config) and [`policy_cache`](crate::core::policy_cache),
//! which is bounded in a way a network call is not.
//!
//! Three of the product's claims rest on that and on nothing else:
//!
//! * the board is rebuildable, because run state is a pure
//!   `(Run, Event) -> Run` that can be replayed;
//! * the inbox is correct after a restart, because it is derived and never
//!   stored;
//! * the permission policy cannot fail open, because it cannot wait on
//!   anything — it runs on the synchronous hook a Claude Code session is
//!   blocked on.
//!
//! `tests/purity.rs` fails the build if any file here breaks the rule.
//!
//! * [`ids`](crate::core::ids), [`event`](crate::core::event), [`run`](crate::core::run),
//!   [`work`](crate::core::work), [`project`](crate::core::project),
//!   [`attention`](crate::core::attention) — the domain.
//! * [`reduce`](crate::core::reduce) — the state machine, replay-tested.
//! * [`world`](crate::core::world) — the in-memory state and the derived inbox.
//! * [`policy`](crate::core::policy), [`policy_cache`](crate::core::policy_cache) — permission rules, per project.
//! * [`config`](crate::core::config) — `vibeplane.toml`, and what it means for it to be wrong.
//! * [`decision`](crate::core::decision) — what Vibeplane decided, and on whose authority.
//! * [`deeplink`](crate::core::deeplink) — links that open a coding agent.
//! * [`text`](crate::core::text) — cutting strings a human reads and an agent wrote.
//! * [`transcript`](crate::core::transcript) — what a driven agent said.

pub mod attention;
pub mod command;
pub mod config;
pub mod decision;
pub mod deeplink;
pub mod event;
pub mod ids;
pub mod policy;
pub mod policy_cache;
pub mod project;
pub mod provider;
pub mod reduce;
pub mod run;
pub mod templates;
pub mod text;
pub mod transcript;
pub mod work;
pub mod world;

pub use attention::{Action, AttentionConfig, AttentionItem, AttentionKind, Level};
pub use config::{ConfigError, GlobalConfig, Problem, ProjectConfig};
pub use decision::{Actor, Decision};
pub use event::{ApiUsage, Choice, Event, EventEnvelope, Source, WaitingFor};
pub use ids::{AttentionId, ProjectId, RunId, SessionId, WorkId};
pub use policy::{Policy, Rule, Verdict};
pub use policy_cache::PolicyCache;
pub use project::Project;
pub use run::{BlockedOn, PlanStep, Run, RunMode, RunState, RunTotals, ToolCall};
pub use text::{clip, tail};
pub use transcript::{Coalescer, Frame, Message, Role};
pub use work::{CommandResult, GateReport, Overlap, Phase, Work, WorkKind};
pub use world::{Ambiguous, BoardSummary, Change, RunHint, World};

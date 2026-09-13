//! Vibeplane core: the pure logic between observations and the board.
//!
//! Three pieces, all deliberately I/O-free so they can be tested without a
//! provider, a database or a clock beyond the one the events carry:
//!
//! * [`reduce`] — `(Run, Event) -> Run`, the state machine.
//! * [`world`] — the authoritative in-memory state and the derived inbox.
//! * [`policy`] — the permission rules, shared by the hook, ACP and (later)
//!   Vibeplane's own effects.

pub mod config;
pub mod policy;
pub mod policy_cache;
pub mod reduce;
pub mod world;

pub use config::{ConfigError, ProjectConfig};
pub use policy::{Policy, Rule, Verdict};
pub use policy_cache::PolicyCache;
pub use world::{BoardSummary, Change, RunHint, World};

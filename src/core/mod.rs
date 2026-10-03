//! Devplane's types and the pure logic over them. Nothing here may reach the
//! outside world: no async, no `tokio`, `sqlx`, `reqwest` or `axum`, no file
//! I/O and no clock — what a file said and the time are passed in as values
//! (`crate::config`, `crate::policy_cache` and `crate::stamp` read them). That
//! is why run state replays as a pure `(Run, Event) -> Run` ([`reduce`]), the
//! inbox is derived rather than stored ([`world`]), and the permission policy
//! cannot fail open by waiting. `tests/purity.rs` enforces it.

pub mod ask;
pub mod attention;
pub mod attest;
pub mod base_checks;
pub mod caches;
pub mod candidates;
pub mod certificate;
pub mod change;
pub mod claim;
pub mod clock;
pub mod close;
pub mod command;
pub mod comments;
pub mod config;
pub mod context;
pub mod controls;
pub mod decision;
pub mod declare;
pub mod deeplink;
pub mod diff;
pub mod draft;
pub mod event;
pub mod firstrun;
pub mod forge;
pub mod genai;
pub mod hash;
pub mod ids;
pub mod offer;
pub mod policy;
pub mod preflight;
pub mod project;
pub mod question;
pub mod reduce;
pub mod review;
pub mod run;
pub mod testcount;
pub mod text;
pub mod transcript;
pub mod vendors;
pub mod verify;
pub mod worktreeinclude;
pub mod world;

pub use attention::{Action, AttentionConfig, AttentionItem, AttentionKind, Level};
pub use change::{Change, ChangeState, CommandResult, GateReport, Overlap, Waiting};
pub use config::{ConfigError, GlobalConfig, Problem, ProjectConfig};
pub use decision::{Authority, DecidedEnvelope, Decision};
pub use event::{ApiUsage, Choice, Event, EventEnvelope, Source, WaitingFor};
pub use forge::{ForgeCounts, ForgeIssue, ForgePullRequest, ProjectForge};
pub use ids::{AskId, AttentionId, ChangeId, ProjectId, RunId, SessionId};
pub use policy::{Policy, Rule, Verdict};
pub use project::Project;
pub use run::{
    AgentCapabilityRecord, BlockedOn, PlanStep, Run, RunMode, RunState, RunTotals, ToolCall,
};
pub use text::{clip, tail};
pub use transcript::{Coalescer, Frame, Message, Role};
pub use world::{Ambiguous, BoardSummary, Changed, RunHint, World};

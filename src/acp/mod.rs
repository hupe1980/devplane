//! Driving agents over the Agent Client Protocol.
//!
//! One client for every agent. Claude Code, Codex, OpenCode, Gemini CLI and the
//! rest of the ACP registry all speak the same protocol, so adding an agent is
//! a line of configuration rather than another integration to maintain — and
//! the vendors, not Devplane, keep the adapters working.

pub mod agent;
pub mod session;

pub use agent::{AgentSpec, available, builtin, resolve, user_agents};
pub use session::{
    AcpEvent, Command, PermissionOption, PlanStep, Session, ToolRequest, resume, spawn,
};

//! Driving agents over the Agent Client Protocol.
//!
//! One client for every agent. Claude Code, Codex, OpenCode, Gemini CLI and the
//! rest of the ACP registry all speak the same protocol, so adding an agent is
//! a line of configuration rather than another integration to maintain — and
//! the vendors, not Vibeplane, keep the adapters working.

pub mod agent;
pub mod session;

pub use agent::{AgentSpec, builtin, resolve};
pub use session::{AcpEvent, Command, PermissionOption, Session, spawn};

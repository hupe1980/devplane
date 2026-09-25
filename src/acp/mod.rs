//! Driving agents over the Agent Client Protocol: one client for every agent
//! in the ACP registry, so adding an agent is configuration, not integration.

pub mod agent;
pub mod session;

pub use agent::{AgentSpec, available, builtin, resolve, user_agents};
pub use session::{AcpEvent, PermissionOption, PlanStep, Session, ToolRequest, resume, spawn};

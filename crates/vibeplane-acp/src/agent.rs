//! Which agents can be driven, and how to start them.
//!
//! An agent is a command. The Agent Client Protocol registry publishes the
//! command for each one — `npx @agentclientprotocol/claude-agent-acp@0.76`,
//! `opencode acp`, a downloaded binary — so adding an agent is data rather than
//! code, and the version is pinned to the one the conformance suite passed
//! against.

use serde::{Deserialize, Serialize};

/// How to launch one agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpec {
    /// The id used on the board and in `vibeplane dispatch --agent`.
    pub id: String,
    pub name: String,
    /// The command line, as the registry publishes it.
    pub command: String,
    /// Whether the conformance suite has passed against this exact command.
    /// An unverified agent still runs; the UI just stops pretending it is
    /// known to work.
    #[serde(default)]
    pub verified: bool,
}

impl AgentSpec {
    pub fn new(id: &str, name: &str, command: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            command: command.into(),
            verified: false,
        }
    }
}

/// The agents Vibeplane knows about out of the box.
///
/// Pinned to exact versions on purpose. An agent that silently upgrades under a
/// conformance suite is an agent whose suite proves nothing, and `npx` will
/// happily fetch a new major version overnight otherwise.
pub fn builtin() -> Vec<AgentSpec> {
    vec![
        AgentSpec {
            id: "claude".into(),
            name: "Claude Code".into(),
            command: "npx -y @agentclientprotocol/claude-agent-acp@0.76".into(),
            verified: false,
        },
        AgentSpec {
            id: "codex".into(),
            name: "Codex".into(),
            command: "npx -y @agentclientprotocol/codex-acp@1.11".into(),
            verified: false,
        },
        AgentSpec {
            id: "opencode".into(),
            name: "OpenCode".into(),
            command: "opencode acp".into(),
            verified: false,
        },
        AgentSpec {
            id: "gemini".into(),
            name: "Gemini CLI".into(),
            command: "npx -y @google/gemini-cli@0.59.0 --acp".into(),
            verified: false,
        },
    ]
}

/// Looks an agent up by id, falling back to treating the id as a command so a
/// developer can point at a binary without editing a registry.
///
/// "Looks like a command" means it has arguments or a path separator, or names
/// a file that exists. A bare word is a registry id and nothing else — turning
/// a typo into a launch attempt would report a missing binary instead of an
/// unknown agent.
pub fn resolve(id: &str, extra: &[AgentSpec]) -> Option<AgentSpec> {
    extra
        .iter()
        .chain(builtin().iter())
        .find(|a| a.id == id)
        .cloned()
        .or_else(|| looks_like_command(id).then(|| AgentSpec::new("custom", "custom", id)))
}

fn looks_like_command(id: &str) -> bool {
    id.contains(' ') || id.contains('/') || id.contains('\\') || std::path::Path::new(id).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_agents_are_pinned() {
        // An unpinned `npx` fetches whatever is newest, which makes a
        // conformance run a statement about a version nobody recorded.
        for a in builtin() {
            if a.command.starts_with("npx") {
                assert!(
                    a.command.contains('@') && a.command.rsplit('@').next().unwrap().contains('.'),
                    "{} is not pinned: {}",
                    a.id,
                    a.command
                );
            }
        }
    }

    #[test]
    fn an_unknown_id_can_still_be_a_command() {
        // A typo stays a typo, so the error names the agent rather than a
        // binary nobody meant to run.
        assert!(resolve("nope", &[]).is_none());
        assert!(resolve("claud", &[]).is_none());

        for command in [
            "/opt/my-agent --acp",
            "/opt/my-agent",
            "./target/debug/examples/echo_agent",
            "opencode acp",
        ] {
            assert_eq!(
                resolve(command, &[]).expect(command).command,
                command,
                "{command} should be usable directly"
            );
        }
    }
}

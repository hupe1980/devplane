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
    /// The id used on the board and in `devplane dispatch --agent`.
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

/// The agents Devplane knows about out of the box.
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
            // The only other agent that documents all three channels, so the
            // only other one this product can *watch* as well as drive. The
            // ACP server is a public preview and the version is pinned like
            // every other: a conformance run against something unpinned is a
            // statement about a version nobody recorded.
            id: "copilot".into(),
            name: "GitHub Copilot".into(),
            command: "npx -y @github/copilot@1.0.83 --acp".into(),
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

/// Agents the user added, from `<home>/agents.toml`.
///
/// The point of speaking a standard is that a new agent costs no code, so the
/// launch commands are data rather than a release — and data the user owns,
/// which is a smaller trust surface than fetching a registry.
///
/// ```toml
/// [agents.kimi]
/// name    = "Kimi"
/// command = "npx -y @moonshot/kimi-acp@1.2.0"
/// ```
pub fn user_agents(home: &std::path::Path) -> Vec<AgentSpec> {
    #[derive(Deserialize, Default)]
    #[serde(default, deny_unknown_fields)]
    struct File {
        agents: std::collections::BTreeMap<String, Entry>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Entry {
        #[serde(default)]
        name: Option<String>,
        command: String,
    }

    let path = home.join("agents.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match toml::from_str::<File>(&text) {
        Ok(f) => f
            .agents
            .into_iter()
            .map(|(id, e)| {
                let name = e.name.unwrap_or_else(|| id.clone());
                AgentSpec::new(&id, &name, &e.command)
            })
            .collect(),
        Err(e) => {
            // Loud and empty rather than quiet and half-loaded: an agent list
            // that silently lost an entry is a dispatch that fails with
            // "unknown agent" for a name the user can see in their own file.
            tracing::warn!(path = %path.display(), error = %e, "agents.toml was not read");
            Vec::new()
        }
    }
}

/// Looks an agent up by id, falling back to treating the id as a command so a
/// developer can point at a binary without editing a registry.
///
/// The user's own `agents.toml` wins over the built-in list, so a pinned
/// version can be moved without waiting for a release.
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

/// Every agent this machine can drive: the built-in list, with the user's own
/// entries overriding it by id.
pub fn available(home: &std::path::Path) -> Vec<AgentSpec> {
    let mut out = builtin();
    for spec in user_agents(home) {
        match out.iter_mut().find(|a| a.id == spec.id) {
            Some(existing) => *existing = spec,
            None => out.push(spec),
        }
    }
    out
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
    fn the_user_can_add_an_agent_without_a_release() {
        // The argument for speaking a standard is that a new agent costs no
        // code. Four were compiled in and the fifth needed a release, which is
        // the opposite of that argument.
        let home = std::env::temp_dir().join(format!("vp-agents-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join("agents.toml"),
            "[agents.kimi]\nname = \"Kimi\"\ncommand = \"npx -y kimi-acp@1.2.0\"\n\
             [agents.claude]\ncommand = \"npx -y @agentclientprotocol/claude-agent-acp@0.99\"\n",
        )
        .unwrap();

        let all = available(&home);
        let kimi = all.iter().find(|a| a.id == "kimi").expect("added");
        assert_eq!(kimi.name, "Kimi");

        // And a built-in one can be re-pinned without waiting for us.
        let claude = all.iter().find(|a| a.id == "claude").expect("still there");
        assert!(claude.command.ends_with("0.99"), "{}", claude.command);
        assert_eq!(
            all.iter().filter(|a| a.id == "claude").count(),
            1,
            "an override replaces rather than shadows"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_broken_agents_file_adds_nothing_rather_than_half() {
        let home = std::env::temp_dir().join(format!("vp-agents-bad-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("agents.toml"), "[agents.x]\nnme = \"typo\"\n").unwrap();
        assert!(user_agents(&home).is_empty());
        std::fs::remove_dir_all(&home).ok();
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

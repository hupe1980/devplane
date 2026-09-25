//! Which agents can be driven, and how to start them. An agent is a command,
//! as the ACP registry publishes it, pinned to an exact version.

use serde::{Deserialize, Serialize};

/// How to launch one agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpec {
    /// The id used on the board and in `devplane change start --agent`.
    pub id: String,
    pub name: String,
    /// The command line, as the registry publishes it.
    pub command: String,
}

impl AgentSpec {
    pub fn new(id: &str, name: &str, command: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            command: command.into(),
        }
    }
}

/// The agents Devplane knows about out of the box, pinned to exact `x.y.z`
/// versions: `@0.76` is a range, and `npx` would fetch whatever is newest.
pub fn builtin() -> Vec<AgentSpec> {
    vec![
        AgentSpec::new(
            "claude",
            "Claude Code",
            "npx -y @agentclientprotocol/claude-agent-acp@0.79.0",
        ),
        AgentSpec::new(
            "codex",
            "Codex",
            "npx -y @agentclientprotocol/codex-acp@1.12.0",
        ),
        // The registry distributes OpenCode as a binary, launched as
        // `opencode acp`; there is no `npx` entry to pin.
        AgentSpec::new("opencode", "OpenCode", "opencode acp"),
        // The only other agent documenting all three channels, so it can be
        // watched as well as driven.
        AgentSpec::new(
            "copilot",
            "GitHub Copilot",
            "npx -y @github/copilot@1.0.86 --acp",
        ),
        AgentSpec::new(
            "gemini",
            "Gemini CLI",
            "npx -y @google/gemini-cli@0.60.0 --acp",
        ),
    ]
}

/// Agents the user added, from `<home>/agents.toml`, so a new agent needs no
/// release.
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
            // Loud and empty rather than quiet and half-loaded: a silently
            // dropped entry would fail later as "unknown agent".
            tracing::warn!(path = %path.display(), error = %e, "agents.toml was not read");
            Vec::new()
        }
    }
}

/// Looks an agent up by id (`extra` wins over the built-ins), falling back to
/// treating the id as a command. Only something with arguments, a path
/// separator, or an existing file counts as a command, so a typo reports an
/// unknown agent rather than a missing binary.
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
        // A pin is three numeric components; `@0.76` is a range.
        let mut npx = 0;
        for a in builtin() {
            if !a.command.starts_with("npx") {
                // The one binary entry is the registry's own launch line.
                assert_eq!(
                    a.command, "opencode acp",
                    "{} is not the registry's command",
                    a.id
                );
                continue;
            }
            npx += 1;
            let package = a
                .command
                .split_whitespace()
                .find(|w| w.contains('@') && !w.starts_with('-'))
                .unwrap_or_else(|| panic!("{} names no package: {}", a.id, a.command));
            let version = package.rsplit('@').next().unwrap();
            let parts: Vec<&str> = version.split('.').collect();
            assert!(
                parts.len() == 3
                    && parts
                        .iter()
                        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
                "{} is pinned to `{version}`, which is a range rather than a version",
                a.id
            );
        }
        assert_eq!(npx, 4, "four agents come from npm and one is a binary");
    }

    #[test]
    fn the_user_can_add_an_agent_without_a_release() {
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
        // A typo stays an unknown agent, not a launch attempt.
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

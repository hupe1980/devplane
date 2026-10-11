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
    /// The permission mode the person chose for this agent (`mode` in
    /// `agents.toml`), in the agent's own spelling. Requested at set-up when
    /// the agent offers it; `None` lets Devplane ask for the agent's
    /// ask-first mode when nothing else is configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl AgentSpec {
    pub fn new(id: &str, name: &str, command: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            command: command.into(),
            mode: None,
        }
    }

    /// The npm package and exact version this agent is pinned to, if it is
    /// an `npx` command.
    pub fn npm_pin(&self) -> Option<(&str, &str)> {
        if !self.command.starts_with("npx") {
            return None;
        }
        let package = self
            .command
            .split_whitespace()
            .find(|w| w.rfind('@').is_some_and(|i| i > 0) && !w.starts_with('-'))?;
        let at = package.rfind('@')?;
        Some((&package[..at], &package[at + 1..]))
    }
}

/// The agents Devplane knows about out of the box, pinned to exact `x.y.z`
/// versions: `@0.76` is a range, and `npx` would fetch whatever is newest.
/// The pins match the ACP Registry index read on 2026-10-02, and each moves
/// only with a replay recorded from that version (`tests/fixtures/acp/`).
pub fn builtin() -> Vec<AgentSpec> {
    vec![
        AgentSpec::new(
            "claude",
            "Claude Code",
            "npx -y @agentclientprotocol/claude-agent-acp@0.85.1",
        ),
        AgentSpec::new(
            "codex",
            "Codex",
            "npx -y @agentclientprotocol/codex-acp@2.1.1",
        ),
        // The registry distributes OpenCode as a binary, launched as
        // `opencode acp`; there is no `npx` entry to pin.
        AgentSpec::new("opencode", "OpenCode", "opencode acp"),
        // The only other agent documenting all three channels, so it can be
        // watched as well as driven.
        AgentSpec::new(
            "copilot",
            "GitHub Copilot",
            "npx -y @github/copilot@1.0.91 --acp",
        ),
        AgentSpec::new(
            "gemini",
            "Gemini CLI",
            "npx -y @google/gemini-cli@0.62.0 --acp",
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
///
/// [agents.codex]          # a built-in: `command` may be left out
/// mode    = "workspace-write"
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
        #[serde(default)]
        command: Option<String>,
        #[serde(default)]
        mode: Option<String>,
    }

    let path = home.join("agents.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match toml::from_str::<File>(&text) {
        Ok(f) => {
            let builtins = builtin();
            let mut out = Vec::new();
            for (id, e) in f.agents {
                let base = builtins.iter().find(|b| b.id == id);
                // Only a built-in may leave out its command.
                let Some(command) = e.command.or_else(|| base.map(|b| b.command.clone())) else {
                    tracing::warn!(path = %path.display(), agent = %id,
                        "agents.toml names no command for an agent Devplane does not know; \
                         nothing was read");
                    return Vec::new();
                };
                let name = e
                    .name
                    .or_else(|| base.map(|b| b.name.clone()))
                    .unwrap_or_else(|| id.clone());
                let mut spec = AgentSpec::new(&id, &name, &command);
                spec.mode = e.mode;
                out.push(spec);
            }
            out
        }
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
pub(crate) mod tests {
    use super::*;

    /// The ACP registry's id for each built-in npm agent.
    pub(crate) fn registry_id(agent: &str) -> Option<&'static str> {
        match agent {
            "claude" => Some("claude-acp"),
            "codex" => Some("codex-acp"),
            "copilot" => Some("github-copilot-cli"),
            "gemini" => Some("gemini"),
            _ => None,
        }
    }

    /// Where a built-in npm agent's replay recording lives, under
    /// `tests/fixtures/acp/`: `<registry-id>@<pinned version>`.
    pub(crate) fn recording_dir(spec: &AgentSpec) -> Option<String> {
        let id = registry_id(&spec.id)?;
        let (_, version) = spec.npm_pin()?;
        Some(format!("{id}@{version}"))
    }

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
    fn a_mode_can_be_chosen_for_a_built_in_without_its_command() {
        let home = std::env::temp_dir().join(format!("vp-agents-mode-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join("agents.toml"),
            "[agents.codex]\nmode = \"workspace-write\"\n\
             [agents.kimi]\ncommand = \"npx -y kimi-acp@1.2.0\"\nmode = \"careful\"\n",
        )
        .unwrap();
        let all = available(&home);
        let codex = all.iter().find(|a| a.id == "codex").unwrap();
        let built_in = builtin().into_iter().find(|a| a.id == "codex").unwrap();
        assert_eq!(
            codex.command, built_in.command,
            "the built-in command is kept"
        );
        assert_eq!(codex.name, "Codex");
        assert_eq!(codex.mode.as_deref(), Some("workspace-write"));
        let kimi = all.iter().find(|a| a.id == "kimi").unwrap();
        assert_eq!(
            kimi.mode.as_deref(),
            Some("careful"),
            "a user agent keeps its mode"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn an_unknown_agent_without_a_command_reads_nothing() {
        let home = std::env::temp_dir().join(format!("vp-agents-nocmd-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("agents.toml"), "[agents.kimi]\nmode = \"x\"\n").unwrap();
        assert!(user_agents(&home).is_empty());
        std::fs::write(home.join("agents.toml"), "[agents.codex]\nmood = \"x\"\n").unwrap();
        assert!(
            user_agents(&home).is_empty(),
            "an unknown key still fails the file"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    fn fixtures() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/acp")
    }

    #[test]
    fn every_pin_has_its_recording() {
        for a in builtin().iter().filter(|a| a.npm_pin().is_some()) {
            let dir = recording_dir(a).unwrap_or_else(|| panic!("{} has no registry id", a.id));
            let missing: Vec<&str> = ["session.jsonl", "expect.json", "SOURCE"]
                .into_iter()
                .filter(|f| !fixtures().join(&dir).join(f).is_file())
                .collect();
            if !missing.is_empty() {
                let id = registry_id(&a.id).unwrap();
                let recorded: Vec<String> = std::fs::read_dir(fixtures())
                    .map(|d| {
                        d.filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .filter(|n| n.starts_with(&format!("{id}@")))
                            .collect()
                    })
                    .unwrap_or_default();
                panic!(
                    "{} is pinned to {} but {dir} lacks {missing:?}; recordings that exist: \
                     {recorded:?} — re-record with scripts/record-acp.sh {}",
                    a.id,
                    a.npm_pin().unwrap().1,
                    a.id
                );
            }
        }
    }

    #[test]
    fn pins_match_the_recorded_registry() {
        let text = std::fs::read_to_string(fixtures().join("registry.json"))
            .expect("tests/fixtures/acp/registry.json; run: just pins --write");
        let registry: serde_json::Value =
            serde_json::from_str(&text).expect("the registry copy parses");
        for a in builtin().iter().filter(|a| a.npm_pin().is_some()) {
            let id = registry_id(&a.id).unwrap_or_else(|| panic!("{} has no registry id", a.id));
            let entry = registry["agents"]
                .as_array()
                .and_then(|all| all.iter().find(|e| e["id"] == id))
                .unwrap_or_else(|| panic!("{} ({id}) is not in the registry copy", a.id));
            let (package, pin) = a.npm_pin().unwrap();
            let published = entry["distribution"]["npx"]["package"]
                .as_str()
                .unwrap_or("");
            assert_eq!(
                published,
                format!("{package}@{pin}"),
                "{} is pinned to {pin}; the registry copy says {published}",
                a.id
            );
        }
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

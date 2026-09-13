//! `vibeplane.toml` — the project's own definition of done.
//!
//! It lives in the repository and is committed, which is the whole point: the
//! commands that decide whether work is finished are the project's, reviewed
//! like anything else, and never something an agent wrote for itself.
//!
//! Every field has a default, so a repository with no file at all still works:
//! the gates are empty, no rule auto-decides anything, and Vibeplane behaves
//! exactly as it does today. Configuration buys automation, it is not the price
//! of entry.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// The file Vibeplane looks for at a repository root.
pub const CONFIG_FILE: &str = "vibeplane.toml";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub project: Project,
    pub workspace: Workspace,
    pub gates: Gates,
    pub policy: PolicySection,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Project {
    pub name: Option<String>,
    /// What worktrees branch from. Discovered from the repository when unset.
    pub base_branch: Option<String>,
    /// The agent `vibeplane work start` uses when none is named.
    pub default_agent: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Workspace {
    /// Run once in a new worktree, before the agent starts. A fresh checkout
    /// has no `node_modules` and no `.env`; an agent that has to work that out
    /// for itself wastes a turn discovering it.
    pub setup: Option<String>,
    /// Gitignored files to copy in, in the spirit of `.worktreeinclude`.
    pub include: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Gates {
    /// The Definition of Done: commands that must all succeed.
    pub check: Vec<String>,
    /// How long the whole gate may take.
    #[serde(with = "humantime")]
    pub timeout: Duration,
    pub on_fail: OnFail,
    /// How many times failures are handed back to the agent before a human is
    /// asked. Bounded because an agent and a gate can argue indefinitely, and
    /// each round costs real money.
    pub max_feedback_rounds: u32,
}

impl Default for Gates {
    fn default() -> Self {
        Self {
            check: Vec::new(),
            timeout: Duration::from_secs(600),
            on_fail: OnFail::Feedback,
            max_feedback_rounds: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnFail {
    /// Hand the failures back to the same session and let it try again.
    #[default]
    Feedback,
    /// Ask a human immediately.
    Escalate,
    /// Record the report and carry on. For a check that is advisory.
    Ignore,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PolicySection {
    /// Rules that answer a permission request without asking anyone.
    pub auto_allow: Vec<String>,
    /// Rules that refuse one. Never overridable by `auto_allow`.
    pub never_auto: Vec<String>,
    /// How many runs may work on this project at once.
    pub max_parallel_runs: Option<usize>,
}

impl ProjectConfig {
    /// Reads the configuration for a repository root.
    ///
    /// A missing file is not an error — most repositories will never have one.
    /// A malformed file *is*: silently falling back to defaults would mean a
    /// typo in a deny rule quietly removes the rule.
    pub fn load(root: &Path) -> Result<Self, ConfigError> {
        let path = root.join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(ConfigError::Io(path.display().to_string(), e.to_string())),
        };
        toml::from_str(&text)
            .map_err(|e| ConfigError::Parse(path.display().to_string(), e.to_string()))
    }

    /// The compiled permission policy.
    pub fn policy(&self) -> crate::Policy {
        crate::Policy::new(&self.policy.auto_allow, &self.policy.never_auto)
    }

    pub fn has_gates(&self) -> bool {
        !self.gates.check.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {0}: {1}")]
    Io(String, String),
    #[error("{0} is not valid: {1}")]
    Parse(String, String),
}

/// Durations as people write them: `10m`, `90s`, `1h30m`.
mod humantime {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{}s", d.as_secs()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let raw = String::deserialize(d)?;
        parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("`{raw}` is not a duration")))
    }

    pub fn parse(s: &str) -> Option<Duration> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let mut total = 0u64;
        let mut digits = String::new();
        let mut saw_unit = false;
        for c in s.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
                continue;
            }
            let n: u64 = digits.parse().ok()?;
            digits.clear();
            saw_unit = true;
            total += n * match c {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86_400,
                _ => return None,
            };
        }
        // A bare number is seconds, which is what someone writing `30` means.
        if !digits.is_empty() {
            total += digits.parse::<u64>().ok()?;
        } else if !saw_unit {
            return None;
        }
        Some(Duration::from_secs(total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repository_with_no_file_still_works() {
        let c = ProjectConfig::load(Path::new("/definitely/not/here")).unwrap();
        assert!(!c.has_gates());
        assert_eq!(c.gates.max_feedback_rounds, 2);
        // And no rule decides anything on the user's behalf.
        assert!(c.policy.auto_allow.is_empty());
    }

    #[test]
    fn a_full_file_parses() {
        let dir = tempdir("full");
        std::fs::write(
            dir.join(CONFIG_FILE),
            r#"
[project]
name = "saas"
base_branch = "main"
default_agent = "claude"

[workspace]
setup = "pnpm install --frozen-lockfile"
include = [".env", ".env.local"]

[gates]
check = ["pnpm typecheck", "pnpm test -- --run"]
timeout = "10m"
on_fail = "feedback"
max_feedback_rounds = 3

[policy]
auto_allow = ["Read", "Bash(pnpm test *)"]
never_auto = ["Bash(git push *)"]
max_parallel_runs = 2
"#,
        )
        .unwrap();
        let c = ProjectConfig::load(&dir).unwrap();
        assert_eq!(c.project.name.as_deref(), Some("saas"));
        assert_eq!(c.gates.check.len(), 2);
        assert_eq!(c.gates.timeout, Duration::from_secs(600));
        assert_eq!(c.gates.max_feedback_rounds, 3);
        assert!(matches!(
            c.policy()
                .evaluate("Bash", &serde_json::json!({"command": "git push origin"})),
            crate::Verdict::Deny { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_typo_is_an_error_rather_than_a_silent_default() {
        // A misspelled key in a deny list would otherwise remove the rule and
        // say nothing, which is the worst possible way for a policy to fail.
        let dir = tempdir("typo");
        std::fs::write(
            dir.join(CONFIG_FILE),
            "[policy]\nnever_autoo = [\"Bash(git push *)\"]\n",
        )
        .unwrap();
        let err = ProjectConfig::load(&dir).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(..)), "{err:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn durations_are_written_the_way_people_say_them() {
        use humantime::parse;
        assert_eq!(parse("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse("10m"), Some(Duration::from_secs(600)));
        assert_eq!(parse("1h30m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse("45"), Some(Duration::from_secs(45)));
        assert_eq!(parse("soon"), None);
        assert_eq!(parse(""), None);
    }

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-cfg-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

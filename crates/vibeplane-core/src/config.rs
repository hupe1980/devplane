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
use std::collections::BTreeMap;
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
    pub github: GitHub,
    /// Declared chains of agent runs, keyed by the Work kind they serve
    /// (`quick`, `chore`, `bug`, `feature`) or by any name for a standing one.
    pub pipelines: BTreeMap<String, Pipeline>,
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
    /// Gates a pipeline step can ask for by name, beyond the default `check`.
    pub named: BTreeMap<String, NamedGate>,
}

/// A gate that is not the Definition of Done.
///
/// The one that earns its keep is a reproduction: for a bug, the check that
/// *should fail* before the fix is what proves the bug was real. So a gate
/// carries what it expects, and a command that succeeds when failure was the
/// point is a failed gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NamedGate {
    /// Commands that must all meet `expect`.
    pub run: Vec<String>,
    pub expect: Expect,
    /// Overrides the project's gate timeout.
    #[serde(default, with = "humantime_opt")]
    pub timeout: Option<Duration>,
}

impl Default for NamedGate {
    fn default() -> Self {
        Self {
            run: Vec::new(),
            expect: Expect::Pass,
            timeout: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expect {
    #[default]
    Pass,
    /// The commands must fail. A reproduction that passes has not reproduced
    /// anything.
    Fail,
}

impl Default for Gates {
    fn default() -> Self {
        Self {
            check: Vec::new(),
            timeout: Duration::from_secs(600),
            on_fail: OnFail::Feedback,
            max_feedback_rounds: 2,
            named: BTreeMap::new(),
        }
    }
}

/// A declared chain of agent runs (D34).
///
/// "Start Claude to implement, then start an agent to check it" is only worth
/// having if it is written down in the repository rather than improvised per
/// session: the same chain, on every piece of work of that kind, with the
/// human in the places the project chose.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pipeline {
    pub steps: Vec<Step>,
}

/// One step of a pipeline: an agent doing something, or a person.
///
/// Untagged, because `{ human = "merge" }` and `{ role = "review", ... }` read
/// better in TOML than a `type` discriminator would. The human form is tried
/// first, so a step carrying `human` is never mistaken for a role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Step {
    /// Suspends the pipeline until a person releases it.
    Human(HumanStep),
    Role(RoleStep),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanStep {
    /// What the person is being asked to do: `merge`, `spec_review`, …
    pub human: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleStep {
    /// What this step is for: `implement`, `review`, `verify`. Also the name
    /// `back_to` refers to.
    pub role: String,
    /// The agent that does it. `any` — and the default — take the project's
    /// `default_agent`. Naming a different vendor for review is the point: a
    /// model reading its own diff is a weaker reader.
    #[serde(default)]
    pub agent: Option<String>,
    /// A prompt template in `.vibeplane/prompts/<name>.md`, or the text itself
    /// when no such file exists.
    pub prompt: String,
    /// A gate that must pass before the pipeline moves on: `check`, or a name
    /// from `[gates.named]`.
    #[serde(default)]
    pub gate: Option<String>,
    /// What to do when this step reports findings.
    #[serde(default)]
    pub findings: Option<Findings>,
}

/// How a reviewing step sends work back.
///
/// The reviewer writes its findings to a file in the worktree rather than
/// announcing them in prose. Prose has to be parsed and can be wrong in ways
/// that are invisible; a file is either there or it is not, a human can read
/// it, and it is the same evidence the next agent is handed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Findings {
    /// The role to return to. Must name an earlier step.
    pub back_to: String,
    /// How many times. Exhaustion asks a human; it never loops forever (R16).
    #[serde(default = "one")]
    pub max: u32,
    /// Where the reviewer writes them, relative to the worktree.
    #[serde(default = "findings_file")]
    pub file: String,
}

fn one() -> u32 {
    1
}
fn findings_file() -> String {
    ".vibeplane/findings.md".into()
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitHub {
    /// Open a pull request when the gates pass. Off by default: pushing a
    /// branch is the first thing Vibeplane does that other people can see.
    pub pull_request: bool,
    /// Open it as a draft. A pull request that looks finished summons
    /// reviewers, and work a machine just finished has not been read by anyone.
    pub draft: bool,
    /// Issues carrying this label are offered as work.
    pub ready_label: Option<String>,
    /// Merge with a squash rather than a merge commit, once checks pass.
    pub squash: bool,
}

impl Default for GitHub {
    fn default() -> Self {
        Self {
            pull_request: false,
            draft: true,
            ready_label: None,
            squash: true,
        }
    }
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
    /// How long a working run may produce nothing before it is called stalled.
    ///
    /// Per project, because the number is a statement about the work: a
    /// repository whose test suite takes twelve minutes stalls at a different
    /// threshold from one that answers in seconds, and a single machine-wide
    /// number has to be wrong for one of them. Unset falls back to the global
    /// setting.
    #[serde(default, with = "humantime_opt")]
    pub stall_timeout: Option<Duration>,
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

    /// The pipeline that governs a kind of work, if the project declared one.
    pub fn pipeline_for(&self, kind: &str) -> Option<&Pipeline> {
        self.pipelines.get(kind).filter(|p| !p.steps.is_empty())
    }

    /// The commands and expectation behind a gate name.
    ///
    /// `check` is the Definition of Done and always resolves; anything else has
    /// to be declared, and asking for a gate that does not exist is an error
    /// rather than a silent pass — an empty gate that reads as success is the
    /// failure this layer exists to prevent.
    pub fn gate_named(&self, name: &str) -> Option<(Vec<String>, Expect, Duration)> {
        if name == "check" {
            return Some((self.gates.check.clone(), Expect::Pass, self.gates.timeout));
        }
        let g = self.gates.named.get(name)?;
        Some((
            g.run.clone(),
            g.expect,
            g.timeout.unwrap_or(self.gates.timeout),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConfigError {
    #[error("reading {0}: {1}")]
    Io(String, String),
    #[error("{0} is not valid: {1}")]
    Parse(String, String),
}

/// The same, for a setting that may be left out entirely.
mod humantime_opt {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => s.serialize_str(&format!("{}s", d.as_secs())),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        let raw = match Option::<String>::deserialize(d)? {
            Some(r) => r,
            None => return Ok(None),
        };
        super::humantime::parse(&raw)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("`{raw}` is not a duration")))
    }
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
    fn a_pipeline_reads_as_a_chain_of_roles_and_people() {
        let c: ProjectConfig = toml::from_str(
            r#"
[pipelines.feature]
steps = [
  { role = "implement", agent = "claude", prompt = "implement", gate = "check" },
  { role = "review",    agent = "codex",  prompt = "review", findings = { back_to = "implement", max = 2 } },
  { human = "merge" },
]
"#,
        )
        .unwrap();
        let p = c.pipeline_for("feature").expect("declared");
        assert_eq!(p.steps.len(), 3);

        let Step::Role(first) = &p.steps[0] else {
            panic!("the first step is an agent's")
        };
        assert_eq!(first.gate.as_deref(), Some("check"));

        let Step::Role(second) = &p.steps[1] else {
            panic!("so is the second")
        };
        let f = second.findings.as_ref().expect("it sends work back");
        assert_eq!(f.back_to, "implement");
        assert_eq!(f.max, 2);
        // Defaulted rather than required: a pipeline should be writable in
        // three lines, and the path only matters when someone wants it moved.
        assert_eq!(f.file, ".vibeplane/findings.md");

        // A step with `human` is a person, never mistaken for a role.
        assert!(matches!(&p.steps[2], Step::Human(h) if h.human == "merge"));
    }

    #[test]
    fn a_pipeline_with_no_steps_is_not_a_pipeline() {
        // Otherwise declaring `[pipelines.feature]` and forgetting the steps
        // would start work that immediately claims to be finished.
        let c: ProjectConfig = toml::from_str("[pipelines.feature]\nsteps = []\n").unwrap();
        assert!(c.pipeline_for("feature").is_none());
    }

    #[test]
    fn a_gate_that_is_not_declared_is_not_a_pass() {
        // Asking for a gate nobody wrote must fail loudly. A missing gate that
        // reads as success is the exact failure this layer exists to prevent.
        let c: ProjectConfig = toml::from_str("[gates]\ncheck = [\"cargo test\"]\n").unwrap();
        assert!(c.gate_named("check").is_some());
        assert!(c.gate_named("repro-fails-on-base").is_none());
    }

    #[test]
    fn a_reproduction_gate_says_it_expects_to_fail() {
        let c: ProjectConfig = toml::from_str(
            r#"
[gates.named.repro]
run = ["cargo test --test repro"]
expect = "fail"
timeout = "90s"
"#,
        )
        .unwrap();
        let (cmds, expect, timeout) = c.gate_named("repro").expect("declared");
        assert_eq!(cmds, vec!["cargo test --test repro".to_string()]);
        assert_eq!(expect, Expect::Fail);
        assert_eq!(timeout, Duration::from_secs(90));
    }

    #[test]
    fn a_repository_with_no_file_still_works() {
        let c = ProjectConfig::load(Path::new("/definitely/not/here")).unwrap();
        assert!(!c.has_gates());
        assert_eq!(c.gates.max_feedback_rounds, 2);
        // And no rule decides anything on the user's behalf.
        assert!(c.policy.auto_allow.is_empty());
        assert_eq!(c.policy.stall_timeout, None);
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

[github]
pull_request = true
draft = true
ready_label = "vibeplane:ready"
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
    fn github_is_off_until_it_is_asked_for() {
        // Pushing a branch is the first thing Vibeplane does that other people
        // can see, so it is never a default.
        let c = ProjectConfig::default();
        assert!(!c.github.pull_request);
        assert!(c.github.draft, "and when it is on, it is a draft");
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

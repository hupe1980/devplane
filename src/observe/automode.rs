//! The other gate, read back.
//!
//! In **auto mode** Claude Code routes tool calls through a classifier instead
//! of prompting, and the classifier has its own configuration that Devplane
//! neither writes nor controls. That matters here more than it looks: auto mode
//! is the mode people choose precisely when they are *not* watching, and
//! closing the hole on Devplane's own side (prohibitions ride `PreToolUse`,
//! which fires in every mode) only fixed half of it. The other half is that a
//! person supervising twenty agents has no way to see what the classifier will
//! stop.
//!
//! So `doctor` reads it and reports it. **Read, never write** — the entries are
//! prose interpreted by a model, a mistake in one is silent in the direction
//! that widens, and taking responsibility for a classifier's behaviour on
//! somebody else's machine is a promise this product cannot keep. That is the
//! opposite of the `connect` decision, and the three things that make writing
//! hooks defensible — exact, reversible, shown as a diff first — hold for none
//! of this.
//!
//! The documented precedence, which is the thing worth putting on a screen:
//! `permissions.deny` and a content-scoped `permissions.ask` resolve **before**
//! the classifier and it cannot override either; inside it the order is
//! `hard_deny` → `soft_deny` → `allow` → explicit user intent.

use serde::{Deserialize, Serialize};

/// What the classifier is configured with, as `claude auto-mode config` reports
/// it with `"$defaults"` already expanded.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoMode {
    /// Trusted repos, buckets, domains and the context slots around them.
    #[serde(default)]
    pub environment: Vec<String>,
    /// Exceptions to the soft blocks.
    #[serde(default)]
    pub allow: Vec<String>,
    /// Destructive actions that explicit user intent can clear.
    #[serde(default)]
    pub soft_deny: Vec<String>,
    /// Unconditional boundaries. Intent and `allow` do not apply.
    #[serde(default)]
    pub hard_deny: Vec<String>,
}

impl AutoMode {
    /// Whether anything has been configured beyond the built-in defaults.
    ///
    /// The interesting case for a report is the *empty* one: a classifier that
    /// has been told nothing about your infrastructure treats every destination
    /// it does not recognise as a potential exfiltration target, which is the
    /// documented cause of the repeated denials people blame on the agent.
    pub fn is_empty(&self) -> bool {
        self.environment.is_empty()
            && self.allow.is_empty()
            && self.soft_deny.is_empty()
            && self.hard_deny.is_empty()
    }

    pub fn counts(&self) -> [usize; 4] {
        [
            self.environment.len(),
            self.allow.len(),
            self.soft_deny.len(),
            self.hard_deny.len(),
        ]
    }
}

/// Why the effective configuration could not be read.
///
/// Reported rather than swallowed, because "the classifier is unconfigured" and
/// "we could not ask" are different facts and only one of them is a problem
/// with the user's setup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    /// No `claude` on the machine — the ordinary case for somebody driving
    /// only non-Claude agents.
    NoClaude,
    /// The subcommand is older than v2.1.198, or auto mode is not available to
    /// this account.
    NotSupported,
    /// It answered with something this build could not read.
    Unreadable(String),
}

/// Reads the effective configuration back.
///
/// Bounded and best-effort: this runs on a diagnostics request, and a
/// diagnostic that can hang is one that makes the thing it is diagnosing look
/// broken.
pub async fn effective() -> Result<AutoMode, Unavailable> {
    let Some(bin) = crate::observe::locate::claude_binary() else {
        return Err(Unavailable::NoClaude);
    };
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(bin)
            .args(["auto-mode", "config"])
            .output(),
    )
    .await
    .map_err(|_| Unavailable::NotSupported)?
    .map_err(|_| Unavailable::NotSupported)?;

    if !out.status.success() {
        return Err(Unavailable::NotSupported);
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|e| Unavailable::Unreadable(crate::core::text::clip(&e.to_string(), 200)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_lists_are_read_and_an_unconfigured_one_says_so() {
        // The shape `claude auto-mode config` prints, trimmed. The empty case
        // is the one worth reporting: a classifier told nothing about your
        // infrastructure blocks routine internal operations, and the person
        // blames the agent.
        let cfg: AutoMode = serde_json::from_str(
            r#"{
              "allow": ["Test Artifacts: hardcoded test API keys"],
              "soft_deny": ["Git Destructive: force pushing"],
              "hard_deny": ["Data Exfiltration"],
              "environment": ["**Trusted repo**: the git repository the agent started in"]
            }"#,
        )
        .unwrap();
        assert_eq!(cfg.counts(), [1, 1, 1, 1]);
        assert!(!cfg.is_empty());
        assert!(AutoMode::default().is_empty());
    }

    #[test]
    fn a_field_this_build_does_not_know_is_not_an_error() {
        // The classifier's configuration is the vendor's and grows. An
        // observer that fails on a key it has not heard of is an observer that
        // breaks on upgrade — the same rule the hook receiver follows.
        let cfg: AutoMode =
            serde_json::from_str(r#"{"allow": [], "classifyAllShell": true}"#).unwrap();
        assert!(cfg.is_empty());
    }
}

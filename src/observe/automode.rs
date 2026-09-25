//! Claude Code's auto-mode classifier configuration, read back for `doctor`.
//!
//! Auto mode is chosen when nobody is watching, so a person should see what
//! the classifier will stop. Read, never write: the entries are prose a model
//! interprets, and a mistake widens silently. Precedence: `permissions.deny`
//! and a content-scoped `permissions.ask` resolve before the classifier; inside
//! it, `hard_deny` → `soft_deny` → `allow` → explicit user intent.

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
    /// The empty case matters: a classifier told nothing about your
    /// infrastructure treats unknown destinations as exfiltration targets.
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
/// "Unconfigured" and "could not ask" are different facts; only one is a
/// problem with the user's setup.
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
/// Bounded and best-effort: a diagnostic that hangs makes things look broken.
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
        // The shape `claude auto-mode config` prints, trimmed.
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
        // The vendor's configuration grows; an unknown key must not break
        // the read.
        let cfg: AutoMode =
            serde_json::from_str(r#"{"allow": [], "classifyAllShell": true}"#).unwrap();
        assert!(cfg.is_empty());
    }
}

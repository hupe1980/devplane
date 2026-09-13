//! The permission policy.
//!
//! One rule set answers three callers with one audit format: the synchronous
//! `PermissionRequest` hook for sessions Vibeplane only observes, ACP
//! `session/request_permission` for driven runs, and (from M2) Vibeplane's own
//! effects. The rule syntax is Claude Code's, so a rule can be copied between
//! `settings.json` and `vibeplane.toml` without translation.
//!
//! Two properties matter more than expressiveness:
//!
//! * **It cannot fail open.** Evaluation is total, synchronous and in-process;
//!   there is no "policy service unreachable" branch. A hook that answers in a
//!   millisecond is invisible; one that waits on a network is a stall on every
//!   tool call.
//! * **`never_auto` wins.** A deny rule cannot be overridden by an allow rule,
//!   whatever the order, so adding a permissive rule can never silently widen a
//!   prohibition someone wrote deliberately.

use serde::{Deserialize, Serialize};
use std::fmt;

/// What the policy decided about one tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Allowed by `rule`.
    Allow { rule: String },
    /// Denied by `rule`.
    Deny { rule: String },
    /// No rule matched. The provider's own dialog decides, and the request
    /// becomes an inbox item.
    Undecided,
}

impl Verdict {
    pub fn rule(&self) -> Option<&str> {
        match self {
            Verdict::Allow { rule } | Verdict::Deny { rule } => Some(rule),
            Verdict::Undecided => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Allow { .. } => "allow",
            Verdict::Deny { .. } => "deny",
            Verdict::Undecided => "undecided",
        }
    }
}

/// One parsed rule: a tool name and an optional content pattern.
///
/// `Read` matches every use of the tool. `Bash(git push *)` matches only calls
/// whose command starts with `git push `. The trailing space before `*` is
/// significant — `Bash(git diff*)` would also match `git diff-index`, which is
/// the same trap Claude Code's own documentation calls out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    raw: String,
    tool: String,
    pattern: Option<String>,
}

impl Rule {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        match raw.split_once('(') {
            Some((tool, rest)) => {
                let pattern = rest.strip_suffix(')')?;
                Some(Self {
                    raw: raw.to_string(),
                    tool: tool.trim().to_string(),
                    pattern: Some(pattern.to_string()),
                })
            }
            None => Some(Self {
                raw: raw.to_string(),
                tool: raw.to_string(),
                pattern: None,
            }),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Whether this rule covers a tool call.
    pub fn matches(&self, tool: &str, content: Option<&str>) -> bool {
        if !self.tool.eq_ignore_ascii_case(tool) {
            return false;
        }
        match (&self.pattern, content) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(p), Some(c)) => glob_match(p, c),
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

/// A compiled policy.
#[derive(Debug, Clone, Default)]
pub struct Policy {
    allow: Vec<Rule>,
    deny: Vec<Rule>,
}

impl Policy {
    pub fn new(allow: &[String], deny: &[String]) -> Self {
        Self {
            allow: allow.iter().filter_map(|r| Rule::parse(r)).collect(),
            deny: deny.iter().filter_map(|r| Rule::parse(r)).collect(),
        }
    }

    pub fn allow_rules(&self) -> &[Rule] {
        &self.allow
    }
    pub fn deny_rules(&self) -> &[Rule] {
        &self.deny
    }

    /// Decides one tool call. Deny is evaluated first and is not overridable.
    pub fn evaluate(&self, tool: &str, input: &serde_json::Value) -> Verdict {
        let content = rule_content(tool, input);
        let c = content.as_deref();
        if let Some(r) = self.deny.iter().find(|r| r.matches(tool, c)) {
            return Verdict::Deny {
                rule: r.raw.clone(),
            };
        }
        if let Some(r) = self.allow.iter().find(|r| r.matches(tool, c)) {
            return Verdict::Allow {
                rule: r.raw.clone(),
            };
        }
        Verdict::Undecided
    }
}

/// The part of a tool's input a rule pattern is matched against: the command
/// for shells, the path for file tools, the URL for fetches. Anything else has
/// no content, so only a bare tool-name rule can match it.
pub fn rule_content(tool: &str, input: &serde_json::Value) -> Option<String> {
    let key = match tool {
        "Bash" | "PowerShell" | "Monitor" => "command",
        "Read" | "Edit" | "Write" | "NotebookEdit" => "file_path",
        "WebFetch" => "url",
        "Glob" | "Grep" => "pattern",
        _ => return None,
    };
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Glob matching for rule patterns: `*` matches any run of characters,
/// including `/`. Deliberately simple — a rule a reader cannot predict is worse
/// than one that cannot express a corner case.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn inner(p: &[u8], t: &[u8]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some(b'*') => {
                let rest = &p[1..];
                if rest.is_empty() {
                    return true;
                }
                (0..=t.len()).any(|i| inner(rest, &t[i..]))
            }
            Some(&c) => !t.is_empty() && t[0] == c && inner(&p[1..], &t[1..]),
        }
    }
    inner(pattern.as_bytes(), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> Policy {
        Policy::new(
            &[
                "Read".into(),
                "Bash(pnpm test *)".into(),
                "Bash(git status *)".into(),
                "Edit(src/**)".into(),
            ],
            &["Bash(git push *)".into(), "Bash(rm -rf *)".into()],
        )
    }

    #[test]
    fn bare_tool_rule_matches_any_input() {
        assert!(matches!(
            policy().evaluate("Read", &json!({"file_path": "/etc/hosts"})),
            Verdict::Allow { .. }
        ));
    }

    #[test]
    fn prefix_pattern_matches_command() {
        assert!(matches!(
            policy().evaluate("Bash", &json!({"command": "pnpm test -- --run"})),
            Verdict::Allow { .. }
        ));
    }

    #[test]
    fn unmatched_command_is_undecided() {
        assert_eq!(
            policy().evaluate("Bash", &json!({"command": "curl evil.example"})),
            Verdict::Undecided
        );
    }

    #[test]
    fn deny_wins_over_allow() {
        // `Bash(git status *)` is allowed and `Bash(git push *)` denied; a push
        // must never be talked into an allow by rule ordering.
        let p = Policy::new(&["Bash(git *)".into()], &["Bash(git push *)".into()]);
        assert!(matches!(
            p.evaluate("Bash", &json!({"command": "git push origin main"})),
            Verdict::Deny { .. }
        ));
        assert!(matches!(
            p.evaluate("Bash", &json!({"command": "git status"})),
            Verdict::Allow { .. }
        ));
    }

    #[test]
    fn pattern_rule_needs_content() {
        // A tool whose input we cannot read must not be matched by a pattern
        // rule: matching on absent content would allow more than it says.
        let p = Policy::new(&["Bash(ls *)".into()], &[]);
        assert_eq!(p.evaluate("Bash", &json!({})), Verdict::Undecided);
    }

    #[test]
    fn space_before_star_is_significant() {
        let p = Policy::new(&["Bash(git diff *)".into()], &[]);
        assert_eq!(
            p.evaluate("Bash", &json!({"command": "git diff-index HEAD"})),
            Verdict::Undecided
        );
        assert!(matches!(
            p.evaluate("Bash", &json!({"command": "git diff HEAD"})),
            Verdict::Allow { .. }
        ));
    }

    #[test]
    fn glob_spans_path_separators() {
        let p = Policy::new(&["Edit(src/**)".into()], &[]);
        assert!(matches!(
            p.evaluate("Edit", &json!({"file_path": "src/a/b/c.rs"})),
            Verdict::Allow { .. }
        ));
    }

    #[test]
    fn tool_name_is_case_insensitive_but_distinct() {
        let p = Policy::new(&["Read".into()], &[]);
        assert!(matches!(
            p.evaluate("read", &json!({})),
            Verdict::Allow { .. }
        ));
        assert_eq!(p.evaluate("Write", &json!({})), Verdict::Undecided);
    }
}

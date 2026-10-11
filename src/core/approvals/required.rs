//! How many approvals the base branch requires, and where that number was
//! read — or that it could not be read, which is never *0 required*.

use serde::{Deserialize, Serialize};

/// One active `pull_request` rule on the base branch, as the branch's rules
/// return it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rule {
    pub count: u32,
    pub dismiss_stale: bool,
    pub code_owner: bool,
    pub last_push: bool,
}

/// Classic branch protection's review requirement, when it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Protection {
    pub count: u32,
    pub dismiss_stale: bool,
    pub code_owner: bool,
    pub last_push: bool,
}

/// Where a required count was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The branch's active rules (rulesets).
    Rules,
    /// Classic branch protection.
    BranchProtection,
}

impl Source {
    pub fn says(self) -> &'static str {
        match self {
            Source::Rules => "the branch's rules",
            Source::BranchProtection => "branch protection",
        }
    }
}

/// The number the base requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Required {
    #[serde(rename_all = "camelCase")]
    Count {
        n: u32,
        source: Source,
        dismiss_stale: bool,
        code_owner: bool,
        last_push: bool,
    },
    /// Both were read and neither requires an approval.
    NoneRequired,
    /// A sentence saying why; never rendered as zero.
    Unread { why: String },
}

/// The largest count among the active `pull_request` rules; when none
/// applies, classic protection's; otherwise none required, or unread.
///
/// `rules` is the branch's rules, or why they could not be read. `protection`
/// is `None` when it was not asked (a rule applied), else what it said: no
/// review requirement (`Ok(None)`), one, or why it could not be read.
pub fn required(
    rules: Result<Vec<Rule>, String>,
    protection: Option<Result<Option<Protection>, String>>,
) -> Required {
    if let Ok(rules) = &rules
        && let Some(n) = rules.iter().map(|r| r.count).max()
    {
        // Each flag is said when any rule says it.
        return Required::Count {
            n,
            source: Source::Rules,
            dismiss_stale: rules.iter().any(|r| r.dismiss_stale),
            code_owner: rules.iter().any(|r| r.code_owner),
            last_push: rules.iter().any(|r| r.last_push),
        };
    }
    match (rules, protection) {
        (_, Some(Ok(Some(p)))) => Required::Count {
            n: p.count,
            source: Source::BranchProtection,
            dismiss_stale: p.dismiss_stale,
            code_owner: p.code_owner,
            last_push: p.last_push,
        },
        (Ok(_), Some(Ok(None))) => Required::NoneRequired,
        (Ok(_), Some(Err(why))) => Required::Unread { why },
        (Ok(_), None) => Required::Unread {
            why: "branch protection was not read".into(),
        },
        (Err(why), _) => Required::Unread { why },
    }
}

/// The rule line: the count, where it was read, and what else the rule says.
/// Devplane states these; it does not evaluate whether they are met.
pub fn rule_line(r: &Required) -> Option<String> {
    let Required::Count {
        n,
        source,
        dismiss_stale,
        code_owner,
        last_push,
    } = r
    else {
        return None;
    };
    let mut parts = vec![match n {
        1 => "1 approval".to_string(),
        n => format!("{n} approvals"),
    }];
    if *dismiss_stale {
        parts.push("stale approvals are dismissed on push".into());
    }
    if *code_owner {
        parts.push("code-owner review required".into());
    }
    if *last_push {
        parts.push("the last push must be approved by someone else".into());
    }
    Some(format!(
        "Required by {}: {}.",
        source.says(),
        parts.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(count: u32) -> Rule {
        Rule {
            count,
            ..Rule::default()
        }
    }

    #[test]
    fn the_largest_rule_wins_and_any_rules_flag_is_said() {
        let r = required(
            Ok(vec![
                rule(1),
                Rule {
                    count: 2,
                    code_owner: true,
                    ..Rule::default()
                },
                Rule {
                    count: 0,
                    dismiss_stale: true,
                    ..Rule::default()
                },
            ]),
            None,
        );
        assert_eq!(
            r,
            Required::Count {
                n: 2,
                source: Source::Rules,
                dismiss_stale: true,
                code_owner: true,
                last_push: false
            }
        );
        assert_eq!(
            rule_line(&r).unwrap(),
            "Required by the branch's rules: 2 approvals; stale approvals are dismissed on push; \
             code-owner review required."
        );
    }

    #[test]
    fn without_a_rule_branch_protection_counts() {
        let r = required(
            Ok(vec![]),
            Some(Ok(Some(Protection {
                count: 1,
                ..Protection::default()
            }))),
        );
        assert!(matches!(
            r,
            Required::Count {
                n: 1,
                source: Source::BranchProtection,
                ..
            }
        ));
        assert_eq!(
            rule_line(&r).unwrap(),
            "Required by branch protection: 1 approval."
        );
    }

    #[test]
    fn none_required_only_when_both_were_read() {
        assert_eq!(required(Ok(vec![]), Some(Ok(None))), Required::NoneRequired);
        assert_eq!(
            required(
                Ok(vec![]),
                Some(Err("the token cannot read branch protection".into()))
            ),
            Required::Unread {
                why: "the token cannot read branch protection".into()
            }
        );
        assert_eq!(
            required(Err("rules unreadable".into()), Some(Ok(None))),
            Required::Unread {
                why: "rules unreadable".into()
            }
        );
        assert_eq!(
            required(
                Err("rules unreadable".into()),
                Some(Err("protection unreadable".into()))
            ),
            Required::Unread {
                why: "rules unreadable".into()
            }
        );
        // Unread readings have no rule line, so nothing renders a zero.
        assert_eq!(rule_line(&Required::Unread { why: "x".into() }), None);
    }
}

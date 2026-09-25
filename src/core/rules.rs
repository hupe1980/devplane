//! Which registered project is missing a rule, across both rule files.
//!
//! Reports and never applies: an agent runs as the same user as the host and
//! can reach its token, so a route that edited a permission file would be
//! reachable by the party the file bounds. `devplane.toml` (what Devplane
//! refuses) and `.claude/settings.json` (what the agent refuses) are never
//! conflated; every row names its file.

use crate::core::policy::{Class, Rule};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// `devplane.toml` — what Devplane will refuse.
    Devplane,
    /// `.claude/settings.json` — what the agent will refuse.
    Agent,
}

impl Source {
    /// The key a pasted rule of this class belongs under — the one each file
    /// actually reads.
    #[must_use]
    pub fn section(self, class: Class) -> &'static str {
        match (self, class) {
            (Source::Devplane, Class::Ask) => "policy.always_ask",
            (Source::Devplane, _) => "policy.never_auto",
            (Source::Agent, Class::Ask) => "permissions.ask",
            (Source::Agent, Class::Allow) => "permissions.allow",
            (Source::Agent, Class::Deny) => "permissions.deny",
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Devplane => "devplane",
            Source::Agent => "agent",
        }
    }
}

/// What one project's one file says about one rule. `Covered`: a wider rule
/// speaks for every call (`Bash(rm:*)` for `Bash(rm -rf:*)`). `Unreadable` is
/// neither covered nor missing: none of the file's settings are in effect, and
/// its fix is `devplane check`, not a paste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Coverage {
    Has,
    Covered { by: String },
    Missing,
    Unreadable { why: String },
}

impl Coverage {
    #[must_use]
    pub fn wants_the_rule(&self) -> bool {
        matches!(self, Coverage::Missing)
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Coverage::Has => "has",
            Coverage::Covered { .. } => "covered",
            Coverage::Missing => "missing",
            Coverage::Unreadable { .. } => "unreadable",
        }
    }
}

/// One file's rules, as read. When `error` is set the lists are empty: an
/// unparsable file contributes no rules.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleSet {
    pub file: String,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRules {
    pub project: String,
    pub devplane: RuleSet,
    pub agent: RuleSet,
}

/// One project, one file, one verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub project: String,
    pub source: Source,
    pub file: String,
    pub coverage: Coverage,
    /// Where the paste would go; `Some` only when `Missing`.
    pub section: Option<String>,
}

/// What one rule set says about one wanted rule. Under-reports on purpose, as
/// `covers_rule` does: an undecidable pair is `Missing`, never guessed — a
/// needless paste is cheaper than a hidden gap.
#[must_use]
pub fn coverage(rules: &[Rule], wanted: &Rule) -> Coverage {
    // Equivalence first, as mutual containment rather than `==`: `Rule`'s
    // `PartialEq` includes the raw text, so two spellings of one rule differ.
    if rules.iter().any(|r| same(r, wanted)) {
        return Coverage::Has;
    }
    // A negation carves a hole in any wider rule, so containment is unsound.
    if rules.iter().any(Rule::is_negated) {
        return Coverage::Missing;
    }
    if let Some(wider) = rules
        .iter()
        .find(|r| !r.is_malformed() && r.covers_rule(wanted))
    {
        return Coverage::Covered {
            by: wider.as_str().to_string(),
        };
    }
    Coverage::Missing
}

/// Whether two rules deny exactly the same calls; undecidable pairs differ.
fn same(a: &Rule, b: &Rule) -> bool {
    a.covers_rule(b) && b.covers_rule(a)
}

/// Parse a list, dropping what will not parse (`devplane check` reports those).
fn parsed(raw: &[String], class: Class) -> Vec<Rule> {
    raw.iter()
        .filter_map(|r| Rule::parse(r, class))
        .filter(|r| !r.is_malformed())
        .collect()
}

/// Every project's answer for one wanted rule: always both files per project,
/// so an absent `devplane.toml` shows as missing rather than vanishing.
#[must_use]
pub fn compare(projects: &[ProjectRules], wanted: &Rule) -> Vec<Row> {
    let mut out = Vec::new();
    for p in projects {
        for (source, set) in [(Source::Devplane, &p.devplane), (Source::Agent, &p.agent)] {
            let cov = match &set.error {
                Some(why) => Coverage::Unreadable { why: why.clone() },
                None => {
                    // `ask` counts too: covered means it cannot run unattended.
                    let mut rules = parsed(&set.deny, Class::Deny);
                    rules.extend(parsed(&set.ask, Class::Ask));
                    coverage(&rules, wanted)
                }
            };
            out.push(Row {
                project: p.project.clone(),
                source,
                file: set.file.clone(),
                section: cov
                    .wants_the_rule()
                    .then(|| source.section(wanted.class()).to_string()),
                coverage: cov,
            });
        }
    }
    out
}

/// A rule some projects hold and others do not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spread {
    pub rule: String,
    pub source: Source,
    pub held_by: Vec<String>,
    /// Never empty.
    pub missing_from: Vec<String>,
}

/// What the projects disagree about, most-widely-held first. A rule held by one
/// project is still a row. Unparsable files are on neither side.
#[must_use]
pub fn disagreements(projects: &[ProjectRules]) -> Vec<Spread> {
    let mut out = Vec::new();
    for (source, pick) in [
        (
            Source::Devplane,
            (|p: &ProjectRules| &p.devplane) as fn(&ProjectRules) -> &RuleSet,
        ),
        (Source::Agent, |p: &ProjectRules| &p.agent),
    ] {
        let readable: Vec<&ProjectRules> = projects
            .iter()
            .filter(|p| pick(p).error.is_none())
            .collect();
        if readable.len() < 2 {
            continue;
        }
        // Parsed, so two spellings of one rule are one row.
        let mut seen: Vec<(Rule, Vec<String>)> = Vec::new();
        for p in &readable {
            let set = pick(p);
            let mut rules = parsed(&set.deny, Class::Deny);
            rules.extend(parsed(&set.ask, Class::Ask));
            for r in rules {
                if r.is_negated() {
                    continue;
                }
                match seen.iter_mut().find(|(k, _)| same(k, &r)) {
                    Some((_, holders)) => holders.push(p.project.clone()),
                    None => seen.push((r, vec![p.project.clone()])),
                }
            }
        }
        for (rule, mut held_by) in seen {
            held_by.sort();
            held_by.dedup();
            let mut missing_from: Vec<String> = readable
                .iter()
                .map(|p| p.project.clone())
                .filter(|n| !held_by.contains(n))
                .collect();
            missing_from.sort();
            if missing_from.is_empty() {
                continue;
            }
            out.push(Spread {
                rule: rule.as_str().to_string(),
                source,
                held_by,
                missing_from,
            });
        }
    }
    // Ties break by rule text so the output is stable.
    out.sort_by(|a, b| {
        b.held_by
            .len()
            .cmp(&a.held_by.len())
            .then_with(|| a.rule.cmp(&b.rule))
            .then_with(|| a.source.as_str().cmp(b.source.as_str()))
    });
    out
}

/// Shown where somebody would look for *apply to all*.
pub const WHY_NO_APPLY_TO_ALL: &str = "There is no apply-to-all. Across 15 549 agentic pull requests in 148 projects, adding \
     instruction files helped in 27.7% of them and hurt in 26.35% — what separated the two was \
     what the rules said, not that they were there. Pasting one rule into six repositories is a \
     coin flip you would be running on purpose.";

/// Printed by every surface, so the refusal is never merely implied.
pub const WROTE_NOTHING: &str = "Nothing was written. Devplane reads these files and never edits \
                                 them — an agent on this machine runs as your user and can reach \
                                 anything the host can.";

#[cfg(test)]
mod tests {
    use super::*;

    fn set(deny: &[&str]) -> RuleSet {
        RuleSet {
            file: "f".into(),
            deny: deny.iter().map(|s| (*s).to_string()).collect(),
            ..Default::default()
        }
    }

    fn rules(deny: &[&str]) -> Vec<Rule> {
        deny.iter()
            .filter_map(|r| Rule::parse(r, Class::Deny))
            .collect()
    }

    fn want(raw: &str) -> Rule {
        Rule::parse(raw, Class::Deny).expect("a rule this syntax can express")
    }

    #[test]
    fn coverage_answers_only_what_containment_can_prove() {
        assert_eq!(
            coverage(&rules(&["Bash(curl:*)"]), &want("Bash(curl:*)")),
            Coverage::Has
        );
        assert_eq!(
            coverage(&rules(&["Bash(rm:*)"]), &want("Bash(rm -rf:*)")),
            Coverage::Covered {
                by: "Bash(rm:*)".into()
            }
        );
        // A narrower rule never covers a wider one.
        assert_eq!(
            coverage(&rules(&["Bash(rm -rf:*)"]), &want("Bash(rm:*)")),
            Coverage::Missing
        );
        // Undecidable (`?` is literal in a command) is `Missing`.
        assert_eq!(
            coverage(
                &rules(&["Bash(rm -rf /tmp/?:*)"]),
                &want("Bash(rm -rf /tmp/a:*)")
            ),
            Coverage::Missing
        );
    }

    #[test]
    fn an_exception_in_the_list_stops_a_wider_rule_speaking_for_it() {
        assert_eq!(
            coverage(
                &rules(&["Bash(rm:*)", "!Bash(rm -rf:*)"]),
                &want("Bash(rm -rf:*)")
            ),
            Coverage::Missing
        );
    }

    #[test]
    fn a_file_that_will_not_parse_is_never_reported_as_missing() {
        let broken = ProjectRules {
            project: "p".into(),
            devplane: RuleSet {
                file: "p/devplane.toml".into(),
                error: Some("expected `=` at line 4".into()),
                ..Default::default()
            },
            agent: set(&[]),
        };
        let rows = compare(&[broken], &want("Bash(curl:*)"));
        let dev = rows.iter().find(|r| r.source == Source::Devplane).unwrap();
        assert!(matches!(dev.coverage, Coverage::Unreadable { .. }));
        assert_eq!(dev.section, None);
        let agent = rows.iter().find(|r| r.source == Source::Agent).unwrap();
        assert_eq!(agent.coverage, Coverage::Missing);
        assert_eq!(agent.section.as_deref(), Some("permissions.deny"));
    }

    #[test]
    fn the_destination_key_follows_the_file_and_the_class() {
        assert_eq!(Source::Devplane.section(Class::Deny), "policy.never_auto");
        assert_eq!(Source::Devplane.section(Class::Ask), "policy.always_ask");
        assert_eq!(Source::Agent.section(Class::Deny), "permissions.deny");
        assert_eq!(Source::Agent.section(Class::Ask), "permissions.ask");
    }

    fn project(name: &str, deny: &[&str]) -> ProjectRules {
        ProjectRules {
            project: name.into(),
            devplane: RuleSet {
                file: format!("{name}/devplane.toml"),
                ..Default::default()
            },
            agent: set(deny),
        }
    }

    #[test]
    fn disagreements_are_most_widely_held_first_and_keep_the_lone_holder() {
        let out = disagreements(&[
            project("a", &["Read(./.env)", "Bash(curl:*)"]),
            project("b", &["Read(./.env)"]),
            project("c", &["Read(./.env)"]),
            project("d", &[]),
        ]);
        assert_eq!(out[0].rule, "Read(./.env)");
        assert_eq!(out[0].held_by, ["a", "b", "c"]);
        assert_eq!(out[0].missing_from, ["d"]);
        let lone = out
            .iter()
            .find(|s| s.rule == "Bash(curl:*)")
            .expect("the lone holder");
        assert_eq!(lone.held_by, ["a"]);
    }

    #[test]
    fn total_agreement_reports_nothing() {
        assert!(
            disagreements(&[
                project("a", &["Read(./.env)"]),
                project("b", &["Read(./.env)"])
            ])
            .is_empty()
        );
    }

    #[test]
    fn an_unreadable_project_does_not_invent_a_disagreement() {
        let mut broken = project("b", &[]);
        broken.agent.error = Some("unexpected end of input".into());
        let out = disagreements(&[project("a", &["Read(./.env)"]), broken]);
        assert!(
            out.is_empty(),
            "a readable project and an unreadable one cannot disagree: {out:?}"
        );
    }

    /// `Bash( rm:* )` is a pattern beginning with a space; don't "fix" this by
    /// trimming inside the brackets.
    #[test]
    fn spacing_around_an_entry_is_not_a_disagreement_and_spacing_inside_one_is() {
        assert!(
            disagreements(&[
                project("a", &["Bash(rm:*)"]),
                project("b", &["  Bash(rm:*)  "])
            ])
            .is_empty(),
            "a stray space around a list entry read as a different rule"
        );
        let inner = disagreements(&[
            project("a", &["Bash(rm:*)"]),
            project("b", &["Bash( rm:* )"]),
        ]);
        assert_eq!(
            inner.len(),
            2,
            "a pattern that begins with a space denies different calls and must not be folded \
             into one row: {inner:?}"
        );
    }
}

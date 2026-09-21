//! Which repository is missing the rule.
//!
//! **The fleet half of *answer once*.** `core::offer` composes the exact rule
//! text for a single call on a single machine. This asks the same question
//! across every registered project: *which of my six repositories is missing
//! this?*
//!
//! # Why this reports and does not apply
//!
//! Over **15 549 agentic pull requests across 148 projects**, adding
//! instruction files raised the merge rate by ≥20 % in **27.7 %** of projects
//! and **lowered** it in **26.35 %**. What separated the two was length and
//! structure, not presence. So *which repositories are missing this rule* is a
//! question worth answering, and *therefore put it in all six* is the failure
//! that result measures — a coin flip with a progress bar.
//!
//! The refusal is structural rather than a default, and for a second reason
//! that stands on its own: **an agent on this machine runs as the same user as
//! the daemon and can read the bearer token**, so a route that edits a
//! permission file is reachable by the party the file exists to bound. Writing
//! into the *vendor's* settings would be strictly worse — that is the file the
//! vendor's own enforcement reads.
//!
//! This module therefore has no write path, and `tests/purity.rs` proves the
//! absence over the source rather than trusting this paragraph.
//!
//! # Why the two rule sets are never conflated
//!
//! A `devplane.toml` prohibition is what **this product** will refuse. A
//! `permissions.deny` entry is what the **agent** will refuse. They answer
//! different questions and a person with six projects needs the second at
//! least as much, so every row names the file it is about.

use crate::core::policy::{Class, Rule};
use serde::{Deserialize, Serialize};

/// Which file a rule set came from, and therefore which question it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// `devplane.toml` — what Devplane will refuse.
    Devplane,
    /// `.claude/settings.json` — what the agent will refuse.
    Agent,
}

impl Source {
    /// The key a rule of this class belongs under, for the paste.
    ///
    /// Two files, two spellings of the same idea, and getting it wrong hands
    /// somebody text that lands in a key nothing reads. `offer.rs` had exactly
    /// that failure with an allow list that had stopped being evaluated.
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

/// What one project's one file says about one rule.
///
/// **Four states, and the fourth is why this is not a boolean.** A project
/// whose settings will not parse has, per the vendor, *none* of its settings in
/// effect — so it is neither covered nor missing, and its fix is `devplane
/// check` rather than a paste. Reporting it as missing would hand somebody text
/// to add to a file that is already broken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Coverage {
    /// The rule is there, spelled the same way.
    Has,
    /// A **wider** rule already speaks for every call this one names.
    ///
    /// Deliberately distinct from `Has`: `Bash(rm:*)` covers `Bash(rm -rf:*)`,
    /// and a person deciding whether to paste needs to know they are looking at
    /// a broader rule rather than the one they wrote.
    Covered { by: String },
    /// Nothing here speaks for it.
    Missing,
    /// The file does not parse, so none of its settings are in effect.
    Unreadable { why: String },
}

impl Coverage {
    /// Whether this project would need the paste.
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

/// One file's rules, as read.
///
/// `error` and the rule lists are **mutually exclusive by construction** at the
/// call site: a file that did not parse contributes no rules, because the
/// vendor puts none of its settings in effect.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleSet {
    pub file: String,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
    /// Why the file could not be read, when it could not.
    pub error: Option<String>,
}

/// One project's two rule sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRules {
    pub project: String,
    pub devplane: RuleSet,
    pub agent: RuleSet,
}

/// One row of the answer: one project, one file, one verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub project: String,
    pub source: Source,
    /// The file this row is about, so two rows for one project cannot be read
    /// as one.
    pub file: String,
    pub coverage: Coverage,
    /// Where the text would go, when it is missing. `None` otherwise — there is
    /// nothing to paste into a file that already covers the case, and nothing
    /// to paste into one that does not parse.
    pub section: Option<String>,
}

/// What one rule set says about one wanted rule.
///
/// **Under-reports on purpose, because `covers_rule` does.** Containment is
/// decided exactly where this rule language allows it and answers *no*
/// everywhere else — a negation, a differing tool pattern, a `?` in a command
/// pattern. An undecidable pair is therefore reported as `Missing`, never
/// guessed at, and the cost of that choice is showing somebody a paste they may
/// not need. The opposite error hides a gap, which is the one this feature
/// exists to find.
#[must_use]
pub fn coverage(rules: &[Rule], wanted: &Rule) -> Coverage {
    // **Equivalence first**, because *you already have this* and *something
    // wider speaks for it* are different answers and the narrower one is more
    // useful.
    //
    // Equivalence is **mutual containment**, not string equality. `Rule`
    // derives `PartialEq` over its fields including the raw text, so
    // `Bash(rm:*)` and `Bash(rm :*)` compare unequal while denying exactly the
    // same calls — and a reader who wrote the second would be told they are
    // missing the first. Each covering the other is the definition of the same
    // rule, and it reuses the one procedure rather than adding a second reader
    // of this syntax.
    if rules.iter().any(|r| same(r, wanted)) {
        return Coverage::Has;
    }
    // **A negation anywhere in the list stops containment being sound.** An
    // exception carves a hole in the rule above it, so a wider rule with a
    // `!Bash(rm -rf /tmp/*)` under it no longer speaks for everything it names.
    // `Policy::redundancies` takes the same care for the same reason.
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

/// Whether two rules deny exactly the same calls.
///
/// **Mutual containment, which is the definition.** `covers_rule` under-reports
/// on purpose — a pair it cannot decide answers *no* — so two rules it cannot
/// compare are treated as different, which is the safe direction: it shows a
/// row that may be redundant rather than hiding one that is not.
fn same(a: &Rule, b: &Rule) -> bool {
    a.covers_rule(b) && b.covers_rule(a)
}

/// Parse a list, dropping what will not parse.
///
/// A rule the parser rejects is already reported by `devplane check` and by the
/// agent itself at startup; repeating it here would say the same thing twice
/// under a heading about something else.
fn parsed(raw: &[String], class: Class) -> Vec<Rule> {
    raw.iter()
        .filter_map(|r| Rule::parse(r, class))
        .filter(|r| !r.is_malformed())
        .collect()
}

/// Every project's answer for one wanted rule, one row per file.
///
/// Both files are reported for every project, always — including the one that
/// has nothing to say. A project with no `devplane.toml` is *missing* the rule
/// in Devplane's file, and omitting the row would quietly narrow the question
/// from *which repositories* to *which repositories I happened to configure*.
#[must_use]
pub fn compare(projects: &[ProjectRules], wanted: &Rule) -> Vec<Row> {
    let mut out = Vec::new();
    for p in projects {
        for (source, set) in [(Source::Devplane, &p.devplane), (Source::Agent, &p.agent)] {
            let cov = match &set.error {
                Some(why) => Coverage::Unreadable { why: why.clone() },
                None => {
                    // Both lists, because a rule in `ask` does speak for the
                    // call — it interrupts instead of refusing, and a person
                    // asking "is this covered" is asking whether the call can
                    // happen unattended.
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
    /// Projects whose file names this rule, sorted.
    pub held_by: Vec<String>,
    /// Projects whose file does not, sorted. Never empty — a rule everybody
    /// holds is agreement, not a disagreement.
    pub missing_from: Vec<String>,
}

/// What the projects disagree about, most-widely-held first.
///
/// **A rule held by exactly one project is a row, not noise.** One repository
/// carrying a prohibition the other five lack is the most interesting thing
/// this report can say — it is either the one that learned something or the one
/// that is over-restricted, and both are worth a look. Filtering it as an
/// outlier would drop the signal to keep the list short.
///
/// Projects whose file will not parse are **excluded from both sides** of every
/// row. They have no settings in effect, so counting them as missing would
/// invent a disagreement out of a broken file.
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
        // Fewer than two readable files cannot disagree about anything.
        if readable.len() < 2 {
            continue;
        }
        // Compared as parsed rules, so two spellings of one rule are one row.
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
    // Most-widely-held first: the rule five of six projects share is the one
    // most likely to be an oversight in the sixth. Ties break by rule text so
    // two runs over the same machine print the same list.
    out.sort_by(|a, b| {
        b.held_by
            .len()
            .cmp(&a.held_by.len())
            .then_with(|| a.rule.cmp(&b.rule))
            .then_with(|| a.source.as_str().cmp(b.source.as_str()))
    });
    out
}

/// The one sentence shown where somebody would look for *apply to all*.
///
/// **It is about the evidence, not the architecture.** "We cannot write files"
/// reads as a limitation somebody will ask to have lifted. The measured result
/// is the actual reason, and it does not change when the threat model does.
pub const WHY_NO_APPLY_TO_ALL: &str = "There is no apply-to-all. Across 15 549 agentic pull requests in 148 projects, adding \
     instruction files helped in 27.7% of them and hurt in 26.35% — what separated the two was \
     what the rules said, not that they were there. Pasting one rule into six repositories is a \
     coin flip you would be running on purpose.";

/// The line every surface prints, so the refusal is never merely implied.
pub const WROTE_NOTHING: &str = "Nothing was written. Devplane reads these files and never edits \
                                 them — an agent on this machine runs as your user and can reach \
                                 anything the daemon can.";

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

    /// The four containment cases, on the same procedure `Policy::redundancies`
    /// uses — an exact match, a wider rule, a narrower one, and a pair the
    /// procedure cannot decide.
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
        // **A narrower rule does not cover a wider one**, and getting this
        // backwards is the failure that matters: it would report a project as
        // protected by a rule that answers one call out of the family.
        assert_eq!(
            coverage(&rules(&["Bash(rm -rf:*)"]), &want("Bash(rm:*)")),
            Coverage::Missing
        );
        // Undecidable is silent, which is to say `Missing`. A `?` is a wildcard
        // in a path and a literal in a command, so the containment primitive
        // refuses the pair rather than picking a reading.
        assert_eq!(
            coverage(
                &rules(&["Bash(rm -rf /tmp/?:*)"]),
                &want("Bash(rm -rf /tmp/a:*)")
            ),
            Coverage::Missing
        );
    }

    /// **A wider rule with an exception under it covers nothing.**
    ///
    /// The hole the negation carves could be exactly the call being asked
    /// about, and this procedure cannot tell. Reporting `covered` here would
    /// tell somebody they are protected by a rule that has been subtracted
    /// from.
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

    /// Unreadable and missing are different rows with different fixes.
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
        // **And it is offered no paste.** A file that does not parse has none
        // of its settings in effect, so text added to it changes nothing — the
        // fix is `devplane check`.
        assert_eq!(dev.section, None);
        let agent = rows.iter().find(|r| r.source == Source::Agent).unwrap();
        assert_eq!(agent.coverage, Coverage::Missing);
        assert_eq!(agent.section.as_deref(), Some("permissions.deny"));
    }

    /// The paste names the key each file actually reads.
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
        // **The rule exactly one project holds is a row, not noise.** It is
        // either the project that learned something or the one that is
        // over-restricted, and both are worth a look.
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

    /// **A project whose file will not parse is on neither side of a spread.**
    ///
    /// It has no settings in effect, so counting it as missing would invent a
    /// disagreement out of a broken file — and send somebody to paste a rule
    /// into it.
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

    /// **Whitespace around an entry is not a disagreement; whitespace inside a
    /// specifier is.**
    ///
    /// The first is a stray space in a JSON list and means nothing. The second
    /// does not: `Bash( rm:* )` is a pattern that begins with a space, and it
    /// does not speak for `rm` — the parser is right to keep them apart, and
    /// this pins the distinction so nobody "fixes" it by trimming inside the
    /// brackets.
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

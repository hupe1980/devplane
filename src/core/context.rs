//! What a prompt may say about the project it is going to.
//!
//! # Why this is not a template engine
//!
//! Every prompt library in the field is a text snippet manager, because a
//! snippet manager is all any of them can be: none of them is also watching the
//! session, the gate and the repository. **A prompt that can say *the gate
//! failed on these three tests* and mean it is a different object from one that
//! says *fix the failing tests*.**
//!
//! That is the whole feature, and it needs no language. A **closed set** of
//! named values, one pass, no loops and no conditionals. The moment an engine
//! appears, this product owns a language as well as a format, and the argument
//! against owning a format applies unchanged.
//!
//! # The failure to design against is the empty string
//!
//! **A placeholder that cannot be resolved refuses.** Never an empty string,
//! never the literal text, never a silent omission — a confidently wrong prompt
//! sent to an agent is worse than a refused one, and it is the direction every
//! templating system in existence defaults to.
//!
//! # And where it may be written
//!
//! `.devplane/prompts/` is Devplane's own portable form and may carry these. A
//! vendor's `SKILL.md` is **never scanned**: it is passed through byte for byte
//! and takes values as arguments, which is what its `argument-hint` is for.
//! Templating somebody else's file is owning their format.

use std::fmt;

/// The most a resolved prompt may grow to.
///
/// A plan's outline or a flood of gate failures can dwarf the prompt that
/// carries them. Bounded, and a truncation **says what it dropped** rather than
/// quietly shortening somebody's instructions.
pub const MAX_VALUE: usize = 4_000;

/// Everything a prompt may name.
///
/// **Closed, and adding a member is a change with a reason.** Each one must
/// resolve from state this daemon already holds for that project; a member that
/// needed a new reader is a new reader, and belongs in its own change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    Project,
    Branch,
    BaseBranch,
    Dirty,
    /// The specification the in-flight work names.
    PlanPath,
    /// How many boxes are still unticked.
    PlanOpen,
    /// How many lines the project's own words mark unresolved.
    PlanQuestions,
    /// What the last gate actually failed on, already reduced to the lines that
    /// matter by the same extractor the gate report uses.
    GateFailures,
    /// The last decision and, separately, on whose authority.
    LastDecision,
    LastAuthority,
    /// A question an agent asked that nobody answered.
    UnansweredQuestion,
}

impl Field {
    /// Every member, so nothing enumerates them by hand.
    pub const ALL: &'static [Field] = &[
        Field::Project,
        Field::Branch,
        Field::BaseBranch,
        Field::Dirty,
        Field::PlanPath,
        Field::PlanOpen,
        Field::PlanQuestions,
        Field::GateFailures,
        Field::LastDecision,
        Field::LastAuthority,
        Field::UnansweredQuestion,
    ];

    /// What a prompt writes between the braces.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Field::Project => "project",
            Field::Branch => "branch",
            Field::BaseBranch => "base_branch",
            Field::Dirty => "dirty",
            Field::PlanPath => "plan.path",
            Field::PlanOpen => "plan.open",
            Field::PlanQuestions => "plan.questions",
            Field::GateFailures => "gate.failures",
            Field::LastDecision => "decision.last",
            Field::LastAuthority => "decision.authority",
            Field::UnansweredQuestion => "question.unanswered",
        }
    }

    /// What `devplane` prints when somebody asks what a prompt may say.
    #[must_use]
    pub fn about(self) -> &'static str {
        match self {
            Field::Project => "the project's name",
            Field::Branch => "the branch the worktree is on",
            Field::BaseBranch => "what the project branches from",
            Field::Dirty => "whether the worktree has uncommitted changes",
            Field::PlanPath => "the specification the work in flight names",
            Field::PlanOpen => "how many of its boxes are still unticked",
            Field::PlanQuestions => "how many lines it marks unresolved",
            Field::GateFailures => "what the last gate failed on",
            Field::LastDecision => "the last decision recorded here",
            Field::LastAuthority => "on whose authority it was taken",
            Field::UnansweredQuestion => "a question an agent asked and nobody answered",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Field::ALL.iter().copied().find(|f| f.name() == s)
    }
}

/// One value, and **when it was true**.
///
/// A gate that has not run, a plan nobody is working to, a decision from last
/// week: a value with no time on it reads as current, and a prompt that tells
/// an agent about last Tuesday's failures in the present tense is worse than
/// one that says nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub text: String,
    pub at: jiff::Timestamp,
}

impl Value {
    pub fn new(text: impl Into<String>, at: jiff::Timestamp) -> Self {
        Self {
            text: text.into(),
            at,
        }
    }
}

/// What one target can answer.
///
/// Built by the caller that has the disk; this module resolves and never reads.
#[derive(Debug, Clone, Default)]
pub struct Context {
    values: std::collections::BTreeMap<Field, Value>,
}

impl Context {
    pub fn with(mut self, field: Field, value: Value) -> Self {
        self.values.insert(field, value);
        self
    }

    pub fn get(&self, field: Field) -> Option<&Value> {
        self.values.get(&field)
    }
}

/// Why a prompt could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unresolved {
    /// A name outside the closed set.
    NoSuchField(String),
    /// A member of the set with nothing behind it for this target.
    NothingToSay(Field),
}

impl fmt::Display for Unresolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unresolved::NoSuchField(n) => write!(
                f,
                "`{{{n}}}` is not something a prompt can say. It knows: {}",
                Field::ALL
                    .iter()
                    .map(|f| f.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Unresolved::NothingToSay(field) => write!(
                f,
                "`{{{}}}` — {} — and this project has none right now",
                field.name(),
                field.about()
            ),
        }
    }
}

/// Fills a portable prompt's placeholders, or says which it could not.
///
/// **One pass.** A resolved value that happens to contain the syntax is not
/// re-scanned: a gate failure carrying `{project}` in its text is a failure
/// message, not an instruction.
///
/// **A placeholder inside a fenced block is left alone**, because a prompt that
/// documents this mechanism would otherwise rewrite its own example — the same
/// argument the specification reader already makes one delimiter larger.
pub fn resolve(template: &str, ctx: &Context) -> Result<String, Vec<Unresolved>> {
    let mut out = String::with_capacity(template.len());
    let mut problems: Vec<Unresolved> = Vec::new();
    let mut fenced = false;

    for (n, line) in template.lines().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fenced = !fenced;
            out.push_str(line);
            continue;
        }
        if fenced {
            out.push_str(line);
            continue;
        }
        resolve_line(line, ctx, &mut out, &mut problems);
    }
    if template.ends_with('\n') {
        out.push('\n');
    }

    match problems.is_empty() {
        true => Ok(out),
        false => {
            problems.dedup();
            Err(problems)
        }
    }
}

fn resolve_line(line: &str, ctx: &Context, out: &mut String, problems: &mut Vec<Unresolved>) {
    let b: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] != '{' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let Some(end) = b[i + 1..].iter().position(|&c| c == '}').map(|p| i + 1 + p) else {
            // An unclosed brace is a brace. Prose is allowed to contain one.
            out.push(b[i]);
            i += 1;
            continue;
        };
        let name: String = b[i + 1..end].iter().collect();
        // A brace around something that is not a name is prose too: `{1}`,
        // `{"a": 1}`, a JSON example in an instruction.
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
        {
            out.push(b[i]);
            i += 1;
            continue;
        }
        match Field::parse(&name) {
            None => problems.push(Unresolved::NoSuchField(name)),
            Some(field) => match ctx.get(field) {
                None => problems.push(Unresolved::NothingToSay(field)),
                Some(v) => out.push_str(&clip(&v.text)),
            },
        }
        i = end + 1;
    }
}

/// Bounded, and a truncation says what it dropped.
fn clip(s: &str) -> String {
    if s.chars().count() <= MAX_VALUE {
        return s.to_string();
    }
    let kept: String = s.chars().take(MAX_VALUE).collect();
    let dropped = s.chars().count() - MAX_VALUE;
    format!("{kept}\n… and {dropped} more characters, not included")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> jiff::Timestamp {
        "2026-09-23T10:00:00Z".parse().unwrap()
    }

    fn ctx() -> Context {
        Context::default()
            .with(Field::Project, Value::new("payments-api", at()))
            .with(
                Field::GateFailures,
                Value::new("test_login\ntest_logout", at()),
            )
    }

    /// **The whole feature in one assertion.**
    #[test]
    fn a_prompt_can_say_what_the_gate_actually_failed_on() {
        let out = resolve(
            "In {project}, the checks failed on:\n{gate.failures}\nFix the cause.",
            &ctx(),
        )
        .expect("both are known");
        assert!(out.contains("payments-api"));
        assert!(out.contains("test_login"));
        assert!(!out.contains('{'), "a placeholder survived: {out}");
    }

    /// **An unresolved placeholder refuses, and names which.**
    ///
    /// Never an empty string, never the literal text, never a silent omission.
    /// A confidently wrong prompt sent to an agent is worse than a refused one,
    /// and it is the direction every templating system defaults to.
    #[test]
    fn a_placeholder_with_nothing_behind_it_refuses() {
        let e = resolve("In {project}: {plan.open} boxes", &ctx())
            .expect_err("the plan is not in this context");
        assert_eq!(e.len(), 1);
        let says = e[0].to_string();
        assert!(says.contains("plan.open"), "{says}");
        assert!(says.contains("none right now"), "{says}");
    }

    /// **The absence test, over every member of the closed set.**
    ///
    /// Nothing may reach an agent as an empty string or as the literal text.
    #[test]
    fn no_member_of_the_set_can_reach_an_agent_unresolved() {
        for f in Field::ALL {
            let template = format!("before {{{}}} after", f.name());
            let r = resolve(&template, &Context::default());
            let e = r.unwrap_or_else(|_| String::new());
            assert!(
                e.is_empty(),
                "`{}` resolved to something with nothing behind it: {e:?}",
                f.name()
            );
        }
    }

    /// A name outside the set is refused and the set is named.
    #[test]
    fn a_name_nobody_defined_is_refused_with_the_list() {
        let e = resolve("{cost.today}", &ctx()).expect_err("not a field");
        let says = e[0].to_string();
        assert!(says.contains("not something a prompt can say"), "{says}");
        for f in [Field::Project, Field::GateFailures] {
            assert!(says.contains(f.name()), "the list omits {}", f.name());
        }
    }

    /// **One pass.** A value that contains the syntax is not re-scanned.
    #[test]
    fn a_value_carrying_the_syntax_is_not_substituted_again() {
        let c = Context::default().with(Field::Project, Value::new("weird-{branch}-name", at()));
        let out = resolve("{project}", &c).expect("resolves");
        assert_eq!(
            out, "weird-{branch}-name",
            "a resolved value was scanned a second time"
        );
    }

    /// **A placeholder inside a fenced block is documentation.**
    #[test]
    fn a_fenced_example_is_left_alone() {
        let t = "Use it like this:\n```\n{project}\n```\nand here: {project}";
        let out = resolve(t, &ctx()).expect("resolves");
        assert!(
            out.contains("```\n{project}\n```"),
            "a documented example was rewritten: {out}"
        );
        assert!(out.ends_with("and here: payments-api"), "{out}");
    }

    /// Braces that are not placeholders are prose.
    #[test]
    fn prose_containing_braces_is_not_a_template() {
        for t in ["a {1} b", r#"{"key": 1}"#, "unclosed { here", "{}", "{A}"] {
            let out = resolve(t, &ctx()).unwrap_or_else(|e| panic!("{t:?} refused: {e:?}"));
            assert_eq!(out, t, "{t:?} was treated as a placeholder");
        }
    }

    /// **Bounded, and a truncation says what it dropped.**
    #[test]
    fn a_very_large_value_is_bounded_and_says_so() {
        let huge = "x".repeat(MAX_VALUE + 500);
        let c = Context::default().with(Field::GateFailures, Value::new(huge, at()));
        let out = resolve("{gate.failures}", &c).expect("resolves");
        assert!(out.len() < MAX_VALUE + 200, "the bound did not hold");
        assert!(
            out.contains("500 more characters, not included"),
            "a truncation was silent: {}",
            &out[out.len().saturating_sub(80)..]
        );
    }

    /// Every field has a name and a sentence, or the refusal cannot be read.
    #[test]
    fn every_field_is_nameable_and_explainable() {
        let mut names = std::collections::BTreeSet::new();
        for f in Field::ALL {
            assert!(names.insert(f.name()), "`{}` is named twice", f.name());
            assert!(!f.about().is_empty(), "`{}` has no sentence", f.name());
            assert_eq!(
                Field::parse(f.name()),
                Some(*f),
                "`{}` does not parse back",
                f.name()
            );
        }
    }

    /// Several problems are all reported, not just the first — a person fixing
    /// a prompt wants the list rather than one round trip per placeholder.
    #[test]
    fn every_problem_is_reported_at_once() {
        let e =
            resolve("{nope} and {plan.open} and {alsonope}", &ctx()).expect_err("three problems");
        assert_eq!(e.len(), 3, "{e:?}");
    }
}

#[cfg(test)]
mod refusals {
    use super::*;

    /// **A vendor's artefact is never scanned**, and this is the absence that
    /// says so.
    ///
    /// `.devplane/prompts/` is Devplane's own portable form and may carry
    /// placeholders. A `SKILL.md` is the vendor's: it passes through byte for
    /// byte and takes values as arguments, which is what its `argument-hint` is
    /// for. Templating somebody else's file is owning their format, which is
    /// the thing the library exists not to do.
    ///
    /// Held here as a property of the **module** rather than of a call site:
    /// nothing in this file knows what a skill is, so nothing in it can
    /// substitute into one.
    /// **Comments are stripped before the check**, for the third time in this
    /// repository. A guard that reads a file for a forbidden word matches the
    /// paragraph explaining why the word is forbidden — so it fails on a
    /// correct file and can only be satisfied by deleting its own reasoning.
    fn code_only() -> String {
        include_str!("context.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or_default()
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn this_module_knows_nothing_about_a_vendors_format() {
        let code = code_only();
        for forbidden in ["SKILL.md", ".claude/skills", "frontmatter"] {
            assert!(
                !code.contains(forbidden),
                "this resolver has learned about `{forbidden}`, which is the first \
                 step to substituting into a file this product does not own"
            );
        }
    }

    /// **Nothing here writes**, so an artefact's digest cannot change.
    ///
    /// The library's central refusal is that a `SKILL.md` in it is byte
    /// identical to the one in the repository, and the digest proves it. The
    /// strongest form of that guarantee is that the substitution path has no
    /// way to write at all.
    #[test]
    fn nothing_on_this_path_can_write_a_file() {
        let code = code_only();
        for forbidden in ["fs::write", "File::create", "OpenOptions", "fs::copy"] {
            assert!(
                !code.contains(forbidden),
                "the resolver can write (`{forbidden}`); byte identity is then a \
                 claim rather than a property"
            );
        }
    }

    /// **No loops, no conditionals, no expressions.**
    ///
    /// A closed set of names, resolved once. The moment an engine appears this
    /// product owns a language as well as a format, and the argument against
    /// owning a format applies unchanged.
    #[test]
    fn the_syntax_is_a_name_and_nothing_else() {
        let c = Context::default().with(
            Field::Project,
            Value::new("p", "2026-09-23T10:00:00Z".parse().unwrap()),
        );
        // Anything that looks like an expression is prose.
        for t in [
            "{#if project}x{/if}",
            "{project | upper}",
            "{{project}}",
            "{for p in projects}",
            "{project()}",
        ] {
            let out = resolve(t, &c);
            assert!(
                out.as_ref().map(|o| o.contains('{')).unwrap_or(true),
                "`{t}` was evaluated as an expression: {out:?}"
            );
        }
    }
}

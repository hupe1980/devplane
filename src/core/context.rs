//! What a prompt may say about the project it is going to.
//!
//! A closed set of named values substituted in one pass — no loops, no
//! conditionals, no engine. A placeholder that cannot be resolved refuses:
//! never an empty string, never the literal text. Only prompts a person types
//! are resolved; a vendor's `SKILL.md` is passed through byte for byte.

use std::fmt;

/// The most one resolved value may grow to; a truncation says what it dropped.
pub const MAX_VALUE: usize = 4_000;

/// Everything a prompt may name. Closed: each member must resolve from state
/// already held for the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    Project,
    Branch,
    BaseBranch,
    Dirty,
    PlanPath,
    PlanOpen,
    PlanQuestions,
    /// Reduced by the same extractor the gate report uses.
    GateFailures,
    LastDecision,
    LastAuthority,
    UnansweredQuestion,
    /// One `- <text>` line each; bound only when some were chosen, so naming it
    /// with none refuses rather than sending an empty block.
    Tasks,
    /// Never refuses: none resolved is stated as [`NO_REPORTS`].
    Reports,
}

pub const NO_REPORTS: &str = "no reports have been resolved for this change";

/// The last sentence of every sent prompt: a sanctioned way to say *I cannot*,
/// so a stuck agent does not make the checks pass by editing or skipping them.
pub const STOP_AND_SAY: &str = "If the specification cannot be satisfied, stop and say so \
plainly rather than making the checks pass another way.";

/// `prompt`, ending with [`STOP_AND_SAY`] exactly once.
pub fn with_stop_and_say(prompt: &str) -> String {
    let body = without_stop_and_say(prompt);
    format!("{}\n\n{STOP_AND_SAY}\n", body.trim_end())
}

/// `prompt` without [`STOP_AND_SAY`] — for where the prompt is shown to a
/// person as the change's description, such as a pull request body.
pub fn without_stop_and_say(prompt: &str) -> String {
    prompt.replace(STOP_AND_SAY, "").trim_end().to_string()
}

impl Field {
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
        Field::Tasks,
        Field::Reports,
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
            Field::Tasks => "tasks",
            Field::Reports => "reports",
        }
    }

    #[must_use]
    pub fn about(self) -> &'static str {
        match self {
            Field::Project => "the project's name",
            Field::Branch => "the branch the worktree is on",
            Field::BaseBranch => "what the project branches from",
            Field::Dirty => "whether the worktree has uncommitted changes",
            Field::PlanPath => "the specification the change in flight names",
            Field::PlanOpen => "how many of its boxes are still unticked",
            Field::PlanQuestions => "how many lines it marks unresolved",
            Field::GateFailures => "what the last gate failed on",
            Field::LastDecision => "the last decision recorded here",
            Field::LastAuthority => "on whose authority it was taken",
            Field::UnansweredQuestion => "a question an agent asked and nobody answered",
            Field::Tasks => "the tasks this dispatch sends, one line each",
            Field::Reports => "what became of the reports this change filed about other projects",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Field::ALL.iter().copied().find(|f| f.name() == s)
    }
}

/// One value and when it was true — an undated value reads as current.
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

/// What one target can answer. Built by the caller; this module never reads.
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
    NoSuchField(String),
    NothingToSay(Field),
    /// Its own refusal: the fix is choosing tasks, not changing the project.
    NoTasksSelected,
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
            Unresolved::NoTasksSelected => {
                write!(f, "`{{tasks}}` — no tasks were selected for this dispatch")
            }
        }
    }
}

/// Fills a prompt's placeholders, or lists every one it could not. One pass:
/// resolved values are not re-scanned. Fenced blocks are left alone so a
/// documented example is not rewritten.
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
            out.push(b[i]);
            i += 1;
            continue;
        };
        let name: String = b[i + 1..end].iter().collect();
        // `{1}`, `{"a": 1}`: braces around a non-name are prose.
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
                None if field == Field::Tasks => problems.push(Unresolved::NoTasksSelected),
                None => problems.push(Unresolved::NothingToSay(field)),
                Some(v) => out.push_str(&clip(&v.text)),
            },
        }
        i = end + 1;
    }
}

/// Whether a template places `field` itself, outside fenced blocks — so a
/// caller that would otherwise append a block knows not to.
pub fn names(template: &str, field: Field) -> bool {
    let want = format!("{{{}}}", field.name());
    let mut fenced = false;
    template.lines().any(|line| {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            return false;
        }
        !fenced && line.contains(&want)
    })
}

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

    #[test]
    fn the_stop_sentence_ends_a_prompt_exactly_once() {
        let once = with_stop_and_say("Do the thing.");
        assert!(once.trim_end().ends_with(STOP_AND_SAY));
        assert_eq!(once.matches(STOP_AND_SAY).count(), 1);
        let twice = with_stop_and_say(&once);
        assert_eq!(twice.matches(STOP_AND_SAY).count(), 1);
        assert_eq!(without_stop_and_say(&once), "Do the thing.");
        assert_eq!(STOP_AND_SAY.matches(". ").count(), 0, "one sentence");
    }

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

    #[test]
    fn a_template_may_place_the_tasks_and_refuses_when_none_were_chosen() {
        let with = ctx().with(
            Field::Tasks,
            Value::new("- T001 do it\n- T002 test it", at()),
        );
        let out = resolve("Do these:\n{tasks}\n", &with).expect("bound");
        assert!(out.contains("- T001 do it\n- T002 test it"));

        let err = resolve("Do these:\n{tasks}\n", &ctx()).unwrap_err();
        assert_eq!(err, [Unresolved::NoTasksSelected]);
        assert!(
            err[0]
                .to_string()
                .contains("no tasks were selected for this dispatch"),
            "{}",
            err[0]
        );

        assert!(names("Do {tasks} now", Field::Tasks));
        assert!(!names("Do the work", Field::Tasks));
        assert!(
            !names("```\n{tasks}\n```\n", Field::Tasks),
            "a fenced example is not a placement"
        );
        assert_eq!(Field::parse("tasks"), Some(Field::Tasks));
    }

    #[test]
    fn the_reports_field_carries_what_it_is_given_and_says_so_when_empty() {
        let quiet = ctx().with(Field::Reports, Value::new(NO_REPORTS, at()));
        let out = resolve("Since last time: {reports}", &quiet).expect("bound");
        assert!(out.ends_with(NO_REPORTS), "{out}");
        let told = ctx().with(
            Field::Reports,
            Value::new(
                "Report rp-1 to core-lib — rejected\n> reason: by design",
                at(),
            ),
        );
        let out = resolve("{reports}", &told).expect("bound");
        assert!(out.contains("> reason: by design"), "{out}");
        assert_eq!(Field::parse("reports"), Some(Field::Reports));
    }

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

    #[test]
    fn a_placeholder_with_nothing_behind_it_refuses() {
        let e = resolve("In {project}: {plan.open} boxes", &ctx())
            .expect_err("the plan is not in this context");
        assert_eq!(e.len(), 1);
        let says = e[0].to_string();
        assert!(says.contains("plan.open"), "{says}");
        assert!(says.contains("none right now"), "{says}");
    }

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

    #[test]
    fn a_name_nobody_defined_is_refused_with_the_list() {
        let e = resolve("{cost.today}", &ctx()).expect_err("not a field");
        let says = e[0].to_string();
        assert!(says.contains("not something a prompt can say"), "{says}");
        for f in [Field::Project, Field::GateFailures] {
            assert!(says.contains(f.name()), "the list omits {}", f.name());
        }
    }

    #[test]
    fn a_value_carrying_the_syntax_is_not_substituted_again() {
        let c = Context::default().with(Field::Project, Value::new("weird-{branch}-name", at()));
        let out = resolve("{project}", &c).expect("resolves");
        assert_eq!(
            out, "weird-{branch}-name",
            "a resolved value was scanned a second time"
        );
    }

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

    #[test]
    fn prose_containing_braces_is_not_a_template() {
        for t in ["a {1} b", r#"{"key": 1}"#, "unclosed { here", "{}", "{A}"] {
            let out = resolve(t, &ctx()).unwrap_or_else(|e| panic!("{t:?} refused: {e:?}"));
            assert_eq!(out, t, "{t:?} was treated as a placeholder");
        }
    }

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

    /// Production code with comments stripped, so a guard does not match the
    /// comment explaining why a word is forbidden.
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

    #[test]
    fn the_syntax_is_a_name_and_nothing_else() {
        let c = Context::default().with(
            Field::Project,
            Value::new("p", "2026-09-23T10:00:00Z".parse().unwrap()),
        );
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

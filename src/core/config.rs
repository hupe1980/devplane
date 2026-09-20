//! `devplane.toml` — the project's own definition of done.
//!
//! It lives in the repository and is committed, which is the whole point: the
//! commands that decide whether work is finished are the project's, reviewed
//! like anything else, and never something an agent wrote for itself.
//!
//! Every field has a default, so a repository with no file at all still works:
//! the gates are empty, no rule auto-decides anything, and Devplane behaves
//! exactly as it does today. Configuration buys automation, it is not the price
//! of entry.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// The file Devplane looks for at a repository root.
pub const CONFIG_FILE: &str = "devplane.toml";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub project: Project,
    pub workspace: Workspace,
    pub gates: Gates,
    pub policy: PolicySection,
    pub budget: Budget,
    pub transcripts: Transcripts,
    pub github: GitHub,
    pub spec: SpecSection,
    /// What happens to a question nobody answers. Nothing, unless this says so.
    pub questions: Questions,
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
    /// The agent `devplane work start` uses when none is named.
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

/// A declared chain of agent runs.
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
/// `{ human = "merge" }` and `{ role = "review", … }` read better in TOML than
/// a `type` discriminator would, so the shape decides which it is.
///
/// Deserialised by hand rather than with `#[serde(untagged)]`. Untagged reports
/// every mistake as *"data did not match any variant of untagged enum Step"* —
/// a step with a typo, a missing `prompt` and a step that is simply not a step
/// all produce that one sentence, and a person reading it learns nothing about
/// which of forty lines is wrong.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Step {
    /// Suspends the pipeline until a person releases it.
    Human(HumanStep),
    Role(RoleStep),
}

impl Step {
    /// What this step is called — the role, or what the person is asked for.
    pub fn name(&self) -> &str {
        match self {
            Step::Human(h) => &h.human,
            Step::Role(r) => &r.role,
        }
    }
}

/// The shape a step is read through, so every mistake gets its own sentence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StepFields {
    #[serde(default)]
    human: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    gate: Option<String>,
    #[serde(default)]
    findings: Option<Findings>,
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let f = StepFields::deserialize(d)?;
        match (f.human, f.role) {
            (Some(_), Some(role)) => Err(D::Error::custom(format!(
                "step `{role}` sets both `human` and `role`; a step is one or the other"
            ))),
            (Some(human), None) => {
                if f.prompt.is_some() || f.gate.is_some() || f.findings.is_some() {
                    return Err(D::Error::custom(format!(
                        "human step `{human}` also sets `prompt`, `gate` or \
                         `findings`; a person is asked, not prompted"
                    )));
                }
                Ok(Step::Human(HumanStep { human }))
            }
            (None, Some(role)) => Ok(Step::Role(RoleStep {
                prompt: f
                    .prompt
                    .ok_or_else(|| D::Error::custom(format!("step `{role}` has no `prompt`")))?,
                role,
                agent: f.agent,
                gate: f.gate,
                findings: f.findings,
            })),
            (None, None) => Err(D::Error::custom(
                "a pipeline step needs either `role = \"…\"` (an agent) or \
                 `human = \"…\"` (a person)",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HumanStep {
    /// What the person is being asked to do: `merge`, `spec_review`, …
    pub human: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoleStep {
    /// What this step is for: `implement`, `review`, `verify`. Also the name
    /// `back_to` refers to.
    pub role: String,
    /// The agent that does it. `any` — and the default — take the project's
    /// `default_agent`.
    ///
    /// Naming a *different* vendor for review is worth doing when the reviewer
    /// is at least as strong as the implementer, and actively harmful when it is
    /// not: a controlled study of 116 tasks found Claude reviewing Codex drafts
    /// raised the pass rate from 71.6 % to 89.7 %, while Codex reviewing Claude
    /// drafts *lowered* it from 91.4 % to 82.8 % (arXiv:2607.21656). Which is
    /// why `findings` requires a gate: see [`ProjectConfig::validate`].
    #[serde(default)]
    pub agent: Option<String>,
    /// A prompt template in `.devplane/prompts/<name>.md`, or the text itself
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
    /// How many times. Exhaustion asks a human; it never loops forever.
    #[serde(default = "one")]
    pub max: u32,
    /// Where the reviewer writes them, relative to the worktree.
    #[serde(default = "findings_file")]
    pub file: String,
    /// Words that make a finding worth returning the work for.
    ///
    /// Empty — the default — means every finding counts, which is what a
    /// reviewer writing prose produces. It exists for the *graded* reporters:
    /// every spec-driven tool in the field ships a consistency checker that
    /// grades what it finds and then hands the decision back, so a step whose
    /// prompt is one of those needs a line saying which grades stop the work.
    ///
    /// **The words are the project's, never Devplane's.** `CRITICAL` and
    /// `HIGH` are Spec Kit's vocabulary; another tool grades differently and a
    /// third will rename its levels next month. Holding a table of somebody
    /// else's nouns is the mistake this project refuses everywhere else, so the
    /// repository writes the words it means and the matching is
    /// case-insensitive and per line.
    ///
    /// A findings file with no matching line is *nothing found*: the step
    /// passes, exactly as an empty file does.
    #[serde(default)]
    pub only: Vec<String>,
}

fn one() -> u32 {
    1
}
fn findings_file() -> String {
    ".devplane/findings.md".into()
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

/// Whether to keep what driven agents say in this repository.
///
/// On by default, and the reasoning is worth stating because the project is
/// otherwise careful about what it stores. A run Devplane drives has no window
/// of its own: without its transcript the board can say a tool ran and not one
/// word about why, which makes `dispatch` a black box. The text is already in
/// the process — the protocol streams it to us — so discarding it was never a
/// privacy measure, it was an unbuilt feature.
///
/// It is still a repository's decision, because an agent's prose can quote
/// something it read, and `[workspace] include` deliberately copies `.env` into
/// the worktree it works in. Off means nothing is written down and the
/// transcript views say so; it does not stop the agent reading anything.
///
/// Sessions Devplane merely *watches* are unaffected either way: the
/// documented channels carry no prose, which is why `focus` is the honest
/// action for them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Transcripts {
    pub keep: bool,
}

impl Default for Transcripts {
    fn default() -> Self {
        Self { keep: true }
    }
}

/// What a piece of work may cost before somebody is asked about it.
///
/// The loops are already bounded by *count* — `max_feedback_rounds` and a
/// findings `max` — and that stops an agent and a test suite arguing for ever.
/// It does not stop one turn going a long way on its own, which is the other
/// way an afternoon becomes expensive.
///
/// **A ceiling only bites when the agent reports what it spent.** ACP makes the
/// cost field optional, so an agent that never sends one is never stopped by
/// this, and Devplane says so rather than implying a guard it does not have:
/// `devplane check` warns when a budget is set, and `devplane work show`
/// prints the cost so far or "not reported".
/// What happens to a question nobody answers.
///
/// **The default is that nothing happens to it, and that is the whole
/// section.** A product whose argument is that vendors end your questions on
/// clocks you never set does not get to ship a clock you never set — so the
/// only way an ask here ever ends on a timer is a project writing one down,
/// and the row that ends it then names the duration *and this file*.
///
/// It is per project rather than global for the same reason the fan-out dial
/// is: a setting somebody chose once and forgot is a setting that decides
/// things nobody is thinking about. A repository that runs unattended
/// overnight can bound its own waits; the one you are sitting in front of does
/// not have to.
///
/// **Devplane's own ten-minute refusal is gone.** Until 2026-09-20 a permission
/// request nobody answered was denied after six hundred seconds by a constant
/// in this crate, with no surface saying so — which is the trade this product
/// indicts four vendors for, shipped here, and aggravated by the fact that the
/// vendor it mirrors refuses to apply its own question timer to permission
/// prompts at all.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Questions {
    /// `never` (the default), or a duration like `30m`, `4h`, `90s`.
    ///
    /// An unparseable value is a configuration error rather than a silent
    /// fallback: the difference between `4h` and a typo is the difference
    /// between a bounded wait and an unbounded one, and guessing which the
    /// author meant is how a supervision tool ends a question nobody meant it
    /// to end.
    pub deadline: Option<String>,
}

impl Questions {
    /// The deadline this project sets, and nothing where it sets none.
    ///
    /// Returns `None` for an unparseable value so the caller can report it as a
    /// problem; `Deadline::Never` is what an absent setting means and the two
    /// are never conflated.
    pub fn deadline(&self) -> Option<crate::core::ask::Deadline> {
        match self.deadline.as_deref() {
            None => Some(crate::core::ask::Deadline::Never),
            Some(s) => crate::core::ask::parse_deadline(s),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budget {
    /// Applies to any kind that names no figure of its own.
    pub default_usd: Option<f64>,
    pub quick_usd: Option<f64>,
    pub chore_usd: Option<f64>,
    pub bug_usd: Option<f64>,
    pub feature_usd: Option<f64>,
    /// How many agent turns one piece of work may take.
    ///
    /// **The bound that always fires.** A ceiling in dollars only bites when
    /// the agent reports what it spent, which is optional twice over: the
    /// protocol marks the cost field optional, and the OpenTelemetry GenAI
    /// conventions have no notion of money at all. Turns are counted here, from
    /// events Devplane saw itself, so they bind every agent.
    pub max_turns: Option<u32>,
    /// How long one piece of work may run, in the same duration spelling the
    /// rest of the file uses (`45m`, `2h`).
    ///
    /// The other always-observable axis, and the one that catches the failure
    /// turns do not: a single turn that goes a very long way. Measured from
    /// when the work started, not from the last turn — a chain that has been
    /// going for three hours is a chain somebody should look at whether or not
    /// it is making progress.
    #[serde(default, with = "humantime_opt", rename = "max_runtime")]
    pub max_runtime: Option<Duration>,
}

impl Budget {
    /// Whether this project bounds a piece of work by something it can always
    /// observe, rather than only by what an agent chooses to report.
    pub fn has_observable_bound(&self) -> bool {
        self.max_turns.is_some() || self.max_runtime.is_some()
    }

    /// The ceiling for a kind of work, if the project set one.
    pub fn for_kind(&self, kind: &str) -> Option<f64> {
        let named = match kind {
            "quick" => self.quick_usd,
            "chore" => self.chore_usd,
            "bug" => self.bug_usd,
            "feature" => self.feature_usd,
            _ => None,
        };
        named.or(self.default_usd).filter(|c| *c > 0.0)
    }

    pub fn is_set(&self) -> bool {
        [
            self.default_usd,
            self.quick_usd,
            self.chore_usd,
            self.bug_usd,
            self.feature_usd,
        ]
        .iter()
        .any(Option::is_some)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitHub {
    /// Open a pull request when the gates pass. Off by default: pushing a
    /// branch is the first thing Devplane does that other people can see.
    pub pull_request: bool,
    /// Open it as a draft. A pull request that looks finished summons
    /// reviewers, and work a machine just finished has not been read by anyone.
    pub draft: bool,
    /// Issues carrying this label are offered as work.
    pub ready_label: Option<String>,
    // `squash` lived here for the life of the project: parsed, defaulted to
    // true, printed in the configuration reference and on the documentation
    // site, and read by nothing. It belonged to merge-when-green, which is not
    // built. This file's own rule is that every key in it is read by the code
    // and that a designed-but-unbuilt key never appears in a reference, because
    // `deny_unknown_fields` means one such key fails the *whole* file and takes
    // the repository's permission rules down with it. Unbuilt work lives in the
    // roadmap; it does not live here as a field somebody can set and watch do
    // nothing.
}

impl Default for GitHub {
    fn default() -> Self {
        Self {
            pull_request: false,
            draft: true,
            ready_label: None,
        }
    }
}

/// How to read the specification a piece of work names.
///
/// **One key, and it holds the project's words rather than this tool's.** The
/// spec-driven frameworks disagree on almost everything — the folder layout,
/// the requirement identifiers and the section names all differ, and all of
/// them are still moving. So nothing here recognises a section: the outline is
/// the Markdown headings, the progress is the `- [ ]` boxes they do share, and
/// the rest is the repository's to name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpecSection {
    /// Words that mark a question the specification has not answered yet.
    ///
    /// `NEEDS CLARIFICATION` is Spec Kit's spelling, `TBD` is everybody's, and
    /// the next tool will have a third — which is why the list is empty by
    /// default and the repository writes what it means. Matched
    /// case-insensitively, per line, exactly as a reviewer's findings are.
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PolicySection {
    /// Rules that refuse a permission request.
    pub never_auto: Vec<String>,
    /// Rules that always put the call in front of a person.
    ///
    /// Claude Code's third list, so an `ask` rule moved over from
    /// `settings.json` has somewhere to go. Evaluated after `never_auto`.
    pub always_ask: Vec<String>,
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

/// The machine-wide rules, from `~/.devplane/policy.toml`.
///
/// The same `[policy]` shape a project writes, so a rule can be moved between
/// the two files by cutting and pasting it — and read by the same TOML parser,
/// so a comment or a multi-line array means what it looks like.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalConfig {
    pub policy: PolicySection,
}

impl GlobalConfig {
    /// Reads `<home>/policy.toml`. A missing file means no rule decides
    /// anything on the user's behalf, which is the right default; a malformed
    /// one is an error, because a typo in a deny rule must never read as
    /// permission.
    pub fn load(home: &Path) -> Result<Self, ConfigError> {
        let path = home.join("policy.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(ConfigError::Io(path.display().to_string(), e.to_string())),
        };
        toml::from_str(&text)
            .map_err(|e| ConfigError::Parse(path.display().to_string(), e.to_string()))
    }

    pub fn policy(&self) -> crate::core::Policy {
        crate::core::Policy::rules(&self.policy.never_auto, &self.policy.always_ask)
    }

    /// Rules in the machine-wide file that cannot do what they say.
    pub fn validate(&self) -> Vec<Problem> {
        self.policy()
            .problems()
            .into_iter()
            .map(|(fatal, what)| match fatal {
                true => Problem::error("[policy]".into(), what),
                false => Problem::warning("[policy]".into(), what),
            })
            .collect()
    }
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
    pub fn policy(&self) -> crate::core::Policy {
        crate::core::Policy::rules(&self.policy.never_auto, &self.policy.always_ask)
    }

    pub fn has_gates(&self) -> bool {
        !self.gates.check.is_empty()
    }

    /// Everything in this file that parses but cannot do what it says.
    ///
    /// A configuration file gets three chances to be wrong. It can fail to
    /// parse, which TOML reports. It can be malformed in a way the types accept
    /// — a `back_to` naming a step that does not exist. And it can be *dangerous
    /// in a way only the domain knows*, which is what most of this is.
    ///
    /// Checked when work starts and by `devplane check`, so the answer arrives
    /// before an agent has been paid to discover it.
    pub fn validate(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        let named: Vec<&str> = self.gates.named.keys().map(String::as_str).collect();

        // A deadline that will not parse is an **error**, not a default. It is
        // the one setting in this file that ends somebody's question on their
        // behalf, and a typo in it is the difference between a bounded wait and
        // an unbounded one. Devplane leaves the wait unbounded — the safe
        // direction — and says the file is wrong rather than guessing which
        // duration was meant.
        if self.questions.deadline().is_none() {
            out.push(Problem::error(
                "[questions] deadline".into(),
                format!(
                    "is {:?}, which is not a duration — write `never`, `90s`, `30m` or `4h`. \
                     Until it is fixed, a question waits for a person",
                    self.questions.deadline.as_deref().unwrap_or_default()
                ),
            ));
        }

        for (name, gate) in &self.gates.named {
            if gate.run.is_empty() {
                out.push(Problem::error(
                    format!("[gates.named.{name}]"),
                    "has no commands, and an empty gate is not a pass".into(),
                ));
            }
        }
        if self.gates.check.is_empty() && !self.pipelines.is_empty() {
            out.push(Problem::warning(
                "[gates] check".into(),
                "is empty, so `gate = \"check\"` in a pipeline cannot pass".into(),
            ));
        }

        for (pname, pipeline) in &self.pipelines {
            let at = |s: &str| format!("[pipelines.{pname}] {s}");
            if pipeline.steps.is_empty() {
                out.push(Problem::error(
                    at("steps"),
                    "is empty, so work of this kind would finish before it started".into(),
                ));
                continue;
            }

            // Steps are addressed by name — `back_to`, and the cursor a running
            // pipeline carries — so two of a name is an ambiguity the chain
            // resolves silently and wrongly.
            let mut seen: Vec<&str> = Vec::new();
            for step in &pipeline.steps {
                let name = step.name();
                if seen.contains(&name) {
                    out.push(Problem::error(
                        at(name),
                        "appears twice; steps are addressed by name".into(),
                    ));
                }
                seen.push(name);
            }

            let gate_of = |name: &str| -> Option<&str> {
                pipeline.steps.iter().find_map(|s| match s {
                    Step::Role(r) if r.role == name => r.gate.as_deref(),
                    _ => None,
                })
            };

            for (i, step) in pipeline.steps.iter().enumerate() {
                let Step::Role(role) = step else { continue };
                if let Some(gate) = &role.gate
                    && gate != "check"
                    && !named.contains(&gate.as_str())
                {
                    out.push(Problem::error(
                        at(&role.role),
                        format!("asks for gate `{gate}`, which no `[gates.named]` declares"),
                    ));
                }

                let Some(f) = &role.findings else { continue };
                let target = seen.iter().position(|s| *s == f.back_to);
                match target {
                    None => out.push(Problem::error(
                        at(&role.role),
                        format!(
                            "sends findings back to `{}`, which is not a step",
                            f.back_to
                        ),
                    )),
                    Some(t) if t >= i => out.push(Problem::error(
                        at(&role.role),
                        format!(
                            "sends findings back to `{}`, which is not earlier",
                            f.back_to
                        ),
                    )),
                    Some(_) => {}
                }
                if f.max == 0 {
                    out.push(Problem::warning(
                        at(&role.role),
                        "has `max = 0`, so its findings are never acted on".into(),
                    ));
                }

                // The rule the evidence asks for. A reviewer that can send work
                // back, with nothing re-checking the result, can only move the
                // code — and not always forwards: reviewing across vendors
                // raises the pass rate when the reviewer is the stronger model
                // and *lowers* it when it is not (arXiv:2607.21656, 91.4 % →
                // 82.8 % for the weaker direction). A gate on the step that
                // owns the code is what makes the loop safe in both directions.
                if gate_of(&f.back_to).is_none() {
                    out.push(Problem::error(
                        at(&role.role),
                        format!(
                            "sends findings back to `{}`, which declares no `gate`. \
                             A review loop with nothing re-checking the result can make \
                             the code worse as easily as better — give `{}` a gate",
                            f.back_to, f.back_to
                        ),
                    ));
                }
            }
        }

        // A permission rule that cannot work is the most dangerous thing a
        // configuration file can contain, because the failure is silent and, on
        // the deny side, silence reads as permission. Claude Code reports these
        // in its own startup dialog; there is no reason for the file that
        // mirrors its syntax to be quieter about them.
        for (problem, what) in self.policy().problems() {
            let where_ = "[policy]".to_string();
            out.push(match problem {
                true => Problem::error(where_, what),
                false => Problem::warning(where_, what),
            });
        }

        // The warning is about the *money* axis only, and only when it is the
        // whole bound. A project that also sets `max_turns` or `max_runtime`
        // has a guard that always fires, and telling it otherwise would be the
        // kind of warning people learn to ignore.
        if self.budget.is_set() && !self.budget.has_observable_bound() {
            out.push(Problem::warning(
                "[budget]".into(),
                "a `_usd` ceiling only stops work whose agent reports what it spent, which \
                 the protocol makes optional and the GenAI telemetry conventions cannot \
                 express at all. Add `max_turns` or `max_runtime`, which are counted here \
                 and bind every agent"
                    .into(),
            ));
        }

        for pattern in &self.workspace.include {
            let p = Path::new(pattern);
            if p.is_absolute() || p.components().any(|c| c == std::path::Component::ParentDir) {
                out.push(Problem::error(
                    "[workspace] include".into(),
                    format!("`{pattern}` leaves the repository and will not be copied"),
                ));
            }
        }
        out
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

/// A duration in the spelling the file uses, so what is shown back is what
/// somebody would type.
fn human(d: Duration) -> String {
    let s = d.as_secs();
    match (s / 3600, (s % 3600) / 60, s % 60) {
        (0, 0, s) => format!("{s}s"),
        (0, m, 0) => format!("{m}m"),
        (h, 0, 0) => format!("{h}h"),
        (0, m, s) => format!("{m}m{s}s"),
        (h, m, 0) => format!("{h}h{m}m"),
        (h, m, s) => format!("{h}h{m}m{s}s"),
    }
}

impl ProjectConfig {
    /// This file read back: what it will actually do, and everything in it that
    /// cannot do what it says.
    ///
    /// One derivation, for the same reason the inbox has one: `devplane check`
    /// and the board answering "what is configured here" from two different
    /// projections is how a terminal and a browser end up disagreeing about a
    /// repository's own rules.
    ///
    /// The rules are listed in **evaluation order** — deny, then ask, then
    /// allow — because that order is the thing most likely to surprise, and
    /// beside them the three findings a person cannot get by reading the file:
    /// a rule that covers nothing, a rule that grants more than it reads as
    /// granting, and a path denied for reading that is still writable.
    pub fn describe(&self) -> serde_json::Value {
        use serde_json::json;
        let policy = self.policy();
        let rules = |rs: &[crate::core::policy::Rule]| {
            rs.iter()
                .map(|r| json!({"rule": r.to_string(), "negated": r.is_negated()}))
                .collect::<Vec<_>>()
        };
        json!({
            "project": {
                "name": self.project.name,
                "base_branch": self.project.base_branch,
                "default_agent": self.project.default_agent,
            },
            "gates": {
                "check": self.gates.check,
                "timeout": human(self.gates.timeout),
                "on_fail": match self.gates.on_fail {
                    OnFail::Feedback => "feedback",
                    OnFail::Escalate => "escalate",
                    OnFail::Ignore => "ignore",
                },
                "max_feedback_rounds": self.gates.max_feedback_rounds,
                "named": self.gates.named.iter().map(|(name, g)| json!({
                    "name": name,
                    "run": g.run,
                    // A reproduction gate is the one whose *failure* is the
                    // pass, and a list of commands that does not say so reads
                    // as exactly backwards.
                    "expect": match g.expect { Expect::Fail => "fail", Expect::Pass => "pass" },
                    "timeout": g.timeout.map(human),
                })).collect::<Vec<_>>(),
            },
            "pipelines": self.pipelines.iter().map(|(name, p)| json!({
                "name": name,
                "steps": p.steps.iter().map(|s| match s {
                    Step::Human(h) => json!({"kind": "human", "name": h.human}),
                    Step::Role(r) => json!({
                        "kind": "role",
                        "name": r.role,
                        "agent": r.agent,
                        "gate": r.gate,
                        "back_to": r.findings.as_ref().map(|f| f.back_to.clone()),
                        "max": r.findings.as_ref().map(|f| f.max),
                    }),
                }).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "policy": {
                "deny": rules(policy.deny_rules()),
                "ask": rules(policy.ask_rules()),
                "max_parallel_runs": self.policy.max_parallel_runs,
                "stall_timeout": self.policy.stall_timeout.map(human),
                "unused": policy.redundancies(),
                "overbroad": crate::core::policy::overbroad(&[]).into_iter().map(|o| json!({
                    "rule": o.rule, "why": o.why, "suggestion": o.suggestion,
                })).collect::<Vec<_>>(),
                "half_protected": policy.half_protected_paths(),
            },
            "budget": {
                "default_usd": self.budget.default_usd,
                "quick_usd": self.budget.quick_usd,
                "chore_usd": self.budget.chore_usd,
                "bug_usd": self.budget.bug_usd,
                "feature_usd": self.budget.feature_usd,
                "max_turns": self.budget.max_turns,
                "max_runtime": self.budget.max_runtime.map(human),
                // A ceiling in dollars binds only an agent that reports what it
                // spent, so whether this project bounds work by something
                // always observable is a fact about the file worth stating.
                "has_observable_bound": self.budget.has_observable_bound(),
            },
            "github": {
                "pull_request": self.github.pull_request,
                "draft": self.github.draft,
                "ready_label": self.github.ready_label,
            },
            "workspace": {"setup": self.workspace.setup, "include": self.workspace.include},
            "spec": {"open_questions": self.spec.open_questions},
            "transcripts": {"keep": self.transcripts.keep},
            "problems": self.validate(),
        })
    }
}

/// Something in a configuration file that parses but cannot work.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    /// Where it is, in the file's own words: `[pipelines.feature] review`.
    #[serde(rename = "where")]
    pub where_: String,
    pub what: String,
    /// An error makes the file unusable for the thing it configures; a warning
    /// is something that will surprise somebody later.
    pub fatal: bool,
}

impl Problem {
    fn error(where_: String, what: String) -> Self {
        Self {
            where_,
            what,
            fatal: true,
        }
    }
    fn warning(where_: String, what: String) -> Self {
        Self {
            where_,
            what,
            fatal: false,
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.where_, self.what)
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
            let unit = match c {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86_400,
                _ => return None,
            };
            // Checked, because this parses a file from a repository. `999999999d`
            // is not a timeout anybody means, and overflowing on it panics a
            // debug build and silently wraps to a *short* one in release —
            // which would turn a gate timeout into an instant failure.
            total = n.checked_mul(unit).and_then(|v| total.checked_add(v))?;
        }
        // A bare number is seconds, which is what someone writing `30` means.
        if !digits.is_empty() {
            total = total.checked_add(digits.parse::<u64>().ok()?)?;
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
        assert_eq!(f.file, ".devplane/findings.md");

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

    fn parse(toml_text: &str) -> ProjectConfig {
        toml::from_str(toml_text).expect("parses")
    }

    #[test]
    fn a_step_that_is_wrong_says_which_step_and_why() {
        // `#[serde(untagged)]` reported every one of these as "data did not
        // match any variant of untagged enum Step", which tells a reader
        // nothing about which of forty lines to look at.
        let cases = [
            (
                r#"[pipelines.f]
steps = [{ role = "review" }]"#,
                "`review` has no `prompt`",
            ),
            (
                r#"[pipelines.f]
steps = [{ agent = "claude", prompt = "go" }]"#,
                "needs either",
            ),
            (
                r#"[pipelines.f]
steps = [{ role = "review", human = "merge", prompt = "x" }]"#,
                "both",
            ),
            (
                r#"[pipelines.f]
steps = [{ human = "merge", prompt = "x" }]"#,
                "a person is asked",
            ),
            (
                r#"[pipelines.f]
steps = [{ role = "review", prompt = "x", gates = "check" }]"#,
                "gates",
            ),
        ];
        for (text, expected) in cases {
            let err = toml::from_str::<ProjectConfig>(text)
                .expect_err(text)
                .to_string();
            assert!(err.contains(expected), "{text}\n produced: {err}");
        }
    }

    #[test]
    fn a_review_loop_with_nothing_re_checking_it_is_refused() {
        // The evidence: reviewing across vendors raises the pass rate when the
        // reviewer is the stronger model and lowers it when it is not
        // (arXiv:2607.21656). A loop that can send work back with no gate on
        // the step that owns the code can therefore make it worse, quietly.
        let c = parse(
            r#"
[pipelines.feature]
steps = [
  { role = "implement", prompt = "implement" },
  { role = "review", prompt = "review", findings = { back_to = "implement" } },
]
"#,
        );
        let problems = c.validate();
        assert!(
            problems
                .iter()
                .any(|p| p.fatal && p.what.contains("no `gate`")),
            "{problems:?}"
        );

        // With a gate on the implementing step, the loop is checked and fine.
        let ok = parse(
            r#"
[gates]
check = ["cargo test"]
[pipelines.feature]
steps = [
  { role = "implement", prompt = "implement", gate = "check" },
  { role = "review", prompt = "review", findings = { back_to = "implement" } },
  { human = "merge" },
]
"#,
        );
        assert!(ok.validate().is_empty(), "{:?}", ok.validate());
    }

    #[test]
    fn the_mistakes_a_pipeline_can_make_are_named() {
        let c = parse(
            r#"
[gates]
check = ["cargo test"]
[gates.named.empty]
run = []
[pipelines.feature]
steps = [
  { role = "implement", prompt = "x", gate = "nope" },
  { role = "implement", prompt = "y", gate = "check" },
  { role = "review", prompt = "z", findings = { back_to = "verify", max = 0 } },
]
[workspace]
include = ["../../.ssh/id_rsa"]
"#,
        );
        let found: Vec<String> = c.validate().iter().map(|p| p.to_string()).collect();
        let all = found.join("\n");
        for expected in [
            "no `[gates.named]` declares", // gate = "nope"
            "appears twice",               // two `implement` steps
            "is not a step",               // back_to = "verify"
            "never acted on",              // max = 0
            "empty",                       // [gates.named.empty]
            "leaves the repository",       // include
        ] {
            assert!(all.contains(expected), "missing `{expected}` in:\n{all}");
        }
    }

    #[test]
    fn a_findings_loop_may_only_point_backwards() {
        let c = parse(
            r#"
[gates]
check = ["cargo test"]
[pipelines.feature]
steps = [
  { role = "review", prompt = "z", findings = { back_to = "fix" } },
  { role = "fix", prompt = "x", gate = "check" },
]
"#,
        );
        assert!(
            c.validate().iter().any(|p| p.what.contains("not earlier")),
            "a loop forwards is a chain that never ends"
        );
    }

    #[test]
    fn a_permission_rule_that_cannot_work_is_refused_before_an_agent_starts() {
        // The most dangerous thing this file can contain, because the failure
        // is silent and on the deny side silence reads as permission. Claude
        // Code reports these in its own startup dialog; there is no reason for
        // the file that mirrors its syntax to be quieter.
        let c = parse(
            r#"
[policy]
never_auto = ["Write(src/**)", "Bash(command:rm *)", "mcp__github(create_issue)"]
"#,
        );
        let said: Vec<String> = c.validate().iter().map(|p| p.to_string()).collect();
        let all = said.join("\n");
        for expected in [
            "never consulted", // Write(path) is accepted and not checked
            "content field",   // Bash(command:…) is ignored by Claude Code
            "skips any",       // an mcp__ rule with brackets
        ] {
            assert!(all.contains(expected), "missing `{expected}` in:\n{all}");
        }
        assert!(
            c.validate().iter().all(|p| p.fatal),
            "a rule that does nothing is an error, not a note"
        );
    }

    #[test]
    fn the_rules_from_claude_codes_own_documentation_are_accepted() {
        // The claim this file makes is that a rule moves between settings.json
        // and here by cutting and pasting it. These are the spellings that
        // documentation uses.
        let c = parse(
            r#"
[policy]
never_auto = ["Bash(git push *)", "Read(./.env)", "Read(secrets/**)", "mcp__*"]
"#,
        );
        assert_eq!(c.validate(), vec![]);
    }

    #[test]
    fn a_budget_is_per_kind_with_a_fallback() {
        let c = parse("[budget]\ndefault_usd = 5\nfeature_usd = 25\n");
        assert_eq!(c.budget.for_kind("feature"), Some(25.0));
        assert_eq!(c.budget.for_kind("bug"), Some(5.0), "falls back");
        assert_eq!(ProjectConfig::default().budget.for_kind("quick"), None);
        // Zero is not a ceiling of nothing; it is somebody who meant "off".
        assert_eq!(
            parse("[budget]\ndefault_usd = 0\n")
                .budget
                .for_kind("quick"),
            None
        );
        // And setting one says plainly what it can and cannot do.
        assert!(
            c.validate()
                .iter()
                .any(|p| !p.fatal && p.what.contains("reports what it spent")),
            "a guard that may not fire must say so"
        );
    }

    #[test]
    fn a_bound_that_always_fires_silences_the_warning_about_one_that_may_not() {
        // The money axis cannot bind an agent that reports no cost, and the
        // GenAI telemetry conventions have nowhere to report one. Turns and
        // elapsed time are counted here, so a project that sets either has a
        // guard that fires whatever the agent says.
        let money_only = parse("[budget]\ndefault_usd = 10\n");
        assert!(
            money_only
                .validate()
                .iter()
                .any(|p| !p.fatal && p.what.contains("reports what it spent")),
            "a guard that may not fire must say so"
        );
        let bounded = parse("[budget]\ndefault_usd = 10\nmax_turns = 60\n");
        assert!(
            !bounded
                .validate()
                .iter()
                .any(|p| p.what.contains("reports what it spent")),
            "a project with an observable bound is not warned about the other one"
        );
        assert!(bounded.budget.has_observable_bound());
        assert_eq!(bounded.budget.max_turns, Some(60));
        assert_eq!(
            parse("[budget]\nmax_runtime = \"45m\"\n")
                .budget
                .max_runtime,
            Some(Duration::from_secs(45 * 60))
        );
    }

    #[test]
    fn a_correct_file_has_nothing_to_say() {
        let c = parse(
            r#"
[gates]
check = ["cargo test"]
[gates.named.repro]
run = ["cargo test --test repro"]
expect = "fail"
[pipelines.bug]
steps = [
  { role = "reproduce", prompt = "repro", gate = "repro" },
  { role = "fix", prompt = "fix", gate = "check" },
  { role = "review", agent = "any", prompt = "review", findings = { back_to = "fix", max = 1 } },
  { human = "merge" },
]
"#,
        );
        assert_eq!(c.validate(), vec![]);
    }

    #[test]
    fn a_repository_with_no_file_still_works() {
        let c = ProjectConfig::load(Path::new("/definitely/not/here")).unwrap();
        assert!(!c.has_gates());
        assert_eq!(c.gates.max_feedback_rounds, 2);
        // And no rule decides anything on the user's behalf.
        assert!(c.policy.never_auto.is_empty());
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
never_auto = ["Bash(git push *)"]
max_parallel_runs = 2

[github]
pull_request = true
draft = true
ready_label = "devplane:ready"
"#,
        )
        .unwrap();
        let c = ProjectConfig::load(&dir).unwrap();
        assert_eq!(c.project.name.as_deref(), Some("saas"));
        assert_eq!(c.gates.check.len(), 2);
        assert_eq!(c.gates.timeout, Duration::from_secs(600));
        assert_eq!(c.gates.max_feedback_rounds, 3);
        assert!(matches!(
            c.policy().restrictive(
                &crate::core::policy::Context::at(&dir),
                "Bash",
                &serde_json::json!({"command": "git push origin"})
            ),
            crate::core::Verdict::Deny { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn github_is_off_until_it_is_asked_for() {
        // Pushing a branch is the first thing Devplane does that other people
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
    fn the_machine_wide_rules_parse_like_a_project_s() {
        let dir = tempdir("global");
        std::fs::write(
            dir.join("policy.toml"),
            "[policy]\nnever_auto = [\n  \"Bash(rm -rf *)\",  # a comment\n  \"Read(.env)\",\n]\n",
        )
        .unwrap();
        let g = GlobalConfig::load(&dir).unwrap();
        assert_eq!(g.policy.never_auto, ["Bash(rm -rf *)", "Read(.env)"]);
        assert!(matches!(
            g.policy().restrictive(
                &crate::core::policy::Context::at(&dir),
                "Bash",
                &serde_json::json!({"command": "rm -rf /"})
            ),
            crate::core::Verdict::Deny { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_global_policy_decides_nothing() {
        let g = GlobalConfig::load(Path::new("/definitely/not/here")).unwrap();
        assert!(g.policy.never_auto.is_empty());
        assert!(g.policy.always_ask.is_empty());
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
        // A figure no one means, out of a file that came with a repository.
        // Unchecked, this panicked a debug build and wrapped in release — and a
        // wrapped gate timeout is a gate that fails the instant it starts.
        assert_eq!(parse("999999999999999d"), None);
        assert_eq!(parse("99999999999999999999"), None);
    }

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-cfg-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

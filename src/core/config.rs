//! `devplane.toml` — the project's own definition of done, committed and
//! reviewed like any other code, never written by an agent for itself. Every
//! field has a default, so a repository with no file still works: no gates, and
//! no rule decides anything on anyone's behalf.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// The file Devplane looks for at a repository root.
pub const CONFIG_FILE: &str = "devplane.toml";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub project: Project,
    pub agent: AgentSection,
    pub workspace: Workspace,
    pub gates: Gates,
    pub policy: PolicySection,
    pub budget: Budget,
    pub github: GitHub,
    pub spec: SpecSection,
    pub questions: Questions,
    pub review: Review,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Project {
    pub name: Option<String>,
    /// What worktrees branch from. Discovered from the repository when unset.
    pub base_branch: Option<String>,
}

/// `[agent]` — what belongs to the agent a change runs: which one, how many at
/// once, when a silent one counts as stalled, and whether what it says is kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentSection {
    /// The agent `devplane change start` uses when none is named.
    pub default: Option<String>,
    /// How many agents this project may run at once.
    pub max_parallel: Option<usize>,
    /// How long a working run may produce nothing before it is called stalled.
    /// Per project because suites differ; unset uses the global setting.
    #[serde(default, with = "humantime_opt")]
    pub stall_timeout: Option<Duration>,
    /// Whether to keep what driven agents say. On by default: a driven run has
    /// no window of its own, so without its transcript nobody can see why it
    /// acted. Still the repository's choice, because an agent's prose can
    /// quote what it read (`.env` included). Watched sessions carry no prose
    /// either way.
    pub keep_transcripts: bool,
}

impl Default for AgentSection {
    fn default() -> Self {
        Self {
            default: None,
            max_parallel: None,
            stall_timeout: None,
            keep_transcripts: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Workspace {
    /// Run once in a new worktree, before the agent starts.
    pub setup: Option<String>,
    /// Build caches shared between this project's changes, by ecosystem name
    /// from `caches::ECOSYSTEMS`; an unknown name is refused. Files to copy
    /// into a fresh tree belong in `.worktreeinclude`, not here.
    pub share: Vec<String>,
}

/// `[gates]`: the definition of done, and beside it every gate a person runs
/// by name, each its own table (`[gates.slow]`). Read by hand rather than
/// derived, because the named gates share the table with the keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, try_from = "toml::Table")]
pub struct Gates {
    /// The Definition of Done: commands that must all succeed.
    pub check: Vec<String>,
    /// How long the whole gate may take.
    #[serde(with = "humantime")]
    pub timeout: Duration,
    /// How many times failures are handed back to the agent before it stops
    /// and a person is asked; bounded because each round costs money. Zero
    /// stops at the first failure.
    pub max_feedback_rounds: u32,
    /// Gates run only by name (`devplane gate --name <name>`). They never
    /// make a change verified: that is `check` alone.
    #[serde(flatten)]
    pub named: BTreeMap<String, NamedGate>,
    /// JUnit XML files `check`'s runners write, relative to the checkout:
    /// literal paths, read to name tests. Never an input to *verified*.
    pub junit: Vec<String>,
    /// When the base's tests run on a change and its new tests on the base.
    pub base_checks: BaseChecksWhen,
    /// Whether this repository's gates run one at a time on this machine:
    /// for checks that share something two changes cannot both hold, such as
    /// one test database. Off, two changes' gates run at once.
    pub serial: bool,
}

/// When base checks run: the cost is the person's choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseChecksWhen {
    /// When a person offers or asks (`change verify --base-checks`, *Run base
    /// checks*), and in CI.
    #[default]
    Offer,
    /// Also after every passing `check`, in the background.
    Auto,
    /// Never; every surface and the certificate say so.
    Off,
}

impl BaseChecksWhen {
    pub fn as_str(self) -> &'static str {
        match self {
            BaseChecksWhen::Offer => "offer",
            BaseChecksWhen::Auto => "auto",
            BaseChecksWhen::Off => "off",
        }
    }
}

/// The keys `[gates]` itself takes; any other key names a gate.
const GATES_KEYS: &[&str] = &[
    "check",
    "timeout",
    "max_feedback_rounds",
    "junit",
    "base_checks",
    "serial",
];

impl TryFrom<toml::Table> for Gates {
    type Error = String;

    fn try_from(mut t: toml::Table) -> Result<Self, String> {
        let mut g = Gates::default();
        if let Some(v) = t.remove("check") {
            if v.is_table() {
                return Err(
                    "`[gates.check]` cannot be a gate run by name: `check` is the \
                            definition of done, `check = [...]` under `[gates]`. Give the \
                            gate another name"
                        .into(),
                );
            }
            g.check = v
                .try_into()
                .map_err(|e| format!("`[gates] check` is a list of commands: {e}"))?;
        }
        if let Some(v) = t.remove("timeout") {
            let raw = v
                .as_str()
                .ok_or("`[gates] timeout` is a duration like \"10m\"")?;
            g.timeout = humantime::parse(raw).ok_or(format!("`{raw}` is not a duration"))?;
        }
        if let Some(v) = t.remove("max_feedback_rounds") {
            g.max_feedback_rounds = v
                .as_integer()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("`[gates] max_feedback_rounds` is a whole number, zero or more")?;
        }
        if let Some(v) = t.remove("junit") {
            g.junit = v
                .try_into()
                .map_err(|e| format!("`[gates] junit` is a list of paths: {e}"))?;
        }
        if let Some(v) = t.remove("base_checks") {
            g.base_checks = v.clone().try_into().map_err(|_| {
                format!("`[gates] base_checks` is \"offer\", \"auto\" or \"off\", not {v}")
            })?;
        }
        if let Some(v) = t.remove("serial") {
            g.serial = v.as_bool().ok_or("`[gates] serial` is true or false")?;
        }
        for (name, v) in t {
            if let Some(why) = removed_gates_key(&name) {
                return Err(why);
            }
            let toml::Value::Table(_) = v else {
                return Err(format!(
                    "`[gates] {name}` is not a key; the keys are {}, and a gate run by name \
                     is a table of its own: `[gates.{name}]` with `run = [...]`",
                    GATES_KEYS.join(", ")
                ));
            };
            let gate: NamedGate = v.try_into().map_err(|e| format!("`[gates.{name}]`: {e}"))?;
            g.named.insert(name, gate);
        }
        Ok(g)
    }
}

/// A key `[gates]` took once, and what to write instead.
fn removed_gates_key(key: &str) -> Option<String> {
    match key {
        "on_fail" => Some(
            "`[gates] on_fail` is gone: failures go back to the agent up to \
             `max_feedback_rounds` times, then the change stops for a person. Write \
             `max_feedback_rounds = 0` to stop at the first failure"
                .into(),
        ),
        "named" => Some(
            "`[gates.named.<name>]` is `[gates.<name>]`: a gate run by name is a table \
             directly under `[gates]`"
                .into(),
        ),
        _ => None,
    }
}

/// A suite a person can run by name; never decides whether a change is done.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NamedGate {
    pub run: Vec<String>,
    /// Overrides the project's gate timeout.
    #[serde(default, with = "humantime_opt")]
    pub timeout: Option<Duration>,
}

impl Default for Gates {
    fn default() -> Self {
        Self {
            check: Vec::new(),
            timeout: Duration::from_secs(600),
            max_feedback_rounds: 2,
            named: BTreeMap::new(),
            junit: Vec::new(),
            base_checks: BaseChecksWhen::Offer,
            serial: false,
        }
    }
}

/// What happens to a question nobody answers: by default, nothing. An ask only
/// ends on a timer the project wrote down here, and the row that ends it names
/// the duration and this file. Per project, so an unattended repository can
/// bound its waits without bounding everyone's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Questions {
    /// `never` (the default), or a duration like `30m`, `4h`, `90s`. An
    /// unparseable value is a configuration error, never a guessed duration.
    pub deadline: Option<String>,
    /// How long a permission on a watched session that matched an `ask` rule
    /// waits for an answer from the inbox, board or phone before the vendor's
    /// dialog appears. Absent means no hold. `true` means [`Hold::DEFAULT`];
    /// a duration names its own, up to [`Hold::CEILING`]. An unparseable or
    /// over-long value is a configuration error.
    pub hold: Option<toml::Value>,
}

/// A bounded wait on one permission, so somebody can answer it from wherever
/// they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hold(pub Duration);

impl Hold {
    /// What `hold = true` means: long enough to unlock a phone after a
    /// notification, short enough not to freeze an unattended run.
    pub const DEFAULT: Duration = Duration::from_secs(30);

    /// The most a project may ask for. Under the vendor's 600 s hook timeout,
    /// so a hold lapses by Devplane standing down, not by the vendor killing
    /// the hook.
    pub const CEILING: Duration = Duration::from_secs(120);
}

/// [`Hold::CEILING`] in milliseconds.
pub const HOLD_CEILING_MS: u64 = Hold::CEILING.as_millis() as u64;

impl Questions {
    /// The declared hold; `Ok(None)` is no hold. `Err` is the sentence
    /// `devplane config check` prints.
    pub fn hold(&self) -> Result<Option<Hold>, String> {
        let Some(v) = self.hold.as_ref() else {
            return Ok(None);
        };
        match v {
            toml::Value::Boolean(false) => Ok(None),
            toml::Value::Boolean(true) => Ok(Some(Hold(Hold::DEFAULT))),
            toml::Value::String(s) => {
                let d = humantime::parse(s).ok_or_else(|| {
                    format!("`questions.hold = \"{s}\"` is not a duration like \"30s\" or \"2m\"")
                })?;
                if d > Hold::CEILING {
                    return Err(format!(
                        "`questions.hold = \"{s}\"` is longer than {:?}, which is as long as an \
                         agent may be held for a person. A hold is seconds for somebody already \
                         looking at a phone, not a way to pause a session.",
                        Hold::CEILING
                    ));
                }
                if d.is_zero() {
                    return Err("`questions.hold` of zero is not a hold; remove it instead".into());
                }
                Ok(Some(Hold(d)))
            }
            other => Err(format!(
                "`questions.hold` takes `true` or a duration like \"30s\", not {other}"
            )),
        }
    }

    /// `None` for an unparseable value; an absent setting is
    /// `Deadline::Never`, never conflated with it.
    pub fn deadline(&self) -> Option<crate::core::ask::Deadline> {
        match self.deadline.as_deref() {
            None => Some(crate::core::ask::Deadline::Never),
            Some(s) => crate::core::ask::parse_deadline(s),
        }
    }
}

/// What a change may cost before somebody is asked about it. The dollar
/// ceiling only bites when the agent reports cost (optional in ACP), so
/// `devplane config check` warns when it is the only bound.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Budget {
    /// What one change may spend, in US dollars, as its agent reports it.
    pub max_usd: Option<f64>,
    /// How many agent turns one change may take. Counted from events Devplane
    /// saw itself, so it binds every agent.
    pub max_turns: Option<u32>,
    /// How long one change may run (`45m`, `2h`), measured from its start.
    /// Also always observable, and catches one very long turn.
    #[serde(default, with = "humantime_opt", rename = "max_runtime")]
    pub max_runtime: Option<Duration>,
}

impl Budget {
    /// Whether a bound exists that does not depend on agent-reported cost.
    pub fn has_observable_bound(&self) -> bool {
        self.max_turns.is_some() || self.max_runtime.is_some()
    }

    /// The dollar ceiling; zero means none.
    pub fn ceiling_usd(&self) -> Option<f64> {
        self.max_usd.filter(|c| *c > 0.0)
    }

    pub fn is_set(&self) -> bool {
        self.max_usd.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitHub {
    /// Let `change offer` push the branch and open the pull request itself —
    /// always as a draft, since nobody has read the work yet — when a person
    /// offers the change, never on its own when gates pass. Off by default
    /// (the offer then prints the push command and GitHub's address instead):
    /// pushing is the first thing Devplane does that other people can see.
    pub pull_request: bool,
    /// Issues carrying this label are offered as changes.
    pub ready_label: Option<String>,
}

/// How to read the specification a change names, in the project's own words.
/// No framework's sections are recognised: the outline is the Markdown
/// headings, progress is the `- [ ]` boxes, and the rest is declared here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpecSection {
    /// Words that mark an unanswered question (e.g. `NEEDS CLARIFICATION`,
    /// `TBD`). Empty by default; matched case-insensitively, per line.
    pub open_questions: Vec<String>,
    /// Where this repository keeps its plans, relative to the root. No
    /// default: layouts differ, and guessing would model a methodology.
    /// Absent, the Plans page says *not configured* rather than *nothing*.
    pub plans: Option<String>,
    /// Requirement-identifier prefixes; a token is a prefix followed by
    /// digits, and one appearing in a heading and a task line is an edge.
    /// Absent, the defaults are `FR-`, `NFR-`, `SC-`, `REQ-`, `US-`, `AC-`;
    /// setting it replaces them. A prefix may not contain a digit (bare `1.2`
    /// matches versions too), so write `["Req "]` to match `Req 12`.
    /// Case-sensitive.
    pub tokens: Option<Vec<String>>,
}

impl SpecSection {
    /// The configured token shapes, or the defaults. An invalid prefix is
    /// dropped here and reported by `devplane config check`.
    pub fn token_shapes(&self) -> Vec<crate::spec::TokenShape> {
        match self.tokens.as_deref() {
            Some(list) if !list.is_empty() => list
                .iter()
                .filter_map(|p| crate::spec::TokenShape::new(p).ok())
                .collect(),
            _ => crate::spec::TokenShape::defaults(),
        }
    }

    /// What in `[spec]` parses but cannot do what it says. An invalid token
    /// prefix is an error, so a declared notation never silently draws nothing.
    fn problems(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        match self.tokens.as_deref() {
            Some([]) => out.push(Problem::error(
                "[spec] tokens".into(),
                "is empty, which matches nothing; leave the key out for the defaults".into(),
            )),
            Some(list) => {
                for prefix in list {
                    if let Err(why) = crate::spec::TokenShape::new(prefix) {
                        out.push(Problem::error("[spec] tokens".into(), why));
                    }
                }
            }
            None => {}
        }
        out
    }
}

/// `[review]` — the reading order and the test mapping, both declared, never
/// inferred: `*_test.rs` is not a test until the project says so.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Review {
    /// Absent, the change is read in path order and shown as unordered.
    pub roles: Option<Roles>,
    /// Absent, there is no coverage column — not the same as *not covered*.
    pub covers: Option<Vec<Covers>>,
}

/// The six roles, in the order a change is read. A file takes the first
/// role whose patterns match it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Roles {
    pub shared: Vec<String>,
    pub logic: Vec<String>,
    pub security: Vec<String>,
    pub integration: Vec<String>,
    pub wiring: Vec<String>,
    pub tests: Vec<String>,
}

impl Roles {
    /// Each role's name as spelled in the file, with its patterns, in order.
    pub fn named(&self) -> [(&'static str, &[String]); 6] {
        [
            ("shared", &self.shared),
            ("logic", &self.logic),
            ("security", &self.security),
            ("integration", &self.integration),
            ("wiring", &self.wiring),
            ("tests", &self.tests),
        ]
    }
}

/// One declared test-to-source relationship.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Covers {
    /// Also matched against the gate commands to name the one that runs it.
    pub test: String,
    /// Gitignore syntax.
    pub paths: Vec<String>,
}

impl Review {
    /// What in `[review]` parses but cannot do what it says: a pattern that
    /// matches nothing (blank, comment, bare `!`), or a mapping with no paths,
    /// which would switch the coverage column on and mark every file uncovered.
    fn problems(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        let unreadable = |p: &str| {
            p.contains('\n') || crate::core::worktreeinclude::Patterns::parse(p).is_empty()
        };
        if let Some(roles) = &self.roles {
            for (role, patterns) in roles.named() {
                for p in patterns.iter().filter(|p| unreadable(p)) {
                    out.push(Problem::error(
                        format!("[review.roles] {role}"),
                        format!("has `{p}`, which is not a pattern — it would match nothing"),
                    ));
                }
            }
        }
        for c in self.covers.as_deref().unwrap_or_default() {
            if c.paths.is_empty() {
                out.push(Problem::error(
                    "[[review.covers]]".into(),
                    format!("`{}` names no paths, so it covers nothing", c.test),
                ));
            }
            for p in c.paths.iter().filter(|p| unreadable(p)) {
                out.push(Problem::error(
                    "[[review.covers]]".into(),
                    format!("`{}` has `{p}`, which is not a pattern", c.test),
                ));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PolicySection {
    /// Rules that refuse a permission request (Claude Code's `deny` list).
    pub deny: Vec<String>,
    /// Rules that always put the call in front of a person (Claude Code's
    /// `ask` list). Evaluated after `deny`.
    pub ask: Vec<String>,
}

/// The machine-wide rules, from `~/.devplane/policy.toml`, in the same
/// `[policy]` shape a project writes so rules can be pasted between them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalConfig {
    pub policy: PolicySection,
}

impl GlobalConfig {
    /// Parses `policy.toml`'s text, naming any removed key.
    pub fn parse(text: &str) -> Result<Self, String> {
        if let Ok(table) = text.parse::<toml::Table>()
            && let Some(why) = removed_key(&table)
        {
            return Err(why);
        }
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub fn policy(&self) -> crate::core::Policy {
        crate::core::Policy::rules(&self.policy.deny, &self.policy.ask)
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
    /// Parses a file's text, naming any removed key.
    pub fn parse(text: &str) -> Result<Self, String> {
        if let Ok(table) = text.parse::<toml::Table>()
            && let Some(why) = removed_key(&table)
        {
            return Err(why);
        }
        toml::from_str(text).map_err(|e| explain_unknown(&e))
    }

    /// The compiled permission policy.
    pub fn policy(&self) -> crate::core::Policy {
        crate::core::Policy::rules(&self.policy.deny, &self.policy.ask)
    }

    pub fn has_gates(&self) -> bool {
        !self.gates.check.is_empty()
    }

    /// Everything in this file that parses but cannot do what it says. Checked
    /// when a change starts and by `devplane config check`, before an agent is paid.
    pub fn validate(&self) -> Vec<Problem> {
        let mut out = Vec::new();

        // An unparseable deadline is an error: the wait stays unbounded (the
        // safe direction) rather than guessing which duration was meant.
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

        // A JUnit path is read inside the checkout only; one that could point
        // anywhere else is refused before anything reads it.
        let mut junit_seen: Vec<&str> = Vec::new();
        for path in &self.gates.junit {
            let p = std::path::Path::new(path);
            if path.is_empty()
                || p.is_absolute()
                || path.starts_with('/')
                || path.starts_with('\\')
                || p.components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                out.push(Problem::error(
                    "[gates] junit".into(),
                    format!("`{path}` must be relative to the checkout and stay inside it"),
                ));
            } else if junit_seen.contains(&path.as_str()) {
                out.push(Problem::warning(
                    "[gates] junit".into(),
                    format!("`{path}` is declared twice; it is read once"),
                ));
            }
            junit_seen.push(path);
        }
        for (name, gate) in &self.gates.named {
            if gate.run.is_empty() {
                out.push(Problem::error(
                    format!("[gates.{name}]"),
                    "has no commands, and an empty gate is not a pass".into(),
                ));
            }
        }
        // A deny rule that cannot work fails silently, and silence reads as
        // permission, so these are reported like Claude Code's own startup check.
        for (problem, what) in self.policy().problems() {
            let where_ = "[policy]".to_string();
            out.push(match problem {
                true => Problem::error(where_, what),
                false => Problem::warning(where_, what),
            });
        }

        // Warn only when the dollar ceiling is the whole bound.
        if self.budget.is_set() && !self.budget.has_observable_bound() {
            out.push(Problem::warning(
                "[budget]".into(),
                "a `max_usd` ceiling only stops work whose agent reports what it spent, which \
                 the protocol makes optional and the GenAI telemetry conventions cannot \
                 express at all. Add `max_turns` or `max_runtime`, which are counted here \
                 and bind every agent"
                    .into(),
            ));
        }

        out.extend(self.spec.problems());
        out.extend(self.review.problems());

        // An unknown cache name is an error listing the known ones.
        for name in &self.workspace.share {
            if crate::core::caches::lookup(name).is_none() {
                out.push(Problem::error(
                    "[workspace] share".into(),
                    format!(
                        "`{name}` is not an ecosystem this can share a cache for; the known \
                         names are {}",
                        crate::core::caches::known_names()
                            .iter()
                            .map(|n| format!("`{n}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }

        // The base branch is spliced into git argument lists and arrives with
        // somebody else's code, so an invalid name never reaches a command line.
        if let Some(base) = &self.project.base_branch
            && let Err(why) = valid_branch_name(base)
        {
            out.push(Problem::error(
                "[project] base_branch".into(),
                format!("`{base}` is not a branch name: {why}"),
            ));
        }
        out
    }
}

/// Whether git would accept `name` as a branch, by the documented rules of
/// `git check-ref-format --branch`. Pure, because `core/` may not spawn.
pub fn valid_branch_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("it is empty".into());
    }
    if name.starts_with('-') {
        return Err("it starts with `-`, which git reads as an option".into());
    }
    if name == "@" {
        return Err("`@` alone is not a branch name".into());
    }
    if name.contains("..") {
        return Err("it contains `..`".into());
    }
    if name.contains("@{") {
        return Err("it contains `@{`".into());
    }
    if let Some(c) = name
        .chars()
        .find(|c| c.is_control() || *c == ' ' || "~^:?*[\\".contains(*c))
    {
        return Err(format!("it contains {c:?}, which git reserves"));
    }
    if name.ends_with('/') || name.ends_with('.') {
        return Err("it ends with `/` or `.`".into());
    }
    for part in name.split('/') {
        if part.is_empty() {
            return Err("it has an empty path component (`//`, or a leading `/`)".into());
        }
        if part.starts_with('.') {
            return Err(format!("the component `{part}` starts with `.`"));
        }
        if part.ends_with(".lock") {
            return Err(format!("the component `{part}` ends with `.lock`"));
        }
    }
    Ok(())
}

impl ProjectConfig {
    /// The commands and timeout behind a gate name. `check` always resolves;
    /// an undeclared name is `None`, never an empty gate that passes.
    pub fn gate_named(&self, name: &str) -> Option<(Vec<String>, Duration)> {
        if name == "check" {
            return Some((self.gates.check.clone(), self.gates.timeout));
        }
        let g = self.gates.named.get(name)?;
        Some((g.run.clone(), g.timeout.unwrap_or(self.gates.timeout)))
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
    /// This file read back: what it will do and what cannot work. One
    /// derivation shared by `devplane config check` and the board, so they agree.
    /// Rules are in evaluation order, beside the findings reading the file
    /// cannot give: unused rules, overbroad ones, and half-protected paths.
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
            },
            "agent": {
                "default": self.agent.default,
                "max_parallel": self.agent.max_parallel,
                "stall_timeout": self.agent.stall_timeout.map(human),
                "keep_transcripts": self.agent.keep_transcripts,
            },
            "gates": {
                "check": self.gates.check,
                "timeout": human(self.gates.timeout),
                "max_feedback_rounds": self.gates.max_feedback_rounds,
                "named": self.gates.named.iter().map(|(name, g)| json!({
                    "name": name,
                    "run": g.run,
                    "timeout": g.timeout.map(human),
                })).collect::<Vec<_>>(),
                "junit": self.gates.junit,
                "base_checks": self.gates.base_checks.as_str(),
                "serial": self.gates.serial,
            },
            "policy": {
                "deny": rules(policy.deny_rules()),
                "ask": rules(policy.ask_rules()),
                "unused": policy.redundancies(),
                "overbroad": crate::core::policy::overbroad(&[]).into_iter().map(|o| json!({
                    "rule": o.rule, "why": o.why, "suggestion": o.suggestion,
                })).collect::<Vec<_>>(),
                "half_protected": policy.half_protected_paths(),
            },
            "budget": {
                "max_usd": self.budget.max_usd,
                "max_turns": self.budget.max_turns,
                "max_runtime": self.budget.max_runtime.map(human),
                "has_observable_bound": self.budget.has_observable_bound(),
            },
            "github": {
                "pull_request": self.github.pull_request,
                "ready_label": self.github.ready_label,
            },
            "workspace": {"setup": self.workspace.setup, "share": self.workspace.share},
            "spec": {"open_questions": self.spec.open_questions},
            "review": {"roles": self.review.roles, "covers": self.review.covers},
            "problems": self.validate(),
        })
    }
}

/// A removed key, named with what to do instead. `deny_unknown_fields` already
/// refuses it, but an unknown-field error reads as a typo.
fn removed_key(t: &toml::Table) -> Option<String> {
    // Renamed to the words the vendors' own settings use: refused by name,
    // with the new name, never read under the old one.
    const MOVED: &[(&str, &str, &str)] = &[
        ("policy", "never_auto", "`[policy] deny`"),
        ("policy", "always_ask", "`[policy] ask`"),
        ("policy", "max_parallel_runs", "`[agent] max_parallel`"),
        ("policy", "stall_timeout", "`[agent] stall_timeout`"),
        ("project", "default_agent", "`[agent] default`"),
        ("transcripts", "keep", "`[agent] keep_transcripts`"),
        ("budget", "usd", "`[budget] max_usd`"),
    ];
    for (section, key, now) in MOVED {
        if t.get(*section)
            .and_then(|s| s.as_table())
            .is_some_and(|s| s.contains_key(*key))
        {
            return Some(format!("`[{section}] {key}` is gone: write {now}"));
        }
    }
    if let Some(gates) = t.get("gates").and_then(|g| g.as_table()) {
        for key in gates.keys() {
            if let Some(why) = removed_gates_key(key) {
                return Some(why);
            }
        }
    }
    if t.get("github")
        .and_then(|g| g.as_table())
        .is_some_and(|g| g.contains_key("draft"))
    {
        return Some(
            "`[github] draft` is gone: a pull request Devplane opens is always a draft. \
             Delete the key"
                .into(),
        );
    }
    if t.contains_key("reports") {
        return Some(
            "`[reports]` was removed: projects no longer file reports to each other, so there \
             is nothing to deliver. Delete the section"
                .into(),
        );
    }
    if t.contains_key("pipelines") {
        return Some(
            "`[pipelines]` was removed in 0.10: a change is one agent in its own tree and the \
             project's `[gates] check`. Delete the section"
                .into(),
        );
    }
    if let Some(budget) = t.get("budget").and_then(|b| b.as_table()) {
        for key in [
            "default_usd",
            "quick_usd",
            "chore_usd",
            "bug_usd",
            "feature_usd",
        ] {
            if budget.contains_key(key) {
                return Some(format!(
                    "`[budget] {key}` was removed in 0.10: change kinds are gone, so there is \
                     one ceiling — write `max_usd` instead"
                ));
            }
        }
    }
    None
}

/// The parse error, with the retired `include` key pointed at its replacement.
fn explain_unknown(e: &toml::de::Error) -> String {
    let text = e.to_string();
    if text.contains("unknown field `include`") {
        return "`[workspace] include` is gone — write the paths to `.worktreeinclude` at the \
                repository root (gitignore syntax), which Claude Code reads too"
            .into();
    }
    text
}

/// Something in a configuration file that parses but cannot work.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    /// Where it is, in the file's own words: `[gates.slow]`.
    #[serde(rename = "where")]
    pub where_: String,
    pub what: String,
    /// Error rather than warning.
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
    use serde::Serializer;
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{}s", d.as_secs()))
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
            // Checked: an overflow would wrap to a short timeout in release.
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
    fn review_roles_and_covers_are_read_as_declared() {
        let c: ProjectConfig = toml::from_str(
            r#"
[review.roles]
shared   = ["src/types/**"]
security = ["src/auth/**"]

[[review.covers]]
test  = "tests/auth.rs"
paths = ["src/auth/**"]
"#,
        )
        .unwrap();
        let roles = c.review.roles.as_ref().expect("declared");
        assert_eq!(roles.shared, ["src/types/**"]);
        assert!(roles.logic.is_empty(), "an undeclared role is empty");
        let covers = c.review.covers.as_deref().expect("declared");
        assert_eq!(covers[0].test, "tests/auth.rs");
        assert!(c.validate().iter().all(|p| !p.where_.contains("review")));
        let none = ProjectConfig::default();
        assert!(none.review.roles.is_none() && none.review.covers.is_none());
    }

    #[test]
    fn a_review_pattern_that_reads_as_nothing_names_its_role() {
        let c: ProjectConfig =
            toml::from_str("[review.roles]\nlogic = [\"src/**\", \"# not a pattern\"]\n").unwrap();
        let p = c.validate();
        let hit = p
            .iter()
            .find(|p| p.where_ == "[review.roles] logic")
            .expect("the role is named");
        assert!(hit.fatal && hit.what.contains("# not a pattern"), "{hit}");
    }

    #[test]
    fn a_mapping_with_no_paths_is_an_error() {
        let c: ProjectConfig =
            toml::from_str("[[review.covers]]\ntest = \"tests/a.rs\"\npaths = []\n").unwrap();
        assert!(
            c.validate().iter().any(|p| p.fatal
                && p.where_ == "[[review.covers]]"
                && p.what.contains("tests/a.rs"))
        );
    }

    #[test]
    fn review_refuses_a_key_it_does_not_know() {
        for text in [
            "[review]\norder = []\n",
            "[review.roles]\nimportant = [\"src/**\"]\n",
            "[[review.covers]]\ntest = \"t\"\npaths = [\"a\"]\nweight = 2\n",
        ] {
            assert!(toml::from_str::<ProjectConfig>(text).is_err(), "{text}");
        }
    }

    #[test]
    fn a_gate_that_is_not_declared_is_not_a_pass() {
        let c: ProjectConfig = toml::from_str("[gates]\ncheck = [\"cargo test\"]\n").unwrap();
        assert!(c.gate_named("check").is_some());
        assert!(c.gate_named("repro-fails-on-base").is_none());
    }

    #[test]
    fn a_named_gate_is_read_with_its_own_timeout() {
        let c: ProjectConfig = toml::from_str(
            r#"
[gates.slow]
run = ["cargo test --test slow"]
timeout = "90s"
"#,
        )
        .unwrap();
        let (cmds, timeout) = c.gate_named("slow").expect("declared");
        assert_eq!(cmds, vec!["cargo test --test slow".to_string()]);
        assert_eq!(timeout, Duration::from_secs(90));
    }

    #[test]
    fn a_removed_key_is_refused_by_name_with_what_to_do() {
        for (text, key) in [
            (
                "[pipelines.feature]\nsteps = []\n",
                "`[pipelines]` was removed",
            ),
            (
                "[gates.named.repro]\nrun = [\"x\"]\n",
                "`[gates.named.<name>]` is `[gates.<name>]`",
            ),
            ("[policy]\nnever_auto = []\n", "write `[policy] deny`"),
            ("[policy]\nalways_ask = []\n", "write `[policy] ask`"),
            (
                "[policy]\nmax_parallel_runs = 2\n",
                "write `[agent] max_parallel`",
            ),
            (
                "[policy]\nstall_timeout = \"5m\"\n",
                "write `[agent] stall_timeout`",
            ),
            (
                "[project]\ndefault_agent = \"claude\"\n",
                "write `[agent] default`",
            ),
            (
                "[transcripts]\nkeep = false\n",
                "write `[agent] keep_transcripts`",
            ),
            ("[budget]\nusd = 5\n", "write `[budget] max_usd`"),
            (
                "[gates]\non_fail = \"escalate\"\n",
                "`[gates] on_fail` is gone",
            ),
            ("[github]\ndraft = false\n", "`[github] draft` is gone"),
            (
                "[budget]\nfeature_usd = 5\n",
                "`[budget] feature_usd` was removed",
            ),
            (
                "[budget]\ndefault_usd = 5\n",
                "`[budget] default_usd` was removed",
            ),
            (
                "[reports]\ndeliver_from = [\"api\"]\n",
                "`[reports]` was removed",
            ),
        ] {
            let err = ProjectConfig::parse(text).expect_err(text);
            assert!(err.contains(key), "{text}\n produced: {err}");
        }
    }

    #[test]
    fn a_gate_run_by_name_is_a_table_under_gates_and_check_is_not_one() {
        let c = parse("[gates]\ncheck = [\"t\"]\ntimeout = \"5m\"\n[gates.lint]\nrun = [\"l\"]\n");
        assert_eq!(c.gates.check, ["t"]);
        assert_eq!(c.gates.timeout, Duration::from_secs(300));
        assert_eq!(
            c.gate_named("lint").map(|g| g.1),
            Some(Duration::from_secs(300))
        );
        for (text, says) in [
            (
                "[gates.check]\nrun = [\"x\"]\n",
                "`check` is the definition of done",
            ),
            ("[gates]\nsmoke = \"x\"\n", "`[gates.smoke]`"),
            (
                "[gates.lint]\nrun = [\"x\"]\nexpect = 1\n",
                "`[gates.lint]`",
            ),
        ] {
            let err = ProjectConfig::parse(text).expect_err(text);
            assert!(err.contains(says), "{text}\n produced: {err}");
        }
    }

    /// `serial` is a flag of `[gates]`, off unless written; anything but a
    /// boolean is refused, and a misspelling names a gate table, not a pass.
    #[test]
    fn serial_gates_are_a_flag_and_off_by_default() {
        assert!(!parse("[gates]\ncheck = [\"t\"]\n").gates.serial);
        assert!(
            parse("[gates]\ncheck = [\"t\"]\nserial = true\n")
                .gates
                .serial
        );
        let err = ProjectConfig::parse("[gates]\nserial = \"yes\"\n").unwrap_err();
        assert!(err.contains("`[gates] serial` is true or false"), "{err}");
        let err = ProjectConfig::parse("[gates]\nserialise = true\n").unwrap_err();
        assert!(err.contains("serial"), "the keys are listed: {err}");
    }

    #[test]
    fn the_machine_wide_file_refuses_the_old_words_too() {
        let err = GlobalConfig::parse("[policy]\nnever_auto = []\n").unwrap_err();
        assert!(err.contains("`[policy] deny`"), "{err}");
    }

    fn parse(toml_text: &str) -> ProjectConfig {
        toml::from_str(toml_text).expect("parses")
    }

    #[test]
    fn an_empty_named_gate_is_named() {
        let c = parse("[gates]\ncheck = [\"cargo test\"]\n[gates.empty]\nrun = []\n");
        let all: Vec<String> = c.validate().iter().map(|p| p.to_string()).collect();
        assert!(all.join("\n").contains("empty"), "{all:?}");
    }

    #[test]
    fn a_junit_path_stays_inside_the_checkout_or_is_refused_by_name() {
        for bad in ["/tmp/r.xml", "../r.xml", "reports/../../r.xml"] {
            let c = parse(&format!("[gates]\njunit = [\"{bad}\"]\n"));
            let found = c.validate();
            assert!(
                found
                    .iter()
                    .any(|p| p.fatal && p.where_ == "[gates] junit" && p.what.contains(bad)),
                "{bad}: {found:?}"
            );
        }
        let twice = parse("[gates]\njunit = [\"r.xml\", \"r.xml\"]\n").validate();
        assert!(
            twice.iter().any(|p| !p.fatal && p.what.contains("twice")),
            "{twice:?}"
        );
        assert!(
            parse("[gates]\njunit = [\"reports/junit.xml\"]\n")
                .validate()
                .is_empty()
        );
    }

    #[test]
    fn base_checks_takes_three_values_and_the_old_key_is_unknown() {
        assert_eq!(parse("").gates.base_checks, BaseChecksWhen::Offer);
        assert_eq!(
            parse("[gates]\nbase_checks = \"auto\"\n").gates.base_checks,
            BaseChecksWhen::Auto
        );
        let bad = ProjectConfig::parse("[gates]\nbase_checks = \"always\"\n").unwrap_err();
        assert!(
            bad.contains("offer") && bad.contains("auto") && bad.contains("off"),
            "{bad}"
        );
        assert!(ProjectConfig::parse("[gates]\ncompare_base = true\n").is_err());
        let d = parse("[gates]\njunit = [\"r.xml\"]\nbase_checks = \"off\"\n").describe();
        assert_eq!(d["gates"]["base_checks"], "off");
        assert_eq!(d["gates"]["junit"][0], "r.xml");
    }

    #[test]
    fn a_cache_share_names_an_ecosystem_the_table_has_or_is_refused_by_name() {
        let c = parse("[workspace]\nshare = [\"npm\"]\n");
        let found: Vec<String> = c.validate().iter().map(|p| p.to_string()).collect();
        let all = found.join("\n");
        assert!(all.contains("`npm`"), "{all}");
        assert!(all.contains("`cargo`"), "the known names are listed: {all}");
        assert!(c.validate().iter().any(|p| p.fatal));
        assert!(
            parse("[workspace]\nshare = [\"cargo\"]\n")
                .validate()
                .is_empty()
        );
    }

    #[test]
    fn the_retired_include_key_is_refused_naming_the_file_to_write_instead() {
        let dir = tempdir("include-gone");
        std::fs::write(dir.join(CONFIG_FILE), "[workspace]\ninclude = [\".env\"]\n").unwrap();
        let err = ProjectConfig::load(&dir).unwrap_err().to_string();
        assert!(err.contains(".worktreeinclude"), "{err}");
        assert!(err.contains("`[workspace] include` is gone"), "{err}");
    }

    #[test]
    fn a_base_branch_that_is_not_a_branch_name_is_refused_before_it_reaches_git() {
        // `--output=/tmp/x` would be read by git as an option.
        for bad in [
            "--output=/tmp/x",
            "-x",
            "a..b",
            "a b",
            "main.lock",
            "release/",
            "a//b",
            "a/.hidden",
            "a~1",
            "a^",
            "a:b",
            "a?",
            "a*",
            "a[b",
            "a\\b",
            "main.",
            "@",
            "a@{1}",
            "tab\there",
            "",
        ] {
            let why = valid_branch_name(bad).expect_err(bad);
            assert!(!why.is_empty(), "{bad:?} refused with no reason");
            let c = parse(&format!("[project]\nbase_branch = {bad:?}\n"));
            assert!(
                c.validate()
                    .iter()
                    .any(|p| p.fatal && p.where_.contains("base_branch")),
                "`base_branch = {bad:?}` was accepted: {:?}",
                c.validate()
            );
        }
        for good in [
            "main",
            "master",
            "release/1.x",
            "feature-42",
            "a.b",
            "v1.0/rc",
        ] {
            assert_eq!(valid_branch_name(good), Ok(()), "{good}");
            let c = parse(&format!("[project]\nbase_branch = {good:?}\n"));
            assert!(
                !c.validate()
                    .iter()
                    .any(|p| p.where_.contains("base_branch")),
                "`{good}` is a branch name and was refused"
            );
        }
    }

    #[test]
    fn a_permission_rule_that_cannot_work_is_refused_before_an_agent_starts() {
        let c = parse(
            r#"
[policy]
deny = ["Write(src/**)", "Bash(command:rm *)", "mcp__github(create_issue)"]
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
        // Rules paste unchanged from settings.json.
        let c = parse(
            r#"
[policy]
deny = ["Bash(git push *)", "Read(./.env)", "Read(secrets/**)", "mcp__*"]
"#,
        );
        assert_eq!(c.validate(), vec![]);
    }

    #[test]
    fn a_budget_is_one_ceiling() {
        let c = parse("[budget]\nmax_usd = 5\n");
        assert_eq!(c.budget.ceiling_usd(), Some(5.0));
        assert_eq!(ProjectConfig::default().budget.ceiling_usd(), None);
        // Zero means off.
        assert_eq!(parse("[budget]\nmax_usd = 0\n").budget.ceiling_usd(), None);
        assert!(
            c.validate()
                .iter()
                .any(|p| !p.fatal && p.what.contains("reports what it spent")),
            "a guard that may not fire must say so"
        );
    }

    #[test]
    fn a_bound_that_always_fires_silences_the_warning_about_one_that_may_not() {
        let money_only = parse("[budget]\nmax_usd = 10\n");
        assert!(
            money_only
                .validate()
                .iter()
                .any(|p| !p.fatal && p.what.contains("reports what it spent")),
            "a guard that may not fire must say so"
        );
        let bounded = parse("[budget]\nmax_usd = 10\nmax_turns = 60\n");
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
[gates.slow]
run = ["cargo test --test slow"]
"#,
        );
        assert_eq!(c.validate(), vec![]);
    }

    #[test]
    fn a_repository_with_no_file_still_works() {
        let c = ProjectConfig::load(std::path::Path::new("/definitely/not/here")).unwrap();
        assert!(!c.has_gates());
        assert_eq!(c.gates.max_feedback_rounds, 2);
        assert!(c.policy.deny.is_empty());
        assert_eq!(c.agent.stall_timeout, None);
        assert!(
            c.agent.keep_transcripts,
            "what a driven agent says is kept by default"
        );
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

[agent]
default = "claude"
max_parallel = 2
stall_timeout = "12m"
keep_transcripts = false

[workspace]
setup = "pnpm install --frozen-lockfile"
share = ["cargo"]

[gates]
check = ["pnpm typecheck", "pnpm test -- --run"]
timeout = "10m"
max_feedback_rounds = 3

[gates.slow]
run = ["pnpm test:e2e"]

[policy]
deny = ["Bash(git push *)"]

[github]
pull_request = true
ready_label = "devplane:ready"
"#,
        )
        .unwrap();
        let c = ProjectConfig::load(&dir).unwrap();
        assert_eq!(c.project.name.as_deref(), Some("saas"));
        assert_eq!(c.workspace.share, vec!["cargo".to_string()]);
        assert_eq!(c.gates.check.len(), 2);
        assert_eq!(c.gates.timeout, Duration::from_secs(600));
        assert_eq!(c.gates.max_feedback_rounds, 3);
        assert_eq!(c.agent.default.as_deref(), Some("claude"));
        assert_eq!(c.agent.max_parallel, Some(2));
        assert_eq!(c.agent.stall_timeout, Some(Duration::from_secs(720)));
        assert!(!c.agent.keep_transcripts);
        assert_eq!(
            c.gate_named("slow").map(|g| g.0),
            Some(vec!["pnpm test:e2e".into()])
        );
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
        let c = ProjectConfig::default();
        assert!(!c.github.pull_request);
    }

    #[test]
    fn a_typo_is_an_error_rather_than_a_silent_default() {
        // A misspelled deny key must not silently remove the rule.
        let dir = tempdir("typo");
        std::fs::write(
            dir.join(CONFIG_FILE),
            "[policy]\ndenyy = [\"Bash(git push *)\"]\n",
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
            "[policy]\ndeny = [\n  \"Bash(rm -rf *)\",  # a comment\n  \"Read(.env)\",\n]\n",
        )
        .unwrap();
        let g = GlobalConfig::load(&dir).unwrap();
        assert_eq!(g.policy.deny, ["Bash(rm -rf *)", "Read(.env)"]);
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
        let g = GlobalConfig::load(std::path::Path::new("/definitely/not/here")).unwrap();
        assert!(g.policy.deny.is_empty());
        assert!(g.policy.ask.is_empty());
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
        // Overflow is refused, not wrapped.
        assert_eq!(parse("999999999999999d"), None);
        assert_eq!(parse("99999999999999999999"), None);
    }

    fn tempdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-cfg-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

#[cfg(test)]
mod hold_tests {
    use super::*;

    fn cfg(toml_src: &str) -> ProjectConfig {
        toml::from_str(toml_src).expect("a fixture parses")
    }

    #[test]
    fn a_project_that_says_nothing_holds_nothing() {
        assert_eq!(cfg("").questions.hold(), Ok(None));
        assert_eq!(cfg("[questions]\nhold = false").questions.hold(), Ok(None));
    }

    #[test]
    fn turning_it_on_without_a_duration_is_thirty_seconds() {
        assert_eq!(
            cfg("[questions]\nhold = true").questions.hold(),
            Ok(Some(Hold(Duration::from_secs(30))))
        );
        assert_eq!(Hold::DEFAULT, Duration::from_secs(30));
    }

    #[test]
    fn a_named_duration_is_taken() {
        for (src, secs) in [("10s", 10u64), ("45s", 45), ("2m", 120)] {
            assert_eq!(
                cfg(&format!("[questions]\nhold = \"{src}\""))
                    .questions
                    .hold(),
                Ok(Some(Hold(Duration::from_secs(secs)))),
                "`{src}`"
            );
        }
    }

    #[test]
    fn a_value_that_will_not_parse_is_refused_and_leaves_no_hold() {
        // `"30"` is absent: a bare number is seconds, as for every duration key.
        for bad in ["\"soon\"", "\"0s\"", "12", "[]"] {
            let r = cfg(&format!("[questions]\nhold = {bad}")).questions.hold();
            assert!(r.is_err(), "`hold = {bad}` was accepted: {r:?}");
            let says = r.unwrap_err();
            assert!(
                says.contains("hold"),
                "the refusal does not name the key: {says}"
            );
        }
    }

    #[test]
    fn a_hold_may_not_outlast_what_the_vendor_will_wait() {
        assert!(
            Hold::CEILING < Duration::from_secs(600),
            "the ceiling is at or past the vendor's own hook timeout"
        );
        let r = cfg("[questions]\nhold = \"10m\"").questions.hold();
        assert!(r.is_err(), "a ten-minute hold was accepted");
        assert!(
            r.unwrap_err().contains("as long as an agent may be held"),
            "the refusal does not say why"
        );
        assert_eq!(HOLD_CEILING_MS, Hold::CEILING.as_millis() as u64);
    }
}

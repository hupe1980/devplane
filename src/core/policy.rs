//! The permission policy: it can prohibit a call or defer it to a person;
//! nothing here answers yes. The rule syntax is Claude Code's, but a command
//! rule matches tokens after wrappers are stripped (`sudo rm -fr /` meets
//! `Bash(rm -rf *)`), and an unreadable line is [`Verdict::Unresolved`] rather
//! than silence — being wider only ever costs a prompt.

use crate::core::command::{self, Access, FileTarget, Line, Simple, Via};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

/// Which list a rule sits in. `Allow` exists for reading the agent's own
/// settings and for a composed offer; the policy holds only the other two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Allow,
    Deny,
    Ask,
}

impl Class {
    fn is_restrictive(self) -> bool {
        matches!(self, Class::Deny | Class::Ask)
    }
}

/// What the policy decided about one tool call, variants in the order they win.
/// `Unresolved` (*nobody could look*) is distinct from `Undecided`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Deny {
        rule: String,
    },
    Ask {
        rule: String,
    },
    /// A command prohibition is in force and this line hides what runs;
    /// answered as ask.
    Unresolved {
        why: String,
    },
    Undecided,
}

impl Verdict {
    pub fn rule(&self) -> Option<&str> {
        match self {
            Verdict::Deny { rule } | Verdict::Ask { rule } => Some(rule),
            Verdict::Unresolved { .. } | Verdict::Undecided => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Deny { .. } => "deny",
            Verdict::Ask { .. } => "ask",
            Verdict::Unresolved { .. } => "unresolved",
            Verdict::Undecided => "undecided",
        }
    }
    pub fn why(&self) -> Option<&str> {
        match self {
            Verdict::Unresolved { why } => Some(why),
            _ => None,
        }
    }
}

/// Anchors and a symlink resolver for path rules, supplied by the caller so
/// this module never touches a disk.
#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub cwd: &'a Path,
    pub home: Option<&'a Path>,
    /// Where a single leading slash anchors: the rule file's directory.
    pub source: &'a Path,
    /// Returns `None` when a path resolves to itself; absent, symlinks are off.
    pub realpath: Option<fn(&Path) -> Option<PathBuf>>,
}

impl<'a> Context<'a> {
    pub fn at(dir: &'a Path) -> Self {
        Self {
            cwd: dir,
            home: None,
            source: dir,
            realpath: None,
        }
    }
    pub fn with_home(mut self, home: Option<&'a Path>) -> Self {
        self.home = home;
        self
    }
    pub fn with_source(mut self, source: &'a Path) -> Self {
        self.source = source;
        self
    }
    pub fn with_realpath(mut self, f: fn(&Path) -> Option<PathBuf>) -> Self {
        self.realpath = Some(f);
        self
    }
}

impl fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("cwd", &self.cwd)
            .field("home", &self.home)
            .field("source", &self.source)
            .field("realpath", &self.realpath.is_some())
            .finish()
    }
}

// --- Tool vocabulary ---

pub fn rule_content_field(tool: &str) -> Option<&'static str> {
    match tool {
        "Bash" | "PowerShell" | "Monitor" => Some("command"),
        "Read" | "Edit" | "Write" | "MultiEdit" => Some("file_path"),
        "NotebookEdit" => Some("notebook_path"),
        "Glob" | "Grep" => Some("path"),
        "WebFetch" => Some("url"),
        _ => None,
    }
}

/// The keys a file tool whose field is undocumented may carry its path under.
const PATH_FIELDS: &[&str] = &["file_path", "path", "uri", "filePath"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Command,
    FilePath,
    Url,
    Opaque,
}

fn shape_of(tool: &str) -> Shape {
    match tool {
        "Bash" | "PowerShell" | "Monitor" => Shape::Command,
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "Glob" | "Grep" | "LSP" => {
            Shape::FilePath
        }
        "WebFetch" => Shape::Url,
        _ => Shape::Opaque,
    }
}

/// Claude Code's documented built-in tools, to warn (only) on a typo.
/// `scripts/verify-claims.sh` checks it against the vendored reference.
#[rustfmt::skip]
const KNOWN_TOOLS: &[&str] = &[
    "Agent", "Artifact", "AskUserQuestion", "Bash", "CronCreate", "CronDelete", "CronList", "Edit",
    "EndConversation", "EnterPlanMode", "EnterWorktree", "ExitPlanMode", "ExitWorktree", "Glob",
    "Grep", "LSP", "ListAgents", "ListMcpResourcesTool", "Monitor", "MultiEdit", "NotebookEdit",
    "PowerShell", "PushNotification", "Read", "ReadMcpResourceTool", "RemoteTrigger",
    "ReportFindings", "ScheduleWakeup", "SendFeedback", "SendMessage", "SendUserFile",
    "ShareOnboardingGuide", "Skill", "TaskCreate", "TaskGet", "TaskList", "TaskOutput", "TaskStop",
    "TaskUpdate", "TodoWrite", "ToolSearch", "WaitForMcpServers", "WebFetch", "WebSearch",
    "Workflow", "Write",
];

/// The Claude Code release the rule syntax was modelled on.
pub const SYNTAX_MODELLED_ON: &str = "2.1.273";

pub fn is_command_tool(tool: &str) -> bool {
    shape_of(tool) == Shape::Command
}

/// A POSIX command line whose file operands path rules reach (not PowerShell).
pub fn is_shell(tool: &str) -> bool {
    matches!(tool, "Bash" | "Monitor")
}

fn reads_files(tool: &str) -> bool {
    matches!(tool, "Read" | "Grep" | "Glob" | "LSP")
}

fn edits_files(tool: &str) -> bool {
    matches!(tool, "Edit" | "Write" | "MultiEdit" | "NotebookEdit")
}

pub fn rule_content(tool: &str, input: &Value) -> Option<String> {
    if let Some(key) = rule_content_field(tool) {
        return input.get(key).and_then(Value::as_str).map(str::to_string);
    }
    if shape_of(tool) == Shape::FilePath {
        return PATH_FIELDS
            .iter()
            .find_map(|k| input.get(*k).and_then(Value::as_str))
            .map(str::to_string);
    }
    None
}

// --- Rules ---

#[derive(Debug, Clone, PartialEq, Eq)]
enum ToolPattern {
    Exact(String),
    /// Every tool from one MCP server: `mcp__github`.
    Server(String),
    /// A glob after a literal server: `mcp__github__get_*`.
    Anchored(String),
    /// A glob over the whole tool name; deny side only.
    Glob(String),
}

impl ToolPattern {
    fn matches(&self, tool: &str, class: Class) -> bool {
        match self {
            ToolPattern::Exact(t) => t.eq_ignore_ascii_case(tool),
            ToolPattern::Server(s) => tool
                .strip_prefix(s.as_str())
                .is_some_and(|r| r.starts_with("__") && r.len() > 2),
            ToolPattern::Anchored(g) => wildcard(g, tool),
            ToolPattern::Glob(g) => class.is_restrictive() && wildcard(g, tool),
        }
    }
    fn as_str(&self) -> &str {
        match self {
            ToolPattern::Exact(s)
            | ToolPattern::Server(s)
            | ToolPattern::Anchored(s)
            | ToolPattern::Glob(s) => s,
        }
    }
}

/// A command rule as tokens: program, flags required anywhere, operands in
/// order (`*` spans any number), so `rm -rf *` also meets `rm -r -f /`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandSpec {
    program: String,
    flags: Vec<String>,
    operands: Vec<String>,
    /// For PowerShell's literal comparison.
    literal: Vec<String>,
}

impl CommandSpec {
    fn parse(spec: &str) -> Self {
        let words: Vec<String> = spec.split_whitespace().map(str::to_string).collect();
        let program = words
            .first()
            .map(|p| p.rsplit('/').next().unwrap_or(p).to_ascii_lowercase())
            .unwrap_or_default();
        let (flags, operands) = split_flags(words.get(1..).unwrap_or(&[]));
        Self {
            program,
            flags: flags.into_iter().map(|f| f.to_ascii_lowercase()).collect(),
            operands: operands.into_iter().map(|o| o.text).collect(),
            literal: words.iter().map(|w| w.to_ascii_lowercase()).collect(),
        }
    }

    /// `as_set` (restrictive side): an exact rule compares flags as a set, since
    /// missing a respelling is the dangerous direction.
    fn matches(&self, cmd: &Simple, as_set: bool) -> bool {
        if !wildcard(&self.program, &cmd.program) {
            return false;
        }
        let (flags, operands) = split_flags(&cmd.args);
        let letters: String = flags
            .iter()
            .filter(|f| !f.starts_with("--"))
            .flat_map(|f| f.chars().skip(1).filter(char::is_ascii_alphabetic))
            .collect::<String>()
            .to_ascii_lowercase();
        let has = |want: &str| {
            if want.starts_with("--") {
                flags.iter().any(|f| {
                    f.eq_ignore_ascii_case(want)
                        || f.to_ascii_lowercase().starts_with(&format!("{want}="))
                })
            } else {
                want.chars()
                    .skip(1)
                    .filter(char::is_ascii_alphabetic)
                    .all(|c| letters.contains(c))
            }
        };
        let exact = self.is_exact();
        if !self.flags.iter().all(|f| has(f)) {
            return false;
        }
        if exact {
            let same = if as_set {
                flag_set(&flags) == flag_set(&self.flags)
            } else {
                flags.len() == self.flags.len()
            };
            if !same {
                return false;
            }
        }
        // An operand after a flag may be its value (`git -C x push`).
        let values = operands.iter().take_while(|o| o.after_flag).count();
        (0..=values).any(|skip| {
            tokens_match(
                &self.operands,
                &operands[skip..]
                    .iter()
                    .map(|o| o.text.as_str())
                    .collect::<Vec<_>>(),
            )
        })
    }

    fn is_exact(&self) -> bool {
        !self.program.contains(['*', '?']) && !self.operands.iter().any(|t| t.contains(['*', '?']))
    }

    /// Whether every call `other` names, this names too. An exact rule covers
    /// only itself: `Bash(rm)` does not cover `Bash(rm -f)`.
    fn covers(&self, other: &CommandSpec) -> bool {
        if self.is_exact() {
            return other.is_exact()
                && self.program == other.program
                && flag_set(&self.flags) == flag_set(&other.flags)
                && self.operands == other.operands;
        }
        let plain = |t: &str| !t.contains('?');
        self.program == other.program
            && self.flags.iter().all(|f| other.flags.contains(f))
            && self.operands.iter().all(|t| plain(t))
            && other.operands.iter().all(|t| plain(t))
            && match self.operands.split_last() {
                Some((last, head)) if last == "*" => {
                    head.len() <= other.operands.len()
                        && head
                            .iter()
                            .zip(&other.operands)
                            .all(|(a, b)| a == b || (a.contains('*') && wildcard(a, b)))
                }
                _ => self.operands == other.operands,
            }
    }
}

struct Operand {
    text: String,
    after_flag: bool,
}

/// Short letters however bundled, plus long flags, lower-cased.
fn flag_set<S: AsRef<str>>(flags: &[S]) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for f in flags {
        let f = f.as_ref().to_ascii_lowercase();
        if f.starts_with("--") {
            out.insert(f);
        } else {
            for c in f.chars().skip(1) {
                out.insert(format!("-{c}"));
            }
        }
    }
    out
}

fn split_flags<S: AsRef<str>>(args: &[S]) -> (Vec<String>, Vec<Operand>) {
    let (mut flags, mut operands) = (Vec::new(), Vec::new());
    let mut ended = false;
    let mut after_flag = false;
    for a in args {
        let a = a.as_ref();
        if !ended && a == "--" {
            ended = true;
            after_flag = false;
        } else if !ended && a.starts_with('-') && a.len() > 1 {
            flags.push(a.to_string());
            after_flag = true;
        } else {
            operands.push(Operand {
                text: a.to_string(),
                after_flag,
            });
            after_flag = false;
        }
    }
    (flags, operands)
}

/// A `*` token spans any run of tokens; any other token globs over one.
fn tokens_match(pattern: &[String], tokens: &[&str]) -> bool {
    star_match(pattern, tokens, |p| p == "*", |p, t| wildcard(p, t))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Spec {
    Any,
    Command(CommandSpec),
    Path(PathPattern),
    Domain(String),
    Content(String),
    /// `name:value` over a top-level input field; deny side only.
    Param {
        name: String,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    raw: String,
    class: Class,
    tool: ToolPattern,
    spec: Spec,
    /// Unclosed bracket; read as written, reported by `problems`.
    malformed: bool,
    /// `!`: an exception scoped to its own list.
    negated: bool,
}

impl Rule {
    /// `None` for whitespace or a bare `!`.
    pub fn parse(raw: &str, class: Class) -> Option<Self> {
        let raw = raw.trim();
        let (negated, raw) = match raw.strip_prefix('!') {
            Some(rest) if !rest.trim().is_empty() => (true, rest.trim()),
            Some(_) => return None,
            None if raw.is_empty() => return None,
            None => (false, raw),
        };
        let mut malformed = false;
        let (tool_part, spec_part) = match raw.split_once('(') {
            Some((t, rest)) => (
                t.trim(),
                Some(rest.strip_suffix(')').unwrap_or_else(|| {
                    malformed = true;
                    rest
                })),
            ),
            None => (raw, None),
        };
        let tool = if let Some(rest) = tool_part.strip_prefix("mcp__") {
            match rest.split_once("__") {
                _ if rest.starts_with('*') => ToolPattern::Glob(tool_part.to_string()),
                Some((server, "*")) | Some((server, "")) => {
                    ToolPattern::Server(format!("mcp__{server}"))
                }
                Some((server, t)) if t.contains('*') && !server.contains('*') => {
                    ToolPattern::Anchored(tool_part.to_string())
                }
                Some(_) => ToolPattern::Exact(tool_part.to_string()),
                None => ToolPattern::Server(tool_part.to_string()),
            }
        } else if tool_part.contains('*') {
            ToolPattern::Glob(tool_part.to_string())
        } else {
            ToolPattern::Exact(tool_part.to_string())
        };
        let spec = match spec_part {
            None | Some("") | Some("*") => Spec::Any,
            Some(s) => parse_spec(tool.as_str(), s),
        };
        Some(Self {
            raw: if negated {
                format!("!{raw}")
            } else {
                raw.to_string()
            },
            class,
            tool,
            spec,
            malformed,
            negated,
        })
    }

    /// Whether every call `other` speaks for, this does too. Undecidable is
    /// *no*, so reports under-state rather than invite a deletion.
    pub fn covers_rule(&self, other: &Rule) -> bool {
        if self.negated
            || other.negated
            || self.malformed
            || other.malformed
            || self.tool != other.tool
        {
            return false;
        }
        match (&self.spec, &other.spec) {
            (Spec::Any, _) => true,
            (Spec::Command(a), Spec::Command(b)) => a.covers(b),
            (a, b) => a == b,
        }
    }

    pub fn is_malformed(&self) -> bool {
        self.malformed
    }
    pub fn is_negated(&self) -> bool {
        self.negated
    }
    pub fn as_str(&self) -> &str {
        &self.raw
    }
    pub fn class(&self) -> Class {
        self.class
    }

    pub fn matches(&self, ctx: &Context<'_>, tool: &str, input: &Value) -> bool {
        match &self.spec {
            Spec::Path(p) if is_shell(tool) && !self.path_tool_applies(tool) => {
                rule_content(tool, input).is_some_and(|c| {
                    self.shell_path_matches(p, ctx, &command::file_targets(&c).list)
                })
            }
            _ => self.matches_read(ctx, tool, input, None),
        }
    }

    fn matches_read(
        &self,
        ctx: &Context<'_>,
        tool: &str,
        input: &Value,
        line: Option<&Line>,
    ) -> bool {
        if self.malformed && self.class == Class::Allow {
            return false;
        }
        let set = self.class.is_restrictive();
        match &self.spec {
            // `Edit(x)` governs every editor; `Read(.env)` also stops `cat .env`.
            Spec::Path(p) if self.path_tool_applies(tool) => rule_content(tool, input)
                .is_some_and(|raw| p.matches(ctx, Path::new(&raw), self.class)),
            Spec::Path(p) => {
                line.is_some_and(|l| self.shell_path_matches(p, ctx, &command::targets_of(l)))
            }
            _ if !self.command_tool_applies(tool) => false,
            Spec::Any => true,
            Spec::Command(spec) => match (line, rule_content(tool, input)) {
                // Literal tokens; misses surface as `Unresolved` in `restrictive`.
                (_, Some(c)) if tool == "PowerShell" => {
                    let prefix = spec.literal.last().is_some_and(|l| l == "*");
                    let head = &spec.literal[..spec.literal.len() - usize::from(prefix)];
                    c.split(['|', ';', '\n']).any(|part| {
                        let tokens: Vec<String> = part
                            .split_whitespace()
                            .map(|w| w.to_ascii_lowercase())
                            .collect();
                        if prefix {
                            tokens.starts_with(head)
                        } else {
                            tokens == head
                        }
                    })
                }
                (Some(l), _) => l.commands.iter().any(|c| spec.matches(c, set)),
                (None, Some(c)) => command::read(&c)
                    .commands
                    .iter()
                    .any(|c| spec.matches(c, set)),
                (None, None) => false,
            },
            Spec::Domain(host) => rule_content(tool, input)
                .and_then(|u| host_of(&u))
                .is_some_and(|h| wildcard(host, &h)),
            Spec::Content(pattern) => {
                rule_content(tool, input).is_some_and(|c| wildcard(pattern, &c))
            }
            // Not on the content field: a compound command steps around it.
            Spec::Param { .. } if self.class == Class::Allow => false,
            Spec::Param { name, .. } if rule_content_field(tool) == Some(name.as_str()) => false,
            Spec::Param { name, value } => {
                input.get(name).is_some_and(|v| wildcard(value, &scalar(v)))
            }
        }
    }

    /// A `Bash` rule reaches `Monitor`, which runs through the same shell.
    fn command_tool_applies(&self, tool: &str) -> bool {
        (tool == "Monitor" && self.tool.as_str().eq_ignore_ascii_case("Bash"))
            || self.tool.matches(tool, self.class)
    }

    fn path_tool_applies(&self, tool: &str) -> bool {
        let named = self.tool.as_str();
        if named.eq_ignore_ascii_case("Edit") {
            return edits_files(tool);
        }
        if named.eq_ignore_ascii_case("Read") {
            // Restrictive `Read` also forbids replacing (vendor excludes NotebookEdit).
            return reads_files(tool)
                || (self.class.is_restrictive() && edits_files(tool) && tool != "NotebookEdit");
        }
        false
    }

    /// `Edit` governs writes; `Read` governs reads and, restrictively, what a
    /// recognised command writes (`tee .env` replaces it).
    fn shell_path_matches(
        &self,
        p: &PathPattern,
        ctx: &Context<'_>,
        targets: &[FileTarget],
    ) -> bool {
        if !is_shell_rule_tool(self.tool.as_str()) {
            return false;
        }
        let edit = self.tool.as_str().eq_ignore_ascii_case("Edit");
        let restrictive = self.class.is_restrictive();
        for t in targets {
            if t.unresolvable && !restrictive {
                continue;
            }
            let governs = if edit {
                t.access == Access::Write
            } else {
                t.access == Access::Read
                    || (restrictive && t.access == Access::Write && t.via == Via::Command)
            };
            if !governs {
                continue;
            }
            let path = Path::new(&t.path);
            if p.matches(ctx, path, self.class)
                || (restrictive && t.subtree && p.covers_under(ctx, path))
            {
                return true;
            }
        }
        false
    }

    /// `(fatal, sentence)` for everything the gate would not apply or that
    /// reads as more than it is; fatal means the rule does nothing there.
    pub fn problems(&self) -> Vec<(bool, String)> {
        let mut out = Vec::new();
        let raw = &self.raw;
        let named = self.tool.as_str();
        if self.malformed {
            let what = match raw.rfind(')') {
                Some(i) if i + 1 < raw.len() => format!(
                    "`{raw}` has text after its closing bracket and matches nothing — did you mean `{}`?",
                    &raw[..=i]
                ),
                _ => format!("`{raw}` is missing its closing bracket"),
            };
            return vec![(true, what)];
        }
        if self.negated && self.class == Class::Allow {
            return vec![(
                true,
                format!(
                    "`{raw}` is a negation in an allow list, and `!` is read only in a deny or ask list"
                ),
            )];
        }
        if named.eq_ignore_ascii_case("Cd") {
            return vec![(
                true,
                format!(
                    "`{raw}` is a `Cd` rule. Those govern the `/cd` slash command — a person moving the session, not a tool call — so nothing reaches Devplane's gate to match it. Keep it in `settings.json`, where Claude Code evaluates it"
                ),
            )];
        }
        if named.starts_with("mcp__") && raw.contains('(') {
            out.push((true, format!("`{raw}` gives an MCP tool a specifier, and Claude Code skips any `mcp__` rule with brackets. Name the tool alone: `{named}`")));
        }
        if self.class.is_restrictive()
            && matches!(self.tool, ToolPattern::Exact(_))
            && !named.contains(['_', '*'])
            && !KNOWN_TOOLS.iter().any(|t| t.eq_ignore_ascii_case(named))
        {
            out.push((false, format!("`{raw}` names `{named}`, which is not a tool Claude Code documents. A prohibition with a typo in it matches nothing. Check the spelling — the name in the transcript is not always the one rules use, and `Stop Task` is written `TaskStop`")));
        }
        if matches!(self.tool, ToolPattern::Glob(_)) && self.class == Class::Allow {
            out.push((true, format!("`{raw}` is an unanchored wildcard in an allow list, which approves nothing — a tool-name glob is a deny-side pattern")));
        }
        match &self.spec {
            Spec::Path(_)
                if !named.eq_ignore_ascii_case("Read") && !named.eq_ignore_ascii_case("Edit") =>
            {
                let replacement = if reads_files(named) { "Read" } else { "Edit" };
                out.push((true, format!("`{raw}` puts a path on `{named}`, and file permissions are only checked against `Read(…)` and `Edit(…)`. Write it as `{replacement}(…)` — `{named}` rules are accepted and never consulted")));
            }
            Spec::Command(_) if named.eq_ignore_ascii_case("Monitor") => {
                out.push((true, format!("`{raw}` puts a command pattern on `Monitor`, which has no rule format of its own. Write it as `Bash(…)`, which governs both the foreground command and the one `Monitor` runs in the background")));
            }
            Spec::Command(c)
                if c.operands.first().is_none_or(|o| o == "*") && !c.flags.is_empty() =>
            {
                out.push((false, format!("`{raw}` names its flags, so it reaches `{0} {1}` however the letters are spelled or split and not `{0}` with fewer of them or with a long flag. `{named}({0} *)` is the robust spelling", c.program, c.flags.join(" "))));
            }
            Spec::Param { name, .. } if rule_content_field(named) == Some(name.as_str()) => {
                let example = match shape_of(named) {
                    Shape::Command => "Bash(rm *)",
                    Shape::FilePath => "Read(./path)",
                    Shape::Url => "WebFetch(domain:host)",
                    Shape::Opaque => "Tool(value)",
                };
                out.push((true, format!("`{raw}` matches `{named}`'s own content field, which a compound command can step around, so Claude Code ignores it. Write it the way `{example}` is written")));
            }
            Spec::Param { .. } if self.class == Class::Allow => {
                out.push((true, format!("`{raw}` is a parameter rule in an allow list. One parameter being safe does not make a call safe, so parameter rules are deny-side only")));
            }
            Spec::Content(_) if rule_content_field(named).is_none() => {
                out.push((true, format!("`{raw}` gives `{named}` a specifier, and `{named}` has no field for one to match. Name the tool alone, or address an input parameter: `{named}(model:opus)`")));
            }
            _ => {}
        }
        out
    }
}

fn is_shell_rule_tool(named: &str) -> bool {
    named.eq_ignore_ascii_case("Read") || named.eq_ignore_ascii_case("Edit")
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

fn parse_spec(tool: &str, spec: &str) -> Spec {
    let shape = shape_of(tool);
    if shape == Shape::Url
        && let Some(host) = spec.strip_prefix("domain:")
    {
        return Spec::Domain(host.trim().to_string());
    }
    // `:*` is the documented trailing-wildcard spelling of ` *`.
    let spec = match spec.strip_suffix(":*") {
        Some(head) if !head.is_empty() => format!("{head} *"),
        _ => spec.to_string(),
    };
    // Only fields this tool could have, so `Bash(git:* push)` stays a command.
    if let Some((name, value)) = spec.split_once(':')
        && is_identifier(name.trim())
        && is_parameter(tool, name.trim())
    {
        return Spec::Param {
            name: name.trim().to_string(),
            value: value.trim().to_string(),
        };
    }
    match shape {
        Shape::Command => Spec::Command(CommandSpec::parse(&spec)),
        Shape::FilePath => Spec::Path(PathPattern::parse(&spec)),
        Shape::Url | Shape::Opaque => Spec::Content(spec),
    }
}

fn is_parameter(tool: &str, name: &str) -> bool {
    rule_content_field(tool) == Some(name)
        || match shape_of(tool) {
            Shape::Command => matches!(name, "run_in_background" | "timeout" | "description"),
            Shape::FilePath | Shape::Url => false,
            Shape::Opaque => true,
        }
}

fn is_identifier(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest
        .split(['/', '?', '#'])
        .next()?
        .rsplit('@')
        .next()?
        .split(':')
        .next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

// --- Paths ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    /// `//path`
    Root,
    /// `~/path`
    Home,
    /// `/path`, relative to the rule file.
    Source,
    Cwd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPattern {
    anchor: Anchor,
    segments: Vec<String>,
    /// A bare filename matches at any depth.
    any_depth: bool,
    /// `dir/**` floats to any depth on the deny side only.
    single_segment_dir: bool,
}

impl PathPattern {
    fn parse(raw: &str) -> Self {
        let (anchor, rest) = if let Some(r) = raw.strip_prefix("//") {
            (Anchor::Root, r)
        } else if let Some(r) = raw.strip_prefix("~/") {
            (Anchor::Home, r)
        } else if let Some(r) = raw.strip_prefix('/') {
            (Anchor::Source, r)
        } else {
            (Anchor::Cwd, raw.strip_prefix("./").unwrap_or(raw))
        };
        let segments: Vec<String> = rest
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let any_depth = anchor == Anchor::Cwd && segments.len() == 1 && !segments[0].contains("**");
        let single_segment_dir = anchor == Anchor::Cwd
            && segments.len() == 2
            && !segments[0].contains('*')
            && segments[1] == "**";
        Self {
            anchor,
            segments,
            any_depth,
            single_segment_dir,
        }
    }

    fn base(&self, ctx: &Context<'_>) -> Option<PathBuf> {
        match self.anchor {
            Anchor::Root => Some(PathBuf::from("/")),
            Anchor::Home => ctx.home.map(Path::to_path_buf),
            Anchor::Source => Some(ctx.source.to_path_buf()),
            Anchor::Cwd => Some(ctx.cwd.to_path_buf()),
        }
    }

    /// Matched as named and where it really points (both sides resolved): a
    /// deny applies when *either* matches, an allow only when *both* do.
    fn matches(&self, ctx: &Context<'_>, file: &Path, class: Class) -> bool {
        let named = self.matches_one(ctx, file, class);
        if named == class.is_restrictive() {
            return named;
        }
        let Some(resolve) = ctx.realpath else {
            return named;
        };
        let absolute = absolute(ctx, file);
        let real_file = resolve(&absolute).unwrap_or(absolute);
        let resolved = match self.resolved_base(ctx, resolve) {
            Some((base, skip)) => {
                self.matches_from(&base, &self.segments[skip..], &real_file, class)
            }
            None => self.matches_one(ctx, &real_file, class),
        };
        if class.is_restrictive() {
            named || resolved
        } else {
            named && resolved
        }
    }

    /// The leading literal segments resolved, and how many; `None` if floating
    /// or unchanged.
    fn resolved_base(
        &self,
        ctx: &Context<'_>,
        resolve: fn(&Path) -> Option<PathBuf>,
    ) -> Option<(PathBuf, usize)> {
        if self.any_depth || self.single_segment_dir {
            return None;
        }
        let mut base = self.base(ctx)?;
        let taken = self
            .segments
            .iter()
            .take_while(|s| !s.contains(['*', '?', '[']))
            .count();
        if taken == 0 {
            return None;
        }
        self.segments[..taken].iter().for_each(|s| base.push(s));
        let real = resolve(&base)?;
        (real != base).then_some((real, taken))
    }

    /// Whether this names anything strictly beneath `dir` (recursive commands).
    fn covers_under(&self, ctx: &Context<'_>, dir: &Path) -> bool {
        let Some(base) = self.base(ctx) else {
            return false;
        };
        let dir = normalise(&if dir.is_absolute() {
            dir.to_path_buf()
        } else {
            ctx.cwd.join(dir)
        });
        if self.any_depth {
            // Only the immediate child is checkable without a walk.
            return ctx
                .realpath
                .is_some_and(|resolve| resolve(&dir.join(&self.segments[0])).is_some());
        }
        let mut prefix = base;
        self.segments
            .iter()
            .take_while(|s| !s.contains(['*', '?', '[']))
            .for_each(|s| prefix.push(s));
        let prefix = normalise(&prefix);
        prefix != dir && prefix.starts_with(&dir)
    }

    fn matches_from(&self, base: &Path, segments: &[String], file: &Path, class: Class) -> bool {
        let file = normalise(file);
        let Ok(rel) = file.strip_prefix(normalise(base)) else {
            return false;
        };
        let parts: Vec<&str> = rel
            .to_str()
            .unwrap_or_default()
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        segments_match(segments, &parts, class.is_restrictive())
    }

    fn matches_one(&self, ctx: &Context<'_>, file: &Path, class: Class) -> bool {
        let Some(base) = self.base(ctx) else {
            return false; // `~/…` with no home cannot be evaluated
        };
        let file = absolute(ctx, file);
        let floats = self.any_depth || (self.single_segment_dir && class.is_restrictive());
        if floats {
            let mut pattern = vec!["**".to_string()];
            pattern.extend(self.segments.iter().cloned());
            return self.matches_from(&base, &pattern, &file, class);
        }
        self.matches_from(&base, &self.segments, &file, class)
    }
}

fn absolute(ctx: &Context<'_>, file: &Path) -> PathBuf {
    if let Some(home) = ctx.home
        && let Ok(rest) = file.strip_prefix("~")
    {
        return home.join(rest);
    }
    if file.is_absolute() {
        file.to_path_buf()
    } else {
        ctx.cwd.join(file)
    }
}

/// Whether `file`, resolved against `dir`, stays inside it (`~/x` never does).
pub fn within(dir: &Path, file: &Path) -> bool {
    if file.to_string_lossy().starts_with('~') {
        return false;
    }
    let resolved = if file.is_absolute() {
        file.to_path_buf()
    } else {
        dir.join(file)
    };
    normalise(&resolved).starts_with(normalise(dir))
}

/// Collapses `.`/`..`; a Windows drive becomes Claude Code's POSIX shape.
fn normalise(p: &Path) -> PathBuf {
    let text = p.to_string_lossy().replace('\\', "/");
    let text = match text.as_bytes() {
        [drive, b':', b'/', ..] if drive.is_ascii_alphabetic() => {
            format!("/{}{}", drive.to_ascii_lowercase() as char, &text[2..])
        }
        _ => text,
    };
    let mut out: Vec<&str> = Vec::new();
    for seg in text.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    PathBuf::from(format!("/{}", out.join("/")))
}

/// gitignore segment matching. `glob_aware` (restrictive): the agent's own
/// segment may be a glob the shell will expand.
fn segments_match(pattern: &[String], path: &[&str], glob_aware: bool) -> bool {
    star_match(
        pattern,
        path,
        |p| p == "**",
        |p, t| {
            if glob_aware {
                segments_could_meet(p, t)
            } else {
                wildcard(p, t)
            }
        },
    )
}

/// Linear single-anchor backtracking; a recursive matcher would be exponential
/// on attacker-influenced text in the blocking hook.
fn star_match<P, T: Copy>(
    pattern: &[P],
    text: &[T],
    star: impl Fn(&P) -> bool,
    one: impl Fn(&P, T) -> bool,
) -> bool {
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut anchor, mut resume) = (None::<usize>, 0usize);
    while ti < text.len() {
        if pi < pattern.len() && star(&pattern[pi]) {
            anchor = Some(pi);
            resume = ti;
            pi += 1;
        } else if pi < pattern.len() && one(&pattern[pi], text[ti]) {
            pi += 1;
            ti += 1;
        } else if let Some(s) = anchor {
            pi = s + 1;
            resume += 1;
            ti = resume;
        } else {
            return false;
        }
    }
    while pi < pattern.len() && star(&pattern[pi]) {
        pi += 1;
    }
    pi == pattern.len()
}

/// Whether a rule segment and an agent glob could name the same file.
/// Over-approximates (a miss would cost the prohibition); keeps POSIX's dotfile
/// rule so `Read(.env)` does not refuse `cat *`.
fn segments_could_meet(rule: &str, operand: &str) -> bool {
    if !operand.contains(['*', '?', '[']) {
        return wildcard(rule, operand);
    }
    if rule.starts_with('.') && !operand.starts_with('.') {
        return false;
    }
    globs_intersect(rule, &collapse_brackets(operand))
}

/// Whether any string matches both globs; memoised, O(n·m).
fn globs_intersect(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    const MAX_SEGMENT: usize = 256;
    if a.len() > MAX_SEGMENT || b.len() > MAX_SEGMENT {
        return true;
    }
    let width = b.len() + 1;
    let mut seen = vec![false; (a.len() + 1) * width];
    let mut stack = vec![(0usize, 0usize)];
    while let Some((i, j)) = stack.pop() {
        if std::mem::replace(&mut seen[i * width + j], true) {
            continue;
        }
        if i == a.len() {
            if b[j..].iter().all(|c| *c == '*') {
                return true;
            }
            continue;
        }
        if j == b.len() {
            if a[i..].iter().all(|c| *c == '*') {
                return true;
            }
            continue;
        }
        if a[i] == '*' {
            stack.push((i + 1, j));
            stack.push((i, j + 1));
        } else if b[j] == '*' {
            stack.push((i, j + 1));
            stack.push((i + 1, j));
        } else if a[i] == '?' || b[j] == '?' || a[i] == b[j] {
            stack.push((i + 1, j + 1));
        }
    }
    false
}

/// `[abc]` becomes one `?`; an unterminated `[` stays literal.
fn collapse_brackets(seg: &str) -> String {
    let mut out = String::with_capacity(seg.len());
    let mut rest = seg;
    while let Some(open) = rest.find('[') {
        match rest[open + 1..].find(']') {
            Some(close) => {
                out.push_str(&rest[..open]);
                out.push('?');
                rest = &rest[open + 1 + close + 1..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn wildcard(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    star_match(&p, &t, |c| *c == '*', |p, t| *p == '?' || *p == t)
}

// --- The policy ---

/// The first rule in one list that speaks for a call, unless an exception in
/// that list does too. An exception excuses one simple command, never the
/// line: with `!Bash(git status *)`, `git status && git push` is still refused.
fn first_match<'r>(
    rules: &'r [Rule],
    ctx: &Context<'_>,
    tool: &str,
    input: &Value,
    line: Option<&Line>,
) -> Option<&'r Rule> {
    let speaks = |line: Option<&Line>| {
        let matched = rules
            .iter()
            .filter(|r| !r.negated)
            .find(|r| r.matches_read(ctx, tool, input, line))?;
        let excepted = rules
            .iter()
            .filter(|r| r.negated)
            .any(|r| r.matches_read(ctx, tool, input, line));
        (!excepted).then_some(matched)
    };
    match line {
        Some(l) if l.commands.len() > 1 => l.commands.iter().find_map(|c| {
            speaks(Some(&Line {
                commands: vec![c.clone()],
                barrier: None,
            }))
        }),
        _ => speaks(line),
    }
}

/// An allow rule in the agent's own settings that grants an arbitrary program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overbroad {
    pub rule: String,
    pub why: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Default)]
pub struct Policy {
    deny: Vec<Rule>,
    ask: Vec<Rule>,
    /// `"<file>: <error>"`.
    unloadable: Option<String>,
}

impl Policy {
    pub fn rules(deny: &[String], ask: &[String]) -> Self {
        Self {
            deny: deny
                .iter()
                .filter_map(|r| Rule::parse(r, Class::Deny))
                .collect(),
            ask: ask
                .iter()
                .filter_map(|r| Rule::parse(r, Class::Ask))
                .collect(),
            unloadable: None,
        }
    }

    /// A file that would not load makes every call unresolved: a typo must
    /// never read as no prohibitions, and there are no previous rules to keep.
    pub fn unloadable(why: impl Into<String>) -> Self {
        Self {
            unloadable: Some(why.into()),
            ..Self::default()
        }
    }

    pub fn load_error(&self) -> Option<&str> {
        self.unloadable.as_deref()
    }

    pub fn ask_rules(&self) -> &[Rule] {
        &self.ask
    }

    pub fn deny_rules(&self) -> &[Rule] {
        &self.deny
    }

    /// An unloadable file is not empty.
    pub fn is_empty(&self) -> bool {
        self.deny.is_empty() && self.ask.is_empty() && self.unloadable.is_none()
    }

    /// Whether any rule speaks about `tool`; unreadable input for such a tool
    /// is a question for a person, never silence.
    pub fn speaks_about(&self, tool: &str) -> bool {
        self.unloadable.is_some()
            || self.deny.iter().chain(&self.ask).any(|r| {
                r.tool.matches(tool, r.class)
                    || r.command_tool_applies(tool)
                    || r.path_tool_applies(tool)
                    || (is_shell(tool)
                        && matches!(r.spec, Spec::Path(_))
                        && is_shell_rule_tool(r.tool.as_str()))
            })
    }

    /// Deny, then ask, then unresolved, then nothing — not configurable.
    /// `Unresolved` only where a rule about what may run is in force.
    pub fn restrictive(&self, ctx: &Context<'_>, tool: &str, input: &Value) -> Verdict {
        if let Some(why) = &self.unloadable {
            return Verdict::Unresolved {
                why: format!("{why}; until it loads, a person decides every call"),
            };
        }
        let content = is_command_tool(tool)
            .then(|| rule_content(tool, input))
            .flatten();
        let line = content
            .as_deref()
            .filter(|_| is_shell(tool))
            .map(command::read);
        if let Some(r) = first_match(&self.deny, ctx, tool, input, line.as_ref()) {
            return Verdict::Deny {
                rule: r.raw.clone(),
            };
        }
        if let Some(r) = first_match(&self.ask, ctx, tool, input, line.as_ref()) {
            return Verdict::Ask {
                rule: r.raw.clone(),
            };
        }
        let constraining = self.constraining(tool);
        if content.is_some() && !constraining.is_empty() {
            let mut by = constraining
                .iter()
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join("`, `");
            if constraining.len() > 3 {
                by = format!("{by}` and {} more `", constraining.len() - 3);
            }
            if let Some(why) = line.and_then(|l| l.barrier) {
                return Verdict::Unresolved {
                    why: format!("{why}; `{by}` constrains what {tool} may run"),
                };
            }
            if tool == "PowerShell" {
                return Verdict::Unresolved {
                    why: format!(
                        "PowerShell is read only as literal tokens, and none of them matched; \
                         `{by}` constrains what {tool} may run"
                    ),
                };
            }
        }
        Verdict::Undecided
    }

    /// Non-exception rules about what this shell tool may run or reach.
    fn constraining(&self, tool: &str) -> Vec<&str> {
        self.deny
            .iter()
            .chain(&self.ask)
            .filter(|r| !r.negated)
            .filter(|r| match &r.spec {
                Spec::Command(_) => r.command_tool_applies(tool),
                Spec::Path(_) => is_shell(tool) && is_shell_rule_tool(r.tool.as_str()),
                _ => false,
            })
            .map(|r| r.raw.as_str())
            .collect()
    }

    pub fn problems(&self) -> Vec<(bool, String)> {
        self.deny
            .iter()
            .chain(&self.ask)
            .flat_map(Rule::problems)
            .collect()
    }

    /// Rules an earlier one in the same list covers; advice only.
    pub fn redundancies(&self) -> Vec<String> {
        let mut out = Vec::new();
        for list in [&self.deny, &self.ask] {
            if list.iter().any(Rule::is_negated) {
                continue; // an exception below carves a hole in the rule above
            }
            for (i, rule) in list.iter().enumerate() {
                if let Some(earlier) = list[..i]
                    .iter()
                    .find(|e| e.covers_rule(rule) && e.raw != rule.raw)
                {
                    out.push(format!(
                        "`{}` does nothing: `{}` above it already covers every call it names",
                        rule.raw, earlier.raw
                    ));
                }
            }
        }
        out
    }

    /// Paths a `Read` deny protects from reading but not from replacement by
    /// redirection, which is where the vendor draws the line.
    pub fn half_protected_paths(&self) -> Vec<String> {
        let spelled = |r: &Rule| match &r.spec {
            Spec::Path(_) => r
                .raw
                .split_once('(')
                .map(|(_, s)| s.trim_end_matches(')').to_string()),
            _ => None,
        };
        self.deny
            .iter()
            .filter(|r| r.tool.as_str().eq_ignore_ascii_case("Read"))
            .filter_map(spelled)
            .filter(|path| {
                !self.deny.iter().any(|o| {
                    o.tool.as_str().eq_ignore_ascii_case("Edit")
                        && spelled(o).as_deref() == Some(path)
                })
            })
            .collect()
    }
}

/// Allow rules granting a bare interpreter (`Bash(python:*)` allows any code).
/// Reported, never decided.
pub fn overbroad(rules: &[String]) -> Vec<Overbroad> {
    rules
        .iter()
        .filter_map(|r| Rule::parse(r, Class::Allow))
        .filter(|r| !r.negated)
        .filter_map(|r| match &r.spec {
            Spec::Command(c) if c.operands.as_slice() == ["*"] && c.flags.is_empty() && command::read(&format!("{} x", c.program)).barrier.is_some() => Some(Overbroad {
                rule: r.raw.clone(),
                why: format!("`{}` runs whatever code it is given, so this approves any code at all. Claude Code reads it the same way", c.program),
                suggestion: format!("{}({} <the subcommand you mean> *)", r.tool.as_str(), c.program),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> Context<'static> {
        Context::at(Path::new("/repo"))
    }

    fn policy(deny: &[&str], ask: &[&str]) -> Policy {
        let s = |v: &[&str]| v.iter().map(|r| r.to_string()).collect::<Vec<_>>();
        Policy::rules(&s(deny), &s(ask))
    }

    fn bash(p: &Policy, cmd: &str) -> Verdict {
        p.restrictive(&ctx(), "Bash", &json!({"command": cmd}))
    }

    /// Respellings of a prohibited call; none may be `Undecided`.
    const SPELLINGS: &[&str] = &[
        "rm  -rf /",
        "rm\t-rf /",
        "rm -fr /",
        "rm -r -f /",
        "rm -RF /",
        "RM -rf /",
        "/bin/rm -rf /",
        "sh -lc 'rm -rf /'",
        "bash -ec 'rm -rf /'",
        "bash -c'rm -rf /'",
        "bash x.sh",
        "python x.py",
        "echo / | xargs -0 rm -rf",
        "xargs -I{} rm -rf {}",
        "busybox rm -rf /",
        "exec -a x rm -rf /",
        "env -S 'rm -rf /'",
        "su -c 'rm -rf /'",
        "ssh h rm -rf /",
        "flock /tmp/l rm -rf /",
        "awk 'BEGIN{system(\"rm -rf /\")}'",
        "git -c core.pager='rm -rf /' log",
        "docker run x rm -rf /",
        "node --eval 'require(\"child_process\").execSync(\"rm -rf /\")'",
        "sudo rm -rf /",
        "$(echo rm) -rf /",
        "`echo rm` -rf /",
        "eval 'rm -rf /'",
        "sudo -s",
        "cat <<EOF | sudo bash\nrm -rf /\nEOF",
        "ls && rm -rf /",
        "true; rm -rf /",
        "(rm -rf /)",
        "nohup rm -rf / &",
        "timeout 5 rm -rf /",
        "FOO=1 rm -rf /",
        "\\rm -rf /",
        "r''m -rf /",
        "\"rm\" -rf /",
        "rm -rf -- /",
        "rm -v -rf /",
        "sh <<EOF\nrm -rf /\nEOF",
        "find / -delete",
        "cat x | sh",
        "nice -n 5 rm -rf /",
        "kubectl exec p -- rm -rf /",
        "function f { rm -rf /; }; f",
        "coproc rm -rf /",
        "/bin/r? -rf /",
        "/bin/r[m] -rf /",
        "{rm,-rf,/}",
        "bash <<< 'rm -rf /'",
        "sh < x.sh",
        "builtin rm -rf /",
        "sudo --user root rm -rf /",
        "sudo -R /tmp rm -rf /",
        "env -P /bin rm -rf /",
        "/usr/bin/time -o f rm -rf /",
        "caffeinate -i rm -rf /",
        "arch -arm64 rm -rf /",
        "strace -f -o log rm -rf /",
        "watch rm -rf /",
        "script -qc 'rm -rf /' /dev/null",
        "sudo --made-up-flag x rm -rf /",
    ];

    #[test]
    fn every_measured_spelling_is_denied_or_unresolved_and_never_silent() {
        let p = policy(&["Bash(rm -rf *)"], &[]);
        let mut denied = 0;
        for cmd in SPELLINGS {
            let v = bash(&p, cmd);
            assert!(
                !matches!(v, Verdict::Undecided | Verdict::Ask { .. }),
                "{cmd:?} → {v:?}"
            );
            denied += usize::from(matches!(v, Verdict::Deny { .. }));
        }
        assert!(
            denied >= 25,
            "the token matcher should name the rule for most spellings, not defer them: {denied}"
        );
    }

    #[test]
    fn a_readable_line_that_names_no_prohibited_program_is_undecided() {
        let p = policy(&["Bash(rm -rf *)"], &[]);
        for cmd in [
            "echo \"a; rm -rf x\"",
            "cat <<EOF\nrm -rf /\nEOF",
            "git status",
            "rm -r x",
            "ls -la",
            "cargo test",
            "python --version",
            "echo rm -rf /",
        ] {
            assert_eq!(bash(&p, cmd), Verdict::Undecided, "{cmd:?}");
        }
        assert!(matches!(
            bash(&p, "sh <<EOF\nls\nEOF"),
            Verdict::Unresolved { .. }
        ));
    }

    #[test]
    fn a_line_that_cannot_be_read_is_unresolved_only_where_a_rule_constrains_the_tool() {
        assert!(matches!(
            bash(&policy(&["Bash(rm *)"], &[]), "$(x) y"),
            Verdict::Unresolved { .. }
        ));
        assert!(matches!(
            bash(&policy(&["Read(.env)"], &[]), "$(x) y"),
            Verdict::Unresolved { .. }
        ));
        assert!(matches!(
            bash(&policy(&[], &["Bash(git push *)"]), "sh -c x"),
            Verdict::Unresolved { .. }
        ));
        assert_eq!(
            bash(&policy(&["Read(.env)"], &[]), "cat .env"),
            Verdict::Deny {
                rule: "Read(.env)".into()
            }
        );
        assert_eq!(
            bash(&policy(&["WebFetch(domain:x.y)"], &[]), "$(x) y"),
            Verdict::Undecided
        );
        assert_eq!(bash(&policy(&[], &[]), "$(x) y"), Verdict::Undecided);
        assert_eq!(
            bash(&policy(&["Bash"], &[]), "$(x) y"),
            Verdict::Deny {
                rule: "Bash".into()
            }
        );
    }

    #[test]
    fn deny_beats_ask_beats_unresolved_beats_undecided() {
        let p = policy(&["Bash(rm -rf *)"], &["Bash(git push *)"]);
        assert!(matches!(
            bash(&p, "git push && rm -rf / && $(x)"),
            Verdict::Deny { .. }
        ));
        assert!(matches!(bash(&p, "git push && $(x)"), Verdict::Ask { .. }));
        assert!(matches!(bash(&p, "ls && $(x)"), Verdict::Unresolved { .. }));
        assert_eq!(bash(&p, "ls"), Verdict::Undecided);
        // A deny in the ask list's shadow is still a deny.
        let p = policy(&["Bash(git push *)"], &["Bash(git push *)"]);
        assert!(matches!(bash(&p, "git push"), Verdict::Deny { .. }));
    }

    #[test]
    fn flags_are_a_set_and_operands_a_prefix() {
        let hit = |rule: &str, cmd: &str| {
            matches!(bash(&policy(&[rule], &[]), cmd), Verdict::Deny { .. })
        };
        assert!(hit("Bash(rm -rf *)", "rm -fr x"));
        assert!(hit("Bash(rm -rf *)", "rm -f -r x"));
        assert!(hit("Bash(rm -rf *)", "rm -rfv x"));
        assert!(hit("Bash(rm -rf *)", "rm -rf"));
        assert!(!hit("Bash(rm -rf *)", "rm -r x"));
        assert!(!hit("Bash(rm -rf *)", "rmdir -rf x"));
        assert!(hit("Bash(rm *)", "rm -r x"));
        assert!(hit("Bash(rm *)", "rm"));
        assert!(hit("Bash(git push:*)", "git push origin main"));
        assert!(hit("Bash(git push:*)", "git -C /x push"));
        assert!(!hit("Bash(git push:*)", "git log push"));
        assert!(hit("Bash(git push --force *)", "git push origin --force"));
        assert!(!hit(
            "Bash(git push --force *)",
            "git push origin --force-with-lease=x"
        ));
        assert!(hit("Bash(git status)", "git status"));
        assert!(!hit("Bash(git status)", "git status --short"));
        assert!(hit("Bash(npm run build*)", "npm run build:prod"));
        assert!(hit("Bash(git * main)", "git push origin main"));
        assert!(hit("Bash(ls *)", "ls"));
        assert!(!hit("Bash(pnpm test *)", "pnpm testx"));
        assert!(hit("Bash(cargo test --lib diff)", "cargo test --lib diff"));
    }

    #[test]
    fn a_denied_command_stays_denied_under_any_permutation_of_its_tokens() {
        let p = policy(&["Bash(rm -rf *)"], &[]);
        let tokens = ["-rf", "/", "-v", "x", "sudo", "--force", "-i"];
        // Deterministic shuffle; `rm` and `-rf` always present.
        let mut seed = 0x9e37_79b9u32;
        for _ in 0..500 {
            let mut deck: Vec<&str> = tokens.to_vec();
            for i in (1..deck.len()).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                deck.swap(i, (seed as usize) % (i + 1));
            }
            let n = (seed as usize) % deck.len() + 1;
            let mut words: Vec<&str> = vec!["rm"];
            words.extend(deck[..n].iter().filter(|t| **t != "-rf"));
            let at = (seed as usize) % words.len() + 1;
            words.insert(at, "-rf");
            let cmd = words.join(" ");
            let v = bash(&p, &cmd);
            assert!(matches!(v, Verdict::Deny { .. }), "{cmd:?} → {v:?}");
        }
        // Only the shell's own end-of-options changes what the flags mean.
        assert_eq!(bash(&p, "rm -- -rf x"), Verdict::Undecided);
    }

    #[test]
    fn a_bash_rule_reaches_the_monitor_tool_and_powershell_is_literal() {
        let p = policy(&["Bash(rm *)"], &[]);
        assert!(matches!(
            p.restrictive(&ctx(), "Monitor", &json!({"command": "rm x"})),
            Verdict::Deny { .. }
        ));
        let ps = policy(&["PowerShell(Remove-Item *)"], &[]);
        let run = |c: &str| ps.restrictive(&ctx(), "PowerShell", &json!({"command": c}));
        assert!(matches!(
            run("remove-item -Recurse C:\\x"),
            Verdict::Deny { .. }
        ));
        assert!(matches!(
            run("Get-ChildItem | Remove-Item"),
            Verdict::Deny { .. }
        ));
        assert!(matches!(run("rm -r x"), Verdict::Unresolved { .. }));
        assert_eq!(
            p.restrictive(&ctx(), "PowerShell", &json!({"command": "rm x"})),
            Verdict::Undecided
        );
    }

    #[test]
    fn negations_are_exceptions_scoped_to_their_own_list() {
        let p = policy(&["Bash(git *)", "!Bash(git status *)"], &[]);
        assert!(matches!(bash(&p, "git push"), Verdict::Deny { .. }));
        assert_eq!(bash(&p, "git status -s"), Verdict::Undecided);
        assert!(Rule::parse("!", Class::Deny).is_none());
    }

    #[test]
    fn an_exception_excuses_one_simple_command_and_not_the_line() {
        let p = policy(&["Bash(git *)", "!Bash(git status *)"], &[]);
        for line in [
            "git status && git push --force",
            "git push --force; git status",
            "git status | git push --force",
            "git status -s && git push",
        ] {
            assert!(matches!(bash(&p, line), Verdict::Deny { .. }), "{line}");
        }
        assert_eq!(bash(&p, "git status && git status -s"), Verdict::Undecided);
        assert_eq!(bash(&p, "git status && ls"), Verdict::Undecided);
        let ask = policy(&[], &["Bash(git *)", "!Bash(git status *)"]);
        assert!(matches!(
            bash(&ask, "git status && git push --force"),
            Verdict::Ask { .. }
        ));
    }

    #[test]
    fn an_exact_rule_matches_its_flags_however_they_are_spelled() {
        let p = policy(&["Bash(rm -rf /)"], &[]);
        for line in [
            "rm -rf /",
            "rm -r -f /",
            "rm -fr /",
            "rm -f -r /",
            "RM -Rf /",
        ] {
            assert!(matches!(bash(&p, line), Verdict::Deny { .. }), "{line}");
        }
        // Exact still means exact: a flag more or fewer is another call.
        for line in ["rm -r /", "rm -rfv /", "rm -rf /tmp"] {
            assert_eq!(bash(&p, line), Verdict::Undecided, "{line}");
        }
        // An allow rule keeps its spelling.
        let allow = Rule::parse("Bash(rm -rf /)", Class::Allow).unwrap();
        assert!(!allow.matches(&ctx(), "Bash", &json!({"command": "rm -r -f /"})));
    }

    #[test]
    fn an_exact_rule_is_never_called_redundant_by_a_wider_one_it_is_not_under() {
        let r = |s: &str| Rule::parse(s, Class::Deny).unwrap();
        assert!(!r("Bash(rm)").covers_rule(&r("Bash(rm -f)")));
        assert!(!r("Bash(git push)").covers_rule(&r("Bash(git push origin)")));
        assert!(
            policy(&["Bash(rm)", "Bash(rm -rf /)"], &[])
                .redundancies()
                .is_empty(),
            "a working rule must not be reported as doing nothing"
        );
        // The same call spelled twice is still a duplicate.
        assert!(r("Bash(rm -rf /)").covers_rule(&r("Bash(rm -fr /)")));
    }

    #[test]
    fn a_rule_set_that_would_not_load_asks_about_every_call() {
        let p = Policy::unloadable("/repo/devplane.toml is not valid: unknown field `never_autoo`");
        assert!(!p.is_empty());
        assert!(p.speaks_about("Read"));
        for (tool, input) in [
            ("Bash", json!({"command": "ls"})),
            ("Read", json!({"file_path": "x"})),
            ("WebFetch", json!({"url": "https://x"})),
        ] {
            match p.restrictive(&ctx(), tool, &input) {
                Verdict::Unresolved { why } => assert!(why.contains("devplane.toml"), "{why}"),
                v => panic!("{tool}: {v:?}"),
            }
        }
    }

    #[test]
    fn an_unresolved_line_names_the_rules_that_made_it_a_question() {
        let p = policy(&["Bash(rm -rf *)"], &["Bash(git push *)"]);
        match bash(&p, "$(x)") {
            Verdict::Unresolved { why } => {
                assert!(
                    why.contains("Bash(rm -rf *)") && why.contains("Bash(git push *)"),
                    "{why}"
                )
            }
            v => panic!("{v:?}"),
        }
    }

    #[test]
    fn path_rules_reach_file_tools_and_shell_commands() {
        let p = policy(
            &["Read(.env)", "Edit(devplane.toml)", "Read(~/.ssh/**)"],
            &[],
        );
        let c = Context::at(Path::new("/repo")).with_home(Some(Path::new("/home/me")));
        let read = |path: &str| p.restrictive(&c, "Read", &json!({"file_path": path}));
        assert!(matches!(read("/repo/a/b/.env"), Verdict::Deny { .. }));
        assert!(matches!(read("/home/me/.ssh/id_rsa"), Verdict::Deny { .. }));
        assert_eq!(read("/repo/.envrc"), Verdict::Undecided);
        assert!(
            matches!(
                p.restrictive(&c, "Write", &json!({"file_path": ".env"})),
                Verdict::Deny { .. }
            ),
            "a Read deny stops the overwrite"
        );
        assert!(matches!(
            p.restrictive(&c, "Write", &json!({"file_path": "devplane.toml"})),
            Verdict::Deny { .. }
        ));
        assert!(matches!(
            p.restrictive(&c, "Grep", &json!({"path": "/repo/.env"})),
            Verdict::Deny { .. }
        ));
        for cmd in [
            "cat .env",
            "cat .en?",
            "cat .env*",
            "echo x | tee .env",
            "sudo cat /repo/.env",
            "grep -r x /repo/sub/.env",
            "cat ~/.ssh/id_rsa",
            "echo x > devplane.toml",
            "cp a devplane.toml",
        ] {
            assert!(
                matches!(
                    p.restrictive(&c, "Bash", &json!({"command": cmd})),
                    Verdict::Deny { .. }
                ),
                "{cmd}"
            );
        }
        assert_eq!(
            p.restrictive(&c, "Bash", &json!({"command": "cat *"})),
            Verdict::Undecided,
            "a wildcard does not expand onto a dotfile"
        );
        assert_eq!(
            p.restrictive(&c, "Bash", &json!({"command": "echo x > .env"})),
            Verdict::Undecided,
            "a redirect is Edit business"
        );
        assert_eq!(p.half_protected_paths(), [".env", "~/.ssh/**"]);
    }

    #[test]
    fn the_three_anchors_and_the_floating_forms() {
        let c = Context::at(Path::new("/repo/sub"))
            .with_home(Some(Path::new("/home/me")))
            .with_source(Path::new("/repo"));
        let hit = |rule: &str, file: &str| {
            Rule::parse(rule, Class::Deny)
                .unwrap()
                .matches(&c, "Read", &json!({"file_path": file}))
        };
        assert!(hit("Read(//etc/**)", "/etc/passwd"));
        assert!(hit("Read(~/x)", "/home/me/x"));
        assert!(hit("Read(/secrets/**)", "/repo/secrets/k"));
        assert!(!hit("Read(/secrets/**)", "/repo/sub/secrets/k"));
        assert!(hit("Read(./k)", "/repo/sub/k"));
        assert!(hit("Read(k)", "/repo/sub/deep/k"));
        assert!(
            hit("Read(node_modules/**)", "/repo/sub/a/node_modules/x"),
            "a single directory floats on deny"
        );
        assert!(
            !Rule::parse("Edit(node_modules/**)", Class::Allow)
                .unwrap()
                .matches(
                    &c,
                    "Edit",
                    &json!({"file_path": "/repo/sub/a/node_modules/x"})
                )
        );
        assert!(hit("Read(src/*.rs)", "/repo/sub/src/a.rs"));
        assert!(!hit("Read(src/*.rs)", "/repo/sub/src/x/a.rs"));
        assert!(hit("Read(**/*.rs)", "/repo/sub/src/x/a.rs"));
        assert!(
            hit("Read(k)", "x/../k"),
            "a path that climbs is read where it lands"
        );
        assert!(
            !Rule::parse("Read(~/x)", Class::Deny).unwrap().matches(
                &ctx(),
                "Read",
                &json!({"file_path": "/home/me/x"})
            ),
            "no home, no match"
        );
    }

    fn fake_realpath(p: &Path) -> Option<PathBuf> {
        let s = p.to_string_lossy();
        s.starts_with("/repo/link")
            .then(|| PathBuf::from(s.replacen("/repo/link", "/real", 1)))
    }

    #[test]
    fn a_deny_reaches_a_symlink_from_either_end() {
        let c = Context::at(Path::new("/repo")).with_realpath(fake_realpath);
        let hit = |rule: &str, file: &str| {
            Rule::parse(rule, Class::Deny)
                .unwrap()
                .matches(&c, "Read", &json!({"file_path": file}))
        };
        assert!(
            hit("Read(//real/**)", "/repo/link/secret"),
            "the file is a link into the forbidden tree"
        );
        assert!(
            hit("Read(link/deep/**)", "/real/deep/secret"),
            "the rule names the link and the call the real path"
        );
        assert!(!hit("Read(link/deep/**)", "/real/other"));
        let ok = Rule::parse("Read(//repo/**)", Class::Allow).unwrap();
        assert!(
            !ok.matches(&c, "Read", &json!({"file_path": "/repo/link/x"})),
            "an allow needs both spellings to match"
        );
        assert!(ok.matches(&c, "Read", &json!({"file_path": "/repo/plain/x"})));
    }

    #[test]
    fn other_tool_shapes_still_match() {
        let p = policy(
            &[
                "WebFetch(domain:evil.example)",
                "mcp__github__create_*",
                "mcp__slack",
                "Agent(model:opus)",
                "Skill(deploy)",
                "mcp__*",
            ],
            &[],
        );
        let run = |tool: &str, input: Value| p.restrictive(&ctx(), tool, &input);
        assert!(matches!(
            run("WebFetch", json!({"url": "https://a@evil.example:8/x"})),
            Verdict::Deny { .. }
        ));
        assert_eq!(
            run("WebFetch", json!({"url": "https://good.example"})),
            Verdict::Undecided
        );
        assert!(matches!(
            run("mcp__github__create_issue", json!({})),
            Verdict::Deny { .. }
        ));
        assert!(matches!(
            run("mcp__slack__post", json!({})),
            Verdict::Deny { .. }
        ));
        assert!(matches!(
            run("Agent", json!({"model": "opus"})),
            Verdict::Deny { .. }
        ));
        assert_eq!(run("Agent", json!({})), Verdict::Undecided);
        assert!(
            matches!(run("Skill", json!({"skill": "deploy"})), Verdict::Undecided),
            "a bare specifier on a tool with no content field never matches"
        );
        assert!(matches!(
            run("mcp__other__x", json!({})),
            Verdict::Deny { .. }
        ));
        assert!(!Rule::parse("mcp__*", Class::Allow).unwrap().matches(
            &ctx(),
            "mcp__x__y",
            &json!({})
        ));
    }

    #[test]
    fn problems_name_the_rules_the_gate_cannot_apply() {
        let fatal = |r: &str| {
            Rule::parse(r, Class::Deny)
                .unwrap()
                .problems()
                .iter()
                .any(|(f, _)| *f)
        };
        for r in [
            "Write(src/**)",
            "Glob(x)",
            "NotebookEdit(x)",
            "mcp__github(create_issue)",
            "Bash(command:rm *)",
            "Agent(researcher)",
            "Bash(rm -rf *",
            "Bash(rm *) x",
            "Monitor(npm *)",
            "Cd(/x)",
        ] {
            assert!(fatal(r), "{r}");
        }
        let clean = |r: &str, c: Class| Rule::parse(r, c).unwrap().problems().is_empty();
        for r in [
            "Bash(rm *)",
            "Read(.env)",
            "PowerShell(Remove-Item *)",
            "!Bash(git status *)",
            "mcp__github__get_*",
            "Bash(git push:*)",
            "WebFetch(domain:x)",
            "Agent(model:opus)",
        ] {
            assert!(
                clean(r, Class::Deny),
                "{r}: {:?}",
                Rule::parse(r, Class::Deny).unwrap().problems()
            );
        }
        assert!(clean("Edit(src/**)", Class::Allow));
        let warn = |r: &str| {
            Rule::parse(r, Class::Deny)
                .unwrap()
                .problems()
                .iter()
                .any(|(f, _)| !*f)
        };
        assert!(
            warn("Bash(rm -rf *)"),
            "a flag cluster is order-specific intent"
        );
        assert!(warn("Sotp(x)"), "a typo in a tool name");
        assert!(!warn("Bash(git push --force *)"));
        assert!(!fatal("!Bash(x)"));
        assert!(
            !Rule::parse("!Bash(x)", Class::Allow)
                .unwrap()
                .problems()
                .is_empty()
        );
    }

    #[test]
    fn coverage_and_redundancy_are_decided_on_tokens() {
        let r = |s: &str| Rule::parse(s, Class::Deny).unwrap();
        assert!(r("Bash(rm:*)").covers_rule(&r("Bash(rm -rf:*)")));
        assert!(!r("Bash(rm -rf:*)").covers_rule(&r("Bash(rm:*)")));
        assert!(r("Bash(cargo *)").covers_rule(&r("Bash(cargo test *)")));
        assert!(!r("Bash(cargo test *)").covers_rule(&r("Bash(cargo *)")));
        assert!(r("Bash(git status)").covers_rule(&r("Bash(git status)")));
        assert!(!r("Bash(rm -rf /tmp/?:*)").covers_rule(&r("Bash(rm -rf /tmp/a:*)")));
        assert!(r("Bash").covers_rule(&r("Bash(x)")));
        let p = policy(&["Bash(rm *)", "Bash(rm -rf *)"], &[]);
        assert_eq!(p.redundancies().len(), 1);
        assert!(
            policy(&["Bash(rm *)", "!Bash(rm -i *)", "Bash(rm -rf *)"], &[])
                .redundancies()
                .is_empty()
        );
        assert_eq!(
            overbroad(&[
                "Bash(python:*)".into(),
                "Bash(python -m pytest *)".into(),
                "Bash(ls:*)".into()
            ])
            .len(),
            1
        );
    }

    #[test]
    fn nothing_an_agent_can_write_panics_or_stalls_the_matcher() {
        let p = policy(
            &[
                "Bash(rm -rf *)",
                "Read(.env)",
                "Read(**/secrets/**/*.key)",
                "Bash(git * main)",
            ],
            &[],
        );
        let hostile = [
            "*".repeat(300),
            "a*".repeat(200),
            "?".repeat(300),
            "$(".repeat(500),
            "'".repeat(999),
            "<<".repeat(300),
            "x ".repeat(20_000),
            "\u{0}\u{ff}".repeat(100),
        ];
        let started = std::time::Instant::now();
        for h in &hostile {
            let _ = bash(&p, h);
            let _ = p.restrictive(&ctx(), "Read", &json!({"file_path": h}));
            let _ = bash(&p, &format!("cat {h}"));
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
}

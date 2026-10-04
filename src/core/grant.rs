//! A grant: the person's own rule, written in their `~/.devplane/policy.toml`
//! under one project, that answers *yes* to a call they have answered *yes* to
//! before. The only way an allow exists in Devplane.
//!
//! A [`Grant`] has one way in: [`Grants::parse`], over what the person's file
//! said. There is no `Default`, no `Deserialize`, no public field and no
//! `From`, so no model output, clock, default or repository text can become
//! one. A grant matches more narrowly than the vendor's allow list: readable
//! text only, every simple command on the line, nothing redirected, no
//! argument the shell rewrites. It is consulted only after every prohibition,
//! question and doubt has passed (`PolicyCache::decide`).
//!
//! The offer is derived here too: from the answers a person gave, as the host
//! proved them, to the same call in the same project.

use crate::core::command::{self, ArgKind};
use crate::core::policy::{self, Class, Context, Rule, SpecView};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Identical, consistent answers before the first offer, when the person's
/// file does not say.
pub const OFFER_AFTER: u32 = 5;
/// Further answers before the offer is shown again, when the file does not say.
pub const OFFER_AGAIN_AFTER: u32 = 5;
/// Neither may go lower: a repository-free file still decides how soon an
/// allow is put in front of the person, and once is not *repeatedly*.
pub const LOWEST: u32 = 2;

/// Where the person's grants live, as surfaces name it.
pub const FILE: &str = "~/.devplane/policy.toml";

/// Tools a grant may name: their effects are the project's worktree (a path
/// grant inside the root) or a command the person ran themselves before.
const GRANTABLE: &[&str] = &["Bash", "Read", "Edit"];

/// Programs that write to a forge whatever their arguments.
const FORGES: &[&str] = &["gh", "glab", "hub", "tea"];

/// Programs that run another program or any code they are given: a grant of
/// one with a wildcard approves anything at all.
const RUNS_ANYTHING: &[&str] = &[
    "sh", "bash", "zsh", "dash", "ksh", "fish", "python", "python2", "python3", "perl", "ruby",
    "node", "nodejs", "deno", "bun", "php", "lua", "rscript", "awk", "gawk", "mawk", "nawk", "env",
    "sudo", "doas", "xargs", "nohup", "timeout", "nice", "exec", "eval", "command", "builtin",
    "time", "npx", "pnpx", "bunx", "uvx",
];

/// `[grants]` in the person's file: when an offer is made. Never a grant.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GrantSettings {
    pub offer_after: Option<u32>,
    pub offer_again_after: Option<u32>,
}

/// `[projects."<root>"]` in the person's file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectGrants {
    pub allow: Vec<GrantEntry>,
    pub offer_after: Option<u32>,
    pub offer_again_after: Option<u32>,
}

/// One line of `allow`: the rule, or the rule with an expiry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GrantEntry {
    Rule(String),
    Until { rule: String, until: String },
}

impl GrantEntry {
    pub fn rule(&self) -> &str {
        match self {
            GrantEntry::Rule(r) | GrantEntry::Until { rule: r, .. } => r,
        }
    }
}

/// One rule the person wrote for one project. See the module documentation
/// for why nothing but [`Grants::parse`] can make one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    rule: Rule,
    root: PathBuf,
    until: Option<Timestamp>,
}

impl Serialize for Grant {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Grant", 4)?;
        st.serialize_field("name", &self.name())?;
        st.serialize_field("rule", self.rule())?;
        st.serialize_field("root", &self.root.display().to_string())?;
        st.serialize_field("until", &self.until.map(|t| t.to_string()))?;
        st.end()
    }
}

impl Grant {
    pub fn rule(&self) -> &str {
        self.rule.as_str()
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn until(&self) -> Option<Timestamp> {
        self.until
    }
    /// How the record and every surface name it: `` `Bash(cargo test)` for /r ``.
    pub fn name(&self) -> String {
        name_of(self.rule(), &self.root)
    }
    pub fn is_lapsed(&self, now: Timestamp) -> bool {
        self.until.is_some_and(|u| now >= u)
    }

    /// Whether this grant, alone, answers one call (see [`Grants::matching`]).
    fn covers(&self, ctx: &Context<'_>, tool: &str, input: &Value) -> bool {
        let Some(content) = policy::rule_content(tool, input) else {
            return false;
        };
        if policy::is_shell(tool) {
            return self.rule.tool_name().eq_ignore_ascii_case("Bash")
                && line_is_plain(&content)
                && command::read(&content)
                    .commands
                    .iter()
                    .all(|c| self.rule.allows_command(c));
        }
        file_inside(ctx, &self.root, &content)
            && !policy::is_command_tool(tool)
            && matches!(self.rule.spec_view(), SpecView::Path(_))
            && self.rule.matches(ctx, tool, input)
    }
}

/// The name of a grant that may no longer exist, for the record.
pub fn name_of(rule: &str, root: &Path) -> String {
    format!("`{rule}` for {}", root.display())
}

/// Whether a shell line is one a grant may read as an approval: readable,
/// nothing rewritten by the shell, nothing redirected to a file, no wrapper
/// or assignment before a program, and every program named bare.
fn line_is_plain(content: &str) -> bool {
    let line = command::read(content);
    if line.barrier.is_some() || line.commands.is_empty() {
        return false;
    }
    // The reader strips wrappers (`sudo`) and assignments (`X=1`); an
    // approval may not ride through them, so each command's first word must
    // be its program, as written.
    let firsts: Vec<&str> = content
        .split(['\n', ';', '|', '&'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.split_whitespace().next())
        .collect();
    if firsts.len() != line.commands.len() {
        return false;
    }
    line.commands.iter().zip(firsts).all(|(c, first)| {
        !c.by_path
            && first.eq_ignore_ascii_case(&c.program)
            && c.kinds.iter().all(|k| *k == ArgKind::Literal)
            && c.redirects
                .iter()
                .all(|r| r.target.starts_with('&') || r.target == "/dev/null")
    })
}

/// Whether `file`, as the call names it, is inside the project root once
/// resolved against the call's directory.
fn file_inside(ctx: &Context<'_>, root: &Path, file: &str) -> bool {
    if file.is_empty() || file.starts_with('~') || file.contains('$') {
        return false;
    }
    let p = Path::new(file);
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        ctx.cwd.join(p)
    };
    if abs
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    let real = ctx.realpath.and_then(|f| f(&abs)).unwrap_or(abs);
    policy::within(root, &real)
}

/// Why a rule cannot be a grant, or `None` when it can. Every refusal names
/// the rule; the same check runs when the file loads and before an offer.
pub fn refusal(raw: &str) -> Option<String> {
    let Some(rule) = Rule::parse(raw, Class::Allow) else {
        return Some("an empty rule grants nothing".into());
    };
    let tool = rule.tool_name();
    if rule.is_negated() {
        return Some(format!(
            "`{raw}` is an exception, and a grant names what is allowed, never what is not"
        ));
    }
    if rule.is_malformed() {
        return Some(format!("`{raw}` is missing its closing bracket"));
    }
    if !GRANTABLE.iter().any(|t| t.eq_ignore_ascii_case(tool)) || tool.contains('*') {
        return Some(format!(
            "`{raw}` names `{tool}`. A grant may name only `Bash`, `Read` or `Edit`: their \
             writes are the project's own files, and Devplane cannot say what another tool \
             touches"
        ));
    }
    match rule.spec_view() {
        SpecView::Any => Some(format!(
            "`{raw}` has a wildcard where the command goes, so it would approve anything \
             `{tool}` can do. A grant is as narrow as the answers it came from"
        )),
        SpecView::Command {
            program, operands, ..
        } => {
            if !tool.eq_ignore_ascii_case("Bash") {
                return Some(format!("`{raw}` gives a file tool a command"));
            }
            if program.is_empty() || program.contains(['*', '?']) {
                return Some(format!(
                    "`{raw}` has a wildcard where the command goes, so it would approve any \
                     program at all"
                ));
            }
            if FORGES.contains(&program)
                || (program == "git"
                    && (operands.is_empty()
                        || operands[0].contains(['*', '?'])
                        || operands.iter().any(|o| o == "push")))
            {
                return Some(format!(
                    "`{raw}` is a forge write (or could be one), and a grant never authorises \
                     one: pushing and opening pull requests stay a person's act"
                ));
            }
            if RUNS_ANYTHING.contains(&program) {
                return Some(format!(
                    "`{raw}`: `{program}` runs whatever code or program it is given, so a grant \
                     of it approves anything at all"
                ));
            }
            None
        }
        SpecView::Path(p) => {
            if !tool.eq_ignore_ascii_case("Read") && !tool.eq_ignore_ascii_case("Edit") {
                return Some(format!("`{raw}` gives a command tool a path"));
            }
            if p.starts_with("//")
                || p.starts_with('~')
                || p.contains("..")
                || p.contains('$')
                || p.chars().nth(1) == Some(':')
            {
                return Some(format!(
                    "`{raw}` reaches outside the project. A path grant is relative to the \
                     project's root and stays inside it"
                ));
            }
            None
        }
        SpecView::Other => Some(format!(
            "`{raw}` is not a command or a path, so there is nothing a grant could name"
        )),
    }
}

/// The person's grants, by project, as their file said them.
#[derive(Debug, Clone, Default)]
pub struct Grants {
    by_root: BTreeMap<PathBuf, Vec<Grant>>,
    thresholds: BTreeMap<PathBuf, (u32, u32)>,
    default_thresholds: (u32, u32),
}

fn threshold(where_: &str, key: &str, v: Option<u32>) -> Result<Option<u32>, String> {
    match v {
        Some(n) if n < LOWEST => Err(format!(
            "`{where_} {key} = {n}` is below {LOWEST}: a grant is offered only after the same \
             answer more than once"
        )),
        other => Ok(other),
    }
}

/// `2026-12-01` (the start of that day, UTC) or a full timestamp.
fn parse_until(s: &str) -> Option<Timestamp> {
    let s = s.trim();
    if let Ok(t) = s.parse::<Timestamp>() {
        return Some(t);
    }
    let d: jiff::civil::Date = s.parse().ok()?;
    d.to_zoned(jiff::tz::TimeZone::UTC)
        .ok()
        .map(|z| z.timestamp())
}

impl Grants {
    /// The one constructor of a [`Grant`], over what the person's own file
    /// said. The first rule that cannot be a grant is the error, by name.
    pub fn parse(
        settings: &GrantSettings,
        projects: &BTreeMap<String, ProjectGrants>,
    ) -> Result<Self, String> {
        let default_thresholds = (
            threshold("[grants]", "offer_after", settings.offer_after)?.unwrap_or(OFFER_AFTER),
            threshold("[grants]", "offer_again_after", settings.offer_again_after)?
                .unwrap_or(OFFER_AGAIN_AFTER),
        );
        let mut out = Grants {
            default_thresholds,
            ..Grants::default()
        };
        for (root, p) in projects {
            let table = format!("[projects.\"{root}\"]");
            let path = PathBuf::from(root);
            if !path.is_absolute() {
                return Err(format!(
                    "`{table}` is not an absolute path; a grant is keyed by the project's root"
                ));
            }
            let t = (
                threshold(&table, "offer_after", p.offer_after)?.unwrap_or(default_thresholds.0),
                threshold(&table, "offer_again_after", p.offer_again_after)?
                    .unwrap_or(default_thresholds.1),
            );
            out.thresholds.insert(path.clone(), t);
            let mut list = Vec::new();
            for e in &p.allow {
                if let Some(why) = refusal(e.rule()) {
                    return Err(format!("{table} allow: {why}"));
                }
                let until = match e {
                    GrantEntry::Rule(_) => None,
                    GrantEntry::Until { until, rule } => {
                        Some(parse_until(until).ok_or_else(|| {
                            format!(
                                "{table} allow: `{rule}` has `until = \"{until}\"`, which is not a \
                             date like \"2026-12-01\" or a timestamp"
                            )
                        })?)
                    }
                };
                let rule = Rule::parse(e.rule(), Class::Allow)
                    .ok_or_else(|| format!("{table} allow: an empty rule grants nothing"))?;
                list.push(Grant {
                    rule,
                    root: path.clone(),
                    until,
                });
            }
            out.by_root.insert(path, list);
        }
        Ok(out)
    }

    /// Every grant, by project.
    pub fn all(&self) -> impl Iterator<Item = &Grant> {
        self.by_root.values().flatten()
    }

    pub fn in_project(&self, root: &Path) -> &[Grant] {
        self.by_root
            .get(root)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Whether the person's file has a table for this project.
    pub fn has_project(&self, root: &Path) -> bool {
        self.by_root.contains_key(root)
    }

    pub fn is_empty(&self) -> bool {
        self.by_root.values().all(Vec::is_empty)
    }

    /// `(offer_after, offer_again_after)` for a project.
    pub fn thresholds(&self, root: &Path) -> (u32, u32) {
        self.thresholds
            .get(root)
            .copied()
            .unwrap_or(self.default_thresholds)
    }

    /// The grant that answers this call in the project at `root`, if one does:
    /// readable input only; on a shell line, every simple command matched by
    /// one grant of the project, nothing rewritten and nothing redirected; a
    /// path grant only for a file inside the root. A lapsed grant never does.
    pub fn matching(
        &self,
        ctx: &Context<'_>,
        root: &Path,
        tool: &str,
        input: &Value,
        now: Timestamp,
    ) -> Option<&Grant> {
        let live: Vec<&Grant> = self
            .in_project(root)
            .iter()
            .filter(|g| !g.is_lapsed(now))
            .collect();
        self.pick(ctx, tool, input, &live)
    }

    /// A lapsed grant that would have answered this call, so the person is
    /// told it lapsed rather than silently asked again.
    pub fn lapsed_matching(
        &self,
        ctx: &Context<'_>,
        root: &Path,
        tool: &str,
        input: &Value,
        now: Timestamp,
    ) -> Option<&Grant> {
        let lapsed: Vec<&Grant> = self
            .in_project(root)
            .iter()
            .filter(|g| g.is_lapsed(now))
            .collect();
        self.pick(ctx, tool, input, &lapsed)
    }

    fn pick<'g>(
        &self,
        ctx: &Context<'_>,
        tool: &str,
        input: &Value,
        among: &[&'g Grant],
    ) -> Option<&'g Grant> {
        let content = policy::rule_content(tool, input)?;
        if policy::is_shell(tool) {
            if !line_is_plain(&content) {
                return None;
            }
            // Every command must be granted; the first grant is the one named.
            let line = command::read(&content);
            let mut named = None;
            for c in &line.commands {
                let g = among.iter().find(|g| {
                    g.rule.tool_name().eq_ignore_ascii_case("Bash") && g.rule.allows_command(c)
                })?;
                named.get_or_insert(*g);
            }
            return named;
        }
        among.iter().copied().find(|g| g.covers(ctx, tool, input))
    }
}

// ── The offer ────────────────────────────────────────────────────────────────

/// One answer a person gave to a permission, as the host proved where it came
/// from (`window` or `notification` only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub ask: String,
    pub at: Timestamp,
    pub from: String,
    pub tool: String,
    pub content: String,
    /// `Some(true)` an allow, `Some(false)` a refusal, `None` neither.
    pub allowed: Option<bool>,
}

impl Answered {
    /// A permission ask a person answered, as the offer counts it; `None` for
    /// a row with no call or no answer.
    pub fn from_ask(ask: &crate::core::ask::Ask) -> Option<Self> {
        let p = &ask.payload;
        let (tool, input) = match p.get("call") {
            Some(Value::Object(c)) => (c.get("tool")?.as_str()?, c.get("input")?),
            _ => (p.get("tool")?.as_str()?, p.get("input")?),
        };
        let content = policy::rule_content(tool, input)?;
        let chose = ask.answer.as_ref()?.get("permission")?.as_str()?;
        let allowed = match chose {
            "allow" => Some(true),
            "deny" => Some(false),
            id => p
                .get("options")
                .and_then(Value::as_array)
                .and_then(|os| {
                    os.iter()
                        .find(|o| o.get("id").and_then(Value::as_str) == Some(id))
                })
                .and_then(|o| o.get("kind").and_then(Value::as_str))
                .and_then(|k| {
                    if k.starts_with("allow") {
                        Some(true)
                    } else if k.starts_with("reject") {
                        Some(false)
                    } else {
                        None
                    }
                }),
        };
        Some(Answered {
            ask: ask.id.as_str().to_string(),
            at: ask.answered_at.unwrap_or(ask.asked_at),
            from: ask.answered_from.clone()?,
            tool: tool.to_string(),
            content,
            allowed,
        })
    }
}

/// One answer an offer derives from, as a surface lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsweredRow {
    pub ask: String,
    pub at: String,
    pub from: String,
}

/// One line of the change to the person's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    /// `" "` context, `"+"` added, `"-"` removed.
    pub op: String,
    pub text: String,
}

/// A grant offered on a permission item: the exact rule, what it derives
/// from, what it may write, and the text it adds to the person's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantOffer {
    pub rule: String,
    pub root: String,
    pub file: String,
    /// How many identical answers allowed it.
    pub allowed: u32,
    pub derived_from: Vec<AnsweredRow>,
    /// What the project declares the command may write.
    pub writes: Vec<String>,
    pub writes_says: String,
    /// Broader than the agent's own allow list.
    pub widens: bool,
    pub diff: Vec<DiffLine>,
}

/// Why no grant is offered. Only some are said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoGrant {
    BelowThreshold { allowed: u32, needed: u32 },
    NotThisTime,
    Inconsistent { allowed: u32, denied: u32 },
    Ungrantable { why: String },
    AlreadyGranted,
    NoCall,
}

impl NoGrant {
    /// The sentence a permission item shows, or `None` where silence is right.
    pub fn says(&self) -> Option<String> {
        match self {
            NoGrant::Inconsistent { allowed, denied } => Some(format!(
                "No grant is offered: you allowed this {allowed} {} and refused it {denied} {}, \
                 so there is no one answer to write down.",
                times(*allowed),
                times(*denied)
            )),
            NoGrant::Ungrantable { why } => Some(format!("No grant can be offered: {why}.")),
            _ => None,
        }
    }
}

fn times(n: u32) -> &'static str {
    if n == 1 { "time" } else { "times" }
}

/// What the offer needs besides the answers.
#[derive(Clone)]
pub struct OfferContext<'a> {
    pub root: &'a Path,
    pub thresholds: (u32, u32),
    /// The `[workspace] share` caches the project declares.
    pub shared: &'a [String],
    /// Whether the person's file already has a `[projects."<root>"]` table.
    pub has_table: bool,
    /// Rules the project already grants.
    pub granted: Vec<&'a str>,
    /// Whether the agent's own allow list already covers the rule.
    pub agent_allows: &'a dyn Fn(&str) -> bool,
}

/// The rule a grant for this call would be: the call's own exact rule.
pub fn rule_for(tool: &str, content: &str) -> Result<String, NoGrant> {
    if policy::is_shell(tool) {
        let line = command::read(content);
        if line.barrier.is_some() || line.commands.len() != 1 || !line_is_plain(content) {
            return Err(NoGrant::Ungrantable {
                why: "only a single plain command can be granted — this one is several \
                      commands, or the shell rewrites part of it"
                    .into(),
            });
        }
        return Ok(format!("Bash({})", content.trim()));
    }
    if tool.eq_ignore_ascii_case("Read") || tool.eq_ignore_ascii_case("Edit") {
        return Ok(format!("{tool}({content})"));
    }
    Err(NoGrant::Ungrantable {
        why: format!("`{tool}` calls are not grantable; only shell commands and project files"),
    })
}

/// The offer for this call, from the person's proved answers to the same call.
pub fn offer(
    tool: &str,
    content: &str,
    history: &[Answered],
    cx: &OfferContext<'_>,
) -> Result<GrantOffer, NoGrant> {
    let same: Vec<&Answered> = history
        .iter()
        .filter(|a| a.tool.eq_ignore_ascii_case(tool) && a.content.trim() == content.trim())
        .filter(|a| a.from == "window" || a.from == "notification")
        .collect();
    let allowed: Vec<&&Answered> = same.iter().filter(|a| a.allowed == Some(true)).collect();
    let denied = same.iter().filter(|a| a.allowed == Some(false)).count() as u32;
    let n = allowed.len() as u32;
    let (after, again) = cx.thresholds;
    if n < after {
        return Err(NoGrant::BelowThreshold {
            allowed: n,
            needed: after,
        });
    }
    if denied > 0 {
        return Err(NoGrant::Inconsistent { allowed: n, denied });
    }
    let rule = rule_for(tool, content)?;
    if let Some(why) = refusal(&rule) {
        return Err(NoGrant::Ungrantable { why });
    }
    if cx.granted.contains(&rule.as_str()) {
        return Err(NoGrant::AlreadyGranted);
    }
    if !(n - after).is_multiple_of(again.max(1)) {
        return Err(NoGrant::NotThisTime);
    }
    let root = cx.root.display().to_string();
    let mut writes = vec!["the change's worktree".to_string()];
    writes.extend(cx.shared.iter().map(|c| format!("the shared {c} cache")));
    let mut derived: Vec<AnsweredRow> = allowed
        .iter()
        .map(|a| AnsweredRow {
            ask: a.ask.clone(),
            at: a.at.to_string(),
            from: a.from.clone(),
        })
        .collect();
    derived.sort_by(|a, b| a.at.cmp(&b.at));
    Ok(GrantOffer {
        diff: diff(&root, &rule, cx.has_table),
        widens: !(cx.agent_allows)(&rule),
        rule,
        root,
        file: FILE.into(),
        allowed: n,
        derived_from: derived,
        writes,
        writes_says: "Devplane does not know what this command itself touches; it knows only \
                      what the project declares"
            .into(),
    })
}

/// The lines a grant adds to the person's file.
pub fn diff(root: &str, rule: &str, has_table: bool) -> Vec<DiffLine> {
    let line = |op: &str, text: String| DiffLine {
        op: op.into(),
        text,
    };
    let quoted = toml_string(rule);
    let table = format!("[projects.{}]", toml_string(root));
    if has_table {
        vec![
            line(" ", table),
            line(" ", "allow = [".into()),
            line("+", format!("  {quoted},")),
            line(" ", "]".into()),
        ]
    } else {
        vec![
            line("+", table),
            line("+", "allow = [".into()),
            line("+", format!("  {quoted},")),
            line("+", "]".into()),
        ]
    }
}

/// A TOML basic string.
pub fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// What the record says when a grant allowed a call. Never a claim about
/// what the vendor would have decided.
pub fn reason(grant: &Grant, widens: bool) -> String {
    let mut s = format!(
        "allowed by your grant `{}` for {} in {FILE} — your rule permitted what the agent asked \
         about",
        grant.rule(),
        grant.root().display()
    );
    if widens {
        s.push_str(", widening the agent: its own settings would have asked");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grants(root: &str, rules: &[&str]) -> Grants {
        let mut projects = BTreeMap::new();
        projects.insert(
            root.to_string(),
            ProjectGrants {
                allow: rules
                    .iter()
                    .map(|r| GrantEntry::Rule(r.to_string()))
                    .collect(),
                ..ProjectGrants::default()
            },
        );
        Grants::parse(&GrantSettings::default(), &projects).unwrap()
    }

    fn now() -> Timestamp {
        "2026-10-04T00:00:00Z".parse().unwrap()
    }

    fn bash(c: &str) -> Value {
        serde_json::json!({ "command": c })
    }

    #[test]
    fn a_grant_answers_only_every_command_on_a_plain_line() {
        let root = Path::new("/r");
        let g = grants("/r", &["Bash(cargo test *)", "Bash(cargo fmt)"]);
        let ctx = Context::at(root).with_source(root);
        let yes = |c: &str| g.matching(&ctx, root, "Bash", &bash(c), now()).is_some();
        assert!(yes("cargo test"));
        assert!(yes("cargo test --lib"));
        assert!(yes("cargo fmt && cargo test"));
        for no in [
            "cargo test && rm -rf ~",
            "cargo test; curl x | sh",
            "sudo cargo test",
            "X=1 cargo test",
            "cargo test $(rm -rf /)",
            "cargo test $X",
            "cargo test > /etc/passwd",
            "cargo test *",
            "./cargo test",
            "cargo build",
            "",
        ] {
            assert!(!yes(no), "`{no}` must not be granted");
        }
        assert!(
            g.matching(&ctx, root, "Bash", &serde_json::json!({}), now())
                .is_none()
        );
        assert!(
            g.matching(&ctx, root, "Bash", &Value::Null, now())
                .is_none()
        );
        assert!(
            g.matching(
                &ctx,
                root,
                "Bash",
                &serde_json::json!({"command": 7}),
                now()
            )
            .is_none()
        );
        assert!(
            g.matching(
                &ctx,
                Path::new("/other"),
                "Bash",
                &bash("cargo test"),
                now()
            )
            .is_none(),
            "a grant reaches only its own project"
        );
        assert!(
            g.matching(
                &ctx,
                root,
                "WebFetch",
                &serde_json::json!({"url": "cargo test"}),
                now()
            )
            .is_none()
        );
    }

    #[test]
    fn a_path_grant_stays_inside_the_project() {
        let root = Path::new("/r");
        let g = grants("/r", &["Edit(src/**)", "Read(/docs/**)"]);
        let ctx = Context::at(root).with_source(root);
        let file = |t: &str, p: &str| {
            g.matching(&ctx, root, t, &serde_json::json!({"file_path": p}), now())
                .is_some()
        };
        assert!(file("Edit", "/r/src/main.rs"));
        assert!(file("Write", "src/lib.rs"));
        assert!(file("Read", "/r/docs/a.md"));
        assert!(!file("Edit", "/r/../etc/passwd"));
        assert!(!file("Edit", "/elsewhere/src/x"));
        assert!(!file("Edit", "~/.devplane/policy.toml"));
        assert!(!file("Bash", "/r/src/main.rs"));
    }

    #[test]
    fn a_lapsed_grant_answers_nothing_and_is_found_as_lapsed() {
        let mut projects = BTreeMap::new();
        projects.insert(
            "/r".to_string(),
            ProjectGrants {
                allow: vec![GrantEntry::Until {
                    rule: "Bash(cargo test)".into(),
                    until: "2026-10-01".into(),
                }],
                ..ProjectGrants::default()
            },
        );
        let g = Grants::parse(&GrantSettings::default(), &projects).unwrap();
        let root = Path::new("/r");
        let ctx = Context::at(root);
        assert!(
            g.matching(&ctx, root, "Bash", &bash("cargo test"), now())
                .is_none()
        );
        assert!(
            g.lapsed_matching(&ctx, root, "Bash", &bash("cargo test"), now())
                .is_some()
        );
    }

    #[test]
    fn every_refusal_names_the_rule() {
        for (rule, says) in [
            ("Bash", "wildcard where the command goes"),
            ("Bash(*)", "wildcard where the command goes"),
            ("Bash(* test)", "wildcard where the command goes"),
            ("Bash(python *)", "runs whatever"),
            ("Bash(bash -c *)", "runs whatever"),
            ("Bash(sudo ls)", "runs whatever"),
            ("Bash(gh pr create *)", "forge write"),
            ("Bash(git push)", "forge write"),
            ("Bash(git *)", "forge write"),
            ("Bash(git)", "forge write"),
            ("WebFetch(domain:x.com)", "may name only"),
            ("mcp__github__create_pull_request", "may name only"),
            ("Edit(//etc/**)", "outside the project"),
            ("Edit(~/x)", "outside the project"),
            ("Read(../x)", "outside the project"),
            ("!Bash(ls)", "exception"),
            ("Bash(ls", "closing bracket"),
        ] {
            let why = refusal(rule).unwrap_or_else(|| panic!("`{rule}` was accepted"));
            assert!(why.contains(says), "`{rule}`: {why}");
        }
        for ok in [
            "Bash(cargo test)",
            "Bash(git status)",
            "Bash(cargo test *)",
            "Edit(src/**)",
        ] {
            assert_eq!(refusal(ok), None, "{ok}");
        }
    }

    #[test]
    fn thresholds_are_the_persons_and_never_below_two() {
        let mut projects = BTreeMap::new();
        projects.insert(
            "/r".to_string(),
            ProjectGrants {
                offer_after: Some(3),
                ..ProjectGrants::default()
            },
        );
        let g = Grants::parse(&GrantSettings::default(), &projects).unwrap();
        assert_eq!(g.thresholds(Path::new("/r")), (3, OFFER_AGAIN_AFTER));
        assert_eq!(
            g.thresholds(Path::new("/x")),
            (OFFER_AFTER, OFFER_AGAIN_AFTER)
        );
        let low = GrantSettings {
            offer_after: Some(1),
            offer_again_after: None,
        };
        let e = Grants::parse(&low, &BTreeMap::new()).unwrap_err();
        assert!(
            e.contains("[grants] offer_after = 1") && e.contains("below 2"),
            "{e}"
        );
        projects.get_mut("/r").unwrap().offer_again_after = Some(0);
        let e = Grants::parse(&GrantSettings::default(), &projects).unwrap_err();
        assert!(e.contains("offer_again_after = 0"), "{e}");
    }

    fn answered(n: usize, allowed: Option<bool>, from: &str, content: &str) -> Vec<Answered> {
        (0..n)
            .map(|i| Answered {
                ask: format!("a{i}{from}"),
                at: now(),
                from: from.into(),
                tool: "Bash".into(),
                content: content.into(),
                allowed,
            })
            .collect()
    }

    fn cx<'a>(root: &'a Path, granted: Vec<&'a str>) -> OfferContext<'a> {
        OfferContext {
            root,
            thresholds: (5, 5),
            shared: &[],
            has_table: false,
            granted,
            agent_allows: &|_| false,
        }
    }

    #[test]
    fn an_offer_follows_repeated_identical_proved_answers() {
        let root = Path::new("/r");
        let h = answered(5, Some(true), "window", "cargo test");
        let o = offer("Bash", "cargo test", &h, &cx(root, vec![])).unwrap();
        assert_eq!(o.rule, "Bash(cargo test)");
        assert_eq!(o.allowed, 5);
        assert_eq!(o.derived_from.len(), 5);
        assert!(o.widens);
        assert!(
            o.writes_says
                .contains("does not know what this command itself touches")
        );
        assert_eq!(o.diff[0].text, "[projects.\"/r\"]");
        assert!(
            o.diff
                .iter()
                .any(|l| l.op == "+" && l.text == "  \"Bash(cargo test)\",")
        );

        // Answers from the command line are any token holder's: never counted.
        let cli = answered(9, Some(true), "cli", "cargo test");
        assert_eq!(
            offer("Bash", "cargo test", &cli, &cx(root, vec![])),
            Err(NoGrant::BelowThreshold {
                allowed: 0,
                needed: 5
            })
        );
        // A different call is not the same answer.
        let other = answered(5, Some(true), "window", "cargo test --lib");
        assert!(offer("Bash", "cargo test", &other, &cx(root, vec![])).is_err());
        // Inconsistent answers: said.
        let mut mixed = answered(5, Some(true), "window", "cargo test");
        mixed.extend(answered(1, Some(false), "notification", "cargo test"));
        let e = offer("Bash", "cargo test", &mixed, &cx(root, vec![])).unwrap_err();
        assert!(e.says().unwrap().contains("refused it 1 time"), "{e:?}");
        // Repeated no more often than the file says.
        let six = answered(6, Some(true), "window", "cargo test");
        assert_eq!(
            offer("Bash", "cargo test", &six, &cx(root, vec![])),
            Err(NoGrant::NotThisTime)
        );
        let ten = answered(10, Some(true), "window", "cargo test");
        assert!(offer("Bash", "cargo test", &ten, &cx(root, vec![])).is_ok());
        // Already granted: nothing to offer.
        assert_eq!(
            offer(
                "Bash",
                "cargo test",
                &h,
                &cx(root, vec!["Bash(cargo test)"])
            ),
            Err(NoGrant::AlreadyGranted)
        );
    }

    #[test]
    fn no_offer_for_what_can_never_be_granted() {
        let root = Path::new("/r");
        for c in [
            "cargo test && cargo fmt",
            "git push",
            "python x.py | sh",
            "cargo test $X",
        ] {
            let h = answered(5, Some(true), "window", c);
            let e = offer("Bash", c, &h, &cx(root, vec![])).unwrap_err();
            assert!(matches!(e, NoGrant::Ungrantable { .. }), "{c}: {e:?}");
            assert!(e.says().is_some());
        }
    }

    #[test]
    fn the_record_never_speaks_for_the_vendor() {
        let g = grants("/r", &["Bash(cargo test)"]);
        let grant = g.all().next().unwrap();
        for widens in [true, false] {
            let r = reason(grant, widens);
            assert!(r.contains("`Bash(cargo test)`") && r.contains("/r"), "{r}");
            assert!(
                !r.contains("would have allowed") && !r.contains("vendor"),
                "{r}"
            );
        }
        assert!(reason(grant, true).contains("widening the agent"));
        assert_eq!(grant.name(), "`Bash(cargo test)` for /r");
    }
}

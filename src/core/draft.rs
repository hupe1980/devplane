//! Drafting a specification by the project's own methodology: the pure half.
//!
//! A draft is a change started with intent rather than a prompt. What is sent
//! to the agent is the methodology's own invocation followed by the person's
//! words, and nothing Devplane composed. Every function here is a function of
//! what a file said, passed in as a value: which spelling the integration
//! record selects, which folder number is free, which span of a line a
//! bracketed answer replaces. Nothing here writes a word of a specification.

use crate::core::ids::RunId;
use serde::{Deserialize, Serialize};

/// The two methodologies a draft dispatches. GSD and Kiro are detected but
/// not drafted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Methodology {
    #[serde(rename = "speckit")]
    SpecKit,
    #[serde(rename = "openspec")]
    OpenSpec,
}

impl Methodology {
    /// The spelling on the wire and on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            Methodology::SpecKit => "speckit",
            Methodology::OpenSpec => "openspec",
        }
    }

    /// The name the dialect reader gives the same layout.
    pub fn dialect(self) -> &'static str {
        match self {
            Methodology::SpecKit => "Spec Kit",
            Methodology::OpenSpec => "OpenSpec",
        }
    }

    /// The methodology spelled `s`, if any.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "speckit" | "spec-kit" | "spec kit" => Some(Methodology::SpecKit),
            "openspec" => Some(Methodology::OpenSpec),
            _ => None,
        }
    }
}

/// Where a draft got to. Computed from the run and the tree, then the
/// person's decision; never a model's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftOutcome {
    /// The agent is working, or has not ended its turn.
    Pending,
    /// The run ended and the folder the methodology writes exists.
    Produced,
    /// The run ended and there is no folder where the methodology puts one.
    NoneProduced,
    /// A person accepted it: the folder is the change's specification.
    Accepted,
    /// A person rejected it: the worktree was put back.
    Rejected,
}

/// One draft dispatch: what was sent, by whom, where it should land.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub methodology: Methodology,
    /// The Devplane agent id the person chose.
    pub agent: String,
    /// The person's words, verbatim.
    pub intent: String,
    /// Exactly what was sent: the invocation, one space, the intent.
    pub sent: String,
    /// Where the methodology writes: the Spec Kit folder Devplane assigned,
    /// or OpenSpec's change root until the run says which change it made.
    pub folder: String,
    /// The worktree's commit at dispatch; what a rejection restores.
    #[serde(default)]
    pub base_commit: Option<String>,
    /// Spec Kit's version from its integration record, kept so the change can
    /// say when it predates the setting that stops it persisting a feature.
    #[serde(default)]
    pub speckit_version: Option<String>,
    /// The OpenSpec change folders present at dispatch, so the one the run
    /// made can be told from the ones it found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub found: Vec<String>,
    /// The run that drafted it.
    #[serde(default)]
    pub run: Option<RunId>,
    pub outcome: DraftOutcome,
}

impl Draft {
    /// Whether the draft still stands for the change: a second draft is
    /// refused unless this one was rejected.
    pub fn stands(&self) -> bool {
        self.outcome != DraftOutcome::Rejected
    }

    /// Whether the review treats [`Draft::folder`] as the one thing asked for.
    pub fn asks_for_folder(&self) -> bool {
        matches!(
            self.outcome,
            DraftOutcome::Pending | DraftOutcome::Produced | DraftOutcome::NoneProduced
        )
    }

    /// The sentences the change carries about this draft, composed here so
    /// every surface says the same thing.
    pub fn says(&self) -> Vec<String> {
        let mut out = Vec::new();
        match self.outcome {
            DraftOutcome::Pending => out.push(format!(
                "drafting by {}'s own command; nothing is checked until a person accepts it",
                self.methodology.dialect()
            )),
            DraftOutcome::Produced => out.push(format!(
                "the command wrote {}; review it, then accept or reject it",
                self.folder
            )),
            DraftOutcome::NoneProduced => out.push(format!(
                "the command produced no specification at {}",
                self.folder
            )),
            DraftOutcome::Accepted => out.push(format!(
                "accepted: {} is the specification this change answers",
                self.folder
            )),
            DraftOutcome::Rejected => out.push(
                "rejected: the worktree was put back; the dispatch and its permissions stay on the record"
                    .into(),
            ),
        }
        if let Some(v) = self
            .speckit_version
            .as_deref()
            .filter(|v| below(v, NO_PERSIST_FROM))
        {
            out.push(format!(
                "Spec Kit {v} predates {NO_PERSIST_FROM}, which honours SPECIFY_FEATURE_NO_PERSIST; \
                 its active feature is recorded inside this worktree only"
            ));
        }
        out
    }
}

/// The first Spec Kit release whose scripts honour `SPECIFY_FEATURE_NO_PERSIST`.
pub const NO_PERSIST_FROM: &str = "1.0.11";

/// Whether dotted version `v` sorts before `than`; an unreadable one does not.
pub fn below(v: &str, than: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u64>> {
        s.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| p.parse().ok())
            .collect()
    };
    match (parse(v), parse(than)) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Spec Kit: the invocation, spelled per agent
// ---------------------------------------------------------------------------

/// How Spec Kit spells a command for an integration in skills mode. From
/// `src/specify_cli/_invocation_style.py` in github/spec-kit, read on
/// 2026-10-03 (release v1.1.0, published 2026-10-02): dollar-skills agents use
/// `$`, Kimi uses `/skill:`, every other agent `/`. In command mode (the `.`
/// separator) every agent uses `/`.
const DOLLAR_SKILLS: &[&str] = &["codex", "zcode", "command-code"];
const SKILL_COLON: &[&str] = &["kimi"];

/// The Spec Kit integration keys Devplane recognises. An agent id outside
/// this list is refused by name: the repository's file selects among these
/// spellings and never adds one.
pub const SPECKIT_KEYS: &[&str] = &[
    "claude",
    "codex",
    "copilot",
    "gemini",
    "opencode",
    "kimi",
    "zcode",
    "command-code",
    "cursor-agent",
    "qwen",
    "windsurf",
];

/// Devplane's agent id as Spec Kit's integration key. The built-in agents
/// share their names with Spec Kit's keys (dated 2026-10-03); a user agent is
/// recognised when its id is one of [`SPECKIT_KEYS`].
pub fn speckit_key(agent: &str) -> Option<&'static str> {
    SPECKIT_KEYS.iter().copied().find(|k| *k == agent)
}

/// The project's Spec Kit integration record, as `.specify/integration.json`
/// holds it — only the fields read here.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Integrations {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub installed_integrations: Vec<String>,
    #[serde(default)]
    pub integration_settings: std::collections::BTreeMap<String, IntegrationSettings>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct IntegrationSettings {
    #[serde(default)]
    pub invoke_separator: Option<String>,
}

/// `/speckit-specify` spelled for Spec Kit integration `key` with separator
/// `sep`. Only the spellings in Spec Kit's own table are produced; anything
/// else is refused naming the value found.
pub fn speckit_invocation(key: &str, sep: &str) -> Result<String, String> {
    if !SPECKIT_KEYS.contains(&key) {
        return Err(format!(
            "Devplane does not know how Spec Kit spells its commands for `{key}`"
        ));
    }
    match sep {
        "-" => {
            let prefix = if DOLLAR_SKILLS.contains(&key) {
                "$"
            } else if SKILL_COLON.contains(&key) {
                "/skill:"
            } else {
                "/"
            };
            Ok(format!("{prefix}speckit-specify"))
        }
        "." => Ok("/speckit.specify".into()),
        other => Err(format!(
            "the integration record names separator `{other}` for `{key}`, which is not one Spec Kit \
             documents (`-` or `.`)"
        )),
    }
}

/// The invocation for `agent` from the project's integration record, or the
/// refusal naming what is installed and Spec Kit's own command for adding one.
pub fn speckit_for(agent: &str, record: &Integrations) -> Result<String, String> {
    let Some(key) = speckit_key(agent) else {
        return Err(format!(
            "agent `{agent}` is not one Devplane can match to a Spec Kit integration; Spec Kit \
             integrations Devplane spells: {}",
            SPECKIT_KEYS.join(", ")
        ));
    };
    if !record.installed_integrations.iter().any(|i| i == key) {
        let installed = match record.installed_integrations.as_slice() {
            [] => "none".to_string(),
            some => some.join(", "),
        };
        return Err(format!(
            "Spec Kit has no `{key}` integration installed in this project (installed: {installed}); \
             `specify integration install {key}` adds it"
        ));
    }
    let sep = record
        .integration_settings
        .get(key)
        .and_then(|s| s.invoke_separator.as_deref())
        .unwrap_or("-");
    speckit_invocation(key, sep)
}

// ---------------------------------------------------------------------------
// Spec Kit: the feature folder
// ---------------------------------------------------------------------------

/// How the project numbers its feature folders: `.specify/init-options.json`
/// `feature_numbering`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbering {
    Sequential,
    Timestamp,
}

impl Numbering {
    /// `feature_numbering` as the file has it; absent or unknown is sequential.
    pub fn of(value: Option<&str>) -> Self {
        match value {
            Some("timestamp") => Numbering::Timestamp,
            _ => Numbering::Sequential,
        }
    }
}

/// The folder name a change title gives: lowercase words of letters and
/// digits, at most five, joined by `-`. Choosing a name is not writing prose.
pub fn folder_slug(title: &str) -> String {
    let words: Vec<String> = title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(5)
        .map(|w| w.to_ascii_lowercase())
        .collect();
    match words.is_empty() {
        true => "feature".into(),
        false => words.join("-"),
    }
}

/// Whether a folder name is a timestamp-numbered one: `YYYYMMDD-HHMMSS-`.
fn timestamped(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 16
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'-'
}

/// The sequential number a folder name carries, as Spec Kit's own
/// `create-new-feature.sh` reads one: three or more leading digits and a
/// `-`, timestamp names skipped.
fn sequence_of(name: &str) -> Option<u64> {
    if timestamped(name) {
        return None;
    }
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    (digits.len() >= 3 && name[digits.len()..].starts_with('-'))
        .then(|| digits.parse().ok())
        .flatten()
}

/// The next feature folder name for `slug`, unused by any of `taken` (folder
/// names gathered from the checkout, every open change's worktree and every
/// pending draft). Sequential: one more than the highest number, padded to
/// three. Timestamp: `now`'s `YYYYMMDD-HHMMSS`, a second later while taken.
pub fn next_folder(
    numbering: Numbering,
    slug: &str,
    taken: &[String],
    now: jiff::Timestamp,
) -> String {
    match numbering {
        Numbering::Sequential => {
            let highest = taken
                .iter()
                .filter_map(|n| sequence_of(n))
                .max()
                .unwrap_or(0);
            format!("{:03}-{slug}", highest + 1)
        }
        Numbering::Timestamp => {
            let mut at = now;
            loop {
                let stamp = at
                    .to_zoned(jiff::tz::TimeZone::UTC)
                    .strftime("%Y%m%d-%H%M%S");
                let prefix = format!("{stamp}-");
                if !taken.iter().any(|t| t.starts_with(&prefix)) {
                    return format!("{stamp}-{slug}");
                }
                at += jiff::SignedDuration::from_secs(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OpenSpec: the installed propose workflow
// ---------------------------------------------------------------------------

/// Where OpenSpec's generators put its propose workflow for each agent, and
/// how the agent invokes it: OpenSpec's supported-tools reference and its
/// command-generation adapters, read on 2026-10-03 (v1.14.0, published
/// 2026-09-30). The skill comes first: it is preferred where both exist.
pub const OPENSPEC_PROPOSE: &[(&str, &str, &str)] = &[
    (
        "claude",
        ".claude/skills/openspec-propose/SKILL.md",
        "/openspec-propose",
    ),
    (
        "claude",
        ".claude/commands/opsx/propose.md",
        "/opsx:propose",
    ),
    (
        "codex",
        ".agents/skills/openspec-propose/SKILL.md",
        "$openspec-propose",
    ),
    (
        "gemini",
        ".gemini/skills/openspec-propose/SKILL.md",
        "/openspec-propose",
    ),
    (
        "gemini",
        ".gemini/commands/opsx/propose.toml",
        "/opsx:propose",
    ),
    (
        "copilot",
        ".github/skills/openspec-propose/SKILL.md",
        "/openspec-propose",
    ),
    (
        "copilot",
        ".github/prompts/opsx-propose.prompt.md",
        "/opsx-propose",
    ),
    (
        "opencode",
        ".opencode/skills/openspec-propose/SKILL.md",
        "/openspec-propose",
    ),
    (
        "opencode",
        ".opencode/commands/opsx-propose.md",
        "/opsx-propose",
    ),
];

/// OpenSpec's tool id for a Devplane agent, as `openspec init --tools` takes it.
fn openspec_tool(agent: &str) -> &str {
    match agent {
        "copilot" => "github-copilot",
        other => other,
    }
}

/// The propose workflow's invocation for `agent`, given which files exist in
/// the change's worktree (`present`). Neither file present is refused, naming
/// what was looked for and OpenSpec's own command for installing it.
pub fn openspec_invocation(agent: &str, present: impl Fn(&str) -> bool) -> Result<String, String> {
    let rows: Vec<&(&str, &str, &str)> = OPENSPEC_PROPOSE
        .iter()
        .filter(|(a, _, _)| *a == agent)
        .collect();
    if rows.is_empty() {
        return Err(format!(
            "agent `{agent}` is not one Devplane can match to an OpenSpec tool; OpenSpec tools \
             Devplane spells: claude, codex, gemini, copilot, opencode"
        ));
    }
    if let Some((_, _, how)) = rows.iter().find(|(_, file, _)| present(file)) {
        return Ok((*how).to_string());
    }
    let looked: Vec<&str> = rows.iter().map(|(_, f, _)| *f).collect();
    Err(format!(
        "OpenSpec's propose workflow is not installed for `{agent}` in this change's worktree \
         (looked for {}); `openspec init --tools {}` installs it, or `openspec update` after \
         choosing the workflow",
        looked.join(" and "),
        openspec_tool(agent)
    ))
}

/// The OpenSpec change the run made: the one folder under its change root
/// that was not there at dispatch. More than one, or none, is no single
/// answer.
pub fn made_change(found: &[String], now: &[String]) -> Option<String> {
    let new: Vec<&String> = now
        .iter()
        .filter(|n| *n != "archive" && !found.contains(n))
        .collect();
    match new.as_slice() {
        [one] => Some((*one).clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// What is sent
// ---------------------------------------------------------------------------

/// The prompt: the invocation, one space, the person's words verbatim. An
/// empty intent is refused before anything starts.
pub fn sent(invocation: &str, intent: &str) -> Result<String, String> {
    if intent.trim().is_empty() {
        return Err("write what the change is for: a draft starts from your words".into());
    }
    Ok(format!("{invocation} {intent}"))
}

/// Which methodology a draft runs, from the dialects detected (by name) and
/// the person's choice. Never chosen for them when both are present.
pub fn choose(detected: &[String], asked: Option<Methodology>) -> Result<Methodology, String> {
    let has = |m: Methodology| detected.iter().any(|d| d == m.dialect());
    let offered: Vec<Methodology> = [Methodology::SpecKit, Methodology::OpenSpec]
        .into_iter()
        .filter(|m| has(*m))
        .collect();
    match (asked, offered.as_slice()) {
        (Some(m), _) if has(m) => Ok(m),
        (Some(m), _) => Err(format!(
            "{} is not set up in this project; detected: {}",
            m.dialect(),
            match detected {
                [] => "no specification layout".to_string(),
                d => d.join(", "),
            }
        )),
        (None, [one]) => Ok(*one),
        (None, []) => Err(match detected {
            [] => "drafting dispatches Spec Kit and OpenSpec only, and this project has neither"
                .to_string(),
            d => format!(
                "drafting dispatches Spec Kit and OpenSpec only; this project has {}",
                d.join(", ")
            ),
        }),
        (None, _) => Err(
            "this project has both Spec Kit and OpenSpec; choose one with `--methodology speckit` or \
             `--methodology openspec`"
                .into(),
        ),
    }
}

/// A draft's run as the review groups it: every changed file under the
/// draft's folder belongs to the draft run, as if that folder were a task it
/// was sent; everything else stays *not asked for*.
pub fn asked_for_folder(
    mut intent: crate::core::review::Intent,
    set: &crate::core::diff::ChangeSet,
    run: &RunId,
    folder: &str,
    sent: &str,
) -> crate::core::review::Intent {
    if intent.unavailable.is_some() {
        return intent;
    }
    let folder = folder.trim_end_matches('/');
    let under = |p: &str| p == folder || p.starts_with(&format!("{folder}/"));
    let (mine, rest): (Vec<_>, Vec<_>) = intent
        .not_asked_for
        .into_iter()
        .partition(|(i, _)| set.files.get(*i).is_some_and(|f| under(&f.path)));
    intent.not_asked_for = rest;
    if mine.is_empty() {
        return intent;
    }
    let mut files: Vec<usize> = mine.into_iter().map(|(i, _)| i).collect();
    files.sort_by(|a, b| set.files[*a].path.cmp(&set.files[*b].path));
    intent.groups.push(crate::core::review::IntentGroup {
        run: run.clone(),
        tasks: vec![crate::spec::SentTask {
            path: folder.to_string(),
            text: sent.to_string(),
            cites: Vec::new(),
            line: 0,
            occurrence: 1,
        }],
        files,
    });
    intent
}

// ---------------------------------------------------------------------------
// Clarifying: one marker, one span
// ---------------------------------------------------------------------------

/// Where the first of `markers` sits in `line`, as a byte range, matched
/// without regard to ASCII case.
fn find_marker(line: &str, markers: &[String]) -> Option<(usize, usize)> {
    let lower = line.to_ascii_lowercase();
    markers
        .iter()
        .filter(|m| !m.trim().is_empty())
        .filter_map(|m| {
            let m = m.to_ascii_lowercase();
            lower.find(&m).map(|at| (at, at + m.len()))
        })
        .min()
}

/// The bracket that holds the marker: from its `[` to the matching `]`, the
/// innermost one that contains the whole marker.
pub fn bracketed_span(line: &str, markers: &[String]) -> Option<(usize, usize)> {
    let (from, to) = find_marker(line, markers)?;
    let mut open: Vec<usize> = Vec::new();
    for (i, c) in line.char_indices() {
        match c {
            '[' => open.push(i),
            ']' => {
                if let Some(start) = open.pop()
                    && start < from
                    && i >= to
                {
                    return Some((start, i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Whether a marker line is bracketed, so Devplane can replace exactly it.
pub fn is_bracketed(line: &str, markers: &[String]) -> bool {
    bracketed_span(line, markers).is_some()
}

/// `line` with its bracketed marker replaced by `answer`, verbatim, and
/// every other byte as it was. `None` for an unbracketed marker: Devplane
/// does not edit that line.
pub fn replace_bracketed(line: &str, markers: &[String], answer: &str) -> Option<String> {
    let (start, end) = bracketed_span(line, markers)?;
    Some(format!("{}{answer}{}", &line[..start], &line[end..]))
}

/// A text's lines with their endings kept, so a rewrite changes nothing but
/// the span it names.
fn lines_kept(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// The file with line `number` (1-based) answered. Refused when the line no
/// longer reads `expected` (an edit moved it) or its marker is not bracketed.
pub fn answer_in(
    text: &str,
    number: u32,
    expected: &str,
    markers: &[String],
    answer: &str,
) -> Result<(String, String, String), Clarify> {
    if answer.trim().is_empty() {
        return Err(Clarify::Empty);
    }
    let lines = lines_kept(text);
    let Some(raw) = lines
        .get(number.saturating_sub(1) as usize)
        .filter(|_| number > 0)
    else {
        return Err(Clarify::Moved);
    };
    let ending = if raw.ends_with("\r\n") {
        "\r\n"
    } else if raw.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let line = &raw[..raw.len() - ending.len()];
    if line.trim() != expected.trim() {
        return Err(Clarify::Moved);
    }
    let Some(new_line) = replace_bracketed(line, markers, answer) else {
        return Err(Clarify::Unbracketed);
    };
    let mut out = String::with_capacity(text.len() + answer.len());
    for (i, l) in lines.iter().enumerate() {
        match i + 1 == number as usize {
            true => {
                out.push_str(&new_line);
                out.push_str(ending);
            }
            false => out.push_str(l),
        }
    }
    Ok((out, line.to_string(), new_line))
}

/// Why an answer was not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clarify {
    /// Nothing to write.
    Empty,
    /// The line is not what it was when it was read.
    Moved,
    /// The marker has no bracket around it.
    Unbracketed,
}

impl Clarify {
    pub fn says(&self, change: &str) -> String {
        match self {
            Clarify::Empty => "type your answer first — an empty answer is not one".into(),
            Clarify::Moved => "the line changed since it was read; read it again".into(),
            Clarify::Unbracketed => format!(
                "Devplane does not edit this line; send your answer to the agent with \
                 `devplane change prompt {change} -- <answer>`"
            ),
        }
    }
}

/// A one-line unified diff of `path` at line `number`.
pub fn line_diff(path: &str, number: u32, old: &str, new: &str) -> String {
    format!("--- a/{path}\n+++ b/{path}\n@@ -{number} +{number} @@\n-{old}\n+{new}\n")
}

/// The inbox id of one marker line: the change, the file, the line, and the
/// line's own content, so an edit above it does not re-point an answer.
pub fn question_id(change: &str, path: &str, line: u32, text: &str) -> String {
    let hash = crate::core::hash::hex(text.trim().as_bytes());
    format!("spec-question:{change}:{path}:{line}:{}", &hash[..12])
}

// ---------------------------------------------------------------------------
// Offering the marker setting
// ---------------------------------------------------------------------------

/// The marker Spec Kit's own specification template writes. OpenSpec
/// defines none, so none is offered for it.
pub const SPECKIT_MARKER: &str = "NEEDS CLARIFICATION";

/// `devplane.toml` with `[spec] open_questions` set to Spec Kit's marker,
/// fenced by a comment naming Devplane and `date`, and the diff that shows
/// it. The key goes inside an existing `[spec]` table so the file still
/// parses; otherwise a table is appended. `None` when the file already
/// declares the key or would not parse afterwards.
pub fn open_questions_offer(text: &str, date: &str) -> Option<(String, String)> {
    let parsed: toml::Table = toml::from_str(text).ok()?;
    if parsed
        .get("spec")
        .and_then(|s| s.as_table())
        .is_some_and(|s| s.contains_key("open_questions"))
    {
        return None;
    }
    let fence =
        format!("# Added by Devplane on {date}: Spec Kit's own unresolved-question marker.");
    let key = format!("open_questions = [\"{SPECKIT_MARKER}\"]");
    let lines = lines_kept(text);
    let header = lines
        .iter()
        .position(|l| l.trim() == "[spec]" || l.trim().starts_with("[spec]"));
    let (out, at, added) = match header {
        Some(h) => {
            let mut out = String::new();
            for (i, l) in lines.iter().enumerate() {
                out.push_str(l);
                if i == h {
                    if !l.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(&format!("{fence}\n{key}\n"));
                }
            }
            (out, h + 2, vec![fence.clone(), key.clone()])
        }
        None => {
            let mut out = text.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            let blank = !out.is_empty();
            if blank {
                out.push('\n');
            }
            out.push_str(&format!("[spec]\n{fence}\n{key}\n"));
            let mut added = Vec::new();
            if blank {
                added.push(String::new());
            }
            added.extend(["[spec]".to_string(), fence.clone(), key.clone()]);
            (out, lines.len() + 1, added)
        }
    };
    let check: toml::Table = toml::from_str(&out).ok()?;
    let declared = check
        .get("spec")
        .and_then(|s| s.get("open_questions"))
        .and_then(|v| v.as_array())
        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(SPECKIT_MARKER)));
    if !declared {
        return None;
    }
    let mut diff = format!(
        "--- a/devplane.toml\n+++ b/devplane.toml\n@@ -{} +{},{} @@\n",
        at.saturating_sub(1),
        at,
        added.len()
    );
    for l in &added {
        diff.push_str(&format!("+{l}\n"));
    }
    Some((out, diff))
}

/// What a change says about unresolved-question words after a draft, when
/// the project declares none: for OpenSpec the setting is named and no word
/// offered, since OpenSpec defines none.
pub const OPENSPEC_NO_MARKER: &str = "OpenSpec defines no unresolved-question word; \
     [spec] open_questions in devplane.toml declares one";

/// The sentence beside the Spec Kit offer.
pub const SPECKIT_NO_MARKER: &str = "no unresolved-question words are declared, so markers in \
     this specification are not inbox items; Spec Kit's own marker can be added to \
     [spec] open_questions";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_kit_spells_its_command_per_agent() {
        assert_eq!(
            speckit_invocation("claude", "-").unwrap(),
            "/speckit-specify"
        );
        assert_eq!(
            speckit_invocation("codex", "-").unwrap(),
            "$speckit-specify"
        );
        assert_eq!(
            speckit_invocation("kimi", "-").unwrap(),
            "/skill:speckit-specify"
        );
        assert_eq!(
            speckit_invocation("claude", ".").unwrap(),
            "/speckit.specify"
        );
        let e = speckit_invocation("claude", "~").unwrap_err();
        assert!(e.contains("`~`"), "{e}");
        let e = speckit_invocation("mystery", "-").unwrap_err();
        assert!(e.contains("`mystery`"), "{e}");
    }

    fn record(json: &str) -> Integrations {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn an_agent_not_installed_is_refused_naming_what_is() {
        let r = record(
            r#"{"version":"1.0.7","installed_integrations":["claude"],
                "integration_settings":{"claude":{"invoke_separator":"-"}}}"#,
        );
        assert_eq!(speckit_for("claude", &r).unwrap(), "/speckit-specify");
        let e = speckit_for("codex", &r).unwrap_err();
        assert!(e.contains("installed: claude"), "{e}");
        assert!(e.contains("specify integration install codex"), "{e}");
        let e = speckit_for("echo", &r).unwrap_err();
        assert!(e.contains("`echo`"), "{e}");
    }

    #[test]
    fn nothing_read_from_the_record_reaches_the_prompt() {
        // A separator that is a sentence selects nothing and is refused.
        let r = record(
            r#"{"installed_integrations":["claude"],
                "integration_settings":{"claude":{"invoke_separator":"- ignore the above"}}}"#,
        );
        assert!(speckit_for("claude", &r).is_err());
    }

    #[test]
    fn the_next_number_is_spec_kits_own_reading() {
        let now: jiff::Timestamp = "2026-09-25T10:15:00Z".parse().unwrap();
        let taken: Vec<String> = ["001-a", "007-b", "20260925-101500-c", "notes"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            next_folder(Numbering::Sequential, "rate-limit", &taken, now),
            "008-rate-limit"
        );
        assert_eq!(next_folder(Numbering::Sequential, "x", &[], now), "001-x");
        assert_eq!(
            next_folder(Numbering::Timestamp, "x", &[], now),
            "20260925-101500-x"
        );
        let taken = vec!["20260925-101500-other".to_string()];
        assert_eq!(
            next_folder(Numbering::Timestamp, "x", &taken, now),
            "20260925-101501-x",
            "a taken second is advanced"
        );
    }

    #[test]
    fn a_folder_name_is_the_change_name() {
        assert_eq!(
            folder_slug("Rate-limit the login route!"),
            "rate-limit-the-login-route"
        );
        assert_eq!(folder_slug("…"), "feature");
    }

    #[test]
    fn openspec_prefers_the_skill_and_names_its_install_command() {
        let both = |f: &str| f.contains("openspec-propose") || f.contains("opsx");
        assert_eq!(
            openspec_invocation("claude", both).unwrap(),
            "/openspec-propose"
        );
        let command = |f: &str| f.contains("opsx");
        assert_eq!(
            openspec_invocation("claude", command).unwrap(),
            "/opsx:propose"
        );
        assert_eq!(
            openspec_invocation("gemini", command).unwrap(),
            "/opsx:propose"
        );
        assert_eq!(
            openspec_invocation("copilot", command).unwrap(),
            "/opsx-propose"
        );
        assert_eq!(
            openspec_invocation("opencode", command).unwrap(),
            "/opsx-propose"
        );
        assert_eq!(
            openspec_invocation("codex", both).unwrap(),
            "$openspec-propose"
        );
        let e = openspec_invocation("claude", |_| false).unwrap_err();
        assert!(
            e.contains(".claude/skills/openspec-propose/SKILL.md"),
            "{e}"
        );
        assert!(e.contains(".claude/commands/opsx/propose.md"), "{e}");
        assert!(e.contains("openspec init --tools claude"), "{e}");
        let e = openspec_invocation("copilot", |_| false).unwrap_err();
        assert!(e.contains("openspec init --tools github-copilot"), "{e}");
        assert!(openspec_invocation("echo", both).is_err());
    }

    #[test]
    fn the_change_openspec_made_is_the_new_one() {
        let found = vec!["old".to_string()];
        assert_eq!(
            made_change(&found, &["old".into(), "new".into(), "archive".into()]),
            Some("new".into())
        );
        assert_eq!(made_change(&found, &["old".into()]), None);
        assert_eq!(made_change(&[], &["a".into(), "b".into()]), None);
    }

    #[test]
    fn what_is_sent_is_the_invocation_and_the_words() {
        assert_eq!(
            sent("/speckit-specify", "Two sentences.  Verbatim.").unwrap(),
            "/speckit-specify Two sentences.  Verbatim."
        );
        assert!(sent("/speckit-specify", "  \n").is_err());
    }

    #[test]
    fn a_methodology_is_never_chosen_for_the_person() {
        let both = vec!["Spec Kit".to_string(), "OpenSpec".to_string()];
        assert!(choose(&both, None).unwrap_err().contains("--methodology"));
        assert_eq!(
            choose(&both, Some(Methodology::OpenSpec)).unwrap(),
            Methodology::OpenSpec
        );
        assert_eq!(
            choose(&["Spec Kit".into()], None).unwrap(),
            Methodology::SpecKit
        );
        let e = choose(&["GSD".into(), "Kiro".into()], None).unwrap_err();
        assert!(
            e.contains("drafting dispatches Spec Kit and OpenSpec only"),
            "{e}"
        );
        assert!(choose(&["Spec Kit".into()], Some(Methodology::OpenSpec)).is_err());
    }

    fn m() -> Vec<String> {
        vec![SPECKIT_MARKER.to_string()]
    }

    #[test]
    fn a_bracketed_answer_replaces_exactly_the_bracket() {
        let line = "- **FR-006**: lock for [NEEDS CLARIFICATION: which timeout?] after five.";
        assert_eq!(
            replace_bracketed(line, &m(), "30 seconds").unwrap(),
            "- **FR-006**: lock for 30 seconds after five."
        );
        assert_eq!(
            replace_bracketed("- which timeout? NEEDS CLARIFICATION", &m(), "x"),
            None,
            "an unbracketed marker is not edited"
        );
        assert_eq!(
            replace_bracketed("a [b [needs clarification: c] d] e", &m(), "X").unwrap(),
            "a [b X d] e",
            "the innermost bracket holding the marker"
        );
        assert_eq!(
            replace_bracketed("[x] then [NEEDS CLARIFICATION: y]", &m(), "Z").unwrap(),
            "[x] then Z"
        );
    }

    #[test]
    fn an_answer_changes_one_span_in_one_line() {
        let text = "# Spec\r\n\r\n- A [NEEDS CLARIFICATION: a?] x\r\n- B\n";
        let (out, old, new) =
            answer_in(text, 3, "- A [NEEDS CLARIFICATION: a?] x", &m(), "yes").unwrap();
        assert_eq!(out, "# Spec\r\n\r\n- A yes x\r\n- B\n");
        assert_eq!(old, "- A [NEEDS CLARIFICATION: a?] x");
        assert_eq!(new, "- A yes x");
        assert_eq!(
            answer_in(text, 4, "- A [NEEDS CLARIFICATION: a?] x", &m(), "y"),
            Err(Clarify::Moved)
        );
        assert_eq!(
            answer_in(text, 3, "- A [NEEDS CLARIFICATION: a?] x", &m(), " "),
            Err(Clarify::Empty)
        );
        assert_eq!(
            answer_in(
                "NEEDS CLARIFICATION here\n",
                1,
                "NEEDS CLARIFICATION here",
                &m(),
                "y"
            ),
            Err(Clarify::Unbracketed)
        );
        assert_eq!(answer_in(text, 0, "", &m(), "y"), Err(Clarify::Moved));
    }

    #[test]
    fn the_offer_goes_inside_an_existing_spec_table() {
        let text = "[gates]\ncheck = [\"true\"]\n\n[spec]\nplans = \"docs\"\n";
        let (out, diff) = open_questions_offer(text, "2026-10-03").unwrap();
        let t: toml::Table = toml::from_str(&out).unwrap();
        assert_eq!(t["spec"]["plans"].as_str(), Some("docs"));
        assert_eq!(
            t["spec"]["open_questions"][0].as_str(),
            Some("NEEDS CLARIFICATION")
        );
        assert!(out.contains("# Added by Devplane on 2026-10-03"));
        assert!(
            diff.contains("+open_questions = [\"NEEDS CLARIFICATION\"]"),
            "{diff}"
        );
        assert!(out.starts_with(text.split("[spec]").next().unwrap()));

        let (out, _) = open_questions_offer("[gates]\ncheck = []", "d").unwrap();
        assert!(out.starts_with("[gates]\ncheck = []\n\n[spec]\n"), "{out}");
        assert!(
            open_questions_offer(&out, "d").is_none(),
            "already declared"
        );
        assert!(open_questions_offer("not = [toml", "d").is_none());
    }

    #[test]
    fn a_version_below_no_persist_is_said() {
        assert!(below("1.0.7", NO_PERSIST_FROM));
        assert!(!below("1.0.11", NO_PERSIST_FROM));
        assert!(!below("1.1.0", NO_PERSIST_FROM));
        assert!(!below("garbage", NO_PERSIST_FROM));
    }

    #[test]
    fn a_question_id_names_the_line_and_its_content() {
        let a = question_id("c-1", "specs/a/spec.md", 4, "- x [NEEDS CLARIFICATION]");
        let b = question_id("c-1", "specs/a/spec.md", 4, "- y [NEEDS CLARIFICATION]");
        assert!(a.starts_with("spec-question:c-1:specs/a/spec.md:4:"));
        assert_ne!(a, b);
    }
}

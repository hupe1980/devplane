//! The rule to paste, composed from the calls this machine has seen interrupt.
//! Everything here composes; nothing writes: an agent runs as the same user
//! and can read the bearer token, so a write path to a permission file would
//! be reachable by the party the rules govern.

use serde::{Deserialize, Serialize};

use crate::core::command;
use crate::core::policy::{self, Class, Context, Rule};

/// How many distinct calls must want a family rule before it is offered.
pub const WORTH_A_RULE: usize = 3;

/// The most calls counted exactly; beyond it a count is a floor ("50+").
pub const MAX_KIN: usize = 50;

/// Whether the rule covers this call alone or its family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Call,
    Family,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleOffer {
    /// The text as it must be typed: `Bash(cargo test *)`.
    pub rule: String,
    pub basis: Basis,
    /// Distinct calls this rule would answer that nothing answers today (≥ 1).
    pub covers: u32,
    /// Counting stopped at [`MAX_KIN`].
    #[serde(default)]
    pub more: bool,
    pub file: String,
    pub section: String,
}

impl RuleOffer {
    /// The line to paste, as JSON for the agent's `settings.json`.
    pub fn pasteable(&self) -> String {
        let rule = serde_json::Value::String(self.rule.clone());
        format!("\"permissions\": {{ \"allow\": [{rule}] }}")
    }
}

/// Why there is no rule; a blank where an offer belongs reads as broken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoOffer {
    /// The tool's rules take no specifier that could be widened.
    NoShape,
    /// No prefix rule may approve this call; `why` names the construct.
    Unapprovable { why: String },
    /// Several commands in one call: several grants.
    Compound,
    /// A composed rule, replayed against the call, did not decide it.
    WouldNotDecide,
    /// A prompt raised through a notification that names no tool or input.
    UnknownCall,
}

impl NoOffer {
    /// No two of these may be the same.
    pub fn sentence(&self) -> String {
        match self {
            Self::NoShape => "This tool's rules take no pattern, so there is nothing to widen.".into(),
            Self::Unapprovable { why } => format!("No rule can pre-approve this call: {why}."),
            Self::Compound => "This call is several commands, and they are several separate grants. Answer them one at a time.".into(),
            Self::WouldNotDecide => "A rule was composed for this and then refused — replayed against the call, it did not decide it. You would have pasted it in and been asked again.".into(),
            Self::UnknownCall => "Claude Code raised this through its own dialog and did not say which call it is about, so there is nothing to write a rule from.".into(),
        }
    }
}

/// A refusal with its sentence, for a page that cannot call [`NoOffer::sentence`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoOfferView {
    pub reason: NoOffer,
    pub sentence: String,
}

impl From<NoOffer> for NoOfferView {
    fn from(reason: NoOffer) -> Self {
        Self {
            sentence: reason.sentence(),
            reason,
        }
    }
}

pub const ALLOW_KEY: &str = "permissions.allow";

/// Where the rule goes; the caller picks the file by whether the call was in
/// a registered project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub file: String,
    pub section: String,
}

/// A call that would interrupt, not merely one that happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupting {
    pub tool: String,
    /// What the tool's rules match: a command, a path, a URL.
    pub content: String,
}

/// The rule to offer for this call, or the reason there is none. `others` is
/// every other call that would interrupt in the same scope.
pub fn compose(
    tool: &str,
    input: &serde_json::Value,
    ctx: &Context<'_>,
    others: &[Interrupting],
    dest: &Destination,
) -> Result<RuleOffer, NoOffer> {
    let Some(content) = policy::rule_content(tool, input) else {
        return Err(NoOffer::NoShape);
    };
    // A pasted rule approves calls Devplane never sees again, so a line the
    // reader cannot see through gets none: it would be wider than they believe.
    if policy::is_command_tool(tool) {
        let line = command::read(&content);
        if let Some(why) = line.barrier {
            return Err(NoOffer::Unapprovable { why });
        }
        if line.commands.len() != 1 {
            return Err(NoOffer::Compound);
        }
    }
    let family = command::rule_family(tool, &content);
    let mut kin: Vec<String> = vec![content.clone()];
    let more = others.len() >= MAX_KIN;
    for o in others {
        // Distinct calls: the asked-about call is in the log too.
        if o.tool.eq_ignore_ascii_case(tool)
            && command::rule_family(tool, &o.content) == family
            && !kin.contains(&o.content)
        {
            kin.push(o.content.clone());
        }
    }
    let offer = |rule, basis, covers| RuleOffer {
        rule,
        basis,
        covers,
        more: basis == Basis::Family && more,
        file: dest.file.clone(),
        section: dest.section.clone(),
    };
    // Family rule first; if it fails the replay, fall back to the call's own.
    if kin.len() >= WORTH_A_RULE
        && let Some(spec) = command::suggest_rule_for(tool, &kin)
        && let Some(rule) = decides(&spec, tool, input, ctx)
    {
        return Ok(offer(rule, Basis::Family, kin.len() as u32));
    }
    let Some(spec) = command::suggest_rule_for(tool, std::slice::from_ref(&content)) else {
        return Err(NoOffer::NoShape);
    };
    let rule = decides(&spec, tool, input, ctx).ok_or(NoOffer::WouldNotDecide)?;
    Ok(offer(rule, Basis::Call, 1))
}

/// The rule text, only if it parses and actually matches the call.
fn decides(spec: &str, tool: &str, input: &serde_json::Value, ctx: &Context<'_>) -> Option<String> {
    let rule = format!("{tool}({spec})");
    let parsed = Rule::parse(&rule, Class::Allow)?;
    parsed.matches(ctx, tool, input).then_some(rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn dest() -> Destination {
        Destination {
            file: "/repo/.claude/settings.json".into(),
            section: ALLOW_KEY.into(),
        }
    }

    fn ctx(dir: &Path) -> Context<'_> {
        Context::at(dir)
    }

    fn bash(cmd: &str) -> serde_json::Value {
        serde_json::json!({ "command": cmd })
    }

    fn kin(cmds: &[&str]) -> Vec<Interrupting> {
        cmds.iter()
            .map(|c| Interrupting {
                tool: "Bash".into(),
                content: (*c).to_string(),
            })
            .collect()
    }

    /// Three calls in one family earn the wide rule; two do not.
    #[test]
    fn the_wide_rule_needs_the_evidence_for_it() {
        let dir = Path::new("/repo");
        let two = compose(
            "Bash",
            &bash("cargo test --lib diff"),
            &ctx(dir),
            &kin(&["cargo test --lib spec"]),
            &dest(),
        )
        .expect("a rule for the call itself");
        assert_eq!(two.basis, Basis::Call);
        assert_eq!(two.covers, 1);
        assert_eq!(two.rule, "Bash(cargo test --lib diff)");

        let three = compose(
            "Bash",
            &bash("cargo test --lib diff"),
            &ctx(dir),
            &kin(&["cargo test --lib spec", "cargo test --doc"]),
            &dest(),
        )
        .expect("a rule for the family");
        assert_eq!(three.basis, Basis::Family);
        assert_eq!(three.covers, 3);
        assert_eq!(three.rule, "Bash(cargo test *)");
    }

    #[test]
    fn a_call_does_not_count_itself_twice() {
        let dir = Path::new("/repo");
        let o = compose(
            "Bash",
            &bash("cargo test --lib diff"),
            &ctx(dir),
            &kin(&["cargo test --lib diff", "cargo test --doc"]),
            &dest(),
        )
        .expect("a rule");
        assert_eq!(
            (o.basis, o.covers),
            (Basis::Call, 1),
            "two distinct calls, not three — the asked-about one is in the log too"
        );
    }

    #[test]
    fn only_calls_that_could_share_the_rule_are_counted() {
        let dir = Path::new("/repo");
        let o = compose(
            "Bash",
            &bash("cargo test --lib diff"),
            &ctx(dir),
            &kin(&["git status", "npm run build", "rm -rf target"]),
            &dest(),
        )
        .expect("a rule");
        assert_eq!(
            (o.basis, o.covers),
            (Basis::Call, 1),
            "three unrelated interruptions are not three votes for this rule"
        );
    }

    #[test]
    fn each_refusal_is_its_own_sentence() {
        let all = [
            NoOffer::NoShape,
            NoOffer::Unapprovable {
                why: "`eval` runs a string the shell builds".into(),
            },
            NoOffer::Compound,
            NoOffer::WouldNotDecide,
            NoOffer::UnknownCall,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(!a.sentence().trim().is_empty(), "a blank is not a reason");
            for b in all.iter().skip(i + 1) {
                assert_ne!(a.sentence(), b.sentence(), "two refusals read the same");
            }
        }
    }

    /// Three of the five refusals, reached through `compose`. `UnknownCall` is
    /// raised by the host; `WouldNotDecide` is a guard that only a composer bug
    /// could reach.
    #[test]
    fn the_refusals_are_reachable() {
        let dir = Path::new("/repo");
        let c = |tool: &str, input: serde_json::Value| {
            compose(tool, &input, &ctx(dir), &[], &dest()).unwrap_err()
        };
        assert_eq!(
            c("Bash", bash("cargo build && rm -rf /")),
            NoOffer::Compound
        );
        assert!(matches!(
            c("Bash", bash("eval \"$CMD\"")),
            NoOffer::Unapprovable { .. }
        ));
        assert_eq!(c("TodoWrite", serde_json::json!({})), NoOffer::NoShape);
    }

    /// A shell rule is a command prefix, a path rule a directory glob, a
    /// `WebFetch` rule a domain: the only shapes those rules have.
    #[test]
    fn a_rule_is_written_in_the_vocabulary_its_tool_uses() {
        let dir = Path::new("/repo");
        let one = |tool: &str, input: serde_json::Value| {
            compose(tool, &input, &ctx(dir), &[], &dest()).map(|o| o.rule)
        };
        assert_eq!(
            one("Bash", bash("pnpm test --run")).as_deref(),
            Ok("Bash(pnpm test --run)")
        );
        assert_eq!(
            one("Read", serde_json::json!({"file_path": "src/main.rs"})).as_deref(),
            Ok("Read(src/**)")
        );
        assert_eq!(
            one("WebFetch", serde_json::json!({"url": "https://docs.rs/x"})).as_deref(),
            Ok("WebFetch(domain:docs.rs)")
        );
    }

    /// The offered rule is never wider than the narrowest one covering what it
    /// was composed from. `covers_rule` under-reports, so a pass is real.
    #[test]
    fn no_offer_is_wider_than_it_has_to_be() {
        let dir = Path::new("/repo");
        let o = compose(
            "Bash",
            &bash("cargo test --lib diff"),
            &ctx(dir),
            &kin(&["cargo test --lib spec", "cargo test --doc"]),
            &dest(),
        )
        .expect("a rule");
        let offered = Rule::parse(&o.rule, Class::Allow).expect("parses");
        // `cargo *` would also cover all three and is wider.
        let wider = Rule::parse("Bash(cargo *)", Class::Allow).expect("parses");
        assert!(
            wider.covers_rule(&offered),
            "the sanity check: the wide rule really does contain the offered one"
        );
        assert!(
            !offered.covers_rule(&wider),
            "the offered rule must not be as wide as `cargo *`"
        );
    }

    /// The composed rule decides the call it was composed from.
    #[test]
    fn everything_offered_answers_the_call_it_came_from() {
        let dir = Path::new("/repo");
        for cmd in [
            "cargo test --lib diff",
            "git status",
            "pnpm run build",
            "ls -la",
        ] {
            let o = compose("Bash", &bash(cmd), &ctx(dir), &[], &dest())
                .unwrap_or_else(|e| panic!("{cmd}: {e:?}"));
            let rule = Rule::parse(&o.rule, Class::Allow).expect("parses");
            assert!(
                rule.matches(&ctx(dir), "Bash", &bash(cmd)),
                "{cmd}: offered {} and it does not decide the call",
                o.rule
            );
        }
    }

    /// The offered line must parse as JSON, the language of `settings.json`.
    #[test]
    fn the_line_offered_parses_as_the_file_it_is_pasted_into() {
        let dir = Path::new("/repo");
        let o = compose(
            "Bash",
            &bash("cargo test --lib diff"),
            &ctx(dir),
            &[],
            &dest(),
        )
        .expect("an offer");
        assert!(
            o.section.starts_with("permissions"),
            "the destination moved"
        );
        let line = o.pasteable();
        // A fragment, wrapped the way pasting it into `{ … }` would.
        let doc: serde_json::Value = serde_json::from_str(&format!("{{{line}}}"))
            .unwrap_or_else(|e| panic!("{line} is not JSON: {e}"));
        assert_eq!(doc["permissions"]["allow"][0], serde_json::json!(o.rule));
    }
}

#[cfg(test)]
mod unmodelled_tests {
    use super::*;
    use std::path::Path;

    /// `cat $'\x2e\x65nv'` is `cat .env` in a quoting form the matcher does not
    /// model; `Bash(cat *)` would approve more than the reader sees. No rule is
    /// offered, and the reason names the construct.
    #[test]
    fn no_rule_is_offered_for_a_construct_the_matcher_does_not_model() {
        let ctx = Context::at(Path::new("/repo"));
        let dest = Destination {
            file: "/repo/.claude/settings.json".into(),
            section: ALLOW_KEY.into(),
        };
        let input = serde_json::json!({ "command": "cat $'\\x2e\\x65nv'" });
        let out = compose("Bash", &input, &ctx, &[], &dest);
        match out {
            Err(NoOffer::Unapprovable { why }) => {
                assert!(why.contains("$'"), "the refusal names the construct: {why}")
            }
            other => panic!("a rule was offered for an unmodelled construct: {other:?}"),
        }

        // The ordinary case still gets one.
        let plain = serde_json::json!({ "command": "cat notes.txt" });
        assert!(
            compose("Bash", &plain, &ctx, &[], &dest).is_ok(),
            "a command with no unmodelled construct still gets an offer"
        );
    }
}

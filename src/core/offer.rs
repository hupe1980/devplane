//! The rule to paste, composed from what this machine has actually seen.
//!
//! A permission item can be answered twice: once for the call in front of you,
//! and once for every call like it. The second answer is the one people want
//! and the one nothing here could give, because the evidence for it is not on
//! the item — it is in the event log, in the shape of *how many calls like this
//! one would interrupt me today*.
//!
//! **Everything here composes; nothing here writes.** There is no route that
//! puts a rule in a file and there is not going to be one: an agent on this
//! machine runs as the same user and can read the bearer token, so a write path
//! to `[policy]` would be reachable by the party the rules govern. The handover
//! is a paste, and what is lost is one gesture.
//!
//! **And nothing here reaches the outside world.** The evidence arrives as an
//! argument, which is what lets a composition bug be caught by a table of calls
//! rather than by somebody pasting a rule that does not work.

use serde::{Deserialize, Serialize};

use crate::core::command;
use crate::core::policy::{self, Class, Context, Rule};

/// How many calls must want a rule before one is worth writing.
///
/// Taken from `explain --replay`, which has used it since before this file
/// existed. Below it a standing grant costs more than the interruption it
/// saves, and suggesting one for a call somebody made once is how a list ends
/// up recommending `rm -rf node_modules`.
pub const WORTH_A_RULE: usize = 3;

/// How many distinct calls in one family are worth counting exactly.
///
/// Counting costs a policy evaluation each, and the figure is persuasive long
/// before it is large: *covers 50+ calls like it* says everything *covers 401*
/// would. Measured — an unbounded count made the inbox poll grow with the event
/// log rather than with the number of items (SC-007).
pub const MAX_KIN: usize = 50;

/// What a rule was composed from.
///
/// Two variants because *this is the call you were asked about, written as a
/// rule* and *this is the rule that answers the ones like it* are different
/// offers, and somebody deciding how much to grant is entitled to know which
/// one they are reading. The count alone would not carry it: a family of
/// exactly one is a `Call`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Call,
    Family,
}

/// A rule, and everything needed to act on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleOffer {
    /// The text, complete, as it must be typed: `Bash(cargo test *)`.
    pub rule: String,
    pub basis: Basis,
    /// How many **distinct** calls this rule would answer that nothing answers
    /// today. Always at least one — the call that raised the item.
    pub covers: u32,
    /// Whether the search stopped before the evidence ran out.
    ///
    /// Counting exactly costs a policy evaluation per distinct call, so it is
    /// bounded — and a bounded count rendered as an exact one is the kind of
    /// small lie this product is about. `covers: 50, more: true` reads as
    /// *fifty and counting*, which is what it is.
    #[serde(default)]
    pub more: bool,
    /// The file to paste it into.
    pub file: String,
    /// The key within that file.
    pub section: String,
}

/// Why there is no rule.
///
/// A value rather than an absence, because a blank where an offer belongs reads
/// as broken and every one of these is a different thing to know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoOffer {
    /// The tool's rules take no specifier that could be widened.
    NoShape,
    /// No prefix rule may approve this call, and the construct that decides it.
    Unapprovable { why: String },
    /// Several commands in one call. They are several grants.
    Compound,
    /// A rule was composed and then refused: replayed against the call, it did
    /// not decide it.
    WouldNotDecide,
}

impl NoOffer {
    /// The sentence somebody reads. No two of these may be the same.
    pub fn sentence(&self) -> String {
        match self {
            Self::NoShape => {
                "This tool's rules take no pattern, so there is nothing to widen.".into()
            }
            Self::Unapprovable { why } => {
                format!("No rule can pre-approve this call: {why}.")
            }
            Self::Compound => "This call is several commands, and they are several \
                 separate grants. Answer them one at a time."
                .into(),
            Self::WouldNotDecide => "A rule was composed for this and then refused — replayed \
                 against the call, it did not decide it. You would have pasted \
                 it in and been asked again."
                .into(),
        }
    }
}

/// A refusal as both surfaces need it: the reason, and the words.
///
/// The sentence is on the wire rather than in each renderer because the page
/// cannot call [`NoOffer::sentence`] and a second copy of the wording in
/// JavaScript is a second thing to keep true. The tag stays beside it so a test
/// can assert the reason without matching prose.
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

/// Where the rule goes. Supplied by the caller, which is the half that knows
/// whether this call was made inside a registered project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub file: String,
    pub section: String,
}

/// A call that **would interrupt** — not merely one that happened.
///
/// The distinction is the whole of `covers`. A family auto-approved fifty times
/// and asked about once is a family the existing rules already answer, and
/// counting those fifty would credit a new grant with an old one's work. The
/// filtering is the caller's, because deciding it needs the policy and the
/// policy needs a disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupting {
    pub tool: String,
    /// The text that tool's rules are written about — a command, a path, a URL.
    pub content: String,
}

/// The rule to offer for this call, or the reason there is none.
///
/// `others` is every *other* call that would interrupt in the same scope; the
/// call being asked about is supplied separately and is never expected to
/// appear in it.
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

    // Everything in this family that would also interrupt, this call included.
    // `rule_family` is what decides two calls could plausibly share a rule:
    // the program and the subcommand for a shell, the directory for a path,
    // the host for a URL.
    let family = command::rule_family(tool, &content);
    let mut kin: Vec<String> = vec![content.clone()];
    // The caller stops gathering at its own bound and says so; past it the exact
    // number stops mattering and the cost of finding it does not.
    let more = others.len() >= MAX_KIN;
    for o in others {
        if !o.tool.eq_ignore_ascii_case(tool) {
            continue;
        }
        if command::rule_family(tool, &o.content) != family {
            continue;
        }
        // **Distinct calls, not occurrences**, and the reason is arithmetic
        // rather than taste: the call being asked about is itself an observed
        // call, so it arrives in `others` too and counting occurrences would
        // have it voting for itself. Distinct also makes the number mean
        // something a person can check — *six different commands this answers*
        // — where occurrences would be six runs of the same one.
        if !kin.contains(&o.content) {
            kin.push(o.content.clone());
        }
    }

    // The wide answer first, and only where the evidence is there for it.
    if kin.len() >= WORTH_A_RULE
        && let Some(spec) = command::suggest_rule_for(tool, &kin)
        && let Some(rule) = decides(&spec, tool, input, ctx)
    {
        return Ok(RuleOffer {
            rule,
            basis: Basis::Family,
            covers: kin.len() as u32,
            more,
            file: dest.file.clone(),
            section: dest.section.clone(),
        });
    }

    // **Falling back is not the same as widening.** A family rule that fails
    // the replay is withheld, and the narrower rule for this one call is still
    // true and still worth having — so it is tried rather than the whole offer
    // being dropped. Nothing unverified reaches anybody either way.
    let Some(spec) = command::suggest_rule_for(tool, std::slice::from_ref(&content)) else {
        return Err(why_not(tool, &content));
    };
    let Some(rule) = decides(&spec, tool, input, ctx) else {
        return Err(NoOffer::WouldNotDecide);
    };
    Ok(RuleOffer {
        rule,
        basis: Basis::Call,
        covers: 1,
        more: false,
        file: dest.file.clone(),
        section: dest.section.clone(),
    })
}

/// The rule text, but only if it parses and actually matches the call.
///
/// **This is the check a comment in `command.rs` claimed for years and nothing
/// performed.** Composing a rule from a call and assuming it covers that call
/// is the kind of assumption that holds until it does not, and the failure is
/// the expensive one: somebody pastes the rule in, is asked again, and stops
/// believing every later suggestion.
fn decides(spec: &str, tool: &str, input: &serde_json::Value, ctx: &Context<'_>) -> Option<String> {
    let rule = format!("{tool}({spec})");
    let parsed = Rule::parse(&rule, Class::Allow)?;
    parsed.matches(ctx, tool, input).then_some(rule)
}

/// Why no rule could be composed for a single call.
///
/// Asked only after the composer has already said no, so this classifies a
/// failure rather than predicting one — which is what keeps the two from
/// drifting apart. A reason this cannot name is `NoShape`: there is a specifier
/// and nothing here can make a rule out of it.
fn why_not(tool: &str, content: &str) -> NoOffer {
    if !policy::is_command_tool(tool) {
        return NoOffer::NoShape;
    }
    if let Some(why) = command::unapprovable_by_prefix(content) {
        return NoOffer::Unapprovable { why };
    }
    match command::subcommands(content) {
        Some(parts) if parts.len() > 1 => NoOffer::Compound,
        None => NoOffer::Compound,
        _ => NoOffer::NoShape,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn dest() -> Destination {
        Destination {
            file: "/repo/devplane.toml".into(),
            section: "[policy] auto_allow".into(),
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
    ///
    /// The threshold is the whole difference between a suggestion and a guess,
    /// and it is the same constant `explain --replay` has always used.
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

    /// The call being asked about arrives in `others` as well — it is an
    /// observed call like any other — and must not vote for itself.
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

    /// A call in another family is not evidence for this one.
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

    /// Every reason there is no rule reads differently, and none reads empty.
    #[test]
    fn each_refusal_is_its_own_sentence() {
        let all = [
            NoOffer::NoShape,
            NoOffer::Unapprovable {
                why: "`eval` runs a string the shell builds".into(),
            },
            NoOffer::Compound,
            NoOffer::WouldNotDecide,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(!a.sentence().trim().is_empty(), "a blank is not a reason");
            for b in all.iter().skip(i + 1) {
                assert_ne!(a.sentence(), b.sentence(), "two refusals read the same");
            }
        }
    }

    /// The refusals, reached through `compose` rather than constructed.
    ///
    /// A variant nothing produces is a sentence nobody will ever read, which is
    /// the same defect as a wrong one.
    ///
    /// **Three of the four, and the fourth is deliberate.** `WouldNotDecide`
    /// has no input that produces it, because producing it means the composer
    /// built a rule that does not cover the call it was built from — a bug,
    /// not a shape. It is a guard against this file being wrong, so a test
    /// that reached it would be a test asserting the bug. `decides` is what
    /// keeps it from ever reaching a person.
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

    /// Each tool's rules are written in that tool's own vocabulary.
    ///
    /// A shell rule is a command prefix, a path rule is a directory glob, a
    /// `WebFetch` rule is a domain. Offering the wrong one is worse than
    /// offering nothing: somebody pastes `Read(src/main.rs)` and is asked again
    /// about the next file in the same directory.
    ///
    /// **This is also where "the exact call, never a pattern" stops being
    /// true**, and it always did. It holds for a shell, where a prefix is a
    /// real widening. A `Read` of one file is already answered with the
    /// directory, and a `WebFetch` with the host, because those are the only
    /// shapes those rules have.
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
    /// was composed from (FR-006).
    ///
    /// `covers_rule` is the comparator the product already uses to decide one
    /// rule does nothing another does not, and it under-reports on purpose — so
    /// a pass here is a real containment rather than a coincidence of spelling.
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
        // `cargo *` would also cover all three and is wider. If the composer
        // ever reached for it, this fails.
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
    ///
    /// Stated as a property over a table rather than as one example, because
    /// the failure this guards is a shape that slips through, not a typo.
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
}

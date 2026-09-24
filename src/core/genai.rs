//! The decision log, in the OpenTelemetry GenAI conventions' own shape.
//!
//! # Why this is a renderer and not an exporter
//!
//! Devplane **receives** OTLP; it has never sent any, and that is a decision
//! rather than an omission — no telemetry of its own, nothing in the
//! background, no endpoint configured for it. Adding an exporter to make a point in somebody else's tracker would
//! be a subsystem nobody asked for, running all the time, to produce output
//! that is wanted once.
//!
//! So this is a **rendering of rows that already exist**, written when a person
//! runs a command, to standard output. It is a producer in the only sense the
//! argument needs: real records, from a real machine, in the proposed shape.
//!
//! # What the conventions propose, and what they leave out
//!
//! `open-telemetry/semantic-conventions-genai` **#535**, opened 2026-09-23 and
//! waiting on reviewers, adds `gen_ai.tool.call.decision` with
//! `gen_ai.tool.call.decision.outcome` of `allow`, `deny` or `require_approval`.
//! Its note reads:
//!
//! > *"This event SHOULD be recorded only when a **framework, harness,
//! > application policy, or human approval flow** makes an explicit decision
//! > before tool execution."*
//!
//! **Four deciders in the note. Three attributes on the wire. None of them says
//! which.** An `allow` decided by a person and an `allow` decided by a
//! classifier are the same three bytes — which is the distinction this product
//! exists to record, stated in somebody else's prose and dropped from their
//! schema.
//!
//! # The private attribute, and why it is private
//!
//! The authority rides under an application-specific prefix, which is the
//! pattern #417 established precisely so that a producer does not mint
//! normative `gen_ai.*` names. It is defined **once**, [`AUTHORITY_KEY`], so a
//! normative name retires it in one edit.
//!
//! # What is never emitted
//!
//! Prompt and message content, on any path. This carries a tool name, an
//! outcome and an authority.

use crate::core::decision::{Authority, Decision};

/// The event name #535 proposes.
pub const EVENT: &str = "gen_ai.tool.call.decision";

/// The outcome attribute #535 proposes, with its three members.
pub const OUTCOME_KEY: &str = "gen_ai.tool.call.decision.outcome";

/// **The attribute the proposal does not have**, under an application-specific
/// prefix so that nothing here mints a normative name.
///
/// In one place, so the day the conventions adopt a name for it this is one
/// edit rather than a search.
pub const AUTHORITY_KEY: &str = "devplane.gen_ai.tool.call.decision.authority";

/// What #535's outcome can say about a decision this product recorded.
///
/// **Mapped from what Devplane can decide, never from the member list.** There
/// is no `Verdict::Allow` in this codebase and there has not been since
/// approving was deleted, so `allow` is reachable from exactly one place: a
/// recorded human selection carried to an agent. Everything else this product
/// records is a refusal, a question, or a thing that is not a tool call at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A recorded human selection, carried. The only path to this value.
    Allow,
    /// A prohibition fired.
    Deny,
    /// Suspended pending somebody — a prompt, a question, a hold.
    RequireApproval,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Allow => "allow",
            Outcome::Deny => "deny",
            Outcome::RequireApproval => "require_approval",
        }
    }

    /// The outcome for one recorded decision, or `None` where the row is not a
    /// pre-execution decision about a tool call at all.
    ///
    /// **`None` is the common case and that is correct.** A gate verdict, a
    /// pipeline advancing and a pull request opening are decisions this product
    /// records and are not what this event is for — #535 says so itself:
    /// *"not intended for generic hook lifecycle telemetry or post-execution
    /// tool outcomes."*
    pub fn of(d: &Decision) -> Option<Self> {
        // A tool call, and nothing else. A row with no tool is a gate, a merge
        // or a pipeline step.
        d.tool.as_deref()?;
        match d.outcome.as_str() {
            "deny" => Some(Outcome::Deny),
            "ask" | "unresolved" => Some(Outcome::RequireApproval),
            // **The one path to `allow`, and it is narrow on purpose.** Only a
            // decision a *person* made can be exported as an approval: this
            // product cannot produce one from a rule, a clock, a default or a
            // model, and emitting an `allow` it could not stand behind would be
            // committing the exact error it is reviewing.
            "allow" if d.authority == Authority::Person => Some(Outcome::Allow),
            _ => None,
        }
    }
}

/// Whether the authority is knowable for this row.
///
/// **Unknown is unset, never a default.** #386 argues the same for evidence
/// origin, and it is the rule that keeps the column honest: a vendor classifier
/// that decided out of sight is *nobody looked*, which is a different fact from
/// `allow` and must not collapse into it.
///
/// **`Daemon` is absent, and the commonest row on a real machine is one.**
/// Measured on this repository's own log: 162 pre-execution decisions, of which
/// **150 are `daemon`/`unresolved`** — Devplane declining to decide and handing
/// the call to whoever is at the keyboard. That is not an authority. Nobody has
/// decided yet, and the person may well have answered in the vendor's own
/// dialog where this product never saw it.
///
/// Writing `daemon` there would be claiming a decider for a call nobody has
/// decided; writing `nobody` would be claiming it was abandoned. Absent is the
/// only honest value, and it is also the sharpest illustration of what #535's
/// `require_approval` is missing: **a suspension with no resolution**. The
/// schema can say a call was held and cannot say whether anybody ever came.
pub fn authority_of(d: &Decision) -> Option<&'static str> {
    match d.authority {
        Authority::Person | Authority::Rule | Authority::Timer | Authority::Nobody => {
            Some(d.authority.as_str())
        }
        Authority::Daemon => None,
    }
}

/// One decision as an OTLP/JSON log record carrying #535's event.
///
/// OTLP/JSON rather than a bespoke shape, because the point is that a collector
/// can read it: this is the format Claude Code's own exporter sends and the
/// format this daemon already parses on the way in.
pub fn record(d: &Decision) -> Option<serde_json::Value> {
    let outcome = Outcome::of(d)?;
    let mut attributes = vec![
        serde_json::json!({
            "key": OUTCOME_KEY,
            "value": {"stringValue": outcome.as_str()}
        }),
        serde_json::json!({
            "key": "gen_ai.tool.call.id",
            "value": {"stringValue": d.id}
        }),
    ];
    if let Some(tool) = d.tool.as_deref() {
        attributes.push(serde_json::json!({
            "key": "gen_ai.tool.name",
            "value": {"stringValue": tool}
        }));
    }
    // **The attribute the proposal drops.** Absent where it cannot be known.
    if let Some(a) = authority_of(d) {
        attributes.push(serde_json::json!({
            "key": AUTHORITY_KEY,
            "value": {"stringValue": a}
        }));
    }
    Some(serde_json::json!({
        "timeUnixNano": d.at.as_nanosecond().to_string(),
        "eventName": EVENT,
        "attributes": attributes,
    }))
}

/// A whole decision log as one OTLP/JSON `resourceLogs` payload.
pub fn payload(decisions: &[Decision]) -> serde_json::Value {
    let records: Vec<_> = decisions.iter().filter_map(record).collect();
    serde_json::json!({
        "resourceLogs": [{
            "resource": {"attributes": [
                {"key": "service.name", "value": {"stringValue": "devplane"}}
            ]},
            "scopeLogs": [{
                "scope": {"name": "devplane"},
                "logRecords": records
            }]
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(authority: Authority, outcome: &str, tool: Option<&str>) -> Decision {
        let mut x = Decision::new(authority, "agent:tool.use", "cat a", outcome);
        x.tool = tool.map(str::to_string);
        x
    }

    /// **No `allow` exists that no recorded human selection stands behind.**
    ///
    /// The absence test, over every authority. This product has no
    /// `Verdict::Allow` and cannot produce an approval from a rule, a clock, a
    /// default or a model — so exporting one would be this project making the
    /// exact error it is reviewing in somebody else's schema.
    #[test]
    fn only_a_person_can_produce_an_allow() {
        for a in [
            Authority::Rule,
            Authority::Timer,
            Authority::Nobody,
            Authority::Daemon,
        ] {
            assert_ne!(
                Outcome::of(&d(a, "allow", Some("Bash"))),
                Some(Outcome::Allow),
                "{} produced an allow this product cannot stand behind",
                a.as_str()
            );
        }
        assert_eq!(
            Outcome::of(&d(Authority::Person, "allow", Some("Bash"))),
            Some(Outcome::Allow)
        );
    }

    /// An authority that cannot be known is **unset**, never defaulted.
    #[test]
    fn an_unknowable_authority_is_absent_rather_than_flattering() {
        let r = record(&d(Authority::Daemon, "deny", Some("Bash"))).expect("a deny is an event");
        let keys: Vec<&str> = r["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["key"].as_str().unwrap())
            .collect();
        assert!(
            !keys.contains(&AUTHORITY_KEY),
            "an authority nobody can establish was given a value: {keys:?}"
        );
        // And the ones that *are* knowable carry it.
        for a in [
            Authority::Person,
            Authority::Rule,
            Authority::Timer,
            Authority::Nobody,
        ] {
            let r = record(&d(a, "deny", Some("Bash"))).expect("a deny is an event");
            let has = r["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["key"] == AUTHORITY_KEY && x["value"]["stringValue"] == a.as_str());
            assert!(has, "{} was not carried", a.as_str());
        }
    }

    /// **Not every decision is this event**, and #535 says so itself.
    #[test]
    fn a_row_that_is_not_a_pre_execution_tool_decision_is_not_emitted() {
        // A gate verdict: no tool, and a post-execution outcome.
        assert!(record(&d(Authority::Daemon, "pass", None)).is_none());
        assert!(record(&d(Authority::Daemon, "done", None)).is_none());
        // A tool row whose outcome this event has no member for.
        assert!(record(&d(Authority::Rule, "undecided", Some("Bash"))).is_none());
    }

    /// The three outcomes map from what this product can decide.
    #[test]
    fn the_outcomes_map_from_this_products_own_vocabulary() {
        assert_eq!(
            Outcome::of(&d(Authority::Rule, "deny", Some("Bash"))),
            Some(Outcome::Deny)
        );
        for o in ["ask", "unresolved"] {
            assert_eq!(
                Outcome::of(&d(Authority::Rule, o, Some("Bash"))),
                Some(Outcome::RequireApproval),
                "`{o}` is a call suspended pending somebody"
            );
        }
    }

    /// **The private prefix is defined once**, so a normative name retires it in
    /// one edit rather than in a search.
    #[test]
    fn the_authority_attribute_is_named_in_exactly_one_place() {
        let src = include_str!("genai.rs");
        let literal = src
            .matches("\"devplane.gen_ai.tool.call.decision.authority\"")
            .count();
        assert_eq!(
            literal, 1,
            "the private attribute name is written {literal} times; it is a const for a reason"
        );
        assert!(
            !AUTHORITY_KEY.starts_with("gen_ai."),
            "a producer may not mint a normative name"
        );
    }

    /// Prompt and message content never reach this path.
    #[test]
    fn nothing_here_carries_content() {
        let mut x = d(Authority::Rule, "deny", Some("Bash"));
        x.reason = Some("the rule text, which is not exported".into());
        x.subject = "rm -rf /secret-project".into();
        let r = record(&x).unwrap();
        let body = serde_json::to_string(&r).unwrap();
        assert!(!body.contains("secret-project"), "the subject was exported");
        assert!(!body.contains("rule text"), "the reason was exported");
    }
}

#[cfg(test)]
mod measured {
    use super::*;
    use crate::core::decision::{Authority, Decision};

    /// **The shape of a real machine's log, which is the argument.**
    ///
    /// Measured on this repository on 2026-09-23: 162 pre-execution decisions —
    /// 152 `require_approval`, 10 `deny`, and **no `allow` at all**, because
    /// this product cannot produce an approval. 150 of the 152 carry no
    /// authority, because nobody has decided them yet.
    ///
    /// Under #535 as proposed, every one of those 162 events is a tool name and
    /// an outcome. A fleet where a classifier approved everything and a fleet
    /// where an engineer approved everything are the same telemetry — which is
    /// precisely the distinction its own motivation, *incident and security
    /// investigation*, is asked for.
    ///
    /// This test pins the shape rather than the numbers: the numbers are a fact
    /// about one machine on one day and belong in the notes.
    #[test]
    fn a_held_call_carries_no_authority_and_no_resolution() {
        let mut held = Decision::new(Authority::Daemon, "agent:tool.use", "cat a", "unresolved");
        held.tool = Some("Bash".into());

        let r = record(&held).expect("a held call is a pre-execution decision");
        let attrs: Vec<&str> = r["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["key"].as_str().unwrap())
            .collect();

        assert_eq!(
            r["attributes"][0]["value"]["stringValue"], "require_approval",
            "a call Devplane declined to decide is suspended pending somebody"
        );
        assert!(
            !attrs.contains(&AUTHORITY_KEY),
            "a call nobody has decided was given a decider"
        );
        // **And there is nowhere to say whether anybody ever came.** #535's
        // `require_approval` has no resolution, which is #445's dropped
        // deadline and resolution fields under a second name, in a second
        // thread that does not cite the first. Recorded here as an absence so
        // that the day a resolution exists, this test is what fails.
        assert!(
            !attrs.iter().any(|k| k.contains("resolution")),
            "a resolution attribute appeared; the notes and the comment on #535 \
             both say there is none, and one of the three is now wrong"
        );
    }
}

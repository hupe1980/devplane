//! The decision log rendered in the OpenTelemetry GenAI conventions' shape.
//!
//! A renderer, not an exporter: Devplane receives OTLP and sends none, so this
//! prints existing rows to stdout when a person runs a command. The proposed
//! `gen_ai.tool.call.decision` event (semantic-conventions-genai #535) has no
//! attribute for *who* decided; that rides under a private prefix,
//! [`AUTHORITY_KEY`]. Prompt and message content is never emitted.

use crate::core::decision::{Authority, Decision};

/// The event name #535 proposes.
pub const EVENT: &str = "gen_ai.tool.call.decision";

/// The outcome attribute #535 proposes, with its three members.
pub const OUTCOME_KEY: &str = "gen_ai.tool.call.decision.outcome";

/// Who decided — absent from the proposal. Application-prefixed so nothing here
/// mints a normative name; defined once so adopting one is a single edit.
pub const AUTHORITY_KEY: &str = "devplane.gen_ai.tool.call.decision.authority";

/// Set for `allow_always` / `reject_always`: every later call it covers is
/// decided inside the agent and never reaches this log, so it must not be
/// flattened into a one-off.
pub const STANDING_KEY: &str = "devplane.gen_ai.tool.call.decision.standing";

/// The proposed outcome for a recorded decision. There is no `Verdict::Allow`,
/// so `allow` comes only from a recorded human selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A recorded human selection, carried.
    Allow,
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

    /// `None` where the row is not a pre-execution tool-call decision (gate
    /// verdicts, merges) — the common case, and out of scope for the event.
    pub fn of(d: &Decision) -> Option<Self> {
        d.tool.as_deref()?;
        match d.outcome.as_str() {
            "deny" | "reject_always" => Some(Outcome::Deny),
            // `chosen`: an option whose meaning the agent never declared, so
            // it claims neither yes nor no.
            "ask" | "unresolved" | "chosen" => Some(Outcome::RequireApproval),
            // Only a person's decision may be exported as an approval.
            "allow" | "allow_always" if d.authority == Authority::Person => Some(Outcome::Allow),
            _ => None,
        }
    }
}

fn is_standing(d: &Decision) -> bool {
    matches!(d.outcome.as_str(), "allow_always" | "reject_always")
}

/// The authority, or `None` when it cannot be known — unset, never defaulted.
/// `Devplane` means Devplane declined to decide and handed the call on; the
/// person may have answered in the vendor's dialog unseen, so neither
/// `devplane` nor `nobody` would be true.
pub fn authority_of(d: &Decision) -> Option<&'static str> {
    match d.authority {
        Authority::Person | Authority::Rule | Authority::Timer | Authority::Nobody => {
            Some(d.authority.as_str())
        }
        Authority::Devplane => None,
    }
}

/// One decision as an OTLP/JSON log record, so any collector can read it.
pub fn record(d: &Decision) -> Option<serde_json::Value> {
    let outcome = Outcome::of(d)?;
    // No `gen_ai.tool.call.id`: that is the model's id for the call, which
    // the hook channel never carries; a Devplane row id is not it.
    let mut attributes = vec![serde_json::json!({
        "key": OUTCOME_KEY,
        "value": {"stringValue": outcome.as_str()}
    })];
    if is_standing(d) {
        attributes.push(serde_json::json!({
            "key": STANDING_KEY,
            "value": {"boolValue": true}
        }));
    }
    if let Some(tool) = d.tool.as_deref() {
        attributes.push(serde_json::json!({
            "key": "gen_ai.tool.name",
            "value": {"stringValue": tool}
        }));
    }
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

    /// No `allow` exists that no recorded human selection stands behind.
    #[test]
    fn only_a_person_can_produce_an_allow() {
        for a in [
            Authority::Rule,
            Authority::Timer,
            Authority::Nobody,
            Authority::Devplane,
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

    #[test]
    fn an_unknowable_authority_is_absent_rather_than_flattering() {
        let r = record(&d(Authority::Devplane, "deny", Some("Bash"))).expect("a deny is an event");
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

    #[test]
    fn a_row_that_is_not_a_pre_execution_tool_decision_is_not_emitted() {
        // A gate verdict: no tool, and a post-execution outcome.
        assert!(record(&d(Authority::Devplane, "pass", None)).is_none());
        assert!(record(&d(Authority::Devplane, "done", None)).is_none());
        // A tool row whose outcome this event has no member for.
        assert!(record(&d(Authority::Rule, "undecided", Some("Bash"))).is_none());
    }

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

    #[test]
    fn a_persons_standing_choice_is_exported_as_standing() {
        let always = record(&d(Authority::Person, "allow_always", Some("Bash")))
            .expect("a standing grant is a pre-execution decision");
        let attrs = always["attributes"].as_array().unwrap();
        assert_eq!(attrs[0]["value"]["stringValue"], "allow");
        assert!(
            attrs
                .iter()
                .any(|a| a["key"] == STANDING_KEY && a["value"]["boolValue"] == true),
            "a standing grant exported as a one-off: {attrs:?}"
        );

        let never = record(&d(Authority::Person, "reject_always", Some("Bash"))).unwrap();
        assert_eq!(never["attributes"][0]["value"]["stringValue"], "deny");
        assert!(is_standing(&d(
            Authority::Person,
            "reject_always",
            Some("Bash")
        )));

        // A one-off carries no standing attribute at all, rather than `false`.
        let once = record(&d(Authority::Person, "allow", Some("Bash"))).unwrap();
        assert!(
            !once["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["key"] == STANDING_KEY),
            "a one-off was marked"
        );

        // `chosen` claims nothing about yes or no, and so does its export.
        let chosen = Outcome::of(&d(Authority::Person, "chosen", Some("Bash")));
        assert_eq!(chosen, Some(Outcome::RequireApproval));
        // And a standing grant is still only a person's to make.
        assert_ne!(
            Outcome::of(&d(Authority::Rule, "allow_always", Some("Bash"))),
            Some(Outcome::Allow)
        );
    }

    #[test]
    fn the_decision_id_is_not_exported_as_the_models_call_id() {
        let r = record(&d(Authority::Rule, "deny", Some("Bash"))).unwrap();
        assert!(
            !r["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["key"] == "gen_ai.tool.call.id"),
            "{r}"
        );
    }

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

    /// A held call carries no authority, and the proposal has no attribute for
    /// whether it was ever resolved.
    #[test]
    fn a_held_call_carries_no_authority_and_no_resolution() {
        let mut held = Decision::new(Authority::Devplane, "agent:tool.use", "cat a", "unresolved");
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
        // Pinned as an absence: if a resolution attribute appears, this fails.
        assert!(
            !attrs.iter().any(|k| k.contains("resolution")),
            "a resolution attribute appeared; the notes and the comment on #535 \
             both say there is none, and one of the three is now wrong"
        );
    }
}

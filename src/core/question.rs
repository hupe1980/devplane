//! A question an agent asked the person, and the person's answer to it.
//!
//! **This is not a permission.** A permission asks whether an action is
//! allowed; a question asks which of several things the person wants, and no
//! rule can answer it. The two arrive on different channels and only one of
//! them has a policy: `session/request_permission` carries the first and
//! `elicitation/create` carries this.
//!
//! **The shape here was measured, not read.** The vendor's hook documentation
//! describes a different payload entirely — the one a `PreToolUse` hook sees —
//! and the ACP adapter renders the same tool as a *form elicitation* whose
//! schema carries option descriptions, several questions at once, and a
//! free-text box that the hook shape has no room for. Every field below came off
//! the wire on 2026-09-19 against `claude-agent-acp@0.76`, and the fixture in
//! the tests is that capture byte for byte.
//!
//! Two properties of the adapter's own answer-folding decide the API, and both
//! are the opposite of the obvious guess:
//!
//! * **A typed custom answer beats the selection.** If the person writes in the
//!   *Other* box, that is the answer, whatever radio button is also set.
//! * **Declining is not refusing.** The adapter turns a decline into
//!   *answered, with no answers* — the agent proceeds having asked and heard
//!   nothing, which is the exact failure this product exists to prevent. The
//!   refusal that stops the call is **cancel**. Nothing here may ever decline.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One thing the person may choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    /// What goes back on the wire.
    pub value: String,
    /// What the person reads.
    pub label: String,
    /// The agent's own sentence about what this option means, where it wrote
    /// one. Rendered, never summarised: it is the difference between *Drop it*
    /// and *Drop it — removes the legacy route*.
    pub detail: Option<String>,
}

/// One question, with everything the person needs to answer it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    /// The schema property this answer goes back under — `question_0`.
    pub field: String,
    /// The agent's own heading for it.
    pub title: String,
    pub options: Vec<Choice>,
    /// The free-text field, where the agent offered one. **A product that
    /// renders only `options` is showing a smaller question than was asked.**
    pub custom_field: Option<String>,
}

/// A question the agent is waiting on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub session_id: String,
    /// The agent's id for the call, where it bridged one.
    ///
    /// **Optional, and the fixture is why.** The Claude adapter sends it
    /// because it is rendering a *tool call* as a form; an elicitation that did
    /// not come from a tool has nothing to put here, and the protocol does not
    /// require it. Requiring it made `parse` reject such a question outright —
    /// so a question with no tool behind it was dropped as unrenderable, which
    /// is the failure this whole module exists to prevent, one layer down.
    pub tool_call_id: Option<String>,
    /// The question as the agent phrased it.
    pub message: String,
    pub questions: Vec<Question>,
}

/// What the person chose for one question: an option, or their own words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Chosen {
    Option(String),
    Custom(String),
}

/// The marker the adapter puts on a free-text field so it can be told from a
/// question. Matched rather than inferred from the name, because a question
/// legitimately called `question_0_custom` would otherwise disappear.
const CUSTOM_MARKER: &str = "_askUserQuestionCustomAnswer";

fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

impl Ask {
    /// Reads an `elicitation/create` payload, or `None` if it is not a question
    /// this can present.
    ///
    /// **`None` is a real answer and must not be treated as an error.** The
    /// same method carries elicitations from MCP servers, which have their own
    /// shapes; one this cannot render is one to hand back untouched rather than
    /// to guess at.
    pub fn parse(v: &Value) -> Option<Ask> {
        if v.get("mode").and_then(Value::as_str) != Some("form") {
            return None;
        }
        let props = v.get("requestedSchema")?.get("properties")?.as_object()?;

        // Custom fields first, so a question knows whether it has one. The
        // marker says which question it belongs to; the name is not evidence.
        let mut custom_for: std::collections::HashMap<String, String> = Default::default();
        for (name, spec) in props {
            if let Some(meta) = spec.get("_meta").and_then(|m| m.get(CUSTOM_MARKER))
                && let Some(owner) = str_of(meta, "questionId")
            {
                custom_for.insert(owner, name.clone());
            }
        }

        let mut questions = Vec::new();
        for (name, spec) in props {
            if spec
                .get("_meta")
                .and_then(|m| m.get(CUSTOM_MARKER))
                .is_some()
            {
                continue;
            }
            let Some(one_of) = spec.get("oneOf").and_then(Value::as_array) else {
                continue;
            };
            let options: Vec<Choice> = one_of
                .iter()
                .filter_map(|o| {
                    let value = str_of(o, "const")?;
                    Some(Choice {
                        label: str_of(o, "title").unwrap_or_else(|| value.clone()),
                        detail: str_of(o, "description"),
                        value,
                    })
                })
                .collect();
            if options.is_empty() {
                continue;
            }
            questions.push(Question {
                title: str_of(spec, "title").unwrap_or_else(|| name.clone()),
                custom_field: custom_for.get(name).cloned(),
                field: name.clone(),
                options,
            });
        }
        if questions.is_empty() {
            return None;
        }
        // The map iterates arbitrarily; `question_0` must come before
        // `question_10`, so sort by the number where there is one.
        questions.sort_by_key(|q| {
            (
                q.field
                    .rsplit('_')
                    .next()
                    .and_then(|n| n.parse::<u32>().ok())
                    .unwrap_or(u32::MAX),
                q.field.clone(),
            )
        });

        Some(Ask {
            session_id: str_of(v, "sessionId")?,
            tool_call_id: str_of(v, "toolCallId"),
            message: str_of(v, "message").unwrap_or_default(),
            questions,
        })
    }

    /// The `content` map for an accepted answer.
    ///
    /// A question with no entry is left out rather than sent empty: the adapter
    /// drops an empty string, and an explicit blank would read as an answer.
    pub fn content(&self, answers: &[(String, Chosen)]) -> Value {
        let mut out = serde_json::Map::new();
        for (field, chosen) in answers {
            let Some(q) = self.questions.iter().find(|q| &q.field == field) else {
                continue;
            };
            match chosen {
                Chosen::Option(v) => {
                    // Only an option the agent offered. Anything else is this
                    // product inventing an answer, which is the one thing it
                    // may never do.
                    if q.options.iter().any(|o| &o.value == v) {
                        out.insert(q.field.clone(), Value::String(v.clone()));
                    }
                }
                Chosen::Custom(text) => {
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    if let Some(cf) = &q.custom_field {
                        out.insert(cf.clone(), Value::String(text.to_string()));
                    }
                }
            }
        }
        Value::Object(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 2026-09-19 capture, byte for byte. Every expectation below is about
    /// what the adapter actually sent, not about what its documentation says.
    fn captured() -> Value {
        serde_json::json!({
          "mode": "form",
          "sessionId": "b3e0eb7f-3638-4357-a181-f678c987ad57",
          "toolCallId": "toolu_01DciGAoKow3W1N2k7aLFCMo",
          "requestedSchema": { "type": "object", "properties": {
            "question_0": { "type": "string", "title": "/v1/login", "oneOf": [
              { "const": "Keep it", "title": "Keep it",
                "description": "Retain the legacy /v1/login route as-is." },
              { "const": "Drop it", "title": "Drop it",
                "description": "Remove the legacy /v1/login route." } ] },
            "question_0_custom": { "type": "string", "title": "Other",
              "description": "Type your own answer instead of choosing an option above (optional).",
              "_meta": { "_askUserQuestionCustomAnswer":
                { "questionId": "question_0", "isCustomAnswer": true } } } } },
          "message": "Should the legacy /v1/login route be kept or dropped?"
        })
    }

    #[test]
    fn it_reads_the_question_the_agent_actually_sent() {
        let a = Ask::parse(&captured()).expect("the capture parses");
        assert_eq!(
            a.tool_call_id.as_deref(),
            Some("toolu_01DciGAoKow3W1N2k7aLFCMo")
        );
        assert_eq!(
            a.message,
            "Should the legacy /v1/login route be kept or dropped?"
        );
        assert_eq!(a.questions.len(), 1);
        let q = &a.questions[0];
        assert_eq!(q.title, "/v1/login");
        assert_eq!(q.options.len(), 2);
        assert_eq!(q.options[0].value, "Keep it");
    }

    #[test]
    fn an_options_own_description_survives() {
        // The field the first data model had no room for. Losing it renders
        // "Drop it" where the agent wrote "Drop it — removes the legacy route".
        let a = Ask::parse(&captured()).unwrap();
        assert_eq!(
            a.questions[0].options[1].detail.as_deref(),
            Some("Remove the legacy /v1/login route.")
        );
    }

    #[test]
    fn the_free_text_box_is_found_by_its_marker_not_its_name() {
        let a = Ask::parse(&captured()).unwrap();
        assert_eq!(
            a.questions[0].custom_field.as_deref(),
            Some("question_0_custom")
        );
        // And it is not mistaken for a question of its own.
        assert_eq!(a.questions.len(), 1);
    }

    #[test]
    fn a_custom_answer_is_sent_under_the_custom_field() {
        let a = Ask::parse(&captured()).unwrap();
        let c = a.content(&[(
            "question_0".into(),
            Chosen::Custom("  keep it behind a flag  ".into()),
        )]);
        assert_eq!(c["question_0_custom"], "keep it behind a flag");
        assert!(
            c.get("question_0").is_none(),
            "a custom answer is not also a selection"
        );
    }

    #[test]
    fn an_option_nobody_offered_is_refused() {
        // The product may carry an answer; it may never invent one.
        let a = Ask::parse(&captured()).unwrap();
        let c = a.content(&[("question_0".into(), Chosen::Option("Maybe".into()))]);
        assert_eq!(c.as_object().map(|o| o.len()), Some(0));
    }

    #[test]
    fn an_empty_custom_answer_is_not_an_answer() {
        let a = Ask::parse(&captured()).unwrap();
        let c = a.content(&[("question_0".into(), Chosen::Custom("   ".into()))]);
        assert_eq!(c.as_object().map(|o| o.len()), Some(0));
    }

    #[test]
    fn questions_keep_the_order_the_agent_asked_them_in() {
        let mut v = captured();
        let props = v["requestedSchema"]["properties"].as_object_mut().unwrap();
        for n in [2u32, 10, 1] {
            props.insert(
                format!("question_{n}"),
                serde_json::json!({ "title": format!("q{n}"),
                    "oneOf": [{ "const": "y", "title": "y" }] }),
            );
        }
        let a = Ask::parse(&v).unwrap();
        let fields: Vec<&str> = a.questions.iter().map(|q| q.field.as_str()).collect();
        assert_eq!(
            fields,
            ["question_0", "question_1", "question_2", "question_10"]
        );
    }

    /// A question with no tool call behind it is still a question.
    ///
    /// Found by the fixture: it sends the captured payload without a
    /// `toolCallId`, because it is not bridging a tool — and `parse` rejected
    /// the whole thing, so a perfectly renderable question was reported as one
    /// this client could not show.
    #[test]
    fn a_question_from_no_tool_call_is_still_a_question() {
        let mut v = captured();
        v.as_object_mut().unwrap().remove("toolCallId");
        let a = Ask::parse(&v).expect("a question without a tool call still parses");
        assert!(a.tool_call_id.is_none());
        assert_eq!(a.questions.len(), 1);
    }

    #[test]
    fn an_elicitation_this_cannot_render_is_handed_back_rather_than_guessed_at() {
        // MCP servers use the same method with their own shapes. `None` means
        // "not mine", and must never become an empty question.
        assert!(Ask::parse(&serde_json::json!({ "mode": "url" })).is_none());
        assert!(
            Ask::parse(&serde_json::json!({
                "mode": "form", "sessionId": "s", "toolCallId": "t",
                "requestedSchema": { "properties": { "name": { "type": "string" } } }
            }))
            .is_none(),
            "a plain text field is not a question with options"
        );
    }
}

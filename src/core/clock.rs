//! Whose clock can answer a question in your name, and where it was set.
//!
//! **The authority column pointed at somebody else's timer.** Every `timer` row
//! this product writes is about its own clock; this is the first fact it can
//! report about one it did not set:
//!
//! > `askUserQuestionTimeout` — *"Let an unanswered `AskUserQuestion` dialog
//! > auto-continue after a period of idle time, **submitting whatever options
//! > you had already selected**… With the default, questions wait until you
//! > answer them."*
//!
//! It is off unless somebody turns it on and permission prompts are exempt, so
//! nothing here may imply a hole in the vendor's design. What makes it worth
//! reporting is its scope — **user or managed** — because an administrator can
//! set one and the vendor's own `/config` hides the row while they have.
//!
//! Read, never written: how long a person gets to answer is not this product's
//! decision to take.
//!
//! **What it cannot see is said rather than implied.** Managed settings also
//! compose from drop-ins, a policy helper and a Windows registry chain; this
//! reads the two documented files, so an absent timer means *nothing in the
//! files I read*.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The settings key, as the vendor spells it.
pub const KEY: &str = "askUserQuestionTimeout";

/// Which file set it.
///
/// **`Managed` is the interesting one**, and it is the reason this enum exists
/// rather than a bare `Option<String>`: *you chose this* and *somebody else
/// chose this for you* are different facts about the same duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "ClockSource", export, export_to = "wire/")
)]
pub enum Source {
    /// The person's own `~/.claude/settings.json`.
    User,
    /// Deployed by whoever administers this machine. The vendor's own settings
    /// UI hides the row while this is in force.
    Managed,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::User => "user",
            Source::Managed => "managed",
        }
    }
}

/// What can answer a question on this machine without the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct QuestionClock {
    /// The duration, in the vendor's own spelling — `60s`, `5m`, `10m`.
    pub after: String,
    pub source: Source,
    /// The file it came from, so a person can go and look at it.
    pub file: String,
}

impl QuestionClock {
    /// One sentence, and the managed case is deliberately the louder one.
    pub fn says(&self) -> String {
        match self.source {
            Source::User => format!(
                "you set a {} timer on your own questions — after that, whatever is selected is \
                 submitted",
                self.after
            ),
            Source::Managed => format!(
                "a {} timer on your questions was set for you, in managed settings — after that, \
                 whatever is selected is submitted, and your own settings cannot turn it off",
                self.after
            ),
        }
    }

    /// Whether the person at the keyboard chose this.
    pub fn chosen_by_the_person(&self) -> bool {
        matches!(self.source, Source::User)
    }
}

/// Reads the timer out of the two settings documents, managed winning.
///
/// **Managed wins because the vendor says it does**, and because the direction
/// of that precedence is the whole reason this is worth reporting: a person
/// cannot turn off a timer their administrator set, and a surface that showed
/// them their own value would be showing them a number that is not in force.
///
/// A value that is not a string is ignored rather than rendered: the setting
/// takes `60s`, `5m` or `10m`, and anything else is the vendor's to complain
/// about at startup, not this product's to guess at.
pub fn question_clock(
    user: &Map<String, Value>,
    user_file: &str,
    managed: &Map<String, Value>,
    managed_file: &str,
) -> Option<QuestionClock> {
    let read = |m: &Map<String, Value>| {
        m.get(KEY)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    if let Some(after) = read(managed) {
        return Some(QuestionClock {
            after,
            source: Source::Managed,
            file: managed_file.to_string(),
        });
    }
    read(user).map(|after| QuestionClock {
        after,
        source: Source::User,
        file: user_file.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(json: serde_json::Value) -> Map<String, Value> {
        json.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn nothing_set_is_nothing_claimed() {
        assert_eq!(
            question_clock(
                &map(serde_json::json!({})),
                "u",
                &map(serde_json::json!({})),
                "m"
            ),
            None
        );
    }

    #[test]
    fn the_persons_own_timer_is_reported_as_theirs() {
        let c = question_clock(
            &map(serde_json::json!({ KEY: "5m" })),
            "~/.claude/settings.json",
            &map(serde_json::json!({})),
            "m",
        )
        .expect("a timer");
        assert_eq!(c.source, Source::User);
        assert!(c.chosen_by_the_person());
        assert!(c.says().starts_with("you set a 5m timer"), "{}", c.says());
    }

    /// The case the whole module exists for: somebody else's timer, on your
    /// questions, that your own settings cannot turn off.
    #[test]
    fn a_managed_timer_wins_and_says_it_was_set_for_you() {
        let c = question_clock(
            &map(serde_json::json!({ KEY: "10m" })),
            "~/.claude/settings.json",
            &map(serde_json::json!({ KEY: "60s" })),
            "/Library/Application Support/ClaudeCode/managed-settings.json",
        )
        .expect("a timer");
        assert_eq!(c.source, Source::Managed);
        assert_eq!(
            c.after, "60s",
            "managed wins, because the vendor says it does"
        );
        assert!(!c.chosen_by_the_person());
        assert!(c.says().contains("was set for you"), "{}", c.says());
        assert!(
            c.says().contains("cannot turn it off"),
            "the person is told the precedence, not just the number: {}",
            c.says()
        );
    }

    /// A shape the setting cannot take is not rendered as though it could.
    #[test]
    fn a_value_that_is_not_a_duration_string_is_ignored() {
        for bad in [
            serde_json::json!({ KEY: 300 }),
            serde_json::json!({ KEY: true }),
            serde_json::json!({ KEY: "" }),
            serde_json::json!({ KEY: "   " }),
            serde_json::json!({ KEY: serde_json::Value::Null }),
        ] {
            assert_eq!(
                question_clock(&map(bad.clone()), "u", &map(serde_json::json!({})), "m"),
                None,
                "{bad}"
            );
        }
    }

    /// The two sentences have to be told apart at a glance, because one of them
    /// is a decision somebody else took about this person's attention.
    #[test]
    fn the_two_sentences_do_not_read_alike() {
        let mine = QuestionClock {
            after: "5m".into(),
            source: Source::User,
            file: "u".into(),
        };
        let theirs = QuestionClock {
            after: "5m".into(),
            source: Source::Managed,
            file: "m".into(),
        };
        assert_ne!(mine.says(), theirs.says());
    }

    #[test]
    fn the_wire_spellings_are_what_serde_writes() {
        for s in [Source::User, Source::Managed] {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.as_str())
            );
        }
    }
}

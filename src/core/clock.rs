//! Whose clock can answer a question in your name, and where it was set: the
//! vendor's `askUserQuestionTimeout` (user or managed settings, managed winning)
//! and `CLAUDE_AFK_TIMEOUT_MS`, which outranks both files and turns
//! auto-continue on even where they say `never`. The variable is read per
//! session from the environment `devplane hook` inherits. Read, never written.
//! Only the two documented files are read, so an absent timer means *nothing in
//! the files read and no variable in the sessions seen*, not *no timer*.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The settings key, as the vendor spells it.
pub const KEY: &str = "askUserQuestionTimeout";

/// Who set it. *You chose this* and *somebody chose this for you* are different
/// facts about the same duration.
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
    /// Deployed by whoever administers this machine; the vendor's `/config`
    /// hides the row while it is in force.
    Managed,
    /// `CLAUDE_AFK_TIMEOUT_MS` in the session's environment. Outranks both
    /// files and turns auto-continue on even where a file says `never`.
    Environment,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::User => "user",
            Source::Managed => "managed",
            Source::Environment => "environment",
        }
    }
}

/// How long a question waits before the clock answers it. `Immediately` is its
/// own variant: `CLAUDE_AFK_TIMEOUT_MS=0` closes the dialog at once, which a
/// surface must say in words, not as `after 0s`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum After {
    /// The dialog closes the instant it opens.
    Immediately,
    /// Idle time, in the vendor's own spelling — `60s`, `5m`, `10m`.
    Idle(String),
    /// `never` — the setting's default and one of its documented values. Kept
    /// distinct from absence: a `never` overridden by the environment must
    /// still be visible, and it must never read as a timer.
    Never,
}

impl After {
    /// The duration as a person would say it, or the word for the wait it is.
    pub fn says(&self) -> &str {
        match self {
            After::Immediately => "immediately",
            After::Idle(d) => d,
            After::Never => "never",
        }
    }

    /// Whether this ends a question without the person — the predicate every
    /// surface branches on; false only for `never`.
    pub fn answers_for_you(&self) -> bool {
        !matches!(self, After::Never)
    }
}

/// What can answer a question on this machine without the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct QuestionClock {
    pub after: After,
    pub source: Source,
    /// Where to change it: a settings path, or the variable's name.
    pub where_set: String,
}

impl QuestionClock {
    /// One sentence, distinct per case. `Immediately` comes first: it means the
    /// product's central promise is untrue in that session.
    pub fn says(&self) -> String {
        match (&self.after, self.source) {
            (After::Immediately, _) => "questions in this session are closed immediately — nobody \
                 is asked, and whatever happens to be selected is submitted"
                .to_string(),
            // The reassuring answer, still printed because somebody set it;
            // never coloured as an alarm.
            (After::Never, Source::User) => {
                "you set your own questions to wait until you answer them".to_string()
            }
            (After::Never, Source::Managed) => {
                "managed settings say your questions wait until you \
                 answer them — your own settings cannot turn that off either"
                    .to_string()
            }
            // Unreachable through `from_millis`; answered with a true sentence
            // rather than a panic.
            (After::Never, Source::Environment) => {
                "this session's environment sets no timer on its questions".to_string()
            }
            (After::Idle(d), Source::User) => format!(
                "you set a {d} timer on your own questions — after that, whatever is selected is \
                 submitted"
            ),
            (After::Idle(d), Source::Managed) => format!(
                "a {d} timer on your questions was set for you, in managed settings — after that, \
                 whatever is selected is submitted, and your own settings cannot turn it off"
            ),
            (After::Idle(d), Source::Environment) => format!(
                "this session was started with a {d} timer on its questions, from its environment \
                 — it overrides your settings, including `never`"
            ),
        }
    }

    /// Whether the person at the keyboard chose this. The environment is not
    /// theirs by default: the vendor documents it for demos and automated tests.
    pub fn chosen_by_the_person(&self) -> bool {
        matches!(self.source, Source::User)
    }

    /// Whether this ends questions with no wait at all.
    pub fn is_immediate(&self) -> bool {
        matches!(self.after, After::Immediately)
    }

    /// Whether a clock answers for the person at all.
    pub fn answers_for_you(&self) -> bool {
        self.after.answers_for_you()
    }
}

/// Reads `CLAUDE_AFK_TIMEOUT_MS` in the vendor's spelling, so `devplane modes`
/// and `/config` agree. A non-number is ignored rather than guessed.
pub fn from_millis(raw: &str) -> Option<After> {
    let ms: u64 = raw.trim().parse().ok()?;
    if ms == 0 {
        return Some(After::Immediately);
    }
    // Seconds below two minutes to match the setting's `60s`/`5m`/`10m`; below
    // a second, milliseconds, since `0s` would read as `Immediately`.
    let d = match ms {
        ms if ms < 1_000 => format!("{ms}ms"),
        ms if ms < 120_000 && ms % 1_000 == 0 => format!("{}s", ms / 1_000),
        ms if ms % 3_600_000 == 0 => format!("{}h", ms / 3_600_000),
        ms if ms % 60_000 == 0 => format!("{}m", ms / 60_000),
        ms if ms % 1_000 == 0 => format!("{}s", ms / 1_000),
        ms => format!("{ms}ms"),
    };
    Some(After::Idle(d))
}

/// One clock as a surface reads it: the sentence is composed here so the CLI,
/// the board and `--json` render words, not ingredients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct ClockLine {
    /// The duration as a person would say it, or `immediately`.
    pub after: String,
    /// Whether questions close with no wait. A flag, not a string compare on
    /// `after`, so a rename cannot silently disable it.
    pub immediate: bool,
    pub source: Source,
    pub where_set: String,
    /// The whole sentence, which is what a surface prints.
    pub says: String,
    pub chosen_by_the_person: bool,
    /// Whether this clock answers for the person; `false` only for `never`.
    /// A flag for the same reason as `immediate`.
    pub answers_for_you: bool,
}

impl From<&QuestionClock> for ClockLine {
    fn from(c: &QuestionClock) -> Self {
        Self {
            after: c.after.says().to_string(),
            immediate: c.is_immediate(),
            source: c.source,
            where_set: c.where_set.clone(),
            says: c.says(),
            chosen_by_the_person: c.chosen_by_the_person(),
            answers_for_you: c.answers_for_you(),
        }
    }
}

/// The variable's name, which is where a person goes to change it.
pub const ENV_KEY: &str = "CLAUDE_AFK_TIMEOUT_MS";

/// A session's own clock, from the environment it was started in.
pub fn from_environment(raw: &str) -> Option<QuestionClock> {
    from_millis(raw).map(|after| QuestionClock {
        after,
        source: Source::Environment,
        where_set: ENV_KEY.to_string(),
    })
}

/// Reads the timer from the two settings documents. Managed wins, as the vendor
/// documents: a person cannot turn off a timer their administrator set. A
/// non-string value is ignored rather than rendered.
pub fn question_clock(
    user: &Map<String, Value>,
    user_file: &str,
    managed: &Map<String, Value>,
    managed_file: &str,
) -> Option<QuestionClock> {
    let read = |m: &Map<String, Value>| m.get(KEY).and_then(|v| v.as_str()).and_then(from_setting);
    if let Some(after) = read(managed) {
        return Some(QuestionClock {
            after,
            source: Source::Managed,
            where_set: managed_file.to_string(),
        });
    }
    read(user).map(|after| QuestionClock {
        after,
        source: Source::User,
        where_set: user_file.to_string(),
    })
}

/// Reads one `askUserQuestionTimeout` value (documented: `"60s"`, `"5m"`,
/// `"10m"` or `"never"`, default `"never"`). Durations are matched by shape so a
/// new vendor value is still reported; an unknown word is `None`, never a timer.
fn from_setting(raw: &str) -> Option<After> {
    let v = raw.trim();
    if v.eq_ignore_ascii_case("never") {
        return Some(After::Never);
    }
    let (digits, unit) = v.split_at(v.find(|c: char| !c.is_ascii_digit())?);
    match (digits.is_empty(), unit) {
        (false, "ms" | "s" | "m" | "h") => Some(After::Idle(v.to_string())),
        _ => None,
    }
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

    /// Somebody else's timer, which your own settings cannot turn off.
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
            c.after.says(),
            "60s",
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

    fn clock(source: Source) -> QuestionClock {
        QuestionClock {
            after: After::Idle("5m".into()),
            source,
            where_set: "somewhere".into(),
        }
    }

    #[test]
    fn no_two_sentences_read_alike() {
        let said: Vec<String> = [Source::User, Source::Managed, Source::Environment]
            .into_iter()
            .map(|s| clock(s).says())
            .collect();
        for (i, a) in said.iter().enumerate() {
            for b in said.iter().skip(i + 1) {
                assert_ne!(a, b, "two sources say the same thing");
            }
        }
        assert!(
            said[2].contains("overrides your settings"),
            "the environment's sentence must say it outranks the files: {}",
            said[2]
        );
    }

    #[test]
    fn the_wire_spellings_are_what_serde_writes() {
        for s in [Source::User, Source::Managed, Source::Environment] {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.as_str())
            );
        }
    }

    #[test]
    fn zero_milliseconds_is_immediately_and_never_a_duration() {
        assert_eq!(from_millis("0"), Some(After::Immediately));
        let c = from_environment("0").expect("a clock");
        assert!(c.is_immediate());
        let says = c.says();
        assert!(!says.contains("0s"), "{says}");
        assert!(says.contains("immediately"), "{says}");
        assert!(says.contains("nobody"), "{says}");
    }

    #[test]
    fn milliseconds_are_read_in_the_vendors_own_spelling() {
        for (raw, want) in [
            ("60000", "60s"),
            ("300000", "5m"),
            ("600000", "10m"),
            ("3600000", "1h"),
            ("1500", "1500ms"),
        ] {
            assert_eq!(from_millis(raw), Some(After::Idle(want.into())), "{raw}");
        }
    }

    #[test]
    fn a_value_that_is_not_a_number_is_ignored_rather_than_guessed() {
        for bad in ["", "   ", "soon", "5m", "-1", "60s", "1e3"] {
            assert_eq!(from_millis(bad), None, "{bad}");
            assert_eq!(from_environment(bad), None, "{bad}");
        }
    }

    /// `never` is a value, not an absence, and a clock that does not answer.
    #[test]
    fn never_is_a_clock_that_does_not_answer_for_you() {
        let c = question_clock(
            &map(serde_json::json!({ KEY: "never" })),
            "~/.claude/settings.json",
            &map(serde_json::json!({})),
            "m",
        )
        .expect("never is a value somebody set, not silence");

        assert_eq!(c.after, After::Never);
        assert!(
            !c.answers_for_you(),
            "the whole defect: a `never` clock does not answer for anybody"
        );
        assert!(!c.is_immediate());

        let says = c.says();
        assert!(
            says.contains("wait until you answer"),
            "the sentence has to say the questions wait: {says}"
        );
        for wrong in ["never timer", "whatever is selected is submitted"] {
            assert!(
                !says.contains(wrong),
                "this is what the surface said before the fix: {says}"
            );
        }
    }

    /// An administrator's `never` is hardening, not an alarm.
    #[test]
    fn a_managed_never_is_not_an_alarm() {
        let c = question_clock(
            &map(serde_json::json!({})),
            "u",
            &map(serde_json::json!({ KEY: "never" })),
            "/Library/Application Support/ClaudeCode/managed-settings.json",
        )
        .expect("a value");
        assert_eq!(c.source, Source::Managed);
        assert!(!c.answers_for_you());
        assert!(
            c.says().contains("wait until you"),
            "an administrator turning the timer off is good news: {}",
            c.says()
        );
    }

    #[test]
    fn a_duration_is_read_by_shape_and_a_word_is_not_guessed_at() {
        for good in ["60s", "5m", "10m", "30s", "2h", "500ms"] {
            let c = question_clock(
                &map(serde_json::json!({ KEY: good })),
                "u",
                &map(serde_json::json!({})),
                "m",
            )
            .unwrap_or_else(|| panic!("{good} is duration-shaped"));
            assert_eq!(c.after, After::Idle(good.into()));
            assert!(c.answers_for_you(), "{good}");
        }
        // Not a duration and not `never`: nothing is claimed.
        for bad in ["soon", "off", "disabled", "5 minutes", "m5", "-1", "", "  "] {
            assert_eq!(
                question_clock(
                    &map(serde_json::json!({ KEY: bad })),
                    "u",
                    &map(serde_json::json!({})),
                    "m"
                ),
                None,
                "{bad}"
            );
        }
    }

    /// Adding a variant forces a decision on this predicate.
    #[test]
    fn every_wait_says_whether_it_answers_for_you() {
        assert!(After::Immediately.answers_for_you());
        assert!(After::Idle("5m".into()).answers_for_you());
        assert!(!After::Never.answers_for_you());
    }

    #[test]
    fn the_rendered_line_carries_the_predicate_and_the_location() {
        let never = ClockLine::from(&QuestionClock {
            after: After::Never,
            source: Source::User,
            where_set: "~/.claude/settings.json".into(),
        });
        assert!(!never.answers_for_you);
        assert!(!never.immediate);
        assert_eq!(never.after, "never");
        assert_eq!(
            never.where_set, "~/.claude/settings.json",
            "a surface that cannot say where it was set cannot be acted on"
        );

        let timer = ClockLine::from(&clock(Source::Managed));
        assert!(timer.answers_for_you);
        assert!(!timer.chosen_by_the_person);
    }
}

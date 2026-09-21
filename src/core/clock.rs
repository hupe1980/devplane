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
//! **And there is a third source, which outranks both files.**
//!
//! > `CLAUDE_AFK_TIMEOUT_MS` — *"when set, it takes precedence over that setting
//! > and **turns auto-continue on even when the setting is unset or `never`**.
//! > Setting `0` doesn't turn the timeout off; **it closes the dialog
//! > immediately**."*
//!
//! Until 2026-09-20 this module read the two files and nothing else, so
//! `devplane modes` printed *nothing is set* — the sentence that means *your
//! questions wait for you* — about a session in which every question was being
//! ended by nobody the instant it was asked. **A surface wrong in the direction
//! of reassurance is worse than an absent one**, and this is the third time this
//! product's own thesis has failed inward.
//!
//! It is readable **exactly**, because `SessionStart` accepts only `command`
//! hooks: `devplane hook` is spawned as a child of the session and inherits its
//! environment. So this source is **per session**, which is the scope the
//! override actually has, while the two files are per machine.
//!
//! **What it cannot see is said rather than implied.** Managed settings also
//! compose from drop-ins, a policy helper and a Windows registry chain; this
//! reads the two documented files, so an absent timer means *nothing in the
//! files I read and no variable in the sessions I saw start*.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The settings key, as the vendor spells it.
pub const KEY: &str = "askUserQuestionTimeout";

/// Who set it, and it is not always a file.
///
/// **The enum exists because *you chose this* and *somebody else chose this for
/// you* are different facts about the same duration** — and a bare
/// `Option<String>` cannot tell them apart. `Managed` was the interesting one
/// until `Environment` was added, and `Environment` is now the dangerous one:
/// it outranks both files and turns the timer on where they say `never`.
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
    /// `CLAUDE_AFK_TIMEOUT_MS`, in the environment the session was started
    /// from. **Outranks both files**, and turns auto-continue on even where a
    /// file says `never` — so it is the one source that can make every other
    /// surface's answer wrong.
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

/// How long a question waits before the clock answers it.
///
/// **Zero is a different fact, not a small number.** `CLAUDE_AFK_TIMEOUT_MS=0`
/// *"doesn't turn the timeout off; it closes the dialog immediately"* — every
/// question in that session ended by nobody the instant it is asked. Rendering
/// that as `after 0s` is arithmetic where a sentence is owed, so the type
/// refuses to let a surface do it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum After {
    /// The dialog closes the instant it opens.
    Immediately,
    /// Idle time, in the vendor's own spelling — `60s`, `5m`, `10m`.
    Idle(String),
    /// **`never`, which is one of the setting's four documented values and is
    /// its default** — *"string, one of `60s`, `5m`, `10m`, or `never`;
    /// default `never`"*.
    ///
    /// **This variant is missing until 2026-09-21, and its absence was the
    /// fourth time this module's own thesis failed inward.** Every string in
    /// the file became an [`After::Idle`], so a person who had explicitly set
    /// `never` — or an administrator who had deployed it, which is the
    /// *hardening* choice — was told *"a never timer was set for you … after
    /// that, whatever is selected is submitted"*. The sentence is false, it is
    /// false about the default, and it is false in the direction of alarm.
    ///
    /// It is kept as a value rather than collapsed into `None` because the two
    /// are different facts and this module states the difference as a
    /// requirement: a person whose `never` is being overridden by
    /// `CLAUDE_AFK_TIMEOUT_MS` **needs to see the `never`**. Absent means
    /// *nothing said so*; this means *somebody said no*.
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

    /// Whether this ends a question without the person.
    ///
    /// **The predicate the surfaces branch on, so that none of them has to know
    /// which variants mean *a clock answers for you*.** `Some(clock)` used to
    /// be that predicate, and it was wrong for exactly one value.
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
    /// **Where to go and change it** — a settings path for the two files, the
    /// variable's name for the environment. One meaning, two shapes, which is
    /// what a person actually needs; it was `file`, and a field called `file`
    /// holding a variable name is a field with two meanings.
    pub where_set: String,
}

impl QuestionClock {
    /// One sentence, and no two of these read alike.
    ///
    /// **The `Immediately` arms come first because they are the ones that
    /// matter.** A session whose questions close the instant they open is a
    /// session in which this product's central promise is untrue, and a
    /// surface that renders it as a duration has technically reported it and
    /// practically not.
    pub fn says(&self) -> String {
        match (&self.after, self.source) {
            (After::Immediately, _) => "questions in this session are closed immediately — nobody \
                 is asked, and whatever happens to be selected is submitted"
                .to_string(),
            // **`never` is the reassuring answer and it is still a fact worth
            // printing**, because somebody wrote it down. It is the one arm
            // where the sentence says a question *waits*, and no surface may
            // colour it as an alarm.
            (After::Never, Source::User) => {
                "you set your own questions to wait until you answer them".to_string()
            }
            (After::Never, Source::Managed) => {
                "managed settings say your questions wait until you \
                 answer them — your own settings cannot turn that off either"
                    .to_string()
            }
            // Unreachable through `from_millis`, which has no spelling for
            // `never`: the variable turns auto-continue *on*. Answered rather
            // than `unreachable!()`, because a panic in a daemon is a worse
            // answer to a shape that cannot occur than a true sentence is.
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

    /// Whether the person at the keyboard chose this.
    ///
    /// **The environment is *not* theirs by default**, and that is the point of
    /// the variant: the vendor documents it as an override *"for demos and
    /// automated tests"*, which is to say a wrapper script, a CI job or
    /// somebody else's shell profile.
    pub fn chosen_by_the_person(&self) -> bool {
        matches!(self.source, Source::User)
    }

    /// Whether this ends questions with no wait at all.
    pub fn is_immediate(&self) -> bool {
        matches!(self.after, After::Immediately)
    }

    /// **Whether a clock answers for the person at all.**
    ///
    /// The predicate every surface branches on. `Some(clock)` was that
    /// predicate until 2026-09-21 and was wrong for `never` — which is the
    /// setting's default, so the one value most likely to be in a file was the
    /// one value the surfaces reported backwards.
    pub fn answers_for_you(&self) -> bool {
        self.after.answers_for_you()
    }
}

/// Reads `CLAUDE_AFK_TIMEOUT_MS` as the vendor defines it.
///
/// **A value that is not a number is ignored rather than guessed.** What a
/// malformed value does is the vendor's to decide, and inventing a reading of
/// it would be this product asserting a clock nobody can point at.
///
/// Rendered in the vendor's own spelling so one duration is spelled one way on
/// every surface: a person comparing `devplane modes` with `/config` should not
/// have to divide by a thousand.
pub fn from_millis(raw: &str) -> Option<After> {
    let ms: u64 = raw.trim().parse().ok()?;
    if ms == 0 {
        return Some(After::Immediately);
    }
    // **Seconds below two minutes, because the setting's own three values are
    // `60s`, `5m` and `10m`.** The natural unit for 60 000 ms is `1m`, and a
    // person comparing this line with the vendor's `/config` row would be
    // comparing two spellings of one number — which is the whole thing this
    // conversion exists to prevent.
    //
    // And below a second the vendor's vocabulary has no word at all, so it is
    // reported in milliseconds rather than rounded to `0s`, which would read as
    // `Immediately` and is a different fact.
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

/// One clock, as a surface reads it.
///
/// **The sentence is composed here and rendered nowhere.** A figure with two
/// homes rots in one of them, and this one has three readers — the CLI, the
/// board and `--json`. They get the words rather than the ingredients, which is
/// the rule `#vacuity` bought: the surface renders nothing it computes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct ClockLine {
    /// The duration as a person would say it, or `immediately`.
    pub after: String,
    /// Whether questions are closed with no wait at all. A separate flag rather
    /// than a string comparison, because a surface that colours on
    /// `after == "immediately"` is one string away from silently never firing.
    pub immediate: bool,
    pub source: Source,
    pub where_set: String,
    /// The whole sentence, which is what a surface prints.
    pub says: String,
    /// Whether the person at the keyboard chose this.
    pub chosen_by_the_person: bool,
    /// **Whether this clock answers for the person at all.**
    ///
    /// `false` only for `never`, and a surface that treats the presence of a
    /// clock as the alarm condition is one value away from telling somebody
    /// their questions are being auto-answered when they have explicitly said
    /// the opposite. A flag rather than `after == "never"`, for the same reason
    /// `immediate` is a flag: a string comparison is one rename from silently
    /// never firing.
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

// **`InForce` and `in_force` were deleted on 2026-09-21, and the deletion is the
// finding.**
//
// They existed to answer *what did this session's environment override?* —
// a requirement this module states in its own prose: "a person whose `never` is
// being ignored needs to see the `never`." Nothing ever called them. They were
// public, tested, and exported a TypeScript type that no page imported.
//
// **They could not have worked, either**, which is why the deletion is safe
// rather than merely tidy: `never` had no representation, so the very value the
// requirement is about could not be carried in the `overrode` field. A type
// implementing a requirement it cannot express, with no caller, is machinery
// standing in for the feature.
//
// **The requirement is met without them.** `devplane modes` prints the machine
// clock as a header and each session's own clock beside the session, so both
// facts are already on the screen — and the `Environment` sentence says in
// words that it "overrides your settings, including `never`". A third type that
// recombines two things a surface already shows is a second home for one
// relationship.

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

/// Reads one `askUserQuestionTimeout` value, as the vendor types it.
///
/// > **Type**: string, one of `"60s"`, `"5m"`, `"10m"`, or `"never"`.
/// > **Default**: `"never"`.
///
/// **Three answers, because the setting has three kinds of value** — and the
/// middle one is the one that was missing. Until 2026-09-21 every string became
/// a duration, so `"never"` rendered as *a never timer … after that, whatever is
/// selected is submitted*.
///
/// **A shape the setting cannot take is `None` rather than a guess.** The
/// duration arm is matched by *shape* rather than against the three documented
/// spellings, so a value the vendor adds later is still reported; a word this
/// function does not recognise is not, because inventing a reading of it is how
/// `"never"` became a timer in the first place.
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

    fn clock(source: Source) -> QuestionClock {
        QuestionClock {
            after: After::Idle("5m".into()),
            source,
            where_set: "somewhere".into(),
        }
    }

    /// The sentences have to be told apart at a glance, because two of the
    /// three are a decision somebody else took about this person's attention.
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

    /// **Zero is a sentence, not a duration.** `CLAUDE_AFK_TIMEOUT_MS=0` closes
    /// each question the instant it opens; rendering that as `after 0s` is
    /// arithmetic where a person is owed an explanation.
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

    /// One duration is spelled one way on every surface, so a person comparing
    /// this with the vendor's own `/config` is not dividing by a thousand.
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

    /// A value the vendor decides the meaning of is not one this product
    /// invents a reading for.
    #[test]
    fn a_value_that_is_not_a_number_is_ignored_rather_than_guessed() {
        for bad in ["", "   ", "soon", "5m", "-1", "60s", "1e3"] {
            assert_eq!(from_millis(bad), None, "{bad}");
            assert_eq!(from_environment(bad), None, "{bad}");
        }
    }

    /// The value the setting defaults to, and the one that was reported
    /// backwards for the life of the module.
    ///
    /// **`never` is a value, not an absence.** It renders as a clock that does
    /// not answer, so a surface can show *somebody wrote this down* without
    /// claiming anybody's questions are being closed.
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

    /// The same value deployed by an administrator, which is the *hardening*
    /// choice and read as an alarm until 2026-09-21.
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

    /// A duration is still a duration, and a word this function does not know
    /// is not turned into one.
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
        // Not a duration and not `never`: nothing is claimed. Reporting one of
        // these as a timer is the class of defect this whole test exists for.
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

    /// Every variant answers the predicate the surfaces branch on, so adding a
    /// variant without deciding this is a compile error rather than a silent
    /// `true`.
    #[test]
    fn every_wait_says_whether_it_answers_for_you() {
        assert!(After::Immediately.answers_for_you());
        assert!(After::Idle("5m".into()).answers_for_you());
        assert!(!After::Never.answers_for_you());
    }

    /// The rendered line carries the predicate, because the CLI and the board
    /// both branch on it and neither may re-derive it from a string.
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

//! The close: what the day came to, for an inbox that is empty. An empty list
//! alone looks broken; with the day's tally it says the unwatched hours are
//! accounted for, and by what. Pure functions of existing rows; the day is
//! passed in, never read from a clock.

use crate::core::{Authority, Decision};
use serde::{Deserialize, Serialize};

/// What one calendar day came to: counts, never a rate, trend or score, which
/// would measure the person.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Tally {
    /// `by_person` through `by_nobody` are decisions taken on the person's
    /// behalf; Devplane's own gates and bounds are counted apart.
    pub by_person: u32,
    pub by_rule: u32,
    pub by_timer: u32,
    /// Asked, never answered, and the moment passed.
    pub by_nobody: u32,
    pub by_devplane: u32,
    /// Questions that waited for a person today: a count, never how long.
    pub questions_waited: u32,
}

impl Tally {
    /// A quiet day gets a sentence, not a table of zeroes.
    #[must_use]
    pub fn quiet(&self) -> bool {
        self.by_person == 0
            && self.by_rule == 0
            && self.by_timer == 0
            && self.by_nobody == 0
            && self.by_devplane == 0
            && self.questions_waited == 0
    }

    #[must_use]
    pub fn on_your_behalf(&self) -> u32 {
        self.by_person + self.by_rule + self.by_timer + self.by_nobody
    }

    /// Counts the decisions given; the caller picks which rows are today's.
    #[must_use]
    pub fn of(decisions: &[Decision]) -> Self {
        let mut t = Self::default();
        for d in decisions {
            match d.authority {
                Authority::Person => t.by_person += 1,
                Authority::Rule => t.by_rule += 1,
                Authority::Timer => t.by_timer += 1,
                Authority::Nobody => t.by_nobody += 1,
                Authority::Devplane => t.by_devplane += 1,
            }
        }
        t
    }

    #[must_use]
    pub fn with_waited(mut self, waited: u32) -> Self {
        self.questions_waited = waited;
        self
    }

    /// The tally as sentences, composed once for every surface; empty when quiet.
    #[must_use]
    pub fn sentences(&self) -> Vec<String> {
        if self.quiet() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let n = self.on_your_behalf();
        if n > 0 {
            let mut parts = Vec::new();
            for (count, word) in [
                (self.by_person, "you"),
                (self.by_rule, "a rule"),
                (self.by_timer, "a clock"),
                (self.by_nobody, "nobody"),
            ] {
                if count > 0 {
                    parts.push(format!("{count} by {word}"));
                }
            }
            out.push(format!(
                "{n} {} taken in your name today — {}.",
                plural(n, "decision", "decisions"),
                parts.join(", ")
            ));
        }
        if self.questions_waited > 0 {
            out.push(format!(
                "{} {} waited for you.",
                self.questions_waited,
                plural(self.questions_waited, "question", "questions"),
            ));
        }
        if self.by_devplane > 0 {
            out.push(format!(
                "{} {} Devplane did mechanically — gates it ran and bounds it enforced.",
                self.by_devplane,
                plural(self.by_devplane, "thing", "things")
            ));
        }
        out
    }
}

fn plural(n: u32, one: &str, many: &str) -> String {
    match n {
        1 => one.to_string(),
        _ => many.to_string(),
    }
}

/// Whether an item was raised after the person last read the inbox; decided
/// here so surfaces agree. With no previous look nothing is new, and an item
/// raised exactly at the mark was on screen.
#[must_use]
pub fn new_to_you(raised: jiff::Timestamp, last_look: Option<jiff::Timestamp>) -> bool {
    match last_look {
        None => false,
        Some(mark) => raised > mark,
    }
}

/// A duration in its largest whole unit, rounded down: 16h30m is *16h*.
#[must_use]
pub fn human_secs(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m", secs / 60),
        3600..86_400 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

/// How long ago somebody last read the inbox; `None` with no previous look.
#[must_use]
pub fn since_last_look(now: jiff::Timestamp, last: Option<jiff::Timestamp>) -> Option<String> {
    let last = last?;
    let secs = now.as_second() - last.as_second();
    // A corrected clock can make the gap negative: render nothing.
    let secs = u64::try_from(secs).ok()?;
    // Under a minute is a glance, not a boundary.
    (secs >= 60).then(|| human_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(a: Authority) -> Decision {
        Decision::new(a, "agent:tool.use", "x", "deny")
    }

    #[test]
    fn what_the_tool_did_is_counted_apart_from_what_was_decided_for_you() {
        let t = Tally::of(&[
            d(Authority::Rule),
            d(Authority::Nobody),
            d(Authority::Devplane),
            d(Authority::Devplane),
        ]);
        assert_eq!(t.on_your_behalf(), 2);
        assert_eq!(t.by_devplane, 2);
    }

    #[test]
    fn a_quiet_day_gets_a_sentence_and_never_a_table_of_zeroes() {
        let t = Tally::default();
        assert!(t.quiet());
        assert!(t.sentences().is_empty());
    }

    #[test]
    fn a_day_with_only_mechanical_work_is_not_quiet() {
        let t = Tally::of(&[d(Authority::Devplane)]);
        assert!(!t.quiet());
        assert_eq!(t.on_your_behalf(), 0);
        assert_eq!(t.sentences().len(), 1);
    }

    #[test]
    fn the_sentences_name_every_authority_that_appeared_and_no_other() {
        let t = Tally::of(&[d(Authority::Person), d(Authority::Nobody)]);
        let s = t.sentences().join(" ");
        assert!(s.contains("1 by you"), "{s}");
        assert!(s.contains("1 by nobody"), "{s}");
        assert!(!s.contains("a rule"), "{s}");
        assert!(!s.contains("a clock"), "{s}");
    }

    #[test]
    fn there_is_no_rate_anywhere_in_it() {
        let t = Tally::of(&[d(Authority::Person), d(Authority::Rule)]).with_waited(2);
        let s = t.sentences().join(" ");
        assert!(!s.contains('%'), "{s}");
        assert!(!s.contains(" of "), "{s}");
    }

    /// How long a question waited measures the person; neither the sentence
    /// nor the wire carries it.
    #[test]
    fn the_tally_counts_questions_and_never_times_the_person() {
        let t = Tally::default().with_waited(2);
        let s = t.sentences().join(" ");
        assert_eq!(s, "2 questions waited for you.");
        for timing in ["longest", "11h", "41400", "for 1"] {
            assert!(!s.contains(timing), "the tally times the person: {s}");
        }
        let json = serde_json::to_string(&t).unwrap();
        assert!(
            !json.contains("longest"),
            "a wait duration is on the wire: {json}"
        );
    }

    #[test]
    fn a_gap_is_a_sentence_in_the_largest_honest_unit() {
        assert_eq!(human_secs(45), "45s");
        assert_eq!(human_secs(59), "59s");
        assert_eq!(human_secs(60), "1m");
        assert_eq!(human_secs(600), "10m");
        // Down, never up: 16h30m has not reached seventeen hours.
        assert_eq!(human_secs(59_400), "16h");
        assert_eq!(human_secs(200_000), "2d");
    }

    #[test]
    fn there_is_no_boundary_until_there_has_been_a_look() {
        let now = jiff::Timestamp::now();
        assert_eq!(since_last_look(now, None), None);
    }

    #[test]
    fn a_glance_is_not_a_gap() {
        let now = jiff::Timestamp::now();
        let recent = now - jiff::SignedDuration::from_secs(12);
        assert_eq!(since_last_look(now, Some(recent)), None);
        let a_minute = now - jiff::SignedDuration::from_secs(60);
        assert_eq!(since_last_look(now, Some(a_minute)).as_deref(), Some("1m"));
    }

    #[test]
    fn a_clock_that_moved_backwards_renders_nothing() {
        let now = jiff::Timestamp::now();
        let later = now + jiff::SignedDuration::from_secs(600);
        assert_eq!(since_last_look(now, Some(later)), None);
        // And the same instant is not a gap either.
        assert_eq!(since_last_look(now, Some(now)), None);
    }

    #[test]
    fn a_real_gap_renders_in_hours() {
        let now = jiff::Timestamp::now();
        let last = now - jiff::SignedDuration::from_secs(57_600);
        assert_eq!(since_last_look(now, Some(last)).as_deref(), Some("16h"));
    }

    #[test]
    fn an_item_is_new_only_when_there_is_a_boundary_to_be_new_to() {
        let mark: jiff::Timestamp = "2026-09-21T09:00:00Z".parse().unwrap();
        let before: jiff::Timestamp = "2026-09-21T08:59:59Z".parse().unwrap();
        let after: jiff::Timestamp = "2026-09-21T09:00:01Z".parse().unwrap();

        assert!(new_to_you(after, Some(mark)), "raised after the look");
        assert!(!new_to_you(before, Some(mark)), "raised before the look");
        assert!(
            !new_to_you(mark, Some(mark)),
            "raised exactly at the mark was on screen"
        );

        // No previous look, no boundary: a first run marks nothing new.
        assert!(!new_to_you(after, None));
        assert!(!new_to_you(before, None));
    }
}

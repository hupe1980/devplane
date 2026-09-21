//! The close: what the day came to, for an inbox that is empty.
//!
//! **Every board in this category is built to be full. This one has to be able
//! to be empty, and say so.** An inbox that renders an empty list looks broken.
//! An inbox that renders an empty list *and the day's tally* has told somebody
//! something they could not otherwise know: that the hours they were not
//! watching are accounted for, and by what.
//!
//! **The timing has a number behind it.** Fifteen professional developers, five
//! days, 229 proactive interventions across 5,732 interaction points:
//! interventions delivered at a **workflow boundary** reached **52 %**
//! engagement, while **mid-task** ones were **dismissed 62 %** of the time — and
//! a well-timed one cost **45.4 s** to interpret against **101.4 s** when the
//! person had to reconstruct the context themselves. An empty inbox is a
//! boundary; it is the cheapest moment this product will ever get.
//!
//! Everything here is a pure function of rows that already exist. The day is
//! passed in rather than read from a clock, so a test does not have to own one.

use crate::core::{Authority, Decision};
use serde::{Deserialize, Serialize};

/// What one calendar day came to.
///
/// **Counts, and no rate.** *You answered 4 of 17* is one word away from a
/// performance metric about the person; this names what happened and never how
/// well they did it. There is no percentage, no trend and no score.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Tally {
    /// Decisions taken **on the person's behalf**, by authority.
    ///
    /// `daemon` is deliberately not in here: it is Devplane running a gate or
    /// advancing a pipeline, which is the tool doing the job it was configured
    /// to do rather than a decision taken for somebody. Folding the two
    /// together would make the headline number a measure of how much the tool
    /// did, which is the opposite of what this is for.
    pub by_person: u32,
    pub by_rule: u32,
    pub by_timer: u32,
    /// Asked, never answered, and the moment passed. **The row this product
    /// exists to be able to write.**
    pub by_nobody: u32,
    /// What Devplane did mechanically, counted apart.
    pub by_daemon: u32,
    /// Questions that waited for a person at some point today.
    pub questions_waited: u32,
    /// The longest any one of them waited, in seconds.
    pub longest_wait_secs: u64,
}

impl Tally {
    /// Whether anything happened at all.
    ///
    /// A day with nothing in it gets a sentence, not a table of zeroes.
    #[must_use]
    pub fn quiet(&self) -> bool {
        self.by_person == 0
            && self.by_rule == 0
            && self.by_timer == 0
            && self.by_nobody == 0
            && self.by_daemon == 0
            && self.questions_waited == 0
    }

    /// Decisions taken on the person's behalf — everything but the mechanical.
    #[must_use]
    pub fn on_your_behalf(&self) -> u32 {
        self.by_person + self.by_rule + self.by_timer + self.by_nobody
    }

    /// Counts one day's decisions.
    ///
    /// The caller decides which rows are today's; this counts what it is given,
    /// so the window is the caller's business and the arithmetic is testable
    /// without one.
    #[must_use]
    pub fn of(decisions: &[Decision]) -> Self {
        let mut t = Self::default();
        for d in decisions {
            match d.authority {
                Authority::Person => t.by_person += 1,
                Authority::Rule => t.by_rule += 1,
                Authority::Timer => t.by_timer += 1,
                Authority::Nobody => t.by_nobody += 1,
                Authority::Daemon => t.by_daemon += 1,
            }
        }
        t
    }

    /// Adds what the day's questions came to.
    #[must_use]
    pub fn with_waits(mut self, waited: u32, longest_secs: u64) -> Self {
        self.questions_waited = waited;
        self.longest_wait_secs = longest_secs;
        self
    }

    /// The tally as sentences, composed here so that no surface has to turn a
    /// number into prose and two surfaces cannot word it differently.
    ///
    /// Empty where the day was quiet — the caller says so in one line instead.
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
                "{} {} waited for you{}.",
                self.questions_waited,
                plural(self.questions_waited, "question", "questions"),
                match self.longest_wait_secs {
                    0 => String::new(),
                    s => format!(", the longest for {}", human_secs(s)),
                }
            ));
        }
        if self.by_daemon > 0 {
            out.push(format!(
                "{} {} Devplane ran for you — gates, pipelines, pull requests.",
                self.by_daemon,
                plural(self.by_daemon, "thing", "things")
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

/// Whether an item was raised after the person last read the inbox.
///
/// **The predicate, and it lives here because the daemon decides it.** The
/// plan said *"the surface compares it to the mark"*, and two surfaces each
/// doing their own comparison is the second place a day gets counted — the
/// thing [`Tally`] is composed here to prevent. One function, one answer, both
/// surfaces render it.
///
/// **Absent means *not new*, never *new*.** With no previous look there is no
/// boundary, so nothing can be on the far side of it: a first-ever render that
/// marked every row as new would be announcing a gap it cannot measure. Same
/// for an item raised exactly at the mark — it was on screen.
#[must_use]
pub fn new_to_you(raised: jiff::Timestamp, last_look: Option<jiff::Timestamp>) -> bool {
    match last_look {
        None => false,
        Some(mark) => raised > mark,
    }
}

/// A duration in the largest unit that is still honest.
///
/// Never a bare number of seconds past a minute: *waited for 41400* is a figure
/// nobody can read, and this is a sentence rather than a field.
/// **Rounded down, with a floor of one unit.** *16h* for sixteen and a half is
/// how everybody reads "how long ago"; rounding up would say *17h* for a gap
/// that has not reached it, and for the longest wait it would overstate the one
/// figure somebody might act on.
#[must_use]
pub fn human_secs(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m", secs / 60),
        3600..86_400 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

/// How long ago somebody last read the inbox, as the hairline says it.
///
/// `None` where there is no previous look — there is no *last* to be since, and
/// a line that said *0m* would be inventing one.
#[must_use]
pub fn since_last_look(now: jiff::Timestamp, last: Option<jiff::Timestamp>) -> Option<String> {
    let last = last?;
    let secs = now.as_second() - last.as_second();
    // **Not positive is not rendered.** A machine whose clock is corrected after
    // the mark was written would otherwise produce a negative gap, and a
    // negative gap rendered in any unit is a surface contradicting itself.
    let secs = u64::try_from(secs).ok()?;
    // **And under a minute is not a boundary.** *Since you last looked · 12s* is
    // a line that costs a row and tells nobody anything; somebody who looked
    // twelve seconds ago knows they did. The threshold is stated rather than
    // falling out of integer seconds, because those are two different reasons to
    // print nothing and only one of them is a decision.
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
        // Folding these together would make the headline number a measure of
        // how much Devplane did, which is the opposite of what the seat is for.
        let t = Tally::of(&[
            d(Authority::Rule),
            d(Authority::Nobody),
            d(Authority::Daemon),
            d(Authority::Daemon),
        ]);
        assert_eq!(t.on_your_behalf(), 2);
        assert_eq!(t.by_daemon, 2);
    }

    #[test]
    fn a_quiet_day_gets_a_sentence_and_never_a_table_of_zeroes() {
        let t = Tally::default();
        assert!(t.quiet());
        assert!(t.sentences().is_empty());
    }

    #[test]
    fn a_day_with_only_mechanical_work_is_not_quiet() {
        // Devplane ran four gates and nothing was decided for anybody. That is
        // worth one sentence and it is not "nothing happened".
        let t = Tally::of(&[d(Authority::Daemon)]);
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
        // *You answered 4 of 17* is one word from a performance metric about
        // the person. This names what happened, never how well.
        let t = Tally::of(&[d(Authority::Person), d(Authority::Rule)]).with_waits(2, 60);
        let s = t.sentences().join(" ");
        assert!(!s.contains('%'), "{s}");
        assert!(!s.contains(" of "), "{s}");
    }

    #[test]
    fn a_wait_is_a_sentence_in_the_largest_honest_unit() {
        let t = Tally::default().with_waits(1, 41_400);
        assert!(t.sentences()[0].contains("11h"), "{:?}", t.sentences());
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
        // Somebody who looked twelve seconds ago knows they did.
        let now = jiff::Timestamp::now();
        let recent = now - jiff::SignedDuration::from_secs(12);
        assert_eq!(since_last_look(now, Some(recent)), None);
        let a_minute = now - jiff::SignedDuration::from_secs(60);
        assert_eq!(since_last_look(now, Some(a_minute)).as_deref(), Some("1m"));
    }

    #[test]
    fn a_clock_that_moved_backwards_renders_nothing() {
        // A corrected clock would otherwise produce a negative gap, and a
        // negative gap in any unit is the surface contradicting itself.
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

    /// **Marking an item as new had no implementation at all until 2026-09-21.** The
    /// boundary line shipped, `looked_at` shipped so a surface *could* compare,
    /// and no surface ever did — so *"items raised after that moment are
    /// marked"* was an acceptance scenario nothing satisfied.
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

        // **The case that decides whether a first run lies.** No previous look
        // means no boundary, so nothing is on the far side of one — marking
        // every row new would announce a gap this machine cannot measure.
        assert!(!new_to_you(after, None));
        assert!(!new_to_you(before, None));
    }
}

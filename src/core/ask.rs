//! A question an agent put to a person, as a row that outlives the process
//! that asked it.
//!
//! The asking state used to live in a map on a live connection, so it survived
//! the person leaving for the day and not a daemon restart: the run, the agent
//! and the question died together and the inbox said *"Nothing needs you"*.
//!
//! The five properties durable-execution systems converged on, and where each
//! one lives here:
//!
//! | Property | Here |
//! |---|---|
//! | the asking state persists **outside** the asking process | this row, in `asks` |
//! | the wait costs nothing | a deadline is a column a sweep reads, not a task per ask |
//! | a **durable deadline** giving it a defined end | [`Deadline`], opt-in, naming the file that set it |
//! | resume is idempotent, by an **opaque token** | [`AskId`]; [`Ask::answer`] refuses the second answer |
//! | the answer is durable **before** the effect | the caller writes the row, then delivers ([`Delivery`]) |
//!
//! **None of those systems records who ended an unanswered request.** An ask
//! here ends with an authority on the row — a person, a timer with its duration
//! and the file that set it, or nobody.
//!
//! Two refusals: nothing here invents an answer, and nothing here invents a
//! deadline. [`Deadline::Never`] is the default because it is what the vendor
//! does, and a product whose argument is that vendors end your questions on
//! clocks you did not set may not ship one.

use crate::core::ids::{AskId, ProjectId, RunId};
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

/// What kind of thing was asked, which decides how it is answered and never how
/// it waits.
///
/// Not a shade of the same thing: a **permission** asks whether an action is
/// allowed, so a grant or a refusal is what the options mean however the agent
/// spells them; a
/// **question** asks which of several things the person wants, and no rule can
/// answer it. They arrive on different protocol channels — `session/request_permission`
/// and `elicitation/create` — and only one of them has a policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
// **Renamed on the wire, because the wire has one namespace and Rust has
// modules.** `ask::Kind` and `batch::Kind` are unambiguous here and both
// exported a file called `Kind.ts`, so whichever generated last silently
// replaced the other — the interface would have compiled against a type
// describing the wrong thing entirely. Caught by the guard that compares the
// checked-in types to the Rust shapes.
#[cfg_attr(
    feature = "typescript",
    ts(rename = "AskKind", export, export_to = "wire/")
)]
pub enum Kind {
    Permission,
    Question,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Permission => "permission",
            Kind::Question => "question",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "permission" => Some(Kind::Permission),
            "question" => Some(Kind::Question),
            _ => None,
        }
    }
}

/// How long an unanswered ask may wait.
///
/// **`Never` is the default and is not a placeholder.** It is what the agent's
/// own vendor does, it is what a terminal does when it shows a dialog and
/// nobody is at the desk, and it is the only setting under which this product's
/// argument survives contact with its own code: a question that ends on a clock
/// nobody chose is the thing [`DIRECTION`](https://hupe1980.github.io/devplane)
/// §1 indicts four vendors for.
///
/// A project that wants a bound sets one, and then the row that ends the ask
/// names the duration **and the file it came from**, because *a timer* without
/// *whose timer* is the same non-answer as *the daemon decided*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "AskDeadline", export, export_to = "wire/")
)]
pub enum Deadline {
    /// It waits. The run shows as waiting on a person for as long as that is
    /// true, which is a fact rather than a failure.
    #[default]
    Never,
    /// Seconds a question may wait **while somebody could have answered it**,
    /// set by a project's `devplane.toml`. Counted from [`Ask::clock_starts`],
    /// not from `asked_at`: a daemon that was down was not showing anybody the
    /// question, so that time is not part of the wait.
    After(u32),
}

impl Deadline {
    /// When this ask stops being answerable, if ever — counted from the moment
    /// it was **reachable by a person**, which is not always the moment it was
    /// asked.
    ///
    /// **A deadline bounds how long a question waits for somebody who could
    /// have answered it.** While the daemon is down there is no board, no
    /// inbox and no notification: the question is not in front of anybody, so
    /// the clock is not running. Counting wall-clock time from `asked_at`
    /// instead meant that a project with any deadline set had every waiting
    /// question killed within a minute of the next start — a laptop closed at
    /// 17:00 with a `10m` deadline came back at 09:00 to a row reading *"a
    /// clock refused it after 10m"*, about ten minutes nobody was given.
    ///
    /// That is [`Ended::Timer`] writing a sentence that is not true, in the
    /// product whose whole argument is that a question must not end on a clock
    /// its owner did not get to run against. It also silently contradicted the
    /// durable ask: *a daemon that was stopped leaves the ask open and
    /// answerable* was false for every project that set a deadline.
    ///
    /// The bias is deliberate and is the one this codebase always takes: a
    /// restart **extends** the wait rather than shortening it, because guessing
    /// that somebody is not needed when they are is the expensive mistake.
    pub fn at(self, answerable_since: Timestamp) -> Option<Timestamp> {
        match self {
            Deadline::Never => None,
            Deadline::After(secs) => answerable_since
                .checked_add(SignedDuration::from_secs(secs as i64))
                .ok(),
        }
    }

    /// What a surface says about it, in the person's own terms.
    ///
    /// **The sentence for `Never` is the one that has to be exactly right**,
    /// because it is the answer to *"what happens if I ignore this?"* — and the
    /// two wrong answers are *"nothing"* and *"the agent decides"*.
    pub fn says(self) -> String {
        match self {
            Deadline::Never => "it waits — nothing answers this but you".to_string(),
            Deadline::After(secs) => format!(
                "your project ends it after {}, and records that a clock did",
                humanise(secs)
            ),
        }
    }
}

/// `4h`, `30m`, `90s` — how a duration is written where a person types one.
pub fn humanise(secs: u32) -> String {
    match secs {
        s if s % 3600 == 0 && s >= 3600 => format!("{}h", s / 3600),
        s if s % 60 == 0 && s >= 60 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

/// `4h`, `30m`, `90s`, `never` — and nothing else, because a format that
/// guesses is one that eventually guesses wrong about somebody's timeout.
pub fn parse_deadline(s: &str) -> Option<Deadline> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("never") {
        return Some(Deadline::Never);
    }
    let (digits, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit())?);
    let n: u32 = digits.parse().ok()?;
    // A zero would mean "end it the instant it is asked", which is not a
    // deadline, it is a refusal to ask — and a refusal wearing a timer's name
    // is exactly the disguise this module exists to strip off.
    if n == 0 {
        return None;
    }
    let secs = match unit {
        "s" => n,
        "m" => n.checked_mul(60)?,
        "h" => n.checked_mul(3600)?,
        _ => return None,
    };
    Some(Deadline::After(secs))
}

/// How an ask stopped waiting, and on whose authority.
///
/// **Three, and the third is the one the product is named for.** Every system
/// that lets a request expire has the first two; the row that says *nobody
/// decided this and here is what was asked* is the one nothing else writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "AskEnded", export, export_to = "wire/")
)]
pub enum Ended {
    /// A person answered. The one authority in this product that is known
    /// rather than inferred.
    Person,
    /// A deadline the project set ran out. Carries what it was and where it
    /// came from, because a timer with no owner reads as the product having
    /// decided.
    Timer { after: u32, set_by: String },
    /// The run ended, the daemon stopped, or the agent cancelled the call
    /// before anybody answered.
    Nobody { because: String },
}

impl Ended {
    /// The authority column's spelling, which is the same vocabulary the
    /// decision log uses because they are the same fact.
    pub fn authority(&self) -> &'static str {
        match self {
            Ended::Person => "person",
            Ended::Timer { .. } => "timer",
            Ended::Nobody { .. } => "nobody",
        }
    }

    /// One sentence, and no two of these read alike — a person has to be able
    /// to tell *you answered it* from *a clock did* from *nobody did* without
    /// opening a transcript.
    pub fn says(&self) -> String {
        match self {
            Ended::Person => "you answered it".to_string(),
            Ended::Timer { after, set_by } => format!(
                "a clock refused it after {} — set in {set_by}",
                humanise(*after)
            ),
            Ended::Nobody { because } => format!("nobody answered: {because}"),
        }
    }
}

/// Whether the person's answer reached the agent, and how.
///
/// **Recorded separately from the answer itself, because they are different
/// facts and the gap between them is the whole feature.** An answer written
/// down at 09:00 and delivered at 14:00 to a resumed session is a success; an
/// answer written down and never delivered is a different thing entirely, and
/// collapsing the two would let this product claim a delivery it never made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "AskDelivery", export, export_to = "wire/")
)]
pub enum Delivery {
    /// Straight down the connection the ask arrived on. The ordinary case.
    Live,
    /// The agent that asked was gone, so its session was resumed and the
    /// person's own words were delivered into it. **Said out loud on every
    /// surface**: the agent's turn had ended, so this is the answer arriving as
    /// a new message rather than as a reply, and a reader who is not told that
    /// would reasonably assume otherwise.
    Resumed,
    /// Recorded and not delivered: the agent cannot be resumed, or resuming it
    /// failed. The answer is still the person's and is still on the record.
    Undeliverable { because: String },
}

impl Delivery {
    pub fn says(&self) -> String {
        match self {
            Delivery::Live => "delivered to the waiting agent".to_string(),
            Delivery::Resumed => {
                "delivered into a resumed session — the agent's turn had already ended".to_string()
            }
            Delivery::Undeliverable { because } => {
                format!("recorded, not delivered: {because}")
            }
        }
    }

    pub fn reached_the_agent(&self) -> bool {
        !matches!(self, Delivery::Undeliverable { .. })
    }
}

/// One thing an agent asked a person, and everything that became of it.
///
/// **The id is an opaque token and that is deliberate.** It is what a person
/// answers by, from any surface, at any later time — Restate calls the same
/// thing an awakeable, Temporal a signal id, Inngest a match. Addressing an
/// answer by *session* was what made an answer undeliverable the moment the
/// session was gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Ask {
    pub id: AskId,
    pub kind: Kind,
    pub run: RunId,
    pub project: Option<ProjectId>,
    /// The protocol's own id for the in-flight request. Only meaningful while
    /// the connection that carried it is alive, which is exactly why it is not
    /// the key.
    pub request_id: String,
    /// What the agent asked, as the agent wrote it.
    pub message: String,
    /// The options and form, untouched. Rendered, never summarised.
    ///
    /// Typed as `unknown` on the wire rather than given a shape here: it
    /// carries whatever the agent asked, in the agent's own schema, and
    /// inventing a TypeScript type for it would be this product claiming to
    /// know the shape of somebody else's question.
    #[cfg_attr(feature = "typescript", ts(type = "unknown"))]
    pub payload: serde_json::Value,
    #[cfg_attr(feature = "typescript", ts(type = "string"))]
    pub asked_at: Timestamp,
    #[serde(default)]
    pub deadline: Deadline,
    /// What the person chose, written **before** anything is delivered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(type = "unknown | null"))]
    pub answer: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(type = "string | null"))]
    pub answered_at: Option<Timestamp>,
    /// Which surface it came from — `cli`, `board`, `mcp`.
    ///
    /// *Who answered this?* has to be answerable without opening a transcript,
    /// and an answer is the one record in this product whose authority is
    /// **known** rather than inferred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Delivery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended: Option<Ended>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(type = "string | null"))]
    pub ended_at: Option<Timestamp>,
}

/// What an agent asked, as the caller hands it over.
///
/// A struct rather than six positional arguments: `new(id, kind, run, request,
/// message, payload, at, deadline)` is a call nobody can read and two of whose
/// arguments are strings that would swap silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub kind: Kind,
    /// The protocol's own id for the in-flight request.
    pub request_id: String,
    pub message: String,
    pub payload: serde_json::Value,
    pub at: Timestamp,
    pub deadline: Deadline,
}

/// What went wrong when somebody tried to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Somebody already answered it. **Not an error the caller invented**: it
    /// is the idempotency guarantee doing its job, and it carries what the
    /// first answer was so the second surface can show it rather than a failure.
    AlreadyAnswered { at: Timestamp, from: Option<String> },
    /// A clock or an ending closed it before this answer arrived.
    AlreadyEnded(Ended),
}

impl Refused {
    pub fn says(&self) -> String {
        match self {
            Refused::AlreadyAnswered { at, from } => match from {
                Some(f) => format!("already answered from the {f} at {at}"),
                None => format!("already answered at {at}"),
            },
            Refused::AlreadyEnded(e) => format!("no longer answerable — {}", e.says()),
        }
    }
}

impl Ask {
    pub fn new(id: AskId, run: RunId, asked: Asked) -> Self {
        Self {
            id,
            kind: asked.kind,
            run,
            project: None,
            request_id: asked.request_id,
            message: asked.message,
            payload: asked.payload,
            asked_at: asked.at,
            deadline: asked.deadline,
            answer: None,
            answered_at: None,
            answered_from: None,
            delivery: None,
            ended: None,
            ended_at: None,
        }
    }

    pub fn in_project(mut self, project: Option<ProjectId>) -> Self {
        self.project = project;
        self
    }

    /// Still waiting for a person.
    pub fn is_open(&self) -> bool {
        self.ended.is_none()
    }

    /// When this ask's clock starts: when it was asked, or when a person could
    /// next have reached it, whichever is **later**.
    ///
    /// `reachable_since` is the moment the surfaces came back — in the daemon,
    /// its own start time. An ask asked while the daemon was up is unaffected,
    /// because `asked_at` is then the later of the two.
    pub fn clock_starts(&self, reachable_since: Timestamp) -> Timestamp {
        self.asked_at.max(reachable_since)
    }

    /// Whether this ask has passed a deadline as of `now`, given when it last
    /// became reachable by a person.
    ///
    /// A pure question about three timestamps, which is what lets the sweep be
    /// a query rather than a timer task per waiting ask — the second of the
    /// five properties. See [`Deadline::at`] for why the third one is here.
    pub fn is_overdue(&self, now: Timestamp, reachable_since: Timestamp) -> bool {
        self.is_open()
            && self
                .deadline
                .at(self.clock_starts(reachable_since))
                .is_some_and(|d| now >= d)
    }

    /// Records a person's answer, once.
    ///
    /// **Idempotent by construction and the second caller is told what the
    /// first one chose.** Two surfaces answering at the same moment is the
    /// ordinary case, not the edge: the board is open on a phone and the
    /// terminal is open on the desk. One of them wins, the other is told who
    /// did, and the agent hears the answer exactly once.
    ///
    /// **It does not deliver anything**, and it is written to be called before
    /// delivery is attempted — a crash between *answered* and *acted* has to
    /// replay as answered, never as *ask again*, because asking twice looks
    /// like caution and is a lost answer.
    pub fn answer(
        &mut self,
        answer: serde_json::Value,
        from: impl Into<String>,
        at: Timestamp,
    ) -> Result<(), Refused> {
        if let Some(e) = &self.ended {
            // An answer that arrives after a person already answered is a
            // duplicate; one that arrives after a clock closed it is a loss,
            // and they are told apart because only the second is somebody's
            // answer going nowhere.
            return Err(match (e, self.answered_at) {
                (Ended::Person, Some(at)) => Refused::AlreadyAnswered {
                    at,
                    from: self.answered_from.clone(),
                },
                _ => Refused::AlreadyEnded(e.clone()),
            });
        }
        self.answer = Some(answer);
        self.answered_at = Some(at);
        self.answered_from = Some(from.into());
        self.ended = Some(Ended::Person);
        self.ended_at = Some(at);
        Ok(())
    }

    /// Closes an ask nobody answered. Never overwrites an ending.
    pub fn end(&mut self, ended: Ended, at: Timestamp) {
        if self.ended.is_none() {
            self.ended = Some(ended);
            self.ended_at = Some(at);
        }
    }

    /// What became of it, in one sentence, with the delivery when there was one.
    ///
    /// Six settled outcomes have to be distinguishable from one another and
    /// from silence, and this is the one place that sentence is composed so two
    /// surfaces cannot word it differently.
    pub fn outcome(&self) -> String {
        match (&self.ended, &self.delivery) {
            (None, _) => match self.deadline {
                Deadline::Never => "waiting for you".to_string(),
                Deadline::After(_) => format!("waiting for you — {}", self.deadline.says()),
            },
            (Some(Ended::Person), Some(d)) => format!("{} · {}", Ended::Person.says(), d.says()),
            (Some(Ended::Person), None) => {
                // Answered and not yet delivered is a real and momentary state,
                // and it is the one a crash lands in. Saying so is how the next
                // reader knows the answer was not lost.
                "you answered it · not delivered yet".to_string()
            }
            (Some(e), _) => e.says(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn ask() -> Ask {
        Ask::new(
            AskId::new("a1"),
            RunId::new("r1"),
            Asked {
                kind: Kind::Question,
                request_id: "req-1".into(),
                message: "Keep the legacy /v1/login route?".into(),
                payload: serde_json::json!({}),
                at: at("2026-09-21T09:00:00Z"),
                deadline: Deadline::Never,
            },
        )
    }

    /// The default is the vendor's default, and the product's argument depends
    /// on it: *with the default, questions wait until you answer them*.
    #[test]
    fn nothing_ends_an_ask_by_default() {
        let a = ask();
        assert_eq!(a.deadline, Deadline::default());
        assert_eq!(a.deadline.at(a.asked_at), None);
        assert!(!a.is_overdue(at("2027-01-01T00:00:00Z"), a.asked_at));
        assert_eq!(a.outcome(), "waiting for you");
    }

    /// The answer to *"what happens if you ignore this?"* may not be "nothing"
    /// and may not be "the agent decides".
    #[test]
    fn the_sentence_about_being_ignored_names_the_only_two_outcomes() {
        assert_eq!(
            Deadline::Never.says(),
            "it waits — nothing answers this but you"
        );
        let s = Deadline::After(14_400).says();
        assert!(s.contains("4h"), "{s}");
        assert!(s.contains("a clock"), "{s}");
    }

    #[test]
    fn a_deadline_is_read_and_written_the_same_way() {
        for (text, want) in [
            ("never", Deadline::Never),
            ("90s", Deadline::After(90)),
            ("30m", Deadline::After(1800)),
            ("4h", Deadline::After(14_400)),
        ] {
            assert_eq!(parse_deadline(text), Some(want), "{text}");
        }
        assert_eq!(humanise(14_400), "4h");
        assert_eq!(humanise(1800), "30m");
        assert_eq!(humanise(90), "90s");
    }

    /// A zero deadline is a refusal to ask wearing a timer's name.
    #[test]
    fn a_zero_deadline_is_refused_rather_than_accepted_as_immediate() {
        assert_eq!(parse_deadline("0s"), None);
        assert_eq!(parse_deadline("0h"), None);
        assert_eq!(parse_deadline(""), None);
        assert_eq!(parse_deadline("4"), None);
        assert_eq!(parse_deadline("4d"), None);
        assert_eq!(parse_deadline("soon"), None);
    }

    #[test]
    fn a_deadline_that_is_set_expires_at_the_moment_it_says() {
        let mut a = ask();
        a.deadline = Deadline::After(3600);
        assert!(!a.is_overdue(at("2026-09-21T09:59:59Z"), a.asked_at));
        assert!(a.is_overdue(at("2026-09-21T10:00:00Z"), a.asked_at));
        a.end(
            Ended::Timer {
                after: 3600,
                set_by: "devplane.toml".into(),
            },
            at("2026-09-21T10:00:00Z"),
        );
        assert!(
            !a.is_overdue(at("2026-09-21T11:00:00Z"), a.asked_at),
            "ended once"
        );
        assert_eq!(a.ended.as_ref().unwrap().authority(), "timer");
        assert!(
            a.outcome().contains("set in devplane.toml"),
            "{}",
            a.outcome()
        );
    }

    /// **The clock does not run while nobody can be reached.**
    ///
    /// A laptop closed at 17:00 with a question waiting and a `10m` deadline
    /// came back at 09:00 the next morning to a row reading *"a clock refused
    /// it after 10m"* — about ten minutes the person was never given. That is
    /// this product ending a question on a clock its owner never got to run
    /// against, which is the charge it makes against four vendors.
    ///
    /// It also contradicted the durable ask outright: *a daemon that was
    /// stopped leaves the ask open and answerable* was false for every project
    /// that set a deadline, and nothing anywhere said so.
    #[test]
    fn a_deadline_does_not_run_while_the_daemon_is_down() {
        let mut a = ask();
        a.asked_at = at("2026-09-20T17:00:00Z");
        a.deadline = Deadline::After(600);

        // Asked at 17:00, daemon stopped at 17:01, restarted at 09:00.
        let restarted = at("2026-09-21T09:00:00Z");

        // Sixteen hours of wall clock have passed and the ask is **not**
        // overdue: it has been reachable for one minute.
        assert!(
            !a.is_overdue(at("2026-09-21T09:01:00Z"), restarted),
            "a restart must not retroactively expire a question nobody could reach"
        );
        // The person gets the whole window the project asked for, from the
        // moment the surfaces came back.
        assert!(!a.is_overdue(at("2026-09-21T09:09:59Z"), restarted));
        assert!(a.is_overdue(at("2026-09-21T09:10:00Z"), restarted));

        // And an ask raised while the daemon was already up is unaffected,
        // because `asked_at` is then the later of the two.
        a.asked_at = at("2026-09-21T09:30:00Z");
        assert!(!a.is_overdue(at("2026-09-21T09:39:59Z"), restarted));
        assert!(a.is_overdue(at("2026-09-21T09:40:00Z"), restarted));
    }

    /// The clock only ever moves the deadline **later**, which is the direction
    /// the doubt has to go: guessing that somebody is not needed, when they
    /// are, is the expensive mistake.
    #[test]
    fn a_restart_can_only_extend_a_wait_never_shorten_it() {
        let mut a = ask();
        a.asked_at = at("2026-09-21T09:00:00Z");
        a.deadline = Deadline::After(600);
        for reachable in [
            at("2026-09-20T00:00:00Z"),
            at("2026-09-21T08:59:59Z"),
            at("2026-09-21T09:00:00Z"),
        ] {
            assert_eq!(
                a.clock_starts(reachable),
                a.asked_at,
                "a daemon that started before the ask changes nothing"
            );
        }
        assert_eq!(
            a.clock_starts(at("2026-09-21T09:05:00Z")),
            at("2026-09-21T09:05:00Z")
        );
    }

    /// Two surfaces, one answer, and the loser is told who won rather than
    /// being handed a failure.
    #[test]
    fn the_same_ask_is_answered_exactly_once_however_many_surfaces_try() {
        let mut a = ask();
        assert!(
            a.answer(
                serde_json::json!({"question_0": "keep"}),
                "board",
                at("2026-09-21T09:05:00Z")
            )
            .is_ok()
        );
        let again = a.answer(
            serde_json::json!({"question_0": "drop"}),
            "cli",
            at("2026-09-21T09:05:01Z"),
        );
        match again {
            Err(Refused::AlreadyAnswered { from, .. }) => {
                assert_eq!(from.as_deref(), Some("board"))
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(a.answer, Some(serde_json::json!({"question_0": "keep"})));
        assert_eq!(a.answered_from.as_deref(), Some("board"));
    }

    /// An answer arriving after a clock closed it is a *loss*, not a duplicate,
    /// and the person is told which.
    #[test]
    fn an_answer_after_a_clock_closed_it_reads_differently_from_a_duplicate() {
        let mut a = ask();
        a.end(
            Ended::Timer {
                after: 60,
                set_by: "devplane.toml".into(),
            },
            at("2026-09-21T09:01:00Z"),
        );
        let refused = a
            .answer(serde_json::json!({}), "cli", at("2026-09-21T09:02:00Z"))
            .unwrap_err();
        assert!(matches!(refused, Refused::AlreadyEnded(_)));
        assert!(refused.says().contains("no longer answerable"));
        assert!(a.answer.is_none(), "a refused answer is not recorded");
    }

    /// The ending is written once. A run that dies after a person answered may
    /// not overwrite *who answered* with *nobody did*.
    #[test]
    fn a_run_ending_cannot_overwrite_a_person_who_already_answered() {
        let mut a = ask();
        a.answer(serde_json::json!({}), "cli", at("2026-09-21T09:05:00Z"))
            .unwrap();
        a.end(
            Ended::Nobody {
                because: "the run ended".into(),
            },
            at("2026-09-21T09:06:00Z"),
        );
        assert_eq!(a.ended, Some(Ended::Person));
        assert_eq!(a.ended.as_ref().unwrap().authority(), "person");
    }

    /// Answered and undelivered is a state, and it is the one a crash between
    /// *approved* and *acted* lands in.
    #[test]
    fn answered_but_undelivered_says_so_rather_than_claiming_a_delivery() {
        let mut a = ask();
        a.answer(serde_json::json!({}), "cli", at("2026-09-21T09:05:00Z"))
            .unwrap();
        assert_eq!(a.outcome(), "you answered it · not delivered yet");
        a.delivery = Some(Delivery::Resumed);
        assert!(a.outcome().contains("resumed session"), "{}", a.outcome());
        a.delivery = Some(Delivery::Undeliverable {
            because: "that agent cannot be resumed".into(),
        });
        assert!(!a.delivery.as_ref().unwrap().reached_the_agent());
        assert!(a.outcome().contains("recorded, not delivered"));
    }

    /// No two settled outcomes read alike, which is what lets a person tell
    /// them apart without opening a transcript.
    #[test]
    fn every_ending_has_its_own_sentence() {
        let endings = [
            Ended::Person,
            Ended::Timer {
                after: 3600,
                set_by: "devplane.toml".into(),
            },
            Ended::Nobody {
                because: "the run ended".into(),
            },
        ];
        let said: Vec<String> = endings.iter().map(|e| e.says()).collect();
        for (i, a) in said.iter().enumerate() {
            for b in said.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
        let authorities: Vec<&str> = endings.iter().map(|e| e.authority()).collect();
        assert_eq!(authorities, ["person", "timer", "nobody"]);
    }

    /// The wire spellings are asked of serde rather than reconstructed from
    /// `Debug`, which is the standing rule for anything that crosses a boundary.
    #[test]
    fn the_wire_spellings_are_what_serde_writes() {
        for k in [Kind::Permission, Kind::Question] {
            assert_eq!(
                serde_json::to_string(&k).unwrap(),
                format!("\"{}\"", k.as_str())
            );
            assert_eq!(Kind::parse(k.as_str()), Some(k));
        }
    }
}

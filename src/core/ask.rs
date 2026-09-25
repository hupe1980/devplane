//! A question an agent put to a person, as a row that outlives the process
//! that asked it. The row is the asking state; a deadline is a column a sweep
//! reads; [`AskId`] is the opaque token answers are addressed by; the answer is
//! written before it is delivered ([`Delivery`]). Every ending records its
//! authority — a person, a timer naming the file that set it, or nobody.
//! Nothing here invents an answer or a deadline: [`Deadline::Never`] is the default.

use crate::core::ids::{AskId, ProjectId, RunId};
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};

/// What was asked: a **permission** (is this action allowed — `session/request_permission`,
/// which policy can answer) or a **question** (which thing does the person want —
/// `elicitation/create`, which no rule can answer). Decides how it is answered, never how it waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
// Renamed on the wire: ts-rs exports one flat namespace, and two `Kind.ts`
// files would silently overwrite each other.
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

/// How long an unanswered ask may wait. `Never` is the default, deliberately:
/// a question must not end on a clock its owner did not choose. A project that
/// sets a bound gets a row naming the duration and the file it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "AskDeadline", export, export_to = "wire/")
)]
pub enum Deadline {
    /// It waits; the run shows as waiting on a person.
    #[default]
    Never,
    /// Seconds a question may wait while somebody could have answered it, set
    /// in `devplane.toml`. Counted from [`Ask::clock_starts`], not `asked_at`.
    After(u32),
}

impl Deadline {
    /// When this ask stops being answerable, counted from when it became
    /// reachable by a person. While the host is down nobody can see the
    /// question, so the clock does not run: a restart extends the wait and
    /// never shortens it, and [`Ended::Timer`] never claims time nobody was given.
    pub fn at(self, answerable_since: Timestamp) -> Option<Timestamp> {
        match self {
            Deadline::Never => None,
            Deadline::After(secs) => answerable_since
                .checked_add(SignedDuration::from_secs(secs as i64))
                .ok(),
        }
    }

    /// What a surface says about it. The `Never` sentence answers *"what happens
    /// if I ignore this?"* and must say neither "nothing" nor "the agent decides".
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

/// `4h`, `30m`, `90s`, `never` — nothing else; the format does not guess.
pub fn parse_deadline(s: &str) -> Option<Deadline> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("never") {
        return Some(Deadline::Never);
    }
    let (digits, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit())?);
    let n: u32 = digits.parse().ok()?;
    // Zero would be a refusal to ask disguised as a deadline.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "AskEnded", export, export_to = "wire/")
)]
pub enum Ended {
    /// A person answered.
    Person,
    /// A person stopped the run before answering: their act, not an answer.
    Stopped,
    /// A project deadline ran out; carries the duration and where it was set.
    Timer { after: u32, set_by: String },
    /// The run ended, the host stopped, or the agent cancelled the call
    /// before anybody answered.
    Nobody { because: String },
}

impl Ended {
    /// The authority column's spelling, shared with the decision log.
    pub fn authority(&self) -> &'static str {
        match self {
            Ended::Person | Ended::Stopped => "person",
            Ended::Timer { .. } => "timer",
            Ended::Nobody { .. } => "nobody",
        }
    }

    /// One sentence, distinct per ending, so *you* / *a clock* / *nobody* can
    /// be told apart without a transcript.
    pub fn says(&self) -> String {
        match self {
            Ended::Person => "you answered it".to_string(),
            Ended::Stopped => "you stopped the run before answering".to_string(),
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
/// Whether the person's answer reached the agent, and how. Kept separate from
/// the answer: recorded-but-undelivered must never read as delivered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "AskDelivery", export, export_to = "wire/")
)]
pub enum Delivery {
    /// Straight down the connection the ask arrived on.
    Live,
    /// The asking agent was gone, so its session was resumed and the answer
    /// delivered as a new message. Every surface says so.
    Resumed,
    /// Recorded and not delivered: the agent cannot be resumed, or resuming failed.
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

/// One thing an agent asked a person, and everything that became of it. The id
/// is an opaque token a person answers by from any surface at any later time —
/// never the session, which may be gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct Ask {
    pub id: AskId,
    pub kind: Kind,
    pub run: RunId,
    pub project: Option<ProjectId>,
    /// The protocol's in-flight request id; only meaningful while its connection
    /// lives, which is why it is not the key.
    pub request_id: String,
    /// What the agent asked, as the agent wrote it.
    pub message: String,
    /// The options and form, untouched. `unknown` on the wire: it is the agent's
    /// own schema, not ours to type.
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
    /// Which surface answered — `cli`, `board`, `mcp`.
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
    /// Somebody already answered; carries the first answer so the second
    /// surface can show it. This is the idempotency guarantee, not a failure.
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

    /// When this ask's clock starts: the later of `asked_at` and
    /// `reachable_since` (when the surfaces came back, i.e. host start).
    pub fn clock_starts(&self, reachable_since: Timestamp) -> Timestamp {
        crate::core::reduce::facts::clock_starts(self.asked_at, reachable_since)
    }

    /// Whether this ask has passed its deadline as of `now`. Pure, so the sweep
    /// is a query rather than a timer per ask.
    pub fn is_overdue(&self, now: Timestamp, reachable_since: Timestamp) -> bool {
        self.is_open()
            && self
                .deadline
                .at(self.clock_starts(reachable_since))
                .is_some_and(|d| now >= d)
    }

    /// Records a person's answer, once. Concurrent answers from two surfaces
    /// are ordinary: one wins, the other is told who did. Does not deliver —
    /// call it before delivery so a crash replays as *answered*, never *ask again*.
    pub fn answer(
        &mut self,
        answer: serde_json::Value,
        from: impl Into<String>,
        at: Timestamp,
    ) -> Result<(), Refused> {
        if let Some(e) = &self.ended {
            // A duplicate after a person's answer vs. a loss after a clock
            // closed it: only the second is an answer going nowhere.
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
    /// What became of it, in one sentence — composed only here so surfaces
    /// cannot word it differently.
    pub fn outcome(&self) -> String {
        match (&self.ended, &self.delivery) {
            (None, _) => match self.deadline {
                Deadline::Never => "waiting for you".to_string(),
                Deadline::After(_) => format!("waiting for you — {}", self.deadline.says()),
            },
            (Some(Ended::Person), Some(d)) => format!("{} · {}", Ended::Person.says(), d.says()),
            (Some(Ended::Person), None) => {
                // Answered, not yet delivered: the state a crash lands in.
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

    #[test]
    fn nothing_ends_an_ask_by_default() {
        let a = ask();
        assert_eq!(a.deadline, Deadline::default());
        assert_eq!(a.deadline.at(a.asked_at), None);
        assert!(!a.is_overdue(at("2027-01-01T00:00:00Z"), a.asked_at));
        assert_eq!(a.outcome(), "waiting for you");
    }

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

    /// The clock does not run while nobody can be reached.
    #[test]
    fn a_deadline_does_not_run_while_the_host_is_down() {
        let mut a = ask();
        a.asked_at = at("2026-09-20T17:00:00Z");
        a.deadline = Deadline::After(600);

        // Asked at 17:00, host stopped at 17:01, restarted at 09:00.
        let restarted = at("2026-09-21T09:00:00Z");

        // Sixteen hours of wall clock, but reachable for one minute.
        assert!(
            !a.is_overdue(at("2026-09-21T09:01:00Z"), restarted),
            "a restart must not retroactively expire a question nobody could reach"
        );
        assert!(!a.is_overdue(at("2026-09-21T09:09:59Z"), restarted));
        assert!(a.is_overdue(at("2026-09-21T09:10:00Z"), restarted));

        // An ask raised while the host was up is unaffected.
        a.asked_at = at("2026-09-21T09:30:00Z");
        assert!(!a.is_overdue(at("2026-09-21T09:39:59Z"), restarted));
        assert!(a.is_overdue(at("2026-09-21T09:40:00Z"), restarted));
    }

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
                "a host that started before the ask changes nothing"
            );
        }
        assert_eq!(
            a.clock_starts(at("2026-09-21T09:05:00Z")),
            at("2026-09-21T09:05:00Z")
        );
    }

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

    /// A run that dies after a person answered may not overwrite the ending.
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

//! The one sentence every surface prints: how many current approvals there
//! are against how many the base requires, and from whom, by kind. Counts in
//! words — no percentage, no fraction figure, no score, and no word that the
//! change is fit to merge.

use super::required::{Required, rule_line};
use super::{Bearing, Kind, OnHead, ReviewState};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// The longest login a sentence carries, in characters.
pub const LOGIN_CHARS: usize = 64;

/// What a model's approval is called before the record has a word for it.
pub const UNATTRIBUTED: &str =
    "could not be attributed — a model decided, and the record has no word for it yet";

/// What every surface carrying the sentence says about its limits.
pub const LIMITS_LINE: &str = "Devplane read these reviews; which approvals GitHub counted toward \
its rule — write access, code owners, the last push — is GitHub's to decide.";

/// Which wording composed a model's approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelWord {
    /// The authorities have no `model`: *could not be attributed*.
    Unattributed,
    /// The authorities have `model`.
    Model,
}

/// Whether the record's authorities have a word for a model. Read from the
/// vocabulary itself, so it changes only with the vocabulary — no setting,
/// flag or file can claim it.
pub fn model_ratified() -> bool {
    crate::core::Authority::parse("model").is_some()
}

/// One read of a pull request's approvals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub number: u64,
    pub url: String,
    /// The head commit the reviews were compared against.
    pub head: String,
    /// The base branch's name.
    pub base: String,
    pub required: Required,
    /// The count, where it was read and what else the rule says; absent
    /// when no count was read.
    #[serde(default)]
    pub required_says: Option<String>,
    pub bearing: Vec<Bearing>,
    /// Pages of reviews past the reading's bound; when set, approvals are
    /// not stated by kind.
    #[serde(default)]
    pub unread_pages: Option<u32>,
    pub sentence: String,
    pub model_word: ModelWord,
    pub read_at: Timestamp,
    /// Read after the forge said the pull request merged.
    #[serde(default)]
    pub after_merge: bool,
}

impl Reading {
    /// Writes the sentence, the rule line and which wording was used, from
    /// the reading's facts and whether the record has a word for a model.
    #[must_use]
    pub fn composed(mut self, model_ratified: bool) -> Self {
        self.sentence = sentence(&self, model_ratified);
        self.required_says = rule_line(&self.required);
        self.model_word = match model_ratified {
            true => ModelWord::Model,
            false => ModelWord::Unattributed,
        };
        self
    }
}

/// A pull request's approvals on a change: read, kept from an earlier read
/// with why it is not current, or never read with why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReadingState {
    Current { reading: Box<Reading> },
    Stale { last: Box<Reading>, why: String },
    NotRead { why: String },
}

impl ReadingState {
    /// The reading this state carries, current or not.
    pub fn reading(&self) -> Option<&Reading> {
        match self {
            Self::Current { reading } => Some(reading),
            Self::Stale { last, .. } => Some(last),
            Self::NotRead { .. } => None,
        }
    }

    /// What a failed read leaves: the last reading kept and marked, or why
    /// nothing was read.
    #[must_use]
    pub fn failed(previous: Option<&ReadingState>, why: String) -> Self {
        match previous.and_then(ReadingState::reading) {
            Some(last) => Self::Stale {
                last: Box::new(last.clone()),
                why,
            },
            None => Self::NotRead { why },
        }
    }
}

/// Repository text inside our sentence: one line, no control characters,
/// bounded.
fn shown(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    crate::core::text::clip(&flat, LOGIN_CHARS)
}

fn short(sha: &str) -> String {
    shown(&sha.chars().take(7).collect::<String>())
}

fn login(b: &Bearing) -> Option<String> {
    b.account.as_ref().map(|a| shown(&a.login))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

/// Where a review was, for the tails.
fn on(b: &Bearing) -> String {
    match (b.on_head, b.commit.as_deref()) {
        (OnHead::Gone, _) | (_, None) => "on a commit that no longer exists".into(),
        (_, Some(c)) => format!("on {}", short(c)),
    }
}

/// One reviewer named by kind, for the tails.
fn who(b: &Bearing, ratified: bool) -> String {
    let named = login(b).unwrap_or_else(|| "no user".into());
    match &b.kind {
        Kind::Person => format!("a person's account ({named})"),
        Kind::Model { product, .. } if ratified => format!("{product} (a model)"),
        Kind::Model { product, .. } => {
            format!("a model the record has no word for yet ({product}, {named})")
        }
        Kind::Bot => format!("a bot account ({named})"),
        Kind::Unclassified { received } => format!(
            "an account Devplane could not classify (type {})",
            shown(received)
        ),
    }
}

/// Where a group's approvals sit, when some are not on the head.
fn not_on_head(group: &[&Bearing]) -> String {
    let older: Vec<String> = group
        .iter()
        .filter(|b| b.on_head == OnHead::Older)
        .filter_map(|b| b.commit.as_deref().map(short))
        .collect();
    let gone = group.iter().filter(|b| b.on_head == OnHead::Gone).count();
    let all = group.len();
    let mut out = String::new();
    if !older.is_empty() {
        out.push_str(&match older.len() == all {
            true => format!(", on an older commit ({})", older.join(", ")),
            false => format!(
                ", {} of them on an older commit ({})",
                older.len(),
                older.join(", ")
            ),
        });
    }
    if gone > 0 {
        out.push_str(&match gone == all {
            true => ", on a commit that no longer exists".to_string(),
            false => format!(", {gone} of them on a commit that no longer exists"),
        });
    }
    out
}

/// The approvals of one kind, as a clause.
fn clause(kind: &Kind, group: &[&Bearing], ratified: bool) -> String {
    let n = group.len();
    let names = || {
        group
            .iter()
            .map(|b| login(b).unwrap_or_else(|| "no user".into()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let head = match kind {
        Kind::Person => format!("{n} from a person's account ({})", names()),
        Kind::Model { product, .. } if ratified => format!("{n} from {product} (a model)"),
        Kind::Model { product, .. } => format!("{n} {UNATTRIBUTED} ({product}, {})", names()),
        Kind::Bot => format!("{n} from a bot account ({})", names()),
        Kind::Unclassified { received } => format!(
            "{n} from an account Devplane could not classify (type {})",
            shown(received)
        ),
    };
    format!("{head}{}", not_on_head(group))
}

/// The sentence. The only writer of it: the change, the certificate and
/// `devplane verify` print what this returns.
pub fn sentence(r: &Reading, ratified: bool) -> String {
    let approvals: Vec<&Bearing> = r.bearing.iter().filter(|b| b.is_approval()).collect();
    let a = approvals.len();
    let head = match &r.required {
        Required::Count { n, .. } if *n > 0 && a <= *n as usize => {
            format!("{a} of {n} required approvals")
        }
        Required::Count { n, .. } if *n > 0 => {
            format!("{}, {n} required", plural(a, "approval", "approvals"))
        }
        Required::Count { .. } | Required::NoneRequired => format!(
            "{}; no approval is required by the rules Devplane could read",
            plural(a, "approval", "approvals")
        ),
        Required::Unread { why } => format!(
            "{}; the number required could not be read ({})",
            plural(a, "approval", "approvals"),
            why.trim_end_matches('.')
        ),
    };

    if let Some(pages) = r.unread_pages {
        return format!(
            "{head} among the reviews read; {} of reviews were not read, so who approved is \
             not stated by kind.",
            plural(pages as usize, "more page", "more pages")
        );
    }

    let mut out = match a {
        0 => format!("{head}; nobody approved."),
        _ => {
            // Fixed order: models, then people (always said, zero included),
            // then bot accounts, then accounts that could not be classified.
            let mut clauses: Vec<String> = Vec::new();
            let mut models: Vec<&Kind> = Vec::new();
            for b in &approvals {
                if matches!(b.kind, Kind::Model { .. }) && !models.contains(&&b.kind) {
                    models.push(&b.kind);
                }
            }
            for k in models {
                let g: Vec<&Bearing> = approvals.iter().copied().filter(|b| &b.kind == k).collect();
                clauses.push(clause(k, &g, ratified));
            }
            let people: Vec<&Bearing> = approvals
                .iter()
                .copied()
                .filter(|b| b.kind == Kind::Person)
                .collect();
            match people.is_empty() {
                true => clauses.push("0 from a person's account".into()),
                false => clauses.push(clause(&Kind::Person, &people, ratified)),
            }
            let bots: Vec<&Bearing> = approvals
                .iter()
                .copied()
                .filter(|b| b.kind == Kind::Bot)
                .collect();
            if !bots.is_empty() {
                clauses.push(clause(&Kind::Bot, &bots, ratified));
            }
            let mut odd: Vec<&Kind> = Vec::new();
            for b in &approvals {
                if matches!(b.kind, Kind::Unclassified { .. }) && !odd.contains(&&b.kind) {
                    odd.push(&b.kind);
                }
            }
            for k in odd {
                let g: Vec<&Bearing> = approvals.iter().copied().filter(|b| &b.kind == k).collect();
                clauses.push(clause(k, &g, ratified));
            }
            format!("{head}: {}.", clauses.join("; "))
        }
    };

    // The reviews that are not current approvals, each by kind.
    let tail = |state: &dyn Fn(&ReviewState) -> bool, with_commit: bool| -> Vec<String> {
        r.bearing
            .iter()
            .filter(|b| state(&b.state))
            .map(|b| match with_commit {
                true => format!("{}, {}", who(b, ratified), on(b)),
                false => who(b, ratified),
            })
            .collect()
    };
    let dismissed = tail(&|s| *s == ReviewState::Dismissed, true);
    if !dismissed.is_empty() {
        out.push_str(&format!(
            " {} dismissed: {}.",
            dismissed.len(),
            dismissed.join("; ")
        ));
    }
    let changes = tail(&|s| *s == ReviewState::ChangesRequested, false);
    if !changes.is_empty() {
        out.push_str(&format!(
            " {} requesting changes: {}.",
            changes.len(),
            changes.join("; ")
        ));
    }
    let mut unknown: Vec<String> = Vec::new();
    for b in r
        .bearing
        .iter()
        .filter(|b| matches!(b.state, ReviewState::Other(_)))
    {
        unknown.push(format!(
            "{} ({})",
            who(b, ratified),
            shown(b.state.as_str())
        ));
    }
    if !unknown.is_empty() {
        out.push_str(&format!(
            " {} in a state Devplane does not read: {}.",
            unknown.len(),
            unknown.join("; ")
        ));
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::tests::{COPILOT, account, review};
    use super::super::{Review, Source, bearing};
    use super::*;

    pub const HEAD: &str = "3f2a9c1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    pub fn reading(required: Required, reviews: &[Review], ratified: bool) -> Reading {
        Reading {
            number: 7,
            url: "https://github.com/acme/app/pull/7".into(),
            head: HEAD.into(),
            base: "main".into(),
            required,
            required_says: None,
            bearing: bearing("github.com", reviews, HEAD),
            unread_pages: None,
            sentence: String::new(),
            model_word: ModelWord::Unattributed,
            read_at: Timestamp::UNIX_EPOCH,
            after_merge: false,
        }
        .composed(ratified)
    }

    fn count(n: u32) -> Required {
        Required::Count {
            n,
            source: Source::Rules,
            dismiss_stale: false,
            code_owner: false,
            last_push: false,
        }
    }

    /// A Copilot approval on the head and a person's approval dismissed by
    /// a later push: the shape the recorded pull request has.
    pub fn copilot_and_dismissed() -> Vec<Review> {
        vec![
            review(
                1,
                Some(account(10, "alice", "User")),
                "APPROVED",
                Some("3f2a9c1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab"),
            ),
            review(
                2,
                Some(account(10, "alice", "User")),
                "DISMISSED",
                Some("3f2a9c1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab"),
            ),
            review(3, Some(account(20, COPILOT, "Bot")), "APPROVED", Some(HEAD)),
        ]
    }

    #[test]
    fn a_model_approval_before_the_record_has_a_word_for_it() {
        let r = reading(count(1), &copilot_and_dismissed(), false);
        assert_eq!(
            r.sentence,
            "1 of 1 required approvals: 1 could not be attributed — a model decided, and the \
             record has no word for it yet (Copilot code review, \
             copilot-pull-request-reviewer[bot]); 0 from a person's account. 1 dismissed: a \
             person's account (alice), on 3f2a9c1."
        );
        assert_eq!(r.model_word, ModelWord::Unattributed);
    }

    #[test]
    fn a_model_approval_once_the_record_has_a_word_for_it() {
        let r = reading(count(1), &copilot_and_dismissed(), true);
        assert_eq!(
            r.sentence,
            "1 of 1 required approvals: 1 from Copilot code review (a model); 0 from a \
             person's account. 1 dismissed: a person's account (alice), on 3f2a9c1."
        );
        assert_eq!(r.model_word, ModelWord::Model);
    }

    #[test]
    fn a_model_is_never_counted_as_a_person_in_either_state() {
        for ratified in [false, true] {
            let r = reading(count(1), &copilot_and_dismissed(), ratified);
            assert!(
                r.sentence.contains("0 from a person's account"),
                "{}",
                r.sentence
            );
            assert!(!r.sentence.contains("1 from a person's account"));
        }
    }

    #[test]
    fn the_other_shapes() {
        assert_eq!(
            reading(count(2), &[], false).sentence,
            "0 of 2 required approvals; nobody approved."
        );
        let bob = || Some(account(30, "bob", "User"));
        let two = [
            review(1, bob(), "APPROVED", Some(HEAD)),
            review(
                2,
                Some(account(40, "deploy-bot[bot]", "Bot")),
                "APPROVED",
                Some(HEAD),
            ),
        ];
        assert_eq!(
            reading(count(1), &two, false).sentence,
            "2 approvals, 1 required: 1 from a person's account (bob); 1 from a bot account \
             (deploy-bot[bot])."
        );
        assert_eq!(
            reading(
                Required::Unread {
                    why: "the token cannot read branch protection".into()
                },
                &two[..1],
                false
            )
            .sentence,
            "1 approval; the number required could not be read (the token cannot read branch \
             protection): 1 from a person's account (bob)."
        );
        assert_eq!(
            reading(Required::NoneRequired, &two[..1], false).sentence,
            "1 approval; no approval is required by the rules Devplane could read: 1 from a \
             person's account (bob)."
        );
        assert_eq!(
            reading(
                count(1),
                &[review(
                    1,
                    bob(),
                    "APPROVED",
                    Some("9e1d0aabbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                )],
                false
            )
            .sentence,
            "1 of 1 required approvals: 1 from a person's account (bob), on an older commit \
             (9e1d0aa)."
        );
        assert_eq!(
            reading(count(1), &[review(1, bob(), "APPROVED", None)], false).sentence,
            "1 of 1 required approvals: 1 from a person's account (bob), on a commit that no \
             longer exists."
        );
        assert_eq!(
            reading(
                count(1),
                &[review(1, bob(), "CHANGES_REQUESTED", Some(HEAD))],
                false
            )
            .sentence,
            "0 of 1 required approvals; nobody approved. 1 requesting changes: a person's \
             account (bob)."
        );
        assert_eq!(
            reading(count(1), &[review(1, None, "APPROVED", Some(HEAD))], false).sentence,
            "1 of 1 required approvals: 0 from a person's account; 1 from an account Devplane \
             could not classify (type no user)."
        );
    }

    #[test]
    fn past_the_bound_no_kind_is_counted() {
        let mut r = reading(count(1), &copilot_and_dismissed(), false);
        r.unread_pages = Some(3);
        let s = sentence(&r, false);
        assert!(s.contains("3 more pages of reviews were not read"), "{s}");
        assert!(!s.contains("from a person's account"), "{s}");
        assert!(!s.contains(UNATTRIBUTED), "{s}");
    }

    #[test]
    fn logins_are_one_bounded_line() {
        let long = format!("evil\n`<img src=x>`{}", "a".repeat(200));
        let r = reading(
            count(1),
            &[review(
                1,
                Some(account(1, &long, "User")),
                "APPROVED",
                Some(HEAD),
            )],
            false,
        );
        assert!(!r.sentence.contains('\n'));
        let inside = r
            .sentence
            .split("account (")
            .nth(1)
            .unwrap()
            .split(')')
            .next()
            .unwrap();
        assert!(inside.chars().count() <= LOGIN_CHARS, "{inside}");
    }

    #[test]
    fn no_sentence_holds_a_ratio_or_a_verdict_word() {
        let bob = || Some(account(30, "bob", "User"));
        let mut cases = vec![
            reading(count(1), &copilot_and_dismissed(), false),
            reading(count(1), &copilot_and_dismissed(), true),
            reading(count(2), &[], false),
            reading(Required::NoneRequired, &[], false),
            reading(
                Required::Unread { why: "x".into() },
                &[review(1, bob(), "APPROVED", Some("old"))],
                false,
            ),
            reading(
                count(1),
                &[
                    review(1, bob(), "CHANGES_REQUESTED", None),
                    review(2, Some(account(3, "q", "Mannequin")), "X", None),
                ],
                false,
            ),
        ];
        let mut paged = reading(count(1), &copilot_and_dismissed(), false);
        paged.unread_pages = Some(1);
        paged.sentence = sentence(&paged, false);
        cases.push(paged);
        for r in cases {
            let s = r.sentence.to_lowercase();
            assert!(!s.contains('%'), "{s}");
            let b = s.as_bytes();
            for i in 1..b.len().saturating_sub(1) {
                assert!(
                    !(b[i] == b'/' && b[i - 1].is_ascii_digit() && b[i + 1].is_ascii_digit()),
                    "a fraction figure: {s}"
                );
            }
            for word in [
                "score", "grade", "safe", "good", "ready", "percent", "ratio",
            ] {
                assert!(
                    !s.split(|c: char| !c.is_alphanumeric()).any(|w| w == word),
                    "`{word}` in: {s}"
                );
            }
            if let Some(line) = &r.required_says {
                assert!(!line.contains('%'));
            }
        }
    }

    #[test]
    fn the_live_state_follows_the_authority_vocabulary() {
        assert_eq!(
            model_ratified(),
            crate::core::Authority::parse("model").is_some()
        );
        // Nothing else can switch it: no setting, variable or flag is read.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/approvals");
        for e in std::fs::read_dir(dir).unwrap() {
            let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
            let code = text.split("#[cfg(test)]").next().unwrap();
            for banned in ["std::env", "env!(", "cfg!(feature", "cfg(feature", "Config"] {
                assert!(
                    !code.contains(banned),
                    "the approvals module names `{banned}`"
                );
            }
        }
    }

    #[test]
    fn a_failed_read_keeps_the_last_reading_marked() {
        let r = reading(count(1), &[], false);
        let now = ReadingState::Current {
            reading: Box::new(r.clone()),
        };
        assert_eq!(
            ReadingState::failed(Some(&now), "signed out".into()),
            ReadingState::Stale {
                last: Box::new(r),
                why: "signed out".into()
            }
        );
        assert_eq!(
            ReadingState::failed(None, "signed out".into()),
            ReadingState::NotRead {
                why: "signed out".into()
            }
        );
    }

    #[test]
    fn the_wire_shape_is_camel_case_and_tagged() {
        let r = reading(count(1), &copilot_and_dismissed(), false);
        let v = serde_json::to_value(ReadingState::Current {
            reading: Box::new(r),
        })
        .unwrap();
        assert_eq!(v["state"], "current");
        let r = &v["reading"];
        for k in [
            "number",
            "url",
            "head",
            "base",
            "required",
            "bearing",
            "sentence",
            "modelWord",
            "readAt",
            "afterMerge",
        ] {
            assert!(r.get(k).is_some(), "{k} in {r}");
        }
        assert_eq!(r["modelWord"], "unattributed");
        assert_eq!(r["required"]["state"], "count");
        assert_eq!(r["bearing"][0]["kind"], "person");
        assert_eq!(r["bearing"][0]["state"], "DISMISSED");
        let m = &r["bearing"][1];
        assert_eq!(m["kind"], "model");
        assert_eq!(m["product"], "Copilot code review");
        assert_eq!(m["state"], "APPROVED");
        assert_eq!(m["onHead"], "head");
        assert_eq!(m["account"]["type"], "Bot");
        let back: ReadingState = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(back).unwrap(), v);
    }
}

//! Who approved a pull request, read from the forge's documented review list
//! and said in one sentence.
//!
//! Pure: the reviews, the base branch's rules, the clock and whether the
//! record has a word for a model all arrive as values. A reviewer's kind comes
//! only from the account type the forge reports and, for a model, from an
//! identity a vendor documents ([`crate::core::vendors::MODEL_REVIEWERS`]) —
//! never from what a login looks like. Nothing here decides, gates or records
//! a decision: the merge was decided on the forge, and this is evidence about
//! it.

pub mod required;
pub mod sentence;

pub use required::{Protection, Required, Rule, Source, required};
pub use sentence::{ModelWord, Reading, ReadingState, model_ratified, sentence};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// The account a review came from, as the forge reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: u64,
    pub login: String,
    /// `user.type`, verbatim.
    #[serde(rename = "type")]
    pub account_type: String,
}

/// A review's state, in the forge's own spelling. Anything else is kept as
/// received and neither counted nor dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
    Pending,
    Other(String),
}

impl ReviewState {
    /// Exact spellings only; no case folding.
    pub fn parse(s: &str) -> Self {
        match s {
            "APPROVED" => Self::Approved,
            "CHANGES_REQUESTED" => Self::ChangesRequested,
            "COMMENTED" => Self::Commented,
            "DISMISSED" => Self::Dismissed,
            "PENDING" => Self::Pending,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Approved => "APPROVED",
            Self::ChangesRequested => "CHANGES_REQUESTED",
            Self::Commented => "COMMENTED",
            Self::Dismissed => "DISMISSED",
            Self::Pending => "PENDING",
            Self::Other(s) => s,
        }
    }

    /// Approves, requests changes or was dismissed: the states that say
    /// where a reviewer stands. A comment does not replace one.
    fn is_opinionated(&self) -> bool {
        matches!(
            self,
            Self::Approved | Self::ChangesRequested | Self::Dismissed
        )
    }
}

impl From<String> for ReviewState {
    fn from(s: String) -> Self {
        Self::parse(&s)
    }
}

impl From<ReviewState> for String {
    fn from(s: ReviewState) -> Self {
        s.as_str().to_string()
    }
}

/// One entry of the forge's review list, as read. The body, the author's
/// association and every URL are not part of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub id: u64,
    /// `None` when the forge returned no user (a deleted account).
    pub account: Option<Account>,
    pub state: ReviewState,
    /// `None` when the commit no longer exists.
    pub commit: Option<String>,
    pub submitted_at: Option<Timestamp>,
}

/// Who a reviewer is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// The account type is exactly `User`.
    Person,
    /// The account exactly matches a model reviewer a vendor documents.
    Model { product: String, vendor: String },
    /// The account type is exactly `Bot`, and it is no documented model.
    Bot,
    /// No user, or an account type that is neither.
    Unclassified { received: String },
}

/// The kind of the account behind a review on `host`. Whole-string equality
/// only: no substring, no case folding, no pattern.
pub fn classify(host: &str, account: Option<&Account>) -> Kind {
    let Some(a) = account else {
        return Kind::Unclassified {
            received: "no user".into(),
        };
    };
    match a.account_type.as_str() {
        "User" => Kind::Person,
        "Bot" => crate::core::vendors::MODEL_REVIEWERS
            .iter()
            .find(|m| m.host == host && m.account_type == "Bot" && m.login == a.login)
            .map(|m| Kind::Model {
                product: m.product.into(),
                vendor: m.vendor.into(),
            })
            .unwrap_or(Kind::Bot),
        other => Kind::Unclassified {
            received: other.to_string(),
        },
    }
}

/// Where a review sits against the head the reading compared it with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnHead {
    Head,
    /// On another commit, named in [`Bearing::commit`].
    Older,
    /// The commit no longer exists.
    Gone,
}

impl OnHead {
    fn of(commit: Option<&str>, head: &str) -> Self {
        match commit {
            None => Self::Gone,
            Some(c) if c == head => Self::Head,
            Some(_) => Self::Older,
        }
    }
}

/// One reviewer's standing: their latest review that approves, requests
/// changes or was dismissed — or a review in a state the forge did not
/// document, listed as received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bearing {
    pub account: Option<Account>,
    #[serde(flatten)]
    pub kind: Kind,
    pub state: ReviewState,
    pub commit: Option<String>,
    pub on_head: OnHead,
    /// A fact as the forge gave it; never subtracted from another.
    pub submitted_at: Option<Timestamp>,
}

impl Bearing {
    pub fn is_approval(&self) -> bool {
        self.state == ReviewState::Approved
    }
}

/// Each reviewer's bearing review, in the order those reviews were
/// submitted. Reviews arrive in the forge's documented chronological order.
/// A review with no account stands alone; a pending one is not shown.
pub fn bearing(host: &str, reviews: &[Review], head: &str) -> Vec<Bearing> {
    // (position of the bearing review, the bearing).
    let mut by_reviewer: std::collections::BTreeMap<u64, (usize, Bearing)> = Default::default();
    let mut alone: Vec<(usize, Bearing)> = Vec::new();
    for (i, r) in reviews.iter().enumerate() {
        let entry = || Bearing {
            account: r.account.clone(),
            kind: classify(host, r.account.as_ref()),
            state: r.state.clone(),
            commit: r.commit.clone(),
            on_head: OnHead::of(r.commit.as_deref(), head),
            submitted_at: r.submitted_at,
        };
        match &r.state {
            ReviewState::Pending | ReviewState::Commented => {}
            // A state nobody documented: listed as received, never counted,
            // never standing in for a reviewer's approval.
            ReviewState::Other(_) => alone.push((i, entry())),
            s if s.is_opinionated() => match &r.account {
                Some(a) => {
                    by_reviewer.insert(a.id, (i, entry()));
                }
                None => alone.push((i, entry())),
            },
            _ => {}
        }
    }
    let mut all: Vec<(usize, Bearing)> = by_reviewer.into_values().chain(alone).collect();
    all.sort_by_key(|(i, _)| *i);
    all.into_iter().map(|(_, b)| b).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn account(id: u64, login: &str, ty: &str) -> Account {
        Account {
            id,
            login: login.into(),
            account_type: ty.into(),
        }
    }

    pub fn review(id: u64, who: Option<Account>, state: &str, commit: Option<&str>) -> Review {
        Review {
            id,
            account: who,
            state: ReviewState::parse(state),
            commit: commit.map(str::to_string),
            submitted_at: None,
        }
    }

    pub const COPILOT: &str = "copilot-pull-request-reviewer[bot]";

    #[test]
    fn a_kind_comes_from_the_account_type_and_a_documented_identity_only() {
        let k = |host: &str, a: Option<Account>| classify(host, a.as_ref());
        assert_eq!(
            k("github.com", Some(account(1, "alice", "User"))),
            Kind::Person
        );
        assert_eq!(
            k("github.com", Some(account(2, COPILOT, "Bot"))),
            Kind::Model {
                product: "Copilot code review".into(),
                vendor: "GitHub".into()
            }
        );
        for login in [
            "copilot-helper[bot]",
            "Copilot",
            "COPILOT-PULL-REQUEST-REVIEWER[bot]",
        ] {
            assert_eq!(
                k("github.com", Some(account(3, login, "Bot"))),
                Kind::Bot,
                "{login} is a bot account, never a model"
            );
        }
        // The documented login, but as a person's account type, is a person's.
        assert_eq!(
            k("github.com", Some(account(2, COPILOT, "User"))),
            Kind::Person
        );
        // No identity is documented for an Enterprise host.
        assert_eq!(k("ghe.corp", Some(account(2, COPILOT, "Bot"))), Kind::Bot);
        assert_eq!(
            k("github.com", None),
            Kind::Unclassified {
                received: "no user".into()
            }
        );
        for ty in ["Organization", "Mannequin", "bot", ""] {
            assert_eq!(
                k("github.com", Some(account(4, "x", ty))),
                Kind::Unclassified {
                    received: ty.into()
                }
            );
        }
    }

    #[test]
    fn the_latest_opinionated_review_bears_and_a_comment_does_not_replace_it() {
        let alice = || Some(account(1, "alice", "User"));
        let b = |rs: &[Review]| bearing("github.com", rs, "head");
        // Approve, then comment: the approval stands.
        let got = b(&[
            review(1, alice(), "APPROVED", Some("head")),
            review(2, alice(), "COMMENTED", Some("head")),
        ]);
        assert_eq!(got.len(), 1);
        assert!(got[0].is_approval());
        // Approve, then request changes: the request stands.
        let got = b(&[
            review(1, alice(), "APPROVED", Some("head")),
            review(2, alice(), "CHANGES_REQUESTED", Some("head")),
        ]);
        assert_eq!(got[0].state, ReviewState::ChangesRequested);
        assert!(!got[0].is_approval());
        // Approve, then dismissed: not counted.
        let got = b(&[
            review(1, alice(), "APPROVED", Some("old")),
            review(2, alice(), "DISMISSED", Some("old")),
        ]);
        assert_eq!(got[0].state, ReviewState::Dismissed);
        assert!(!got[0].is_approval());
        // Pending is not shown.
        assert!(b(&[review(1, alice(), "PENDING", None)]).is_empty());
        // A state nobody documented is listed verbatim and never counted.
        let got = b(&[
            review(1, alice(), "APPROVED", Some("head")),
            review(2, alice(), "X", Some("head")),
        ]);
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].state, ReviewState::Other("X".into()));
        assert_eq!(got.iter().filter(|g| g.is_approval()).count(), 1);
    }

    #[test]
    fn where_an_approval_sits_against_the_head() {
        let got = bearing(
            "github.com",
            &[
                review(1, Some(account(1, "a", "User")), "APPROVED", Some("head")),
                review(
                    2,
                    Some(account(2, "b", "User")),
                    "APPROVED",
                    Some("9e1d0aa"),
                ),
                review(3, Some(account(3, "c", "User")), "APPROVED", None),
                // No account: each review is its own entry.
                review(4, None, "APPROVED", Some("head")),
                review(5, None, "APPROVED", Some("head")),
            ],
            "head",
        );
        let on: Vec<OnHead> = got.iter().map(|g| g.on_head).collect();
        assert_eq!(
            on,
            vec![
                OnHead::Head,
                OnHead::Older,
                OnHead::Gone,
                OnHead::Head,
                OnHead::Head
            ]
        );
        assert_eq!(got[1].commit.as_deref(), Some("9e1d0aa"));
    }

    #[test]
    fn a_state_reads_and_writes_in_the_forges_spelling() {
        for s in [
            "APPROVED",
            "CHANGES_REQUESTED",
            "COMMENTED",
            "DISMISSED",
            "PENDING",
        ] {
            assert_eq!(ReviewState::parse(s).as_str(), s);
            assert!(!matches!(ReviewState::parse(s), ReviewState::Other(_)));
        }
        assert_eq!(
            ReviewState::parse("approved"),
            ReviewState::Other("approved".into())
        );
    }
}

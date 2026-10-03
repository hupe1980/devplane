//! A pull request's reviews and its base branch's review requirement, read
//! over GitHub's documented REST API. Read-only: every request here is a
//! `GET`, and nothing is written to the forge.
//!
//! Used by the host's poller with the signed-in token and by `devplane
//! verify --approvals` with the job's own token; both compose the same
//! [`Reading`] through [`crate::core::approvals`].

use super::{Client, Error};
use crate::core::approvals::{self, Account, Protection, Reading, Review, ReviewState, Rule};
use serde::Deserialize;
use serde_json::Value;

/// Reviews per page, GitHub's documented maximum.
pub const PER_PAGE: usize = 100;
/// Pages read before the reading stops and says how many it left.
pub const MAX_PAGES: u32 = 10;

/// What protection's refusal means for the count.
pub const PROTECTION_UNREADABLE: &str = "the token cannot read branch protection";

/// Only the fields the reading uses: the body, the author's association and
/// every URL are never deserialised.
#[derive(Deserialize)]
struct RawReview {
    id: u64,
    #[serde(default)]
    user: Option<RawUser>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    commit_id: Option<String>,
    #[serde(default)]
    submitted_at: Option<String>,
}

#[derive(Deserialize)]
struct RawUser {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    login: String,
    #[serde(default, rename = "type")]
    account_type: String,
}

/// One page of the review list, as reviews. An entry that is not a review
/// is skipped, never guessed into one.
pub fn parse_page(body: &Value) -> Vec<Review> {
    body.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|v| serde_json::from_value::<RawReview>(v.clone()).ok())
                .map(|r| Review {
                    id: r.id,
                    account: r.user.map(|u| Account {
                        id: u.id,
                        login: u.login,
                        account_type: u.account_type,
                    }),
                    state: ReviewState::parse(&r.state),
                    commit: r.commit_id.filter(|c| !c.is_empty()),
                    submitted_at: r.submitted_at.and_then(|t| t.parse().ok()),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The `pull_request` rules among a branch's active rules.
pub fn parse_rules(body: &Value) -> Vec<Rule> {
    body.as_array()
        .map(|items| {
            items
                .iter()
                .filter(|r| r["type"] == "pull_request")
                .map(|r| {
                    let p = &r["parameters"];
                    Rule {
                        count: p["required_approving_review_count"]
                            .as_u64()
                            .unwrap_or(0)
                            .min(u64::from(u32::MAX)) as u32,
                        dismiss_stale: p["dismiss_stale_reviews_on_push"]
                            .as_bool()
                            .unwrap_or(false),
                        code_owner: p["require_code_owner_review"].as_bool().unwrap_or(false),
                        last_push: p["require_last_push_approval"].as_bool().unwrap_or(false),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Classic protection's review requirement; `None` when it requires no
/// review.
pub fn parse_protection(body: &Value) -> Option<Protection> {
    let r = body.get("required_pull_request_reviews")?;
    if !r.is_object() {
        return None;
    }
    Some(Protection {
        count: r["required_approving_review_count"]
            .as_u64()
            .unwrap_or(0)
            .min(u64::from(u32::MAX)) as u32,
        dismiss_stale: r["dismiss_stale_reviews"].as_bool().unwrap_or(false),
        code_owner: r["require_code_owner_reviews"].as_bool().unwrap_or(false),
        last_push: r["require_last_push_approval"].as_bool().unwrap_or(false),
    })
}

/// A path segment: a branch name may hold `/`, which GitHub's routes take
/// as written; anything else outside the unreserved set is escaped.
fn segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Every review on the pull request, to [`MAX_PAGES`] pages, and how many
/// pages past that bound were not read.
pub async fn reviews(
    client: &Client,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<(Vec<Review>, Option<u32>), Error> {
    let mut all = Vec::new();
    for page in 1..=MAX_PAGES {
        let path = format!(
            "/repos/{}/{}/pulls/{number}/reviews?per_page={PER_PAGE}&page={page}",
            segment(owner),
            segment(repo)
        );
        let a = client.rest(reqwest::Method::GET, &path, None).await?;
        let got = a.body.as_array().map(Vec::len).unwrap_or(0);
        all.extend(parse_page(&a.body));
        if got < PER_PAGE {
            return Ok((all, None));
        }
        if page == MAX_PAGES {
            // GitHub's `last` page, when it says; at least one otherwise.
            let unread = a
                .last_page
                .map(|l| l.saturating_sub(MAX_PAGES))
                .filter(|n| *n > 0)
                .unwrap_or(1);
            return Ok((all, Some(unread)));
        }
    }
    Ok((all, None))
}

/// The `pull_request` rules active on `base`.
pub async fn rules(
    client: &Client,
    owner: &str,
    repo: &str,
    base: &str,
) -> Result<Vec<Rule>, Error> {
    let path = format!(
        "/repos/{}/{}/rules/branches/{}?per_page=100",
        segment(owner),
        segment(repo),
        segment(base)
    );
    let a = client.rest(reqwest::Method::GET, &path, None).await?;
    Ok(parse_rules(&a.body))
}

/// Classic protection's review requirement on `base`: `Ok(None)` when the
/// branch is not protected or requires no review.
pub async fn protection(
    client: &Client,
    owner: &str,
    repo: &str,
    base: &str,
) -> Result<Option<Protection>, Error> {
    let path = format!(
        "/repos/{}/{}/branches/{}/protection",
        segment(owner),
        segment(repo),
        segment(base)
    );
    match client.rest(reqwest::Method::GET, &path, None).await {
        Ok(a) => Ok(parse_protection(&a.body)),
        Err(Error::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Why a count could not be read, as one clause.
fn why(e: &Error) -> String {
    match e {
        Error::Forbidden(_) | Error::Expired(_) => PROTECTION_UNREADABLE.into(),
        other => other.to_string(),
    }
}

/// What a reading needs to know about the pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target<'a> {
    /// The forge host the reviews are classified for (`github.com`).
    pub host: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
    pub number: u64,
    pub url: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub after_merge: bool,
}

/// Reads the reviews and the required count and composes the reading. A
/// review list that cannot be read is the error; a count that cannot be read
/// is said inside the reading. `now` is read by the caller.
pub async fn read(client: &Client, t: &Target<'_>, now: jiff::Timestamp) -> Result<Reading, Error> {
    let (list, unread_pages) = reviews(client, t.owner, t.repo, t.number).await?;
    let rules = rules(client, t.owner, t.repo, t.base).await;
    // Classic protection only when no rule says a count.
    let protection = match &rules {
        Ok(r) if !r.is_empty() => None,
        _ => Some(
            protection(client, t.owner, t.repo, t.base)
                .await
                .map_err(|e| why(&e)),
        ),
    };
    let rules = rules.map_err(|e| match e {
        Error::Forbidden(_) => "the token cannot read the branch's rules".to_string(),
        other => other.to_string(),
    });
    let required = approvals::required(rules, protection);
    Ok(Reading {
        number: t.number,
        url: t.url.to_string(),
        head: t.head.to_string(),
        base: t.base.to_string(),
        required,
        required_says: None,
        bearing: approvals::bearing(t.host, &list, t.head),
        unread_pages,
        sentence: String::new(),
        model_word: approvals::ModelWord::Unattributed,
        read_at: now,
        after_merge: t.after_merge,
    }
    .composed(approvals::model_ratified()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_page_reads_only_the_fields_the_reading_uses() {
        let planted = "B".repeat(1 << 20);
        let page = json!([
            {"id": 1, "user": {"login": "alice", "id": 10, "type": "User"},
             "body": planted, "state": "APPROVED", "commit_id": "abc",
             "author_association": "MEMBER", "html_url": "https://x",
             "submitted_at": "2026-10-01T10:00:00Z"},
            {"id": 2, "user": null, "body": "", "state": "DISMISSED", "commit_id": null},
            {"id": 3, "user": {"login": "copilot-pull-request-reviewer[bot]", "id": 20, "type": "Bot"},
             "state": "PENDING", "commit_id": "abc"},
            "not a review"
        ]);
        let got = parse_page(&page);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].account.as_ref().unwrap().login, "alice");
        assert_eq!(got[0].account.as_ref().unwrap().account_type, "User");
        assert_eq!(got[0].state, ReviewState::Approved);
        assert!(got[0].submitted_at.is_some());
        assert_eq!(got[1].account, None);
        assert_eq!(got[1].commit, None);
        assert_eq!(got[2].state, ReviewState::Pending);
        assert!(
            !format!("{got:?}").contains("BBBB"),
            "the body is never read"
        );
    }

    #[test]
    fn rules_and_protection_read_their_documented_fields() {
        let rules = parse_rules(&json!([
            {"type": "required_signatures"},
            {"type": "copilot_code_review", "parameters": {"review_on_push": true}},
            {"type": "pull_request", "parameters": {
                "required_approving_review_count": 2, "dismiss_stale_reviews_on_push": true,
                "require_code_owner_review": false, "require_last_push_approval": true,
                "required_reviewers": [{"x": 1}]}}
        ]));
        assert_eq!(
            rules,
            vec![Rule {
                count: 2,
                dismiss_stale: true,
                code_owner: false,
                last_push: true
            }]
        );
        assert_eq!(
            parse_protection(&json!({"required_pull_request_reviews": {
                "required_approving_review_count": 1, "dismiss_stale_reviews": true,
                "require_code_owner_reviews": true}})),
            Some(Protection {
                count: 1,
                dismiss_stale: true,
                code_owner: true,
                last_push: false
            })
        );
        assert_eq!(parse_protection(&json!({"enforce_admins": {}})), None);
    }

    #[test]
    fn a_refusal_of_protection_is_said_as_unreadable() {
        assert_eq!(why(&Error::Forbidden("x".into())), PROTECTION_UNREADABLE);
        assert!(why(&Error::Unreachable("timed out".into())).contains("timed out"));
    }

    #[test]
    fn a_branch_name_stays_one_path() {
        assert_eq!(segment("release/1.0"), "release/1.0");
        assert_eq!(segment("a b?c#d"), "a%20b%3Fc%23d");
    }

    #[test]
    fn the_module_has_no_write_method() {
        let src = include_str!("reviews.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for m in ["POST", "PUT", "PATCH", "DELETE"] {
            assert!(
                !code.contains(&format!("Method::{m}")),
                "reviews.rs names Method::{m}"
            );
        }
        assert!(code.contains("Method::GET"));
    }
}

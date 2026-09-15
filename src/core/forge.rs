//! What the forge says about a project: its open issues and pull requests.
//!
//! A control plane for *all* of somebody's projects has to show the things
//! waiting on them, and half of those are not sessions — they are the issues
//! and pull requests on GitHub, per repository, that a person otherwise finds
//! by opening eight browser tabs. So GitHub is an observed source beside the
//! roster: polled, kept in memory, rebuilt on restart, never written to from
//! here. Nothing in this module can post a comment, apply a label or merge.
//!
//! The types are the forge's facts reduced to what a board needs. The
//! `gh`-shaped structs and the process that runs `gh` live in `crate::github`,
//! on the other side of the purity line; this side only derives.

use crate::core::attention::{Action, AttentionItem, AttentionKind, Snoozed};
use crate::core::ids::{AttentionId, ProjectId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// An open issue, as the board shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgeIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub labels: Vec<String>,
    /// Assigned to the person whose `gh` this is.
    #[serde(default)]
    pub assigned_to_me: bool,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// An open pull request, as the board shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgePullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    /// The one word a person has to act on — `failing`, `ready_to_merge`,
    /// `changes_requested`, … — the same vocabulary the Work items use.
    pub status: String,
    #[serde(default)]
    pub draft: bool,
    /// Authored by the person whose `gh` this is.
    #[serde(default)]
    pub mine: bool,
    /// A review was requested from that person, directly or through a team.
    #[serde(default)]
    pub review_requested: bool,
    #[serde(default)]
    pub head_ref: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl ForgePullRequest {
    /// The one thing this pull request asks of the person, if it asks
    /// anything.
    ///
    /// The heading's count and the inbox's rows are the same question, so they
    /// are the same function: [`needs_me`] is this, and [`items_for_forge`]
    /// renders whatever kind comes back. A heading that counts one thing while
    /// the inbox lists another is a board that lies.
    ///
    /// **A draft is the author saying the work is not finished**, and that
    /// governs what the author is asked about: red checks and a waiting
    /// approval are both statements about finished work. What still reaches
    /// them is what a *person* asked for — a review requested of them, or
    /// changes requested on their own.
    ///
    /// [`needs_me`]: Self::needs_me
    pub fn asks_of_me(&self) -> Option<AttentionKind> {
        if self.review_requested {
            return Some(AttentionKind::ReviewRequested);
        }
        if !self.mine {
            return None;
        }
        match self.status.as_str() {
            "changes_requested" => Some(AttentionKind::ChangesRequested),
            "failing" if !self.draft => Some(AttentionKind::CiRed),
            "ready_to_merge" if !self.draft => Some(AttentionKind::PrReady),
            _ => None,
        }
    }

    /// Whether this pull request is waiting on the person rather than on
    /// somebody else or on a machine.
    pub fn needs_me(&self) -> bool {
        self.asks_of_me().is_some()
    }
}

/// One project's forge, as of the last poll.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectForge {
    pub project_id: ProjectId,
    /// The `owner/name` slug, when the remote is a recognisable forge URL. It
    /// is what a launch link needs.
    #[serde(default)]
    pub repo: Option<String>,
    pub fetched_at: Timestamp,
    #[serde(default)]
    pub issues: Vec<ForgeIssue>,
    #[serde(default)]
    pub pull_requests: Vec<ForgePullRequest>,
    /// Why the last poll produced nothing. Kept beside stale data rather than
    /// replacing it: "GitHub was unreachable at 10:42" is a fact the board can
    /// show; an empty list is not.
    #[serde(default)]
    pub error: Option<String>,
}

/// The numbers a project heading carries, and why they might be wrong.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeCounts {
    pub issues: usize,
    pub pull_requests: usize,
    /// Assigned issues, requested reviews, and the person's own pull requests
    /// that are red, contested or approved and unmerged.
    pub needs_you: usize,
    /// Why the last poll of *this* project failed, if it did. The counts
    /// beside it are the last good ones.
    ///
    /// A count that is quietly stale is worse than no count: the heading's
    /// promise is that it says what GitHub says now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<String>,
}

impl ProjectForge {
    pub fn counts(&self) -> ForgeCounts {
        ForgeCounts {
            issues: self.issues.len(),
            pull_requests: self.pull_requests.len(),
            needs_you: self.issues.iter().filter(|i| i.assigned_to_me).count()
                + self.pull_requests.iter().filter(|p| p.needs_me()).count(),
            stale: self.error.clone(),
        }
    }
}

/// Parses the forge's timestamp, so an item's age is the thing's own age and
/// not the moment this daemon first saw it — otherwise everything discovered
/// in one poll is exactly as old as everything else.
fn since(updated_at: Option<&str>, fallback: Timestamp) -> Timestamp {
    updated_at
        .and_then(|s| s.parse::<Timestamp>().ok())
        .unwrap_or(fallback)
}

/// The inbox items one project's forge produces.
///
/// `own_prs` are the pull requests Vibeplane itself opened for this project.
/// Their Work already raises `ci_red`, `changes_requested` and `pr_ready`, so
/// they are skipped here rather than reported twice under two ids.
///
/// Only what needs *this* person: an issue assigned to them, a review asked
/// of them, or their own pull request that is red, contested, or approved and
/// waiting for a merge. Everything else is a count on the board, not a row in
/// the inbox — a list that names every open issue in every repository is a
/// list nobody reads.
pub fn items_for_forge(
    forge: &ProjectForge,
    snoozed: &Snoozed,
    own_prs: &BTreeSet<u64>,
) -> Vec<AttentionItem> {
    let mut out = Vec::new();
    let launch = |prompt: String| -> Option<String> {
        crate::core::deeplink::open_repo(forge.repo.as_deref()?, &prompt)
    };
    let mk = |kind: AttentionKind,
              number: u64,
              title: String,
              detail: Option<String>,
              url: &str,
              since_at: Timestamp,
              actions: Vec<Action>,
              launch: Option<String>| {
        if snoozed.hides(&kind) {
            return None;
        }
        Some(AttentionItem {
            id: AttentionId::new(format!(
                "gh:{}:{}:{number}",
                forge.project_id.as_str(),
                kind.as_str()
            )),
            // **Normal, whatever the kind defaults to, because a poll is not
            // an event**: nothing here just happened, a sweep noticed a
            // standing condition. `high` is not a label — it fires a desktop
            // notification and outranks live sessions in the inbox, and an
            // agent waiting on an answer now beats a week-old red check.
            level: crate::core::attention::Level::Normal,
            kind,
            run_id: None,
            project_id: Some(forge.project_id.clone()),
            title,
            detail,
            options: vec![],
            actions,
            request_id: None,
            url: Some(url.to_string()),
            launch,
            work_id: None,
            suggested_rule: None,
            since: since_at,
        })
    };

    for issue in &forge.issues {
        if !issue.assigned_to_me {
            continue;
        }
        let detail = (!issue.labels.is_empty()).then(|| issue.labels.join(", "));
        out.extend(mk(
            AttentionKind::IssueAssigned,
            issue.number,
            format!("#{} is assigned to you: {}", issue.number, issue.title),
            detail,
            &issue.url,
            since(issue.updated_at.as_deref(), forge.fetched_at),
            vec![Action::OpenIssue, Action::Snooze],
            launch(format!(
                "Look at GitHub issue #{} — {}. Confirm the problem in the code before changing anything.",
                issue.number, issue.title
            )),
        ));
    }

    for pr in &forge.pull_requests {
        if own_prs.contains(&pr.number) {
            continue;
        }
        let at = since(pr.updated_at.as_deref(), forge.fetched_at);
        let Some(kind) = pr.asks_of_me() else {
            continue;
        };
        let (title, launch) = match kind {
            AttentionKind::ReviewRequested => (
                format!("Review requested: #{} {}", pr.number, pr.title),
                None,
            ),
            AttentionKind::CiRed => (
                format!("Checks are red on your #{} {}", pr.number, pr.title),
                launch(format!(
                    "The checks on pull request #{} are failing. Find out why and fix it.",
                    pr.number
                )),
            ),
            AttentionKind::ChangesRequested => (
                format!("Changes requested on your #{} {}", pr.number, pr.title),
                launch(format!(
                    "A reviewer asked for changes on pull request #{}. Read the comments and address them.",
                    pr.number
                )),
            ),
            AttentionKind::PrReady => (
                format!("#{} is approved and green: {}", pr.number, pr.title),
                None,
            ),
            // `asks_of_me` returns only the four above.
            _ => continue,
        };
        out.extend(mk(
            kind,
            pr.number,
            title,
            Some(format!("branch {}", pr.head_ref)),
            &pr.url,
            at,
            vec![Action::OpenPr, Action::Snooze],
            launch,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forge() -> ProjectForge {
        ProjectForge {
            project_id: ProjectId::new("p1"),
            repo: Some("acme/app".into()),
            fetched_at: Timestamp::now(),
            issues: vec![
                ForgeIssue {
                    number: 7,
                    title: "Login fails".into(),
                    url: "https://github.com/acme/app/issues/7".into(),
                    labels: vec!["bug".into()],
                    assigned_to_me: true,
                    updated_at: Some("2026-09-01T10:00:00Z".into()),
                },
                ForgeIssue {
                    number: 8,
                    title: "Somebody else's".into(),
                    url: String::new(),
                    labels: vec![],
                    assigned_to_me: false,
                    updated_at: None,
                },
            ],
            pull_requests: vec![
                ForgePullRequest {
                    number: 142,
                    title: "Fix flaky test".into(),
                    url: "https://github.com/acme/app/pull/142".into(),
                    status: "failing".into(),
                    draft: false,
                    mine: true,
                    review_requested: false,
                    head_ref: "fix/flaky".into(),
                    updated_at: None,
                },
                ForgePullRequest {
                    number: 143,
                    title: "Please review".into(),
                    url: "https://github.com/acme/app/pull/143".into(),
                    status: "ready_for_review".into(),
                    draft: false,
                    mine: false,
                    review_requested: true,
                    head_ref: "feat/x".into(),
                    updated_at: None,
                },
                ForgePullRequest {
                    number: 144,
                    title: "Not mine, not asked".into(),
                    url: String::new(),
                    status: "ready_for_review".into(),
                    draft: false,
                    mine: false,
                    review_requested: false,
                    head_ref: "x".into(),
                    updated_at: None,
                },
            ],
            error: None,
        }
    }

    #[test]
    fn the_counts_are_what_the_heading_says() {
        let c = forge().counts();
        assert_eq!((c.issues, c.pull_requests), (2, 3));
        // One assigned issue, one red PR of mine, one review asked of me.
        assert_eq!(c.needs_you, 3);
    }

    #[test]
    fn only_what_needs_this_person_reaches_the_inbox() {
        let items = items_for_forge(&forge(), &Snoozed::default(), &BTreeSet::new());
        let kinds: Vec<_> = items.iter().map(|i| i.kind.as_str()).collect();
        assert_eq!(kinds, ["issue_assigned", "ci_red", "review_requested"]);
        // Every item carries where to go, and nothing here can write to GitHub.
        assert!(items.iter().all(|i| i.url.is_some()));
        assert!(items.iter().all(|i| !i.actions.contains(&Action::Allow)));
        // A red PR of mine gets the same launch link a Work's `ci_red` does —
        // the slug is percent-encoded inside it, so the check is on the scheme.
        assert!(
            items[1]
                .launch
                .as_deref()
                .unwrap_or("")
                .starts_with("claude-cli://open?repo=acme%2Fapp")
        );
    }

    #[test]
    fn a_pull_request_vibeplane_opened_is_not_reported_twice() {
        // Its Work already raises `ci_red`; a second row under a second id
        // would be the same fact asking twice.
        let own = BTreeSet::from([142u64]);
        let items = items_for_forge(&forge(), &Snoozed::default(), &own);
        assert!(items.iter().all(|i| i.kind != AttentionKind::CiRed));
    }

    #[test]
    fn a_snooze_hides_the_kind_and_only_the_kind() {
        let mut s = Snoozed::default();
        s.hide(
            [AttentionKind::ReviewRequested],
            Timestamp::now() + jiff::SignedDuration::from_hours(1),
        );
        let items = items_for_forge(&forge(), &s, &BTreeSet::new());
        assert!(
            items
                .iter()
                .all(|i| i.kind != AttentionKind::ReviewRequested)
        );
        assert!(items.iter().any(|i| i.kind == AttentionKind::IssueAssigned));
    }

    #[test]
    fn an_items_age_is_the_forges_not_the_daemons() {
        let items = items_for_forge(&forge(), &Snoozed::default(), &BTreeSet::new());
        let issue = items
            .iter()
            .find(|i| i.kind == AttentionKind::IssueAssigned)
            .unwrap();
        assert_eq!(issue.since.to_string(), "2026-09-01T10:00:00Z");
    }

    #[test]
    fn nothing_the_forge_produces_interrupts_a_person() {
        // `ci_red` and `changes_requested` are *high* by default, and high is
        // not a label: it fires a desktop notification, and the notifier's
        // memory lasts one daemon. Four red pull requests meant four
        // notifications at every restart, about four things that had been true
        // for days. A poll is not an event — it notices standing conditions.
        let mut f = forge();
        f.pull_requests[0].status = "changes_requested".into();
        let items = items_for_forge(&f, &Snoozed::default(), &BTreeSet::new());
        assert!(!items.is_empty());
        for i in &items {
            assert_eq!(
                i.level,
                crate::core::attention::Level::Normal,
                "{} would interrupt somebody",
                i.kind.as_str()
            );
        }
        // And a live session asking a question still outranks all of it.
        let asking = crate::core::attention::Level::High;
        assert!(asking > items[0].level);
    }

    #[test]
    fn a_draft_of_your_own_does_not_ask_you_for_anything() {
        // You marked it a draft; red checks on unfinished work are its
        // ordinary condition, and this is the "twelve closed editor tabs"
        // mistake one source further out — true, and not news.
        let mut f = forge();
        f.pull_requests[0].draft = true; // #142: mine, checks failing
        assert_eq!(f.pull_requests[0].asks_of_me(), None);
        let items = items_for_forge(&f, &Snoozed::default(), &BTreeSet::new());
        assert!(items.iter().all(|i| i.kind != AttentionKind::CiRed));
        assert_eq!(f.counts().needs_you, 2, "and the heading agrees");
    }

    #[test]
    fn what_a_person_asked_for_reaches_you_through_a_draft() {
        // A machine's verdict on unfinished work is noise. Somebody spending
        // their time to ask is not, and a review requested of you by name is
        // a direct request whatever state the branch is in.
        let mut pr = forge().pull_requests[0].clone();
        pr.draft = true;
        pr.status = "changes_requested".into();
        assert_eq!(pr.asks_of_me(), Some(AttentionKind::ChangesRequested));

        let mut asked = forge().pull_requests[1].clone();
        asked.draft = true;
        assert!(asked.review_requested);
        assert_eq!(asked.asks_of_me(), Some(AttentionKind::ReviewRequested));
    }

    #[test]
    fn the_heading_counts_exactly_what_the_inbox_lists() {
        // The heading's number and the inbox's rows were two independent
        // readings of one question — `counts` called `needs_me`, and
        // `items_for_forge` ran its own `match` over the same statuses. They
        // are one function now, and this is what says so: over every status
        // and every combination of the flags that gate them, a pull request
        // counts if and only if it produces a row.
        let mut f = forge();
        f.issues.clear();
        for status in [
            "failing",
            "changes_requested",
            "ready_to_merge",
            "ready_for_review",
            "pending",
            "draft",
            "merged",
        ] {
            for draft in [false, true] {
                for mine in [false, true] {
                    for review_requested in [false, true] {
                        f.pull_requests = vec![ForgePullRequest {
                            number: 1,
                            title: "t".into(),
                            url: "u".into(),
                            status: status.into(),
                            draft,
                            mine,
                            review_requested,
                            head_ref: "h".into(),
                            updated_at: None,
                        }];
                        let counted = f.counts().needs_you;
                        let listed =
                            items_for_forge(&f, &Snoozed::default(), &BTreeSet::new()).len();
                        assert_eq!(
                            counted, listed,
                            "{status} draft={draft} mine={mine} review={review_requested}: \
                             the heading says {counted} and the inbox lists {listed}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_stale_error_is_kept_beside_stale_data_and_reaches_the_heading() {
        let mut f = forge();
        f.error = Some("gh: connection refused".into());
        assert_eq!(f.counts().issues, 2, "an error does not empty the board");
        // And it is *carried*, which it was not: the poller wrote this field
        // and nothing served it, so a project whose reads had been failing for
        // an hour showed hour-old numbers as confidently as fresh ones.
        assert_eq!(
            f.counts().stale.as_deref(),
            Some("gh: connection refused"),
            "a count that is quietly stale is worse than no count"
        );
        assert_eq!(forge().counts().stale, None, "and a good poll says nothing");
    }
}

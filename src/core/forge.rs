//! What the forge says about a project: its open issues and pull requests.
//! GitHub is an observed source, polled and kept in memory, never written to:
//! nothing here can comment, label or merge. The process that runs `gh` lives
//! in `crate::github`; this side only derives.

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
    /// `failing`, `ready_to_merge`, `changes_requested`, …: the Change vocabulary.
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
    /// The one thing this pull request asks of the person, if any. Both the
    /// heading's count ([`needs_me`]) and the inbox rows ([`items_for_forge`])
    /// use this, so they cannot disagree. On a draft only what a person asked
    /// for reaches its author (a requested review, requested changes), not red
    /// checks or a waiting approval.
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

    /// Whether this pull request is waiting on the person.
    pub fn needs_me(&self) -> bool {
        self.asks_of_me().is_some()
    }
}

/// One project's forge, as of the last poll.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectForge {
    pub project_id: ProjectId,
    /// The `owner/name` slug a launch link needs.
    #[serde(default)]
    pub repo: Option<String>,
    pub fetched_at: Timestamp,
    #[serde(default)]
    pub issues: Vec<ForgeIssue>,
    #[serde(default)]
    pub pull_requests: Vec<ForgePullRequest>,
    /// Why the last poll failed, kept beside the stale data rather than
    /// replacing it with an empty list.
    #[serde(default)]
    pub error: Option<String>,
}

/// The numbers a project heading carries, and why they might be wrong.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeCounts {
    pub issues: usize,
    pub pull_requests: usize,
    /// Assigned issues, requested reviews, and the person's own red, contested
    /// or approved-unmerged pull requests.
    pub needs_you: usize,
    /// Why the last poll of this project failed; the counts are the last good
    /// ones, and must not look fresh.
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

/// Parses the forge's timestamp, so an item's age is its own, not the poll's.
fn since(updated_at: Option<&str>, fallback: Timestamp) -> Timestamp {
    updated_at
        .and_then(|s| s.parse::<Timestamp>().ok())
        .unwrap_or(fallback)
}

/// The inbox items one project's forge produces: only what needs this person.
/// `own_prs` (opened by Devplane) are skipped, since their Change already
/// raises the same items.
pub fn items_for_forge(
    forge: &ProjectForge,
    snoozed: &Snoozed,
    own_prs: &BTreeSet<u64>,
) -> crate::core::attention::Derived {
    let mut out = Vec::new();
    // Hidden is counted, never dropped.
    let hidden = std::cell::Cell::new(0usize);
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
            hidden.set(hidden.get() + 1);
            return None;
        }
        Some(AttentionItem {
            id: AttentionId::new(format!(
                "gh:{}:{}:{number}",
                forge.project_id.as_str(),
                kind.as_str()
            )),
            // Normal whatever the kind's default: a poll notices standing
            // conditions, and `high` would fire a desktop notification and
            // outrank a live session waiting on an answer.
            level: crate::core::attention::Level::Normal,
            kind,
            run_id: None,
            project_id: Some(forge.project_id.clone()),
            title,
            detail,
            answer_in: None,
            ask: None,
            options: vec![],
            actions,
            request_id: None,
            form: None,
            url: Some(url.to_string()),
            launch,
            change_id: None,
            offer: None,
            no_offer: None,
            report: None,
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
    crate::core::attention::Derived {
        items: out,
        snoozed: hidden.get(),
    }
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
        let items = items_for_forge(&forge(), &Snoozed::default(), &BTreeSet::new()).items;
        let kinds: Vec<_> = items.iter().map(|i| i.kind.as_str()).collect();
        assert_eq!(kinds, ["issue_assigned", "ci_red", "review_requested"]);
        assert!(items.iter().all(|i| i.url.is_some()));
        assert!(items.iter().all(|i| !i.actions.contains(&Action::Allow)));
        // The slug is percent-encoded in the launch link; check the scheme.
        assert!(
            items[1]
                .launch
                .as_deref()
                .unwrap_or("")
                .starts_with("claude-cli://open?repo=acme%2Fapp")
        );
    }

    #[test]
    fn a_pull_request_devplane_opened_is_not_reported_twice() {
        // Its Change already raises `ci_red`.
        let own = BTreeSet::from([142u64]);
        let items = items_for_forge(&forge(), &Snoozed::default(), &own).items;
        assert!(items.iter().all(|i| i.kind != AttentionKind::CiRed));
    }

    #[test]
    fn a_snooze_hides_the_kind_and_only_the_kind() {
        let mut s = Snoozed::default();
        s.hide(
            [AttentionKind::ReviewRequested],
            Timestamp::now() + jiff::SignedDuration::from_hours(1),
        );
        let derived = items_for_forge(&forge(), &s, &BTreeSet::new());
        assert_eq!(derived.snoozed, 1, "hidden is counted, not dropped");
        let items = derived.items;
        assert!(
            items
                .iter()
                .all(|i| i.kind != AttentionKind::ReviewRequested)
        );
        assert!(items.iter().any(|i| i.kind == AttentionKind::IssueAssigned));
    }

    #[test]
    fn an_items_age_is_the_forges_not_the_hosts() {
        let items = items_for_forge(&forge(), &Snoozed::default(), &BTreeSet::new()).items;
        let issue = items
            .iter()
            .find(|i| i.kind == AttentionKind::IssueAssigned)
            .unwrap();
        assert_eq!(issue.since.to_string(), "2026-09-01T10:00:00Z");
    }

    #[test]
    fn nothing_the_forge_produces_interrupts_a_person() {
        // High by default, but a poll is not an event: no notification.
        let mut f = forge();
        f.pull_requests[0].status = "changes_requested".into();
        let items = items_for_forge(&f, &Snoozed::default(), &BTreeSet::new()).items;
        assert!(!items.is_empty());
        for i in &items {
            assert_eq!(
                i.level,
                crate::core::attention::Level::Normal,
                "{} would interrupt somebody",
                i.kind.as_str()
            );
        }
        let asking = crate::core::attention::Level::High;
        assert!(asking > items[0].level);
    }

    #[test]
    fn a_draft_of_your_own_does_not_ask_you_for_anything() {
        // Red checks on a draft are its ordinary condition.
        let mut f = forge();
        f.pull_requests[0].draft = true; // #142: mine, checks failing
        assert_eq!(f.pull_requests[0].asks_of_me(), None);
        let items = items_for_forge(&f, &Snoozed::default(), &BTreeSet::new()).items;
        assert!(items.iter().all(|i| i.kind != AttentionKind::CiRed));
        assert_eq!(f.counts().needs_you, 2, "and the heading agrees");
    }

    #[test]
    fn what_a_person_asked_for_reaches_you_through_a_draft() {
        // A person's request reaches the author whatever state the branch is in.
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
        // Over every status and flag combination, a pull request counts if and
        // only if it produces an inbox row.
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
                        let listed = items_for_forge(&f, &Snoozed::default(), &BTreeSet::new())
                            .items
                            .len();
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
        assert_eq!(
            f.counts().stale.as_deref(),
            Some("gh: connection refused"),
            "a count that is quietly stale is worse than no count"
        );
        assert_eq!(forge().counts().stale, None, "and a good poll says nothing");
    }
}

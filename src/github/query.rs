//! The GraphQL documents Devplane sends, and their mapping into the types the
//! board already uses. One document per repository per poll: open pull
//! requests (with their latest commit's checks) and open issues together;
//! and one per host for what is asked of the signed-in person.

use super::{Author, Check, Issue, Label, PullRequest};
use serde::Deserialize;
use serde_json::Value;

/// How many of each list one poll reads; the rest is a count.
pub const PAGE: u32 = 20;

const PR_FIELDS: &str = "number title url state isDraft headRefName reviewDecision \
mergeStateStatus updatedAt author { login } \
commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 50) { nodes { \
__typename \
... on CheckRun { name status conclusion detailsUrl checkSuite { workflowRun { workflow { name } } } } \
... on StatusContext { context state targetUrl } } } } } } }";

const ISSUE_FIELDS: &str = "number title body url updatedAt \
labels(first: 20) { nodes { name } } assignees(first: 20) { nodes { login } }";

/// A repository's open pull requests and issues, in one request, with the
/// name GitHub itself spells it by (a remote may differ in case, or name a
/// repository that was renamed).
pub fn snapshot() -> String {
    format!(
        "query($owner: String!, $name: String!, $first: Int!) {{ \
         repository(owner: $owner, name: $name) {{ nameWithOwner \
         pullRequests(states: OPEN, first: $first, orderBy: {{field: CREATED_AT, direction: DESC}}) {{ totalCount nodes {{ {PR_FIELDS} }} }} \
         issues(states: OPEN, first: $first, orderBy: {{field: CREATED_AT, direction: DESC}}) {{ totalCount nodes {{ {ISSUE_FIELDS} }} }} \
         }} }}"
    )
}

/// Open issues, optionally only those carrying a label.
pub fn issues() -> String {
    format!(
        "query($owner: String!, $name: String!, $first: Int!, $labels: [String!]) {{ \
         repository(owner: $owner, name: $name) {{ \
         issues(states: OPEN, labels: $labels, first: $first, orderBy: {{field: CREATED_AT, direction: DESC}}) {{ totalCount nodes {{ {ISSUE_FIELDS} }} }} \
         }} }}"
    )
}

/// One issue by its number, open or not.
pub fn issue_by_number() -> String {
    format!(
        "query($owner: String!, $name: String!, $number: Int!) {{ \
         repository(owner: $owner, name: $name) {{ \
         issue(number: $number) {{ state {ISSUE_FIELDS} }} \
         }} }}"
    )
}

/// The newest pull requests whose head is `branch`, in any state. A fork's
/// branch of the same name is a head too, so the head's owner comes along and
/// the caller keeps only the repository's own.
pub fn pr_for_branch() -> String {
    format!(
        "query($owner: String!, $name: String!, $branch: String!) {{ \
         repository(owner: $owner, name: $name) {{ \
         pullRequests(headRefName: $branch, first: 10, states: [OPEN, CLOSED, MERGED], orderBy: {{field: CREATED_AT, direction: DESC}}) {{ totalCount nodes {{ headRepositoryOwner {{ login }} {PR_FIELDS} }} }} \
         }} }}"
    )
}

/// What waits on the signed-in person across every repository on the host,
/// in one request: pull requests asking for their review, and open issues
/// assigned to them. GitHub resolves `@me` and team membership.
pub fn asks() -> String {
    format!(
        "query($reviews: String!, $assigned: String!, $first: Int!) {{ \
         reviews: search(type: ISSUE, query: $reviews, first: $first) {{ issueCount nodes {{ \
         ... on PullRequest {{ repository {{ nameWithOwner }} {PR_FIELDS} }} }} }} \
         assigned: search(type: ISSUE, query: $assigned, first: $first) {{ issueCount nodes {{ \
         ... on Issue {{ repository {{ nameWithOwner }} {ISSUE_FIELDS} }} }} }} \
         }}"
    )
}

pub const REVIEW_QUERY: &str = "is:pr is:open review-requested:@me";
pub const ASSIGNED_QUERY: &str = "is:issue is:open assignee:@me";

/// A list and how many GitHub says there are.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
}

/// One repository, as one poll read it.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// `owner/name` as GitHub spells it; `None` when it did not say.
    pub name_with_owner: Option<String>,
    pub pull_requests: Page<PullRequest>,
    pub issues: Page<Issue>,
}

/// What a host's person is asked for, each keyed by the repository's
/// `owner/name` as GitHub spells it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Asks {
    /// Open pull requests asking for this person's review.
    pub reviews: Vec<(String, PullRequest)>,
    /// Open issues assigned to this person.
    pub assigned: Vec<(String, Issue)>,
}

#[derive(Deserialize, Default)]
struct Conn<T> {
    #[serde(default, rename = "totalCount")]
    total: usize,
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<T>>,
}

#[derive(Deserialize, Default)]
struct Nodes<T> {
    #[serde(default = "Vec::new")]
    nodes: Vec<Option<T>>,
}

#[derive(Deserialize)]
struct GqlPr {
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    state: String,
    #[serde(default, rename = "isDraft")]
    is_draft: bool,
    #[serde(default, rename = "headRefName")]
    head_ref: String,
    #[serde(default, rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(default, rename = "mergeStateStatus")]
    merge_state: Option<String>,
    #[serde(default, rename = "updatedAt")]
    updated_at: Option<String>,
    #[serde(default)]
    author: Option<Author>,
    #[serde(default)]
    commits: Option<Nodes<CommitNode>>,
}

#[derive(Deserialize)]
struct CommitNode {
    commit: Commit,
}

#[derive(Deserialize)]
struct Commit {
    #[serde(default, rename = "statusCheckRollup")]
    rollup: Option<Rollup>,
}

#[derive(Deserialize)]
struct Rollup {
    #[serde(default)]
    contexts: Option<Nodes<Value>>,
}

#[derive(Deserialize)]
struct GqlIssue {
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "updatedAt")]
    updated_at: Option<String>,
    #[serde(default)]
    labels: Option<Nodes<Label>>,
    #[serde(default)]
    assignees: Option<Nodes<Author>>,
}

/// One check context, flattened as GitHub's own CLI flattened it: a check run's
/// conclusion once it has one, else its status; a commit status's state.
fn check(v: &Value) -> Option<Check> {
    let s = |k: &str| v[k].as_str().map(str::to_string);
    match v["__typename"].as_str()? {
        "CheckRun" => Some(Check {
            name: s("name").unwrap_or_default(),
            state: s("conclusion")
                .filter(|c| !c.is_empty())
                .or_else(|| s("status"))
                .unwrap_or_default(),
            workflow: v["checkSuite"]["workflowRun"]["workflow"]["name"]
                .as_str()
                .map(str::to_string),
            link: s("detailsUrl"),
        }),
        "StatusContext" => Some(Check {
            name: s("context").unwrap_or_default(),
            state: s("state").unwrap_or_default(),
            workflow: None,
            link: s("targetUrl"),
        }),
        _ => None,
    }
}

fn pr(g: GqlPr) -> PullRequest {
    let checks = g
        .commits
        .and_then(|c| c.nodes.into_iter().flatten().next())
        .and_then(|n| n.commit.rollup)
        .and_then(|r| r.contexts)
        .map(|c| c.nodes.iter().flatten().filter_map(check).collect())
        .unwrap_or_default();
    PullRequest {
        number: g.number,
        title: g.title,
        url: g.url,
        state: g.state,
        is_draft: g.is_draft,
        head_ref: g.head_ref,
        review_decision: g.review_decision.filter(|d| !d.is_empty()),
        merge_state: g.merge_state,
        checks,
        author: g.author,
        updated_at: g.updated_at,
    }
}

fn issue(g: GqlIssue) -> Issue {
    Issue {
        number: g.number,
        title: g.title,
        body: g.body,
        url: g.url,
        labels: g
            .labels
            .map(|l| l.nodes.into_iter().flatten().collect())
            .unwrap_or_default(),
        assignees: g
            .assignees
            .map(|a| a.nodes.into_iter().flatten().collect())
            .unwrap_or_default(),
        updated_at: g.updated_at,
    }
}

fn parse<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, super::Error> {
    serde_json::from_value(v.clone()).map_err(|e| super::Error::Api {
        status: 200,
        message: format!("GitHub answered in a shape Devplane does not read: {e}"),
    })
}

/// The repository object, or *not found* when GitHub says there is none.
fn repository(data: &Value) -> Result<&Value, super::Error> {
    let r = &data["repository"];
    if r.is_null() {
        return Err(super::Error::NotFound(
            "GitHub has no such repository, or this sign-in cannot see it".into(),
        ));
    }
    Ok(r)
}

pub fn read_snapshot(data: &Value) -> Result<Snapshot, super::Error> {
    let r = repository(data)?;
    let prs: Conn<GqlPr> = parse(&r["pullRequests"])?;
    let issues: Conn<GqlIssue> = parse(&r["issues"])?;
    Ok(Snapshot {
        name_with_owner: r["nameWithOwner"]
            .as_str()
            .filter(|n| !n.is_empty())
            .map(str::to_string),
        pull_requests: Page {
            items: prs.nodes.into_iter().flatten().map(pr).collect(),
            total: prs.total,
        },
        issues: Page {
            items: issues.nodes.into_iter().flatten().map(issue).collect(),
            total: issues.total,
        },
    })
}

pub fn read_issues(data: &Value) -> Result<Page<Issue>, super::Error> {
    let r = repository(data)?;
    let c: Conn<GqlIssue> = parse(&r["issues"])?;
    Ok(Page {
        items: c.nodes.into_iter().flatten().map(issue).collect(),
        total: c.total,
    })
}

/// One issue and whether it is open; `None` when the repository has none by
/// that number.
pub fn read_issue(data: &Value) -> Result<Option<(Issue, bool)>, super::Error> {
    let r = repository(data)?;
    if r["issue"].is_null() {
        return Ok(None);
    }
    let open = r["issue"]["state"].as_str() == Some("OPEN");
    let g: GqlIssue = parse(&r["issue"])?;
    Ok(Some((issue(g), open)))
}

/// The newest pull request whose head is `owner`'s own branch: a fork's
/// branch of the same name is somebody else's pull request.
pub fn read_pr_for_branch(data: &Value, owner: &str) -> Result<Option<PullRequest>, super::Error> {
    let r = repository(data)?;
    let c: Conn<Value> = parse(&r["pullRequests"])?;
    for v in c.nodes.into_iter().flatten() {
        let head_owner = v["headRepositoryOwner"]["login"].as_str().unwrap_or("");
        if !head_owner.eq_ignore_ascii_case(owner) {
            continue;
        }
        let g: GqlPr = parse(&v)?;
        return Ok(Some(pr(g)));
    }
    Ok(None)
}

/// Both halves of [`asks`]. A hit of the other kind is an empty object.
pub fn read_asks(data: &Value) -> Result<Asks, super::Error> {
    fn hits<T: serde::de::DeserializeOwned>(v: &Value) -> Result<Vec<(String, T)>, super::Error> {
        let c: Conn<Value> = parse(v)?;
        Ok(c.nodes
            .into_iter()
            .flatten()
            .filter_map(|n| {
                let repo = n["repository"]["nameWithOwner"].as_str()?.to_string();
                Some((repo, serde_json::from_value::<T>(n).ok()?))
            })
            .collect())
    }
    Ok(Asks {
        reviews: hits::<GqlPr>(&data["reviews"])?
            .into_iter()
            .map(|(r, g)| (r, pr(g)))
            .collect(),
        assigned: hits::<GqlIssue>(&data["assigned"])?
            .into_iter()
            .map(|(r, g)| (r, issue(g)))
            .collect(),
    })
}

/// A pull request as REST returns it (`POST /repos/{o}/{r}/pulls`).
pub fn rest_pr(v: &Value) -> Option<PullRequest> {
    Some(PullRequest {
        number: v["number"].as_u64()?,
        title: v["title"].as_str().unwrap_or_default().to_string(),
        url: v["html_url"].as_str()?.to_string(),
        state: match (v["merged_at"].is_string(), v["state"].as_str()) {
            (true, _) => "MERGED".into(),
            (_, Some(s)) => s.to_ascii_uppercase(),
            _ => "OPEN".into(),
        },
        is_draft: v["draft"].as_bool().unwrap_or(false),
        head_ref: v["head"]["ref"].as_str().unwrap_or_default().to_string(),
        review_decision: None,
        merge_state: None,
        checks: vec![],
        author: v["user"]["login"].as_str().map(|l| Author {
            login: l.to_string(),
        }),
        updated_at: v["updated_at"].as_str().map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checks_flatten_the_way_gh_flattened_them() {
        let data = json!({"repository": {
            "pullRequests": {"totalCount": 31, "nodes": [{
                "number": 142, "title": "Fix the flaky login test",
                "url": "https://github.com/acme/app/pull/142", "state": "OPEN",
                "isDraft": false, "headRefName": "fix/flaky-login-a1b2c3",
                "reviewDecision": null, "mergeStateStatus": "BLOCKED",
                "author": {"login": "hupe1980"}, "updatedAt": "2026-09-15T10:00:00Z",
                "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [
                    {"__typename": "CheckRun", "name": "build", "status": "COMPLETED",
                     "conclusion": "SUCCESS", "detailsUrl": "https://github.com/acme/app/actions/runs/900",
                     "checkSuite": {"workflowRun": {"workflow": {"name": "CI"}}}},
                    {"__typename": "CheckRun", "name": "test", "status": "COMPLETED",
                     "conclusion": "FAILURE", "detailsUrl": "https://github.com/acme/app/actions/runs/901"},
                    {"__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS", "conclusion": null},
                    {"__typename": "StatusContext", "context": "ci/legacy", "state": "PENDING",
                     "targetUrl": "https://ci.example/1"}
                ]}}}}]}
            }]},
            "issues": {"totalCount": 1, "nodes": [{
                "number": 7, "title": "Login fails", "body": "b", "url": "u",
                "labels": {"nodes": [{"name": "bug"}]}, "assignees": {"nodes": [{"login": "hupe1980"}]}
            }]}
        }});
        let s = read_snapshot(&data).unwrap();
        let pr = &s.pull_requests.items[0];
        assert_eq!(s.pull_requests.total, 31, "the rest is a count");
        assert_eq!(s.pull_requests.items.len(), 1);
        assert_eq!(pr.checks.len(), 4);
        assert_eq!(pr.checks[0].state, "SUCCESS");
        assert_eq!(pr.checks[0].workflow.as_deref(), Some("CI"));
        assert_eq!(pr.checks[2].state, "IN_PROGRESS");
        assert_eq!(pr.checks[3].name, "ci/legacy");
        assert_eq!(pr.review_decision, None);
        assert_eq!(pr.status(), super::super::PrStatus::Failing);
        assert_eq!(pr.failing_checks()[0].name, "test");
        let i = &s.issues.items[0];
        assert_eq!(i.labels[0].name, "bug");
        assert!(i.to_forge(Some("hupe1980")).assigned_to_me);
    }

    #[test]
    fn a_missing_repository_is_not_found_not_empty() {
        assert!(matches!(
            read_snapshot(&json!({"repository": null})),
            Err(super::super::Error::NotFound(_))
        ));
    }

    #[test]
    fn the_asks_search_reads_each_half_by_its_kind() {
        let asks = read_asks(&json!({
            "reviews": {"issueCount": 2, "nodes": [
                {"number": 5, "title": "t", "url": "https://github.com/Acme/App/pull/5",
                 "state": "OPEN", "repository": {"nameWithOwner": "Acme/App"}},
                {}
            ]},
            "assigned": {"issueCount": 1, "nodes": [
                {"number": 40, "title": "old", "url": "https://github.com/Acme/App/issues/40",
                 "repository": {"nameWithOwner": "Acme/App"},
                 "assignees": {"nodes": [{"login": "octocat"}]}}
            ]}
        }))
        .unwrap();
        assert_eq!(
            asks.reviews.len(),
            1,
            "a hit that is not a pull request is skipped"
        );
        assert_eq!(asks.reviews[0].0, "Acme/App");
        assert_eq!(asks.reviews[0].1.number, 5);
        assert_eq!(asks.assigned[0].1.number, 40);
    }

    #[test]
    fn a_forks_branch_of_the_same_name_is_not_the_changes_pull_request() {
        let node = |n: u64, owner: &str| {
            json!({"number": n, "title": "t", "url": "u", "state": "OPEN",
                   "headRepositoryOwner": {"login": owner}})
        };
        let data = json!({"repository": {"pullRequests": {"totalCount": 2,
            "nodes": [node(12, "stranger"), node(9, "Acme")]}}});
        assert_eq!(
            read_pr_for_branch(&data, "acme").unwrap().unwrap().number,
            9
        );
        assert_eq!(read_pr_for_branch(&data, "nobody").unwrap(), None);
    }

    #[test]
    fn an_issue_by_number_says_whether_it_is_open() {
        let data = json!({"repository": {"issue": {"number": 3, "title": "t", "url": "u",
            "state": "CLOSED"}}});
        let (i, open) = read_issue(&data).unwrap().unwrap();
        assert_eq!((i.number, open), (3, false));
        assert_eq!(
            read_issue(&json!({"repository": {"issue": null}})).unwrap(),
            None
        );
    }

    #[test]
    fn a_created_pull_request_reads_from_rest() {
        let p = rest_pr(
            &json!({"number": 9, "html_url": "https://github.com/acme/app/pull/9",
            "state": "open", "draft": true, "head": {"ref": "feat/x"}, "title": "t"}),
        )
        .unwrap();
        assert_eq!((p.number, p.state.as_str(), p.is_draft), (9, "OPEN", true));
        assert_eq!(p.status(), super::super::PrStatus::Draft);
    }
}

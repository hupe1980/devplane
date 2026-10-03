//! A GitHub double: an axum server answering the device flow, GraphQL and the
//! REST routes Devplane calls, with switches for the failures a surface must
//! say. Every request is recorded, so a test can count them. Never the network,
//! never the real credential store.
#![allow(dead_code)]

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// The token the double issues and accepts.
pub const TOKEN: &str = "gho_DOUBLEtoken_0123456789abcdef";
pub const USER_CODE: &str = "WDJB-MJHT";

/// The head commit every pull request the double serves sits on.
pub const HEAD_OID: &str = "3f2a9c1e0b7d3a5c9e1f2a4b6c8d0e2f4a6b8c0d";

/// What the review routes answer for one repository: the review list (paged
/// by the double), and the branch's rules and protection — each a body, or
/// the status it fails with.
#[derive(Debug, Clone)]
pub struct ApprovalsAnswer {
    /// The pull request's head commit, as the double serves it.
    pub head: String,
    pub reviews: Vec<Value>,
    pub rules: Result<Value, u16>,
    pub protection: Result<Value, u16>,
}

/// One review, as GitHub's list carries it.
pub fn review_json(
    id: u64,
    login: &str,
    user_id: u64,
    ty: &str,
    state: &str,
    commit: Option<&str>,
) -> Value {
    json!({
        "id": id,
        "node_id": format!("PRR_{id}"),
        "user": {"login": login, "id": user_id, "type": ty, "site_admin": false},
        "body": "", "state": state, "commit_id": commit,
        "author_association": if ty == "User" { "MEMBER" } else { "NONE" },
        "submitted_at": "2026-10-01T10:00:00Z",
        "html_url": "https://github.com/acme/app/pull/9#pullrequestreview-1",
    })
}

/// A synthetic twin of the pull request the recording must come from: a
/// person's approval dismissed by a later push, then a Copilot code review
/// approval on the head, under a rule requiring one approval. Written by
/// hand for tests that run before the owner's recording exists — never
/// presented as GitHub's answer.
pub fn synthetic() -> ApprovalsAnswer {
    let older = "9e1d0aa5b7c3d2e1f0a9b8c7d6e5f4a3b2c1d0e9";
    ApprovalsAnswer {
        head: HEAD_OID.into(),
        reviews: vec![
            review_json(1, "alice", 10, "User", "DISMISSED", Some(older)),
            review_json(
                2,
                "copilot-pull-request-reviewer[bot]",
                175728472,
                "Bot",
                "APPROVED",
                Some(HEAD_OID),
            ),
            review_json(
                3,
                "copilot-pull-request-reviewer[bot]",
                175728472,
                "Bot",
                "COMMENTED",
                Some(HEAD_OID),
            ),
        ],
        rules: Ok(json!([
            {"type": "pull_request", "ruleset_source_type": "Repository", "ruleset_source": "acme/app",
             "ruleset_id": 1, "parameters": {
                "required_approving_review_count": 1, "dismiss_stale_reviews_on_push": true,
                "require_code_owner_review": false, "require_last_push_approval": false,
                "required_review_thread_resolution": false}}
        ])),
        protection: Err(404),
    }
}

/// The owner's recording of GitHub's own answers, when it exists:
/// `tests/fixtures/github/merge-decider/` with its `SOURCE`. Never written
/// by hand; `scripts/capture-reviews.sh` makes it.
pub fn recorded() -> Option<ApprovalsAnswer> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/github/merge-decider");
    if !dir.join("SOURCE").is_file() {
        return None;
    }
    let read = |name: &str| -> Result<Value, u16> {
        let text = std::fs::read_to_string(dir.join(name)).map_err(|_| 404u16)?;
        let v: Value = serde_json::from_str(&text).map_err(|_| 500u16)?;
        // A refusal is recorded as `{"status": 403, ...}`.
        match v.get("status").and_then(Value::as_u64) {
            Some(code) if v.get("message").is_some() => Err(code as u16),
            _ => Ok(v),
        }
    };
    let head = recorded_source().get("head").cloned()?;
    Some(ApprovalsAnswer {
        head,
        reviews: read("reviews.json").ok()?.as_array()?.clone(),
        rules: read("rules.json"),
        protection: read("protection.json"),
    })
}

/// The recording's `SOURCE`, as key-value lines.
pub fn recorded_source() -> BTreeMap<String, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/github/merge-decider/SOURCE");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            l.split_once('=')
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

/// What every API route answers with, instead of its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ok,
    /// `401`: the token is no longer accepted.
    Unauthorized,
    /// `403` with the limit spent, resetting at this epoch second.
    RateLimited(i64),
}

#[derive(Default)]
pub struct Seen {
    /// `METHOD /path` or `graphql:<kind>`, in order.
    pub requests: Vec<String>,
    /// Every `Authorization` header received.
    pub authorization: Vec<String>,
    /// Bodies of created pull requests and issues.
    pub created: Vec<(String, Value)>,
}

pub struct Double {
    pub addr: SocketAddr,
    pub name: String,
    mode: Arc<Mutex<Mode>>,
    /// What each poll of the token route answers, front first; `token` when
    /// empty.
    polls: Arc<Mutex<VecDeque<&'static str>>>,
    pub seen: Arc<Mutex<Seen>>,
    approvals: Arc<Mutex<BTreeMap<String, ApprovalsAnswer>>>,
    pr: Arc<Mutex<(String, String)>>,
}

#[derive(Clone)]
struct S {
    name: String,
    mode: Arc<Mutex<Mode>>,
    polls: Arc<Mutex<VecDeque<&'static str>>>,
    seen: Arc<Mutex<Seen>>,
    base: String,
    /// Per `owner/repo`; [`synthetic`] for any repository not set.
    approvals: Arc<Mutex<BTreeMap<String, ApprovalsAnswer>>>,
    /// The branch's own pull request's state and `updatedAt`.
    pr: Arc<Mutex<(String, String)>>,
}

impl Double {
    /// Starts a double that stands for `name` (`github.com`, `ghe.test`).
    pub async fn start(name: &str) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mode = Arc::new(Mutex::new(Mode::Ok));
        let polls = Arc::new(Mutex::new(VecDeque::new()));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let approvals = Arc::new(Mutex::new(BTreeMap::new()));
        let pr = Arc::new(Mutex::new((
            "OPEN".to_string(),
            "2026-09-15T10:00:00Z".to_string(),
        )));
        let s = S {
            name: name.to_string(),
            mode: mode.clone(),
            polls: polls.clone(),
            seen: seen.clone(),
            base: format!("http://{addr}"),
            approvals: approvals.clone(),
            pr: pr.clone(),
        };
        let app = Router::new()
            .route("/login/device/code", post(device_code))
            .route("/login/oauth/access_token", post(access_token))
            .route("/user", get(user))
            .route("/graphql", post(graphql))
            .route("/repos/{o}/{r}/pulls", post(create_pull))
            .route("/repos/{o}/{r}/pulls/{n}/reviews", get(list_reviews))
            .route("/repos/{o}/{r}/rules/branches/{*b}", get(branch_rules))
            .route(
                "/repos/{o}/{r}/branches/{b}/protection",
                get(branch_protection),
            )
            .fallback(unrouted)
            .with_state(s);
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        Self {
            addr,
            name: name.to_string(),
            mode,
            polls,
            seen,
            approvals,
            pr,
        }
    }

    /// What the review routes answer for `owner/repo`.
    pub fn set_approvals(&self, repo: &str, a: ApprovalsAnswer) {
        self.approvals.lock().unwrap().insert(repo.to_string(), a);
    }

    /// The branch's own pull request's state (`OPEN`, `MERGED`) and
    /// `updatedAt`.
    pub fn set_pr(&self, state: &str, updated_at: &str) {
        *self.pr.lock().unwrap() = (state.to_string(), updated_at.to_string());
    }

    /// Requests to the review routes, as `METHOD path`.
    pub fn review_requests(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|r| {
                r.contains("/reviews") || r.contains("/rules/") || r.contains("/protection")
            })
            .collect()
    }

    pub fn base(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// This double's addresses, for `GitHub::route`.
    pub fn host(&self) -> devplane::github::GitHubHost {
        with_base(&self.name, &self.base())
    }

    pub fn set_mode(&self, m: Mode) {
        *self.mode.lock().unwrap() = m;
    }

    /// Scripts the token route: `pending`, `slow_down`, `denied`, `expired`,
    /// `token`.
    pub fn script(&self, answers: &[&'static str]) {
        self.polls.lock().unwrap().extend(answers.iter().copied());
    }

    pub fn requests(&self) -> Vec<String> {
        self.seen.lock().unwrap().requests.clone()
    }

    /// Requests to the API (not the sign-in routes).
    pub fn api_requests(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|r| !r.contains("/login/"))
            .collect()
    }

    pub fn created(&self) -> Vec<(String, Value)> {
        self.seen.lock().unwrap().created.clone()
    }

    pub fn authorization(&self) -> Vec<String> {
        self.seen.lock().unwrap().authorization.clone()
    }
}

fn note(s: &S, what: String, headers: &HeaderMap) {
    let mut seen = s.seen.lock().unwrap();
    seen.requests.push(what);
    if let Some(a) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        seen.authorization.push(a.to_string());
    }
}

/// The mode's refusal, or `None` to answer normally. A wrong token is a 401
/// whatever the mode.
fn refused(s: &S, headers: &HeaderMap) -> Option<Response> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let mode = *s.mode.lock().unwrap();
    if auth != format!("Bearer {TOKEN}") || mode == Mode::Unauthorized {
        return Some(
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"message": "Bad credentials"})),
            )
                .into_response(),
        );
    }
    if let Mode::RateLimited(reset) = mode {
        return Some(
            (
                StatusCode::FORBIDDEN,
                [
                    ("x-ratelimit-remaining", "0".to_string()),
                    ("x-ratelimit-reset", reset.to_string()),
                ],
                Json(json!({"message": "API rate limit exceeded"})),
            )
                .into_response(),
        );
    }
    None
}

async fn device_code(State(s): State<S>, headers: HeaderMap, body: String) -> impl IntoResponse {
    note(&s, "POST /login/device/code".into(), &headers);
    if !body.contains("client_id=") || !body.contains("scope=repo") {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "bad"}))).into_response();
    }
    Json(json!({
        "device_code": "device-code-secret",
        "user_code": USER_CODE,
        "verification_uri": format!("{}/login/device", s.base),
        "expires_in": 900,
        "interval": 1,
    }))
    .into_response()
}

async fn access_token(State(s): State<S>, headers: HeaderMap, body: String) -> impl IntoResponse {
    note(&s, "POST /login/oauth/access_token".into(), &headers);
    assert!(
        body.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"),
        "{body}"
    );
    let next = s.polls.lock().unwrap().pop_front().unwrap_or("token");
    Json(match next {
        "pending" => json!({"error": "authorization_pending"}),
        "slow_down" => json!({"error": "slow_down", "interval": 2}),
        "denied" => json!({"error": "access_denied"}),
        "expired" => json!({"error": "expired_token"}),
        _ => json!({"access_token": TOKEN, "token_type": "bearer", "scope": "repo,read:org"}),
    })
}

async fn user(State(s): State<S>, headers: HeaderMap) -> Response {
    note(&s, "GET /user".into(), &headers);
    if let Some(r) = refused(&s, &headers) {
        return r;
    }
    (
        [("x-oauth-scopes", "repo, read:org")],
        Json(json!({"login": "octocat"})),
    )
        .into_response()
}

/// A pull request node, as the snapshot query asks for it.
pub fn pr_node(n: u64, owner: &str, name: &str, host: &str) -> Value {
    json!({
        "number": n, "title": format!("pull request {n}"),
        "url": format!("https://{host}/{owner}/{name}/pull/{n}"), "state": "OPEN",
        "isDraft": false, "headRefName": format!("feat/{n}"), "reviewDecision": null,
        "headRefOid": HEAD_OID, "baseRefName": "main",
        "mergeStateStatus": "BLOCKED", "author": {"login": "octocat"},
        "updatedAt": "2026-09-15T10:00:00Z",
        "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [
            {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS",
             "detailsUrl": "https://example/1"},
            {"__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "FAILURE",
             "detailsUrl": "https://example/2"}
        ]}}}}]}
    })
}

pub fn issue_node(n: u64, owner: &str, name: &str, host: &str, label: &str) -> Value {
    json!({
        "number": n, "title": format!("issue {n}"), "body": "steps",
        "url": format!("https://{host}/{owner}/{name}/issues/{n}"),
        "updatedAt": "2026-09-15T10:00:00Z",
        "labels": {"nodes": [{"name": label}]}, "assignees": {"nodes": [{"login": "octocat"}]}
    })
}

async fn graphql(State(s): State<S>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let q = body["query"].as_str().unwrap_or_default().to_string();
    let v = &body["variables"];
    let kind = if q.contains("search(") {
        "search"
    } else if q.contains("issue(number:") {
        "issue"
    } else if q.contains("headRefName: $branch") {
        "pr_for_branch"
    } else if q.contains("labels: $labels") {
        "issues"
    } else {
        "snapshot"
    };
    note(&s, format!("graphql:{kind}"), &headers);
    if let Some(r) = refused(&s, &headers) {
        return r;
    }
    let owner = v["owner"].as_str().unwrap_or_default();
    let name = v["name"].as_str().unwrap_or_default();
    let host = s.name.as_str();
    if name == "missing" {
        return Json(json!({"data": {"repository": null},
            "errors": [{"type": "NOT_FOUND", "message": "Could not resolve to a Repository"}]}))
        .into_response();
    }
    // GitHub's own spelling of the repository, whatever case the remote used.
    let canonical = format!("{}/{}", owner.to_lowercase(), name.to_lowercase());
    let data = match kind {
        // What `acme/big` asks of the person: a review on a row of its page
        // (#2), a review past the page (#50), and an issue assigned long ago
        // (#40), none of which a 20-row page shows.
        "search" => {
            let (o, n) = ("acme", "big");
            let hit = |mut v: Value| {
                v["repository"] = json!({"nameWithOwner": "acme/big"});
                v
            };
            json!({
                "reviews": {"issueCount": 2, "nodes": [
                    hit(pr_node(2, o, n, host)), hit(pr_node(50, o, n, host)), {}
                ]},
                "assigned": {"issueCount": 1, "nodes": [
                    hit(issue_node(40, o, n, host, "old"))
                ]}
            })
        }
        "issue" => match v["number"].as_u64().unwrap_or(0) {
            404 => json!({"repository": {"issue": null}}),
            n => {
                let mut i = issue_node(n, owner, name, host, "bug");
                i["state"] = json!(if n == 3 { "CLOSED" } else { "OPEN" });
                json!({"repository": {"issue": i}})
            }
        },
        "pr_for_branch" => {
            // A fork's pull request from a branch of the same name comes
            // first; the repository's own is behind it.
            let mut fork = pr_node(12, owner, name, host);
            fork["headRepositoryOwner"] = json!({"login": "stranger"});
            let mut own = pr_node(9, owner, name, host);
            own["headRepositoryOwner"] = json!({"login": owner});
            let (state, updated) = s.pr.lock().unwrap().clone();
            own["state"] = json!(state);
            own["updatedAt"] = json!(updated);
            own["reviewDecision"] = json!("APPROVED");
            own["headRefOid"] = json!(answer_for(&s, owner, name).head);
            json!({"repository": {"pullRequests": {"totalCount": 2, "nodes": [fork, own]}}})
        }
        "issues" => {
            let label = v["labels"][0].as_str().unwrap_or("bug");
            json!({"repository": {"issues": {"totalCount": 1,
                "nodes": [issue_node(7, owner, name, host, label)]}}})
        }
        _ => json!({"repository": {
            "nameWithOwner": canonical,
            "pullRequests": {"totalCount": 25, "nodes": [
                pr_node(1, owner, name, host), pr_node(2, owner, name, host)]},
            "issues": {"totalCount": 1, "nodes": [issue_node(7, owner, name, host, "bug")]}
        }}),
    };
    Json(json!({"data": data})).into_response()
}

#[derive(serde::Deserialize)]
struct PageQuery {
    page: Option<usize>,
    per_page: Option<usize>,
}

fn answer_for(s: &S, o: &str, r: &str) -> ApprovalsAnswer {
    s.approvals
        .lock()
        .unwrap()
        .get(&format!("{o}/{r}"))
        .cloned()
        .unwrap_or_else(synthetic)
}

fn status_body(code: u16) -> Response {
    let message = match code {
        403 => "Resource not accessible by integration",
        404 => "Branch not protected",
        _ => "error",
    };
    (
        StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(json!({"message": message})),
    )
        .into_response()
}

async fn list_reviews(
    State(s): State<S>,
    headers: HeaderMap,
    Path((o, r, n)): Path<(String, String, u64)>,
    Query(q): Query<PageQuery>,
) -> Response {
    let page = q.page.unwrap_or(1).max(1);
    let per = q.per_page.unwrap_or(30).clamp(1, 100);
    note(
        &s,
        format!("GET /repos/{o}/{r}/pulls/{n}/reviews?page={page}"),
        &headers,
    );
    if let Some(x) = refused(&s, &headers) {
        return x;
    }
    let all = answer_for(&s, &o, &r).reviews;
    let items: Vec<Value> = all
        .iter()
        .skip((page - 1) * per)
        .take(per)
        .cloned()
        .collect();
    let last = all.len().div_ceil(per).max(1);
    let mut resp = Json(Value::Array(items)).into_response();
    if last > 1 {
        let link = format!(
            "<{}/repos/{o}/{r}/pulls/{n}/reviews?per_page={per}&page={last}>; rel=\"last\"",
            s.base
        );
        resp.headers_mut().insert("link", link.parse().unwrap());
    }
    resp
}

async fn branch_rules(
    State(s): State<S>,
    headers: HeaderMap,
    Path((o, r, b)): Path<(String, String, String)>,
) -> Response {
    note(
        &s,
        format!("GET /repos/{o}/{r}/rules/branches/{b}"),
        &headers,
    );
    if let Some(x) = refused(&s, &headers) {
        return x;
    }
    match answer_for(&s, &o, &r).rules {
        Ok(v) => Json(v).into_response(),
        Err(code) => status_body(code),
    }
}

async fn branch_protection(
    State(s): State<S>,
    headers: HeaderMap,
    Path((o, r, b)): Path<(String, String, String)>,
) -> Response {
    note(
        &s,
        format!("GET /repos/{o}/{r}/branches/{b}/protection"),
        &headers,
    );
    if let Some(x) = refused(&s, &headers) {
        return x;
    }
    match answer_for(&s, &o, &r).protection {
        Ok(v) => Json(v).into_response(),
        Err(code) => status_body(code),
    }
}

/// Anything the double does not serve is recorded with its method, so a
/// test sees a write it did not expect.
async fn unrouted(
    State(s): State<S>,
    method: axum::http::Method,
    uri: axum::http::Uri,
    headers: HeaderMap,
) -> Response {
    note(&s, format!("{method} {}", uri.path()), &headers);
    (StatusCode::NOT_FOUND, Json(json!({"message": "Not Found"}))).into_response()
}

async fn create_pull(
    State(s): State<S>,
    headers: HeaderMap,
    Path((o, r)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    note(&s, format!("POST /repos/{o}/{r}/pulls"), &headers);
    if let Some(x) = refused(&s, &headers) {
        return x;
    }
    // A branch with a pull request already open, as GitHub refuses it.
    if body["head"] == "feat/9" {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "Validation Failed", "errors": [
                {"resource": "PullRequest", "code": "custom",
                 "message": format!("A pull request already exists for {o}:feat/9.")}
            ]})),
        )
            .into_response();
    }
    s.seen
        .lock()
        .unwrap()
        .created
        .push(("pull".into(), body.clone()));
    (
        StatusCode::CREATED,
        Json(json!({
            "number": 77, "title": body["title"], "state": "open", "draft": body["draft"],
            "html_url": format!("https://{}/{o}/{r}/pull/77", s.name),
            "head": {"ref": body["head"]}, "user": {"login": "octocat"}
        })),
    )
        .into_response()
}

/// A `gh` on `PATH` that records any invocation: Devplane must never run it.
/// Returns the log it would write.
pub fn trap_gh() -> std::path::PathBuf {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("dp-gh-trap-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let log = d.join("ran");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let gh = d.join("gh");
            std::fs::write(
                &gh,
                format!("#!/bin/sh\necho \"$@\" >> {}\nexit 1\n", log.display()),
            )
            .unwrap();
            std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(windows)]
        std::fs::write(
            d.join("gh.cmd"),
            format!("@echo %* >> {}\r\n", log.display()),
        )
        .unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut parts = vec![d.clone()];
        parts.extend(std::env::split_paths(&path));
        // SAFETY: set once, before any request, by every test in the binary
        // that uses the double; nothing reads PATH concurrently in a way that
        // a prepended directory would break.
        unsafe { std::env::set_var("PATH", std::env::join_paths(parts).unwrap()) };
        d
    });
    dir.join("ran")
}

/// Asserts the `gh` trap never ran.
pub fn assert_no_gh(log: &std::path::Path) {
    assert!(
        !log.exists(),
        "a `gh` process was started: {}",
        std::fs::read_to_string(log).unwrap_or_default()
    );
}

/// A host whose every address is the double's base — test-only, so the
/// production host keeps only GitHub's documented addresses.
pub fn with_base(name: &str, base: &str) -> devplane::github::GitHubHost {
    let base = base.trim_end_matches('/');
    devplane::github::GitHubHost {
        name: name.to_ascii_lowercase(),
        api: base.to_string(),
        graphql: format!("{base}/graphql"),
        oauth: base.to_string(),
    }
}

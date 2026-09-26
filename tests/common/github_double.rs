//! A GitHub double: an axum server answering the device flow, GraphQL and the
//! REST routes Devplane calls, with switches for the failures a surface must
//! say. Every request is recorded, so a test can count them. Never the network,
//! never the real credential store.
#![allow(dead_code)]

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// The token the double issues and accepts.
pub const TOKEN: &str = "gho_DOUBLEtoken_0123456789abcdef";
pub const USER_CODE: &str = "WDJB-MJHT";

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
}

#[derive(Clone)]
struct S {
    name: String,
    mode: Arc<Mutex<Mode>>,
    polls: Arc<Mutex<VecDeque<&'static str>>>,
    seen: Arc<Mutex<Seen>>,
    base: String,
}

impl Double {
    /// Starts a double that stands for `name` (`github.com`, `ghe.test`).
    pub async fn start(name: &str) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mode = Arc::new(Mutex::new(Mode::Ok));
        let polls = Arc::new(Mutex::new(VecDeque::new()));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let s = S {
            name: name.to_string(),
            mode: mode.clone(),
            polls: polls.clone(),
            seen: seen.clone(),
            base: format!("http://{addr}"),
        };
        let app = Router::new()
            .route("/login/device/code", post(device_code))
            .route("/login/oauth/access_token", post(access_token))
            .route("/user", get(user))
            .route("/graphql", post(graphql))
            .route("/repos/{o}/{r}/pulls", post(create_pull))
            .route("/repos/{o}/{r}/issues", post(create_issue))
            .with_state(s);
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        Self {
            addr,
            name: name.to_string(),
            mode,
            polls,
            seen,
        }
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

async fn create_issue(
    State(s): State<S>,
    headers: HeaderMap,
    Path((o, r)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    note(&s, format!("POST /repos/{o}/{r}/issues"), &headers);
    if let Some(x) = refused(&s, &headers) {
        return x;
    }
    s.seen
        .lock()
        .unwrap()
        .created
        .push(("issue".into(), body.clone()));
    (
        StatusCode::CREATED,
        Json(json!({"number": 7, "html_url": format!("https://{}/{o}/{r}/issues/7", s.name)})),
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

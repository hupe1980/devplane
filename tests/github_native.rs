//! Devplane's own GitHub adapter, against a GitHub double: the device flow,
//! every operation, the failure states each surface must say, sign-out, and an
//! Enterprise host. The token store is in memory; nothing reaches the network
//! or the machine's credential store, and a `gh` on `PATH` records any run.

#[path = "common/github_double.rs"]
mod double;

use devplane::core::Policy;
use devplane::github::{self, Error, GitHub, Memory, Secret};
use devplane::host::{AppState, Shared};
use double::{Double, Mode, TOKEN, USER_CODE};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct World {
    state: Shared,
    addr: SocketAddr,
    http: reqwest::Client,
    home: PathBuf,
    store: Arc<Memory>,
    gh_log: PathBuf,
    /// Every body an API route answered with, to search for the token.
    said: std::sync::Mutex<Vec<String>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dp-ghn-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A host whose GitHub hosts are the doubles given, `app.toml` as written.
async fn boot(doubles: &[&Double], app_toml: &str) -> World {
    let gh_log = double::trap_gh();
    let home = scratch("home");
    // Every double has an OAuth App unless the test's own `app.toml` says
    // otherwise — configured as a person would, in `[github.hosts]`.
    let mut app_toml = app_toml.to_string();
    for d in doubles {
        let named = app_toml.contains(&format!("host = \"{}\"", d.name))
            || app_toml.contains(&format!("\"{}\"]", d.name));
        if !named {
            app_toml.push_str(&format!(
                "\n[github.hosts.\"{}\"]\nclient_id = \"Iv1.test\"\n",
                d.name
            ));
        }
    }
    std::fs::write(home.join("app.toml"), app_toml).unwrap();
    let state = AppState::new(
        home.join("devplane.db"),
        "tok".into(),
        Policy::default(),
        home.clone(),
    )
    .await
    .unwrap();
    let store = Arc::new(Memory::default());
    state.github.use_store(store.clone());
    for d in doubles {
        state.github.route(&d.name, d.host());
    }
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    World {
        state,
        addr,
        http: reqwest::Client::new(),
        home,
        store,
        gh_log,
        said: Default::default(),
    }
}

impl World {
    async fn get(&self, path: &str) -> Value {
        let text = self
            .http
            .get(format!("http://{}{path}", self.addr))
            .bearer_auth("tok")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        self.said.lock().unwrap().push(text.clone());
        serde_json::from_str(&text).unwrap()
    }

    async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let r = self
            .http
            .post(format!("http://{}{path}", self.addr))
            .bearer_auth("tok")
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = r.status().as_u16();
        let text = r.text().await.unwrap();
        self.said.lock().unwrap().push(text.clone());
        (status, serde_json::from_str(&text).unwrap())
    }

    fn hub(&self) -> &GitHub {
        &self.state.github
    }

    /// Signs in as the double's user without the device flow.
    async fn sign_in(&self, host: &str) {
        self.hub().finish(host, Secret::new(TOKEN)).await.unwrap();
    }

    /// The Forge answer's state for the configured host.
    async fn forge_state(&self) -> Value {
        self.get("/api/forge").await
    }

    /// Registers a git repository whose `origin` is `remote` (or none).
    async fn project(&self, name: &str, remote: Option<&str>) -> (PathBuf, String) {
        let dir = scratch(name).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        if let Some(url) = remote {
            git(&dir, &["remote", "add", "origin", url]);
        }
        let root = dir.canonicalize().unwrap();
        let mut p = devplane::core::Project::from_root(root.clone());
        p.trusted = true;
        let id = self.state.world.lock().await.upsert_project(p);
        (root, id.to_string())
    }
}

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

async fn until<T, F, Fut>(what: &str, within: Duration, mut f: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let start = Instant::now();
    loop {
        if let Some(v) = f().await {
            return v;
        }
        assert!(start.elapsed() < within, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn host_view<'a>(all: &'a Value, host: &str) -> &'a Value {
    all["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["host"] == host)
        .unwrap_or_else(|| panic!("no {host} in {all}"))
}

// ── Sign in without another tool ────────────────────────────────────────────

/// The window starts the flow, shows the code, a second start shows the same
/// code, `slow_down` is honoured, and the token lands in the store only.
#[tokio::test]
async fn the_device_flow_signs_in_from_the_window() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    gh.script(&["pending", "slow_down", "token"]);

    let started = Instant::now();
    let (code, first) = w
        .post("/api/github/login", json!({"host": "github.com"}))
        .await;
    assert_eq!(code, 200, "{first}");
    assert_eq!(first["state"], "pending", "{first}");
    assert_eq!(first["user_code"], USER_CODE);
    assert!(
        first["verification_uri"]
            .as_str()
            .unwrap()
            .ends_with("/login/device")
    );
    // Back from the browser: the same code, not a new one.
    let (_, again) = w.post("/api/github/login", json!({})).await;
    assert_eq!(again["user_code"], USER_CODE);
    assert_eq!(
        gh.requests()
            .iter()
            .filter(|r| r.contains("device/code"))
            .count(),
        1,
        "one code per host"
    );

    let done = until("the sign-in to finish", Duration::from_secs(20), || async {
        let all = w.get("/api/github").await;
        let h = host_view(&all, "github.com").clone();
        (h["state"] == "signed_in").then_some(h)
    })
    .await;
    assert_eq!(done["login"], "octocat");
    assert_eq!(done["scopes"], json!(["repo", "read:org"]));
    let polls = gh
        .requests()
        .iter()
        .filter(|r| r.contains("access_token"))
        .count();
    assert_eq!(polls, 3, "pending, slow_down, token");
    assert!(
        started.elapsed() >= Duration::from_millis(3500),
        "slow_down raised the interval: {:?}",
        started.elapsed()
    );
    assert_eq!(w.store.hosts(), ["github.com"]);
    assert!(w.store.contains(TOKEN));
    for body in w.said.lock().unwrap().iter() {
        assert!(
            !body.contains(TOKEN),
            "an API answer carried the token: {body}"
        );
    }
    double::assert_no_gh(&w.gh_log);
}

/// Denied and expired each end with their own sentence, and store nothing.
#[tokio::test]
async fn a_denied_or_expired_code_says_which_and_stores_nothing() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    for (answer, want, says) in [
        ("denied", Error::Denied, "denied"),
        ("expired", Error::CodeExpired, "expired"),
    ] {
        gh.script(&[answer]);
        let p = w.hub().start("github.com").await.unwrap();
        assert_eq!(p.user_code, USER_CODE);
        assert_eq!(w.hub().wait(p).await.unwrap_err(), want);
        assert!(w.store.hosts().is_empty(), "{answer}: nothing is stored");
        let all = w.get("/api/github").await;
        let h = host_view(&all, "github.com");
        assert_eq!(h["state"], "signed_out", "{h}");
        assert!(h["said"].as_str().unwrap().contains(says), "{h}");
    }
}

/// With no client id, the login is refused with the documented sentence and
/// `--with-token` still works.
#[tokio::test]
async fn without_a_client_id_only_a_token_signs_in() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "[github.hosts.\"github.com\"]\nclient_id = \"\"\n").await;
    let (code, v) = w.post("/api/github/login", json!({})).await;
    assert_eq!(code, 409, "{v}");
    assert_eq!(v["error"], devplane::github::auth::NO_CLIENT_ID);
    // The refusal names a way in that works without a registered app.
    for says in [
        "gh auth token | devplane login github --with-token",
        "personal access token",
    ] {
        assert!(v["error"].as_str().unwrap().contains(says), "{v}");
    }
    assert!(gh.requests().is_empty(), "GitHub was not asked");

    let viewer = w
        .hub()
        .finish("github.com", Secret::new(TOKEN))
        .await
        .unwrap();
    assert_eq!(viewer.login, "octocat");
    // A token GitHub rejects is not stored.
    let bad = w
        .hub()
        .finish("github.com", Secret::new("gho_wrong"))
        .await
        .unwrap_err();
    assert_eq!(bad, Error::Expired("github.com".into()));
}

// ── Everything GitHub, with `gh` absent ─────────────────────────────────────

/// Each operation returns the types the board reads, through the double, and
/// no `gh` process starts.
#[tokio::test]
async fn every_operation_goes_through_the_adapter_and_no_gh_runs() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    let (root, _) = w.project("app", Some("git@github.com:acme/app.git")).await;
    let hub = w.hub();

    assert!(hub.signed_in("github.com").is_none(), "not signed in yet");
    assert!(matches!(
        github::issues(hub, &root, None, 20).await,
        Err(Error::NotSignedIn(_))
    ));
    assert!(gh.api_requests().is_empty(), "nothing asked before sign-in");
    w.sign_in("github.com").await;

    assert_eq!(
        hub.signed_in("github.com").map(|v| v.login).as_deref(),
        Some("octocat")
    );
    let repo = github::remote::of_dir(&root).await.unwrap();
    let prs = github::snapshot(hub, &repo)
        .await
        .unwrap()
        .pull_requests
        .items;
    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].status(), github::PrStatus::Failing);
    assert_eq!(prs[0].failing_checks()[0].name, "test");
    let issues = github::issues(hub, &root, None, 20).await.unwrap();
    assert_eq!(issues[0].number, 7);
    assert_eq!(issues[0].labels[0].name, "bug");
    let asks = github::asks(hub, "github.com").await.unwrap();
    assert!(
        asks.reviews
            .iter()
            .any(|(r, p)| r == "acme/big" && p.number == 2)
    );
    assert_eq!(asks.assigned[0].1.number, 40);
    let pr = github::pr_for_branch(hub, &root, "feat/9").await.unwrap();
    assert_eq!(
        pr.unwrap().number,
        9,
        "the repository's own branch, not a fork's of the same name"
    );
    let ready = github::issues(hub, &root, Some("devplane:ready"), 30)
        .await
        .unwrap();
    assert_eq!(ready[0].labels[0].name, "devplane:ready");
    let opened = github::create_pr(hub, &root, "feat/x", "main", "t", "the certificate", true)
        .await
        .unwrap();
    assert!(!opened.existed);
    assert_eq!(opened.pull_request.number, 77);
    assert!(opened.pull_request.is_draft);
    let url = github::create_issue(hub, "github.com", "acme/core-lib", "a report", "> quoted")
        .await
        .unwrap();
    assert_eq!(url, "https://github.com/acme/core-lib/issues/7");

    let created = gh.created();
    assert_eq!(created[0].0, "pull");
    assert_eq!(created[0].1["draft"], true, "offered as a draft");
    assert_eq!(created[0].1["head"], "feat/x");
    assert_eq!(created[1].0, "issue");
    assert_eq!(created[1].1["body"], "> quoted");
    assert!(
        gh.authorization()
            .iter()
            .all(|a| a == &format!("Bearer {TOKEN}")),
        "every request carries the token, to its own host only"
    );
    double::assert_no_gh(&w.gh_log);
}

/// Five repositories on one host: one snapshot each and one review search.
#[tokio::test]
async fn a_poll_across_five_repositories_is_six_requests() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    for i in 0..5 {
        w.project(
            &format!("r{i}"),
            Some(&format!("https://github.com/acme/r{i}.git")),
        )
        .await;
    }
    w.sign_in("github.com").await;
    let before = gh.api_requests().len();
    devplane::poller::forge_once(&w.state).await;
    let asked: Vec<String> = gh.api_requests().split_off(before);
    assert_eq!(asked.len(), 6, "{asked:?}");
    assert_eq!(asked.iter().filter(|r| *r == "graphql:search").count(), 1);
    assert_eq!(asked.iter().filter(|r| *r == "graphql:snapshot").count(), 5);

    let f = w.forge_state().await;
    assert_eq!(f["github"]["state"], "signed_in", "{f}");
    assert_eq!(f["pull_requests"].as_array().unwrap().len(), 10);
    let p = &f["projects"][0];
    assert_eq!(p["github"]["state"], "signed_in", "{p}");
    assert_eq!(p["pull_requests_more"], 23, "one page read, the rest said");
    assert!(p["stale"].is_null());
    double::assert_no_gh(&w.gh_log);
}

// ── Failure states, said, never empty ───────────────────────────────────────

#[tokio::test]
async fn nobody_signed_in_asks_nothing_and_says_so() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("app", Some("https://github.com/acme/app.git"))
        .await;
    devplane::poller::forge_once(&w.state).await;
    assert!(gh.requests().is_empty(), "{:?}", gh.requests());
    let f = w.forge_state().await;
    assert_eq!(f["github"]["state"], "signed_out");
    assert_eq!(f["projects"][0]["github"]["state"], "signed_out", "{f}");
    assert!(f["issues"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn a_rejected_token_is_deleted_and_the_state_is_expired() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("app", Some("https://github.com/acme/app.git"))
        .await;
    w.sign_in("github.com").await;
    devplane::poller::forge_once(&w.state).await;
    assert_eq!(w.forge_state().await["issues"].as_array().unwrap().len(), 1);

    gh.set_mode(Mode::Unauthorized);
    devplane::poller::forge_once(&w.state).await;
    assert!(w.store.hosts().is_empty(), "a 401 removes the token");
    let f = w.forge_state().await;
    assert_eq!(f["github"]["state"], "expired", "{f}");
    assert_eq!(f["projects"][0]["github"]["state"], "expired");
    assert!(
        f["issues"].as_array().unwrap().is_empty(),
        "rows of a sign-in that ended are not shown as current"
    );
}

#[tokio::test]
async fn a_spent_rate_limit_says_when_and_waits_for_it() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("app", Some("https://github.com/acme/app.git"))
        .await;
    w.sign_in("github.com").await;
    devplane::poller::forge_once(&w.state).await;

    let reset = jiff::Timestamp::now().as_second() + 3600;
    gh.set_mode(Mode::RateLimited(reset));
    devplane::poller::forge_once(&w.state).await;
    let f = w.forge_state().await;
    assert_eq!(f["github"]["state"], "rate_limited", "{f}");
    let until: jiff::Timestamp = f["github"]["until"].as_str().unwrap().parse().unwrap();
    assert_eq!(until.as_second(), reset);
    // The last data stays, marked stale.
    assert_eq!(f["issues"].as_array().unwrap().len(), 1);
    assert!(f["projects"][0]["stale"].is_string(), "{f}");

    let before = gh.api_requests().len();
    devplane::poller::forge_once(&w.state).await;
    assert_eq!(
        gh.api_requests().len(),
        before,
        "nothing is asked before the reset"
    );
}

#[tokio::test]
async fn an_unreachable_github_keeps_the_last_snapshot_marked_stale() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("app", Some("https://github.com/acme/app.git"))
        .await;
    w.sign_in("github.com").await;
    devplane::poller::forge_once(&w.state).await;

    // A port nothing listens on.
    let dead = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    w.hub().route(
        "github.com",
        double::with_base("github.com", &format!("http://{dead}")),
    );
    devplane::poller::forge_once(&w.state).await;
    let f = w.forge_state().await;
    assert_eq!(f["github"]["state"], "unreachable", "{f}");
    assert!(f["github"]["why"].is_string());
    assert_eq!(
        f["issues"].as_array().unwrap().len(),
        1,
        "the last data stays"
    );
    assert!(f["projects"][0]["stale"].is_string());
    assert_eq!(
        w.store.hosts(),
        ["github.com"],
        "a network failure keeps the sign-in"
    );
}

#[tokio::test]
async fn a_repository_elsewhere_is_not_a_github_repository() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("lab", Some("git@gitlab.example.com:acme/app.git"))
        .await;
    w.project("plain", None).await;
    w.sign_in("github.com").await;
    devplane::poller::forge_once(&w.state).await;
    let f = w.forge_state().await;
    let states: Vec<&str> = f["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["github"]["state"].as_str().unwrap())
        .collect();
    assert_eq!(states, ["not_github", "not_github"], "{f}");
    assert!(
        gh.api_requests().iter().all(|r| r == "GET /user"),
        "{:?}",
        gh.api_requests()
    );
}

// ── Sign out removes everything ─────────────────────────────────────────────

#[tokio::test]
async fn sign_out_leaves_the_token_nowhere() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("app", Some("https://github.com/acme/app.git"))
        .await;
    gh.script(&["token"]);
    let p = w.hub().start("github.com").await.unwrap();
    w.hub().wait(p).await.unwrap();
    devplane::poller::forge_once(&w.state).await;
    assert!(
        !w.forge_state().await["issues"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let _ = w.get("/api/github").await;
    let _ = w.get("/api/diagnostics").await;

    let (code, out) = w
        .post("/api/github/logout", json!({"host": "github.com"}))
        .await;
    assert_eq!(code, 200, "{out}");
    assert_eq!(out["state"], "signed_out");
    assert_eq!(out["revoke_at"], "https://github.com/settings/applications");
    assert!(
        out["deleted_from"]
            .as_str()
            .unwrap()
            .contains("github:github.com")
    );
    assert!(w.store.hosts().is_empty(), "the store is empty");
    let f = w.forge_state().await;
    assert_eq!(f["github"]["state"], "signed_out");
    assert!(f["issues"].as_array().unwrap().is_empty());
    assert!(f["pull_requests"].as_array().unwrap().is_empty());

    // Nowhere: no answer, no file under the home, no recorded event.
    for body in w.said.lock().unwrap().iter() {
        assert!(
            !body.contains(TOKEN),
            "an API answer carried the token: {body}"
        );
    }
    for entry in walk(&w.home) {
        let bytes = std::fs::read(&entry).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(TOKEN),
            "{} holds the token",
            entry.display()
        );
    }
    assert!(!format!("{:?}", w.hub()).contains(TOKEN));
    assert!(
        !Error::Expired("github.com".into())
            .to_string()
            .contains(TOKEN)
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// ── GitHub Enterprise ───────────────────────────────────────────────────────

/// The configured Enterprise host takes the device flow and its own
/// repositories; github.com keeps its own sign-in and its own requests.
#[tokio::test]
async fn an_enterprise_host_is_signed_in_and_read_on_its_own() {
    let com = Double::start("github.com").await;
    let ghe = Double::start("ghe.test").await;
    let w = boot(
        &[&com, &ghe],
        "[github]\nhost = \"ghe.test\"\nclient_id = \"Iv1.ghe\"\n",
    )
    .await;
    assert_eq!(w.hub().default_host(), "ghe.test");
    assert_eq!(w.hub().client_id("ghe.test").as_deref(), Some("Iv1.ghe"));
    assert_ne!(
        w.hub().client_id("github.com").as_deref(),
        Some("Iv1.ghe"),
        "the Enterprise app is not offered to github.com"
    );

    ghe.script(&["token"]);
    let (code, v) = w.post("/api/github/login", json!({})).await;
    assert_eq!((code, v["state"].as_str()), (200, Some("pending")), "{v}");
    until(
        "the Enterprise sign-in",
        Duration::from_secs(10),
        || async {
            let all = w.get("/api/github").await;
            (host_view(&all, "ghe.test")["state"] == "signed_in").then_some(())
        },
    )
    .await;
    assert!(
        com.requests().is_empty(),
        "github.com was not asked: {:?}",
        com.requests()
    );
    assert_eq!(w.store.hosts(), ["ghe.test"]);

    w.project("tool", Some("https://ghe.test/team/tool.git"))
        .await;
    w.project("app", Some("https://github.com/acme/app.git"))
        .await;
    devplane::poller::forge_once(&w.state).await;
    assert!(
        com.api_requests().is_empty(),
        "github.com is not signed in, so it is not asked"
    );
    let f = w.forge_state().await;
    let by_name = |n: &str| {
        f["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["project_name"] == n)
            .unwrap()["github"]["state"]
            .clone()
    };
    assert_eq!(by_name("tool"), "signed_in");
    assert_eq!(by_name("app"), "signed_out");

    w.sign_in("github.com").await;
    let com_before = com.api_requests().len();
    let ghe_before = ghe.api_requests().len();
    devplane::poller::forge_once(&w.state).await;
    assert_eq!(
        com.api_requests().len() - com_before,
        2,
        "search + one snapshot"
    );
    assert_eq!(
        ghe.api_requests().len() - ghe_before,
        2,
        "search + one snapshot"
    );
    let urls: Vec<String> = w.forge_state().await["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["url"].as_str().unwrap().to_string())
        .collect();
    assert!(
        urls.iter()
            .any(|u| u.starts_with("https://ghe.test/team/tool"))
    );
    assert!(
        urls.iter()
            .any(|u| u.starts_with("https://github.com/acme/app"))
    );
    double::assert_no_gh(&w.gh_log);
}

// ── What needs you, past the page ───────────────────────────────────────────

/// An issue assigned long ago and a review requested on a pull request past
/// the first page still reach the person, and a remote typed in another case
/// than GitHub spells the repository still matches. One search request per
/// host carries both.
#[tokio::test]
async fn what_needs_you_is_found_past_the_page_and_whatever_the_remotes_case() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("big", Some("https://github.com/ACME/Big.git"))
        .await;
    w.sign_in("github.com").await;
    let before = gh.api_requests().len();
    devplane::poller::forge_once(&w.state).await;
    let asked: Vec<String> = gh.api_requests().split_off(before);
    assert_eq!(asked, ["graphql:search", "graphql:snapshot"], "{asked:?}");

    let f = w.forge_state().await;
    let issue = |n: u64| {
        f["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["number"] == n)
            .cloned()
            .unwrap_or_else(|| panic!("no issue #{n} in {f}"))
    };
    let pr = |n: u64| {
        f["pull_requests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["number"] == n)
            .cloned()
            .unwrap_or_else(|| panic!("no pull request #{n} in {f}"))
    };
    assert_eq!(issue(40)["assigned_to_me"], true, "an old assignment");
    assert_eq!(pr(2)["review_requested"], true, "a row of the page");
    assert_eq!(pr(50)["review_requested"], true, "past the page");
    assert_eq!(pr(1)["review_requested"], false);
    let inbox = w.get("/api/inbox").await.to_string();
    assert!(inbox.contains("#40 is assigned to you"), "{inbox}");
    assert!(inbox.contains("Review requested: #50"), "{inbox}");
}

// ── Offering a branch that already has a pull request ───────────────────────

/// GitHub refuses a second pull request for a branch; the one already open
/// is the offer, not a failure.
#[tokio::test]
async fn a_branch_with_an_open_pull_request_offers_that_one() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    let (root, _) = w.project("app", Some("git@github.com:acme/app.git")).await;
    w.sign_in("github.com").await;
    let offered = github::create_pr(w.hub(), &root, "feat/9", "main", "t", "b", true)
        .await
        .unwrap();
    assert!(offered.existed);
    assert_eq!(offered.pull_request.number, 9);
    assert!(
        gh.api_requests()
            .iter()
            .any(|r| r == "graphql:pr_for_branch"),
        "{:?}",
        gh.api_requests()
    );
}

// ── An issue by its number ──────────────────────────────────────────────────

/// `change start --issue N` asks for N itself: an issue older than any list
/// is found, a closed one and a missing one are each said.
#[tokio::test]
async fn an_issue_is_found_by_number_whatever_its_age() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    let (root, _) = w.project("app", Some("git@github.com:acme/app.git")).await;
    w.sign_in("github.com").await;
    let (old, open) = github::issue(w.hub(), &root, 1234).await.unwrap().unwrap();
    assert_eq!((old.number, open), (1234, true));
    let (_, open) = github::issue(w.hub(), &root, 3).await.unwrap().unwrap();
    assert!(!open, "#3 is closed");
    assert_eq!(github::issue(w.hub(), &root, 404).await.unwrap(), None);
    assert!(
        !gh.api_requests().iter().any(|r| r == "graphql:issues"),
        "no list was read to find it"
    );
}

// ── Sign-in housekeeping ────────────────────────────────────────────────────

/// A token store that counts reads.
#[derive(Default)]
struct Counting {
    inner: Memory,
    gets: std::sync::atomic::AtomicUsize,
}

impl github::TokenStore for Counting {
    fn get(&self, host: &str) -> Result<Option<Secret>, Error> {
        self.gets.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        github::TokenStore::get(&self.inner, host)
    }
    fn set(&self, host: &str, token: &Secret) -> Result<(), Error> {
        github::TokenStore::set(&self.inner, host, token)
    }
    fn delete(&self, host: &str) -> Result<(), Error> {
        github::TokenStore::delete(&self.inner, host)
    }
    fn describe(&self, host: &str) -> String {
        github::TokenStore::describe(&self.inner, host)
    }
}

/// A poll does not open the credential store per request: the token is read
/// once and kept, and dropped when GitHub rejects it.
#[tokio::test]
async fn the_token_is_read_from_the_store_once() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    for i in 0..3 {
        w.project(
            &format!("r{i}"),
            Some(&format!("https://github.com/acme/r{i}.git")),
        )
        .await;
    }
    let store = Arc::new(Counting::default());
    w.hub().use_store(store.clone());
    w.sign_in("github.com").await;
    devplane::poller::forge_once(&w.state).await;
    devplane::poller::forge_once(&w.state).await;
    assert_eq!(
        store.gets.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "signing in keeps the token it stored"
    );
    w.hub().forget_token("github.com");
    devplane::poller::forge_once(&w.state).await;
    devplane::poller::forge_once(&w.state).await;
    assert_eq!(store.gets.load(std::sync::atomic::Ordering::SeqCst), 1);

    gh.set_mode(Mode::Unauthorized);
    devplane::poller::forge_once(&w.state).await;
    assert!(store.inner.hosts().is_empty(), "a 401 deletes it");
    assert!(matches!(
        w.hub().client("github.com").await,
        Err(Error::NotSignedIn(_))
    ));
}

/// A sign-in that finishes fills the Forge at once, without waiting for the
/// next tick.
#[tokio::test]
async fn a_finished_sign_in_reads_the_forge_at_once() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.project("app", Some("https://github.com/acme/app.git"))
        .await;
    gh.script(&["token"]);
    let (code, v) = w.post("/api/github/login", json!({})).await;
    assert_eq!(code, 200, "{v}");
    until(
        "the forge after sign-in",
        Duration::from_secs(10),
        || async {
            let f = w.forge_state().await;
            (!f["issues"].as_array().unwrap().is_empty()).then_some(())
        },
    )
    .await;
}

/// A denied attempt while an older sign-in stands leaves that sign-in and
/// says the denial beside it, so no surface reads it as success.
#[tokio::test]
async fn a_denied_sign_in_over_a_standing_one_is_said() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    w.sign_in("github.com").await;
    gh.script(&["denied"]);
    let (code, _) = w.post("/api/github/login", json!({})).await;
    assert_eq!(code, 200);
    let h = until("the attempt to end", Duration::from_secs(10), || async {
        let all = w.get("/api/github").await;
        let h = host_view(&all, "github.com").clone();
        (h["state"] != "pending").then_some(h)
    })
    .await;
    assert_eq!(h["state"], "signed_in", "the older sign-in stands");
    assert!(h["said"].as_str().unwrap().contains("denied"), "{h}");
}

/// A pending sign-in withdrawn by a sign-out ends as cancelled, not as an
/// expired code.
#[tokio::test]
async fn a_withdrawn_sign_in_says_cancelled() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    gh.script(&["pending", "pending", "pending"]);
    let p = w.hub().start("github.com").await.unwrap();
    let hub = w.state.clone();
    let waiting = tokio::spawn(async move { hub.github.wait(p).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(w.hub().logout("github.com").await.unwrap(), None);
    assert_eq!(waiting.await.unwrap().unwrap_err(), Error::Cancelled);
}

/// Signing out of a host nobody signed in to deletes nothing and says so.
#[tokio::test]
async fn signing_out_of_a_host_never_signed_in_claims_nothing() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    let (code, v) = w
        .post("/api/github/logout", json!({"host": "github.com"}))
        .await;
    assert_eq!(code, 200, "{v}");
    assert!(v["deleted_from"].is_null(), "{v}");
}

/// A sign-in whose record cannot be written takes its token back out of the
/// store: no token is left that no record names.
#[tokio::test]
async fn a_sign_in_that_cannot_be_recorded_leaves_no_token() {
    let gh = Double::start("github.com").await;
    let w = boot(&[&gh], "").await;
    // A directory where the record goes: the rename onto it fails.
    std::fs::create_dir_all(w.home.join("github.json").join("x")).unwrap();
    let e = w
        .hub()
        .finish("github.com", Secret::new(TOKEN))
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Store(_)), "{e:?}");
    assert!(w.store.hosts().is_empty(), "the token was taken back out");
}

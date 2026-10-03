//! Who approved a change's pull request, read from a GitHub double: the
//! change, its certificate and `devplane verify --approvals` each say the
//! same sentence, the double sees only `GET`s, and the token appears in no
//! output.
//!
//! The recorded tests serve the owner's recording of GitHub's own answers
//! (`tests/fixtures/github/merge-decider/`, made by
//! `scripts/capture-reviews.sh`). Until it exists they say they were skipped,
//! by name, and their `_synthetic` twins run the same assertions over the
//! double's hand-written answers.

mod common;
#[path = "common/github_double.rs"]
mod double;

use common::{git, scratch};
use devplane::core::Policy;
use devplane::core::change::{
    Change, CommandResult, CommitStamp, Completion, Declared, GateReport, PullRequestRef, Reach,
};
use devplane::github::{Memory, Secret};
use devplane::host::{AppState, Shared};
use double::{ApprovalsAnswer, Double, Mode, TOKEN};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const BIN: &str = env!("CARGO_BIN_EXE_devplane");

const SKIPPED: &str = "skipped — no recording; run scripts/capture-reviews.sh";

/// The recording, or `None` after saying the test was skipped.
fn recording(test: &str) -> Option<ApprovalsAnswer> {
    let r = double::recorded();
    if r.is_none() {
        eprintln!("{test}: {SKIPPED}");
    }
    r
}

struct World {
    state: Shared,
    addr: SocketAddr,
    http: reqwest::Client,
    home: PathBuf,
    /// Every body the API answered with, to search for the token.
    said: std::sync::Mutex<Vec<String>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

async fn boot(d: &Double) -> World {
    double::trap_gh();
    let home = scratch("md-home");
    std::fs::write(
        home.join("app.toml"),
        format!("[github.hosts.\"{}\"]\nclient_id = \"Iv1.test\"\n", d.name),
    )
    .unwrap();
    let state = AppState::new(
        home.join("devplane.db"),
        "tok".into(),
        Policy::default(),
        home.clone(),
    )
    .await
    .unwrap();
    state.github.use_store(Arc::new(Memory::default()));
    state.github.route(&d.name, d.host());
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    World {
        state,
        addr,
        http: reqwest::Client::new(),
        home,
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

    async fn sign_in(&self) {
        self.state
            .github
            .finish("github.com", Secret::new(TOKEN))
            .await
            .unwrap();
    }

    /// A project whose `origin` is `acme/app` on github.com, and a change on
    /// `feat/9` there — with a pull request, or without one.
    async fn change(&self, with_pr: bool) -> devplane::core::ChangeId {
        let dir = scratch("md-proj").join("app");
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(
            &dir,
            &["remote", "add", "origin", "git@github.com:acme/app.git"],
        );
        let root = dir.canonicalize().unwrap();
        let mut p = devplane::core::Project::from_root(root.clone());
        p.trusted = true;
        let pid = self.state.world.lock().await.upsert_project(p);
        let mut c = Change::new(pid, "Add rate limiting".into(), "do it".into());
        c.worktree = Some(root);
        c.branch = Some("feat/9".into());
        c.gates.push(gate_report());
        let basis = Completion::of(
            &c,
            Declared::Checks(&["cargo test".to_string()]),
            Some(&stamp()),
        );
        c.completion = Some(basis);
        if with_pr {
            c.pull_request = Some(PullRequestRef {
                number: 9,
                url: "https://github.com/acme/app/pull/9".into(),
                status: "ready_for_review".into(),
                ..Default::default()
            });
        }
        let id = c.id.clone();
        self.state.changes.lock().await.insert(id.clone(), c);
        id
    }

    async fn pull_request(&self, id: &devplane::core::ChangeId) -> Value {
        let v = self.get(&format!("/api/changes/{}", id.as_str())).await;
        v["pull_request"].clone()
    }

    async fn decisions(&self) -> usize {
        self.state
            .store
            .decisions(None, 10_000)
            .await
            .unwrap()
            .len()
    }

    fn said(&self) -> String {
        self.said.lock().unwrap().join("\n")
    }
}

fn stamp() -> CommitStamp {
    CommitStamp {
        commit: Some("4f2a9c1e0b7d3a5c9e1f2a4b6c8d0e2f4a6b8c0d".into()),
        tree: Some("0d8c0b6a4f2e0d8c6b4a2f1e9c5a3d7b0e1c9a2f".into()),
        tree_unknown: None,
        branch: Some("feat/9".into()),
        clean: true,
        changed_files: 0,
        reach: Reach::Remote,
        remote: Some("git@github.com:acme/app.git".into()),
    }
}

fn gate_report() -> GateReport {
    GateReport {
        junit: Vec::new(),
        base_commit: None,
        gate: "check".into(),
        at: jiff::Timestamp::now(),
        duration_ms: 10,
        commands: vec![CommandResult::exited("cargo test", 0)],
        attempt: 1,
        spec: None,
        commit: Some(stamp()),
    }
}

/// What a reading of these answers must say: composed by the one composer
/// from the answers themselves, for the double's host.
fn expected_sentence(a: &ApprovalsAnswer) -> String {
    use devplane::core::approvals::{self, ModelWord, Reading};
    use devplane::github::reviews;
    let list = reviews::parse_page(&Value::Array(a.reviews.clone()));
    let rules = a
        .rules
        .as_ref()
        .map(reviews::parse_rules)
        .map_err(|c| format!("status {c}"));
    let protection = match &rules {
        Ok(r) if !r.is_empty() => None,
        _ => Some(match &a.protection {
            Ok(v) => Ok(reviews::parse_protection(v)),
            Err(404) => Ok(None),
            Err(_) => Err(reviews::PROTECTION_UNREADABLE.to_string()),
        }),
    };
    Reading {
        number: 9,
        url: String::new(),
        head: a.head.clone(),
        base: "main".into(),
        required: approvals::required(rules, protection),
        required_says: None,
        bearing: approvals::bearing("github.com", &list, &a.head),
        unread_pages: None,
        sentence: String::new(),
        model_word: ModelWord::Unattributed,
        read_at: jiff::Timestamp::UNIX_EPOCH,
        after_merge: false,
    }
    .composed(approvals::model_ratified())
    .sentence
}

/// Only `GET`s, and only to the review routes and the GraphQL read.
fn assert_only_reads(d: &Double) {
    for r in d.api_requests() {
        assert!(
            r.starts_with("GET ") || r.starts_with("graphql:"),
            "a request that is not a read: {r}"
        );
    }
}

// ── The change ──────────────────────────────────────────────────────────────

async fn the_change_says_who_approved_over(answer: ApprovalsAnswer) -> (Value, String) {
    let gh = Double::start("github.com").await;
    gh.set_approvals("acme/app", answer.clone());
    let w = boot(&gh).await;
    w.sign_in().await;
    let id = w.change(true).await;
    let before = w.decisions().await;

    devplane::poller::pull_requests_once(&w.state).await;

    let pr = w.pull_request(&id).await;
    assert_eq!(pr["approvals"]["state"], "current", "{pr}");
    let sentence = pr["approvals"]["reading"]["sentence"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(sentence, expected_sentence(&answer));
    assert_eq!(pr["review_decision"], "APPROVED");
    assert!(
        pr["approvals"]["reading"]["bearing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["state"] == "DISMISSED" && b["kind"] == "person"),
        "the dismissed person's review is on the change: {pr}"
    );
    assert!(
        !gh.review_requests().is_empty(),
        "the reviews were read: {:?}",
        gh.requests()
    );
    assert_only_reads(&gh);
    assert!(!w.said().contains(TOKEN), "the token reached a response");
    assert_eq!(w.decisions().await, before, "a reading writes no decision");

    // The certificate says the same sentence.
    let cert = w
        .get(&format!("/api/changes/{}/certificate", id.as_str()))
        .await;
    assert_eq!(
        cert["statement"]["predicate"]["devplane"]["mergeApprovals"]["sentence"],
        sentence.as_str(),
        "{}",
        cert["statement"]
    );
    assert_eq!(cert["page"]["merge_approvals_says"], sentence.as_str());
    assert!(!w.said().contains(TOKEN));
    (pr, sentence)
}

#[tokio::test]
async fn the_change_says_who_approved() {
    let Some(answer) = recording("the_change_says_who_approved") else {
        return;
    };
    let source = double::recorded_source();
    eprintln!("recorded from {source:?}");
    the_change_says_who_approved_over(answer).await;
}

#[tokio::test]
async fn the_change_says_who_approved_synthetic() {
    let (_, sentence) = the_change_says_who_approved_over(double::synthetic()).await;
    // Before the record has a word for a model, a model's approval is never
    // a person's and never said as a model's decision.
    if !devplane::core::approvals::model_ratified() {
        assert_eq!(
            sentence,
            "1 of 1 required approvals: 1 could not be attributed — a model decided, and the \
             record has no word for it yet (Copilot code review, \
             copilot-pull-request-reviewer[bot]); 0 from a person's account. 1 dismissed: a \
             person's account (alice), on 9e1d0aa."
        );
    }
}

#[tokio::test]
async fn the_certificate_says_the_same_sentence() {
    let Some(answer) = recording("the_certificate_says_the_same_sentence") else {
        return;
    };
    // The change test checks the certificate beside the change.
    the_change_says_who_approved_over(answer).await;
}

#[tokio::test]
async fn the_certificate_says_the_same_sentence_synthetic() {
    let (pr, sentence) = the_change_says_who_approved_over(double::synthetic()).await;
    assert_eq!(pr["approvals"]["reading"]["sentence"], sentence.as_str());
}

#[tokio::test]
async fn no_pull_request_no_request() {
    let gh = Double::start("github.com").await;
    let w = boot(&gh).await;
    w.sign_in().await;
    let id = w.change(false).await;
    for _ in 0..3 {
        devplane::poller::pull_requests_once(&w.state).await;
    }
    assert!(gh.review_requests().is_empty(), "{:?}", gh.requests());
    // The sign-in's own `GET /user` is all the double saw.
    assert_eq!(gh.api_requests(), vec!["GET /user".to_string()]);
    let v = w.get(&format!("/api/changes/{}", id.as_str())).await;
    assert!(v["pull_request"].is_null());
}

#[tokio::test]
async fn an_unchanged_pull_request_is_not_re_read() {
    let gh = Double::start("github.com").await;
    let w = boot(&gh).await;
    w.sign_in().await;
    w.change(true).await;
    devplane::poller::pull_requests_once(&w.state).await;
    let first = gh.review_requests().len();
    assert!(first > 0);
    devplane::poller::pull_requests_once(&w.state).await;
    assert_eq!(gh.review_requests().len(), first, "the same updatedAt");
    gh.set_pr("OPEN", "2026-09-16T10:00:00Z");
    devplane::poller::pull_requests_once(&w.state).await;
    assert!(
        gh.review_requests().len() > first,
        "a moved pull request is read"
    );
}

#[tokio::test]
async fn a_merge_takes_one_final_reading() {
    let gh = Double::start("github.com").await;
    let w = boot(&gh).await;
    w.sign_in().await;
    let id = w.change(true).await;
    devplane::poller::pull_requests_once(&w.state).await;
    let first = gh.review_requests().len();
    gh.set_pr("MERGED", "2026-09-15T10:00:00Z");
    devplane::poller::pull_requests_once(&w.state).await;
    let after = gh.review_requests().len();
    assert!(after > first, "one more reading at merge");
    let pr = w.pull_request(&id).await;
    assert_eq!(pr["status"], "merged");
    assert_eq!(pr["approvals"]["reading"]["afterMerge"], true, "{pr}");
    devplane::poller::pull_requests_once(&w.state).await;
    devplane::poller::pull_requests_once(&w.state).await;
    assert_eq!(gh.review_requests().len(), after, "and never again");
}

#[tokio::test]
async fn signed_out_keeps_the_last_reading_marked_stale() {
    let gh = Double::start("github.com").await;
    let w = boot(&gh).await;
    w.sign_in().await;
    let id = w.change(true).await;
    devplane::poller::pull_requests_once(&w.state).await;
    let before = w.pull_request(&id).await;
    let sentence = before["approvals"]["reading"]["sentence"].clone();
    gh.set_mode(Mode::Unauthorized);
    devplane::poller::pull_requests_once(&w.state).await;
    let pr = w.pull_request(&id).await;
    assert_eq!(pr["approvals"]["state"], "stale", "{pr}");
    assert_eq!(pr["approvals"]["last"]["sentence"], sentence);
    assert!(
        pr["approvals"]["why"].as_str().unwrap().contains("expired"),
        "{pr}"
    );
    assert!(!w.said().contains("0 approvals"));
}

// ── The required count, where it lives ──────────────────────────────────────

#[tokio::test]
async fn the_required_count_names_its_source() {
    let gh = Double::start("github.com").await;
    let w = boot(&gh).await;
    w.sign_in().await;
    let base = double::synthetic();
    let cases = [
        (
            ApprovalsAnswer {
                rules: Ok(json!([{"type": "pull_request", "parameters": {
                    "required_approving_review_count": 2}}])),
                ..base.clone()
            },
            "1 of 2 required approvals",
            Some("Required by the branch's rules: 2 approvals."),
        ),
        (
            ApprovalsAnswer {
                rules: Ok(json!([])),
                protection: Ok(json!({"required_pull_request_reviews": {
                    "required_approving_review_count": 1, "dismiss_stale_reviews": false,
                    "require_code_owner_reviews": false}})),
                ..base.clone()
            },
            "1 of 1 required approvals",
            Some("Required by branch protection: 1 approval."),
        ),
        (
            ApprovalsAnswer {
                rules: Err(403),
                protection: Err(403),
                ..base.clone()
            },
            "1 approval; the number required could not be read",
            None,
        ),
    ];
    for (answer, head, rule) in cases {
        gh.set_approvals("acme/app", answer);
        let id = w.change(true).await;
        devplane::poller::pull_requests_once(&w.state).await;
        let pr = w.pull_request(&id).await;
        let r = &pr["approvals"]["reading"];
        let s = r["sentence"].as_str().unwrap();
        assert!(s.starts_with(head), "{s}");
        assert!(!s.contains("0 required"), "{s}");
        assert_eq!(r["requiredSays"].as_str(), rule, "{r}");
        w.state.changes.lock().await.remove(&id);
    }
}

// ── `verify --approvals` ────────────────────────────────────────────────────

/// A checkout on `main` whose `check` passes, and an event payload naming
/// pull request 9 of `acme/app`.
fn checkout(head: &str) -> (PathBuf, PathBuf) {
    let dir = common::scratch_repo("md-verify", common::PASSING);
    let event = dir.parent().unwrap().join("event.json");
    std::fs::write(
        &event,
        json!({"action": "submitted", "pull_request": {
            "number": 9, "html_url": "https://github.com/acme/app/pull/9",
            "head": {"sha": head}, "base": {"ref": "main"}}})
        .to_string(),
    )
    .unwrap();
    (dir, event)
}

async fn verify(dir: &Path, event: &Path, api: &str, out: &Path) -> std::process::Output {
    let (dir, event, api, out) = (
        dir.to_path_buf(),
        event.to_path_buf(),
        api.to_string(),
        out.to_path_buf(),
    );
    tokio::task::spawn_blocking(move || {
        std::process::Command::new(BIN)
            .args([
                "verify",
                "--approvals",
                "--json",
                "--no-base-checks",
                "--base",
                "main",
            ])
            .arg("--summary")
            .arg(&out)
            .current_dir(&dir)
            .env("DEVPLANE_HOME", dir.parent().unwrap().join("no-home"))
            .env("GITHUB_EVENT_PATH", &event)
            .env("GITHUB_EVENT_NAME", "pull_request_review")
            .env("GITHUB_API_URL", &api)
            .env("GITHUB_SERVER_URL", "https://github.com")
            .env("GITHUB_REPOSITORY", "acme/app")
            .env("GITHUB_TOKEN", TOKEN)
            .env_remove("GITHUB_STEP_SUMMARY")
            .env_remove("GITHUB_ACTIONS")
            .output()
            .unwrap()
    })
    .await
    .unwrap()
}

async fn verify_says_the_same_sentence_over(answer: ApprovalsAnswer) {
    let gh = Double::start("github.com").await;
    gh.set_approvals("acme/app", answer.clone());
    let (dir, event) = checkout(&answer.head);
    let out = dir.parent().unwrap().join("summary.md");
    let o = verify(&dir, &event, &gh.base(), &out).await;
    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
    assert_eq!(o.status.code(), Some(0), "{stdout}\n{stderr}");
    let doc: Value = serde_json::from_str(&stdout).unwrap();
    let sentence = expected_sentence(&answer);
    assert_eq!(doc["approvals"]["sentence"], sentence.as_str(), "{doc}");
    let md = std::fs::read_to_string(&out).unwrap();
    let section = md
        .split("### Approvals")
        .nth(1)
        .expect("an Approvals section");
    // The sentence as the summary renders repository text: inert.
    let shown = sentence
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('@', "&#64;");
    assert!(section.contains(&shown), "{section}");
    assert_only_reads(&gh);
    assert!(!gh.review_requests().is_empty());
    for text in [&stdout, &stderr, &md] {
        assert!(!text.contains(TOKEN), "the token was output");
    }
}

#[tokio::test]
async fn verify_says_the_same_sentence() {
    let Some(answer) = recording("verify_says_the_same_sentence") else {
        return;
    };
    verify_says_the_same_sentence_over(answer).await;
}

#[tokio::test]
async fn verify_says_the_same_sentence_synthetic() {
    verify_says_the_same_sentence_over(double::synthetic()).await;
}

// ── Evidence, never a decision ──────────────────────────────────────────────

/// The reading names no decision and writes none: the merge was decided on
/// the forge. (The host test above also counts the decision log.)
#[test]
fn a_reading_writes_no_decision() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("src/github/reviews.rs")];
    for e in std::fs::read_dir(root.join("src/core/approvals")).unwrap() {
        files.push(e.unwrap().path());
    }
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        let code = text.split("#[cfg(test)]").next().unwrap();
        for banned in [
            "Decision",
            "record_decision",
            "decide(",
            "INSERT INTO decisions",
        ] {
            assert!(!code.contains(banned), "{} names `{banned}`", f.display());
        }
    }
    // The poller's reading path takes no decision either.
    let poller = std::fs::read_to_string(root.join("src/poller.rs")).unwrap();
    let start = poller.find("pub async fn pull_requests_once").unwrap();
    let body = &poller[start..];
    let body = &body[..body.find("\n}\n").unwrap()];
    for banned in ["Decision", "record_decision", "decide("] {
        assert!(
            !body.contains(banned),
            "the poller's reading names `{banned}`"
        );
    }
}

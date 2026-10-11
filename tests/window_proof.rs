//! Where an answer came from, and who may trust a repository over HTTP, are
//! what the host can prove — never what a request says about itself. The
//! control token is readable by an agent's tools, so holding it proves only
//! that the caller is the person *or any process running as them*: an answer
//! is recorded from the window only with the window's proof for that ask, and
//! the trust route refuses everything but the window.

use devplane::core::Policy;
use devplane::host::{AppState, Shared};
use serde_json::Value;
use std::net::SocketAddr;

const TOKEN: &str = "tok";

/// A host on an ephemeral port; with `key`, as the app runs it.
async fn boot(key: Option<&str>) -> (SocketAddr, reqwest::Client, Shared) {
    let db = std::env::temp_dir().join(format!("dp-proof-{}.db", uuid::Uuid::new_v4().simple()));
    let state = AppState::new(
        db.clone(),
        TOKEN.into(),
        Policy::default(),
        db.with_extension("home"),
    )
    .await
    .unwrap();
    if let Some(k) = key {
        state.window_key.set(k.to_string()).unwrap();
    }
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new(), state)
}

/// A permission waiting on a run with no live session, as a restart leaves one.
async fn waiting(state: &Shared, id: &str) {
    let ask = devplane::core::ask::Ask::new(
        devplane::core::AskId::new(id),
        devplane::core::RunId::new(format!("run-{id}")),
        devplane::core::ask::Asked {
            kind: devplane::core::ask::Kind::Permission,
            request_id: String::new(),
            message: "Bash · ls".into(),
            payload: serde_json::json!({
                "tool": "Bash",
                "call": "ls",
                "options": [{"id": "allow_once"}, {"id": "reject_once"}],
            }),
            at: jiff::Timestamp::now(),
            deadline: devplane::core::ask::Deadline::Never,
        },
    );
    state.store.save_ask(&ask).await.unwrap();
}

/// Answers `id` with a deny (an allow is refused from inside an agent session,
/// where these tests may run), with whatever extra headers are given; the
/// path the host recorded.
async fn answer(
    c: &reqwest::Client,
    addr: &SocketAddr,
    id: &str,
    extra: &[(&str, String)],
) -> Value {
    let mut req = c
        .post(format!("http://{addr}/api/asks/{id}/answer"))
        .bearer_auth(TOKEN)
        .json(&serde_json::json!({ "decision": "deny", "from": "window" }));
    for (k, v) in extra {
        req = req.header(*k, v);
    }
    let res: Value = req.send().await.unwrap().json().await.unwrap();
    assert_eq!(res["open"], false, "the answer was taken: {res}");
    res["answered_from"].clone()
}

#[tokio::test]
async fn an_origin_header_does_not_make_an_answer_the_windows() {
    let key = devplane::config::window_key();
    let (addr, c, state) = boot(Some(&key)).await;
    waiting(&state, "a").await;
    let from = answer(&c, &addr, "a", &[("Origin", format!("http://{addr}"))]).await;
    assert_eq!(
        from, "cli",
        "a token holder sending a page's Origin and no proof is a client like the CLI"
    );
}

#[tokio::test]
async fn a_proof_for_one_ask_is_worth_nothing_for_another() {
    let key = devplane::config::window_key();
    let (addr, c, state) = boot(Some(&key)).await;
    waiting(&state, "a").await;
    waiting(&state, "b").await;
    let for_a = devplane::config::ask_proof(&key, "a");
    let from = answer(
        &c,
        &addr,
        "b",
        &[
            ("X-Devplane-Ask-Proof", for_a),
            ("Origin", format!("http://{addr}")),
        ],
    )
    .await;
    assert_eq!(
        from, "cli",
        "a proof replayed on another ask is not the window"
    );
}

#[tokio::test]
async fn the_windows_proof_records_the_window() {
    let key = devplane::config::window_key();
    let (addr, c, state) = boot(Some(&key)).await;
    waiting(&state, "a").await;
    let proof = devplane::config::ask_proof(&key, "a");
    let from = answer(&c, &addr, "a", &[("X-Devplane-Ask-Proof", proof)]).await;
    assert_eq!(from, "window");
    let row = state.store.ask("a").await.unwrap().unwrap();
    assert_eq!(
        row.answered_from.as_deref(),
        Some("window"),
        "and it is stored so"
    );
}

/// A malformed proof, a proof minted by an earlier run of the app (another
/// key), and a body naming the notification: each is a token holder, `cli`.
/// No request can make the host record `notification`.
#[tokio::test]
async fn nothing_a_request_carries_yields_the_window_or_the_notification() {
    let key = devplane::config::window_key();
    let (addr, c, state) = boot(Some(&key)).await;
    for id in ["m", "r", "n"] {
        waiting(&state, id).await;
    }
    let malformed = answer(
        &c,
        &addr,
        "m",
        &[("X-Devplane-Ask-Proof", "zz-not-hex".into())],
    )
    .await;
    assert_eq!(malformed, "cli");
    let before_restart = devplane::config::ask_proof(&devplane::config::window_key(), "r");
    let replayed = answer(&c, &addr, "r", &[("X-Devplane-Ask-Proof", before_restart)]).await;
    assert_eq!(
        replayed, "cli",
        "a proof from another run of the app proves nothing"
    );
    let res: Value = c
        .post(format!("http://{addr}/api/asks/n/answer"))
        .bearer_auth(TOKEN)
        .header("Origin", format!("http://{addr}"))
        .json(&serde_json::json!({ "decision": "deny", "from": "notification" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(res["answered_from"], "cli", "{res}");
}

#[tokio::test]
async fn a_host_with_no_window_records_every_answer_as_the_cli() {
    // `devplane up` in a browser: no key, so nothing can prove the window —
    // not even a proof made with some other key.
    let (addr, c, state) = boot(None).await;
    waiting(&state, "a").await;
    let forged = devplane::config::ask_proof("guessed", "a");
    let from = answer(
        &c,
        &addr,
        "a",
        &[
            ("X-Devplane-Ask-Proof", forged),
            ("Origin", format!("http://{addr}")),
        ],
    )
    .await;
    assert_eq!(from, "cli");
}

async fn trust(
    c: &reqwest::Client,
    addr: &SocketAddr,
    path: &str,
    extra: &[(&str, String)],
) -> (u16, Value) {
    let mut req = c
        .post(format!("http://{addr}/api/projects/trust"))
        .bearer_auth(TOKEN)
        .json(&serde_json::json!({ "path": path }));
    for (k, v) in extra {
        req = req.header(*k, v);
    }
    let res = req.send().await.unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

fn repo() -> String {
    let dir = std::env::temp_dir().join(format!("dp-trust-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.to_string_lossy().to_string()
}

#[tokio::test]
async fn the_control_token_alone_cannot_trust_a_repository() {
    let key = devplane::config::window_key();
    let (addr, c, state) = boot(Some(&key)).await;
    let path = repo();

    let (status, said) = trust(&c, &addr, &path, &[]).await;
    assert_eq!(status, 403, "{said}");
    assert!(
        said["error"]
            .as_str()
            .unwrap_or_default()
            .contains("devplane project trust"),
        "the refusal names the way a person does it: {said}"
    );
    let (status, _) = trust(&c, &addr, &path, &[("Origin", format!("http://{addr}"))]).await;
    assert_eq!(status, 403, "an Origin header is not the window");

    // The window's proof for another path, or for an answer, is not this one.
    let other = devplane::config::trust_proof(&key, "/somewhere/else");
    let (status, _) = trust(&c, &addr, &path, &[("X-Devplane-Trust-Proof", other)]).await;
    assert_eq!(status, 403);
    let ask = devplane::config::ask_proof(&key, &path);
    let (status, _) = trust(&c, &addr, &path, &[("X-Devplane-Trust-Proof", ask)]).await;
    assert_eq!(status, 403);

    assert!(
        devplane::trust::Trusted::read(&state.home)
            .unwrap()
            .roots
            .is_empty(),
        "nothing was trusted"
    );

    let proof = devplane::config::trust_proof(&key, &path);
    let (status, said) = trust(&c, &addr, &path, &[("X-Devplane-Trust-Proof", proof)]).await;
    assert_eq!(status, 200, "{said}");
    assert_eq!(said["trusted"], true);
    // Recorded in the person's file, not the store.
    let root = std::path::PathBuf::from(&path).canonicalize().unwrap();
    assert!(
        devplane::trust::Trusted::read(&state.home)
            .unwrap()
            .contains(&root)
    );
}

#[tokio::test]
async fn a_host_with_no_window_refuses_every_trust_request() {
    let (addr, c, _state) = boot(None).await;
    let path = repo();
    let forged = devplane::config::trust_proof("guessed", &path);
    let (status, _) = trust(&c, &addr, &path, &[("X-Devplane-Trust-Proof", forged)]).await;
    assert_eq!(status, 403);
}

/// Trust a person wrote to `trusted.toml` reaches a running host when it is
/// told to re-read, and the re-read grants nothing the file does not say: a
/// project row marked trusted in the store grants nothing at all.
#[tokio::test]
async fn reloading_takes_up_trust_recorded_in_its_file() {
    let (addr, c, state) = boot(None).await;
    let root = std::path::PathBuf::from(repo()).canonicalize().unwrap();
    let mut p = devplane::core::Project::from_root(root.clone());
    p.trusted = true;
    state.store.save_project(&p).await.unwrap();
    let reload = || async {
        c.post(format!("http://{addr}/api/projects/reload"))
            .bearer_auth(TOKEN)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    reload().await;
    assert!(
        !state
            .world
            .lock()
            .await
            .projects()
            .any(|q| q.root == root && q.trusted),
        "the store says nothing about trust"
    );
    devplane::trust::add(&state.home, [root.clone()]).unwrap();
    assert!(
        !state
            .world
            .lock()
            .await
            .projects()
            .any(|q| q.root == root && q.trusted),
        "not yet"
    );

    let res: Value = c
        .post(format!("http://{addr}/api/projects/reload"))
        .bearer_auth(TOKEN)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(res["projects"], 1, "{res}");
    assert!(
        state
            .world
            .lock()
            .await
            .projects()
            .any(|q| q.root == root && q.trusted)
    );
}

/// Only a debug build may take the window's key from the environment, so the
/// bars can hand a browser the key the app hands its webview. A release build
/// that read it would let anything that sets a variable pass as the window.
#[test]
fn only_a_debug_build_reads_the_window_key_from_the_environment() {
    let mut reads = 0;
    for entry in walk(std::path::Path::new("src")) {
        let text = std::fs::read_to_string(&entry).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains("\"DEVPLANE_WINDOW_KEY\"") {
                continue;
            }
            reads += 1;
            let guarded = lines[..i]
                .iter()
                .rev()
                .take(3)
                .any(|l| l.trim() == "#[cfg(debug_assertions)]");
            assert!(
                guarded,
                "{}:{} reads DEVPLANE_WINDOW_KEY outside a #[cfg(debug_assertions)] block",
                entry.display(),
                i + 1
            );
        }
    }
    assert!(
        reads > 0,
        "no read of DEVPLANE_WINDOW_KEY was found to check"
    );
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

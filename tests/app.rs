//! The app's host, driven through the API its page uses, served on loopback in
//! this process. Built with the `app` feature only.

#![cfg(feature = "app")]
#![cfg(unix)]

mod common;

use devplane::core::Policy;
use devplane::host::AppState;
use serde_json::Value;
use std::path::{Path, PathBuf};

const SETTLE: std::time::Duration = std::time::Duration::from_secs(90);
const POLL: std::time::Duration = std::time::Duration::from_millis(100);

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-app-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A trusted repository with one gate.
fn scratch_repo(tag: &str) -> PathBuf {
    let dir = scratch_dir(&format!("repo-{tag}"));
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("git");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(
        dir.join("devplane.toml"),
        "[gates]\ncheck = [\"true\"]\ntimeout = \"30s\"\n",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

fn echo_agent() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?.parent()?.join("examples").join("echo_agent");
    bin.exists().then(|| bin.to_string_lossy().to_string())
}

async fn state_in(home: &Path) -> devplane::host::Shared {
    AppState::new(
        home.join("devplane.db"),
        "tok".into(),
        Policy::default(),
        home.to_path_buf(),
    )
    .await
    .unwrap()
}

/// The host as a router on a port, with no `host.json`.
async fn boot(
    home: &Path,
) -> (
    std::net::SocketAddr,
    reqwest::Client,
    devplane::host::Shared,
) {
    let state = state_in(home).await;
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new(), state)
}

async fn post(c: &reqwest::Client, addr: &std::net::SocketAddr, path: &str, body: Value) -> Value {
    c.post(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn get(c: &reqwest::Client, addr: &std::net::SocketAddr, path: &str) -> Value {
    c.get(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn trust(c: &reqwest::Client, addr: &std::net::SocketAddr, repo: &Path) {
    let res = post(
        c,
        addr,
        "/api/projects/trust",
        serde_json::json!({ "path": repo.to_string_lossy() }),
    )
    .await;
    assert_eq!(res["trusted"], true, "{res}");
}

/// Polls the *needs you* list for a row.
async fn await_needs_you(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    want: impl Fn(&Value) -> bool,
) -> Option<Value> {
    let deadline = std::time::Instant::now() + SETTLE;
    while std::time::Instant::now() < deadline {
        let inbox = get(c, addr, "/api/inbox?needs_you=true").await;
        if let Some(hit) = inbox["items"]
            .as_array()
            .and_then(|items| items.iter().find(|i| want(i)))
        {
            return Some(hit.clone());
        }
        tokio::time::sleep(POLL).await;
    }
    None
}

fn echo_pids() -> std::collections::HashSet<u32> {
    devplane::observe::procs::snapshot()
        .into_iter()
        .filter(|p| p.command.contains("echo_agent"))
        .map(|p| p.pid)
        .collect()
}

/// Waiting time is measured from the question, not from when the app opened.
#[tokio::test]
async fn a_question_asked_an_hour_ago_reads_as_an_hour_old_not_as_new() {
    use devplane::core::ask::{Ask, Asked, Deadline, Kind};
    use devplane::core::ids::{AskId, RunId};

    let home = scratch_dir("since");
    let an_hour_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(1);
    {
        let store = devplane::store::Store::open(&home.join("devplane.db"))
            .await
            .unwrap();
        let ask = Ask::new(
            AskId::new("a-last-night"),
            RunId::new("r-gone"),
            Asked {
                kind: Kind::Question,
                request_id: "1".into(),
                message: "Keep the legacy route?".into(),
                payload: serde_json::json!({"options": [{"id": "keep", "label": "Keep it"}]}),
                at: an_hour_ago,
                deadline: Deadline::Never,
            },
        );
        store.save_ask(&ask).await.unwrap();
    }

    let state = state_in(&home).await;
    let items = state.current_inbox().await;
    let row = items
        .iter()
        .find(|i| i.ask.as_ref().is_some_and(|a| a.as_str() == "a-last-night"))
        .expect("the stranded question is in the inbox");
    let drift = row.since.duration_since(an_hour_ago).abs();
    assert!(
        drift < jiff::SignedDuration::from_secs(5),
        "since is {} and the question was asked at {an_hour_ago}",
        row.since
    );
    std::fs::remove_dir_all(&home).ok();
}

/// Answering through the app's *needs you* route releases the agent and records
/// the person.
#[tokio::test]
async fn answering_through_the_answer_surface_names_the_person_and_releases_the_run() {
    let Some(agent) = echo_agent() else { return };
    let _serial = common::one_agent_at_a_time();
    let home = scratch_dir("answer");
    let repo = scratch_repo("answer");
    let (addr, c, state) = boot(&home).await;
    trust(&c, &addr, &repo).await;

    let spec = devplane::acp::resolve(&agent, &state.agents).expect("the echo agent resolves");
    let run = devplane::driven::dispatch(
        &state,
        &spec,
        repo.clone(),
        Some("question".into()),
        Vec::new(),
    )
    .await
    .expect("a run")
    .to_string();

    let row = await_needs_you(&c, &addr, |i| {
        i["kind"] == "question" && i["run_id"] == run.as_str()
    })
    .await
    .expect("the question reaches the needs-you list");
    let ask = row["ask"].as_str().expect("answerable").to_string();
    let field = row["form"][0]["field"]
        .as_str()
        .unwrap_or("question_0")
        .to_string();
    let option = row["form"][0]["options"][0]["value"]
        .as_str()
        .unwrap_or("Keep it")
        .to_string();

    let answered = post(
        &c,
        &addr,
        &format!("/api/asks/{ask}/answer"),
        serde_json::json!({ "option": option, "field": field, "from": "board" }),
    )
    .await;
    assert_eq!(answered["open"], false, "{answered}");

    // Released: the run leaves *waiting*.
    let deadline = std::time::Instant::now() + SETTLE;
    loop {
        let r = get(&c, &addr, &format!("/api/runs/{run}")).await;
        if r["state"] != "waiting" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the run is still waiting: {r}"
        );
        tokio::time::sleep(POLL).await;
    }

    // The decision is the person's.
    let decisions = get(&c, &addr, "/api/decisions").await;
    let mine = decisions.as_array().unwrap().iter().find(|d| {
        d["authority"] == "person" && d["run_id"] == run.as_str() && d["action"] == "agent:question"
    });
    assert!(
        mine.is_some(),
        "no decision by a person names the run's question: {decisions}"
    );

    for (_, s) in state.sessions.lock().await.drain() {
        s.stop();
    }
    std::fs::remove_dir_all(&home).ok();
    std::fs::remove_dir_all(&repo).ok();
}

/// Without an opener (the CLI host), the route returns the path as a sentence
/// and opens nothing.
#[tokio::test]
async fn the_browser_is_handed_the_path_to_open_itself() {
    let Some(agent) = echo_agent() else { return };
    let _serial = common::one_agent_at_a_time();
    let home = scratch_dir("open");
    let repo = scratch_repo("open");
    let (addr, c, state) = boot(&home).await;
    trust(&c, &addr, &repo).await;

    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({ "cwd": repo.to_string_lossy(), "title": "open me", "agent": agent }),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let id = started["change_id"].as_str().unwrap().to_string();
    let worktree = started["change"]["worktree"]
        .as_str()
        .expect("an isolated checkout")
        .to_string();

    for place in ["editor", "terminal"] {
        let r = post(
            &c,
            &addr,
            &format!("/api/changes/{id}/open?in={place}"),
            serde_json::json!({}),
        )
        .await;
        assert_eq!(r["opened"], false, "{r}");
        assert_eq!(r["path"], worktree.as_str(), "{r}");
        assert_eq!(
            r["says"],
            format!("open it yourself: {worktree}").as_str(),
            "{r}"
        );
    }
    let r = c
        .post(format!("http://{addr}/api/changes/{id}/open?in=nowhere"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400, "an unknown place is refused");

    for (_, s) in state.sessions.lock().await.drain() {
        s.stop();
    }
    std::fs::remove_dir_all(&home).ok();
    std::fs::remove_dir_all(&repo).ok();
}

/// Serves the host in this process with `host.json` in `home`; returns the port.
async fn serve_in(
    home: &Path,
    state: &devplane::host::Shared,
) -> (tokio::task::JoinHandle<anyhow::Result<()>>, u16) {
    // SAFETY: the agent lock serialises every test that serves a host.
    unsafe { std::env::set_var("DEVPLANE_HOME", home) };
    let serving = tokio::spawn(devplane::host::serve(state.clone(), 0));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Ok(text) = std::fs::read_to_string(home.join("host.json"))
            && let Ok(v) = serde_json::from_str::<Value>(&text)
            && v["pid"].as_u64() == Some(u64::from(std::process::id()))
        {
            return (serving, v["port"].as_u64().unwrap() as u16);
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the host never published where it is listening"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

fn autostart_listing() -> Vec<String> {
    let dir = if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join("Library/LaunchAgents"))
    } else {
        dirs::home_dir().map(|h| h.join(".config/autostart"))
    };
    let mut names: Vec<String> = dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

/// Quitting from the app names the agent first (the CLI's sentence), stops it,
/// and leaves no agent, no `host.json` and nothing in the autostart folder.
#[tokio::test]
async fn quitting_from_the_app_names_the_agent_leaves_nothing_and_installs_nothing() {
    let Some(agent) = echo_agent() else { return };
    let _serial = common::one_agent_at_a_time();
    let autostart_before = autostart_listing();
    let home = scratch_dir("quit");
    let repo = scratch_repo("quit");
    let state = state_in(&home).await;
    let (serving, port) = serve_in(&home, &state).await;
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let c = reqwest::Client::new();
    trust(&c, &addr, &repo).await;

    let before = echo_pids();
    let spec = devplane::acp::resolve(&agent, &state.agents).expect("the echo agent resolves");
    let run =
        devplane::driven::dispatch(&state, &spec, repo.clone(), Some("slow".into()), Vec::new())
            .await
            .expect("a run")
            .to_string();
    let mut agent_pid = None;
    for _ in 0..100 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Some(new) = echo_pids().difference(&before).copied().next() {
            agent_pid = Some(new);
            break;
        }
    }
    let agent_pid = agent_pid.expect("the dispatch started no agent, so this proves nothing");
    assert!(devplane::poller::process_alive(agent_pid));

    // 1. Named before it happens, in the CLI's sentence.
    let held = get(&c, &addr, "/api/quitting").await;
    let says = held["says"].as_str().expect("the sentence").to_string();
    assert!(
        says.contains(&run),
        "the quit does not name the run: {says}"
    );
    let counted: devplane::core::reduce::facts::Quitting =
        serde_json::from_value(held.clone()).unwrap();
    assert_eq!(
        counted.says(),
        says,
        "the served sentence is Quitting::says()"
    );
    assert_eq!(counted.stops.len(), 1);

    // 2. Stopped through the quit request.
    let quit = post(&c, &addr, "/api/quit", serde_json::json!({})).await;
    assert_eq!(quit["stopping"], true, "{quit}");
    let done = tokio::time::timeout(std::time::Duration::from_secs(30), serving)
        .await
        .expect("the host stops within thirty seconds")
        .unwrap();
    assert!(done.is_ok(), "{done:?}");

    // 3. Nothing remains.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while devplane::poller::process_alive(agent_pid) {
        assert!(
            std::time::Instant::now() < deadline,
            "the agent {agent_pid} outlived the quit"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        !home.join("host.json").exists(),
        "host.json outlived the host"
    );

    assert_eq!(
        autostart_listing(),
        autostart_before,
        "something was written to the autostart folder"
    );
    std::fs::remove_dir_all(&home).ok();
    std::fs::remove_dir_all(&repo).ok();
}

/// The app's source never names an auto-start mechanism.
#[test]
fn the_app_never_names_an_autostart_mechanism() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    let forbidden = [
        "LaunchAgents",
        "autostart",
        "login item",
        "RunAtLoad",
        "tauri-plugin-autostart",
        "tauri_plugin_autostart",
    ];
    let mut checked = 0;
    for e in std::fs::read_dir(&dir).expect("src/app").flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        checked += 1;
        let text = std::fs::read_to_string(&p).unwrap().to_lowercase();
        for word in forbidden {
            assert!(
                !text.contains(&word.to_lowercase()),
                "{} names `{word}`; the app installs nothing that starts at login",
                p.display()
            );
        }
    }
    assert!(checked >= 5, "the guard read {checked} files under src/app");
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    assert!(
        !manifest.contains("tauri-plugin-autostart"),
        "the autostart plugin is a dependency"
    );
}

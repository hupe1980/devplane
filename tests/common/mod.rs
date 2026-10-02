//! Shared test scaffolding: scratch directories and repositories, the fixture
//! agent, and an in-process host with its HTTP helpers.
//!
//! Tests that drive an agent run in parallel, within and across test binaries.
//! No lock serialises them: a test that looks for "its" agent among the
//! machine's processes filters by parent pid (the host `exec`s the agent
//! through `sh`, so the host is the parent), never by "new since I looked".
//! An earlier global `flock` here was blamed on `async-process`'s reactor; the
//! one failure it actually hid was that pid race.

#![allow(dead_code)] // each test binary uses a different part of this

use devplane::core::Policy;
use devplane::host::{AppState, Shared};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

// ── Scratch space ────────────────────────────────────────────────────────────

/// A fresh, empty directory under the temp dir, unique to this call.
/// Not canonicalised; see [`scratch_repo`] for one the host will compare.
pub fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dp-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Runs git in `dir`, failing the test with git's stderr if it fails. Returns
/// trimmed stdout. An author is set in the environment, so a commit works
/// wherever `HOME` points.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("git is on PATH");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A repository on `main` with `devplane.toml` set to `config` and `files`
/// alongside, all in one commit. Canonicalised: macOS temp dirs sit behind a
/// symlink the host resolves.
pub fn scratch_repo_with(tag: &str, config: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = scratch(&format!("repo-{tag}"));
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("devplane.toml"), config).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

/// [`scratch_repo_with`] and no other files.
pub fn scratch_repo(tag: &str, config: &str) -> PathBuf {
    scratch_repo_with(tag, config, &[])
}

/// One gate that always passes.
pub const PASSING: &str = "[gates]\ncheck = [\"true\"]\ntimeout = \"30s\"\n";

// ── The fixture agent ────────────────────────────────────────────────────────

/// The fixture ACP agent next to the test binary, or `None` when it has not
/// been built (`cargo build --all-targets`). Panics if it is older than its
/// source: `cargo test` does not rebuild examples, and a stale agent passes
/// for the wrong reason, which is worse than failing.
pub fn echo_agent() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    // target/debug/deps/<binary> → target/debug/examples/echo_agent
    let bin = exe
        .parent()?
        .parent()?
        .join("examples")
        .join(if cfg!(windows) {
            "echo_agent.exe"
        } else {
            "echo_agent"
        });
    if !bin.exists() {
        return None;
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/echo_agent.rs");
    if let (Ok(b), Ok(s)) = (
        std::fs::metadata(&bin).and_then(|m| m.modified()),
        std::fs::metadata(&src).and_then(|m| m.modified()),
    ) && s > b
    {
        panic!("the fixture agent is older than its source: run `cargo build --examples`");
    }
    Some(bin.to_string_lossy().into_owned())
}

/// [`echo_agent`] as a launch spec.
pub fn echo_spec() -> Option<devplane::acp::AgentSpec> {
    echo_agent().map(|bin| devplane::acp::AgentSpec::new("echo", "Echo", &bin))
}

// ── An in-process host ───────────────────────────────────────────────────────

/// A host for one test that stops its agents on drop, so a panicking test
/// leaves no ACP session behind.
pub struct Host(pub Shared);

impl std::ops::Deref for Host {
    type Target = Shared;
    fn deref(&self) -> &Shared {
        &self.0
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        for _ in 0..100 {
            if let Ok(mut map) = self.0.sessions.try_lock() {
                for (_, session) in map.drain() {
                    session.stop();
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

/// A host with a fresh database and the default policy, serving on a free
/// loopback port with bearer token `tok`.
pub async fn boot() -> (SocketAddr, reqwest::Client, Host) {
    let db = std::env::temp_dir().join(format!("dp-host-{}.db", uuid::Uuid::new_v4().simple()));
    let state = AppState::new(
        db.clone(),
        "tok".into(),
        Policy::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new(), Host(state))
}

pub async fn get(c: &reqwest::Client, addr: &SocketAddr, path: &str) -> Value {
    c.get(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn post(c: &reqwest::Client, addr: &SocketAddr, path: &str, body: Value) -> Value {
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

/// Trusts `repo` through the API, as a person would.
pub async fn trust(c: &reqwest::Client, addr: &SocketAddr, repo: &Path) {
    let res = post(
        c,
        addr,
        "/api/projects/trust",
        serde_json::json!({ "path": repo.to_string_lossy() }),
    )
    .await;
    assert_eq!(res["trusted"], true, "{res}");
}

// ── Waiting ──────────────────────────────────────────────────────────────────

/// Polls `f` every 20 ms until it returns `Some`, or panics naming `what`
/// after `within`. Replaces a fixed sleep with the condition it stood for.
pub async fn until<T, F, Fut>(what: &str, within: std::time::Duration, mut f: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = std::time::Instant::now() + within;
    loop {
        if let Some(v) = f().await {
            return v;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out after {within:?} waiting for {what}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

// ── Spawned hosts ────────────────────────────────────────────────────────────

/// Kills a spawned host if the test ends (e.g. panics) before reaping it.
/// Holds the pid so the child can be handed to a reaper thread; `disarm` once
/// the process is seen to exit, so a reused pid is never signalled.
pub struct HostGuard(Option<u32>);

impl HostGuard {
    pub fn new(pid: u32) -> Self {
        Self(Some(pid))
    }

    pub fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for HostGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            // SAFETY: a signal to a pid this test spawned and has not seen exit.
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
    }
}

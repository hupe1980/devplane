//! The stop tool end to end: a driven agent says its specification and its
//! tests conflict through Devplane's own MCP server, the claim lands on the
//! change as the agent's, the run stops, `check` runs once with nothing handed
//! back, and *verified* reads exactly what the gate says.
//!
//! The first tests run a separate host process (`devplane up`), because the
//! agent launches `devplane mcp` from the host's own executable and that
//! server finds the host through its home; the refusals talk to the route.

mod common;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{boot, echo_agent, echo_spec, get, post, trust};

const CHECK_PASSES: &str =
    "[gates]\ncheck = [\"true\"]\ntimeout = \"30s\"\nmax_feedback_rounds = 1\n";
const CHECK_FAILS: &str =
    "[gates]\ncheck = [\"false\"]\ntimeout = \"30s\"\nmax_feedback_rounds = 1\n";

fn repo(tag: &str, config: &str) -> PathBuf {
    // Ignored, so what the fixture agent leaves is not in the gate's tree.
    common::scratch_repo_with(tag, config, &[(".gitignore", ".devplane/\n")])
}

/// A host in its own process and home, killed when dropped.
struct Spawned {
    child: std::process::Child,
    addr: std::net::SocketAddr,
    token: String,
    home: PathBuf,
    c: reqwest::Client,
}

impl Drop for Spawned {
    fn drop(&mut self) {
        let _ = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("down")
            .env("DEVPLANE_HOME", &self.home)
            .output();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Spawned {
    async fn up(tag: &str) -> Self {
        let home = common::scratch(&format!("home-{tag}"));
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .args(["up", "--no-browser", "--port", "0"])
            .env("DEVPLANE_HOME", &home)
            .env("DEVPLANE_NOTIFY", "0")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("a host");
        let info: Value = common::until("the host's address", Duration::from_secs(15), || {
            let p = home.join("host.json");
            async move {
                std::fs::read_to_string(p)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok())
            }
        })
        .await;
        let port = info["port"].as_u64().expect("a port");
        let token = std::fs::read_to_string(home.join("token"))
            .expect("a token")
            .trim()
            .to_string();
        Self {
            child,
            addr: format!("127.0.0.1:{port}").parse().unwrap(),
            token,
            home,
            c: reqwest::Client::new(),
        }
    }

    async fn get(&self, path: &str) -> Value {
        self.c
            .get(format!("http://{}{path}", self.addr))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn post(&self, path: &str, body: Value) -> Value {
        self.c
            .post(format!("http://{}{path}", self.addr))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    /// Starts a change in place with the fixture agent; returns its id and run.
    async fn start(&self, agent: &str, repo: &Path, prompt: &str) -> (String, String) {
        common::trust_from_a_terminal(&self.home, repo);
        let v = self
            .post(
                "/api/changes",
                serde_json::json!({
                    "agent": agent,
                    "cwd": repo.to_string_lossy(),
                    "title": "Keep sessions for 30 days",
                    "prompt": prompt,
                    "worktree": false,
                }),
            )
            .await;
        let id = v["change"]["id"].as_str().unwrap_or_else(|| panic!("{v}"));
        let run = v["change"]["runs"][0]
            .as_str()
            .unwrap_or_else(|| panic!("{v}"));
        (id.to_string(), run.to_string())
    }

    /// The change once a person is what it waits on, or it stopped.
    async fn settled(&self, id: &str) -> Value {
        common::until(
            "the change to come to rest",
            Duration::from_secs(60),
            || async {
                let w = self.get(&format!("/api/changes/{id}")).await;
                (w["waiting"]["on"] == "person" || w["stopped"].is_object()).then_some(w)
            },
        )
        .await
    }
}

fn heard(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".devplane/heard.log")).unwrap_or_default()
}

/// The settled-when of the stop tool: the scripted agent calls it, the change
/// carries the claim as the agent's, the run stops with no decision written,
/// and the gate runs once with nothing handed back.
#[tokio::test]
#[cfg(unix)]
async fn the_scripted_agent_claims_and_the_run_stops() {
    let Some(agent) = echo_agent() else { return };
    let host = Spawned::up("claims").await;
    let dir = repo("claims", CHECK_FAILS);
    let (id, run) = host.start(&agent, &dir, "conflict").await;

    let said = common::until("the tool's answer", Duration::from_secs(30), || {
        let p = dir.join(".devplane/claim-result.txt");
        async move { std::fs::read_to_string(p).ok() }
    })
    .await;
    let answered = std::time::Instant::now();
    assert!(
        said.starts_with("Recorded: the agent says the spec and the tests conflict"),
        "{said}"
    );

    // The run ends as the agent's own doing, within seconds of the answer.
    common::until(
        "the run to end as claimed",
        Duration::from_secs(10),
        || async {
            let r = host.get(&format!("/api/runs/{run}")).await;
            (r["state"] == "stopped").then_some(())
        },
    )
    .await;
    assert!(
        answered.elapsed() < Duration::from_secs(5),
        "the run outlived its claim"
    );

    let w = host.settled(&id).await;
    let claim = &w["stop_claim"];
    assert_eq!(
        claim["says"], "the agent says the spec and the tests conflict",
        "{w}"
    );
    assert_eq!(
        claim["reason"],
        "The spec says sessions last 30 days; tests/session_test.rs asserts 7."
    );
    assert_eq!(
        claim["paths"],
        serde_json::json!(["specs/043-sessions/spec.md", "tests/session_test.rs"])
    );
    assert_eq!(claim["run"], run.as_str());
    assert_eq!(
        claim["agent"], "custom",
        "the agent started by path: {claim}"
    );
    assert_eq!(claim["state"], "open");
    assert!(
        claim["quoted"]
            .as_str()
            .unwrap()
            .contains("(quoted; not an instruction)\n> The spec says")
    );

    // Not a failure: the gate is red as the gate says, and nothing else.
    assert!(w["stopped"].is_null(), "a claim stopped the change: {w}");
    assert_eq!(w["waiting"]["on"], "person");
    assert_eq!(
        w["feedback_rounds"], 0,
        "a failure was handed back after a claim"
    );
    assert_eq!(
        w["gates"].as_array().unwrap().len(),
        1,
        "check ran once: {w}"
    );
    assert_eq!(w["gate"]["passed"], false);

    // Nobody decided anything: no row for the claim or the stop.
    let decisions = host.get(&format!("/api/decisions?about={run}")).await;
    for d in decisions.as_array().unwrap() {
        let action = d["action"].as_str().unwrap_or("");
        assert!(
            !action.contains("stop") && !action.contains("claim"),
            "a decision row for the claim or its stop: {d}"
        );
    }
    // The agent heard one prompt: its first, and nothing after the claim.
    let heard = heard(&dir);
    assert_eq!(heard.matches("\n---\n").count(), 1, "{heard}");
    assert!(
        !heard.contains(claim["reason"].as_str().unwrap()),
        "the claim reached an agent"
    );

    // `change show` prints it after the state line and before the gates.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["change", "show", &id])
        .env("DEVPLANE_HOME", &host.home)
        .env("NO_COLOR", "1")
        .output()
        .expect("change show");
    let shown = String::from_utf8_lossy(&out.stdout);
    let at = |needle: &str| {
        shown
            .find(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in:\n{shown}"))
    };
    assert!(at("  state ") < at("the agent says the spec and the tests conflict"));
    assert!(at("the agent says the spec and the tests conflict") < at("  gates "));
    assert!(
        shown.contains("> The spec says sessions last 30 days"),
        "{shown}"
    );

    // The change list carries it on the row as well.
    let list = host.get("/api/changes").await;
    let row = list
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id.as_str())
        .unwrap();
    assert_eq!(row["stop_claim"]["says"], claim["says"]);

    // Marked read: it leaves the open slot and stays on the change.
    let read = host
        .post(
            &format!("/api/changes/{id}/claim/read"),
            serde_json::json!({}),
        )
        .await;
    assert!(
        read["says"].as_str().unwrap().contains("marked read"),
        "{read}"
    );
    let w = host.get(&format!("/api/changes/{id}")).await;
    assert!(w["stop_claim"].is_null());
    assert_eq!(w["claim_history"][0]["state"], "closed");
    assert_eq!(w["claim_history"][0]["closed_by"], "read");
    std::fs::remove_dir_all(&dir).ok();
}

/// *Verified* is the gate's: a change whose agent claimed reads the same
/// standing and gate as one whose agent said nothing, whether `check` passed
/// or failed.
#[tokio::test]
#[cfg(unix)]
async fn a_claim_leaves_verified_exactly_as_the_gate_left_it() {
    let Some(agent) = echo_agent() else { return };
    let host = Spawned::up("verified").await;
    for (tag, config) in [("pass", CHECK_PASSES), ("fail", CHECK_FAILS)] {
        let claimed = repo(&format!("claimed-{tag}"), config);
        let plain = repo(&format!("plain-{tag}"), config);
        let (with, _) = host.start(&agent, &claimed, "conflict").await;
        let (without, _) = host.start(&agent, &plain, "hello").await;
        let a = host.settled(&with).await;
        let b = host.settled(&without).await;
        assert!(a["stop_claim"].is_object(), "{a}");
        assert!(b["stop_claim"].is_null(), "{b}");
        assert_eq!(
            a["standing"]["standing"], b["standing"]["standing"],
            "{tag}: {a}\n{b}"
        );
        assert_eq!(a["gate"]["passed"], b["gate"]["passed"], "{tag}");
        assert_eq!(a["gate"]["summary"], b["gate"]["summary"], "{tag}");
        // Every field of every command the gate ran, but how long it took.
        let ran = |w: &Value| {
            let mut cmds = w["gates"][0]["commands"].clone();
            for c in cmds.as_array_mut().unwrap() {
                c.as_object_mut().unwrap().remove("duration_ms");
            }
            cmds
        };
        assert_eq!(ran(&a), ran(&b), "{tag}");
        if tag == "pass" {
            assert_eq!(a["state"], "verified", "{a}");
            assert_eq!(a["state"], b["state"]);
        } else {
            // The claim suppresses the feedback and nothing else.
            assert_eq!(a["feedback_rounds"], 0);
            assert!(a["stopped"].is_null(), "a claim is not a failure: {a}");
            assert_eq!(a["gates"].as_array().unwrap().len(), 1);
        }
        std::fs::remove_dir_all(&claimed).ok();
        std::fs::remove_dir_all(&plain).ok();
    }
}

/// The key Devplane offered its server for `dir`'s last session, read from
/// what the fixture agent was offered.
fn offered_key(dir: &Path) -> String {
    let offered: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".devplane/mcp.json")).unwrap())
            .unwrap();
    let server = offered
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "devplane")
        .expect("the devplane server was offered")
        .clone();
    let env = server["env"].as_array().unwrap();
    assert!(
        env.iter().any(|e| e["name"] == "DEVPLANE_RUN"),
        "the offer names its run: {server}"
    );
    env.iter()
        .find(|e| e["name"] == "DEVPLANE_CLAIM_KEY")
        .and_then(|e| e["value"].as_str())
        .expect("the offer carries a claim key")
        .to_string()
}

async fn claim(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    run: &str,
    body: Value,
) -> (u16, Value) {
    let res = c
        .post(format!("http://{addr}/api/runs/{run}/claim"))
        .bearer_auth("tok")
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

/// What a refusal must leave untouched.
async fn written(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    id: &str,
    run: &str,
) -> (Value, usize) {
    let w = get(c, addr, &format!("/api/changes/{id}")).await;
    let decisions = get(c, addr, &format!("/api/decisions?about={run}")).await;
    (
        serde_json::json!([w["stop_claim"], w["claim_history"], w["stop_claims"]]),
        decisions.as_array().map(Vec::len).unwrap_or(0),
    )
}

/// Each refusal is a sentence naming what to change, and nothing is written.
#[tokio::test]
async fn a_claim_is_refused_with_a_sentence_and_nothing_is_written() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let dir = repo("refusals", CHECK_PASSES);
    trust(&c, &addr, &dir).await;
    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "agent": agent, "cwd": dir.to_string_lossy(), "title": "t",
            "prompt": "slow", "worktree": false,
        }),
    )
    .await;
    let id = started["change"]["id"].as_str().unwrap().to_string();
    let run = started["change"]["runs"][0].as_str().unwrap().to_string();
    let key = common::until("the offer", Duration::from_secs(20), || {
        let d = dir.clone();
        async move {
            d.join(".devplane/mcp.json")
                .exists()
                .then(|| offered_key(&d))
        }
    })
    .await;
    let before = written(&c, &addr, &id, &run).await;
    let body = |reason: Value, paths: Value, change: Value, key: &str| serde_json::json!({"reason": reason, "paths": paths, "change": change, "key": key});
    let ok_reason = serde_json::json!("spec says 30, test says 7");
    let cases: Vec<(&str, String, Value, u16, &str)> = vec![
        (
            "a run this host does not drive",
            "acp-nobody".into(),
            body(ok_reason.clone(), serde_json::json!([]), Value::Null, &key),
            404,
            "not_driven",
        ),
        (
            "another change's id",
            run.clone(),
            body(
                ok_reason.clone(),
                serde_json::json!([]),
                "c-other".into(),
                &key,
            ),
            409,
            "wrong_change",
        ),
        (
            "no key",
            run.clone(),
            body(ok_reason.clone(), serde_json::json!([]), Value::Null, ""),
            403,
            "bad_key",
        ),
        (
            "another session's key",
            run.clone(),
            body(
                ok_reason.clone(),
                serde_json::json!([]),
                Value::Null,
                "0".repeat(32).as_str(),
            ),
            403,
            "bad_key",
        ),
        (
            "an empty reason",
            run.clone(),
            body(
                serde_json::json!("   "),
                serde_json::json!([]),
                Value::Null,
                &key,
            ),
            422,
            "empty_reason",
        ),
        (
            "a 1025-byte reason",
            run.clone(),
            body(
                serde_json::json!("a".repeat(1025)),
                serde_json::json!([]),
                Value::Null,
                &key,
            ),
            422,
            "too_large",
        ),
        (
            "eleven paths",
            run.clone(),
            body(
                ok_reason.clone(),
                serde_json::json!((0..11).map(|i| format!("f{i}")).collect::<Vec<_>>()),
                Value::Null,
                &key,
            ),
            422,
            "too_large",
        ),
        (
            "a path out of the project",
            run.clone(),
            body(
                ok_reason.clone(),
                serde_json::json!(["../../etc/passwd"]),
                Value::Null,
                &key,
            ),
            422,
            "path_escapes",
        ),
    ];
    for (what, at, b, status, refused) in cases {
        let (got, v) = claim(&c, &addr, &at, b).await;
        assert_eq!(
            (got, v["refused"].as_str()),
            (status, Some(refused)),
            "{what}: {v}"
        );
        assert_eq!(v["recorded"], false, "{what}");
        assert!(
            v["says"].as_str().is_some_and(|s| s.len() > 20),
            "{what}: {v}"
        );
        assert_eq!(
            written(&c, &addr, &id, &run).await,
            before,
            "{what} wrote something"
        );
    }
    // An argument naming somebody else's run is not a field the route takes.
    let res = c
        .post(format!("http://{addr}/api/runs/{run}/claim"))
        .bearer_auth("tok")
        .json(&serde_json::json!({"reason": "r", "key": key, "run": "acp-x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 422);
    assert!(
        devplane::driven::is_live(&state, &devplane::core::RunId::new(run.clone())).await,
        "a refused claim stopped the run"
    );

    // A run with no change behind it has nothing to disagree with.
    let lone_dir = repo("refusals-lone", CHECK_PASSES);
    trust(&c, &addr, &lone_dir).await;
    let lone = devplane::driven::dispatch(
        &state,
        &echo_spec().unwrap(),
        lone_dir.clone(),
        Some("slow".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let (got, v) = claim(
        &c,
        &addr,
        lone.as_str(),
        body(ok_reason.clone(), serde_json::json!([]), Value::Null, &key),
    )
    .await;
    assert_eq!(
        (got, v["refused"].as_str()),
        (409, Some("no_change")),
        "{v}"
    );

    // Recorded once; the second call from the same run is refused.
    let (got, v) = claim(
        &c,
        &addr,
        &run,
        body(
            ok_reason.clone(),
            serde_json::json!(["tests/a.rs"]),
            id.as_str().into(),
            &key,
        ),
    )
    .await;
    assert_eq!(got, 200, "{v}");
    assert!(
        v["says"]
            .as_str()
            .unwrap()
            .starts_with("Recorded: the agent says"),
        "{v}"
    );
    let (got, v) = claim(
        &c,
        &addr,
        &run,
        body(ok_reason.clone(), serde_json::json!([]), Value::Null, &key),
    )
    .await;
    assert_eq!(got, 409, "{v}");
    assert!(
        matches!(v["refused"].as_str(), Some("already_open" | "stopping")),
        "a second claim from one run: {v}"
    );
    let w = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(w["claim_history"].as_array().map(Vec::len), Some(1), "{w}");

    // A run going back to work on the change closes the claim, and keeps it.
    let claimed = devplane::core::RunId::new(run.clone());
    common::until(
        "the claiming run to stop",
        Duration::from_secs(15),
        || async { (!devplane::driven::is_live(&state, &claimed).await).then_some(()) },
    )
    .await;
    common::until(
        "the change to wait on a person",
        Duration::from_secs(30),
        || async {
            let w = get(&c, &addr, &format!("/api/changes/{id}")).await;
            (w["waiting"]["on"] == "person").then_some(())
        },
    )
    .await;
    let resumed = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/resume"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        resumed["ok"], true,
        "a claiming run cannot be answered: {resumed}"
    );
    let w = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert!(w["stop_claim"].is_null(), "{w}");
    assert_eq!(w["claim_history"][0]["closed_by"], "resumed", "{w}");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&lone_dir).ok();
}

/// The first prompt and a gate-feedback round each end with the sentence
/// that names the tool, once; a person's own follow-up is sent as written.
#[tokio::test]
async fn every_implementation_prompt_names_the_conflict_tool_once() {
    let Some(agent) = echo_agent() else { return };
    let (addr, c, state) = boot().await;
    let dir = repo("prompt", CHECK_FAILS);
    trust(&c, &addr, &dir).await;
    let started = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "agent": agent, "cwd": dir.to_string_lossy(), "title": "t",
            "prompt": "hello", "worktree": false,
        }),
    )
    .await;
    let id = started["change"]["id"].as_str().unwrap().to_string();
    let sentence = devplane::core::context::STOP_AND_SAY;
    assert!(sentence.contains("spec_and_tests_conflict"));
    let w = common::until(
        "the feedback budget to be spent",
        Duration::from_secs(60),
        || async {
            let w = get(&c, &addr, &format!("/api/changes/{id}")).await;
            w["stopped"].is_object().then_some(w)
        },
    )
    .await;
    assert_eq!(w["feedback_rounds"], 1, "{w}");
    let heard = heard(&dir);
    let prompts: Vec<&str> = heard.split("\n---\n").filter(|p| !p.is_empty()).collect();
    assert_eq!(
        prompts.len(),
        2,
        "a first prompt and one feedback round: {heard}"
    );
    for p in &prompts {
        assert_eq!(p.matches(sentence).count(), 1, "{p}");
        assert!(p.trim_end().ends_with(sentence), "{p}");
    }

    // A person's follow-up carries nothing Devplane added.
    let lone_dir = repo("prompt-lone", CHECK_PASSES);
    trust(&c, &addr, &lone_dir).await;
    let lone = devplane::driven::dispatch(
        &state,
        &echo_spec().unwrap(),
        lone_dir.clone(),
        Some("slowish".into()),
        Vec::new(),
    )
    .await
    .unwrap();
    let v = post(
        &c,
        &addr,
        &format!("/api/runs/{lone}/prompt"),
        serde_json::json!({"text": "use the helper"}),
    )
    .await;
    assert_eq!(v["ok"], true, "{v}");
    let heard = common::until("the follow-up to be heard", Duration::from_secs(30), || {
        let d = lone_dir.clone();
        async move {
            let h = std::fs::read_to_string(d.join(".devplane/heard.log")).unwrap_or_default();
            h.contains("use the helper").then_some(h)
        }
    })
    .await;
    let prompts: Vec<&str> = heard.split("\n---\n").filter(|p| !p.is_empty()).collect();
    assert_eq!(prompts.last().copied(), Some("use the helper"), "{heard}");
    assert_eq!(
        heard.matches(sentence).count(),
        1,
        "only the first prompt: {heard}"
    );
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&lone_dir).ok();
}

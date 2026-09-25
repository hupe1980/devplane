//! Reviewing a change against a real repository: the fixture branch touches one
//! file per role, one only by whitespace, and one nobody asked for. Each test
//! reads `/api/changes/{id}/review` from an in-process host.

mod common;
#[path = "common/sandbox.rs"]
mod sandbox;

use devplane::core::Policy;
use devplane::host::{AppState, Shared};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A host for one test; its agents end with it.
struct Host(Shared, #[allow(dead_code)] common::AgentSerial);
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

async fn boot() -> (std::net::SocketAddr, reqwest::Client, Host) {
    let serial = common::one_agent_at_a_time();
    let db = std::env::temp_dir().join(format!("vp-review-{}.db", uuid::Uuid::new_v4().simple()));
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
    (addr, reqwest::Client::new(), Host(state, serial))
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

/// The fixture's branch, adopted as a change.
async fn adopted(c: &reqwest::Client, addr: &std::net::SocketAddr, repo: &Path) -> String {
    let v = post(
        c,
        addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/review", "project": repo.to_string_lossy() }),
    )
    .await;
    assert!(v["error"].is_null(), "{v}");
    v["change_id"].as_str().unwrap().to_string()
}

fn echo_agent() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?.parent()?.join("examples").join("echo_agent");
    bin.exists().then(|| bin.to_string_lossy().to_string())
}

/// Every file in reading order, across the role groups.
fn reading_order(review: &Value) -> Vec<String> {
    review["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["files"].as_array().unwrap().iter())
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect()
}

fn file<'a>(review: &'a Value, path: &str) -> &'a Value {
    review["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["files"].as_array().unwrap().iter())
        .find(|f| f["path"] == path)
        .unwrap_or_else(|| panic!("{path} is not in the review: {review}"))
}

/// Every key and every string value in a body, for the no-aggregate sweep.
fn walk<'a>(v: &'a Value, keys: &mut Vec<&'a str>, strings: &mut Vec<&'a str>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                keys.push(k);
                walk(x, keys, strings);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, keys, strings)),
        Value::String(s) => strings.push(s),
        _ => {}
    }
}

#[tokio::test]
async fn a_change_reads_in_declared_role_order_or_says_it_is_unordered() {
    let repo = sandbox::review_repo("order");
    let (addr, c, _host) = boot().await;
    trust(&c, &addr, &repo).await;
    let id = adopted(&c, &addr, &repo).await;

    // Nothing declared: path order, and the sentence naming the key.
    let r = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    assert_eq!(
        r["unordered"], "unordered — `[review] roles` orders it",
        "{r}"
    );
    let order = reading_order(&r);
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted, "with no roles the order is path order");
    assert_eq!(order.len(), 8, "{order:?}");

    // The whitespace-only hunk is collapsed, counted and flagged.
    assert_eq!(r["formatter_only_collapsed"], 1, "{r}");
    let fmt = file(&r, "src/util/fmt.rs");
    assert_eq!(fmt["hunks"][0]["formatter_only"], true, "{fmt}");
    let flagged = r["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["files"].as_array().unwrap().iter())
        .flat_map(|f| f["hunks"].as_array().unwrap().iter())
        .filter(|h| h["formatter_only"] == true)
        .count();
    assert_eq!(flagged, 1, "the count is the number of flagged hunks");
    assert_eq!(r["formatter_only_says"], "1 formatter-only hunk collapsed");

    // Declared: shared, then logic, then security, first.
    std::fs::write(
        repo.join("devplane.toml"),
        r#"[gates]
check = ["true"]

[review.roles]
shared   = ["src/types/**"]
logic    = ["src/core/**"]
security = ["src/auth/**"]
wiring   = ["src/routes/**"]
tests    = ["tests/**", "*.md"]
"#,
    )
    .unwrap();
    let r = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    assert!(r["unordered"].is_null(), "{r}");
    assert_eq!(
        reading_order(&r)[..3],
        [
            "src/types/user.rs",
            "src/core/rules.rs",
            "src/auth/session.rs"
        ],
        "{r}"
    );
    assert_eq!(file(&r, "src/unrelated.rs")["role"], Value::Null);
    assert_eq!(
        r["shape_says"],
        "8 files · 11+ 7− lines · 1 shared · 1 security · 1 formatter-only hunk collapsed",
        "{r}"
    );

    // No aggregate over evidence, anywhere in the body.
    let (mut keys, mut strings) = (Vec::new(), Vec::new());
    walk(&r, &mut keys, &mut strings);
    for k in &keys {
        for tell in ["score", "confidence", "grade", "risk_score", "coverage_pct"] {
            assert!(!k.contains(tell), "the review carries `{k}`");
        }
    }
    for s in &strings {
        assert!(!s.contains('%'), "a value carries a percentage: {s}");
    }

    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn absent_is_not_no_and_the_standing_is_said_once() {
    let repo = sandbox::review_repo("covers");
    let (addr, c, host) = boot().await;
    trust(&c, &addr, &repo).await;
    let id = adopted(&c, &addr, &repo).await;

    // No mapping: one sentence at the top, and no per-file value at all.
    let r = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    assert_eq!(
        r["coverage_absent"],
        "no test mapping declared — `[review] covers` adds one"
    );
    for path in reading_order(&r) {
        let f = file(&r, &path);
        assert!(
            f["coverage_says"].is_null(),
            "absent rendered as a per-file value: {f}"
        );
        assert!(
            f["marker_commands"][0]
                .as_str()
                .unwrap()
                .starts_with("git diff "),
            "{f}"
        );
    }

    // A mapping: covered by name, and *no declared test* where none matches.
    std::fs::write(
        repo.join("devplane.toml"),
        "[gates]\ncheck = [\"true\"]\n\n[[review.covers]]\ntest = \"tests/auth.rs\"\npaths = [\"src/auth/**\"]\n",
    )
    .unwrap();
    host.record(
        devplane::core::Decision::new(
            devplane::core::Authority::Rule,
            "agent:tool.use",
            "Edit src/auth/session.rs",
            "allow",
        )
        .for_change(&devplane::core::ChangeId::new(&id)),
    )
    .await;
    let r = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    assert!(r["coverage_absent"].is_null(), "{r}");
    let session = file(&r, "src/auth/session.rs");
    assert_eq!(session["coverage_says"], "covered by `tests/auth.rs`");
    assert_eq!(
        session["marker_commands"][1], "tests/auth.rs",
        "no gate names the test, so the marker is the test itself: {session}"
    );
    assert_eq!(
        file(&r, "src/routes/login.rs")["coverage_says"],
        "no declared test covers this path"
    );
    // The decision sits on the file its subject names, and only there.
    assert_eq!(
        session["decisions"].as_array().unwrap().len(),
        1,
        "{session}"
    );
    for path in reading_order(&r) {
        if path != "src/auth/session.rs" {
            assert!(
                file(&r, &path)["decisions"].as_array().unwrap().is_empty(),
                "{path} carries a decision about another file"
            );
        }
    }

    // The standing is said once, at the top, and no hunk carries a gate.
    let says = r["standing_says"].as_str().unwrap().to_string();
    assert_eq!(says, "`check` has not run against this change");
    let body = r.to_string();
    assert_eq!(body.matches(&says).count(), 1, "{body}");
    let (mut keys, mut strings) = (Vec::new(), Vec::new());
    walk(&r["groups"], &mut keys, &mut strings);
    assert!(
        !keys
            .iter()
            .any(|k| k.contains("gate") || k.contains("standing")),
        "a file or hunk carries a gate field"
    );

    // The change states its size and nothing about time.
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    let shape = row["shape_says"].as_str().unwrap_or_default().to_string();
    assert!(shape.starts_with("8 files · "), "{row}");
    assert!(
        !shape.contains("min") && !shape.contains("estimated"),
        "{shape}"
    );
    let bytes = shape.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if b.is_ascii_digit() {
            assert!(
                !shape[i + 1..].starts_with("m "),
                "a duration in the shape: {shape}"
            );
        }
    }

    std::fs::remove_dir_all(&repo).ok();
}

/// Waits until a person is asked to look, or the change stops.
async fn settled(c: &reqwest::Client, addr: &std::net::SocketAddr, id: &str) -> Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let w = get(c, addr, &format!("/api/changes/{id}")).await;
        if w["waiting"]["on"] == "person" || w["stopped"].is_object() {
            return w;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the change never settled: {w}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn a_file_nobody_asked_for_is_under_its_own_heading() {
    let Some(agent) = echo_agent() else { return };
    let repo = sandbox::review_repo("intent");
    let (addr, c, host) = boot().await;
    trust(&c, &addr, &repo).await;

    let v = post(
        &c,
        &addr,
        "/api/changes",
        serde_json::json!({
            "cwd": repo.to_string_lossy(),
            "title": "open sessions",
            "prompt": "write-file src/session_open.rs",
            "agent": agent,
            "spec": "specs/001-review",
            "tasks": ["FR-001"],
        }),
    )
    .await;
    assert!(v.get("error").is_none(), "{v}");
    let id = v["change_id"].as_str().unwrap().to_string();
    let w = settled(&c, &addr, &id).await;
    let worktree = PathBuf::from(w["worktree"].as_str().unwrap());
    assert!(
        worktree.join("src/session_open.rs").is_file(),
        "the agent wrote nothing"
    );
    // And a person edits a file by hand in the same checkout.
    std::fs::write(worktree.join("src/by_hand.rs"), "pub fn nobody() {}\n").unwrap();

    let r = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    let intent = &r["intent"];
    assert_eq!(intent["heading"], "grouped by the run that wrote each file");
    assert!(intent["unavailable"].is_null(), "{intent}");
    let groups = intent["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "{intent}");
    assert_eq!(
        groups[0]["files"],
        serde_json::json!(["src/session_open.rs"])
    );
    let tasks: Vec<&str> = groups[0]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        tasks,
        [
            "T001 Open a session (FR-001)",
            "T002 Close a session (FR-001)"
        ]
    );
    assert!(
        groups[0]["title"]
            .as_str()
            .unwrap()
            .contains("T001 Open a session (FR-001)")
    );
    assert_eq!(
        intent["not_asked_for"],
        serde_json::json!([{
            "path": "src/by_hand.rs",
            "why": {"why": "no_run_wrote"},
            "says": "no run Devplane dispatched wrote this",
        }]),
        "{intent}"
    );
    assert_eq!(intent["not_asked_for_heading"], "not asked for");
    // Per file, the task it belongs to — or why it belongs to none.
    assert!(
        file(&r, "src/session_open.rs")["task_says"]
            .as_str()
            .unwrap()
            .contains("T001 Open a session")
    );
    assert_eq!(
        file(&r, "src/by_hand.rs")["task_says"],
        "not asked for — no run Devplane dispatched wrote this"
    );

    // A change whose only run was observed, not started, cannot be grouped and says so.
    let other = adopted(&c, &addr, &repo).await;
    let payload: devplane::observe::hook::HookPayload = serde_json::from_value(serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": "s-watched",
        "cwd": repo.to_string_lossy(),
        "source": "startup",
    }))
    .unwrap();
    devplane::record::observe(&host.store, &payload, devplane::core::Source::Hook)
        .await
        .unwrap();
    devplane::poller::tail_once(&host).await.unwrap();
    host.changes
        .lock()
        .await
        .get_mut(&devplane::core::ChangeId::new(&other))
        .expect("the adopted change")
        .runs
        .push(devplane::core::RunId::new("s-watched"));
    let r = get(&c, &addr, &format!("/api/changes/{other}/review")).await;
    assert_eq!(
        r["intent"]["unavailable"],
        "every run on this change was watched, not dispatched by Devplane, so grouping by task \
         is unavailable",
        "{r}"
    );
    assert!(r["intent"]["groups"].as_array().unwrap().is_empty());
    assert!(r["intent"]["not_asked_for"].as_array().unwrap().is_empty());

    std::fs::remove_dir_all(&repo).ok();
}

/// The review module divides nothing and names no figure over evidence.
#[test]
fn the_review_computes_no_percentage_ratio_or_score() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/review.rs");
    let source = std::fs::read_to_string(&path).expect("src/core/review.rs is the review");
    assert!(source.contains("pub fn by_intent"), "not the review module");
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut hits = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        // The tests below the module plant the words they refuse.
        if code.contains("#[cfg(test)]") {
            break;
        }
        if code.contains('%') {
            hits.push(format!("{}: `%`", n + 1));
        }
        if code.contains(" / ") || code.contains("as f32") || code.contains("as f64") {
            hits.push(format!("{}: a division — {}", n + 1, code.trim()));
        }
        for word in [
            "percent",
            "percentage",
            "ratio",
            "score",
            "grade",
            "confidence",
        ] {
            let lower = code.to_lowercase();
            let mut from = 0;
            while let Some(i) = lower[from..].find(word) {
                let at = from + i;
                let before = lower[..at].chars().next_back().is_none_or(|c| !is_word(c));
                let after = lower[at + word.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_word(c));
                if before && after {
                    hits.push(format!("{}: `{word}` — {}", n + 1, code.trim()));
                }
                from = at + word.len();
            }
        }
    }
    assert!(hits.is_empty(), "the review computes a figure: {hits:?}");
}

/// A weakened check (skip marker, deleted test, edited gate definition) is read
/// first, with its reason, and not repeated under its role.
#[tokio::test]
async fn checks_weakened_or_changed_come_first() {
    let repo = sandbox::review_repo("weakened");
    let (addr, c, _host) = boot().await;
    trust(&c, &addr, &repo).await;
    let v = post(
        &c,
        &addr,
        "/api/changes/adopt",
        serde_json::json!({ "branch": "feat/review", "project": repo.to_string_lossy() }),
    )
    .await;
    let id = v["change_id"].as_str().unwrap().to_string();
    let wt = PathBuf::from(v["change"]["worktree"].as_str().unwrap());

    std::fs::remove_file(wt.join("tests/auth.rs")).unwrap();
    std::fs::write(
        wt.join("tests/login.rs"),
        "#[test]\n#[ignore]\nfn refuses_a_bad_password() {}\n",
    )
    .unwrap();
    std::fs::write(wt.join("devplane.toml"), "[gates]\ncheck = []\n").unwrap();

    let r = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    let first = &r["groups"][0];
    assert_eq!(first["says"], "checks weakened or changed", "{r}");
    assert!(first["role"].is_null());
    let weak = first["weakened"].as_array().expect("the reasons");
    let has = |path: &str, why: &str| {
        weak.iter()
            .any(|w| w["path"] == path && w["why"].as_str().unwrap().contains(why))
    };
    assert!(has("tests/auth.rs", "deletes a test file"), "{weak:?}");
    assert!(has("tests/login.rs", "#[ignore"), "{weak:?}");
    assert!(has("devplane.toml", "gates' own definition"), "{weak:?}");
    let order = reading_order(&r);
    for path in ["tests/auth.rs", "tests/login.rs", "devplane.toml"] {
        assert_eq!(
            order.iter().filter(|p| *p == path).count(),
            1,
            "{path} is read once: {order:?}"
        );
    }
    // Nothing else is listed there.
    assert_eq!(first["files"].as_array().unwrap().len(), 3, "{first}");

    // The certificate says the same in one sentence.
    let cert = get(&c, &addr, &format!("/api/changes/{id}/certificate")).await;
    let md = cert["markdown"].as_str().unwrap();
    assert!(
        md.contains("This change itself altered the checks it was verified by"),
        "{md}"
    );
    std::fs::remove_dir_all(&repo).ok();
}

/// A change that alters no check has no such group: absent, not empty.
#[tokio::test]
async fn a_change_that_alters_no_check_has_no_such_heading() {
    let repo = sandbox::review_repo("unweakened");
    let (addr, c, _host) = boot().await;
    trust(&c, &addr, &repo).await;
    let id = adopted(&c, &addr, &repo).await;
    let r = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    for g in r["groups"].as_array().unwrap() {
        assert_ne!(g["says"], "checks weakened or changed", "{g}");
        assert!(g.get("weakened").is_none(), "{g}");
    }
    std::fs::remove_dir_all(&repo).ok();
}

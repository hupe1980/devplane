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

// ── What a change weakened, read from its diff alone ────────────────────────

mod weakening {
    use devplane::core::diff::parse;
    use devplane::core::review::{Qualifier, WeakKind, Weakened, weakened};
    use std::collections::HashSet;

    /// One modified file's diff: `lines` are `' '`, `'+'` or `'-'` prefixed.
    fn modified(path: &str, lines: &[&str]) -> String {
        let old = lines.iter().filter(|l| !l.starts_with('+')).count();
        let new = lines.iter().filter(|l| !l.starts_with('-')).count();
        format!(
            "diff --git a/{path} b/{path}\nindex 1111111..2222222 100644\n--- a/{path}\n+++ b/{path}\n@@ -1,{old} +1,{new} @@\n{}\n",
            lines.join("\n")
        )
    }

    fn deleted(path: &str, lines: &[&str]) -> String {
        format!(
            "diff --git a/{path} b/{path}\ndeleted file mode 100644\nindex 1111111..0000000\n--- a/{path}\n+++ /dev/null\n@@ -1,{} +0,0 @@\n{}\n",
            lines.len(),
            lines
                .iter()
                .map(|l| format!("-{l}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    fn rows(diff: &str) -> Vec<Weakened> {
        rows_with(diff, &[])
    }

    fn rows_with(diff: &str, gates: &[&str]) -> Vec<Weakened> {
        let set = parse("main", diff, "git diff");
        let gates: Vec<String> = gates.iter().map(|g| g.to_string()).collect();
        weakened(&set, None, &gates)
    }

    fn has(rows: &[Weakened], kind: WeakKind, why: &str) -> bool {
        rows.iter().any(|w| w.kind == kind && w.why.contains(why))
    }

    // The qualifier: counts by kind, and nothing when nothing was weakened.

    #[test]
    fn a_skip_marker_is_one_check_weakened() {
        let r = rows(&modified(
            "tests/login.test.ts",
            &[
                " test(\"login\", () => {});",
                "+it.skip(\"rejects the sixth\", () => {});",
            ],
        ));
        let q = Qualifier::of(&r, &HashSet::new());
        assert_eq!(
            (q.weakened, q.deleted, q.gates_changed, q.unseen),
            (1, 0, 0, 1),
            "{r:?}"
        );
        assert_eq!(q.says, "1 check weakened");
        assert_eq!(r[0].matched, "it.skip(\"rejects the sixth\", () => {});");
    }

    #[test]
    fn a_deleted_test_and_an_edited_workflow_count_by_kind() {
        let diff = deleted("tests/auth.rs", &["#[test]", "fn a() {}"])
            + &modified(
                ".github/workflows/ci.yml",
                &[" jobs:", "-  x: 1", "+  x: 2"],
            );
        let q = Qualifier::of(&rows(&diff), &HashSet::new());
        assert_eq!((q.weakened, q.deleted, q.gates_changed), (0, 1, 1));
        assert_eq!(q.says, "1 test deleted · 1 gate changed");
    }

    #[test]
    fn nothing_weakened_says_nothing() {
        let r = rows(&modified("src/lib.rs", &[" fn a() {}", "+fn b() {}"]));
        let q = Qualifier::of(&r, &HashSet::new());
        assert!(q.is_empty() && q.says.is_empty() && q.unseen == 0, "{q:?}");
    }

    /// A marker already in the base is context, never the change's own.
    #[test]
    fn a_marker_in_the_base_is_not_the_changes() {
        let r = rows(&modified(
            "tests/login.test.ts",
            &[" it.skip(\"old\", () => {});", "+test(\"new\", () => {});"],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    /// A seen mark is keyed by what the row matched, so a different marker is
    /// a different, unseen row.
    #[test]
    fn a_seen_mark_lapses_when_the_matched_text_changes() {
        let before = rows(&modified(
            "tests/a.test.ts",
            &["+it.skip(\"a\", () => {});"],
        ));
        let seen: HashSet<(String, String)> = before
            .iter()
            .map(|w| (w.path.clone(), w.matched.clone()))
            .collect();
        assert_eq!(Qualifier::of(&before, &seen).unseen, 0);
        let after = rows(&modified(
            "tests/a.test.ts",
            &["+it.skip(\"b\", () => {});"],
        ));
        assert_eq!(Qualifier::of(&after, &seen).unseen, 1);
    }

    // Removed assertions, in test files only.

    #[test]
    fn a_removed_assertion_with_none_in_its_place_is_named() {
        let r = rows(&modified(
            "tests/test_limit.py",
            &[
                " def test_limit():",
                "     r = limit()",
                "-    assert r == 5",
            ],
        ));
        assert!(has(&r, WeakKind::Skip, "removes an assertion"), "{r:?}");
        assert_eq!(r[0].matched, "assert r == 5");
        for (path, line) in [
            ("tests/a.rs", "-    assert_eq!(x, 1);"),
            ("tests/a.test.ts", "-  expect(x).toBe(1);"),
            ("pkg/a_test.go", "-\tt.Fatalf(\"bad\")"),
            ("tests/test_b.py", "-        self.assertEqual(a, b)"),
        ] {
            let r = rows(&modified(path, &[" fn a() {", line]));
            assert!(
                has(&r, WeakKind::Skip, "removes an assertion"),
                "{path}: {r:?}"
            );
        }
    }

    #[test]
    fn a_replaced_or_moved_assertion_is_not_removed() {
        let r = rows(&modified(
            "tests/test_limit.py",
            &[
                " def test_limit():",
                "-    assert r == 5",
                "+    assert r == 6",
            ],
        ));
        assert!(r.is_empty(), "{r:?}");
        // Outside the tests, an assertion is code, not a check.
        let r = rows(&modified(
            "src/limit.py",
            &[" def f():", "-    assert r == 5"],
        ));
        assert!(r.is_empty(), "{r:?}");
        // Rust's `.expect("…")` asserts nothing.
        let r = rows(&modified(
            "tests/a.rs",
            &[" fn a() {", "-    let x = y.expect(\"y\");"],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    // A test removed from a file that stays.

    #[test]
    fn a_test_removed_from_a_kept_file_is_a_deleted_test() {
        let r = rows(&modified(
            "tests/test_limit.py",
            &[" import limit", "-def test_sixth():", "-    pass"],
        ));
        assert!(
            has(&r, WeakKind::Deleted, "removes a test and keeps the file"),
            "{r:?}"
        );
        assert_eq!(Qualifier::of(&r, &HashSet::new()).says, "1 test deleted");
    }

    #[test]
    fn a_renamed_test_is_not_a_deleted_one() {
        let r = rows(&modified(
            "tests/test_limit.py",
            &[
                " import limit",
                "-def test_sixth():",
                "+def test_the_sixth():",
                "     pass",
            ],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    // Tolerances.

    #[test]
    fn a_changed_tolerance_is_named() {
        let r = rows(&modified(
            "tests/test_rate.py",
            &[
                " def test_rate():",
                "-    assert rate() == approx(0.5, rel=1e-6)",
                "+    assert rate() == approx(0.5, rel=0.5)",
            ],
        ));
        assert!(has(&r, WeakKind::Skip, "changes a tolerance"), "{r:?}");
    }

    #[test]
    fn a_new_tolerance_in_a_new_test_is_not_a_change() {
        let r = rows(&modified(
            "tests/test_rate.py",
            &[
                " import rate",
                "+def test_new():",
                "+    assert rate() == approx(0.5, rel=1e-6)",
            ],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    // Assertions that cannot fail.

    #[test]
    fn an_assertion_weakened_to_true_is_named() {
        let r = rows(&modified(
            "tests/test_rate.py",
            &[
                " def test_rate():",
                "-    assert rate() == 5",
                "+    assert True",
            ],
        ));
        assert!(has(&r, WeakKind::Skip, "cannot fail"), "{r:?}");
        assert!(!has(&r, WeakKind::Skip, "removes an assertion"), "{r:?}");
        let r = rows(&modified(
            "tests/a.test.ts",
            &[" it(\"a\", () => {", "+  expect(true).toBe(true);"],
        ));
        assert!(has(&r, WeakKind::Skip, "cannot fail"), "{r:?}");
    }

    // Tests added with nothing in them that checks.

    #[test]
    fn a_new_test_with_no_assertion_is_named() {
        for (path, lines) in [
            (
                "tests/test_rate.py",
                &[" import rate", "+def test_rate():", "+    rate()"][..],
            ),
            (
                "tests/limits.rs",
                &[
                    " use limits;",
                    "+#[test]",
                    "+fn parses() {",
                    "+    let _ = limits::parse(\"5\");",
                    "+}",
                ][..],
            ),
            (
                "src/a.test.ts",
                &[
                    " import { a } from './a';",
                    "+it(\"runs\", () => {",
                    "+  a();",
                    "+});",
                ][..],
            ),
        ] {
            let r = rows(&modified(path, lines));
            assert!(has(&r, WeakKind::Skip, "no assertion"), "{path}: {r:?}");
        }
    }

    #[test]
    fn a_new_test_that_checks_anything_is_not_named() {
        for lines in [
            &[
                " import rate",
                "+def test_rate():",
                "+    assert rate() == 5",
            ][..],
            &[
                " import rate",
                "+def test_rate():",
                "+    check_rate(rate())",
            ][..],
            &[
                " import rate",
                "+def test_rate():",
                "+    with pytest.raises(ValueError):",
                "+        rate(-1)",
            ][..],
            // A rename: the body was already there.
            &[" import rate", "+def test_the_rate():", "     rate()"][..],
        ] {
            let r = rows(&modified("tests/test_rate.py", lines));
            assert!(r.is_empty(), "{lines:?}: {r:?}");
        }
        let r = rows(&modified(
            "tests/limits.rs",
            &[
                " use limits;",
                "+#[test]",
                "+#[should_panic]",
                "+fn rejects() {",
                "+    limits::parse(\"x\");",
                "+}",
            ],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn an_assertion_about_a_true_value_is_not_trivial() {
        let r = rows(&modified(
            "tests/test_rate.py",
            &[
                " def test_rate():",
                "+    assert rate() is True",
                "+    assert True_ish()",
            ],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    // Suppressions, anywhere but prose.

    #[test]
    fn a_lint_or_type_suppression_is_named_in_any_file() {
        for (path, line) in [
            ("src/a.ts", "+// @ts-ignore"),
            ("src/b.ts", "+/* eslint-disable no-console */"),
            ("src/c.py", "+import os  # noqa"),
            ("src/d.py", "+x = f()  # type: ignore"),
            ("src/e.rs", "+#[allow(dead_code)]"),
            ("pkg/f.go", "+x := 1 //nolint"),
        ] {
            let r = rows(&modified(path, &[" a", line]));
            assert!(has(&r, WeakKind::Skip, "suppression"), "{path}: {r:?}");
        }
    }

    #[test]
    fn a_suppression_named_in_documentation_is_not_one() {
        let r = rows(&modified(
            "README.md",
            &[" # Lint", "+Never add `@ts-ignore` or `# noqa`."],
        ));
        assert!(r.is_empty(), "{r:?}");
        // Nor is one already in the base.
        let r = rows(&modified("src/a.ts", &[" // @ts-ignore", "+const x = 1;"]));
        assert!(r.is_empty(), "{r:?}");
    }

    // What defines the checks.

    #[test]
    fn an_edit_to_a_check_defining_file_is_a_gate_changed() {
        for path in [
            "justfile",
            "Makefile",
            ".pre-commit-config.yaml",
            "tox.ini",
            "setup.cfg",
            "pytest.ini",
            "clippy.toml",
            "deny.toml",
            "rustfmt.toml",
            ".eslintrc.json",
            "eslint.config.js",
            "tsconfig.json",
            "ui/tsconfig.app.json",
            "codecov.yml",
            ".coveragerc",
        ] {
            let r = rows(&modified(path, &[" a", "-b", "+c"]));
            assert!(r.iter().any(|w| w.kind == WeakKind::Gate), "{path}: {r:?}");
        }
    }

    #[test]
    fn an_ordinary_configuration_file_is_not_a_gate() {
        for path in [
            "src/config.json",
            "tsconfig.md",
            "docs/Makefile.md",
            "src/setup.py",
        ] {
            let r = rows(&modified(path, &[" a", "-b", "+c"]));
            assert!(r.is_empty(), "{path}: {r:?}");
        }
    }

    /// The script a declared gate runs defines passing as much as the gate.
    #[test]
    fn a_file_a_gate_command_names_is_a_gate() {
        let diff = modified("scripts/test.sh", &[" #!/bin/sh", "-npm test", "+exit 0"]);
        let r = rows_with(&diff, &["sh ./scripts/test.sh", "cargo test --locked"]);
        assert!(
            has(&r, WeakKind::Gate, "which a declared gate runs"),
            "{r:?}"
        );
        // Without that gate, it is an ordinary script.
        assert!(rows_with(&diff, &["cargo test"]).is_empty());
    }

    // Masked failures, where the checks are defined.

    #[test]
    fn a_masked_failure_in_ci_is_named() {
        for line in [
            "+      - run: cargo test || true",
            "+        continue-on-error: true",
            "+      - run: pytest -k \"not slow\"",
            "+      - run: cargo test -- --skip login",
        ] {
            let r = rows(&modified(".github/workflows/ci.yml", &[" jobs:", line]));
            assert!(
                has(&r, WeakKind::Gate, "masks a failing step"),
                "{line}: {r:?}"
            );
        }
        let r = rows(&modified(
            "justfile",
            &[" check:", "+    cargo clippy || true"],
        ));
        assert!(has(&r, WeakKind::Gate, "masks a failing step"), "{r:?}");
    }

    #[test]
    fn a_masked_step_in_an_ordinary_script_is_not_a_gate() {
        let r = rows(&modified(
            "scripts/deploy.sh",
            &[" #!/bin/sh", "+rm -f cache || true"],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    fn renamed(from: &str, to: &str) -> String {
        format!(
            "diff --git a/{from} b/{to}\nsimilarity index 100%\nrename from {from}\nrename to {to}\n"
        )
    }

    /// A test file renamed where the runner no longer collects it is a
    /// deleted test, and a renamed runner configuration a changed gate.
    #[test]
    fn a_rename_that_hides_a_test_or_a_gate_is_named() {
        for (from, to) in [
            ("tests/test_login.py", "attic/test_login.py.off"),
            ("tests/test_login.py", "tests/login.py"),
            ("src/login.test.ts", "src/login.ts.bak"),
        ] {
            let r = rows(&renamed(from, to));
            assert!(
                has(
                    &r,
                    WeakKind::Deleted,
                    "moves a test file out of the test suite"
                ),
                "{from} → {to}: {r:?}"
            );
        }
        let r = rows(&renamed("conftest.py", "conftest.py.bak"));
        assert!(has(&r, WeakKind::Gate, "renames `conftest.py`"), "{r:?}");
        let r = rows_with(
            &renamed("scripts/test.sh", "scripts/test.sh.old"),
            &["sh ./scripts/test.sh"],
        );
        assert!(
            has(&r, WeakKind::Gate, "renames `scripts/test.sh`"),
            "{r:?}"
        );
        // A test moved within the suite, and an ordinary file renamed, are not.
        assert!(rows(&renamed("tests/test_a.py", "tests/unit/test_a.py")).is_empty());
        assert!(rows(&renamed("src/a.rs", "src/b.rs")).is_empty());
    }

    /// An ignored file is outside the digest and the review; a pattern added
    /// to `.gitignore` is said, a comment or an exception is not.
    #[test]
    fn an_added_ignore_pattern_is_a_gate_changed() {
        let r = rows(&modified(
            ".gitignore",
            &[" target/", "+tests/secret_test.py"],
        ));
        assert!(has(&r, WeakKind::Gate, "adds an ignore pattern"), "{r:?}");
        let r = rows(&modified(
            ".gitignore",
            &[" target/", "+# build output", "+!keep.txt", "-old/"],
        ));
        assert!(r.is_empty(), "{r:?}");
    }

    /// `.skip(` is every Rust iterator: only whole spellings count.
    #[test]
    fn an_iterator_skip_is_not_a_skip_marker() {
        let r = rows(&modified(
            "tests/a.rs",
            &[" fn a() {", "+    let b = v.iter().skip(1);"],
        ));
        assert!(r.is_empty(), "{r:?}");
    }
}

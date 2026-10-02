//! Base checks against a real repository: a pytest-shaped fake runner whose
//! outcome is read from the tree it runs in, an agent that loosens a test and
//! filters the run, and the pair of runs in one throwaway checkout.

mod common;

use devplane::core::Policy;
use devplane::host::{AppState, Shared};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// A runner shaped like pytest. Each `def test_x(): assert "TOKEN" in "FILE"`
/// passes when FILE holds TOKEN; `assert True` always passes. `FILTER` keeps
/// only the tests whose names contain it, as `-k` does. It prints pytest's
/// summary, its failing lines, and JUnit to `reports/junit.xml`, and appends
/// one line per run to a counter outside the repository.
const RUNNER: &str = r#"#!/bin/sh
FILTER=""
echo ran >> "COUNTER"
case "$(pwd)" in *devplane-base-*) [ -f "COUNTER.sleep" ] && sleep 3 ;; esac
mkdir -p reports
awk -v filter="$FILTER" '
/^def test_/ {
  name = $2; sub(/\(.*/, "", name)
  if (filter != "" && index(name, filter) == 0) { desel++; next }
  ok = 1
  if (match($0, /assert "[^"]*" in "[^"]*"/)) {
    s = substr($0, RSTART, RLENGTH); split(s, q, "\"")
    ok = (system("grep -q \"" q[2] "\" \"" q[4] "\" 2>/dev/null") == 0)
  } else if ($0 ~ /assert False/) ok = 0
  cls = FILENAME; sub(/\.py$/, "", cls); gsub("/", ".", cls)
  if (ok) { pass++; xml = xml "<testcase classname=\"" cls "\" name=\"" name "\"/>" }
  else { fail++; xml = xml "<testcase classname=\"" cls "\" name=\"" name "\"><failure/></testcase>"; print "FAILED " FILENAME "::" name " - assert" }
}
END {
  printf "<testsuites><testsuite name=\"pytest\">%s</testsuite></testsuites>\n", xml > "reports/junit.xml"
  out = ""
  if (fail) out = out (out ? ", " : "") fail " failed"
  if (pass) out = out (out ? ", " : "") pass " passed"
  if (desel) out = out (out ? ", " : "") desel " deselected"
  if (out == "") out = "no tests ran"
  print "============ " out " in 0.01s ============"
  exit (fail ? 1 : 0)
}' tests/*.py
"#;

/// Twelve tests; `test_lockout` needs the lockout in `src/login.py`.
fn twelve() -> String {
    let mut t = String::from("def test_lockout(): assert \"lockout\" in \"src/login.py\"\n");
    for n in [
        "login",
        "login_twice",
        "reset",
        "a",
        "b",
        "c",
        "d",
        "e",
        "f",
        "g",
        "h",
    ] {
        t.push_str(&format!(
            "def test_{n}(): assert \"def\" in \"src/login.py\"\n"
        ));
    }
    t
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_out(dir: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(
        &std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git")
            .stdout,
    )
    .into_owned()
}

struct Repo {
    root: PathBuf,
    counter: PathBuf,
}

impl Repo {
    fn runs(&self) -> usize {
        std::fs::read_to_string(&self.counter)
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
        std::fs::remove_file(&self.counter).ok();
    }
}

/// `main` with the runner, twelve tests and `extra` config; then a branch
/// `feat/work` on which `edit` runs and is committed.
fn repo(tag: &str, extra: &str, edit: impl FnOnce(&Path)) -> Repo {
    repo_with(tag, extra, "", edit)
}

/// [`repo`], with `workspace` as the `[workspace]` table's body.
fn repo_with(tag: &str, extra: &str, workspace: &str, edit: impl FnOnce(&Path)) -> Repo {
    let id = uuid::Uuid::new_v4().simple().to_string();
    let root = std::env::temp_dir().join(format!("vp-base-{tag}-{id}"));
    let counter = std::env::temp_dir().join(format!("vp-base-{tag}-{id}.count"));
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.email", "t@example.com"]);
    git(&root, &["config", "user.name", "Test"]);
    std::fs::write(
        root.join("devplane.toml"),
        format!(
            "[gates]\ncheck = [\"sh ./scripts/test.sh\"]\ntimeout = \"60s\"\nmax_feedback_rounds = 0\n\
             junit = [\"reports/junit.xml\"]\n{extra}\n[workspace]\n{workspace}\n[review.roles]\ntests = [\"tests/**\"]\n"
        ),
    )
    .unwrap();
    std::fs::write(root.join(".gitignore"), "reports/\n").unwrap();
    std::fs::write(
        root.join("scripts/test.sh"),
        RUNNER.replace("COUNTER", &counter.to_string_lossy()),
    )
    .unwrap();
    std::fs::write(root.join("tests/test_login.py"), twelve()).unwrap();
    std::fs::write(root.join("src/login.py"), "def login(): pass\n# lockout\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "init"]);
    git(&root, &["checkout", "-qb", "feat/work"]);
    edit(&root);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "the work", "--allow-empty"]);
    git(&root, &["checkout", "-q", "main"]);
    Repo {
        root: root.canonicalize().unwrap(),
        counter,
    }
}

/// The agent drops the lockout, loosens the test that would catch it, and
/// filters the run to the three tests named `lo…`.
fn speed_up_the_login_tests(root: &Path) {
    std::fs::write(root.join("src/login.py"), "def login(): pass\n").unwrap();
    let tests = twelve().replace(
        "def test_lockout(): assert \"lockout\" in \"src/login.py\"",
        "def test_lockout(): assert True",
    );
    std::fs::write(root.join("tests/test_login.py"), tests).unwrap();
    let script = std::fs::read_to_string(root.join("scripts/test.sh")).unwrap();
    std::fs::write(
        root.join("scripts/test.sh"),
        script.replace("FILTER=\"\"", "FILTER=\"lo\""),
    )
    .unwrap();
}

struct Host(Shared);

async fn boot() -> (std::net::SocketAddr, reqwest::Client, Host) {
    let db = std::env::temp_dir().join(format!("vp-base-{}.db", uuid::Uuid::new_v4().simple()));
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

async fn post(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    path: &str,
    body: Value,
) -> (u16, Value) {
    let r = c
        .post(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = r.status().as_u16();
    (status, r.json().await.unwrap_or(Value::Null))
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

/// Trusts the repository and adopts `feat/work`; the change's id and worktree.
async fn adopt(c: &reqwest::Client, addr: &std::net::SocketAddr, repo: &Repo) -> (String, PathBuf) {
    let (_, t) = post(
        c,
        addr,
        "/api/projects/trust",
        json!({ "path": repo.root.to_string_lossy() }),
    )
    .await;
    assert_eq!(t["trusted"], true, "{t}");
    let (_, adopted) = post(
        c,
        addr,
        "/api/changes/adopt",
        json!({ "branch": "feat/work", "project": repo.root.to_string_lossy() }),
    )
    .await;
    assert!(adopted["error"].is_null(), "{adopted}");
    (
        adopted["change_id"].as_str().unwrap().to_string(),
        PathBuf::from(adopted["change"]["worktree"].as_str().unwrap()),
    )
}

fn throwaway_checkouts(root: &Path) -> Vec<String> {
    git_out(root, &["worktree", "list", "--porcelain"])
        .lines()
        .filter(|l| l.contains(".devplane-base-"))
        .map(String::from)
        .collect()
}

/// State and standing, and the same with every count and result stripped.
async fn state_with_and_without(state: &Shared, id: &str, checks: &[String]) -> (String, String) {
    let record = state
        .changes
        .lock()
        .await
        .get(&devplane::core::ChangeId::new(id.to_string()))
        .cloned()
        .unwrap();
    let mut bare = record.clone();
    bare.base_checks = None;
    for g in &mut bare.gates {
        g.junit.clear();
        g.base_commit = None;
        for c in &mut g.commands {
            c.tests = Default::default();
        }
    }
    let declared = devplane::core::change::Declared::Checks(checks);
    let now = record.tree_now.as_ref();
    let with = format!(
        "{:?} {:?}",
        record.state(declared, now),
        record.verdict(declared, now)
    );
    let without = format!(
        "{:?} {:?}",
        bare.state(declared, now),
        bare.verdict(declared, now)
    );
    (with, without)
}

#[tokio::test]
async fn a_count_sits_beside_verified_and_changes_nothing() {
    let repo = repo("count", "", |_| {});
    let (addr, c, host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    let (_, verdict) = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(verdict["passed"], true, "{verdict}");
    let tests = &verdict["report"]["commands"][0]["tests"];
    assert_eq!(tests["count"], "read", "{tests}");
    assert_eq!(tests["tallies"][0]["runner"], "pytest");
    assert_eq!(tests["tallies"][0]["ran"], 12);
    assert!(verdict["report"]["base_commit"].is_string(), "{verdict}");

    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(row["state"], "verified", "{row}");
    assert_eq!(
        row["base_checks"]["counts_says"],
        "ran 12 · base checks not run — they run when you offer, or press Run base checks"
    );
    assert_eq!(row["base_checks"]["can_run"], true);
    assert_eq!(repo.runs(), 1, "nothing runs after check by default");

    let (with, without) =
        state_with_and_without(&host.0, &id, &["sh ./scripts/test.sh".to_string()]).await;
    assert_eq!(with, without);
    assert!(with.starts_with("Verified"), "{with}");
}

#[tokio::test]
async fn a_base_test_that_fails_on_the_change_is_named_and_holds_the_offer() {
    let repo = repo("lockout", "", speed_up_the_login_tests);
    let (addr, c, host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;

    let (_, v) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        json!({ "base_checks": true }),
    )
    .await;
    assert_eq!(v["passed"], true, "check passes with its own filter: {v}");
    let b = &v["base_checks"];
    assert_eq!(
        b["on_change"]["says"], "1 base test fails on this change",
        "{b}"
    );
    assert_eq!(
        b["on_change"]["restored"],
        json!(["scripts/test.sh", "tests/test_login.py"])
    );
    assert_eq!(b["on_change"]["weight"], "warn");
    assert!(
        b["counts_says"].as_str().unwrap().starts_with("ran 12 → 3"),
        "{}",
        b["counts_says"]
    );
    assert!(
        b["counts_says"]
            .as_str()
            .unwrap()
            .contains("deselected 0 → 9")
    );
    let named = &b["on_change"]["named"];
    assert_eq!(
        named["not_run_on_change"].as_array().unwrap().len(),
        9,
        "{named}"
    );
    assert_eq!(
        b["on_change"]["failing"][0],
        "pytest::tests.test_login::test_lockout"
    );
    assert_eq!(b["on_base"]["weight"], "plain", "{}", b["on_base"]);
    assert_eq!(repo.runs(), 3, "check, then one pair of two runs");

    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(
        row["state"], "verified",
        "verified keeps its definition: {row}"
    );
    assert_eq!(row["qualifier"]["base_tests"], 1, "{}", row["qualifier"]);
    assert!(
        row["qualifier"]["says"]
            .as_str()
            .unwrap()
            .contains("1 base test fails on this change")
    );
    let (with, without) =
        state_with_and_without(&host.0, &id, &["sh ./scripts/test.sh".to_string()]).await;
    assert_eq!(with, without);

    // The offer holds until every row is seen, the base test's included.
    let (status, refused) = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(status, 409, "{refused}");
    let rows = refused["rows"].as_array().unwrap().clone();
    assert!(
        rows.iter().any(|r| r["kind"] == "base_tests"
            && r["matched"] == "base test pytest::tests.test_login::test_lockout"),
        "{refused}"
    );
    assert_eq!(repo.runs(), 3, "the offer reused the current result");
    for r in &rows {
        let (s, seen) = post(
            &c,
            &addr,
            &format!("/api/changes/{id}/review/seen"),
            json!({ "path": r["path"], "matched": r["matched"] }),
        )
        .await;
        assert_eq!(s, 200, "{seen}");
    }
    let (status, offered) = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(status, 200, "{offered}");
    assert_eq!(offered["offer"], "commands", "{offered}");

    // The certificate carries the section beside the evidence.
    let cert = get(&c, &addr, &format!("/api/changes/{id}/certificate")).await;
    let md = cert["markdown"].as_str().unwrap();
    assert!(md.contains("## Base checks"), "{md}");
    assert!(md.contains("**1 base test fails on this change.**"), "{md}");
    assert!(md.contains("git read-tree -u --reset"), "{md}");
    let bc = &cert["statement"]["predicate"]["devplane"]["baseChecks"];
    assert_eq!(bc["gen_ai.evidence.origin"], "externally_observed", "{bc}");
    assert_eq!(bc["onChange"]["restored"][0], "scripts/test.sh");
    assert!(throwaway_checkouts(&repo.root).is_empty());
}

#[tokio::test]
async fn no_defining_edit_skips_the_first_run_and_the_result_is_reused() {
    let repo = repo("plain", "", |root| {
        std::fs::write(root.join("src/extra.py"), "x = 1\n").unwrap();
    });
    let (addr, c, _host) = boot().await;
    let (id, worktree) = adopt(&c, &addr, &repo).await;
    let (_, v) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        json!({ "base_checks": true }),
    )
    .await;
    let b = &v["base_checks"];
    assert_eq!(
        b["on_change"]["says"], "the change edits no file that defines the checks",
        "{b}"
    );
    assert_eq!(b["on_base"]["says"], "the change adds or edits no tests");
    assert_eq!(repo.runs(), 1, "neither run started a process");

    // Current: asked again, nothing runs.
    let (status, again) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/base-checks"),
        Value::Null,
    )
    .await;
    assert_eq!((status, again["current"].clone()), (200, json!(true)));

    // An edit makes it stale; `check` has not run on the new tree.
    std::fs::write(worktree.join("src/extra.py"), "x = 2\n").unwrap();
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    let _ = row;
    let (status, refused) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/base-checks"),
        Value::Null,
    )
    .await;
    assert_eq!(status, 409, "{refused}");
    assert_eq!(
        refused["says"],
        "not run — `check` has not run against this tree"
    );
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(row["base_checks"]["stale"], true, "{}", row["base_checks"]);
    assert_eq!(row["base_checks"]["can_run"], false);
}

#[tokio::test]
async fn a_new_test_that_passes_on_the_base_is_said() {
    let repo = repo("newpass", "", |root| {
        std::fs::write(
            root.join("tests/test_rate.py"),
            "def test_limit(): assert \"def\" in \"src/login.py\"\n",
        )
        .unwrap();
    });
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    let (_, v) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        json!({ "base_checks": true }),
    )
    .await;
    let on_base = &v["base_checks"]["on_base"];
    assert_eq!(
        on_base["says"], "1 new test passes without the change",
        "{on_base}"
    );
    assert_eq!(on_base["weight"], "warn");
    assert_eq!(on_base["placed"][0]["path"], "tests/test_rate.py");
    assert!(on_base["placed"][0]["blob"].is_string());
    assert_eq!(on_base["passing"][0], "tests.test_rate::test_limit");
    // Adding a test file is a defining edit: the base's tests on this change
    // run without it, and pass.
    assert_eq!(
        v["base_checks"]["on_change"]["says"],
        "the base's tests pass on this change (1 file put back)"
    );
    // The new tests on the base never hold the offer.
    let (status, offered) = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(status, 200, "{offered}");
}

#[tokio::test]
async fn a_new_test_that_fails_without_the_change_is_plain() {
    let repo = repo("newfail", "", |root| {
        std::fs::write(root.join("src/rate.py"), "limit = 1\n").unwrap();
        std::fs::write(
            root.join("tests/test_rate.py"),
            "def test_limit(): assert \"limit\" in \"src/rate.py\"\n",
        )
        .unwrap();
    });
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    let (_, v) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        json!({ "base_checks": true }),
    )
    .await;
    let on_base = &v["base_checks"]["on_base"];
    assert_eq!(
        on_base["says"], "1 new test fails without the change",
        "{on_base}"
    );
    assert_eq!(on_base["weight"], "plain");
}

#[tokio::test]
async fn off_runs_nothing_and_the_offer_proceeds() {
    let repo = repo("off", "base_checks = \"off\"", speed_up_the_login_tests);
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    let (_, v) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        json!({ "base_checks": true }),
    )
    .await;
    assert_eq!(
        v["base_checks_refused"], "base checks are turned off in devplane.toml",
        "{v}"
    );
    assert!(
        v["base_checks"]["counts_says"]
            .as_str()
            .unwrap()
            .ends_with("base checks are turned off in devplane.toml")
    );
    assert_eq!(repo.runs(), 1);
    let (status, _) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/base-checks"),
        Value::Null,
    )
    .await;
    assert_eq!(status, 409);
    let cert = get(&c, &addr, &format!("/api/changes/{id}/certificate")).await;
    assert!(
        cert["markdown"]
            .as_str()
            .unwrap()
            .contains("Base checks are turned off in devplane.toml."),
        "{}",
        cert["markdown"]
    );
}

#[tokio::test]
async fn the_offer_runs_them_once_per_tree() {
    let repo = repo("offer", "", |root| {
        std::fs::write(root.join("src/extra.py"), "x = 1\n").unwrap();
        // A defining edit, so the first run spends a process.
        let t = twelve() + "def test_more(): assert True\n";
        std::fs::write(root.join("tests/test_login.py"), t).unwrap();
    });
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    let (_, v) = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(v["passed"], true);
    assert_eq!(repo.runs(), 1);
    // Two triggers at once share one pair.
    let (pa, pb) = (
        format!("/api/changes/{id}/base-checks"),
        format!("/api/changes/{id}/offer"),
    );
    let a = post(&c, &addr, &pa, Value::Null);
    let b = post(&c, &addr, &pb, Value::Null);
    let ((sa, _), (sb, offered)) = tokio::join!(a, b);
    assert!(sa == 202 || sa == 200, "{sa}");
    // The diff's own rows (a test file edited) hold it until seen.
    assert!(sb == 409 || sb == 200, "{sb} {offered}");
    assert_eq!(repo.runs(), 3, "one pair, whichever trigger came first");
    let (_, _) = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(repo.runs(), 3, "a second offer reuses the result");
    assert!(throwaway_checkouts(&repo.root).is_empty());
}

#[tokio::test]
async fn a_failing_check_runs_no_base_checks() {
    let repo = repo("failing", "", |root| {
        std::fs::write(
            root.join("tests/test_login.py"),
            twelve() + "def test_broken(): assert False\n",
        )
        .unwrap();
    });
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    let (_, v) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        json!({ "base_checks": true }),
    )
    .await;
    assert_eq!(v["passed"], false);
    assert!(v.get("base_checks").is_none(), "{v}");
    let (status, refused) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/base-checks"),
        Value::Null,
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(
        refused["says"],
        "not run — `check` does not pass with the change"
    );
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(row["base_checks"]["can_run"], false);
    assert_eq!(repo.runs(), 1);
}

#[tokio::test]
async fn auto_mode_waits_on_nothing() {
    let repo = repo("auto", "base_checks = \"auto\"", |root| {
        let t = twelve() + "def test_more(): assert True\n";
        std::fs::write(root.join("tests/test_login.py"), t).unwrap();
    });
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    // The runs in the throwaway checkout sleep; `check` in the change's does not.
    std::fs::write(repo.counter.with_extension("count.sleep"), "").unwrap();
    let started = std::time::Instant::now();
    let (_, v) = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
    assert_eq!(v["passed"], true, "{v}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "the verdict waited on base checks"
    );
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(row["waiting"]["on"], "person", "{row}");
    // While the pair runs, every surface says so and stays usable.
    let mut saw_running = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
        saw_running |= row["base_checks"]["counts_says"] == "ran 13 · base checks running";
        if row["base_checks"]["on_change"].is_object() && row["base_checks"]["absent"].is_null() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{}",
            row["base_checks"]
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(saw_running, "the header said base checks were running");
    assert!(throwaway_checkouts(&repo.root).is_empty());
    std::fs::remove_file(repo.counter.with_extension("count.sleep")).ok();
}

#[tokio::test]
async fn a_checkout_left_by_a_stopped_host_is_swept() {
    let repo = repo("sweep", "", |_| {});
    let name = ".devplane-base-deadbeef-cafe";
    let dir = repo.root.join(".claude/worktrees").join(name);
    git(
        &repo.root,
        &["worktree", "add", "--detach", dir.to_str().unwrap(), "main"],
    );
    assert_eq!(throwaway_checkouts(&repo.root).len(), 1);
    devplane::base_checks::sweep(&repo.root).await;
    assert!(throwaway_checkouts(&repo.root).is_empty());
    assert!(!dir.exists());
}

#[test]
fn counts_and_sentences_carry_no_ratio_or_colour() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/core/base_checks.rs"
    ))
    .unwrap()
        + &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/core/testcount/compare.rs"
        ))
        .unwrap();
    let production = src.split("#[cfg(test)]").next().unwrap_or_default();
    for banned in ["%", " of {", "percent", "ratio", "score"] {
        let hits: Vec<&str> = production
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .filter(|l| l.contains(banned))
            .collect();
        assert!(hits.is_empty(), "`{banned}` in a sentence: {hits:?}");
    }
}

#[tokio::test]
async fn a_setup_that_fails_with_the_change_is_a_row() {
    let repo = repo_with(
        "setup",
        "",
        "setup = \"! ls src | grep -q breaks\"",
        |root| {
            std::fs::write(root.join("src/breaks_setup.py"), "x\n").unwrap();
            let t = twelve() + "def test_more(): assert \"def\" in \"src/login.py\"\n";
            std::fs::write(root.join("tests/test_login.py"), t).unwrap();
        },
    );
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    let (_, v) = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/verify"),
        json!({ "base_checks": true }),
    )
    .await;
    assert_eq!(v["passed"], true, "{v}");
    let on_change = &v["base_checks"]["on_change"];
    assert_eq!(
        on_change["says"],
        "could not be run with this change's files: the setup command `! ls src | grep -q breaks` failed",
        "{on_change}"
    );
    assert_eq!(on_change["weight"], "warn");
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(row["qualifier"]["base_tests"], 1, "{}", row["qualifier"]);
    let (status, refused) = post(&c, &addr, &format!("/api/changes/{id}/offer"), Value::Null).await;
    assert_eq!(status, 409, "{refused}");
    assert_eq!(
        refused["rows"][0]["kind"], "base_tests",
        "base-test rows lead: {refused}"
    );
}

#[tokio::test]
async fn only_the_latest_check_keeps_its_test_names() {
    let repo = repo("junit", "", |_| {});
    let (addr, c, _host) = boot().await;
    let (id, _) = adopt(&c, &addr, &repo).await;
    for _ in 0..2 {
        let (_, v) = post(&c, &addr, &format!("/api/changes/{id}/verify"), Value::Null).await;
        assert_eq!(v["passed"], true, "{v}");
    }
    let row = get(&c, &addr, &format!("/api/changes/{id}")).await;
    let gates = row["gates"].as_array().unwrap();
    assert_eq!(gates.len(), 2);
    assert_eq!(
        gates[0]["junit"][0]["read"]["dropped"], true,
        "{}",
        gates[0]
    );
    assert_eq!(gates[0]["junit"][0]["read"]["cases"], json!([]));
    assert_eq!(
        gates[0]["commands"][0]["tests"]["tallies"][0]["ran"], 12,
        "counts stay"
    );
    assert_eq!(
        gates[1]["junit"][0]["read"]["cases"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    assert_eq!(gates[1]["junit"][0]["ignored"], true);
}

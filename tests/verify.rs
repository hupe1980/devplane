//! `devplane verify`: the base's `check` on the change, the weakened rows
//! first, and an exit code a workflow can rely on — with no host, store,
//! home or network. Each test builds a bare `origin` whose `HEAD` is `main`, a
//! clone of it with the fixture under `tests/fixtures/verify/` committed, and
//! one branch; the binary runs as a process.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_devplane");
const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.email=t@example.com", "-c", "user.name=Test"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

/// A scratch directory holding `origin.git` and its clone `work`.
struct Fixture {
    top: PathBuf,
    work: PathBuf,
}

impl Fixture {
    /// `main` holds the fixture (or `base_config` in place of its
    /// `devplane.toml`, `None` for no file); the clone is on a new `branch`.
    fn with(tag: &str, base_config: Option<&str>) -> Self {
        let top = std::env::temp_dir()
            .join(format!(
                "vp-verify-{tag}-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ))
            .canonicalize_or_self();
        let seed = top.join("seed");
        copy_dir(&Path::new(ROOT).join("tests/fixtures/verify"), &seed);
        match base_config {
            Some(c) => std::fs::write(seed.join("devplane.toml"), c).unwrap(),
            None => std::fs::remove_file(seed.join("devplane.toml")).unwrap(),
        }
        git(&seed, &["init", "-q", "-b", "main"]);
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "-qm", "base"]);
        git(&top, &["clone", "-q", "--bare", "seed", "origin.git"]);
        git(&top, &["clone", "-q", "origin.git", "work"]);
        let work = top.join("work");
        // A clone of a bare repository sets `origin/HEAD` to its default.
        assert_eq!(
            git(
                &work,
                &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]
            ),
            "origin/main"
        );
        Fixture { top, work }
    }

    fn new(tag: &str) -> Self {
        Self::with(tag, Some(&fixture_config()))
    }

    /// The fixture with more files committed on `main` beside it.
    fn beside(tag: &str, files: &[(&str, &str)]) -> Self {
        let f = Self::new(tag);
        for (path, text) in files {
            f.write(path, text);
        }
        f.commit("beside the fixture");
        git(&f.work, &["push", "-q", "origin", "main"]);
        f
    }

    fn branch(&self, name: &str) -> &Self {
        git(&self.work, &["checkout", "-q", "-b", name]);
        self
    }

    fn write(&self, path: &str, text: &str) -> &Self {
        let p = self.work.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
        self
    }

    fn commit(&self, msg: &str) -> &Self {
        git(&self.work, &["add", "-A"]);
        git(&self.work, &["commit", "-qm", msg]);
        self
    }

    fn verify(&self, args: &[&str]) -> Output {
        self.verify_in(&self.work, args, &[])
    }

    fn verify_in(&self, dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut c = Command::new(BIN);
        c.arg("verify")
            .args(args)
            .current_dir(dir)
            .env("DEVPLANE_HOME", self.top.join("no-home"))
            .env_remove("GITHUB_STEP_SUMMARY")
            .env_remove("GITHUB_EVENT_NAME")
            .env_remove("GITHUB_ACTIONS")
            .env_remove("VERIFY_MARK");
        for (k, v) in env {
            c.env(k, v);
        }
        c.output().expect("verify runs")
    }

    fn json(&self, args: &[&str]) -> (Value, i32) {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let out = self.verify(&all);
        parse(out)
    }

    fn path(&self, p: &str) -> PathBuf {
        self.top.join(p)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.top).ok();
    }
}

trait OrSelf {
    fn canonicalize_or_self(self) -> PathBuf;
}

impl OrSelf for PathBuf {
    fn canonicalize_or_self(self) -> PathBuf {
        std::fs::create_dir_all(&self).unwrap();
        self.canonicalize().unwrap_or(self)
    }
}

fn fixture_config() -> String {
    std::fs::read_to_string(Path::new(ROOT).join("tests/fixtures/verify/devplane.toml")).unwrap()
}

fn parse(out: Output) -> (Value, i32) {
    let code = out.status.code().expect("it exits");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "one JSON document ({e}):\n{text}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    assert_eq!(v["exit"], code, "the document's exit is the process's: {v}");
    (v, code)
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("it exits")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The `###` headings, in order.
fn headings(md: &str) -> Vec<String> {
    md.lines()
        .filter(|l| l.starts_with("### "))
        .map(str::to_string)
        .collect()
}

/// The text of the section under the `###` heading that starts with `name`.
fn section<'a>(md: &'a str, name: &str) -> &'a str {
    let start = md
        .find(&format!("### {name}"))
        .unwrap_or_else(|| panic!("no section {name}:\n{md}"));
    let rest = &md[start + 4..];
    let end = rest.find("\n### ").map(|e| e + 1).unwrap_or(rest.len());
    &rest[..end]
}

const SKIPPED: &str = "it.skip(\"rejects an expired token\", async () => {\n  expect(check(\"expired\")).toBe(false);\n});\n";
const FAILING: &str = "it(\"rejects an expired token\", async () => {\nFAIL\n});\n";

// ── A weakened check leads the summary ──────────────────────────────────────

#[test]
fn a_skipped_test_passes_check_and_leads_the_summary() {
    let f = Fixture::new("skip");
    f.branch("skip")
        .write("tests/login.test.js", SKIPPED)
        .commit("skip");
    let out_md = f.path("out.md");
    let out = f.verify(&["--base", "main", "--summary", out_md.to_str().unwrap()]);
    assert_eq!(
        code(&out),
        0,
        "a weakened row is evidence, not a failure: {}",
        stdout(&out)
    );
    let md = std::fs::read_to_string(&out_md).unwrap();
    let h = headings(&md);
    assert!(h[0].starts_with("### Weakened checks"), "{h:?}");
    let first = section(&md, "Weakened");
    assert!(first.contains("tests/login.test.js"), "{first}");
    assert!(
        first.contains("it.skip(\"rejects an expired token\""),
        "{first}"
    );
    assert!(first.contains("adds a skip marker"), "{first}");
    // Base checks ran after the pass: the base's test, put back, passes.
    assert!(
        section(&md, "Base checks").contains("The base's tests on this change"),
        "{md}"
    );
}

/// An edit to the agent's settings and to a lockfile leads the summary,
/// counted by kind, before the gate says `check` passed; the job is green.
#[test]
fn edited_agent_controls_and_lockfile_lead_the_summary() {
    let lock = "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"1.0.0\"\n";
    let f = Fixture::beside(
        "controls",
        &[
            (".claude/settings.json", "{\n  \"hooks\": {}\n}\n"),
            ("Cargo.lock", lock),
        ],
    );
    f.branch("controls")
        .write(
            ".claude/settings.json",
            "{\n  \"hooks\": { \"Stop\": [{ \"command\": \"sh ./x.sh\" }] }\n}\n",
        )
        .write(
            "Cargo.lock",
            &format!("{lock}\n[[package]]\nname = \"b\"\nversion = \"0.1.0\"\n"),
        )
        .commit("a hook and a package");
    let out_md = f.path("out.md");
    let out = f.verify(&["--base", "main", "--summary", out_md.to_str().unwrap()]);
    assert_eq!(code(&out), 0, "rows never fail the job: {}", stdout(&out));
    let md = std::fs::read_to_string(&out_md).unwrap();
    let h = headings(&md);
    assert_eq!(
        h[0], "### Weakened checks — 1 agent control changed · 1 supply-chain file changed",
        "{h:?}"
    );
    let first = section(&md, "Weakened");
    assert!(first.contains("#### 1 agent control changed"), "{first}");
    assert!(
        first.contains("#### 1 supply-chain file changed"),
        "{first}"
    );
    assert!(first.contains(".claude/settings.json"), "{first}");
    assert!(first.contains("Cargo.lock"), "{first}");
    assert!(first.contains("1 package added"), "{first}");
    let gate = md.find("### Gate").expect("a gate section");
    assert!(md.find(".claude/settings.json").unwrap() < gate, "{md}");
    assert!(md.find("Cargo.lock").unwrap() < gate, "{md}");

    let (v, code) = f.json(&["--base", "main"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["weakened"]["counts"]["agentControls"], 1, "{v}");
    assert_eq!(v["weakened"]["counts"]["supplyChain"], 1, "{v}");
    assert_eq!(v["weakened"]["counts"]["ownership"], 0, "{v}");
}

#[test]
fn nothing_weakened_is_a_sentence() {
    let f = Fixture::new("clean");
    f.branch("clean").write("src.js", "x\n").commit("clean");
    let out_md = f.path("out.md");
    let out = f.verify(&["--summary", out_md.to_str().unwrap()]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let md = std::fs::read_to_string(&out_md).unwrap();
    assert!(
        section(&md, "Weakened").contains("This change weakens no check the diff scan recognises."),
        "{md}"
    );
}

#[test]
fn summary_goes_to_github_step_summary_by_append() {
    let f = Fixture::new("append");
    f.branch("append").write("src.js", "x\n").commit("x");
    let step = f.path("step.md");
    std::fs::write(&step, "an earlier step's line\n").unwrap();
    let out = f.verify_in(
        &f.work,
        &[],
        &[("GITHUB_STEP_SUMMARY", step.to_str().unwrap())],
    );
    assert_eq!(code(&out), 0);
    let md = std::fs::read_to_string(&step).unwrap();
    assert!(md.starts_with("an earlier step's line\n"), "{md}");
    assert!(md.contains("## Devplane verify — verified"));

    // `--summary` wins over the variable, and replaces its file.
    let mine = f.path("mine.md");
    std::fs::write(&mine, "old\n").unwrap();
    let before = std::fs::read_to_string(&step).unwrap();
    let out = f.verify_in(
        &f.work,
        &["--summary", mine.to_str().unwrap()],
        &[("GITHUB_STEP_SUMMARY", step.to_str().unwrap())],
    );
    assert_eq!(code(&out), 0);
    assert_eq!(std::fs::read_to_string(&step).unwrap(), before);
    let m = std::fs::read_to_string(&mine).unwrap();
    assert!(m.starts_with("## Devplane verify"), "{m}");
}

// ── The base's config governs, whatever the change did to it ────────────────

#[test]
fn an_edited_check_runs_the_bases_and_names_the_edit() {
    let f = Fixture::new("gate-edit");
    f.branch("gate-edit")
        .write(
            "devplane.toml",
            &fixture_config().replace("check = [\"sh runner.sh\"]", "check = [\"true\"]"),
        )
        .write("tests/login.test.js", FAILING)
        .commit("gate edit");
    let mark = f.path("mark");
    let out_md = f.path("out.md");
    let out = f.verify_in(
        &f.work,
        &["--base", "main", "--summary", out_md.to_str().unwrap()],
        &[("VERIFY_MARK", mark.to_str().unwrap())],
    );
    assert_eq!(
        code(&out),
        1,
        "the base's check ran and failed: {}",
        stdout(&out)
    );
    assert_eq!(
        std::fs::read_to_string(&mark).unwrap().lines().count(),
        1,
        "the base's runner ran, once"
    );
    let md = std::fs::read_to_string(&out_md).unwrap();
    let first = section(&md, "Weakened");
    assert!(first.contains("` devplane.toml `"), "{first}");
    let gate = section(&md, "Gate");
    assert!(
        gate.contains("| Base (ran) | This change (not run) |"),
        "{gate}"
    );
    assert!(gate.contains("| ` sh runner.sh ` | ` true ` |"), "{gate}");
}

#[test]
fn roles_come_from_the_base() {
    let f = Fixture::new("roles");
    f.branch("roles")
        .write(
            "devplane.toml",
            &fixture_config().replace("tests = [\"tests/**\"]", "tests = [\"nothing/**\"]"),
        )
        .write(
            "tests/other.test.js",
            "it(\"x\", () => { expect(1).toBe(1); });\n",
        )
        .commit("add other");
    std::fs::remove_file(f.work.join("tests/login.test.js")).unwrap();
    f.commit("delete login");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 0, "{v}");
    let rows = v["weakened"]["rows"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|r| r["path"] == "tests/login.test.js" && r["kind"] == "deleted"),
        "the base's roles still call the file a test: {rows:?}"
    );
}

#[test]
fn an_unparseable_change_config_still_runs_the_bases_check() {
    let f = Fixture::new("unparseable");
    f.branch("u")
        .write("devplane.toml", "[gates\nnope")
        .commit("break");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["definition"]["changeDeclares"]["is"], "unreadable");
    assert_eq!(v["definition"]["check"][0], "sh runner.sh");
    assert!(
        v["weakened"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["path"] == "devplane.toml")
    );
}

#[test]
fn a_deleted_config_still_runs_the_bases_check() {
    let f = Fixture::new("deleted");
    f.branch("d");
    std::fs::remove_file(f.work.join("devplane.toml")).unwrap();
    f.commit("delete config");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["definition"]["changeDeclares"]["is"], "absent");
    assert!(
        v["weakened"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["path"] == "devplane.toml")
    );
}

// ── Exit codes ──────────────────────────────────────────────────────────────

#[test]
fn exit_codes_a_failing_check_is_one() {
    let f = Fixture::new("failing");
    f.branch("failing")
        .write("tests/login.test.js", FAILING)
        .commit("fail");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 1);
    assert_eq!(v["verdict"], "failed");
    assert_eq!(v["failed"], "command");
    assert_eq!(v["commands"][0]["failures"][0], "FAIL tests/login.test.js");
    let out_md = f.path("out.md");
    let out = f.verify(&["--summary", out_md.to_str().unwrap()]);
    assert_eq!(code_of(&out), 1);
    let md = std::fs::read_to_string(&out_md).unwrap();
    assert!(
        headings(&md)[0].starts_with("### Weakened"),
        "rows lead a failure too"
    );
    assert!(md.contains("<details><summary>Output"), "{md}");
}

fn code_of(out: &Output) -> i32 {
    code(out)
}

#[test]
fn exit_codes_a_check_that_moves_the_tree_is_one() {
    let config = fixture_config().replace(
        "check = [\"sh runner.sh\"]",
        "check = [\"sh runner.sh\", \"sh -c 'date +%N%s > generated.txt'\"]",
    );
    let f = Fixture::with("moves", Some(&config));
    f.branch("moves").write("src.js", "x\n").commit("x");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["failed"], "tree_moved");
    assert_eq!(v["treeMoved"]["paths"][0], "generated.txt");
}

#[test]
fn exit_codes_no_config_at_the_base_is_two() {
    let f = Fixture::with("no-config", None);
    f.branch("adds")
        .write("devplane.toml", &fixture_config())
        .commit("add config");
    let mark = f.path("mark");
    let out = f.verify_in(
        &f.work,
        &["--json"],
        &[("VERIFY_MARK", mark.to_str().unwrap())],
    );
    let (v, code) = parse(out);
    assert_eq!(code, 2, "a change cannot supply its own gate: {v}");
    assert_eq!(v["reason"], "no_config_at_base");
    assert!(
        v["says"]
            .as_str()
            .unwrap()
            .contains("cannot supply its own gate")
    );
    assert_eq!(v["commands"], serde_json::json!([]));
    assert!(!mark.exists(), "no command ran");
}

#[test]
fn exit_codes_no_check_declared_is_two() {
    let f = Fixture::with("no-check", Some("[gates]\ncheck = []\n"));
    f.branch("b").write("src.js", "x\n").commit("x");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 2);
    assert_eq!(v["reason"], "no_check");
    assert_eq!(v["commands"], serde_json::json!([]));
}

#[test]
fn exit_codes_an_unparseable_base_config_is_two() {
    let f = Fixture::with("bad-base", Some("[gates\n"));
    f.branch("b").write("src.js", "x\n").commit("x");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 2);
    assert_eq!(v["reason"], "config_unreadable_at_base");
}

#[test]
fn exit_codes_an_unknown_base_is_two_naming_both_spellings() {
    let f = Fixture::new("nosuch");
    let (v, code) = f.json(&["--base", "nosuch"]);
    assert_eq!(code, 2);
    assert_eq!(v["reason"], "base_unresolvable");
    let says = v["says"].as_str().unwrap();
    assert!(
        says.contains("`origin/nosuch`") && says.contains("`nosuch`"),
        "{says}"
    );
}

#[test]
fn exit_codes_a_shallow_clone_is_two_naming_fetch_depth() {
    let f = Fixture::new("shallow");
    // Move `main` on, and fork a branch from its first commit, so a depth-1
    // clone holds neither history.
    let seed = f.top.join("seed");
    std::fs::write(seed.join("more.js"), "1\n").unwrap();
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-qm", "more"]);
    git(&seed, &["checkout", "-q", "-b", "feature", "HEAD~1"]);
    std::fs::write(seed.join("feature.js"), "1\n").unwrap();
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-qm", "feature"]);
    let url = format!("file://{}", seed.display());
    git(
        &f.top,
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            "--no-single-branch",
            "--branch",
            "feature",
            &url,
            "shallow",
        ],
    );
    let out = f.verify_in(&f.top.join("shallow"), &["--json", "--base", "main"], &[]);
    let (v, code) = parse(out);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["reason"], "no_shared_history");
    assert!(
        v["says"].as_str().unwrap().contains("fetch-depth: 0"),
        "{v}"
    );
}

#[test]
fn exit_codes_no_default_base_suggests_one() {
    let f = Fixture::new("no-default");
    let lone = f.top.join("lone");
    std::fs::create_dir_all(&lone).unwrap();
    git(&lone, &["init", "-q", "-b", "trunk"]);
    std::fs::write(lone.join("devplane.toml"), fixture_config()).unwrap();
    git(&lone, &["add", "-A"]);
    git(&lone, &["commit", "-qm", "x"]);
    let (v, code) = parse(f.verify_in(&lone, &["--json"], &[]));
    assert_eq!(code, 2);
    assert!(v["says"].as_str().unwrap().contains("--base"), "{v}");
}

#[test]
fn exit_codes_outside_a_repository_is_two() {
    let f = Fixture::new("outside");
    let bare = f.top.join("plain");
    std::fs::create_dir_all(&bare).unwrap();
    let (v, code) = parse(f.verify_in(
        &bare,
        &["--json"],
        &[("GIT_CEILING_DIRECTORIES", f.top.to_str().unwrap())],
    ));
    assert_eq!(code, 2);
    assert_eq!(v["reason"], "not_a_repository");
}

#[cfg(unix)]
#[test]
fn exit_codes_an_unwritable_summary_is_two_after_a_pass_and_one_after_a_failure() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new("readonly");
    let ro = f.path("ro");
    std::fs::create_dir_all(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    let target = ro.join("out.md");
    f.branch("b").write("src.js", "x\n").commit("x");
    let (v, code) = f.json(&["--summary", target.to_str().unwrap()]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["reason"], "could_not_write");
    assert!(!v["commands"].as_array().unwrap().is_empty(), "check ran");

    f.write("tests/login.test.js", FAILING).commit("fail");
    let (v, code) = f.json(&["--summary", target.to_str().unwrap()]);
    assert_eq!(code, 1, "a failure stays a failure: {v}");
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_push_to_the_base_verifies_with_no_change() {
    let f = Fixture::new("push");
    let out_md = f.path("out.md");
    let out = f.verify(&["--summary", out_md.to_str().unwrap()]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let md = std::fs::read_to_string(&out_md).unwrap();
    assert!(
        section(&md, "Weakened").contains("No change against ` main `"),
        "{md}"
    );
}

// ── No home, no host, nothing written ───────────────────────────────────────

#[test]
fn verify_needs_no_home_and_no_host() {
    let f = Fixture::new("nohome");
    f.branch("skip")
        .write("tests/login.test.js", SKIPPED)
        .commit("skip");
    // A host that would answer, on the port `verify` would be told to use:
    // a port of its own, so nothing else on this machine talking to the
    // default one can be taken for `verify`.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("a free port");
    let port = listener.local_addr().unwrap().port().to_string();
    let out = f.verify_in(&f.work, &[], &[("DEVPLANE_PORT", port.as_str())]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(
        !f.path("no-home").exists(),
        "the Devplane home was not created"
    );
    listener.set_nonblocking(true).unwrap();
    let accepted = listener.accept();
    assert!(
        matches!(&accepted, Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "verify contacted a host: {accepted:?}"
    );
}

#[test]
fn uncommitted_work_is_what_is_verified() {
    let f = Fixture::new("uncommitted");
    f.branch("wip")
        .write("tests/login.test.js", SKIPPED)
        .write("tests/new.test.js", "it(\"new\", () => {});\n");
    let (v, code) = f.json(&[]);
    assert_eq!(code, 0, "{v}");
    let rows = v["weakened"]["rows"].as_array().unwrap();
    assert!(
        rows.iter().any(|r| r["path"] == "tests/login.test.js"),
        "{rows:?}"
    );
    let head_tree = git(&f.work, &["rev-parse", "HEAD^{tree}"]);
    assert_ne!(
        v["tree"],
        head_tree.as_str(),
        "the digest covers the working tree"
    );
}

#[test]
fn verify_writes_nothing_to_the_repository() {
    let f = Fixture::new("readonly-repo");
    f.branch("skip").write("tests/login.test.js", SKIPPED);
    let status = git(&f.work, &["status", "--porcelain"]);
    let refs = git(&f.work, &["for-each-ref"]);
    let index = std::fs::metadata(f.work.join(".git/index"))
        .unwrap()
        .modified()
        .unwrap();
    let out = f.verify(&[]);
    assert_eq!(code(&out), 0);
    // Read before anything else runs git here: a plain `git status` may
    // refresh the index itself.
    let after = std::fs::metadata(f.work.join(".git/index"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(after, index, "verify rewrote the index");
    assert_eq!(git(&f.work, &["status", "--porcelain"]), status);
    assert_eq!(git(&f.work, &["for-each-ref"]), refs);
}

#[test]
fn verify_names_no_store_host_client_or_home() {
    let mut sources = vec![Path::new(ROOT).join("src/verify.rs")];
    for e in std::fs::read_dir(Path::new(ROOT).join("src/core/verify")).unwrap() {
        sources.push(e.unwrap().path());
    }
    for path in sources {
        let text = std::fs::read_to_string(&path).unwrap();
        for banned in [
            "crate::store",
            "crate::local",
            "crate::client",
            "crate::host",
            "crate::policy_cache",
            "config::home",
            "reqwest",
            "hyper",
            "TcpStream",
        ] {
            assert!(
                !text.contains(banned),
                "{} names `{banned}`: verify needs only git",
                path.display()
            );
        }
    }
}

// ── The statement ───────────────────────────────────────────────────────────

#[test]
fn the_statement_names_the_tree_check_ran_on() {
    let f = Fixture::new("statement");
    f.branch("skip")
        .write("tests/login.test.js", SKIPPED)
        .commit("skip");
    let s = f.path("s.json");
    let (v, code) = f.json(&["--statement", s.to_str().unwrap()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["wrote"]["statement"], s.to_str().unwrap());
    let st: Value = serde_json::from_str(&std::fs::read_to_string(&s).unwrap()).unwrap();
    assert_eq!(st["_type"], "https://in-toto.io/Statement/v1");
    assert_eq!(
        st["predicateType"],
        "https://in-toto.io/attestation/test-result/v0.1"
    );
    assert_eq!(st["subject"][0]["digest"]["gitTree"], v["tree"]);
    assert_eq!(st["predicate"]["result"], "PASSED");
    // The definition that ran: its commands by digest, and the base's file.
    assert_eq!(
        st["predicate"]["configuration"][0]["name"],
        "devplane.toml#gates.check"
    );
    assert_eq!(
        st["predicate"]["configuration"][1]["digest"]["gitBlob"],
        v["definition"]["fileBlob"]
    );
    assert_eq!(
        st["predicate"]["devplane"]["verify"]["baseCommit"],
        v["base"]["mergeBase"]
    );
    assert_eq!(
        st["predicate"]["devplane"]["verify"]["definition"]["blob"],
        v["definition"]["fileBlob"]
    );
    assert!(
        !st["predicate"]["devplane"]["checksAltered"]["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // Weakened, and still PASSED: `result` is `check`'s exit codes alone.
    assert_eq!(st["predicate"]["devplane"]["signed"], false);

    // Failing: written, with a failing result.
    f.write("tests/login.test.js", FAILING).commit("fail");
    let (_, code) = f.json(&["--statement", s.to_str().unwrap()]);
    assert_eq!(code, 1);
    let st: Value = serde_json::from_str(&std::fs::read_to_string(&s).unwrap()).unwrap();
    assert_eq!(st["predicate"]["result"], "FAILED");
    assert!(
        !st["predicate"]["failedTests"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // Could not run: none written.
    let none = f.path("none.json");
    let (_, code) = f.json(&["--base", "nosuch", "--statement", none.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(!none.exists());
}

/// An in-toto Statement v1: its four top-level fields and nothing else, and at
/// least one subject, each with a digest that names something.
fn assert_statement_v1(st: &Value) {
    let mut keys: Vec<&str> = st.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["_type", "predicate", "predicateType", "subject"],
        "{st}"
    );
    let subject = st["subject"].as_array().expect("subject is a list");
    assert!(!subject.is_empty(), "a statement with no subject: {st}");
    for s in subject {
        let digest = s["digest"].as_object().expect("a digest");
        assert!(!digest.is_empty(), "{st}");
        assert!(
            digest
                .values()
                .all(|d| d.as_str().is_some_and(|d| !d.is_empty())),
            "{st}"
        );
    }
}

#[test]
fn the_statement_is_an_in_toto_statement_v1() {
    let f = Fixture::new("statement-v1");
    f.branch("x").write("src.js", "x\n").commit("x");
    let s = f.path("s.json");
    let (v, code) = f.json(&["--statement", s.to_str().unwrap()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["wrote"]["statementWithheld"], Value::Null, "{v}");
    let st: Value = serde_json::from_str(&std::fs::read_to_string(&s).unwrap()).unwrap();
    assert_statement_v1(&st);
}

/// With no tree recorded there is no subject, so no statement is written —
/// never one with an empty subject — and the document says why.
#[test]
fn a_run_with_no_tree_writes_no_statement_and_says_why() {
    let config = fixture_config().replace(
        "check = [\"sh runner.sh\"]",
        "check = [\"sh runner.sh\", \"sh -c 'date +%N%s > generated.txt'\"]",
    );
    let f = Fixture::with("statement-no-tree", Some(&config));
    f.branch("moves").write("src.js", "x\n").commit("x");
    let s = f.path("s.json");
    let (v, code) = f.json(&["--statement", s.to_str().unwrap()]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["failed"], "tree_moved", "{v}");
    assert!(!s.exists(), "a statement was written with no tree to name");
    assert_eq!(v["wrote"]["statement"], Value::Null, "{v}");
    assert!(
        v["wrote"]["statementWithheld"]
            .as_str()
            .is_some_and(|w| w.starts_with("No in-toto statement")),
        "{v}"
    );
}

// ── Base checks ────────────────────────────────────────────────────────────

/// A runner shaped like pytest. Each `def test_x(): assert "TOKEN" in "FILE"`
/// passes when FILE holds TOKEN; `assert True` always passes. It prints
/// pytest's summary and failing lines, and writes JUnit to `reports/junit.xml`.
const PYTEST: &str = r#"#!/bin/sh
mkdir -p reports
awk '
/^def test_/ {
  name = $2; sub(/\(.*/, "", name)
  ok = 1
  if (match($0, /assert "[^"]*" in "[^"]*"/)) {
    s = substr($0, RSTART, RLENGTH); split(s, q, "\"")
    ok = (system("grep -q \"" q[2] "\" \"" q[4] "\" 2>/dev/null") == 0)
  }
  cls = FILENAME; sub(/\.py$/, "", cls); gsub("/", ".", cls)
  if (ok) { pass++; xml = xml "<testcase classname=\"" cls "\" name=\"" name "\"/>" }
  else { fail++; xml = xml "<testcase classname=\"" cls "\" name=\"" name "\"><failure/></testcase>"; print "FAILED " FILENAME "::" name " - assert" }
}
END {
  printf "<testsuites><testsuite name=\"pytest\">%s</testsuite></testsuites>\n", xml > "reports/junit.xml"
  out = ""
  if (fail) out = out (out ? ", " : "") fail " failed"
  if (pass) out = out (out ? ", " : "") pass " passed"
  if (out == "") out = "no tests ran"
  print "============ " out " in 0.01s ============"
  exit (fail ? 1 : 0)
}' tests/*.py
"#;

const LOGIN_TESTS: &str = "def test_lockout(): assert \"lockout\" in \"src/login.py\"\n\
def test_login(): assert \"def\" in \"src/login.py\"\n";

/// `main` runs the pytest-shaped runner, with JUnit, over two login tests.
fn pytest_fixture(tag: &str) -> Fixture {
    let f = Fixture::with(
        tag,
        Some(
            "[gates]\ncheck = [\"sh scripts/test.sh\"]\ntimeout = \"60s\"\n\
             junit = [\"reports/junit.xml\"]\n\n[review.roles]\ntests = [\"tests/**\"]\n",
        ),
    );
    f.write(".gitignore", "reports/\n")
        .write("scripts/test.sh", PYTEST)
        .write("tests/test_login.py", LOGIN_TESTS)
        .write("src/login.py", "def login(): pass\n# lockout\n")
        .commit("pytest");
    git(&f.work, &["push", "-q", "origin", "main"]);
    f
}

fn worktrees(dir: &Path) -> String {
    git(dir, &["worktree", "list", "--porcelain"])
}

#[test]
fn base_checks_join_the_first_section_and_leave_no_checkout() {
    let f = pytest_fixture("basechecks");
    // The change drops the lockout and loosens the test that would catch it:
    // no skip marker, so the diff scan names nothing.
    f.branch("loosen")
        .write("src/login.py", "def login(): pass\n")
        .write(
            "tests/test_login.py",
            &LOGIN_TESTS.replace(
                "def test_lockout(): assert \"lockout\" in \"src/login.py\"",
                "def test_lockout(): assert True",
            ),
        )
        .commit("loosen");
    let before = worktrees(&f.work);
    let out_md = f.path("out.md");
    let st = f.path("s.json");
    let out = f.verify(&[
        "--summary",
        out_md.to_str().unwrap(),
        "--statement",
        st.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&out),
        0,
        "a failing base test is a weakened row, never the exit code: {}",
        stdout(&out)
    );
    assert_eq!(worktrees(&f.work), before, "the throwaway checkout is gone");
    assert!(stdout(&out).contains("tests     ran 2"), "{}", stdout(&out));

    let md = std::fs::read_to_string(&out_md).unwrap();
    let first = section(&md, "Weakened");
    assert!(
        first.starts_with("Weakened checks — 1 base test fails on this change"),
        "{first}"
    );
    assert!(
        first.contains("pytest::tests.test_login::test_lockout"),
        "the base's test is named: {first}"
    );
    assert!(
        md.contains("Tests: ran 2"),
        "the counts sit by the verdict: {md}"
    );
    assert!(
        section(&md, "Base checks").contains("1 base test fails on this change"),
        "{md}"
    );

    let st: Value = serde_json::from_str(&std::fs::read_to_string(&st).unwrap()).unwrap();
    let bc = &st["predicate"]["devplane"]["baseChecks"];
    assert_eq!(bc["gen_ai.evidence.origin"], "externally_observed", "{bc}");
    assert_eq!(
        bc["onChange"]["failing"][0], "pytest::tests.test_login::test_lockout",
        "{bc}"
    );
    assert!(st["predicate"]["devplane"]["verify"]["baseCommit"].is_string());
    assert!(st["predicate"].get("verify").is_none(), "one namespace");

    // Asked not to: `check` alone, and the summary says why.
    let (v, code) = f.json(&["--no-base-checks"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["weakened"]["counts"]["baseTests"], 0, "{v}");
    assert_eq!(v["baseChecks"]["absent"], "declined", "{v}");
    assert!(
        v["baseChecksSays"][0]
            .as_str()
            .unwrap()
            .contains("--no-base-checks"),
        "{v}"
    );
}

#[test]
fn a_new_test_that_passes_on_the_base_is_listed() {
    let f = pytest_fixture("newpass");
    f.branch("rate")
        .write(
            "tests/test_rate.py",
            "def test_limit(): assert \"def\" in \"src/login.py\"\n",
        )
        .commit("rate");
    let out_md = f.path("out.md");
    let out = f.verify(&["--summary", out_md.to_str().unwrap()]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let md = std::fs::read_to_string(&out_md).unwrap();
    let bc = section(&md, "Base checks");
    assert!(bc.contains("1 new test passes without the change"), "{bc}");
    assert!(bc.contains("tests.test_rate::test_limit"), "listed: {bc}");
    assert!(
        section(&md, "Weakened").contains("nothing weakened"),
        "a new test that passes without the change is not a weakened row: {md}"
    );
    assert!(
        stdout(&out).contains("passes without the change  tests.test_rate::test_limit"),
        "{}",
        stdout(&out)
    );
}

// ── Annotations and SARIF ───────────────────────────────────────────────────

const ON_GITHUB: &[(&str, &str)] = &[("GITHUB_ACTIONS", "true")];

/// The structure SARIF 2.1.0 requires, and what a code-scanning upload reads:
/// `version`, `runs[].tool.driver.name`, each result's `message.text`, a
/// `level` from the enumeration, a `ruleId` matching `ruleIndex`, and a
/// relative `artifactLocation.uri` with a `startLine` of at least one.
fn assert_sarif_2_1_0(log: &Value) {
    assert_eq!(log["version"], "2.1.0", "{log}");
    let runs = log["runs"].as_array().expect("runs is an array");
    assert!(!runs.is_empty(), "{log}");
    for run in runs {
        let driver = &run["tool"]["driver"];
        assert!(
            driver["name"].as_str().is_some_and(|n| !n.is_empty()),
            "{run}"
        );
        let rules = driver["rules"].as_array().cloned().unwrap_or_default();
        for r in &rules {
            assert!(r["id"].is_string(), "every rule has an id: {r}");
        }
        for res in run["results"].as_array().expect("results is an array") {
            assert!(res["message"]["text"].is_string(), "{res}");
            assert!(
                ["none", "note", "warning", "error"].contains(&res["level"].as_str().unwrap()),
                "{res}"
            );
            let id = res["ruleId"].as_str().expect("ruleId");
            let i = res["ruleIndex"].as_u64().expect("ruleIndex") as usize;
            assert_eq!(rules[i]["id"], id, "ruleIndex names ruleId: {res}");
            let locs = res["locations"].as_array().expect("locations");
            assert!(!locs.is_empty(), "code scanning needs one location: {res}");
            for l in locs {
                let pl = &l["physicalLocation"];
                let uri = pl["artifactLocation"]["uri"].as_str().expect("uri");
                assert!(
                    !uri.is_empty() && !uri.starts_with('/') && !uri.contains("://"),
                    "relative to the repository: {uri}"
                );
                if let Some(region) = pl.get("region") {
                    assert!(region["startLine"].as_u64().unwrap() >= 1, "{res}");
                }
            }
        }
    }
}

/// The settled-when check, locally: on GitHub Actions a skip marker is a
/// warning annotation on its file and line, the job summary says so under
/// the weakened checks, `--sarif` writes the same row, and the exit is 0.
#[test]
fn on_github_a_skip_marker_is_an_annotation_on_its_line_and_in_sarif() {
    let f = Fixture::new("annotate");
    f.branch("skip")
        .write("tests/login.test.js", SKIPPED)
        .commit("skip");
    let out_md = f.path("out.md");
    let sarif = f.path("verify.sarif");
    let out = f.verify_in(
        &f.work,
        &[
            "--base",
            "main",
            "--summary",
            out_md.to_str().unwrap(),
            "--sarif",
            sarif.to_str().unwrap(),
        ],
        ON_GITHUB,
    );
    let text = stdout(&out);
    assert_eq!(code(&out), 0, "a weakened row never fails the job: {text}");
    let annotations: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("::warning"))
        .collect();
    assert_eq!(
        annotations,
        [
            "::warning file=tests/login.test.js,line=1,title=Devplane verify — check weakened::\
             adds a skip marker `it.skip(` — it.skip(\"rejects an expired token\", async () => {",
            // The scan also reads the replaced `it(` as a test removed: a
            // second row, on the line where it stood.
            "::warning file=tests/login.test.js,line=1,title=Devplane verify — test deleted::\
             removes a test and keeps the file — it(\"rejects an expired token\", async () => {",
        ],
        "{text}"
    );
    // The repository's own text is printed with commands paused, and the
    // annotations after they resume.
    let stop = text
        .lines()
        .position(|l| l.starts_with("::stop-commands::"))
        .expect("commands paused");
    let token = text.lines().nth(stop).unwrap()["::stop-commands::".len()..].to_string();
    assert_eq!(token.len(), 32, "an unguessable token");
    let resume = text
        .lines()
        .position(|l| l == format!("::{token}::"))
        .expect("commands resumed");
    let warning = text
        .lines()
        .position(|l| l.starts_with("::warning"))
        .unwrap();
    assert!(stop < resume && resume < warning, "{text}");
    assert!(text.contains("sarif        written to"), "{text}");

    let md = std::fs::read_to_string(&out_md).unwrap();
    assert!(headings(&md)[0].starts_with("### Weakened checks"), "{md}");
    assert!(
        section(&md, "Weakened").contains("Annotations: 2 written"),
        "{md}"
    );

    let log: Value = serde_json::from_str(&std::fs::read_to_string(&sarif).unwrap()).unwrap();
    assert_sarif_2_1_0(&log);
    let results = log["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "{log}");
    assert_eq!(results[0]["ruleId"], "weakened/skip");
    assert_eq!(results[1]["ruleId"], "weakened/deleted");
    assert_eq!(results[0]["level"], "warning");
    let pl = &results[0]["locations"][0]["physicalLocation"];
    assert_eq!(pl["artifactLocation"]["uri"], "tests/login.test.js");
    assert_eq!(pl["region"]["startLine"], 1);

    // Off GitHub: no command in the output, and the same SARIF.
    let out = f.verify(&["--base", "main"]);
    assert!(!stdout(&out).contains("::"), "{}", stdout(&out));
    // Under `--json` stdout stays one document; the annotations go to stderr.
    let out = f.verify_in(&f.work, &["--json", "--base", "main"], ON_GITHUB);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        err.contains(annotations[0]) && err.contains(annotations[1]),
        "{err}"
    );
    let (v, code) = parse(out);
    assert_eq!(code, 0);
    assert_eq!(v["annotations"]["printed"], 2, "{v}");
    assert_eq!(v["annotations"]["leftOut"], 0, "{v}");
}

#[test]
fn a_failing_named_test_is_an_error_annotation_and_the_exit_is_unchanged() {
    let f = pytest_fixture("annotate-fail");
    f.branch("drop")
        .write("src/login.py", "def login(): pass\n")
        .commit("drop");
    let sarif = f.path("verify.sarif");
    let out = f.verify_in(&f.work, &["--sarif", sarif.to_str().unwrap()], ON_GITHUB);
    let text = stdout(&out);
    assert_eq!(
        code(&out),
        1,
        "check failed, so 1 — as without annotations: {text}"
    );
    let errors: Vec<&str> = text.lines().filter(|l| l.starts_with("::error")).collect();
    assert_eq!(
        errors,
        [
            "::error title=Devplane verify — test failed::tests.test_login::test_lockout failed in \
          the base's check"
        ],
        "JUnit names no file, so the annotation is on none: {text}"
    );
    let log: Value = serde_json::from_str(&std::fs::read_to_string(&sarif).unwrap()).unwrap();
    assert_sarif_2_1_0(&log);
    let r = &log["runs"][0]["results"][0];
    assert_eq!(r["level"], "error", "{log}");
    assert_eq!(r["ruleId"], "test/failed", "{log}");
}

#[test]
fn annotations_past_githubs_per_step_limit_are_counted_in_the_summary() {
    let f = Fixture::new("annotate-cap");
    f.branch("many");
    for i in 0..12 {
        f.write(
            &format!("tests/t{i}.test.js"),
            "it.skip(\"x\", () => {});\n",
        );
    }
    f.commit("many");
    let out_md = f.path("out.md");
    let sarif = f.path("verify.sarif");
    let out = f.verify_in(
        &f.work,
        &[
            "--base",
            "main",
            "--summary",
            out_md.to_str().unwrap(),
            "--sarif",
            sarif.to_str().unwrap(),
        ],
        ON_GITHUB,
    );
    let text = stdout(&out);
    assert_eq!(code(&out), 0, "{text}");
    assert_eq!(
        text.lines().filter(|l| l.starts_with("::warning")).count(),
        10,
        "{text}"
    );
    let md = std::fs::read_to_string(&out_md).unwrap();
    assert!(
        section(&md, "Weakened").contains("Annotations: 10 written") && md.contains("2 left out"),
        "{md}"
    );
    let log: Value = serde_json::from_str(&std::fs::read_to_string(&sarif).unwrap()).unwrap();
    assert_sarif_2_1_0(&log);
    assert_eq!(log["runs"][0]["results"].as_array().unwrap().len(), 12);
}

// ── The action and its documentation ───────────────────────────────────────

fn action() -> String {
    std::fs::read_to_string(Path::new(ROOT).join("action.yml")).expect("action.yml at the root")
}

/// The body of every `run:` block in a YAML file, by indentation.
fn run_blocks(yaml: &str) -> Vec<String> {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        let t = l.trim_start();
        if let Some(rest) = t.strip_prefix("run:").or_else(|| t.strip_prefix("- run:")) {
            let indent = l.len() - t.len();
            let mut body = rest.trim().to_string();
            i += 1;
            while i < lines.len() {
                let n = lines[i];
                let ni = n.len() - n.trim_start().len();
                if !n.trim().is_empty() && ni <= indent {
                    break;
                }
                body.push('\n');
                body.push_str(n);
                i += 1;
            }
            out.push(body);
            continue;
        }
        i += 1;
    }
    out
}

#[test]
fn the_action_interpolates_no_input_into_a_script() {
    let a = action();
    let blocks = run_blocks(&a);
    assert!(blocks.len() >= 3, "the steps are bash: {blocks:?}");
    for b in blocks {
        assert!(!b.contains("${{"), "an expression inside a script: {b}");
    }
}

#[test]
fn the_action_requests_no_forge_write() {
    let a = action();
    for banned in [
        "GITHUB_TOKEN",
        "api.github.com",
        "upload-artifact",
        "secrets.",
        "permissions:",
    ] {
        assert!(!a.contains(banned), "action.yml names `{banned}`");
    }
    // No GitHub CLI call in any script.
    for b in run_blocks(&a) {
        assert!(
            !b.lines().any(|l| {
                let t = l.trim_start();
                t.starts_with("gh ") || t.contains(" gh ") || t.contains("$(gh ")
            }),
            "a script calls the GitHub CLI: {b}"
        );
    }
}

#[test]
fn the_action_supports_only_what_the_installer_does() {
    let a = action();
    assert!(a.contains("Linux") && a.contains("Apple Silicon macOS"));
}

#[test]
fn the_snippet_in_the_docs_uses_fetch_depth_0_and_pull_request() {
    let doc = std::fs::read_to_string(Path::new(ROOT).join("site/content/docs/ci.md"))
        .expect("site/content/docs/ci.md");
    assert!(doc.contains("pull_request:"));
    assert!(!doc.contains("on:\n  pull_request_target"));
    assert!(doc.contains("fetch-depth: 0"));
    assert!(doc.contains("contents: read"));
    assert!(doc.contains("branch protection"));
}

#[test]
fn every_flag_verify_documents_is_implemented() {
    let help = Command::new(BIN)
        .args(["verify", "--help"])
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&help.stdout).to_string();
    let flags = [
        "--base",
        "--json",
        "--summary",
        "--statement",
        "--no-base-checks",
        "--sarif",
    ];
    for f in flags {
        assert!(help.contains(f), "--help lists {f}");
    }
    let flag_re = |text: &str| -> Vec<String> {
        let mut out = Vec::new();
        for (i, _) in text.match_indices("--") {
            let rest = &text[i + 2..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                .collect();
            if !name.is_empty() {
                out.push(format!("--{name}"));
            }
        }
        out
    };
    for flag in flag_re(&help) {
        assert!(
            flags.contains(&flag.as_str()) || flag == "--help",
            "--help offers {flag}, which verify does not implement"
        );
    }
    for page in ["site/content/docs/ci.md", "site/content/docs/cli.md"] {
        let text = std::fs::read_to_string(Path::new(ROOT).join(page)).unwrap();
        for line in text.lines().filter(|l| l.contains("devplane verify")) {
            for flag in flag_re(line) {
                assert!(
                    flags.contains(&flag.as_str()),
                    "{page} documents `devplane verify {flag}`, which does not exist"
                );
            }
        }
    }
    assert_eq!(
        Command::new(BIN)
            .args(["verify", "--no-such-flag"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2),
        "a usage error exits 2"
    );
}

//! A throwaway repository and Devplane home. `HOME` (and `USERPROFILE`) point
//! inside the sandbox for its whole life, so a test that deletes the Devplane
//! home can never reach the real one.

#![allow(dead_code)] // each test binary uses a different part of the fixture

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// `HOME` is process-global, so only one sandbox may exist at a time. The race
/// is between threads of one binary, so an in-process mutex suffices.
fn home_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// A repository and a home, both disposable.
pub struct Sandbox {
    /// A git repository with a commit, a specification folder and a
    /// `devplane.toml`.
    pub repo: PathBuf,
    /// Stands in for `~`. `HOME` points here for the life of the process.
    pub home: PathBuf,
    /// The previous `HOME`, restored on drop.
    prior_home: Option<String>,
    /// Held for the sandbox's life, so `HOME` belongs to one sandbox.
    _home_guard: MutexGuard<'static, ()>,
    /// `PATH` minus every directory holding a `devplane` executable, so removing
    /// Devplane removes the binary too, not just the home.
    path_without_devplane: String,
}

/// Every `PATH` entry that does not contain a `devplane` executable.
fn path_without_devplane() -> String {
    let exe = if cfg!(windows) {
        "devplane.exe"
    } else {
        "devplane"
    };
    let kept: Vec<_> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|d| !d.join(exe).exists())
        .collect();
    std::env::join_paths(kept)
        .expect("a PATH")
        .to_string_lossy()
        .into_owned()
}

/// How much of a repository a fixture has. `NoCommits` is a valid state the
/// product handles, not a failure.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The ordinary case: a commit, a spec folder, gates.
    Full,
    /// Committed and gated, but no specification folder at all.
    NoSpec,
    /// Committed, with a `devplane.toml` declaring no checks.
    NoGates,
    /// `git init` and nothing else.
    NoCommits,
}

fn scratch(kind: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dp-removable-{}-{kind}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&d).expect("a scratch directory");
    d
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git is on PATH");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

impl Sandbox {
    pub fn new(shape: Shape) -> Self {
        // A poisoned lock is fine: the panicking sandbox's `Drop` restored `HOME`.
        let guard = home_lock().lock().unwrap_or_else(|e| e.into_inner());
        let repo = scratch("repo");
        let home = scratch("home");

        // Redirect the home before anything else runs.
        let prior_home = std::env::var("HOME").ok();
        // SAFETY: `home_lock` serialises sandboxes; this is the only writer of `HOME`.
        unsafe {
            std::env::set_var("HOME", &home);
            if cfg!(windows) {
                std::env::set_var("USERPROFILE", &home);
            }
        }
        std::fs::create_dir_all(home.join(".devplane")).expect("a devplane home");

        git(&repo, &["init", "--quiet", "-b", "main"]);
        git(&repo, &["config", "user.email", "fixture@example.invalid"]);
        git(&repo, &["config", "user.name", "Fixture"]);

        if shape != Shape::NoCommits {
            std::fs::write(repo.join("README.md"), "# fixture\n").unwrap();

            if shape != Shape::NoSpec {
                let spec = repo.join("specs/001-example");
                std::fs::create_dir_all(&spec).unwrap();
                std::fs::write(spec.join("spec.md"), "# Example\n\n## Requirements\n").unwrap();
                std::fs::write(spec.join("tasks.md"), "- [x] T001 done\n- [ ] T002 open\n")
                    .unwrap();
            }

            // Gates any machine can run, independent of Devplane.
            let gates = if shape == Shape::NoGates {
                "[project]\nname = \"fixture\"\n\n[gates]\ncheck = []\n"
            } else {
                "[project]\nname = \"fixture\"\n\n[gates]\ncheck = [\"git --version\", \"git status --porcelain\"]\n"
            };
            std::fs::write(repo.join("devplane.toml"), gates).unwrap();

            git(&repo, &["add", "-A"]);
            git(&repo, &["commit", "--quiet", "-m", "fixture"]);
        }

        Self {
            repo,
            home,
            prior_home,
            _home_guard: guard,
            path_without_devplane: path_without_devplane(),
        }
    }

    /// Writes a file covered by a committed `.gitignore` entry and never added.
    pub fn gitignored(&self, path: &str, contents: &str) {
        let ignore = self.repo.join(".gitignore");
        let mut lines = std::fs::read_to_string(&ignore).unwrap_or_default();
        // Ignore the top directory when there is one, as people usually do.
        let entry = match path.split_once('/') {
            Some((dir, _)) => format!("{dir}/"),
            None => path.to_string(),
        };
        if !lines.lines().any(|l| l == entry) {
            lines.push_str(&entry);
            lines.push('\n');
            std::fs::write(&ignore, lines).unwrap();
            git(&self.repo, &["add", ".gitignore"]);
            git(&self.repo, &["commit", "--quiet", "-m", "ignore"]);
        }
        let file = self.repo.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file, contents).unwrap();
    }

    /// The Devplane home inside the sandbox.
    pub fn devplane_home(&self) -> PathBuf {
        self.home.join(".devplane")
    }

    /// Removes the Devplane home; an absent home is already removed.
    pub fn remove_devplane(&self) {
        let h = self.devplane_home();
        if h.exists() {
            std::fs::remove_dir_all(&h).expect("removing the sandbox devplane home");
        }
    }

    /// Runs a shell command with no Devplane in the environment or on `PATH`.
    pub fn plain_shell(&self, command: &str) -> std::process::Output {
        let mut c = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", command]);
            c
        };
        c.current_dir(&self.repo);
        // A command that could still reach Devplane would prove nothing.
        for (k, _) in std::env::vars() {
            if k.starts_with("DEVPLANE") || k.starts_with("CLAUDE") {
                c.env_remove(k);
            }
        }
        c.env("PATH", &self.path_without_devplane);
        c.output().expect("a shell")
    }

    /// Asserts `HOME` still points inside the sandbox and is distinct from the
    /// real home.
    pub fn assert_contained(&self) {
        assert!(
            std::env::var("HOME").is_ok_and(|h| std::path::Path::new(&h).starts_with(&self.home)),
            "HOME escaped the sandbox mid-run"
        );
        if let Some(real) = &self.prior_home {
            let theirs = std::path::Path::new(real).join(".devplane");
            assert!(
                !theirs.starts_with(&self.home),
                "the sandbox home and the real home are the same directory"
            );
        }
    }

    /// Every file git tracks, as repository-relative paths.
    pub fn tracked_files(&self) -> Vec<String> {
        let out = std::process::Command::new("git")
            .args(["ls-files"])
            .current_dir(&self.repo)
            .output()
            .expect("git ls-files");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// The declared gate commands, read as plain TOML rather than via Devplane.
    /// `None` without a file; `Some(empty)` when it declares no checks.
    pub fn declared_gates(&self) -> Option<Vec<String>> {
        let text = std::fs::read_to_string(self.repo.join("devplane.toml")).ok()?;
        let doc: toml::Value = toml::from_str(&text).expect("the fixture's devplane.toml parses");
        Some(
            doc.get("gates")
                .and_then(|g| g.get("check"))
                .and_then(|c| c.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        )
    }

    /// Runs the real host once against the sandbox home and stops it, so the
    /// home holds what a host actually writes.
    pub fn populate_home(&self) {
        let home = self.devplane_home();
        std::fs::create_dir_all(&home).expect("a devplane home");
        // Port 0: test binaries run in parallel.
        let mut host = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("serve")
            .arg("--port")
            .arg("0")
            .env("DEVPLANE_HOME", &home)
            .env("DEVPLANE_NOTIFY", "0")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("a host");
        // Wait for the host to publish its address.
        let published = || home.join("host.json").exists();
        for _ in 0..200 {
            if published() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            published(),
            "the host never published where it is listening"
        );
        // Reap the child, or `quit` waits on a zombie that answers `kill(pid, 0)`.
        let reaper = std::thread::spawn(move || host.wait());
        let quit = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
            .arg("quit")
            .env("DEVPLANE_HOME", &home)
            .output()
            .expect("quit runs");
        assert!(
            quit.status.success(),
            "quit failed: {}",
            String::from_utf8_lossy(&quit.stderr)
        );
        reaper.join().expect("the reaper thread").expect("wait");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // SAFETY: as in `new`; the sandbox still holds `home_lock`.
        unsafe {
            match &self.prior_home {
                Some(h) => std::env::set_var("HOME", h),
                None => std::env::remove_var("HOME"),
            }
        }
        let _ = std::fs::remove_dir_all(&self.repo);
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// A repository whose branch `feat/review` changes one file of each role, one
/// only by whitespace, and one unrelated file. `main` has a passing gate, no
/// `[review]`, a spec, and `.devplane/` ignored. Returns the canonical root
/// (macOS temp dirs sit behind a symlink).
pub fn review_repo(tag: &str) -> PathBuf {
    let dir = scratch(&format!("review-{tag}"));
    let write = |rel: &str, text: &str| {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().expect("a parent")).expect("a directory");
        std::fs::write(p, text).expect("a fixture file");
    };
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    write("devplane.toml", "[gates]\ncheck = [\"true\"]\n");
    write(".gitignore", ".devplane/\n.claude/\n");
    write(
        "src/types/user.rs",
        "pub struct User {\n    pub name: String,\n}\n",
    );
    write(
        "src/core/rules.rs",
        "pub fn allowed(n: u32) -> bool {\n    n < 3\n}\n",
    );
    write(
        "src/auth/session.rs",
        "pub fn open() -> bool {\n    true\n}\n",
    );
    write(
        "src/routes/login.rs",
        "pub fn route() -> &'static str {\n    \"/login\"\n}\n",
    );
    write("tests/auth.rs", "#[test]\nfn opens() {}\n");
    write("README.md", "# Fixture\n\nA repository to review.\n");
    write(
        "src/util/fmt.rs",
        "pub fn pad(s: &str) -> String {\n    format!(\"[{s}]\")\n}\n",
    );
    write("src/unrelated.rs", "pub fn helper() -> u8 {\n    1\n}\n");
    write(
        "specs/001-review/spec.md",
        "# Sessions\n\n## Requirements\n\n- FR-001 sessions open\n",
    );
    write(
        "specs/001-review/tasks.md",
        "# Tasks\n\n- [ ] T001 Open a session (FR-001)\n- [ ] T002 Close a session (FR-001)\n",
    );
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "base"]);

    git(&dir, &["checkout", "-qb", "feat/review"]);
    write(
        "src/types/user.rs",
        "pub struct User {\n    pub name: String,\n    pub email: String,\n}\n",
    );
    write(
        "src/core/rules.rs",
        "pub fn allowed(n: u32) -> bool {\n    n < 5\n}\n",
    );
    write(
        "src/auth/session.rs",
        "pub fn open(token: &str) -> bool {\n    !token.is_empty()\n}\n",
    );
    write(
        "src/routes/login.rs",
        "pub fn route() -> &'static str {\n    \"/v2/login\"\n}\n",
    );
    write(
        "tests/auth.rs",
        "#[test]\nfn opens() {}\n\n#[test]\nfn refuses_empty() {}\n",
    );
    write(
        "README.md",
        "# Fixture\n\nA repository to review, and to read.\n",
    );
    // Whitespace and nothing else: a formatter's hunk.
    write(
        "src/util/fmt.rs",
        "pub fn pad(s: &str) -> String {\n        format!( \"[{s}]\" )\n}\n",
    );
    write("src/unrelated.rs", "pub fn helper() -> u8 {\n    2\n}\n");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "the branch under review"]);
    git(&dir, &["checkout", "-q", "main"]);
    dir.canonicalize().expect("a canonical root")
}

/// Two registered, trusted projects: `api`, and `core-lib` with a GitHub
/// `origin`. A project is named by its directory, hence the shared scratch
/// parent. Each has one commit, a passing gate, and `.devplane/` ignored.
pub struct TwoProjects {
    pub api: PathBuf,
    pub core: PathBuf,
    pub api_id: devplane::core::ProjectId,
    pub core_id: devplane::core::ProjectId,
    parent: PathBuf,
}

impl Drop for TwoProjects {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.parent);
    }
}

pub async fn two_projects(state: &devplane::host::Shared) -> TwoProjects {
    let parent = scratch("two");
    let make = |name: &str, remote: Option<&str>| -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).expect("a project directory");
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["config", "user.email", "t@example.com"]);
        git(&dir, &["config", "user.name", "Test"]);
        std::fs::write(dir.join("devplane.toml"), "[gates]\ncheck = [\"true\"]\n").unwrap();
        std::fs::write(dir.join(".gitignore"), ".devplane/\n.claude/\n").unwrap();
        std::fs::write(dir.join("README.md"), format!("# {name}\n")).unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "init"]);
        if let Some(url) = remote {
            git(&dir, &["remote", "add", "origin", url]);
        }
        dir.canonicalize().expect("a canonical root")
    };
    let api = make("api", None);
    let core = make("core-lib", Some("https://github.com/acme/core-lib.git"));
    let mut ids = Vec::new();
    for root in [&api, &core] {
        let mut p = devplane::core::Project::from_root(root.clone());
        p.trusted = true;
        let id = {
            let mut w = state.world.lock().await;
            let id = w.upsert_project(p);
            w.trust(&id);
            id
        };
        let saved = state
            .world
            .lock()
            .await
            .project(&id)
            .cloned()
            .expect("registered");
        state
            .store
            .save_project(&saved)
            .await
            .expect("the project row");
        ids.push(id);
    }
    TwoProjects {
        api,
        core,
        api_id: ids.remove(0),
        core_id: ids.remove(0),
        parent,
    }
}

/// Rewrites and commits `devplane.toml`, keeping the tree clean.
pub fn configure(root: &Path, toml: &str) {
    std::fs::write(root.join("devplane.toml"), toml).unwrap();
    git(root, &["add", "devplane.toml"]);
    git(root, &["commit", "-qm", "configure"]);
}

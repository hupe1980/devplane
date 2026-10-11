//! Delete Devplane and the project still builds, the specs still read, and
//! nothing tracked depends on the Devplane home. Runs in a sandbox and never
//! launches a vendor agent. Planted-violation tests prove the harness can fail.

mod common;
#[path = "common/sandbox.rs"]
mod sandbox;

use sandbox::{Sandbox, Shape};
use std::time::{Duration, Instant};

/// The harness's time budget.
const BUDGET: Duration = Duration::from_secs(30);

/// One assertion's result. `Skipped` ("could not look") is distinct from
/// `Passed` ("looked and found nothing").
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Passed,
    /// A finding: the subject, and what was wrong with it.
    Finding {
        subject: String,
        detail: String,
    },
    /// The assertion could not be made. Named, never silent.
    Skipped {
        why: String,
    },
}

#[derive(Debug)]
struct Check {
    name: &'static str,
    outcome: Outcome,
}

/// "n checks, m findings, k skipped", then each finding and skip.
fn report(checks: &[Check]) -> String {
    let findings: Vec<_> = checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Finding { .. }))
        .collect();
    let skipped: Vec<_> = checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Skipped { .. }))
        .collect();
    let mut s = format!(
        "{} checks, {} findings, {} skipped",
        checks.len(),
        findings.len(),
        skipped.len()
    );
    for c in &findings {
        if let Outcome::Finding { subject, detail } = &c.outcome {
            s.push_str(&format!("\n  FINDING {}: {subject} — {detail}", c.name));
        }
    }
    for c in &skipped {
        if let Outcome::Skipped { why } = &c.outcome {
            s.push_str(&format!("\n  SKIPPED {}: {why}", c.name));
        }
    }
    s
}

fn findings(checks: &[Check]) -> Vec<&Check> {
    checks
        .iter()
        .filter(|c| matches!(c.outcome, Outcome::Finding { .. }))
        .collect()
}

/// A gate is unrunnable only if its program does not resolve on `PATH` or it
/// needs a Devplane-only environment variable. A gate that runs and fails is fine.
fn gate_is_runnable(s: &Sandbox, command: &str) -> Outcome {
    if command.contains("DEVPLANE") {
        return Outcome::Finding {
            subject: command.into(),
            detail: "depends on an environment variable only Devplane sets".into(),
        };
    }
    let program = command.split_whitespace().next().unwrap_or_default();
    let probe = if cfg!(windows) {
        format!("where {program}")
    } else {
        format!("command -v {program}")
    };
    if !s.plain_shell(&probe).status.success() {
        return Outcome::Finding {
            subject: program.into(),
            detail: "is not resolvable on PATH from a plain shell".into(),
        };
    }
    // Run it. Exit code is the project's business, not ours.
    let _ = s.plain_shell(command);
    Outcome::Passed
}

fn check_gates(s: &Sandbox) -> Check {
    let outcome = match s.declared_gates() {
        // Nothing was checked is not success.
        Some(g) if g.is_empty() => Outcome::Skipped {
            why: "no gates declared, so nothing was checked — this is not a pass".into(),
        },
        None => Outcome::Skipped {
            why: "no devplane.toml, so the project declares no checks".into(),
        },
        Some(gates) => gates
            .iter()
            .map(|c| gate_is_runnable(s, c))
            .find(|o| !matches!(o, Outcome::Passed))
            .unwrap_or(Outcome::Passed),
    };
    Check {
        name: "gates run from a plain shell",
        outcome,
    }
}

/// The three spec layouts in the field, as a root and the shape of a change
/// folder under it. Spec Kit numbers its folders; the other two do not.
const SPEC_LAYOUTS: &[(&str, &str)] = &[
    ("specs/", "NNN-*/"),
    ("openspec/changes/", "*/"),
    (".kiro/specs/", "*/"),
];

/// A change folder reads if it holds a Markdown document. No layout at all is
/// skipped, never passed.
fn check_spec_folder(s: &Sandbox) -> Check {
    let name = "the specification folder reads";
    let present: Vec<&(&str, &str)> = SPEC_LAYOUTS
        .iter()
        .filter(|(root, _)| s.repo.join(root).is_dir())
        .collect();
    if present.is_empty() {
        return Check {
            name,
            outcome: Outcome::Skipped {
                why: "skipped, no spec layout found (specs/, openspec/changes/ or .kiro/specs/), \
                      so its validity was not asserted"
                    .into(),
            },
        };
    }
    for (root, shape) in present {
        let numbered = shape.starts_with("NNN-");
        let readable = std::fs::read_dir(s.repo.join(root))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                !numbered
                    || n.chars().take(3).all(|c| c.is_ascii_digit()) && n.get(3..4) == Some("-")
            })
            .any(|e| {
                std::fs::read_dir(e.path())
                    .into_iter()
                    .flatten()
                    .flatten()
                    .any(|f| f.path().extension().is_some_and(|x| x == "md"))
            });
        if !readable {
            return Check {
                name,
                outcome: Outcome::Finding {
                    subject: root.to_string(),
                    detail: format!(
                        "exists but holds no {shape} change folder with a Markdown document"
                    ),
                },
            };
        }
    }
    Check {
        name,
        outcome: Outcome::Passed,
    }
}

/// Whether a tracked path is something a build, test or spec reads; prose and
/// docs are not dependencies.
fn is_dependency(rel: &str) -> bool {
    const DOCS: &[&str] = &[".md", ".mdx", ".txt", ".adoc"];
    if DOCS.iter().any(|e| rel.ends_with(e)) {
        return false;
    }
    !rel.starts_with("site/") && !rel.starts_with("docs/") && !rel.starts_with(".github/")
}

/// Nothing the project depends on may point into the Devplane home.
fn check_no_home_references(s: &Sandbox) -> Check {
    let needles = [".devplane/", "~/.devplane"];
    for rel in s.tracked_files().into_iter().filter(|r| is_dependency(r)) {
        let Ok(text) = std::fs::read_to_string(s.repo.join(&rel)) else {
            continue; // binary, or gone: not a claim about the home
        };
        for (i, line) in text.lines().enumerate() {
            if needles.iter().any(|n| line.contains(n)) {
                return Check {
                    name: "no tracked file references the Devplane home",
                    outcome: Outcome::Finding {
                        subject: format!("{rel}:{}", i + 1),
                        detail: format!("references the Devplane home: {}", line.trim()),
                    },
                };
            }
        }
    }
    Check {
        name: "no tracked file references the Devplane home",
        outcome: Outcome::Passed,
    }
}

/// Every file the home may hold: history, host bookkeeping and configuration.
/// `.bak` files (old schemas set aside) are also allowed.
const HOME_ALLOWED: &[&str] = &[
    "devplane.db",
    "devplane.db-wal",
    "devplane.db-shm",
    "token",
    // Derived from `token` and rewritten from it on every load.
    "telemetry-token",
    "agents.toml",
    "policy.toml",
    "trusted.toml",
    "app.toml",
    "host.json",
    "host.lock",
    "pending-decisions.jsonl",
];

/// The home holds only allowlisted files. An empty home is skipped: a scan over
/// nothing proves nothing.
fn check_home_contents(s: &Sandbox) -> Check {
    let name = "the home holds only history and configuration";
    let entries: Vec<String> = std::fs::read_dir(s.devplane_home())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    if entries.is_empty() {
        return Check {
            name,
            outcome: Outcome::Skipped {
                why: "the home was never written to, so nothing was checked".into(),
            },
        };
    }
    let strays: Vec<String> = entries
        .into_iter()
        .filter(|n| !HOME_ALLOWED.contains(&n.as_str()) && !n.ends_with(".bak"))
        .collect();
    Check {
        name,
        outcome: if strays.is_empty() {
            Outcome::Passed
        } else {
            Outcome::Finding {
                subject: strays.join(", "),
                detail: "is in the Devplane home and is neither history nor configuration".into(),
            }
        },
    }
}

/// What a preference would be called if it were stored: a person's choices
/// each live in a file named for them (`policy.toml`, `agents.toml`,
/// `app.toml`, `trusted.toml`), so no table or column of the store may carry
/// one of these words.
const PREFERENCE_WORDS: &[&str] = &["trust", "polic", "preference", "setting", "config"];

/// Every table and column the store at `db` has, as `table.column`.
fn store_columns(db: &std::path::Path) -> Vec<String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(db).read_only(true))
            .await
            .expect("the store opens");
        let tables: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT name, sql FROM sqlite_master WHERE type = 'table'")
                .fetch_all(&pool)
                .await
                .expect("the tables");
        // SQLite's own tables, and the shadow tables a full-text index keeps
        // (`events_fts_config` is the index's, not a person's).
        let virtual_tables: Vec<&str> = tables
            .iter()
            .filter(|(_, sql)| {
                sql.as_deref()
                    .is_some_and(|q| q.starts_with("CREATE VIRTUAL TABLE"))
            })
            .map(|(t, _)| t.as_str())
            .collect();
        let sqlites = |t: &str| {
            t.starts_with("sqlite_")
                || virtual_tables
                    .iter()
                    .any(|v| t.strip_prefix(v).is_some_and(|r| r.starts_with('_')))
        };
        let mut out = Vec::new();
        for (t, _) in tables.iter().filter(|(t, _)| !sqlites(t)) {
            let cols: Vec<(String,)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT name FROM pragma_table_info('{t}')"
            )))
            .fetch_all(&pool)
            .await
            .expect("the columns");
            out.push(t.clone());
            out.extend(cols.into_iter().map(|(c,)| format!("{t}.{c}")));
        }
        pool.close().await;
        out
    })
}

/// The store holds history and no preference: no table or column of it is
/// named for one. Skipped, by name, when the home holds no store.
fn check_store_holds_no_preference(s: &Sandbox) -> Check {
    let name = "the store holds no preference";
    let db = s.devplane_home().join("devplane.db");
    if !db.exists() {
        return Check {
            name,
            outcome: Outcome::Skipped {
                why: "the home holds no store, so nothing was checked".into(),
            },
        };
    }
    let found: Vec<String> = store_columns(&db)
        .into_iter()
        .filter(|c| {
            let lower = c.to_lowercase();
            PREFERENCE_WORDS.iter().any(|w| lower.contains(w))
        })
        .collect();
    Check {
        name,
        outcome: match found.is_empty() {
            true => Outcome::Passed,
            false => Outcome::Finding {
                subject: found.join(", "),
                detail: "is a preference kept in the store; it belongs in a file named for it"
                    .into(),
            },
        },
    }
}

/// Trusts the sandbox's repository the way a person does, in a terminal of
/// their own: no agent session in the environment.
fn trust_from_a_terminal(s: &Sandbox) {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"));
    c.args(["project", "trust", "--yes"])
        .arg(&s.repo)
        .env("DEVPLANE_HOME", s.devplane_home())
        .env("DEVPLANE_NOTIFY", "0");
    for v in devplane::hook::AGENT_SESSION_VARS {
        c.env_remove(v);
    }
    let out = c.output().expect("devplane project trust runs");
    assert!(
        out.status.success(),
        "trust failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// No throwaway checkout of base checks outlives its pair: neither git nor
/// the file system holds one once Devplane is gone.
fn check_no_base_checkout(s: &Sandbox) -> Check {
    let name = "no base-check checkout survives";
    let out = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&s.repo)
        .output();
    let Some(out) = out.ok().filter(|o| o.status.success()) else {
        return Check {
            name,
            outcome: Outcome::Skipped {
                why: "git could not list the worktrees, so nothing was checked".into(),
            },
        };
    };
    let listed = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains(".devplane-base-"))
        .map(|l| l.trim_start_matches("worktree ").to_string())
        .collect::<Vec<_>>();
    let on_disk = std::fs::read_dir(s.repo.join(".claude/worktrees"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with(".devplane-base-"))
        .collect::<Vec<_>>();
    let left: Vec<String> = listed.into_iter().chain(on_disk).collect();
    Check {
        name,
        outcome: match left.is_empty() {
            true => Outcome::Passed,
            false => Outcome::Finding {
                subject: left.join(", "),
                detail: "is a base-check checkout that outlived its pair".into(),
            },
        },
    }
}

/// The checks that run once Devplane is gone (the home is checked before).
fn run_all(s: &Sandbox) -> Vec<Check> {
    vec![
        check_gates(s),
        check_spec_folder(s),
        check_no_home_references(s),
        check_no_base_checkout(s),
    ]
}

/// Base checks ran in a throwaway checkout — `devplane verify` on a branch
/// that edits a test — and none is left once Devplane is gone.
#[test]
fn no_base_checkout_survives_base_checks() {
    let s = Sandbox::new(Shape::Full);
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&s.repo)
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["checkout", "-q", "-b", "feat/tests"]);
    std::fs::create_dir_all(s.repo.join("tests")).unwrap();
    std::fs::write(s.repo.join("tests/smoke_test.sh"), "git --version\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "a test"]);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["verify", "--base", "main", "--json"])
        .current_dir(&s.repo)
        .env("DEVPLANE_HOME", s.devplane_home())
        .env_remove("GITHUB_STEP_SUMMARY")
        .env_remove("GITHUB_EVENT_NAME")
        .output()
        .expect("verify runs");
    let text = String::from_utf8_lossy(&out.stdout);
    // A guard that compared nothing proves nothing: the pair must have run.
    assert!(
        text.contains("onChange") || text.contains("on_change"),
        "base checks did not run, so there was no checkout to look for: {text}"
    );
    s.remove_devplane();
    let checks = [check_no_base_checkout(&s)];
    assert_eq!(checks[0].outcome, Outcome::Passed, "{}", report(&checks));
}

/// Planted: a checkout a stopped pair left behind is caught.
#[test]
fn a_planted_base_checkout_is_caught() {
    let s = Sandbox::new(Shape::Full);
    let dir = s
        .repo
        .join(".claude/worktrees/.devplane-base-0123456789ab-01234567");
    let out = std::process::Command::new("git")
        .args(["worktree", "add", "--detach", "-q"])
        .arg(&dir)
        .arg("main")
        .current_dir(&s.repo)
        .output()
        .expect("git");
    assert!(out.status.success());
    assert!(matches!(
        check_no_base_checkout(&s).outcome,
        Outcome::Finding { .. }
    ));
}

#[test]
fn the_project_still_works_with_devplane_removed() {
    let started = Instant::now();
    let s = Sandbox::new(Shape::Full);
    // Check what a real host wrote before removing it.
    s.populate_home();
    let home = check_home_contents(&s);
    assert_ne!(
        home.outcome,
        Outcome::Skipped {
            why: "the home was never written to, so nothing was checked".into()
        },
        "the host wrote nothing, so the home check compared nothing"
    );
    s.remove_devplane();

    let mut checks = vec![home];
    checks.extend(run_all(&s));
    let r = report(&checks);
    assert!(
        findings(&checks).is_empty(),
        "a project Devplane governed stopped working when Devplane was removed:\n{r}"
    );
    assert!(
        started.elapsed() < BUDGET,
        "the harness took {:?}, over its {BUDGET:?} budget — a slow test is a disabled test",
        started.elapsed()
    );
    println!("{r}");
}

/// Removing an already-absent home is not an error.
#[test]
fn an_absent_home_is_already_removed_rather_than_an_error() {
    let s = Sandbox::new(Shape::Full);
    s.remove_devplane();
    s.remove_devplane(); // twice: this must not panic
    assert!(findings(&run_all(&s)).is_empty());
}

/// A missing spec folder is skipped by name, never passed.
#[test]
fn a_repository_with_no_spec_folder_skips_that_check_by_name() {
    let s = Sandbox::new(Shape::NoSpec);
    s.remove_devplane();
    let checks = run_all(&s);
    let spec = checks
        .iter()
        .find(|c| c.name == "the specification folder reads")
        .unwrap();
    assert!(
        matches!(&spec.outcome, Outcome::Skipped { why } if why.contains("no spec layout found")),
        "an absent spec folder must be skipped and named, not passed: {:?}",
        spec.outcome
    );
    assert!(
        report(&checks).contains("1 skipped"),
        "the count must be reported"
    );
}

/// Every supported spec layout reads; a present but empty one is a finding.
#[test]
fn every_spec_layout_in_the_field_reads() {
    let s = Sandbox::new(Shape::NoSpec);
    for (root, change) in [
        ("openspec/changes", "add-rate-limiting"),
        (".kiro/specs", "rate-limiting"),
        ("specs", "001-rate-limiting"),
    ] {
        let dir = s.repo.join(root).join(change);
        std::fs::create_dir_all(&dir).unwrap();
        // Present and empty: exists, reads nothing.
        let c = check_spec_folder(&s);
        assert!(
            matches!(&c.outcome, Outcome::Finding { subject, .. } if subject.starts_with(root)),
            "{root}/ with no document was not a finding: {:?}",
            c.outcome
        );
        std::fs::write(dir.join("tasks.md"), "- [ ] T001\n").unwrap();
        assert_eq!(check_spec_folder(&s).outcome, Outcome::Passed, "{root}");
        std::fs::remove_dir_all(s.repo.join(root.split('/').next().unwrap())).unwrap();
    }
    // Under `specs/`, only numbered folders are change folders.
    std::fs::create_dir_all(s.repo.join("specs/notes")).unwrap();
    std::fs::write(s.repo.join("specs/notes/x.md"), "x").unwrap();
    assert!(matches!(
        check_spec_folder(&s).outcome,
        Outcome::Finding { .. }
    ));
}

/// A real host's home passes the allowlist, and a planted stray is caught.
#[test]
fn the_home_holds_only_history_and_configuration() {
    let s = Sandbox::new(Shape::Full);
    s.populate_home();
    let c = check_home_contents(&s);
    assert_eq!(
        c.outcome,
        Outcome::Passed,
        "a host wrote something into the home that is neither history nor configuration"
    );
    // Guard against the pass above being vacuous.
    let names: Vec<String> = std::fs::read_dir(s.devplane_home())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(names.contains(&"devplane.db".to_string()), "{names:?}");
    assert!(names.contains(&"token".to_string()), "{names:?}");

    std::fs::write(s.devplane_home().join("cache.bin"), "x").unwrap();
    match &check_home_contents(&s).outcome {
        Outcome::Finding { subject, .. } => assert_eq!(subject, "cache.bin"),
        other => panic!("a planted stray in the home was not caught: {other:?}"),
    }
    std::fs::remove_file(s.devplane_home().join("cache.bin")).unwrap();
    std::fs::write(s.devplane_home().join("devplane.v1.bak"), "x").unwrap();
    assert_eq!(check_home_contents(&s).outcome, Outcome::Passed);

    s.remove_devplane();
    assert!(matches!(
        check_home_contents(&s).outcome,
        Outcome::Skipped { .. }
    ));
}

/// A person trusted the repository and a host ran: the trust is in the file
/// named for it, and the store holds no preference. A planted column is
/// caught, so the pass is not vacuous.
#[test]
fn the_store_holds_no_preference() {
    let s = Sandbox::new(Shape::Full);
    trust_from_a_terminal(&s);
    s.populate_home();
    let trusted =
        std::fs::read_to_string(s.devplane_home().join("trusted.toml")).expect("trusted.toml");
    let root = s.repo.canonicalize().unwrap();
    assert!(
        trusted.contains(&*root.to_string_lossy()),
        "the trust is in its own file: {trusted}"
    );
    assert_eq!(
        check_store_holds_no_preference(&s).outcome,
        Outcome::Passed,
        "{:?}",
        store_columns(&s.devplane_home().join("devplane.db"))
    );
    assert_eq!(check_home_contents(&s).outcome, Outcome::Passed);

    s.remove_devplane();
    assert!(matches!(
        check_store_holds_no_preference(&s).outcome,
        Outcome::Skipped { .. }
    ));
}

/// A store with a preference column is caught by name.
#[test]
fn a_planted_preference_in_the_store_is_caught() {
    let s = Sandbox::new(Shape::Full);
    s.populate_home();
    let db = s.devplane_home().join("devplane.db");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&db))
            .await
            .unwrap();
        sqlx::query("ALTER TABLE projects ADD COLUMN trusted INTEGER NOT NULL DEFAULT 0")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    });
    match &check_store_holds_no_preference(&s).outcome {
        Outcome::Finding { subject, .. } => assert_eq!(subject, "projects.trusted"),
        other => panic!("a planted preference in the store was not caught: {other:?}"),
    }
}

/// No declared gates is skipped, not reported as passing.
#[test]
fn a_project_with_no_gates_does_not_report_that_gates_passed() {
    let s = Sandbox::new(Shape::NoGates);
    s.remove_devplane();
    let checks = run_all(&s);
    let gates = checks
        .iter()
        .find(|c| c.name == "gates run from a plain shell")
        .unwrap();
    assert!(
        matches!(&gates.outcome, Outcome::Skipped { why } if why.contains("not a pass")),
        "no gates declared must not read as every gate passed: {:?}",
        gates.outcome
    );
}

/// A `devplane.toml` the window wrote — the person's confirmed commands under
/// an origin comment — is as inert as one written by hand: with the home gone,
/// its gates run from a plain shell.
#[test]
fn a_check_declared_from_the_window_runs_with_devplane_removed() {
    let s = Sandbox::new(Shape::NoGates);
    let existing = std::fs::read_to_string(s.repo.join("devplane.toml")).ok();
    let declared = devplane::core::declare::Declared {
        check: vec!["true".into(), "sh -c 'exit 0'".into()],
        named: Default::default(),
    };
    let text = devplane::core::declare::splice(existing.as_deref(), &declared).unwrap();
    std::fs::write(s.repo.join("devplane.toml"), &text).unwrap();
    s.remove_devplane();
    let checks = run_all(&s);
    let gates = checks
        .iter()
        .find(|c| c.name == "gates run from a plain shell")
        .unwrap();
    assert_eq!(gates.outcome, Outcome::Passed, "{text}");
    assert!(
        !text.contains(".devplane"),
        "no repository file points at the home: {text}"
    );
}

/// A repository with no commits yields no findings.
#[test]
fn a_repository_with_no_commits_is_a_supported_state() {
    let s = Sandbox::new(Shape::NoCommits);
    s.remove_devplane();
    let checks = run_all(&s);
    assert!(
        findings(&checks).is_empty(),
        "an uncommitted repository is a state, not a failure:\n{}",
        report(&checks)
    );
}

/// A tracked file referencing the home is caught, with file and line.
#[test]
fn a_planted_home_reference_is_caught() {
    let s = Sandbox::new(Shape::Full);
    std::fs::write(
        s.repo.join("config.toml"),
        "cache = \"~/.devplane/build\"\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["add", "config.toml"])
        .current_dir(&s.repo)
        .output()
        .unwrap();
    s.remove_devplane();

    let c = check_no_home_references(&s);
    match &c.outcome {
        Outcome::Finding { subject, detail } => {
            assert!(
                subject.starts_with("config.toml:"),
                "must name the file and line: {subject}"
            );
            assert!(detail.contains(".devplane"), "must quote what it found");
        }
        other => panic!("a planted home reference was not caught: {other:?}"),
    }
}

/// A dependency on the home is a defect; prose naming it is not.
#[test]
fn documentation_naming_the_home_is_not_a_dependency() {
    assert!(!is_dependency("CHANGELOG.md"), "a changelog is prose");
    assert!(
        !is_dependency("site/content/docs/install.md"),
        "docs are prose"
    );
    assert!(
        !is_dependency(".github/ISSUE_TEMPLATE/x.md"),
        "a template is prose"
    );
    assert!(is_dependency("devplane.toml"), "configuration is read");
    assert!(is_dependency("scripts/build.sh"), "a script is run");
    assert!(is_dependency("Cargo.toml"), "a manifest is read");
}

/// A gate that invokes Devplane itself is caught, since the binary is off `PATH`.
#[test]
fn a_gate_that_invokes_devplane_itself_is_caught() {
    let s = Sandbox::new(Shape::Full);
    std::fs::write(
        s.repo.join("devplane.toml"),
        "[project]\nname = \"fixture\"\n\n[gates]\ncheck = [\"devplane gate\"]\n",
    )
    .unwrap();
    s.remove_devplane();

    match &check_gates(&s).outcome {
        Outcome::Finding { subject, detail } => {
            assert_eq!(subject, "devplane");
            assert!(detail.contains("not resolvable on PATH"));
        }
        other => panic!("a gate depending on Devplane itself was not caught: {other:?}"),
    }
}

/// The harness mutates only its own temporary directories.
#[test]
fn the_harness_never_leaves_its_sandbox() {
    let s = Sandbox::new(Shape::Full);
    s.assert_contained();
    s.remove_devplane();
    let _ = run_all(&s);
    s.assert_contained();
}

/// A gate whose program does not exist is caught.
#[test]
fn a_planted_unrunnable_gate_is_caught() {
    let s = Sandbox::new(Shape::Full);
    std::fs::write(
        s.repo.join("devplane.toml"),
        "[project]\nname = \"fixture\"\n\n[gates]\ncheck = [\"definitely-not-a-real-program --x\"]\n",
    )
    .unwrap();
    s.remove_devplane();

    match &check_gates(&s).outcome {
        Outcome::Finding { subject, detail } => {
            assert_eq!(subject, "definitely-not-a-real-program");
            assert!(detail.contains("not resolvable on PATH"));
        }
        other => panic!("a gate that cannot run was not caught: {other:?}"),
    }
}

/// A gate that runs and fails is not a finding.
#[test]
fn a_gate_that_runs_and_fails_is_not_a_removability_defect() {
    let s = Sandbox::new(Shape::Full);
    std::fs::write(
        s.repo.join("devplane.toml"),
        "[project]\nname = \"fixture\"\n\n[gates]\ncheck = [\"git rev-parse --verify no-such-ref\"]\n",
    )
    .unwrap();
    s.remove_devplane();
    assert_eq!(
        check_gates(&s).outcome,
        Outcome::Passed,
        "a red gate means the project's own check ran and said no — that is removability working"
    );
}

/// The planted-violation tests exist, are `#[test]`s, and are not ignored.
#[test]
fn the_planted_violations_are_part_of_this_suite() {
    let source = include_str!("removable.rs");
    for planted in [
        "a_planted_home_reference_is_caught",
        "a_planted_unrunnable_gate_is_caught",
        "a_planted_base_checkout_is_caught",
        "a_planted_preference_in_the_store_is_caught",
    ] {
        let Some(at) = source.find(&format!("\nfn {planted}(")) else {
            panic!("{planted} was removed — this suite no longer demonstrates that it can fail");
        };
        // From the nearest `#[test]` above to the `fn`; another `fn` between
        // means this one lost its attribute.
        let before = &source[..at];
        let attrs = &before[before.rfind("#[test]").unwrap_or(0)..];
        assert!(
            attrs.starts_with("#[test]") && !attrs.contains("\nfn "),
            "{planted} is no longer a test — this suite no longer demonstrates that it can fail"
        );
        assert!(
            !attrs.contains("#[ignore"),
            "{planted} is ignored — a planted violation that does not run proves nothing"
        );
    }
}

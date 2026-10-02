//! `devplane verify`: the edge. Reads git and the base's `devplane.toml`, runs
//! the base's `check` in the checkout, and writes the summary and statement a
//! person named. Needs only git and the repository's files: it names no store,
//! host, client or Devplane home, and makes no network request
//! (`tests/verify.rs` holds that to the source).
//!
//! Everything decided is in [`crate::core::verify`]; this module only gathers.

use crate::core::review::Weakened;
use crate::core::verify::json::Wrote;
use crate::core::verify::{
    Base, BaseChecks, BaseSource, ChangeDeclares, CouldNotRun, Definition, Run, Trees, Verdict,
};
use std::path::{Path, PathBuf};

/// The command's flags.
#[derive(Debug, Clone, Default)]
pub struct Args {
    pub base: Option<String>,
    pub json: bool,
    pub summary: Option<PathBuf>,
    pub statement: Option<PathBuf>,
    /// `--no-base-checks`: `check` alone, without the pair.
    pub no_base_checks: bool,
    /// `--sarif`: the weakened rows and failing named tests as SARIF 2.1.0.
    pub sarif: Option<PathBuf>,
}

/// Where the Markdown goes.
enum Destination {
    /// `--summary`: created or replaced.
    Replace(PathBuf),
    /// `$GITHUB_STEP_SUMMARY`: appended to.
    Append(PathBuf),
}

impl Destination {
    fn of(args: &Args) -> Option<Self> {
        if let Some(p) = &args.summary {
            return Some(Destination::Replace(p.clone()));
        }
        std::env::var_os("GITHUB_STEP_SUMMARY")
            .filter(|v| !v.is_empty())
            .map(|v| Destination::Append(PathBuf::from(v)))
    }

    fn name(&self) -> String {
        match self {
            Destination::Replace(p) => p.display().to_string(),
            Destination::Append(_) => "$GITHUB_STEP_SUMMARY".to_string(),
        }
    }

    fn path(&self) -> &Path {
        match self {
            Destination::Replace(p) | Destination::Append(p) => p,
        }
    }

    fn write(&self, text: &str) -> std::io::Result<()> {
        use std::io::Write;
        match self {
            Destination::Replace(p) => std::fs::write(p, text),
            Destination::Append(p) => {
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)?;
                f.write_all(b"\n")?;
                f.write_all(text.as_bytes())
            }
        }
    }
}

/// Everything gathered before the verdict; each step may stop with a reason.
#[derive(Default)]
struct Gathered {
    root: Option<PathBuf>,
    /// The change the rows were read from; `None` when it was not read.
    set: Option<crate::core::diff::ChangeSet>,
    base: Option<Base>,
    definition: Option<Definition>,
    rows: Vec<Weakened>,
    truncated: Option<crate::core::diff::Truncation>,
    no_change: bool,
    report: Option<crate::core::change::GateReport>,
    base_checks: Option<BaseChecks>,
}

/// Runs `verify` in `dir` and returns the exit code.
pub async fn run(dir: &Path, args: &Args) -> i32 {
    let mut g = Gathered::default();
    let verdict = match gather(dir, args, &mut g).await {
        Ok(v) => v,
        Err(why) => Verdict::CouldNotRun { why },
    };
    finish(dir, args, g, verdict).await
}

async fn gather(dir: &Path, args: &Args, g: &mut Gathered) -> Result<Verdict, CouldNotRun> {
    use crate::git;
    let root = git::toplevel(dir)
        .await
        .ok_or(CouldNotRun::NotARepository)?;
    g.root = Some(root.clone());

    // The base: asked for, else what the remote calls its default, else
    // `main` — never the checkout's own config, never its current branch.
    let (asked, asked_from) = match &args.base {
        Some(b) => (b.clone(), BaseSource::Flag),
        None => match git::default_branch(&root).await {
            Some(b) => (b, BaseSource::OriginHead),
            None => ("main".to_string(), BaseSource::Fallback),
        },
    };
    let resolved = git::resolve_commitish(&root, &asked).await.ok_or_else(|| {
        CouldNotRun::BaseUnresolvable {
            tried: vec![format!("origin/{asked}"), asked.clone()],
            fallback: asked_from == BaseSource::Fallback,
        }
    })?;
    let merge_base = match git::merge_base(&root, &asked).await {
        Ok((_, m)) => m,
        Err(_) => {
            return Err(CouldNotRun::NoSharedHistory {
                base: resolved,
                shallow: git::is_shallow(&root).await,
            });
        }
    };
    let head = git::commit_of(&root, "HEAD").await.unwrap_or_default();
    let forge_merge = std::env::var("GITHUB_EVENT_NAME").is_ok_and(|e| e == "pull_request")
        && std::env::var("GITHUB_SHA").is_ok_and(|s| !s.is_empty() && s == head);
    let base = Base {
        asked: asked.clone(),
        asked_from,
        resolved: resolved.clone(),
        merge_base: merge_base.clone(),
        head: head.clone(),
        forge_merge,
    };
    let at = base.short().to_string();
    g.base = Some(base);

    // The whole devplane.toml as committed at the merge base governs.
    let file = crate::core::verify::CONFIG_FILE;
    let bytes = git::read_at(&root, &merge_base, file)
        .await
        .map_err(|e| CouldNotRun::ConfigUnreadableAtBase {
            at: at.clone(),
            why: format!("{e:#}"),
        })?
        .ok_or_else(|| CouldNotRun::NoConfigAtBase { at: at.clone() })?;
    let text = String::from_utf8(bytes).map_err(|_| CouldNotRun::ConfigUnreadableAtBase {
        at: at.clone(),
        why: "it is not UTF-8".into(),
    })?;
    let config = crate::core::ProjectConfig::parse(&text).map_err(|why| {
        CouldNotRun::ConfigUnreadableAtBase {
            at: at.clone(),
            why,
        }
    })?;
    if config.gates.check.is_empty() {
        return Err(CouldNotRun::NoCheck { at });
    }
    // The checkout's own file is read only to say what it declares instead.
    let checkout = match std::fs::read_to_string(root.join(file)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(Err(e.to_string())),
        Ok(t) => Some(crate::core::ProjectConfig::parse(&t).map(|c| c.gates.check)),
    };
    let change_declares = ChangeDeclares::of(&config.gates.check, checkout);
    g.definition = Some(Definition {
        check: config.gates.check.clone(),
        timeout_secs: config.gates.timeout.as_secs(),
        file_blob: git::blob_at(&root, &merge_base, file).await,
        change_declares: change_declares.clone(),
        has_setup: config.workspace.setup.is_some(),
        has_named: !config.gates.named.is_empty(),
    });

    // The weakened rows, by the workbench's own function, under the base's
    // roles and gate commands.
    let set = git::change_set_from(&root, &merge_base, &asked, &resolved)
        .await
        .map_err(|e| CouldNotRun::DiffUnreadable {
            at: at.clone(),
            why: format!("{e:#}"),
        })?;
    g.rows = crate::view::weakened_by(&set, &config);
    crate::core::verify::ensure_definition_row(&mut g.rows, &change_declares);
    g.truncated = set.truncated.clone();
    g.no_change = merge_base == head && set.is_empty();
    g.set = Some(set);

    // The base's `check`, in the checkout's root, under the base's timeout,
    // with the tree read before and after.
    let before = git::digest(&root).await;
    let mut report = crate::gates::run(
        crate::core::change::CHECK,
        &config.gates.check,
        &root,
        config.gates.timeout,
        1,
        &crate::gates::GateInput {
            env: Vec::new(),
            junit: config.gates.junit.clone(),
        },
    )
    .await;
    report.base_commit = Some(merge_base.clone());
    let after = git::digest(&root).await;
    let moved = match (&before, &after) {
        (Ok(b), Ok(a)) => b.moved(a),
        _ => Vec::new(),
    };
    let trees = Trees {
        before: before.map(|d| d.tree).map_err(|e| format!("{e:#}")),
        after: after.map(|d| d.tree).map_err(|e| format!("{e:#}")),
        moved,
    };
    let verdict = Verdict::of(&report, &trees);

    // `#base-checks`, after the verdict and never into it: the base's tests
    // on this change, and this change's new tests on the base, in a
    // throwaway checkout removed when the pair ends.
    use crate::core::base_checks::{self as pure, Absent, View};
    let absent = if args.no_base_checks {
        Some(Absent::Declined)
    } else if config.gates.base_checks == crate::core::config::BaseChecksWhen::Off {
        Some(Absent::Off)
    } else {
        None
    };
    let view = match (absent, &verdict) {
        (Some(a), _) => View::absent(&report, a),
        (None, Verdict::Verified { tree }) => {
            let key = pure::Key {
                base: merge_base.clone(),
                tree: tree.clone(),
                definition: pure::definition(&config),
            };
            let result = crate::base_checks::run(&root, &root, &key, &config, Some(&report)).await;
            g.rows.extend(pure::rows(&result.on_change));
            View::of_result(&report, &result)
        }
        (None, _) => View::absent(&report, Absent::CheckNotPassing),
    };
    g.base_checks = Some(BaseChecks::Shown(Box::new(view)));
    g.report = Some(report);
    Ok(verdict)
}

async fn finish(dir: &Path, args: &Args, g: Gathered, mut verdict: Verdict) -> i32 {
    let checks = g.base_checks.clone().unwrap_or(BaseChecks::NotReached);
    let not_done = crate::core::verify::not_done(g.definition.as_ref());
    let mut wrote = Wrote::default();
    let mut errors: Vec<String> = Vec::new();

    // The statement first: a summary that says *verified* must not be the last
    // thing written when the statement beside it failed.
    if let (Some(path), Some(report), Some(base), Some(def)) =
        (&args.statement, &g.report, &g.base, &g.definition)
    {
        let name = std::fs::canonicalize(dir)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "checkout".into());
        let statement =
            crate::core::certificate::verify_statement(&crate::core::certificate::VerifyEvidence {
                name: &name,
                report,
                weakened: &g.rows,
                verify: serde_json::json!({
                    "base": base.asked,
                    "resolved": base.resolved,
                    "baseCommit": base.merge_base,
                    "head": base.head,
                    "definition": {
                        "path": crate::core::verify::CONFIG_FILE,
                        "at": base.merge_base,
                        "blob": def.file_blob,
                        "check": def.check,
                    },
                    "definitionChanged": !def.change_declares.is_same(),
                    "verdict": verdict.key(),
                }),
                base_checks: checks.json(),
                config_blob: def.file_blob.as_deref(),
            });
        // No tree, no subject, no statement: nothing is written, and the
        // document says why. A *verified* never stands beside a statement
        // that could not be made.
        let written = statement.map(|st| std::fs::write(path, crate::core::attest::serialize(&st)));
        match written {
            Err(why) => {
                errors.push(format!("{why} Nothing was written to {}.", path.display()));
                if matches!(verdict, Verdict::Verified { .. }) {
                    verdict = Verdict::CouldNotRun {
                        why: CouldNotRun::CouldNotWrite {
                            path: path.display().to_string(),
                            why: why.clone(),
                        },
                    };
                }
                wrote.statement_withheld = Some(why);
            }
            Ok(Ok(())) => wrote.statement = Some(path.display().to_string()),
            Ok(Err(e)) => {
                let why = e.to_string();
                errors.push(format!(
                    "the statement could not be written to {}: {why}",
                    path.display()
                ));
                if matches!(verdict, Verdict::Verified { .. }) {
                    verdict = Verdict::CouldNotRun {
                        why: CouldNotRun::CouldNotWrite {
                            path: path.display().to_string(),
                            why,
                        },
                    };
                }
            }
        }
    }

    // The same rows, placed on their files: annotations when GitHub Actions
    // reads this log, SARIF when asked. Neither changes the verdict's word.
    let root = g.root.clone().unwrap_or_else(|| dir.to_path_buf());
    let findings =
        crate::core::verify::annotate::findings(&g.rows, g.set.as_ref(), g.report.as_ref(), &|p| {
            root.join(p).is_file()
        });
    let on_github = std::env::var_os("GITHUB_ACTIONS").is_some_and(|v| !v.is_empty());
    let (commands, annotations) = match on_github {
        true => {
            let (c, n) = crate::core::verify::annotate::commands(&findings);
            (c, Some(n))
        }
        false => (Vec::new(), None),
    };
    if let Some(path) = &args.sarif {
        if g.set.is_some() {
            let log = crate::core::verify::annotate::sarif(&findings, env!("CARGO_PKG_VERSION"));
            let text = serde_json::to_string_pretty(&log).unwrap_or_else(|_| log.to_string());
            match std::fs::write(path, text + "\n") {
                Ok(()) => wrote.sarif = Some(path.display().to_string()),
                Err(e) => {
                    let why = e.to_string();
                    errors.push(format!(
                        "the SARIF log could not be written to {}: {why}",
                        path.display()
                    ));
                    if matches!(verdict, Verdict::Verified { .. }) {
                        verdict = Verdict::CouldNotRun {
                            why: CouldNotRun::CouldNotWrite {
                                path: path.display().to_string(),
                                why,
                            },
                        };
                    }
                }
            }
        } else {
            errors.push(format!(
                "the SARIF log was not written to {}: the change was not read, so there are no \
                 rows to place",
                path.display()
            ));
        }
    }

    let run = |verdict: &Verdict| -> String {
        crate::core::verify::summary::markdown(
            &Run {
                base: g.base.as_ref(),
                definition: g.definition.as_ref(),
                rows: &g.rows,
                truncated: g.truncated.as_ref(),
                no_change: g.no_change,
                report: g.report.as_ref(),
                verdict,
                base_checks: &checks,
                not_done: &not_done,
                annotations,
            },
            crate::core::verify::summary::LIMIT_BYTES,
        )
    };
    if let Some(dest) = Destination::of(args) {
        match dest.write(&run(&verdict)) {
            Ok(()) => wrote.summary = Some(dest.name()),
            Err(e) => {
                let why = e.to_string();
                errors.push(format!(
                    "the summary could not be written to {}: {why}",
                    dest.path().display()
                ));
                if matches!(verdict, Verdict::Verified { .. }) {
                    verdict = Verdict::CouldNotRun {
                        why: CouldNotRun::CouldNotWrite {
                            path: dest.path().display().to_string(),
                            why,
                        },
                    };
                }
            }
        }
    }

    let composed = Run {
        base: g.base.as_ref(),
        definition: g.definition.as_ref(),
        rows: &g.rows,
        truncated: g.truncated.as_ref(),
        no_change: g.no_change,
        report: g.report.as_ref(),
        verdict: &verdict,
        base_checks: &checks,
        not_done: &not_done,
        annotations,
    };
    if args.json {
        let doc = crate::core::verify::json::json(&composed, &wrote);
        println!(
            "{}",
            serde_json::to_string_pretty(&doc).unwrap_or_else(|_| doc.to_string())
        );
    } else {
        // Repository text is printed with workflow commands paused, so no
        // line a change wrote is read by the runner as a command.
        let pause = on_github.then(|| uuid::Uuid::new_v4().simple().to_string());
        if let Some(token) = &pause {
            println!("::stop-commands::{token}");
        }
        print!("{}", crate::core::verify::summary::text(&composed));
        match &wrote.summary {
            Some(s) => println!("{:<13}written to {s}", "summary"),
            None if !errors.is_empty() => {}
            None => println!(
                "{:<13}not written: pass `--summary <path>`, or run where \
                 $GITHUB_STEP_SUMMARY is set",
                "summary"
            ),
        }
        if let Some(s) = &wrote.statement {
            println!("statement    written to {s}");
        }
        if let Some(s) = &wrote.sarif {
            println!("{:<13}written to {s}", "sarif");
        }
        if let Some(token) = &pause {
            println!("::{token}::");
        }
        for c in &commands {
            println!("{c}");
        }
    }
    // Under `--json` stdout is the document; the runner reads stderr too.
    if args.json {
        for c in &commands {
            eprintln!("{c}");
        }
    }
    for e in &errors {
        eprintln!("{e}");
    }
    if let Verdict::CouldNotRun { why } = &verdict
        && args.json
    {
        eprintln!("could not run: {}", why.key());
    }
    verdict.exit_code()
}

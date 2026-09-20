//! `devplane library` — four verbs over artefacts in the vendors' own formats.
//!
//! Devplane owns verbs. It owns no nouns, and none of these four writes a
//! format of its own: `list` and `diff` read, `report` reads, `install` copies
//! bytes, `sync` copies bytes in a direction a person named.
//!
//! **Nothing here grades anything.** There is no *safe*, no score and no tick,
//! and a test asserts the absence across every verb's output. A green tick in
//! front of the case that needs a person is worse than no tick at all.

use crate::client;
use crate::core::library::{Drift, Portability, Refusal};
use crate::library::{self, Artefact, Kind};
use crate::render::{BOLD, DIM, paint};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// A project as the daemon reports it.
struct Project {
    name: String,
    root: PathBuf,
    trusted: bool,
}

/// The registered projects, from the daemon.
///
/// Asked of the running daemon rather than read off disk, so that *which
/// projects are there* has one answer on the board and in this command.
async fn projects() -> Result<Vec<Project>> {
    let c = client::Client::connect_or_start().await?;
    let v: serde_json::Value = c.get("/api/projects").await?;
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some(Project {
                        name: p["name"].as_str()?.to_string(),
                        root: PathBuf::from(p["root"].as_str()?),
                        trusted: p["trusted"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

fn find<'a>(all: &'a [Artefact], name: &str) -> Result<&'a Artefact> {
    all.iter().find(|a| a.name == name).with_context(|| {
        format!(
            "no artefact named `{name}` in {}",
            library::root()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        )
    })
}

/// `devplane library list` — everything this machine can reach.
pub async fn cmd_list(json: bool) -> Result<()> {
    let all = library::list()?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!(
                all.iter()
                    .map(|a| serde_json::json!({
                        "name": a.name,
                        "kind": match a.kind { Kind::Skill => "skill", Kind::Prompt => "prompt" },
                        "digest": a.digest.digest,
                        "origin": a.sidecar.as_ref().map(|s| s.origin.clone()),
                    }))
                    .collect::<Vec<_>>()
            ))?
        );
        return Ok(());
    }
    if all.is_empty() {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "The library is empty. It is a directory you own: {}",
                    library::root()?.display()
                )
            )
        );
        return Ok(());
    }
    for a in &all {
        let kind = match a.kind {
            Kind::Skill => "skill",
            Kind::Prompt => "prompt",
        };
        println!("{:<28}{:<8}{}", paint(BOLD, &a.name), kind, a.digest.digest);
        if let Some(s) = &a.sidecar {
            println!("{:<28}{}", "", paint(DIM, &s.origin));
        }
    }
    Ok(())
}

/// `devplane library diff [artefact]` — drift, coverage and documented
/// portability failures. **Reports; changes nothing.**
///
/// Exit code is `0` whatever it finds. A non-zero exit would make this a gate,
/// and nothing here grades anything.
pub async fn cmd_diff(name: Option<String>, json: bool) -> Result<()> {
    let all = library::list()?;
    let chosen: Vec<&Artefact> = match &name {
        Some(n) => vec![find(&all, n)?],
        None => all.iter().collect(),
    };
    let ps = projects().await?;
    let pairs: Vec<(String, PathBuf)> = ps
        .iter()
        .map(|p| (p.name.clone(), p.root.clone()))
        .collect();

    if json {
        let out: Vec<_> = chosen
            .iter()
            .map(|a| {
                let cov = library::coverage(a, &pairs);
                serde_json::json!({
                    "name": a.name,
                    "digest": a.digest.digest,
                    "copies": cov.iter().map(|c| serde_json::json!({
                        "project": c.project,
                        "path": c.path.display().to_string(),
                        "drift": c.drift,
                        "present": c.present,
                        "ignored": c.ignored,
                    })).collect::<Vec<_>>(),
                    "portability": library::portability_of(a),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    for a in chosen {
        println!("{}", paint(BOLD, &a.name));
        println!();
        let cov = library::coverage(a, &pairs);
        // Failures before successes: a list that buries the one red line under
        // five green ones is a list nobody reads twice.
        // Three tiers, not two: findings, then projects that hold a good copy,
        // then the ones that do not hold it at all. The third is the coverage
        // answer — *which of my six lack this* — and it read as `ok` until
        // somebody ran the command and saw eight projects reported fine for a
        // skill none of them had.
        let mut rows: Vec<_> = cov.iter().collect();
        rows.sort_by_key(|c| (!c.drift.is_finding(), c.present, c.project.clone()));
        for c in &rows {
            if c.drift.is_finding() {
                println!(
                    "  {:<11}{:<16}{}",
                    paint(BOLD, c.drift.label()),
                    c.project,
                    c.drift.says()
                );
            } else if c.present {
                println!("  {:<11}{}", paint(DIM, c.drift.label()), c.project);
            } else {
                println!(
                    "  {:<11}{:<16}{}",
                    paint(DIM, "absent"),
                    c.project,
                    paint(DIM, "no copy here")
                );
            }
        }
        let suppressed: Vec<String> = cov
            .iter()
            .flat_map(|c| c.ignored.iter().map(|i| format!("{}/{i}", c.project)))
            .collect();
        if !suppressed.is_empty() {
            println!();
            println!(
                "  suppressed: {} {}",
                suppressed.join(", "),
                paint(DIM, "(ignored by policy, not by accident)")
            );
        }
        print_portability(&library::portability_of(a));
        println!();
    }
    Ok(())
}

/// The portability half, rendered against **distribution paths** and never
/// against a vendor.
fn print_portability(p: &Portability) {
    if p.unread {
        println!();
        println!(
            "  {}",
            paint(
                BOLD,
                "unread   the frontmatter could not be read, so nothing below is a finding"
            )
        );
        println!("  {}", paint(DIM, Portability::CAVEAT));
        return;
    }
    if !p.findings.is_empty() {
        println!();
        println!("leaving Claude Code:");
        let paths = crate::core::library::DISTRIBUTION_PATHS.join(", ");
        // Padded to the widest field rather than to a fixed sixteen, and with
        // a guaranteed space after it. `disable-model-invocation` is 24
        // characters and ran straight into the next column — found by pointing
        // this at Spec Kit's own commands, which is where the long names are.
        let widest = p.findings.iter().map(|f| f.field.len()).max().unwrap_or(0);
        for f in &p.findings {
            println!(
                "  {:<7}{:<width$} {} on {}",
                paint(BOLD, "ERROR"),
                f.field,
                f.why.says(),
                paths,
                width = widest.max(14)
            );
        }
        println!(
            "  {:<7}{:<width$} allowed: {}",
            "",
            "",
            crate::core::library::PORTABLE_FIELDS.join(", "),
            width = widest.max(14)
        );
    }
    println!();
    println!("{}", paint(DIM, Portability::CAVEAT));
}

/// `devplane library report <artefact>` — what it will be allowed to do, and
/// where it came from. **No verdict, ever.**
pub async fn cmd_report(name: String, json: bool) -> Result<()> {
    let all = library::list()?;
    let a = find(&all, &name)?;
    let entry = match a.kind {
        Kind::Skill => a.path.join("SKILL.md"),
        Kind::Prompt => a.path.clone(),
    };
    let grants = std::fs::read_to_string(&entry)
        .ok()
        .and_then(|t| crate::core::setup::pre_approved_tools(&t));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "name": a.name,
                "path": a.path.display().to_string(),
                "installs_to": crate::library::scope_for(a.kind)
                    .path(std::path::Path::new("<project>"), &a.name)
                    .display()
                    .to_string(),
                "documented_by": crate::library::scope_for(a.kind).documented_by(),
                "origin": a.sidecar.as_ref().map(|s| s.origin.clone()),
                "digest": a.digest.digest,
                "installed": a.sidecar.as_ref().map(|s| s.installed.clone()),
                "by": a.sidecar.as_ref().map(|s| s.by.clone()),
                "pre_approves": grants,
                "also_from_this_origin": a.sidecar.as_ref()
                    .map(|s| library::from_origin(&s.origin, &all))
                    .unwrap_or_default(),
            }))?
        );
        return Ok(());
    }

    println!("{:<32}{}", paint(BOLD, &a.name), a.path.display());
    // Where a copy of this goes, and **who documents that path** — the line
    // that says why this directory and no other. Devplane writes only into
    // paths a vendor documents, and that claim is worth a sentence at the point
    // somebody is deciding whether to install it.
    let scope = crate::library::scope_for(a.kind);
    println!(
        "{:<32}{}",
        "",
        paint(
            DIM,
            &format!(
                "installs to {} — {}",
                scope
                    .path(std::path::Path::new("<project>"), &a.name)
                    .display(),
                scope.documented_by()
            )
        )
    );
    println!();
    match &a.sidecar {
        Some(s) => {
            println!("  from      {}", s.origin);
            println!(
                "  fetched   {}, digest {}, asked for by {}",
                s.installed, s.digest, s.by
            );
            let siblings = library::from_origin(&s.origin, &all);
            if siblings.len() > 1 {
                println!("  also      {}", siblings.join(", "));
            }
        }
        // Absence is printed, not blanked: *nobody recorded where this came
        // from* is the answer, and a blank line would read as *nowhere*.
        None => println!(
            "  {}",
            paint(
                DIM,
                "no provenance recorded — nothing says where this came from"
            )
        ),
    }
    if let Some(g) = grants {
        println!();
        println!("  allowed-tools   {g}");
        println!(
            "  {:<16}{}",
            "",
            paint(DIM, "this skill pre-approves those for whoever installs it")
        );
        println!();
        println!("3.8% of 3,171 public agent setups carry a shell-granting skill.");
    }
    println!();
    println!(
        "{}",
        paint(
            DIM,
            "Devplane does not tell you this is safe. It tells you what it will be allowed to do."
        )
    );
    Ok(())
}

/// `devplane library install <artefact> --to <project>...`
///
/// **Preflight runs first and names every refusal before the first byte is
/// written.** An untrusted target means nothing is written anywhere — never one
/// failure after three successes.
pub async fn cmd_install(name: String, to: Vec<String>, force: bool, json: bool) -> Result<()> {
    let all = library::list()?;
    let a = find(&all, &name)?;
    let ps = projects().await?;

    let wanted: Vec<&Project> = if to.is_empty() {
        ps.iter().collect()
    } else {
        to.iter()
            .filter_map(|t| ps.iter().find(|p| &p.name == t || p.root.ends_with(t)))
            .collect()
    };
    if wanted.is_empty() {
        anyhow::bail!("no registered project matches {}", to.join(", "));
    }

    let targets: Vec<(String, PathBuf, bool)> = wanted
        .iter()
        .map(|p| (p.name.clone(), p.root.clone(), p.trusted))
        .collect();
    let checked = library::preflight_all(a, &targets);

    // A collision is the one refusal `--force` answers. Nothing overrides an
    // untrusted target: the flag exists for a copy somebody edited, not for a
    // repository nobody has looked at.
    let blocked: Vec<_> = checked
        .iter()
        .filter(|(_, _, r)| match r {
            Some(Refusal::Collision) => !force,
            Some(_) => true,
            None => false,
        })
        .collect();

    // The **project root**, not the destination inside it. `preflight_all`
    // returns where the artefact *would go*, and the untrusted refusal prints a
    // command for the person to run — which named `…/.claude/skills` until
    // somebody ran it, telling them to trust a directory that is not a project
    // and would not have worked.
    let root_of = |name: &str| {
        wanted
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.root.clone())
            .unwrap_or_default()
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "artefact": a.name,
                "blocked": blocked.iter().map(|(n, _, r)| serde_json::json!({
                    "project": n, "refusal": r
                })).collect::<Vec<_>>(),
                "would_proceed": checked.len() - blocked.len(),
                "of": checked.len(),
            }))?
        );
    }

    if !blocked.is_empty() {
        for (n, _, r) in &blocked {
            let r = r.expect("blocked rows carry a refusal");
            if !json {
                println!("  {:<10}{:<11}{}", r.label(), n, r.says(&root_of(n)));
            }
        }
        if !json {
            println!();
            println!(
                "Nothing was written. {} of {} targets would proceed.",
                checked.len() - blocked.len(),
                checked.len()
            );
        }
        return Ok(());
    }

    let by = std::env::var("USER").unwrap_or_else(|_| "unknown".into());
    for (n, _, _) in &checked {
        let p = wanted
            .iter()
            .find(|p| &p.name == n)
            .expect("checked targets come from wanted");
        let replaced = library::install_one(a, &p.root, &by)?;
        // **Each overwrite individually.** Five quiet successes and one lost
        // file is how people learn to stop reading output.
        match replaced {
            Some(d) if !json => println!(
                "  {:<10}{:<11}replaced a copy with digest {d}",
                paint(BOLD, "replaced"),
                n
            ),
            None if !json => println!("  {:<10}{}", paint(DIM, "installed"), n),
            _ => {}
        }
    }
    Ok(())
}

/// `devplane library sync <artefact> --from library|project --to <project>`
///
/// Prints what will change and writes nothing without `--apply`. **Not a
/// daemon**: no task, no timer, no watcher.
pub async fn cmd_sync(
    name: String,
    from: String,
    to: String,
    apply: bool,
    json: bool,
) -> Result<()> {
    let all = library::list()?;
    let a = find(&all, &name)?;
    let ps = projects().await?;
    let p = ps
        .iter()
        .find(|p| p.name == to || p.root.ends_with(&to))
        .with_context(|| format!("no registered project named `{to}`"))?;

    let cov = library::coverage(a, &[(p.name.clone(), p.root.clone())]);
    let c = cov.first().context("coverage returned nothing")?;

    // **A two-sided divergence is refused, and a direction is not chosen for
    // you.** Saying which side to take is the one thing this command must not
    // do on somebody's behalf.
    if c.drift == Drift::BothMoved {
        anyhow::bail!(
            "both the library and {}'s copy have moved since it was installed.\n\
             Nobody here will pick: look at both, decide, then run sync with the \
             direction you chose.",
            p.name
        );
    }

    let direction = match from.as_str() {
        "library" => "library → project",
        "project" => "project → library",
        other => anyhow::bail!("--from must be `library` or `project`, not `{other}`"),
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "artefact": a.name, "project": p.name,
                "direction": direction, "drift": c.drift, "applied": apply,
            }))?
        );
    } else {
        println!(
            "  {:<15}{}   {}",
            if apply { "replacing" } else { "would replace" },
            c.path.display(),
            paint(DIM, direction)
        );
    }

    if !apply {
        if !json {
            println!("  {}", paint(DIM, "run with --apply"));
        }
        return Ok(());
    }

    let by = std::env::var("USER").unwrap_or_else(|_| "unknown".into());
    match from.as_str() {
        "library" => {
            library::install_one(a, &p.root, &by)?;
        }
        "project" => {
            // The project's copy becomes the library's. The same copy function,
            // pointed the other way, so neither direction can rewrite bytes the
            // other preserves.
            let source = Artefact {
                name: a.name.clone(),
                kind: a.kind,
                digest: library::digest_at(&c.path)?,
                path: c.path.clone(),
                sidecar: a.sidecar.clone(),
            };
            let lib_root = library::root()?;
            let dest_parent = match a.kind {
                Kind::Skill => lib_root.join("skills"),
                Kind::Prompt => lib_root.join("prompts"),
            };
            std::fs::create_dir_all(&dest_parent)?;
            library::install_into(&source, &a.path)?;
        }
        _ => unreachable!("validated above"),
    }
    Ok(())
}

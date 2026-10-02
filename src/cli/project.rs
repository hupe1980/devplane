//! `devplane project trust`: what a repository's agent configuration does,
//! and a person deciding to let agents start there.

use crate::client;
use crate::render;
use crate::render::{BOLD, DIM, paint};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// `devplane project trust` — show what a repository's agent configuration does, then
/// let a person decide.
///
/// Reports and refuses nothing, prints nothing extra for a repository with
/// none of it, and `--dry-run` trusts nothing.
pub async fn cmd_trust(path: PathBuf, yes: bool, dry_run: bool, json: bool) -> Result<()> {
    let root = path.canonicalize().context("that path does not exist")?;
    let setup = crate::setup::scan(&root);

    if json {
        let findings: Vec<serde_json::Value> = setup
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "kind": f.kind.as_str(), "source": f.source,
                    "subject": f.subject, "detail": f.detail
                })
            })
            .collect();
        if dry_run {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "path": root.display().to_string(),
                    "trusted": false,
                    "findings": findings,
                    // Zero findings does not mean the repository ships nothing.
                    "skills": setup.skills,
                    "unreadable": setup.unreadable,
                }))?
            );
            return Ok(());
        }
        let v = trust_call(&root).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": root.display().to_string(),
                "trusted": v.get("error").is_none(),
                "error": v.get("error"),
                "findings": findings,
                "skills": setup.skills,
                "unreadable": setup.unreadable,
            }))?
        );
        return Ok(());
    }

    print_setup(&root, &setup);

    if dry_run {
        println!("  {}", paint(DIM, "nothing was trusted (--dry-run)"));
        return Ok(());
    }
    // Nothing to show means nothing to confirm.
    if !setup.is_empty() && !crate::cli::confirm("Trust this repository?", yes)? {
        println!("  {}", paint(DIM, "not trusted"));
        return Ok(());
    }
    let v = trust_call(&root).await?;
    match v.get("error").and_then(|e| e.as_str()) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!(
            "{} {}\n  {}",
            paint(render::GREEN, "trusted"),
            root.display(),
            paint(DIM, "agents may now be started here")
        ),
    }
    Ok(())
}

async fn trust_call(root: &Path) -> Result<serde_json::Value> {
    // Through the host where one runs, so its world hears it at once;
    // otherwise straight into the store, which a host reads when it starts.
    // Trusting is a person's record, and needs nothing running.
    if let Ok(c) = client::Client::connect_running().await {
        return c
            .post_json(
                "/api/projects/trust",
                &serde_json::json!({ "path": root.to_string_lossy() }),
            )
            .await;
    }
    let root = root
        .canonicalize()
        .with_context(|| format!("{} does not exist", root.display()))?;
    let store = crate::store::Store::open(&crate::config::db_path()?)
        .await
        .context("opening the store")?;
    let mut project = crate::core::Project::from_root(root);
    project.trusted = true;
    store.save_project(&project).await.with_context(|| {
        format!(
            "could not record that {} is trusted, so it is not",
            project.root.display()
        )
    })?;
    Ok(serde_json::json!({ "trusted": true, "project": project }))
}

/// What starting an agent here will load, grouped the way a person reads it.
fn print_setup(root: &Path, setup: &crate::setup::Setup) {
    if setup.is_empty() {
        // Findings are what is worth flagging; skills are what is there.
        let quiet = match setup.skills {
            0 => "declares no hooks, MCP servers or skills".to_string(),
            1 => "declares 1 skill, and no hooks, MCP servers or pre-approved tools".to_string(),
            n => format!("declares {n} skills, and no hooks, MCP servers or pre-approved tools"),
        };
        println!("  {}", paint(DIM, &format!("{} {quiet}", root.display())));
        return;
    }
    println!(
        "  {}",
        paint(DIM, "starting an agent here loads this repository's own:")
    );
    println!();
    for f in &setup.findings {
        println!(
            "  {:<8}{}",
            paint(BOLD, f.kind.as_str()),
            crate::core::text::clip(&f.subject, 66)
        );
        for line in crate::core::text::wrap(&f.detail, 66) {
            println!("  {:<8}{}", "", paint(DIM, &line));
        }
    }
    for u in &setup.unreadable {
        println!(
            "  {:<8}{}",
            paint(BOLD, "unread"),
            format_args!("{u} exists and could not be parsed")
        );
        println!(
            "  {:<8}{}",
            "",
            paint(DIM, "the agent will read it and this could not")
        );
    }
    println!();
}

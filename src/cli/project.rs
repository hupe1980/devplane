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
        let findings: Vec<_> = setup
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
        // A refusal is the command's error, as without `--json`.
        trust_call(&root).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": root.display().to_string(),
                "trusted": true,
                "error": null,
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
    trust_call(&root).await?;
    println!(
        "{} {}\n  {}",
        paint(render::GREEN, "trusted"),
        root.display(),
        paint(DIM, "agents may now be started here")
    );
    Ok(())
}

/// What `/api/projects/reload` answers; only its refusal is read.
#[derive(serde::Deserialize)]
struct Reloaded {
    #[serde(default)]
    error: Option<String>,
}

async fn trust_call(root: &Path) -> Result<()> {
    // A person's decision: refused from inside an agent's session, which
    // would otherwise trust the repository it is about to run in. Checked by
    // the session's environment, which narrows the hole and does not close
    // it: an agent that unsets the variable, or writes the file itself, is
    // not stopped by it.
    if let Some(var) = crate::hook::agent_session() {
        anyhow::bail!(
            "refused: `{var}` is set, so this is running inside an agent's session, and an agent \
             may not trust a repository for itself. Trust it from a terminal of your own. This \
             check narrows the hole and does not close it: an agent that unsets the variable is \
             not stopped by it."
        );
    }
    // Straight into the person's file, host or no host: the host's own route
    // wants the window's proof, which no terminal has. A running host is then
    // told to re-read, so its world hears the trust at once.
    let root = root
        .canonicalize()
        .with_context(|| format!("{} does not exist", root.display()))?;
    let home = crate::config::home()?;
    crate::trust::add(&home, [root.clone()]).with_context(|| {
        format!(
            "could not record that {} is trusted, so it is not",
            root.display()
        )
    })?;
    // The project row and the decision are history: the store keeps them,
    // and the trust above stands without them.
    let store = crate::store::Store::open(&crate::config::db_path()?)
        .await
        .context("opening the store")?;
    store
        .save_project(&crate::core::Project::from_root(root.clone()))
        .await
        .context("registering the project")?;
    // Trust lets a headless agent run this repository's own hooks and MCP
    // servers, so it is a decision, and the record says who.
    store
        .append_decision(&crate::core::Decision::new(
            crate::core::Authority::Person,
            "project:trust",
            root.display().to_string(),
            "trusted",
        ))
        .await
        .context("recording the decision")?;
    if let Ok(c) = client::Client::connect_running().await {
        let r: Reloaded = c
            .post_json("/api/projects/reload", &serde_json::json!({}))
            .await?;
        if let Some(e) = r.error {
            anyhow::bail!(
                "{} is trusted in {}, and the running host could not re-read it: {e}. \
                 It takes effect when the host restarts.",
                root.display(),
                crate::trust::path(&home).display()
            );
        }
    }
    Ok(())
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

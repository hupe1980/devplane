//! Steering a change while it is in flight: verify, retry, resume, prompt,
//! stop, and deciding about a specification that moved.

use super::{host_and_change, ids, said_error};
use crate::render::{self, DIM, clip, paint};
use anyhow::{Context, Result};

pub(super) async fn cmd_verify(change: &str, base_checks: bool, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    if !json {
        println!("{}", paint(DIM, "running the project's gates…"));
    }
    let v: serde_json::Value = c
        .post_json(
            &format!("/api/changes/{id}/verify"),
            &serde_json::json!({ "base_checks": base_checks }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    if let Some(e) = said_error(&v) {
        anyhow::bail!("{e}");
    }
    let passed = v["passed"].as_bool().unwrap_or(false);
    println!(
        "{} {}",
        match passed {
            true => paint(render::GREEN, "passed"),
            false => paint(render::RED, "failed"),
        },
        v["summary"].as_str().unwrap_or("")
    );
    // Every sentence is the host's; a count is not a verdict, so none is coloured.
    let b = &v["base_checks"];
    if let Some(why) = v["base_checks_refused"].as_str() {
        println!("base checks  {why}");
    }
    for (label, key) in [
        ("base's tests on this change", "on_change"),
        ("new tests on the base", "on_base"),
    ] {
        if let Some(says) = b[key]["says"].as_str() {
            println!("  {label}  {says}");
        }
    }
    if let Some(says) = b["counts_says"].as_str() {
        println!("tests      {says}");
    }
    if !passed {
        let report: Option<crate::core::change::GateReport> =
            serde_json::from_value(v["report"].clone()).ok();
        for cmd in report.iter().flat_map(|r| r.commands.iter()) {
            for f in cmd.failures.iter().take(10) {
                println!("    {f}");
            }
        }
    }
    Ok(())
}

pub(super) async fn cmd_retry(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let v: serde_json::Value = c
        .post_json(&format!("/api/changes/{id}/retry"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match said_error(&v) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!(
            "{} the failures went back to the same session.",
            paint(render::GREEN, "handed back —")
        ),
    }
    Ok(())
}

pub(super) async fn cmd_resume(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let v: serde_json::Value = c
        .post_json(&format!("/api/changes/{id}/resume"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match said_error(&v) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!(
            "{} the agent is back on the same conversation.",
            paint(render::GREEN, "resumed —")
        ),
    }
    Ok(())
}

/// The run a change-or-run argument addresses: a change through its latest run.
async fn run_of(c: &crate::client::Client, needle: &str) -> Result<String> {
    let reader = crate::local::Reader::Host(c.clone());
    match ids::change_or_run(&reader, needle).await? {
        ids::Subject::Run(run) => Ok(run),
        ids::Subject::Change(id) => reader
            .change(&id)
            .await?
            .change
            .runs
            .last()
            .map(|r| r.as_str().to_string())
            .with_context(|| format!("change {id} has no run to address")),
    }
}

pub(super) async fn cmd_prompt(run: &str, text: Vec<String>, json: bool) -> Result<()> {
    let text = text.join(" ");
    if text.trim().is_empty() {
        anyhow::bail!("say what to send: devplane change prompt <change|run> \"use the helper\"");
    }
    let c = crate::client::Client::connect_running().await?;
    let run = run_of(&c, run).await?;
    let v: serde_json::Value = c
        .post_json(
            &format!("/api/runs/{run}/prompt"),
            &serde_json::json!({ "text": text }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match said_error(&v) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!("{}", v["says"].as_str().unwrap_or("sent")),
    }
    Ok(())
}

pub(super) async fn cmd_stop(run: &str, yes: bool, json: bool) -> Result<()> {
    let c = crate::client::Client::connect_running().await?;
    let run = run_of(&c, run).await?;
    // What survives, said before anything is stopped.
    let before: serde_json::Value = c.get(&format!("/api/runs/{run}/stop")).await?;
    if let Some(e) = said_error(&before) {
        anyhow::bail!("{e}");
    }
    match json {
        true => eprintln!("{}", before["says"].as_str().unwrap_or("")),
        false => println!("{}", before["says"].as_str().unwrap_or("")),
    }
    if !crate::cli::confirm("Stop it?", yes)? {
        eprintln!("{}", paint(DIM, "not stopped"));
        return Ok(());
    }
    let v: serde_json::Value = c
        .post_json(&format!("/api/runs/{run}/stop"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match said_error(&v) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!("{}", paint(render::GREEN, "stopped")),
    }
    Ok(())
}

pub(super) async fn cmd_drift(
    change: &str,
    run: &str,
    accept: bool,
    tell: bool,
    json: bool,
) -> Result<()> {
    let verb = match (accept, tell) {
        (true, false) => "accept",
        (false, true) => "tell",
        _ => anyhow::bail!("say which: devplane change drift <id> --run <run> --accept | --tell"),
    };
    let (c, id) = host_and_change(change).await?;
    let run = ids::run(&crate::local::Reader::Host(c.clone()), run).await?;
    let v: serde_json::Value = c
        .post_json(
            &format!("/api/changes/{id}/drift/{verb}"),
            &serde_json::json!({ "run": run }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    if let Some(e) = said_error(&v) {
        anyhow::bail!("{e}");
    }
    match verb {
        "accept" => println!(
            "{} the change now works to the specification run {} saw ({}); recorded as yours.",
            paint(render::GREEN, "accepted —"),
            clip(&run, 12),
            clip(v["fingerprint"].as_str().unwrap_or(""), 12)
        ),
        _ => println!(
            "{} run {} was handed the files that changed; recorded as yours.",
            paint(render::GREEN, "told —"),
            clip(&run, 12)
        ),
    }
    Ok(())
}

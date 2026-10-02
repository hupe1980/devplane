//! Steering a change while it is in flight: verify, retry, resume, prompt,
//! stop, and deciding about a specification that moved.

use super::{Answer, Says, host_and_change, ids};
use crate::render::{self, DIM, clip, paint};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::IgnoredAny;

/// What `verify` came to: the gates' word, and the base checks when asked.
#[derive(Deserialize)]
struct Verified {
    #[serde(default)]
    passed: bool,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    report: Option<crate::core::change::GateReport>,
    #[serde(default)]
    base_checks: Option<crate::core::base_checks::View>,
    #[serde(default)]
    base_checks_refused: Option<String>,
}

/// The specification an accepted drift now works to.
#[derive(Deserialize)]
struct Accepted {
    #[serde(default)]
    fingerprint: Option<String>,
}

pub(super) async fn cmd_verify(change: &str, base_checks: bool, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    if !json {
        println!("{}", paint(DIM, "running the project's gates…"));
    }
    let answer: Answer<Verified> = c
        .post_json(
            &format!("/api/changes/{id}/verify"),
            &serde_json::json!({ "base_checks": base_checks }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    let passed = v.passed;
    println!(
        "{} {}",
        match passed {
            true => paint(render::GREEN, "passed"),
            false => paint(render::RED, "failed"),
        },
        v.summary.as_deref().unwrap_or("")
    );
    // Every sentence is the host's; a count is not a verdict, so none is coloured.
    let b = v.base_checks.as_ref();
    if let Some(why) = &v.base_checks_refused {
        println!("base checks  {why}");
    }
    for (label, says) in [
        (
            "base's tests on this change",
            b.and_then(|b| b.on_change.as_ref()).map(|o| &o.says),
        ),
        (
            "new tests on the base",
            b.and_then(|b| b.on_base.as_ref()).map(|o| &o.says),
        ),
    ] {
        if let Some(says) = says {
            println!("  {label}  {says}");
        }
    }
    if let Some(b) = b {
        println!("tests      {}", b.counts_says);
    }
    if !passed {
        for cmd in v.report.iter().flat_map(|r| r.commands.iter()) {
            for f in cmd.failures.iter().take(10) {
                println!("    {f}");
            }
        }
    }
    Ok(())
}

pub(super) async fn cmd_retry(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let answer: Answer<IgnoredAny> = c
        .post_json(&format!("/api/changes/{id}/retry"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    answer.read()?;
    println!(
        "{} the failures went back to the same session.",
        paint(render::GREEN, "handed back —")
    );
    Ok(())
}

pub(super) async fn cmd_resume(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let answer: Answer<IgnoredAny> = c
        .post_json(&format!("/api/changes/{id}/resume"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    answer.read()?;
    println!(
        "{} the agent is back on the same conversation.",
        paint(render::GREEN, "resumed —")
    );
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
    let answer: Answer<Says> = c
        .post_json(
            &format!("/api/runs/{run}/prompt"),
            &serde_json::json!({ "text": text }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    println!("{}", answer.read()?.says.as_deref().unwrap_or("sent"));
    Ok(())
}

pub(super) async fn cmd_stop(run: &str, yes: bool, json: bool) -> Result<()> {
    let c = crate::client::Client::connect_running().await?;
    let run = run_of(&c, run).await?;
    // What survives, said before anything is stopped.
    let before: Answer<Says> = c.get(&format!("/api/runs/{run}/stop")).await?;
    let says = before.read()?.says.as_deref().unwrap_or("");
    match json {
        true => eprintln!("{says}"),
        false => println!("{says}"),
    }
    if !crate::cli::confirm("Stop it?", yes)? {
        eprintln!("{}", paint(DIM, "not stopped"));
        return Ok(());
    }
    let answer: Answer<IgnoredAny> = c
        .post_json(&format!("/api/runs/{run}/stop"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    answer.read()?;
    println!("{}", paint(render::GREEN, "stopped"));
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
    let answer: Answer<Accepted> = c
        .post_json(
            &format!("/api/changes/{id}/drift/{verb}"),
            &serde_json::json!({ "run": run }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    match verb {
        "accept" => println!(
            "{} the change now works to the specification run {} saw ({}); recorded as yours.",
            paint(render::GREEN, "accepted —"),
            clip(&run, 12),
            clip(v.fingerprint.as_deref().unwrap_or(""), 12)
        ),
        _ => println!(
            "{} run {} was handed the files that changed; recorded as yours.",
            paint(render::GREEN, "told —"),
            clip(&run, 12)
        ),
    }
    Ok(())
}

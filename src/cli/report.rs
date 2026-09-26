//! `devplane report`: a finding one project files about another.
//!
//! The origin is never an argument: the run id comes from `DEVPLANE_RUN` (set
//! on every agent Devplane starts) or `CLAUDE_CODE_SESSION_ID`, and is resolved
//! against the record. A person says `--as-person`. `file`, `ls` and `show`
//! work with nothing running; answering a report needs a host.

use super::ReportCmd;
use crate::client;
use crate::render::{self, BOLD, DIM, clip, paint};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;

/// Where a run id is read from, in order. Set by the process that started
/// the agent, never by the agent's model.
const RUN_FROM: &[&str] = &[crate::driven::RUN_ENV, "CLAUDE_CODE_SESSION_ID"];

pub async fn cmd_report(what: ReportCmd, json: bool) -> Result<()> {
    match what {
        ReportCmd::File {
            to,
            forge,
            keep,
            kind,
            title,
            finding,
            command,
            output_file,
            stack_file,
            commits,
            path,
            as_person,
        } => {
            let run = RUN_FROM
                .iter()
                .find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()));
            let body = json!({
                "kind": kind,
                "title": title,
                "finding": finding,
                "evidence": {
                    "command": command,
                    "output": output_file.as_deref().map(read_bounded).transpose()?,
                    "stack": stack_file.as_deref().map(read_bounded).transpose()?,
                    "commits": commits,
                    "paths": path,
                },
                "to": to,
                "forge": forge,
                "keep": keep,
                "run": run,
                "as_person": as_person,
                "from": std::env::current_dir()?.to_string_lossy(),
            });
            let reader = crate::local::Reader::open().await?;
            let v = reader.file_report(&body).await?;
            if let Some(e) = v.get("error").and_then(Value::as_str) {
                bail!("{e}");
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            println!(
                "{} {} — {}",
                paint(render::GREEN, "filed"),
                paint(BOLD, v["report"]["id"].as_str().unwrap_or("")),
                v["says"].as_str().unwrap_or("")
            );
            Ok(())
        }
        ReportCmd::Ls {
            to_me,
            from_me,
            all,
        } => {
            let reader = crate::local::Reader::open().await?;
            let mut path = String::from("/api/reports?");
            if to_me || from_me {
                let me = here()?;
                let key = if to_me { "to" } else { "from" };
                path.push_str(&format!("{key}={}&", crate::core::text::url_escape(&me)));
            }
            if all {
                path.push_str("all=true");
            }
            let v = reader.get(&path).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            let rows = v.as_array().cloned().unwrap_or_default();
            if rows.is_empty() {
                println!(
                    "{}",
                    match all {
                        true => "No reports have been filed.",
                        false =>
                            "No report is waiting for an answer. `--all` shows the answered ones.",
                    }
                );
            }
            for r in &rows {
                println!(
                    "{}  {:<9} {:<9} {} → {}  {}  {}",
                    paint(BOLD, &clip(r["id"].as_str().unwrap_or(""), 14)),
                    r["state"]["state"].as_str().unwrap_or(""),
                    r["kind"].as_str().unwrap_or(""),
                    r["provenance"]["project_name"].as_str().unwrap_or(""),
                    r["target_says"].as_str().unwrap_or(""),
                    clip(r["title"].as_str().unwrap_or(""), 60),
                    paint(DIM, r["age_says"].as_str().unwrap_or("")),
                );
            }
            if let Some(limits) = reader.limits() {
                println!("{}", paint(DIM, limits));
            }
            Ok(())
        }
        ReportCmd::Show { id } => {
            let reader = crate::local::Reader::open().await?;
            let v = reader.get(&format!("/api/reports/{id}")).await?;
            if let Some(e) = v.get("error").and_then(Value::as_str) {
                bail!("{e}");
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            show(&v);
            Ok(())
        }
        ReportCmd::Start { id, agent } => {
            let c = client::Client::connect_running().await?;
            let v: Value = c
                .post_json(
                    &format!("/api/reports/{id}/start"),
                    &json!({ "agent": agent }),
                )
                .await?;
            answered(&v, json, |v| {
                format!(
                    "started change {} from it; the report is accepted, and it is answered as \
                     fixed when that change is offered or finished",
                    v["change_id"].as_str().unwrap_or("")
                )
            })
        }
        ReportCmd::Reject { id, reason } => resolve(&id, "rejected", Some(reason), json).await,
        ReportCmd::Defer { id, reason } => resolve(&id, "deferred", Some(reason), json).await,
        ReportCmd::Fixed { id, reason } => resolve(&id, "fixed", reason, json).await,
        ReportCmd::Discard { id } => resolve(&id, "discarded", None, json).await,
        ReportCmd::Open { id, yes } => {
            let c = client::Client::connect_running().await?;
            let v: Value = c.get(&format!("/api/reports/{id}")).await?;
            if let Some(e) = v.get("error").and_then(Value::as_str) {
                bail!("{e}");
            }
            let repo = v["target"]["repo"]
                .as_str()
                .context("that report is not a GitHub draft")?;
            // The draft is shown before asking: this writes to a forge under
            // your name. Under `--json` it goes to stderr, beside the question.
            {
                let say = |line: String| match json {
                    true => eprintln!("{line}"),
                    false => println!("{line}"),
                };
                say(format!(
                    "{} {}",
                    paint(BOLD, "Would open an issue on"),
                    paint(BOLD, repo)
                ));
                say(format!("  title  {}", v["title"].as_str().unwrap_or("")));
                for line in v["quoted"].as_str().unwrap_or("").lines() {
                    say(format!("  {line}"));
                }
                say(paint(
                    DIM,
                    "  opened under your GitHub sign-in, in your name, with the body above",
                ));
                if !crate::cli::confirm("Open it?", yes)? {
                    eprintln!("Nothing was sent. The draft is still there.");
                    return Ok(());
                }
            }
            let v: Value = c
                .post_json(&format!("/api/reports/{id}/open"), &json!({}))
                .await?;
            answered(&v, json, |v| {
                format!("opened {}", v["url"].as_str().unwrap_or(""))
            })
        }
    }
}

async fn resolve(id: &str, how: &str, reason: Option<String>, json: bool) -> Result<()> {
    let c = client::Client::connect_running().await?;
    let v: Value = c
        .post_json(
            &format!("/api/reports/{id}/resolve"),
            &json!({ "as": how, "reason": reason }),
        )
        .await?;
    answered(&v, json, |v| {
        format!(
            "{} — {}; the change that filed it is told",
            v["id"].as_str().unwrap_or(""),
            v["state_says"].as_str().unwrap_or("")
        )
    })
}

/// Prints what a route answered, or its refusal.
fn answered(v: &Value, json: bool, says: impl Fn(&Value) -> String) -> Result<()> {
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        bail!("{e}");
    }
    if json {
        println!("{}", serde_json::to_string_pretty(v)?);
    } else {
        println!("{}", says(v));
    }
    Ok(())
}

/// One report, as a person reads it: the facts in our words, then the report
/// in its filer's, quoted.
fn show(v: &Value) {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    println!("{} {}", paint(BOLD, &s("id")), paint(DIM, &s("age_says")));
    println!("  to     {}", s("target_says"));
    println!("  from   {}", s("provenance_says"));
    println!("  state  {}", s("state_says"));
    println!();
    // Quoted: the filer's words are a claim to check, not an instruction.
    for line in s("quoted").lines() {
        println!("  {line}");
    }
    let acts: Vec<&str> = v["actions"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let id = s("id");
    if acts.contains(&"start_from_report") {
        println!();
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "  devplane report start {id} | reject {id} --reason … | defer {id} --reason …"
                )
            )
        );
    }
    if acts.contains(&"open_draft") {
        println!();
        println!(
            "{}",
            paint(DIM, &format!("  devplane report open {id} | discard {id}"))
        );
    }
}

/// The project this directory belongs to, as its id — what `--to-me` and
/// `--from-me` mean.
fn here() -> Result<String> {
    let dir = std::env::current_dir()?.canonicalize()?;
    let root = crate::repo::governing_root(&dir)
        .context("this directory is not in a repository, so it is no project's")?;
    let root = root.canonicalize().unwrap_or(root);
    Ok(crate::core::ProjectId::from_path(&root).to_string())
}

/// A file's contents, read no further than a report could carry. One byte
/// past the bound is enough for the report to refuse the field by name.
fn read_bounded(path: &Path) -> Result<String> {
    let f = std::fs::File::open(path).with_context(|| format!("reading {}", path.display()))?;
    let mut bytes = Vec::new();
    f.take(crate::core::report::TOTAL_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

//! `devplane dispatch --to` and `devplane batch`.
//!
//! One prompt, several repositories, **one reviewable row**. Six runs from one
//! prompt are not six things to review; they are one thing that happened six
//! times, and both surfaces here say so.
//!
//! # What neither surface prints
//!
//! **No aggregate.** No percentage, no `n/m`, no pass rate, no health colour on
//! the batch itself. Partial failure is the normal case — four green, one red,
//! one asking a question is what a fan-out looks like — and a summary over that
//! hides the one row that needs somebody. A test asserts the absence, because
//! the summary is exactly what a later pass helpfully adds.

use crate::client;
use crate::core::batch::{self as core_batch, Position, State};
use crate::render::{BOLD, DIM, paint};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// A project as the daemon reports it.
struct Project {
    name: String,
    root: PathBuf,
    trusted: bool,
}

async fn projects(c: &client::Client) -> Result<Vec<Project>> {
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

/// `devplane dispatch --to a,b,c "prompt"`.
pub async fn cmd_fan_out(
    agent: &str,
    to: Vec<String>,
    prompt: String,
    mode: Option<&str>,
    apply: bool,
    json: bool,
) -> Result<()> {
    if prompt.trim().is_empty() {
        anyhow::bail!("a fan-out needs a prompt");
    }
    let c = client::Client::connect_or_start().await?;
    let all = projects(&c).await?;

    let chosen: Vec<&Project> = to
        .iter()
        .filter_map(|t| all.iter().find(|p| &p.name == t || p.root.ends_with(t)))
        .collect();
    if chosen.is_empty() {
        anyhow::bail!("no registered project matches {}", to.join(", "));
    }
    // **A name that matches nothing stops the whole dispatch.**
    //
    // Found by running it: asking for four projects with one typo proceeded
    // quietly with three. That is precisely the shape this feature exists to
    // refuse — successes with one silent omission — and it is worse here than
    // elsewhere, because the person believes they reached every repository and
    // one of them has heard nothing at all.
    let unknown: Vec<&String> = to
        .iter()
        .filter(|t| {
            !all.iter()
                .any(|p| &&p.name == t || p.root.ends_with(t.as_str()))
        })
        .collect();
    if !unknown.is_empty() {
        let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
        anyhow::bail!(
            "no registered project matches: {}\nNothing was sent. Registered: {}",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            names.join(", ")
        );
    }

    // **The dial, and whether the count chose it.** Above three targets draft
    // is selected regardless of what was asked for, and the person is told that
    // it was selected *for* them rather than discovering it.
    let asked = mode.and_then(Position::parse);
    let chosen_default = core_batch::default_position(chosen.len());
    let position = if chosen_default.forced {
        Position::Draft
    } else {
        asked.unwrap_or(chosen_default.position)
    };
    let overridden = chosen_default.forced && asked.is_some_and(|a| a != Position::Draft);

    let targets: Vec<crate::batch::Target> = chosen
        .iter()
        .map(|p| crate::batch::Target {
            project: crate::core::ProjectId::from_path(&p.root),
            name: p.name.clone(),
            root: p.root.clone(),
            trusted: p.trusted,
        })
        .collect();

    let agents: Vec<String> = {
        let v: serde_json::Value = c.get("/api/agents").await.unwrap_or(serde_json::json!([]));
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };

    let findings = crate::batch::preflight_all(&targets, position, &agents, agent, None, 0).await;
    let accepted = findings.iter().filter(|f| f.refusal.is_none()).count();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "position": position.as_str(),
                "mode_forced": chosen_default.forced,
                "targets": findings,
                "cost_in_runs": accepted,
                "prompt_chars": prompt.chars().count(),
                "prompt_is_long": crate::batch::prompt_is_long(&prompt),
                "applied": apply,
            }))?
        );
    } else {
        // **Every refusal named individually, before anything is live.** Never
        // one failure after three successes: by then two repositories have an
        // agent in them and somebody has to work out which.
        for f in &findings {
            let Some(why) = f.refusal else { continue };
            let t = targets
                .iter()
                .find(|t| t.project == f.project)
                .expect("findings come from targets");
            println!(
                "  {:<10}{}",
                paint(BOLD, "refused"),
                crate::batch::refusal_line(&t.name, why, &t.root)
            );
        }
        println!();
        println!("  {}", paint(BOLD, position.as_str()));
        println!("  {}", paint(DIM, position.says()));
        if chosen_default.forced {
            println!(
                "  {}",
                paint(
                    DIM,
                    &format!(
                        "draft was chosen for you: more than {} targets",
                        core_batch::DRAFT_ABOVE
                    )
                )
            );
        }
        if overridden {
            println!(
                "  {}",
                paint(
                    BOLD,
                    &format!(
                        "`--mode {}` was not honoured, for the reason above",
                        mode.unwrap_or("")
                    )
                )
            );
        }
        // **Cost in runs, never in currency.** A dollar estimate for a model
        // whose price changed last week is asserted rather than measured.
        println!();
        println!(
            "  {} projects × one run",
            paint(BOLD, &accepted.to_string())
        );
        if crate::batch::prompt_is_long(&prompt) {
            println!(
                "  {}",
                paint(
                    DIM,
                    &format!(
                        "{} characters — past {}, each window's warning includes the count and \
                         asks you to scroll before sending",
                        prompt.chars().count(),
                        crate::batch::LONG_PROMPT
                    )
                )
            );
        }
    }

    if !apply {
        if !json {
            println!();
            println!("  {}", paint(DIM, "run with --apply"));
        }
        return Ok(());
    }
    if accepted == 0 {
        if !json {
            println!();
            println!("  {}", paint(BOLD, "every target was refused; nothing ran"));
        }
        return Ok(());
    }

    let by = std::env::var("USER").unwrap_or_else(|_| "unknown".into());
    match position {
        Position::Draft => {
            let b = crate::batch::drafted(&prompt, &by, None, findings.clone());
            let links = crate::batch::draft_links(&b, &targets);
            if !json {
                println!();
                for (name, link) in &links {
                    match link {
                        Some(l) => println!(
                            "  {}{}",
                            crate::render::pad(&paint(BOLD, name), 14),
                            paint(DIM, l)
                        ),
                        // A link the vendor's handler would refuse is reported
                        // rather than printed dead.
                        None => println!(
                            "  {:<14}{}",
                            paint(BOLD, name),
                            paint(DIM, "this path cannot be opened by a deep link")
                        ),
                    }
                }
                println!();
                println!(
                    "  {}",
                    paint(DIM, "each opens with the prompt typed. You send it.")
                );
            }
            post_batch(&c, &b).await?;
        }
        Position::ToGate | Position::ToPullRequest => {
            let b = crate::batch::dispatched(&prompt, &by, None, position, findings.clone());
            // Recorded **before** anything starts, so a daemon that dies
            // between the first member and the last leaves a batch that reads
            // as interrupted rather than leaving orphaned work with nothing
            // saying what it belonged to.
            post_batch(&c, &b).await?;

            // **One call to the existing work path per accepted target, and
            // nothing else.** Not a batch-aware dispatch: the same route, the
            // same trust check, the same worktree, the same gates and the same
            // permission machinery a single dispatch meets. The only thing this
            // adds is a `batch_id`.
            //
            // A member that will not start is reported and the rest continue —
            // the preflight has already refused everything it could see, so a
            // failure here is something that changed underneath it, and
            // abandoning five working targets for it would be worse.
            for f in b.accepted() {
                let Some(t) = targets.iter().find(|t| t.project == f.project) else {
                    continue;
                };
                let body = serde_json::json!({
                    "cwd": t.root.to_string_lossy(),
                    "title": prompt.clone(),
                    "prompt": prompt.clone(),
                    "agent": agent,
                    "batch_id": b.id.as_str(),
                });
                match c.post_json::<serde_json::Value>("/api/work", &body).await {
                    Ok(v) if v.get("error").is_none() => {
                        if !json {
                            println!(
                                "  {:<10}{:<14}{}",
                                paint(DIM, "started"),
                                t.name,
                                paint(DIM, v["work_id"].as_str().unwrap_or(""))
                            );
                        }
                    }
                    Ok(v) => {
                        if !json {
                            println!(
                                "  {:<10}{:<14}{}",
                                paint(BOLD, "failed"),
                                t.name,
                                v["error"].as_str().unwrap_or("could not start")
                            );
                        }
                    }
                    Err(e) => {
                        if !json {
                            println!(
                                "  {}{}{e}",
                                crate::render::pad(&paint(BOLD, "failed"), 10),
                                crate::render::pad(&t.name, 14)
                            );
                        }
                    }
                }
            }

            if !json {
                println!();
                println!("  {}  {}", paint(BOLD, "batch"), b.id.as_str());
                println!(
                    "  {}",
                    paint(DIM, "devplane batch — one row, one outcome per target")
                );
            }
        }
    }
    Ok(())
}

async fn post_batch(c: &client::Client, b: &core_batch::Batch) -> Result<()> {
    let _: serde_json::Value = c
        .post_json("/api/batch", &serde_json::to_value(b)?)
        .await
        .context("recording the batch")?;
    Ok(())
}

/// `devplane batch [id]` — one row, one outcome per target.
pub async fn cmd_batch(id: Option<String>, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let path = match &id {
        Some(i) => format!("/api/batch/{i}"),
        None => "/api/batch".to_string(),
    };
    let v: serde_json::Value = c.get(&path).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    let empty = vec![];
    let list = if v.is_array() {
        v.as_array().unwrap_or(&empty).clone()
    } else {
        vec![v.clone()]
    };
    if list.is_empty() {
        println!("{}", paint(DIM, "No fan-out has been sent yet."));
        return Ok(());
    }

    for b in &list {
        let state = b["state"].as_str().unwrap_or("");
        // **The batch itself is uncoloured.** Colouring it would be inventing
        // the verdict this refuses to compute.
        println!(
            "{}  {}",
            paint(BOLD, b["id"].as_str().unwrap_or("")),
            b["prompt"].as_str().unwrap_or("")
        );
        let says = State::parse(state).map(State::says).unwrap_or("");
        println!(
            "  {:<14}{}",
            b["position"].as_str().unwrap_or(""),
            paint(DIM, says)
        );

        let members = b["members"].as_array().unwrap_or(&empty);
        for m in members {
            let needs = m["needs_you"].as_bool().unwrap_or(false);
            let failed = m["failed"].as_bool().unwrap_or(false);
            let mark = if needs {
                paint(crate::render::YELLOW, "needs you")
            } else if failed {
                paint(crate::render::RED, "failed")
            } else {
                paint(DIM, m["phase"].as_str().unwrap_or(""))
            };
            println!(
                "  {:<14}{:<16}{}",
                mark,
                m["project"].as_str().unwrap_or(""),
                m["title"].as_str().unwrap_or("")
            );
        }
        // Targets the preflight refused stay on the record: a list that dropped
        // them would answer "what did I send this to" with the subset that
        // happened to work.
        for t in b["targets"].as_array().unwrap_or(&empty) {
            if let Some(r) = t["refusal"].as_str() {
                println!(
                    "  {:<14}{:<16}{}",
                    paint(DIM, "refused"),
                    t["project"].as_str().unwrap_or(""),
                    paint(DIM, r)
                );
            }
        }
        println!();
    }
    Ok(())
}

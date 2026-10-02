//! `devplane change review`: a change in review order, and marking a
//! weakened check read.

use crate::render::{BOLD, DIM, YELLOW, clip, paint};
use anyhow::{Context, Result};

/// The review, worded exactly as the surface words it: every sentence is the
/// host's, and this only lays them out.
fn print_review(r: &crate::view::ReviewView, by_intent: bool) {
    println!(
        "{} {}",
        paint(BOLD, &r.title),
        paint(DIM, &format!("against {}", r.base))
    );
    println!("  {}", r.shape_says);
    println!("  {}", r.standing_says);
    for s in [
        &r.unordered,
        &r.coverage_absent,
        &r.truncated_says,
        &r.empty_says,
    ]
    .into_iter()
    .flatten()
    {
        println!("  {}", paint(YELLOW, s));
    }
    let row = |f: &crate::view::ReviewFile| {
        let mut cells = vec![f.path.clone(), f.role_says.clone()];
        if let Some(c) = &f.coverage_says {
            cells.push(c.clone());
        }
        cells.push(match f.decisions.len() {
            0 => "no decision recorded".to_string(),
            1 => "1 decision".to_string(),
            n => format!("{n} decisions"),
        });
        if !f.task_says.is_empty() {
            cells.push(f.task_says.clone());
        }
        println!("    {}", cells.join(" · "));
    };
    if by_intent {
        println!();
        println!("{}", paint(BOLD, &r.intent.heading));
        if let Some(why) = &r.intent.unavailable {
            println!("  {}", paint(YELLOW, why));
        }
        for g in &r.intent.groups {
            println!("  {}", g.title);
            println!("    {}", paint(DIM, &g.says));
            for f in &g.files {
                println!("    {f}");
            }
        }
        if !r.intent.not_asked_for.is_empty() {
            println!("  {}", paint(YELLOW, &r.intent.not_asked_for_heading));
            for n in &r.intent.not_asked_for {
                println!("    {} — {}", n.path, n.says);
            }
        }
    } else {
        for g in &r.groups {
            println!();
            println!("  {}", paint(BOLD, &g.says));
            // The weakened rows, each with what it matched and whether a
            // person marked it seen; then the files.
            for w in &g.weakened {
                println!(
                    "    {} — {} {}",
                    w.row.path,
                    paint(YELLOW, &w.row.why),
                    paint(
                        DIM,
                        match w.seen {
                            true => "(seen)",
                            false => "(unseen)",
                        }
                    )
                );
                println!("      {}", paint(DIM, &clip(&w.row.matched, 120)));
            }
            for f in &g.files {
                row(f);
            }
        }
    }
    if let Some(s) = &r.formatter_only_says {
        println!();
        println!("  {}", paint(DIM, s));
    }
}

pub(super) async fn cmd_review(change: &str, by: &str, json: bool) -> Result<()> {
    let reader = crate::local::Reader::open().await?;
    let id = super::ids::change(&reader, change).await?;
    let v = reader.get(&format!("/api/changes/{id}/review")).await?;
    if json {
        return crate::cli::print_json(&v);
    }
    if let Some(e) = super::said_error(&v) {
        anyhow::bail!("{e}");
    }
    let r: crate::view::ReviewView = serde_json::from_value(v).context("reading the review")?;
    print_review(&r, by == "intent");
    Ok(())
}

/// Marks every current weakened row at each path seen, as the person running
/// this.
pub(super) async fn cmd_mark_seen(change: &str, seen: &[String], json: bool) -> Result<()> {
    let reader = crate::local::Reader::open().await?;
    let id = super::ids::change(&reader, change).await?;
    let mut all = Vec::new();
    for path in seen {
        let v = reader.mark_seen(&id, path).await?;
        if let Some(e) = super::said_error(&v) {
            let rows: Vec<String> = v["rows"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|r| r["path"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            anyhow::bail!(
                "{e}{}",
                match rows.is_empty() {
                    true => String::new(),
                    false => format!("; the weakened rows are at: {}", rows.join(", ")),
                }
            );
        }
        for r in v["seen"].as_array().cloned().unwrap_or_default() {
            if !json {
                println!(
                    "seen: {} — {}",
                    r["path"].as_str().unwrap_or(""),
                    r["why"].as_str().unwrap_or("")
                );
            }
            all.push(r);
        }
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "seen": all }))?
        );
    }
    Ok(())
}

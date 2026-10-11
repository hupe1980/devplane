//! `devplane change review`: a change in review order, and marking a path's
//! hunks seen.

use super::Answer;
use crate::render::{BOLD, DIM, YELLOW, clip, paint};
use anyhow::Result;

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
    println!("  {}", unread_says(r));
    if let Some(s) = &r.not_opened_says {
        println!("  {}", paint(YELLOW, s));
    }
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
    let answer: Answer<crate::view::ReviewView> =
        crate::cli::raw_as(&reader, &format!("/api/changes/{id}/review")).await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    print_review(answer.read()?, by == "intent");
    Ok(())
}

/// *3 hunks unread*, from the host's marks: a count, never a share.
pub(crate) fn unread_says(r: &crate::view::ReviewView) -> String {
    let files = r.groups.iter().flat_map(|g| g.files.iter());
    let n: usize = files
        .map(|f| {
            f.hunks.iter().filter(|h| !h.seen).count() + usize::from(f.unit_seen == Some(false))
        })
        .sum();
    match n {
        1 => "1 hunk unread".to_string(),
        n => format!("{n} hunks unread"),
    }
}

/// Marks every current hunk at each path seen, as the person running this.
pub(super) async fn cmd_mark_seen(change: &str, seen: &[String], json: bool) -> Result<()> {
    let reader = crate::local::Reader::open().await?;
    let id = super::ids::change(&reader, change).await?;
    let mut all = Vec::new();
    for path in seen {
        let answer: Answer<crate::view::Marked> = reader.mark_seen(&id, path).await?;
        let v = answer.read()?;
        let n = v.seen.len();
        if !json {
            println!(
                "seen: {path} — {}",
                match n {
                    1 => "1 hunk".to_string(),
                    n => format!("{n} hunks"),
                }
            );
            for r in &v.weakened_seen {
                println!("  read: {} — {}", r.path, r.why);
            }
        }
        all.push(answer);
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "seen": all }))?
        );
    }
    Ok(())
}

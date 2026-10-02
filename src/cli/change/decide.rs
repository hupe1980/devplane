//! A person's decisions about a change: offer it, finish it, archive it.

use super::{REFUSED, host_and_change, said_error};
use crate::render::{self, BOLD, DIM, paint};
use anyhow::Result;

pub(super) async fn cmd_offer(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let v: serde_json::Value = c
        .post_json(&format!("/api/changes/{id}/offer"), &serde_json::json!({}))
        .await?;
    // A weakened check nobody marked seen: every row, and how to mark one.
    if v["refused"] == "weakened_unseen" {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "refused": v["refused"],
                    "rows": v["rows"],
                    "seen_with": v["seen_with"],
                }))?
            );
            return Err(crate::cli::Exit(REFUSED).into());
        }
        eprintln!(
            "{} {}\n",
            paint(BOLD, "refused:"),
            v["says"].as_str().unwrap_or("")
        );
        for r in v["rows"].as_array().cloned().unwrap_or_default() {
            eprintln!(
                "  {}   {}",
                r["path"].as_str().unwrap_or(""),
                r["why"].as_str().unwrap_or("")
            );
        }
        eprintln!(
            "\nread it in `devplane change review {id}`, then mark it seen:\n  {}",
            v["seen_with"].as_str().unwrap_or("")
        );
        return Err(crate::cli::Exit(REFUSED).into());
    }
    if json {
        return crate::cli::print_json(&v);
    }
    if let Some(e) = said_error(&v) {
        anyhow::bail!("{e}");
    }
    // The commit the offer wrote, when it wrote one: its tree is the checked one.
    if let Some(sha) = v["committed"].as_str() {
        println!(
            "{} {} {}",
            paint(render::GREEN, "committed"),
            crate::core::text::clip(sha, 12),
            paint(
                DIM,
                "— its tree is the one the last passing check ran against"
            )
        );
    }
    match v["offer"].as_str() {
        Some("opened") => println!(
            "{} #{} {}",
            paint(render::GREEN, "offered —"),
            v["pull_request"]["number"].as_u64().unwrap_or(0),
            paint(DIM, v["pull_request"]["url"].as_str().unwrap_or(""))
        ),
        _ => {
            // `[github] pull_request` is off: print the command and the
            // address, run nothing.
            println!(
                "{}",
                paint(
                    DIM,
                    "nothing was pushed — [github] pull_request is not set for this project: \
                     push the branch, then open the address to create the pull request yourself:"
                )
            );
            println!("  {}", v["push"].as_str().unwrap_or(""));
            println!("  {}", v["create"].as_str().unwrap_or(""));
        }
    }
    Ok(())
}

pub(super) async fn cmd_finish(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let v: serde_json::Value = c
        .post_json(&format!("/api/changes/{id}/finish"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match said_error(&v) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!(
            "{} {}",
            paint(render::GREEN, "finished —"),
            paint(
                DIM,
                &format!(
                    "now {}. `devplane change offer {id}` opens the pull request; \
                     `devplane change archive {id}` removes the worktree.",
                    v["state"].as_str().unwrap_or("recorded")
                )
            )
        ),
    }
    Ok(())
}

pub(super) async fn cmd_archive(
    change: &str,
    delete_branch: bool,
    discard_uncommitted: bool,
    force: bool,
    json: bool,
) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let v: serde_json::Value = c
        .post_json(
            &format!(
                "/api/changes/{id}/archive?delete_branch={delete_branch}\
                 &discard_uncommitted={discard_uncommitted}&force={force}"
            ),
            &serde_json::json!({}),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    match said_error(&v) {
        Some(e) => anyhow::bail!("{e}"),
        None => println!(
            "{} {}",
            paint(render::GREEN, "archived —"),
            paint(
                DIM,
                match (
                    v["worktree_removed"].as_bool().unwrap_or(false),
                    v["branch_deleted"].as_bool().unwrap_or(false),
                ) {
                    (true, true) => "worktree removed, branch deleted, record kept",
                    (true, false) => "worktree removed, branch and record kept",
                    _ => "nothing to remove; the record is kept",
                }
            )
        ),
    }
    Ok(())
}

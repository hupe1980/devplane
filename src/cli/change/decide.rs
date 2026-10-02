//! A person's decisions about a change: offer it, finish it, archive it, mark
//! its agent's claim read.

use super::{Answer, REFUSED, Says, host_and_change};
use crate::core::base_checks::Absent;
use crate::render::{self, BOLD, DIM, paint};
use anyhow::Result;
use serde::Deserialize;

/// What an offer came to, or why it was refused: the fields this reads of
/// [`crate::change::Offer`] and of the weakened-unseen refusal.
#[derive(Deserialize)]
struct Offered {
    #[serde(default)]
    refused: Option<String>,
    #[serde(default)]
    says: Option<String>,
    #[serde(default)]
    rows: Option<Vec<crate::core::review::Weakened>>,
    #[serde(default)]
    seen_with: Option<String>,
    #[serde(default)]
    committed: Option<String>,
    #[serde(default)]
    not_opened_says: Option<String>,
    #[serde(default)]
    offer: Option<String>,
    #[serde(default)]
    pull_request: Option<OfferedPullRequest>,
    #[serde(default)]
    push: Option<String>,
    #[serde(default)]
    create: Option<String>,
}

/// The pull request an offer opened.
#[derive(Deserialize)]
struct OfferedPullRequest {
    #[serde(default)]
    number: Option<u64>,
    #[serde(default)]
    url: Option<String>,
}

/// The state a finished change is now in.
#[derive(Deserialize)]
struct Finished {
    #[serde(default)]
    state: Option<String>,
}

/// What archiving removed.
#[derive(Deserialize)]
struct Archived {
    #[serde(default)]
    worktree_removed: bool,
    #[serde(default)]
    branch_deleted: bool,
}

/// A change's base checks as its view composes them, read now; none when
/// the host cannot say.
async fn base_checks_of(
    c: &crate::client::Client,
    id: &str,
) -> Option<crate::core::base_checks::View> {
    c.get::<crate::view::ChangeView<'static>>(&format!("/api/changes/{id}"))
        .await
        .ok()
        .and_then(|w| w.base_checks)
}

pub(super) async fn cmd_offer(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    // The offer runs base checks first, once per tree, and waits for them.
    let before = base_checks_of(&c, &id).await;
    if !json
        && matches!(
            before.and_then(|b| b.absent).map(|a| a.absent),
            Some(Absent::NotRun | Absent::Stale | Absent::Running)
        )
    {
        eprintln!("{}", paint(DIM, crate::core::base_checks::OFFER_WAITS));
    }
    let answer: Answer<Offered> = c
        .post_json(&format!("/api/changes/{id}/offer"), &serde_json::json!({}))
        .await?;
    // What they came to, or why they did not run: said, never a reason to stop.
    if !json && let Some(b) = base_checks_of(&c, &id).await {
        if let Some(why) = &b.did_not_run {
            eprintln!("{}", paint(render::YELLOW, why));
        } else if let Some(h) = &b.headline {
            eprintln!("{} {}", paint(DIM, "base checks"), h.says);
        }
    }
    // A weakened check nobody marked seen: every row, and how to mark one.
    if let Some(v) = answer
        .typed()
        .filter(|v| v.refused.as_deref() == Some("weakened_unseen"))
    {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "refused": v.refused,
                    "rows": v.rows,
                    "seen_with": v.seen_with,
                }))?
            );
            return Err(crate::cli::Exit(REFUSED).into());
        }
        eprintln!(
            "{} {}\n",
            paint(BOLD, "refused:"),
            v.says.as_deref().unwrap_or("")
        );
        for r in v.rows.iter().flatten() {
            eprintln!("  {}   {}", r.path, r.why);
        }
        eprintln!(
            "\nread it in `devplane change review {id}`, then mark it seen:\n  {}",
            v.seen_with.as_deref().unwrap_or("")
        );
        return Err(crate::cli::Exit(REFUSED).into());
    }
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    // The commit the offer wrote, when it wrote one: its tree is the checked one.
    if let Some(sha) = &v.committed {
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
    // The files nobody opened, by name: said, never a reason to refuse.
    if let Some(says) = &v.not_opened_says {
        println!("{}", paint(render::YELLOW, says));
    }
    match v.offer.as_deref() {
        Some("opened") => println!(
            "{} #{} {}",
            paint(render::GREEN, "offered —"),
            v.pull_request.as_ref().and_then(|p| p.number).unwrap_or(0),
            paint(
                DIM,
                v.pull_request
                    .as_ref()
                    .and_then(|p| p.url.as_deref())
                    .unwrap_or("")
            )
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
            println!("  {}", v.push.as_deref().unwrap_or(""));
            println!("  {}", v.create.as_deref().unwrap_or(""));
        }
    }
    Ok(())
}

pub(super) async fn cmd_finish(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let answer: Answer<Finished> = c
        .post_json(&format!("/api/changes/{id}/finish"), &serde_json::json!({}))
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    println!(
        "{} {}",
        paint(render::GREEN, "finished —"),
        paint(
            DIM,
            &format!(
                "now {}. `devplane change offer {id}` opens the pull request; \
                 `devplane change archive {id}` removes the worktree.",
                v.state.as_deref().unwrap_or("recorded")
            )
        )
    );
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
    let answer: Answer<Archived> = c
        .post_json(
            &format!(
                "/api/changes/{id}/archive?delete_branch={delete_branch}\
                 &discard_uncommitted={discard_uncommitted}&force={force}"
            ),
            &serde_json::json!({}),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    println!(
        "{} {}",
        paint(render::GREEN, "archived —"),
        paint(
            DIM,
            match (v.worktree_removed, v.branch_deleted) {
                (true, true) => "worktree removed, branch deleted, record kept",
                (true, false) => "worktree removed, branch and record kept",
                _ => "nothing to remove; the record is kept",
            }
        )
    );
    Ok(())
}

/// Marks the change's open stop claim read. It stays on the change.
pub(super) async fn cmd_claim_read(change: &str, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let answer: Answer<Says> = c
        .post_json(
            &format!("/api/changes/{id}/claim/read"),
            &serde_json::json!({}),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    println!("{}", v.says.as_deref().unwrap_or("marked read"));
    Ok(())
}

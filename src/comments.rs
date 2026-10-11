//! A person's review comments on a change: kept by the host until sent, and
//! sent together as one prompt — never one message per comment.

use crate::core::comments::{self, ReviewComment};
use crate::core::ids::ChangeId;
use crate::host::Shared;
use anyhow::{Context, Result, bail};

/// What a person writes: the line, and their words. The host fills in what
/// the line and its hunk are now.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NewComment {
    pub path: String,
    pub line: u32,
    #[serde(default = "new_side")]
    pub side: String,
    pub body: String,
}

fn new_side() -> String {
    "new".into()
}

/// Why a comment was not kept.
#[derive(Debug)]
pub enum Refused {
    NoSuchChange,
    /// The diff does not show that line, or the comment has no words.
    NotThere(String),
}

/// Keeps a comment on a line the change's diff shows now.
pub async fn create(
    state: &Shared,
    id: &ChangeId,
    c: NewComment,
) -> Result<ReviewComment, Refused> {
    if c.body.trim().is_empty() {
        return Err(Refused::NotThere("a comment needs words".into()));
    }
    if c.side != "new" && c.side != "old" {
        return Err(Refused::NotThere("`side` is `new` or `old`".into()));
    }
    let snap = state.snapshot().await;
    let change = snap.change(id).ok_or(Refused::NoSuchChange)?;
    let set = match crate::view::read_rows_of(&snap, change).await {
        Some(Ok((_, set))) => set,
        Some(Err(e)) => return Err(Refused::NotThere(format!("{e:#}"))),
        None => {
            return Err(Refused::NotThere(
                "the change has no checkout on disk".into(),
            ));
        }
    };
    let (hunk, line_text) = comments::locate(&set, &c.path, &c.side, c.line).ok_or_else(|| {
        Refused::NotThere(format!(
            "the diff does not show {} line {} of `{}`",
            if c.side == "old" {
                "base"
            } else {
                "the change's"
            },
            c.line,
            c.path
        ))
    })?;
    let tree = match change.worktree.as_deref() {
        Some(dir) => crate::git::commit_stamp(dir).await.and_then(|s| s.tree),
        None => None,
    }
    .unwrap_or_else(|| "unknown".into());
    let row = ReviewComment {
        id: format!("rc-{}", uuid::Uuid::now_v7().simple()),
        change_id: id.to_string(),
        path: c.path,
        line: c.line,
        side: c.side,
        hunk_header: hunk.header.clone(),
        hunk_digest: crate::core::review::hunk_digest(hunk),
        line_text,
        body: c.body,
        tree,
        created_at: format!("{:.9}", jiff::Timestamp::now()),
        sent_at: None,
        run_id: None,
    };
    state
        .store
        .add_comment(&row)
        .await
        .map_err(|e| Refused::NotThere(format!("the comment was not kept: {e:#}")))?;
    state.notify_changed();
    Ok(row)
}

/// What sending came to.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Sent {
    pub delivered: crate::change::Delivered,
    pub run: String,
    pub says: String,
    pub sent: usize,
}

/// Sends every unsent comment on a change as one prompt, in the review's
/// order. Nothing is marked sent unless the prompt went.
pub async fn send(state: &Shared, id: &ChangeId) -> Result<Option<Sent>> {
    let unsent: Vec<ReviewComment> = state
        .store
        .comments(id.as_str())
        .await?
        .into_iter()
        .filter(|c| c.sent_at.is_none())
        .collect();
    if unsent.is_empty() {
        return Ok(None);
    }
    let order = reading_order(state, id).await;
    let mut ordered: Vec<&ReviewComment> = unsent.iter().collect();
    ordered.sort_by_key(|c| {
        (
            order
                .iter()
                .position(|p| *p == c.path)
                .unwrap_or(usize::MAX),
            c.path.clone(),
            c.line,
        )
    });
    let text = comments::compose(&ordered);
    let (delivered, run) = crate::change::deliver_prompt(state, id, text).await?;
    let ids: Vec<String> = unsent.iter().map(|c| c.id.clone()).collect();
    state
        .store
        .mark_comments_sent(
            &ids,
            run.as_str(),
            &format!("{:.9}", jiff::Timestamp::now()),
        )
        .await
        .context("the prompt went, but the comments were not marked sent")?;
    state.notify_changed();
    Ok(Some(Sent {
        says: match ids.len() {
            1 => format!("1 comment {}", delivered.says()),
            n => format!("{n} comments {}", delivered.says()),
        },
        delivered,
        run: run.to_string(),
        sent: ids.len(),
    }))
}

/// Deletes an unsent comment.
pub async fn delete(state: &Shared, id: &ChangeId, comment: &str) -> Result<()> {
    if !state
        .store
        .delete_unsent_comment(id.as_str(), comment)
        .await?
    {
        bail!("no unsent comment `{comment}` on this change — a sent comment stays");
    }
    state.notify_changed();
    Ok(())
}

/// The change's files in the order its review reads them.
async fn reading_order(state: &Shared, id: &ChangeId) -> Vec<String> {
    let snap = state.snapshot().await;
    let Some(change) = snap.change(id) else {
        return Vec::new();
    };
    let roles = snap
        .world
        .project(&change.project_id)
        .and_then(|p| crate::core::ProjectConfig::load(&p.root).ok())
        .and_then(|c| c.review.roles);
    match crate::view::read_rows_of(&snap, change).await {
        Some(Ok((rows, set))) => crate::core::review::reading_order(&set, roles.as_ref(), &rows),
        _ => Vec::new(),
    }
}

//! `devplane change start` and `devplane change adopt`: how a change begins.

use super::Answer;
use crate::render::{self, BOLD, DIM, paint};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What starting would do in each project: the fields this reads.
#[derive(Deserialize)]
struct Preflight {
    #[serde(default)]
    targets: Vec<Target>,
}

/// One project's preflight.
#[derive(Deserialize)]
struct Target {
    #[serde(default)]
    name: Option<String>,
    /// Why it is refused, when it is.
    #[serde(default)]
    says: Option<String>,
    /// What the first run in a fresh tree costs, as sentences.
    #[serde(default)]
    notes: Vec<String>,
}

/// The changes a start or an adopt made.
#[derive(Deserialize)]
struct Begun {
    #[serde(default)]
    changes: Vec<BegunOne>,
    /// The one change, when there is exactly one.
    #[serde(default)]
    change_id: Option<String>,
    #[serde(default)]
    change: Option<crate::core::Change>,
}

/// One project's started change.
#[derive(Deserialize)]
struct BegunOne {
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    change_id: Option<String>,
    #[serde(default)]
    change: Option<crate::core::Change>,
}

/// `change start --json`: the host's answer, the first run's costs, and —
/// for one change — what its run was sent.
#[derive(Serialize)]
struct StartShown<'a> {
    #[serde(flatten)]
    answer: &'a Answer<Begun>,
    notes: Vec<&'a String>,
    #[serde(flatten)]
    sent: Option<SentShown<'a>>,
}

/// What one run was sent, as its detail says it; null when unread.
#[derive(Serialize)]
struct SentShown<'a> {
    sent: Option<&'a Vec<crate::spec::SentTask>>,
    sent_says: Option<&'a str>,
}

/// A started change's lines: where it lives and how to check it.
fn print_begun(w: Option<&crate::core::Change>) {
    let Some(w) = w else { return };
    if let Some(b) = &w.branch {
        println!("  branch    {b}");
    }
    if let Some(d) = &w.worktree {
        println!("  worktree  {}", d.display());
    }
    if let Some(sp) = &w.spec {
        println!("  spec      {sp}");
    }
}

/// What `change start` was asked for.
pub(super) struct Start {
    pub title: Vec<String>,
    pub agent: Option<String>,
    pub project: Vec<String>,
    pub no_worktree: bool,
    pub issue: Option<u64>,
    pub spec: Option<String>,
    pub task: Vec<String>,
}

pub(super) async fn cmd_start(s: Start, json: bool) -> Result<()> {
    let c = crate::client::Client::connect_running().await?;
    // Also refused by the parser; repeated for hand-built command lines.
    if !s.task.is_empty() && s.spec.is_none() {
        anyhow::bail!("--task needs --spec: a task is a line in the folder --spec names");
    }
    let projects = match s.project.is_empty() {
        true => vec![std::env::current_dir()?.to_string_lossy().to_string()],
        false => s.project,
    };
    let title = s.title.join(" ");
    if title.is_empty() && s.issue.is_none() {
        anyhow::bail!(
            "say what the change is: devplane change start \"fix the flaky test\"\n\
             or point at an issue:  devplane change start --issue 7"
        );
    }
    let body = serde_json::json!({
        "projects": projects,
        "title": title,
        "agent": s.agent,
        "worktree": !s.no_worktree,
        "issue": s.issue,
        "spec": s.spec,
        "tasks": s.task,
    });
    // Every refusal, and what the first run will cost, before anything is
    // written.
    let pre: Answer<Preflight> = c.post_json("/api/changes/preflight", &body).await?;
    let targets = &pre.read()?.targets;
    let refused: Vec<&str> = targets.iter().filter_map(|t| t.says.as_deref()).collect();
    if !refused.is_empty() {
        if json {
            println!("{}", serde_json::to_string_pretty(&pre)?);
            return Err(crate::cli::Exit(1).into());
        }
        for line in &refused {
            println!("  {:<10}{line}", paint(BOLD, "refused"));
        }
        anyhow::bail!(match projects.len() {
            1 => "nothing was started",
            _ => "nothing was started: every project is checked before any change is made",
        });
    }
    let notes: Vec<(String, String)> = targets
        .iter()
        .flat_map(|t| {
            let name = t.name.clone().unwrap_or_default();
            t.notes.iter().map(move |n| (name.clone(), n.clone()))
        })
        .collect();
    if !json {
        for (name, note) in &notes {
            match projects.len() {
                1 => println!("  {}", paint(DIM, note)),
                _ => println!("  {:<14}{}", name, paint(DIM, note)),
            }
        }
    }

    let answer: Answer<Begun> = c.post_json("/api/changes", &body).await?;
    let started: &[BegunOne] = answer.typed().map_or(&[], |v| &v.changes);
    // What each run was sent lives on the run: read it back.
    let mut sent_of = Vec::new();
    for one in started {
        let run_id = one
            .change
            .as_ref()
            .and_then(|w| w.runs.last())
            .map(|r| r.as_str().to_string());
        let sent: Option<crate::view::RunRowView> = match &run_id {
            Some(r) => c.get(&format!("/api/runs/{r}")).await.ok(),
            None => None,
        };
        sent_of.push(sent);
    }
    if json {
        let shown = StartShown {
            answer: &answer,
            notes: notes.iter().map(|(_, n)| n).collect(),
            sent: match sent_of.as_slice() {
                [one] => Some(SentShown {
                    sent: one.as_ref().and_then(|r| r.sent.as_ref()),
                    sent_says: one.as_ref().map(|r| r.sent_says.as_str()),
                }),
                _ => None,
            },
        };
        // A project that changed between preflight and start fails the
        // command here too.
        return crate::cli::print_json(&shown);
    }
    for (one, sent) in started.iter().zip(&sent_of) {
        let w = one.change.as_ref();
        println!(
            "{} {}",
            paint(render::GREEN, "started"),
            paint(BOLD, w.map_or(title.as_str(), |w| w.title.as_str()))
        );
        if projects.len() > 1 {
            println!("  project   {}", one.project.as_deref().unwrap_or(""));
        }
        print_begun(w);
        if let Some(sent) = sent {
            println!("  tasks     {}", sent.sent_says);
            for t in sent.sent.iter().flatten() {
                println!("            {}", paint(DIM, &t.text));
            }
        }
        println!(
            "  {}",
            paint(
                DIM,
                &format!(
                    "devplane change verify {}",
                    one.change_id.as_deref().unwrap_or("")
                )
            )
        );
    }
    // A project that changed between preflight and start is named last.
    if let Some(e) = answer.error() {
        anyhow::bail!("{e}");
    }
    Ok(())
}

pub(super) async fn cmd_adopt(
    branch: String,
    project: Option<PathBuf>,
    spec: Option<String>,
    title: Option<String>,
    json: bool,
) -> Result<()> {
    let c = crate::client::Client::connect_running().await?;
    let project = match project {
        Some(p) => p,
        None => {
            let here = std::env::current_dir()?;
            crate::repo::find_repo_root(&here).unwrap_or(here)
        }
    };
    let answer: Answer<Begun> = c
        .post_json(
            "/api/changes/adopt",
            &serde_json::json!({
                "branch": branch,
                "project": project.to_string_lossy(),
                "spec": spec,
                "title": title,
            }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    let w = v.change.as_ref();
    println!(
        "{} {}",
        paint(render::GREEN, "adopted"),
        paint(BOLD, w.map_or(branch.as_str(), |w| w.title.as_str()))
    );
    print_begun(w);
    println!(
        "  {}",
        paint(
            DIM,
            &format!(
                "devplane change verify {}",
                v.change_id.as_deref().unwrap_or("")
            )
        )
    );
    Ok(())
}

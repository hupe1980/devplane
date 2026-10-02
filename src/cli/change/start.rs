//! `devplane change start` and `devplane change adopt`: how a change begins.

use super::said_error;
use crate::render::{self, BOLD, DIM, paint};
use anyhow::Result;
use std::path::PathBuf;

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
    let pre: serde_json::Value = c.post_json("/api/changes/preflight", &body).await?;
    if let Some(e) = said_error(&pre) {
        anyhow::bail!("{e}");
    }
    let empty = vec![];
    let targets = pre["targets"].as_array().unwrap_or(&empty);
    let refused: Vec<&str> = targets.iter().filter_map(|t| t["says"].as_str()).collect();
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
            let name = t["name"].as_str().unwrap_or("").to_string();
            t["notes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |n| n.as_str().map(|n| (name.clone(), n.to_string())))
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

    let mut v: serde_json::Value = c.post_json("/api/changes", &body).await?;
    // What each run was sent lives on the run: read it back.
    let mut sent_of = Vec::new();
    for started in v["changes"].as_array().cloned().unwrap_or_default() {
        let run_id = started["change"]["runs"]
            .as_array()
            .and_then(|r| r.last())
            .and_then(|r| r.as_str())
            .map(str::to_string);
        let sent: serde_json::Value = match &run_id {
            Some(r) => c
                .get(&format!("/api/runs/{r}"))
                .await
                .unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::Null,
        };
        sent_of.push(sent);
    }
    if json {
        if let Some(o) = v.as_object_mut() {
            o.insert(
                "notes".into(),
                serde_json::json!(notes.iter().map(|(_, n)| n).collect::<Vec<_>>()),
            );
            if let [one] = sent_of.as_slice() {
                o.insert("sent".into(), one["sent"].clone());
                o.insert("sent_says".into(), one["sent_says"].clone());
            }
        }
        // A project that changed between preflight and start fails the
        // command here too.
        return crate::cli::print_json(&v);
    }
    let started = v["changes"].as_array().cloned().unwrap_or_default();
    for (one, sent) in started.iter().zip(&sent_of) {
        let w = &one["change"];
        println!(
            "{} {}",
            paint(render::GREEN, "started"),
            paint(BOLD, w["title"].as_str().unwrap_or(&title))
        );
        if projects.len() > 1 {
            println!("  project   {}", one["project"].as_str().unwrap_or(""));
        }
        if let Some(b) = w["branch"].as_str() {
            println!("  branch    {b}");
        }
        if let Some(d) = w["worktree"].as_str() {
            println!("  worktree  {d}");
        }
        if let Some(sp) = w["spec"].as_str() {
            println!("  spec      {sp}");
        }
        if let Some(says) = sent["sent_says"].as_str() {
            println!("  tasks     {says}");
            for t in sent["sent"].as_array().unwrap_or(&vec![]) {
                println!(
                    "            {}",
                    paint(DIM, t["text"].as_str().unwrap_or(""))
                );
            }
        }
        println!(
            "  {}",
            paint(
                DIM,
                &format!(
                    "devplane change verify {}",
                    one["change_id"].as_str().unwrap_or("")
                )
            )
        );
    }
    // A project that changed between preflight and start is named last.
    if let Some(e) = said_error(&v) {
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
    let v: serde_json::Value = c
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
        return crate::cli::print_json(&v);
    }
    if let Some(e) = said_error(&v) {
        anyhow::bail!("{e}");
    }
    let w = &v["change"];
    println!(
        "{} {}",
        paint(render::GREEN, "adopted"),
        paint(BOLD, w["title"].as_str().unwrap_or(&branch))
    );
    if let Some(b) = w["branch"].as_str() {
        println!("  branch    {b}");
    }
    if let Some(d) = w["worktree"].as_str() {
        println!("  worktree  {d}");
    }
    if let Some(sp) = w["spec"].as_str() {
        println!("  spec      {sp}");
    }
    println!(
        "  {}",
        paint(
            DIM,
            &format!(
                "devplane change verify {}",
                v["change_id"].as_str().unwrap_or("")
            )
        )
    );
    Ok(())
}

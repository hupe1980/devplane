//! `devplane change draft`, `draft-accept`, `draft-reject` and `clarify`: a
//! specification drafted by the project's own methodology, reviewed as a
//! diff, and its open questions answered.

use super::{Answer, REFUSED, host_and_change};
use crate::render::{self, BOLD, DIM, paint};
use anyhow::Result;
use serde::Deserialize;

/// What `change draft` was asked for.
pub(super) struct Ask {
    pub name: String,
    pub agent: Option<String>,
    pub methodology: Option<String>,
    pub project: Option<String>,
    pub change: Option<String>,
    pub dry_run: bool,
    pub intent: Vec<String>,
}

/// The fields a draft's answer carries.
#[derive(Deserialize)]
struct Drafted {
    #[serde(default)]
    change: Option<String>,
    #[serde(default)]
    run: Option<String>,
    #[serde(default)]
    sent: String,
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    folder_says: Option<String>,
    #[serde(default)]
    says: Vec<String>,
    #[serde(default)]
    worktree: Option<String>,
}

/// Prints a refusal and exits with the refusal status.
fn refused(answer: &Answer<Drafted>, json: bool) -> Result<()> {
    if json {
        crate::cli::print_json(answer)?;
    } else if let Some(e) = answer.error() {
        println!("  {:<10}{e}", paint(BOLD, "refused"));
    }
    Err(crate::cli::Exit(REFUSED).into())
}

pub(super) async fn cmd_draft(ask: Ask, json: bool) -> Result<()> {
    let intent = ask.intent.join(" ");
    let c = crate::client::Client::connect_running().await?;
    let change = match &ask.change {
        Some(needle) => {
            Some(super::ids::change(&crate::local::Reader::Host(c.clone()), needle).await?)
        }
        None => None,
    };
    let project = match ask.project {
        Some(p) => p,
        None => {
            let here = std::env::current_dir()?;
            crate::repo::find_repo_root(&here)
                .unwrap_or(here)
                .to_string_lossy()
                .into_owned()
        }
    };
    let body = serde_json::json!({
        "project": project,
        "name": ask.name,
        "agent": ask.agent,
        "methodology": ask.methodology,
        "intent": intent,
        "change": change,
        "preview": ask.dry_run,
    });
    let answer: Answer<Drafted> = c.post_json("/api/changes/draft", &body).await?;
    if answer.error().is_some() {
        return refused(&answer, json);
    }
    if json {
        return crate::cli::print_json(&answer);
    }
    let v = answer.read()?;
    if ask.dry_run {
        println!("{} {}", paint(BOLD, "would send"), v.sent);
        if let Some(f) = &v.folder_says {
            println!("  {}", paint(DIM, f));
        }
        println!("  {}", paint(DIM, "nothing was started"));
        return Ok(());
    }
    println!(
        "{} {}",
        paint(render::GREEN, "drafting"),
        paint(BOLD, &ask.name)
    );
    println!("  sent      {}", v.sent);
    if let Some(f) = &v.folder {
        println!("  into      {f}");
    }
    if let Some(w) = &v.worktree {
        println!("  worktree  {w}");
    }
    for s in &v.says {
        println!("  {}", paint(DIM, s));
    }
    if let (Some(change), Some(run)) = (&v.change, &v.run) {
        println!("  change {change}  run {run}");
        println!(
            "  {}",
            paint(
                DIM,
                &format!(
                    "devplane change review {change}, then devplane change draft-accept {change} or draft-reject {change}"
                )
            )
        );
    }
    Ok(())
}

/// What accepting or rejecting came to.
#[derive(Deserialize)]
struct Decided {
    #[serde(default)]
    spec: Option<String>,
    #[serde(default)]
    removed: Vec<String>,
    #[serde(default)]
    says: Option<String>,
}

pub(super) async fn cmd_decide(change: &str, accept: bool, json: bool) -> Result<()> {
    let (c, id) = host_and_change(change).await?;
    let verb = if accept { "accept" } else { "reject" };
    let answer: Answer<Decided> = c
        .post_json(
            &format!("/api/changes/{id}/draft/{verb}"),
            &serde_json::json!({}),
        )
        .await?;
    if json {
        crate::cli::print_json(&answer)?;
        return match answer.error() {
            Some(_) => Err(crate::cli::Exit(REFUSED).into()),
            None => Ok(()),
        };
    }
    if let Some(e) = answer.error() {
        println!("  {:<10}{e}", paint(BOLD, "refused"));
        return Err(crate::cli::Exit(REFUSED).into());
    }
    let v = answer.read()?;
    if let Some(s) = &v.says {
        println!("{}", paint(render::GREEN, s));
    }
    if let Some(spec) = &v.spec {
        println!("  spec      {spec}");
    }
    for r in &v.removed {
        println!("  removed   {r}");
    }
    Ok(())
}

/// What answering a marker came to.
#[derive(Deserialize)]
struct Clarified {
    #[serde(default)]
    diff: String,
    #[serde(default)]
    written: bool,
    #[serde(default)]
    says: Option<String>,
}

/// `<path>:<line>` as the inbox names a marker.
fn at_of(at: &str) -> Result<(String, u32)> {
    let (path, line) = at
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("name the line as <path>:<line>, as the inbox does"))?;
    let line: u32 = line
        .parse()
        .map_err(|_| anyhow::anyhow!("`{line}` is not a line number"))?;
    Ok((path.to_string(), line))
}

/// The line's current text, from the change's open questions.
async fn line_text(c: &crate::client::Client, id: &str, path: &str, line: u32) -> Result<String> {
    let inbox = crate::local::Reader::Host(c.clone())
        .inbox(None, false)
        .await?;
    inbox
        .items
        .into_iter()
        .filter(|r| r.item.change_id.as_ref().is_some_and(|c| c.as_str() == id))
        .filter_map(|r| r.item.marker)
        .find(|m| m.path == path && m.line == line)
        .map(|m| m.text)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{path}:{line} is not an unresolved line of this change now; `devplane inbox` lists them"
            )
        })
}

pub(super) async fn cmd_clarify(
    change: &str,
    at: &str,
    yes: bool,
    answer: Vec<String>,
    json: bool,
) -> Result<()> {
    let (path, line) = at_of(at)?;
    let (c, id) = host_and_change(change).await?;
    let text = line_text(&c, &id, &path, line).await?;
    let words = answer.join(" ");
    let body = |confirm: bool| {
        serde_json::json!({
            "path": path, "line": line, "text": text, "answer": words, "confirm": confirm,
        })
    };
    let route = format!("/api/changes/{id}/clarify");
    let shown: Answer<Clarified> = c.post_json(&route, &body(false)).await?;
    if let Some(e) = shown.error() {
        if json {
            crate::cli::print_json(&shown)?;
        } else {
            println!("  {:<10}{e}", paint(BOLD, "refused"));
        }
        return Err(crate::cli::Exit(REFUSED).into());
    }
    let diff = shown.read()?.diff.clone();
    match json {
        true => eprint!("{diff}"),
        false => print!("{diff}"),
    }
    if !crate::cli::confirm("Write it?", yes)? {
        eprintln!("{}", paint(DIM, "nothing was written"));
        return Ok(());
    }
    let written: Answer<Clarified> = c.post_json(&route, &body(true)).await?;
    if json {
        return crate::cli::print_json(&written);
    }
    let v = written.read()?;
    if v.written {
        println!(
            "{}",
            paint(render::GREEN, v.says.as_deref().unwrap_or("written"))
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_named_as_the_inbox_names_it() {
        assert_eq!(
            at_of("specs/001-a/spec.md:42").unwrap(),
            ("specs/001-a/spec.md".to_string(), 42)
        );
        assert!(at_of("specs/001-a/spec.md").is_err());
        assert!(at_of("x:y").is_err());
    }
}

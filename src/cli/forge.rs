//! GitHub across every registered project: `devplane pr list` and
//! `devplane issue list`, what needs you first.

use super::raw;
use crate::render::{BOLD, DIM, clip, paint};
use crate::{client, render};
use anyhow::{Context, Result};

/// What GitHub says across every project: one answer, both halves.
///
/// Each list has its own row type: a PR row has every field an issue row
/// needs, so a shared type would silently read one as the other.
#[derive(Debug, serde::Deserialize)]
struct ForgeList {
    #[serde(default)]
    viewer: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    fetched_at: Option<String>,
    /// The configured GitHub host's sign-in.
    #[serde(default)]
    github: serde_json::Value,
    /// Every registered project and the state its GitHub is in.
    #[serde(default)]
    projects: Vec<ForgeProject>,
    #[serde(default)]
    issues: Vec<ForgeIssueRow>,
    #[serde(default)]
    pull_requests: Vec<ForgePrRow>,
}

#[derive(Debug, serde::Deserialize)]
struct ForgeProject {
    project_name: String,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    github: serde_json::Value,
    /// Why the last poll failed; the rows are the last good ones.
    #[serde(default)]
    stale: Option<String>,
    #[serde(default)]
    issues_more: usize,
    #[serde(default)]
    pull_requests_more: usize,
}

#[derive(Debug, serde::Deserialize)]
struct ForgeIssueRow {
    project_name: String,
    number: u64,
    title: String,
    url: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    assigned_to_me: bool,
}

#[derive(Debug, serde::Deserialize)]
struct ForgePrRow {
    project_name: String,
    number: u64,
    title: String,
    url: String,
    status: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    mine: bool,
    #[serde(default)]
    review_requested: bool,
}

/// What a sign-in state says instead of a list, if anything: not signed in,
/// expired, rate limited or unreachable are never an empty list. `flag` is
/// the `--host` a login for this host needs.
fn sign_in_says(github: &serde_json::Value, flag: &str) -> Option<String> {
    match github["state"].as_str()? {
        "signed_out" => Some(format!(
            "Not signed in to GitHub — run `devplane auth login{flag}`, or sign in from Setup."
        )),
        "pending" => Some(format!(
            "Signing in to GitHub — enter {} at {}.",
            github["user_code"].as_str().unwrap_or("the code"),
            github["verification_uri"]
                .as_str()
                .unwrap_or("GitHub's device page")
        )),
        "expired" => Some(format!(
            "GitHub sign-in expired: GitHub no longer accepts the token — run `devplane auth login{flag}`."
        )),
        "rate_limited" => Some(format!(
            "GitHub's rate limit is spent; it resets at {} and the lists below are from before.",
            github["until"]
                .as_str()
                .unwrap_or("a time GitHub did not say")
        )),
        "unreachable" => Some(format!(
            "GitHub is unreachable ({}); the lists below are from the last read.",
            github["why"].as_str().unwrap_or("no answer")
        )),
        _ => None,
    }
}

/// What the page as a whole says before any project: the configured host's
/// sign-in, a failed poll, or nothing read yet. `false` when there is nothing
/// to list below it.
fn forge_preamble(list: &ForgeList) -> bool {
    if let Some(says) = sign_in_says(&list.github, "") {
        println!("{}\n", paint(render::YELLOW, &says));
    }
    if let Some(e) = &list.error {
        println!(
            "{}\n\n  {}\n",
            paint(render::YELLOW, "GitHub could not be read."),
            paint(DIM, e)
        );
        return false;
    }
    if list.fetched_at.is_none() {
        println!(
            "{}",
            paint(
                DIM,
                "GitHub has not been read yet — the host polls it a few seconds after it starts."
            )
        );
        return false;
    }
    if let Some(v) = &list.viewer {
        println!("{}", paint(DIM, &format!("as {v} · what needs you first")));
    }
    true
}

/// What one project's heading says beside its name: why it has no rows, or
/// why its rows might be old. `None` for a project read cleanly.
fn project_says(p: &ForgeProject, default_host: Option<&str>) -> Option<String> {
    let host = p.host.as_deref().unwrap_or("");
    let flag = match (host, default_host) {
        ("", _) => String::new(),
        (h, Some(d)) if h == d => String::new(),
        (h, _) => format!(" --host {h}"),
    };
    match p.github["state"].as_str() {
        Some("not_github") => Some(format!(
            "not a GitHub repository — {}",
            p.github["why"].as_str().unwrap_or("no GitHub remote")
        )),
        Some("unknown") => Some("not read yet".into()),
        Some("signed_in") | None => p.stale.as_ref().map(|e| format!("stale — {e}")),
        Some(_) => sign_in_says(&p.github, &flag).map(|s| match &p.stale {
            Some(e) => format!("{s} ({e})"),
            None => s,
        }),
    }
}

/// Lists one half of the forge project by project: each heading with what
/// its GitHub says, its rows, and how many GitHub counts past the page.
fn print_by_project<R>(
    list: &ForgeList,
    rows: &[R],
    name_of: impl Fn(&R) -> &str,
    more_of: impl Fn(&ForgeProject) -> usize,
    print_row: impl Fn(&R),
    empty: &str,
) {
    let default_host = list.github["host"].as_str();
    let mut printed = false;
    let mut quiet = Vec::new();
    for p in &list.projects {
        let mine: Vec<&R> = rows
            .iter()
            .filter(|r| name_of(r) == p.project_name)
            .collect();
        let says = project_says(p, default_host);
        // A project with no GitHub remote is named once at the end, not given
        // a heading of its own.
        if mine.is_empty() && p.github["state"] == "not_github" {
            quiet.push(p.project_name.as_str());
            continue;
        }
        if mine.is_empty() && says.is_none() && more_of(p) == 0 {
            continue;
        }
        if printed {
            println!();
        }
        printed = true;
        match &says {
            Some(s) => println!(
                "{}  {}",
                paint(BOLD, &p.project_name),
                paint(render::YELLOW, s)
            ),
            None => println!("{}", paint(BOLD, &p.project_name)),
        }
        for r in &mine {
            print_row(r);
        }
        if more_of(p) > 0 {
            println!(
                "  {}",
                paint(
                    DIM,
                    &format!("… and {} more open at GitHub, not read", more_of(p))
                )
            );
        }
    }
    if !printed {
        println!("{}", paint(render::GREEN, empty));
    }
    if !quiet.is_empty() {
        println!(
            "\n{}",
            paint(DIM, &format!("not on GitHub: {}", quiet.join(", ")))
        );
    }
}

/// One half of `/api/forge` under `--json`: the rows asked for, with the
/// envelope that says how current they are — never the other half.
fn forge_half(v: &serde_json::Value, rows: &str, more: &str) -> serde_json::Value {
    let projects: Vec<serde_json::Value> = v["projects"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|p| {
                    serde_json::json!({
                        "project": p["project"], "project_name": p["project_name"],
                        "repo": p["repo"], "host": p["host"], "github": p["github"],
                        "stale": p["stale"], "read_at": p["read_at"], more: p[more],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    serde_json::json!({
        "viewer": v["viewer"],
        "error": v["error"],
        "fetched_at": v["fetched_at"],
        "github": v["github"],
        "stale": v["stale"],
        "projects": projects,
        rows: v[rows],
        more: v[more],
    })
}

pub async fn cmd_issues(json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let v = raw(&c, "/api/forge").await?;
    if json {
        return super::print_json(&forge_half(&v, "issues", "issues_more"));
    }
    let list: ForgeList = serde_json::from_value(v).context("reading /api/forge")?;
    if !forge_preamble(&list) {
        return Ok(());
    }
    print_by_project(
        &list,
        &list.issues,
        |i| &i.project_name,
        |p| p.issues_more,
        |i| {
            let mark = if i.assigned_to_me {
                paint(render::YELLOW, "◆")
            } else {
                paint(DIM, "○")
            };
            let labels = if i.labels.is_empty() {
                String::new()
            } else {
                format!("  [{}]", i.labels.join(", "))
            };
            println!(
                "  {} #{:<5} {}{}",
                mark,
                i.number,
                clip(&i.title, 64),
                paint(DIM, &labels)
            );
            println!("    {}", paint(DIM, &i.url));
        },
        "No open issues on any registered project.",
    );
    println!(
        "\n{}",
        paint(
            DIM,
            "◆ assigned to you · devplane change start --issue <n> --project <project> turns one into a change"
        )
    );
    Ok(())
}

pub async fn cmd_prs(json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let v = raw(&c, "/api/forge").await?;
    if json {
        return super::print_json(&forge_half(&v, "pull_requests", "pull_requests_more"));
    }
    let list: ForgeList = serde_json::from_value(v).context("reading /api/forge")?;
    if !forge_preamble(&list) {
        return Ok(());
    }
    print_by_project(
        &list,
        &list.pull_requests,
        |p| &p.project_name,
        |p| p.pull_requests_more,
        |p| {
            let needs_me = p.review_requested
                || (p.mine
                    && matches!(
                        p.status.as_str(),
                        "failing" | "changes_requested" | "ready_to_merge"
                    ));
            let mark = if needs_me {
                paint(render::YELLOW, "◆")
            } else {
                paint(DIM, "○")
            };
            let status = match p.status.as_str() {
                "failing" => paint(render::RED, "red"),
                "ready_to_merge" => paint(render::GREEN, "approved · green"),
                "changes_requested" => paint(render::YELLOW, "changes requested"),
                other => paint(DIM, &other.replace('_', " ")),
            };
            let who = if p.review_requested {
                "  review asked of you"
            } else if p.mine {
                "  yours"
            } else {
                ""
            };
            println!(
                "  {} #{:<5} {:<44} {}{}{}",
                mark,
                p.number,
                clip(&p.title, 44),
                status,
                if p.draft {
                    paint(DIM, " · draft")
                } else {
                    String::new()
                },
                paint(DIM, who)
            );
            println!("    {}", paint(DIM, &p.url));
        },
        "No open pull requests on any registered project.",
    );
    Ok(())
}

/// The issues one repository offers as work: `devplane issue list --ready`.
pub async fn cmd_ready_issues(
    cwd: Option<std::path::PathBuf>,
    label: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let c = client::Client::connect_running().await?;
    let cwd = match cwd {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    let v: serde_json::Value = c
        .post_json(
            "/api/issues",
            &serde_json::json!({ "cwd": cwd.to_string_lossy(), "label": label }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        anyhow::bail!("{e}");
    }
    let empty = vec![];
    let issues = v.as_array().unwrap_or(&empty);
    if issues.is_empty() {
        println!("{}", paint(DIM, "no issues are on offer here"));
        return Ok(());
    }
    for i in issues {
        println!(
            "{:>6}  {}",
            paint(BOLD, &format!("#{}", i["number"].as_u64().unwrap_or(0))),
            clip(i["title"].as_str().unwrap_or(""), 70)
        );
    }
    println!(
        "\n  {}",
        paint(DIM, "devplane change start --issue <number>")
    );
    Ok(())
}

#[cfg(test)]
mod forge_tests {
    use super::forge_half;
    use serde_json::json;

    #[test]
    fn each_forge_list_prints_its_own_half() {
        let v = json!({
            "viewer": "octocat", "fetched_at": "t", "github": {"state": "signed_in"},
            "issues": [{"number": 7}], "pull_requests": [{"number": 2}],
            "issues_more": 3, "pull_requests_more": 23,
            "projects": [{"project_name": "app", "github": {"state": "signed_in"},
                          "issues_more": 3, "pull_requests_more": 23}],
        });
        let issues = forge_half(&v, "issues", "issues_more");
        let prs = forge_half(&v, "pull_requests", "pull_requests_more");
        assert_ne!(issues, prs);
        assert_eq!(issues["issues"][0]["number"], 7);
        assert!(issues.get("pull_requests").is_none());
        assert_eq!(prs["pull_requests_more"], 23);
        assert!(prs.get("issues").is_none());
        assert_eq!(prs["projects"][0]["pull_requests_more"], 23);
        assert!(prs["projects"][0].get("issues_more").is_none());
    }
}

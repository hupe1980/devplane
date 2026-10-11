//! GitHub across every registered project: `devplane pr list` and
//! `devplane issue list`, what needs you first.

use super::raw_as;
use crate::github::{HostView, SignIn};
use crate::render::{BOLD, DIM, clip, paint};
use crate::{client, render};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// What GitHub says across every project: one answer, both halves, as
/// `/api/forge` serialises it.
///
/// Each list has its own row type: a PR row has every field an issue row
/// needs, so a shared type would silently read one as the other.
#[derive(Debug, Deserialize)]
struct ForgeList {
    viewer: Option<String>,
    error: Option<String>,
    fetched_at: Option<String>,
    issues: Vec<ForgeRow<crate::core::ForgeIssue>>,
    pull_requests: Vec<ForgeRow<crate::core::ForgePullRequest>>,
    /// Projects whose last poll failed; their rows are the last good ones.
    stale: Vec<StaleProject>,
    /// The configured GitHub host's sign-in.
    github: HostView,
    /// Every registered project and the state its GitHub is in.
    projects: Vec<ForgeProject>,
    issues_more: usize,
    pull_requests_more: usize,
}

/// One row of either list: the item, with the project it belongs to.
#[derive(Debug, Serialize, Deserialize)]
struct ForgeRow<T> {
    project: String,
    project_name: String,
    needs_you: bool,
    #[serde(flatten)]
    item: T,
}

/// A project whose last poll failed.
#[derive(Debug, Serialize, Deserialize)]
struct StaleProject {
    project: String,
    project_name: String,
    error: String,
    last_good: String,
}

#[derive(Debug, Deserialize)]
struct ForgeProject {
    project: String,
    project_name: String,
    repo: Option<String>,
    host: Option<String>,
    github: ProjectGitHub,
    /// Why the last poll failed; the rows are the last good ones.
    stale: Option<String>,
    read_at: Option<String>,
    issues_more: usize,
    pull_requests_more: usize,
}

/// Which state a project's GitHub is in: its host's sign-in, or why it has
/// none.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ProjectGitHub {
    Host(HostView),
    Other(NoSignIn),
}

/// A project with no host to sign in to: not a GitHub repository, or not
/// polled yet.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum NoSignIn {
    NotGithub { why: String },
    Unknown,
}

/// What a sign-in state says instead of a list, if anything: not signed in,
/// expired, rate limited or unreachable are never an empty list. `flag` is
/// the `--host` a login for this host needs.
fn sign_in_says(github: &SignIn, flag: &str) -> Option<String> {
    match github {
        SignIn::SignedOut => Some(format!(
            "Not signed in to GitHub — run `devplane auth login{flag}`, or sign in from Setup."
        )),
        SignIn::Pending {
            user_code,
            verification_uri,
            ..
        } => Some(format!(
            "Signing in to GitHub — enter {user_code} at {verification_uri}."
        )),
        SignIn::Expired => Some(format!(
            "GitHub sign-in expired: GitHub no longer accepts the token — run `devplane auth login{flag}`."
        )),
        SignIn::RateLimited { until } => Some(format!(
            "GitHub's rate limit is spent; it resets at {until} and the lists below are from before."
        )),
        SignIn::Unreachable { why, .. } => Some(format!(
            "GitHub is unreachable ({why}); the lists below are from the last read."
        )),
        SignIn::SignedIn { .. } => None,
    }
}

/// What the page as a whole says before any project: the configured host's
/// sign-in, a failed poll, or nothing read yet. `false` when there is nothing
/// to list below it.
fn forge_preamble(list: &ForgeList) -> bool {
    if let Some(says) = sign_in_says(&list.github.sign_in, "") {
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
    match &p.github {
        ProjectGitHub::Other(NoSignIn::NotGithub { why }) => {
            Some(format!("not a GitHub repository — {why}"))
        }
        ProjectGitHub::Other(NoSignIn::Unknown) => Some("not read yet".into()),
        ProjectGitHub::Host(h) => match sign_in_says(&h.sign_in, &flag) {
            None => p.stale.as_ref().map(|e| format!("stale — {e}")),
            Some(s) => Some(match &p.stale {
                Some(e) => format!("{s} ({e})"),
                None => s,
            }),
        },
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
    let default_host = Some(list.github.host.as_str());
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
        if mine.is_empty() && matches!(p.github, ProjectGitHub::Other(NoSignIn::NotGithub { .. })) {
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
fn forge_half<R: Serialize>(
    v: &ForgeList,
    rows_key: &str,
    rows: &[R],
    more_key: &str,
    more: usize,
    more_of: impl Fn(&ForgeProject) -> usize,
) -> serde_json::Value {
    let projects: Vec<_> = v
        .projects
        .iter()
        .map(|p| {
            serde_json::json!({
                "project": p.project, "project_name": p.project_name,
                "repo": p.repo, "host": p.host, "github": p.github,
                "stale": p.stale, "read_at": p.read_at, more_key: more_of(p),
            })
        })
        .collect();
    serde_json::json!({
        "viewer": v.viewer,
        "error": v.error,
        "fetched_at": v.fetched_at,
        "github": v.github,
        "stale": v.stale,
        "projects": projects,
        rows_key: rows,
        more_key: more,
    })
}

/// The issues half under `--json`.
fn issues_half(v: &ForgeList) -> serde_json::Value {
    forge_half(v, "issues", &v.issues, "issues_more", v.issues_more, |p| {
        p.issues_more
    })
}

/// The pull-requests half under `--json`.
fn prs_half(v: &ForgeList) -> serde_json::Value {
    forge_half(
        v,
        "pull_requests",
        &v.pull_requests,
        "pull_requests_more",
        v.pull_requests_more,
        |p| p.pull_requests_more,
    )
}

pub async fn cmd_issues(json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let list: ForgeList = raw_as(&c, "/api/forge").await?;
    if json {
        return super::print_json(&issues_half(&list));
    }
    if !forge_preamble(&list) {
        return Ok(());
    }
    print_by_project(
        &list,
        &list.issues,
        |i| &i.project_name,
        |p| p.issues_more,
        |row| {
            let i = &row.item;
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
    let list: ForgeList = raw_as(&c, "/api/forge").await?;
    if json {
        return super::print_json(&prs_half(&list));
    }
    if !forge_preamble(&list) {
        return Ok(());
    }
    print_by_project(
        &list,
        &list.pull_requests,
        |p| &p.project_name,
        |p| p.pull_requests_more,
        |row| {
            let p = &row.item;
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

/// What `/api/issues` answers: the issues on offer, in the board's shape, or
/// why they could not be read.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ReadyIssues {
    Refused { error: String },
    Issues(Vec<crate::core::ForgeIssue>),
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
    let v: ReadyIssues = c
        .post_json(
            "/api/issues",
            &serde_json::json!({ "cwd": cwd.to_string_lossy(), "label": label }),
        )
        .await?;
    if json {
        return crate::cli::print_json(&v);
    }
    let issues = match v {
        ReadyIssues::Refused { error } => anyhow::bail!("{error}"),
        ReadyIssues::Issues(issues) => issues,
    };
    if issues.is_empty() {
        println!("{}", paint(DIM, "no issues are on offer here"));
        return Ok(());
    }
    for i in &issues {
        println!(
            "{:>6}  {}",
            paint(BOLD, &format!("#{}", i.number)),
            clip(&i.title, 70)
        );
    }
    println!(
        "\n  {}",
        paint(DIM, "devplane change start --issue <number>")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ForgeList, issues_half, prs_half};
    use serde_json::json;

    #[test]
    fn each_forge_list_prints_its_own_half() {
        let signed_in = json!({"host": "github.com", "state": "signed_in", "login": "octocat",
                               "scopes": [], "last_read": null, "device_flow": true});
        let v = json!({
            "viewer": "octocat", "error": null, "fetched_at": "t",
            "issues": [{"project": "p-1", "project_name": "app", "needs_you": false,
                        "number": 7, "title": "t", "url": "u", "labels": [],
                        "assigned_to_me": false, "updated_at": null}],
            "pull_requests": [{"project": "p-1", "project_name": "app", "needs_you": false,
                               "number": 2, "title": "t", "url": "u", "status": "open",
                               "draft": false, "mine": false, "review_requested": false,
                               "head_ref": "b", "updated_at": null}],
            "stale": [],
            "github": signed_in,
            "projects": [{"project": "p-1", "project_name": "app", "repo": "o/app",
                          "host": "github.com", "github": signed_in, "stale": null,
                          "read_at": "t", "issues_more": 3, "pull_requests_more": 23}],
            "issues_more": 3, "pull_requests_more": 23,
        });
        let list: ForgeList = serde_json::from_value(v).expect("the host's own shape");
        let issues = issues_half(&list);
        let prs = prs_half(&list);
        assert_ne!(issues, prs);
        assert_eq!(issues["issues"][0]["number"], 7);
        assert!(issues.get("pull_requests").is_none());
        assert_eq!(prs["pull_requests_more"], 23);
        assert!(prs.get("issues").is_none());
        assert_eq!(prs["projects"][0]["pull_requests_more"], 23);
        assert!(prs["projects"][0].get("issues_more").is_none());
        assert_eq!(prs["projects"][0]["github"]["state"], "signed_in");
    }
}

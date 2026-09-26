//! Seeing what is running: the board, a run, its transcript, and reaching it.

use super::{fetch_run, raw, urlencode};
use crate::render::{BOLD, DIM, ago, clip, paint, state_marker, surface};
use crate::{client, render};
use anyhow::{Context, Result};

fn print_board(board: &render::BoardResponse) {
    // Grouped by project. The board is sorted by recency, so first appearance
    // decides a project's place and rows keep their order inside it.
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<&render::RunView>> =
        std::collections::HashMap::new();
    for r in &board.runs {
        let key = r
            .project_name
            .clone()
            .unwrap_or_else(|| "(no project)".into());
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(r);
    }

    for (i, project) in order.iter().enumerate() {
        let rows = &groups[project];
        if i > 0 {
            println!();
        }
        // What GitHub says is open here, on the heading.
        let forge = rows
            .iter()
            .find_map(|r| r.project.as_deref())
            .and_then(|id| board.forge.get(id))
            .map(forge_suffix)
            .unwrap_or_default();
        println!(
            "{}{}{}",
            paint(BOLD, project),
            if rows.len() > 1 {
                paint(DIM, &format!("  ·  {} sessions", rows.len()))
            } else {
                String::new()
            },
            paint(DIM, &forge)
        );

        // A short name shared by two sessions falls back to the id.
        let labels: Vec<String> = rows.iter().map(|r| session_label(r, project)).collect();
        for (r, label) in rows.iter().zip(&labels) {
            let label = if labels.iter().filter(|l| *l == label).count() > 1 {
                r.id.chars().take(8).collect()
            } else {
                label.clone()
            };
            let ctx = r
                .context_percent
                .map(|p| format!("{p:3.0}%"))
                .unwrap_or_else(|| "   -".into());
            // Three states: a cost, a genuine nothing, and a gap (telemetry
            // nobody received, which the vendor never replays) shown as `?`.
            let cost = if r.cost_usd > 0.0 {
                format!("${:.2}", r.cost_usd)
            } else if r.cost_unknown {
                "?".into()
            } else {
                "-".into()
            };
            // Trimmed, so an empty summary leaves no trailing spaces in a pipe.
            let line = format!(
                "  {} {:<10} {:<8} {:>5} {:>7} {:>5}  {}",
                state_marker(&r.state),
                label,
                paint(DIM, surface(r)),
                ctx,
                cost,
                paint(DIM, &ago(r.idle_seconds)),
                clip(render::summary_line(r), 58)
            );
            println!("{}", line.trim_end());
            if let Some(w) = &r.worktree {
                println!("    {}", paint(DIM, &format!("worktree {}", clip(w, 68))));
            }
            // What the run was sent, or the sentence saying nothing was.
            if !r.sent_says.is_empty() {
                println!("    {}", paint(DIM, &r.sent_says));
            }
        }
    }
}

/// `  ·  4 issues · 2 PRs (1 need you)`, or nothing when nothing is open.
///
/// A project whose last poll failed says so: its counts are the last good ones.
fn forge_suffix(c: &render::ForgeCounts) -> String {
    if c.issues == 0 && c.pull_requests == 0 {
        return match &c.stale {
            Some(_) => "  ·  github unreadable".into(),
            None => String::new(),
        };
    }
    let mut out = format!(
        "  ·  {} issue{} · {} PR{}",
        c.issues,
        if c.issues == 1 { "" } else { "s" },
        c.pull_requests,
        if c.pull_requests == 1 { "" } else { "s" }
    );
    if c.needs_you > 0 {
        out.push_str(&format!(
            " ({} need{} you)",
            c.needs_you,
            if c.needs_you == 1 { "s" } else { "" }
        ));
    }
    if c.stale.is_some() {
        out.push_str(" · stale");
    }
    out
}

/// What distinguishes one session of a project from another: the name without
/// its `<project>-` prefix, which the header already shows.
fn session_label(run: &render::RunView, project: &str) -> String {
    let name = run.name.as_deref().unwrap_or("");
    let short = name
        .strip_prefix(project)
        .and_then(|rest| rest.strip_prefix('-'))
        .unwrap_or(name);
    if short.is_empty() {
        // Nothing left: fall back to the id `show` and `focus` take.
        return run.id.chars().take(8).collect();
    }
    clip(short, 10)
}

pub async fn cmd_ls(all: bool, project: Option<&str>, needs_you: bool, json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    // A filter may match dormant sessions, so it widens the fetch and narrows
    // here.
    let all = all || project.is_some() || needs_you;
    let path = if all {
        "/api/board?all=true"
    } else {
        "/api/board"
    };
    let mut board: render::BoardResponse = c.get_as(path).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&raw(&c, path).await?)?);
        return Ok(());
    }

    if let Some(needle) = project {
        let needle = needle.to_lowercase();
        board.runs.retain(|r| {
            r.project_name
                .as_deref()
                .is_some_and(|p| p.to_lowercase().contains(&needle))
        });
        if board.runs.is_empty() {
            println!(
                "No session matches {}.\n\n{}",
                paint(BOLD, needle.as_str()),
                paint(DIM, "devplane ls --all lists every project.")
            );
            return Ok(());
        }
    }
    if needs_you {
        board
            .runs
            .retain(|r| r.state == "waiting" || r.state == "failed");
        if board.runs.is_empty() {
            println!("{}", paint(DIM, "Nothing is waiting on you."));
            return Ok(());
        }
    }

    if board.runs.is_empty() && board.summary.dormant > 0 {
        println!(
            "Nothing in play.\n\n  {}\n\n{}",
            paint(
                DIM,
                &format!(
                    "{} session(s) exist and have been quiet for hours — editor tabs, usually.",
                    board.summary.dormant
                )
            ),
            paint(DIM, "devplane ls --all shows them.")
        );
        return Ok(());
    }

    if board.runs.is_empty() {
        // Not Claude-only: any connected agent's session would be listed.
        println!(
            "No agent sessions are running.\n\n\
             Start one — in Claude Code, Codex or Copilot after `devplane connect <agent>`, \n\
             or with `devplane change start` — and it appears here.{}",
            match crate::observe::locate::claude_binary() {
                Some(_) => String::new(),
                // Only the roster of Claude Code sessions needs its binary.
                None => format!(
                    "\n\n{} Point at it with {} to list its sessions without hooks.",
                    paint(DIM, "No `claude` binary was found."),
                    paint(BOLD, "DEVPLANE_CLAUDE_BIN=/path/to/claude")
                ),
            }
        );
        let note = unwatched_note();
        if !note.is_empty() {
            println!("\n{}", paint(DIM, &note));
        }
        return Ok(());
    }

    // A filtered view gets its own header, not the whole machine's.
    let filtered = project.is_some() || needs_you;
    if filtered {
        let projects: std::collections::HashSet<&str> = board
            .runs
            .iter()
            .filter_map(|r| r.project_name.as_deref())
            .collect();
        println!(
            "{} in {}",
            paint(
                BOLD,
                &format!(
                    "{} session{}",
                    board.runs.len(),
                    if board.runs.len() == 1 { "" } else { "s" }
                )
            ),
            format_args!(
                "{} project{}",
                projects.len(),
                if projects.len() == 1 { "" } else { "s" }
            )
        );
        println!();
        print_board(&board);
        return Ok(());
    }

    let s = &board.summary;
    // Every session is in exactly one of these, so the breakdown sums to the
    // total.
    println!(
        "{} · {} · {} working · {} need you · {} idle{}{}{}{}",
        paint(BOLD, &format!("{} projects", s.projects)),
        format_args!("{} sessions", s.runs),
        s.working,
        if s.needs_you > 0 {
            paint(render::YELLOW, &s.needs_you.to_string())
        } else {
            "0".into()
        },
        s.idle,
        if s.failed > 0 {
            paint(render::RED, &format!(" · {} failed", s.failed))
        } else {
            String::new()
        },
        if s.cost_usd > 0.0 {
            paint(DIM, &format!(" · ${:.2}", s.cost_usd))
        } else {
            String::new()
        },
        // Unanswered questions whose sessions are gone: their own clause, since
        // they are not sessions but still need you.
        if s.asks_waiting > 0 {
            paint(
                render::YELLOW,
                &format!(
                    " · {} unanswered from earlier session{}",
                    s.asks_waiting,
                    if s.asks_waiting == 1 { "" } else { "s" }
                ),
            )
        } else {
            String::new()
        },
        // GitHub's half of the picture, once the forge has been read.
        if s.open_issues + s.open_prs + s.forge_stale > 0 {
            let mut t = String::new();
            if s.open_issues + s.open_prs > 0 {
                t.push_str(&format!(" · {} issues · {} PRs", s.open_issues, s.open_prs));
            }
            if s.forge_needs_you > 0 {
                t.push_str(&paint(
                    render::YELLOW,
                    &format!(" ({} need you)", s.forge_needs_you),
                ));
            }
            if s.forge_stale > 0 {
                t.push_str(&format!(
                    " · GitHub unreadable for {} project{}",
                    s.forge_stale,
                    if s.forge_stale == 1 { "" } else { "s" }
                ));
            }
            paint(DIM, &t)
        } else {
            String::new()
        }
    );
    if !all && s.dormant > 0 {
        println!(
            "{}",
            paint(
                DIM,
                &format!(
                    "{} quiet (nothing heard for hours) — devplane ls --all",
                    s.dormant
                )
            )
        );
    }
    // Claude Code's roster carries no cost or context figure, so without
    // `connect` those columns are `-` on every row; say why.
    if !board.runs.is_empty()
        && board
            .runs
            .iter()
            .all(|r| r.cost_usd == 0.0 && r.context_percent.is_none())
    {
        println!(
            "{}",
            paint(
                DIM,
                "cost and context are blank — devplane connect claude adds them",
            )
        );
    }
    println!();

    print_board(&board);

    Ok(())
}

pub async fn cmd_show(run: &str, json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let v = fetch_run(&c, run).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    let str_of = |k: &str| v[k].as_str().unwrap_or("-").to_string();
    println!(
        "{}  {}",
        paint(BOLD, &str_of("name")),
        paint(DIM, v["id"].as_str().unwrap_or(""))
    );
    println!("  state      {}", v["state"].as_str().unwrap_or("?"));
    println!("  agent      {} ({})", str_of("agent"), str_of("mode"));
    // From the run's record, never parsed; a watched session says it carries
    // none rather than guessing from the diff.
    println!("  tasks      {}", str_of("sent_says"));
    for t in v["sent"].as_array().unwrap_or(&vec![]) {
        println!(
            "             {}",
            paint(DIM, t["text"].as_str().unwrap_or(""))
        );
    }
    println!("  where      {}", str_of("cwd"));
    if let Some(w) = v["worktree"].as_str() {
        println!("  worktree   {w}");
    }
    println!("  model      {}", str_of("model"));
    println!("  surface    {}", str_of("entrypoint"));
    if let Some(pid) = v["pid"].as_u64() {
        println!("  pid        {pid}");
    }

    let t = &v["totals"];
    println!(
        "  cost       ${:.4} over {} requests",
        t["cost_usd"].as_f64().unwrap_or(0.0),
        t["api_requests"].as_u64().unwrap_or(0)
    );
    println!(
        "  tools      {} calls, {} errors",
        t["tool_calls"].as_u64().unwrap_or(0),
        t["errors"].as_u64().unwrap_or(0)
    );
    // What the session changed, as the status line reports it (or nothing).
    let (added, removed) = (
        t["lines_added"].as_u64().unwrap_or(0),
        t["lines_removed"].as_u64().unwrap_or(0),
    );
    if added > 0 || removed > 0 {
        println!("  changed    +{added} −{removed} lines");
    }
    // A rate-limit percentage with no reset time is a number with no verb.
    if let Some(pct) = t["rate_limit_percent"].as_f64() {
        let window = match t["rate_limit_window"].as_str() {
            Some("five_hour") => "5-hour",
            Some("seven_day") => "7-day",
            Some("spend_limit") => "spend limit",
            other => other.unwrap_or("window"),
        };
        let when = match t["rate_limit_resets_at"].as_i64() {
            Some(at) => match jiff::Timestamp::from_second(at) {
                Ok(ts) => format!(", resets {}", render::until(ts)),
                Err(_) => String::new(),
            },
            None => String::new(),
        };
        println!("  limits     {window} {pct:.0}% used{when}");
    }
    if let Some(v) = v["claude_version"].as_str() {
        println!("  harness    Claude Code {v}");
    }

    if let Some(b) = v["blocked_on"].as_object() {
        println!("\n{}", paint(render::YELLOW, "blocked on"));
        if let Some(m) = b.get("message").and_then(|m| m.as_str()) {
            println!("  {m}");
        }
        for (n, o) in b
            .get("options")
            .and_then(|o| o.as_array())
            .unwrap_or(&vec![])
            .iter()
            .enumerate()
        {
            println!("  {}. {}", n + 1, o.as_str().unwrap_or(""));
        }
    }

    let empty = vec![];
    // What the agent says it is going to do, where it says anything.
    let plan = v["plan"].as_array().unwrap_or(&empty);
    if !plan.is_empty() {
        println!("\n{}", paint(BOLD, "plan"));
        for step in plan {
            let status = step["status"].as_str().unwrap_or("pending");
            println!(
                "  {} {}",
                match status {
                    "completed" => paint(render::GREEN, "✓"),
                    "in_progress" => paint(render::BLUE, "▸"),
                    _ => paint(DIM, "·"),
                },
                clip(step["content"].as_str().unwrap_or(""), 74)
            );
        }
    }

    let tools = v["recent_tools"].as_array().unwrap_or(&empty);
    if !tools.is_empty() {
        println!("\n{}", paint(BOLD, "recent tools"));
        for t in tools.iter().rev().take(10) {
            println!(
                "  {:<14} {}",
                t["tool"].as_str().unwrap_or("?"),
                match t["ok"].as_bool() {
                    Some(true) => paint(render::GREEN, "ok"),
                    Some(false) => paint(render::RED, "failed"),
                    None => paint(DIM, "running"),
                }
            );
        }
    }

    // The tail of the conversation; `devplane watch <run>` is the live version.
    if v["mode"] == "driven" {
        let said: Vec<render::Message> = c
            .get_as(&format!("/api/runs/{run}/messages?limit=6"))
            .await
            .unwrap_or_default();
        if !said.is_empty() {
            println!("\n{}", paint(BOLD, "said"));
            for m in &said {
                if m.role == "thought" {
                    continue;
                }
                println!(
                    "  {} {}",
                    paint(
                        if m.role == "user" {
                            render::BLUE
                        } else {
                            render::MAGENTA
                        },
                        &format!("{:>5}", if m.role == "user" { "you" } else { "agent" })
                    ),
                    clip(m.text.trim(), 70)
                );
            }
            println!("  {}", paint(DIM, &format!("devplane watch {run}")));
        }
    }
    Ok(())
}

/// Prints one fragment of a conversation, speaker first.
fn print_message(role: &str, text: &str, thinking: bool) {
    if role == "thought" && !thinking {
        return;
    }
    let (label, colour) = match role {
        "user" => ("you", render::BLUE),
        "thought" => ("···", DIM),
        _ => ("agent", render::MAGENTA),
    };
    // Indented under its speaker so a wrapped paragraph reads as one voice.
    let body = text
        .trim_end()
        .lines()
        .collect::<Vec<_>>()
        .join("\n       ");
    println!("{} {}", paint(colour, &format!("{label:>5}")), body);
}

/// Follows a driven run's conversation.
async fn follow_run(run: &str, thinking: bool, history: i64) -> Result<()> {
    let c = client::Client::connect_running().await?;

    // Only a driven run has a conversation to follow; say so otherwise.
    let detail = fetch_run(&crate::local::Reader::Host(c.clone()), run).await?;
    let mode = detail["mode"].as_str().unwrap_or("observed");
    if mode != "driven" {
        anyhow::bail!(
            "that is a session Devplane watches, not one it drives, and the documented \
             channels carry no transcript: hooks report lifecycle and tool inputs, and \
             telemetry redacts prompts and responses.\n\n\
             Its own window already has the conversation; resume it in a terminal:\n  devplane attach {run}"
        );
    }

    let seen: Vec<render::Message> = c
        .get(&format!("/api/runs/{run}/messages?limit={history}"))
        .await?;
    for m in &seen {
        print_message(m.role.as_str(), &m.text, thinking);
    }
    println!("{}", paint(DIM, "— following, ctrl-c to stop —"));

    let res = reqwest::Client::new()
        .get(format!(
            "{}/api/stream?run={}",
            c.base_url(),
            urlencode(run)
        ))
        .bearer_auth(c.token())
        .send()
        .await?;
    let mut stream = res.bytes_stream();
    use futures_util::StreamExt;
    let mut buffered = String::new();
    while let Some(chunk) = stream.next().await {
        buffered.push_str(&String::from_utf8_lossy(&chunk?));
        // A chunk can split an SSE line, so the partial tail is kept for the next.
        let complete = match buffered.rfind('\n') {
            Some(i) => buffered.drain(..=i).collect::<String>(),
            None => continue,
        };
        for line in complete.lines() {
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            match v["frame"].as_str() {
                Some("message") => print_message(
                    v["role"].as_str().unwrap_or("agent"),
                    v["text"].as_str().unwrap_or(""),
                    thinking,
                ),
                // The turn ending marks the answer complete, not paused.
                Some("event") if v["event"]["type"] == "turn_ended" => {
                    println!("{}", paint(DIM, "— turn ended —"));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Hands the terminal to Claude Code, resuming the session in its own
/// directory.
pub async fn cmd_attach(run: &str) -> Result<()> {
    let c = client::Client::connect_running().await?;
    let v = fetch_run(&crate::local::Reader::Host(c.clone()), run).await?;
    let dir = v["worktree"]
        .as_str()
        .or_else(|| v["cwd"].as_str())
        .unwrap_or(".");
    let session = v["session_id"].as_str().unwrap_or(run);
    let mode = v["mode"].as_str().unwrap_or("observed");

    let bin = crate::observe::locate::claude_binary()
        .context("no claude binary found; set DEVPLANE_CLAUDE_BIN")?;

    // A background session belongs to Claude Code's own daemon and is attached
    // to; resuming it as a fresh conversation would fork the transcript.
    let args: Vec<String> = if mode == "background" {
        vec!["attach".into(), session.into()]
    } else {
        vec!["--resume".into(), session.into()]
    };

    println!(
        "{}",
        paint(
            DIM,
            &format!("{} {} (in {dir})", bin.display(), args.join(" "))
        )
    );

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Replace this process rather than holding a pipe to Claude Code.
        let err = std::process::Command::new(&bin)
            .args(&args)
            .current_dir(dir)
            .exec();
        Err(err).context("starting claude")
    }
    #[cfg(not(unix))]
    {
        let status = std::process::Command::new(&bin)
            .args(&args)
            .current_dir(dir)
            .status()
            .context("starting claude")?;
        Err(crate::cli::Exit(status.code().unwrap_or(1)).into())
    }
}

/// The board in a browser. If no host is running, this process becomes the
/// host in the foreground until ctrl-c.
pub async fn cmd_open() -> Result<()> {
    if let Ok(c) = client::Client::connect_running().await {
        return open_browser(
            &format!("{}/?token={}", c.base_url(), c.token()),
            c.base_url(),
        );
    }
    println!(
        "{}",
        paint(
            DIM,
            "nothing is running — hosting here until ctrl-c; the board opens when it is up"
        )
    );
    tokio::spawn(async {
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if let Ok(c) = client::Client::connect()
                && c.healthy().await
            {
                let _ = open_browser(
                    &format!("{}/?token={}", c.base_url(), c.token()),
                    c.base_url(),
                );
                return;
            }
        }
    });
    super::cmd_serve(crate::config::DEFAULT_PORT).await
}

fn open_browser(url: &str, base: &str) -> Result<()> {
    println!("{}", paint(DIM, base));
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let opener = "";

    if !opener.is_empty() {
        std::process::Command::new(opener)
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("opening a browser")?;
    }
    Ok(())
}

/// `devplane watch [run]` — every event, or one driven run's conversation.
pub async fn cmd_watch(run: Option<&str>, thinking: bool, history: i64) -> Result<()> {
    if let Some(run) = run {
        return follow_run(run, thinking, history).await;
    }
    let c = client::Client::connect_running().await?;
    println!("{}", paint(DIM, "watching — ctrl-c to stop"));
    let res = reqwest::Client::new()
        .get(format!("{}/api/stream", c.base_url()))
        .bearer_auth(c.token())
        .send()
        .await?;
    let mut stream = res.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        for line in String::from_utf8_lossy(&chunk).lines() {
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            // Two kinds of frame, told apart on the wire rather than guessed at.
            let (at, what) = match v["frame"].as_str() {
                Some("message") => (
                    v["at"].as_str().unwrap_or(""),
                    format!(
                        "{:<8} {}",
                        v["role"].as_str().unwrap_or(""),
                        clip(v["text"].as_str().unwrap_or("").trim(), 60)
                    ),
                ),
                _ => (
                    v["at"].as_str().unwrap_or(""),
                    v["event"]["type"].as_str().unwrap_or("?").to_string(),
                ),
            };
            println!(
                "{} {} {}",
                paint(DIM, at.get(11..19).unwrap_or("")),
                paint(
                    render::MAGENTA,
                    &clip(v["run_id"].as_str().unwrap_or(""), 8)
                ),
                what
            );
        }
    }
    Ok(())
}

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
            "Not signed in to GitHub — run `devplane login github{flag}`, or sign in from Setup."
        )),
        "pending" => Some(format!(
            "Signing in to GitHub — enter {} at {}.",
            github["user_code"].as_str().unwrap_or("the code"),
            github["verification_uri"]
                .as_str()
                .unwrap_or("GitHub's device page")
        )),
        "expired" => Some(format!(
            "GitHub sign-in expired: GitHub no longer accepts the token — run `devplane login github{flag}`."
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

pub async fn cmd_forge_issues(json: bool) -> Result<()> {
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

pub async fn cmd_forge_prs(json: bool) -> Result<()> {
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

/// The sentence naming the vendors this machine cannot watch, composed from
/// the table the board reads so `devplane ls` and the interface agree.
fn unwatched_note() -> String {
    let w = crate::core::vendors::watching();
    if w.driven_only.is_empty() {
        return String::new();
    }
    format!(
        "{} {} only when Devplane starts them.\nA session you opened yourself in one of those is not listed here.\n`devplane doctor` says what is watched, per channel.",
        w.driven_only.join(", "),
        if w.driven_only.len() == 1 {
            "appears"
        } else {
            "appear"
        }
    )
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

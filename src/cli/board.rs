//! Seeing what is running: the board, a run, its transcript, and reaching it.

use super::{fetch_run, ids, urlencode};
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

/// What `status` and `run list` narrow the board to. Never remembered.
#[derive(Debug, Default)]
pub struct Filter {
    pub all: bool,
    pub project: Option<String>,
    pub needs_you: bool,
}

/// `devplane status` (and bare `devplane`): the machine's summary, then the
/// sessions in play.
pub async fn cmd_status(f: Filter, json: bool) -> Result<()> {
    list(f, json, true).await
}

/// `devplane run list`: the sessions alone, grouped by project.
pub async fn cmd_run_list(f: Filter, json: bool) -> Result<()> {
    list(f, json, false).await
}

async fn list(f: Filter, json: bool, summary: bool) -> Result<()> {
    let Filter {
        all,
        project,
        needs_you,
    } = f;
    let project = project.as_deref();
    let c = crate::local::Reader::open().await?;
    // A filter may match dormant sessions, so it widens the fetch and narrows
    // here.
    let all = all || project.is_some() || needs_you;
    let mut board = c.board(all).await?;

    if let Some(needle) = project {
        let needle = needle.to_lowercase();
        board.runs.retain(|r| {
            r.project_name
                .as_deref()
                .is_some_and(|p| p.to_lowercase().contains(&needle))
        });
        if board.runs.is_empty() && !json {
            println!(
                "No session matches {}.\n\n{}",
                paint(BOLD, needle.as_str()),
                paint(DIM, "devplane status --all lists every project.")
            );
            return Ok(());
        }
    }
    if needs_you {
        board
            .runs
            .retain(|r| r.state == "waiting" || r.state == "failed");
        if board.runs.is_empty() && !json {
            println!("{}", paint(DIM, "Nothing is waiting on you."));
            return Ok(());
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&board)?);
        return Ok(());
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
            paint(DIM, "devplane status --all shows them.")
        );
        return Ok(());
    }

    if board.runs.is_empty() {
        // Not Claude-only: any connected agent's session would be listed.
        println!(
            "No agent sessions are running.\n\n\
             Start one — in Claude Code, Codex or Copilot after `devplane agent connect <agent>`, \n\
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
    if !summary {
        print_board(&board);
        return Ok(());
    }
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
                    "{} quiet (nothing heard for hours) — devplane status --all",
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
                "cost and context are blank — devplane agent connect claude adds them",
            )
        );
    }
    println!();

    print_board(&board);

    Ok(())
}

pub async fn cmd_show(run: &str, json: bool) -> Result<()> {
    let c = crate::local::Reader::open().await?;
    let run = &ids::run(&c, run).await?;
    let detail = fetch_run(&c, run).await?;
    if json {
        // Through a `Value`, whose keys are sorted, as the host's own were.
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::to_value(&detail)?)?
        );
        return Ok(());
    }
    let v = &detail.run;

    let or_dash = |s: Option<&str>| s.unwrap_or("-").to_string();
    println!(
        "{}  {}",
        paint(BOLD, &or_dash(v.name.as_deref())),
        paint(DIM, v.id.as_str())
    );
    // A waiting run's state is an object on the wire, printed as `?`.
    let state = match &v.state {
        crate::core::RunState::Waiting(_) => "?",
        s => s.as_str(),
    };
    println!("  state      {state}");
    println!("  agent      {} ({})", v.agent, v.mode.as_str());
    // From the run's record, never parsed; a watched session says it carries
    // none rather than guessing from the diff.
    println!("  tasks      {}", detail.sent_says);
    for t in v.sent.iter().flatten() {
        println!("             {}", paint(DIM, &t.text));
    }
    println!("  where      {}", v.cwd.display());
    if let Some(w) = &v.worktree {
        println!("  worktree   {}", w.display());
    }
    println!("  model      {}", or_dash(v.model.as_deref()));
    println!("  surface    {}", or_dash(v.entrypoint.as_deref()));
    if let Some(pid) = v.pid {
        println!("  pid        {pid}");
    }

    let t = &v.totals;
    println!(
        "  cost       ${:.4} over {} requests",
        t.cost_usd, t.api_requests
    );
    println!("  tools      {} calls, {} errors", t.tool_calls, t.errors);
    // What the session changed, as the status line reports it (or nothing).
    let (added, removed) = (t.lines_added, t.lines_removed);
    if added > 0 || removed > 0 {
        println!("  changed    +{added} −{removed} lines");
    }
    // A rate-limit percentage with no reset time is a number with no verb.
    if let Some(pct) = t.rate_limit_percent {
        let window = match t.rate_limit_window.as_deref() {
            Some("five_hour") => "5-hour",
            Some("seven_day") => "7-day",
            Some("spend_limit") => "spend limit",
            other => other.unwrap_or("window"),
        };
        let when = match t.rate_limit_resets_at {
            Some(at) => match jiff::Timestamp::from_second(at) {
                Ok(ts) => format!(", resets {}", render::until(ts)),
                Err(_) => String::new(),
            },
            None => String::new(),
        };
        println!("  limits     {window} {pct:.0}% used{when}");
    }
    if let Some(v) = &v.claude_version {
        println!("  harness    Claude Code {v}");
    }

    if let Some(b) = &v.blocked_on {
        println!("\n{}", paint(render::YELLOW, "blocked on"));
        if let Some(m) = &b.message {
            println!("  {m}");
        }
        // Each option is an object on the wire, printed as an empty label.
        for n in 0..b.options.len() {
            println!("  {}. ", n + 1);
        }
    }

    // What the agent says it is going to do, where it says anything.
    if !v.plan.is_empty() {
        println!("\n{}", paint(BOLD, "plan"));
        for step in &v.plan {
            println!(
                "  {} {}",
                match step.status.as_str() {
                    "completed" => paint(render::GREEN, "✓"),
                    "in_progress" => paint(render::BLUE, "▸"),
                    _ => paint(DIM, "·"),
                },
                clip(&step.content, 74)
            );
        }
    }

    if !v.recent_tools.is_empty() {
        println!("\n{}", paint(BOLD, "recent tools"));
        for t in v.recent_tools.iter().rev().take(10) {
            println!(
                "  {:<14} {}",
                t.tool,
                match t.ok {
                    Some(true) => paint(render::GREEN, "ok"),
                    Some(false) => paint(render::RED, "failed"),
                    None => paint(DIM, "running"),
                }
            );
        }
    }

    // The tail of the conversation; `devplane run watch <run>` is the live version.
    if v.mode == crate::core::RunMode::Driven {
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
            println!("  {}", paint(DIM, &format!("devplane run watch {run}")));
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
    let run = &ids::run(&crate::local::Reader::Host(c.clone()), run).await?;

    // Only a driven run has a conversation to follow; say so otherwise.
    let detail = fetch_run(&crate::local::Reader::Host(c.clone()), run).await?;
    if detail.run.mode != crate::core::RunMode::Driven {
        anyhow::bail!(
            "that is a session Devplane watches, not one it drives, and the documented \
             channels carry no transcript: hooks report lifecycle and tool inputs, and \
             telemetry redacts prompts and responses.\n\n\
             Its own window already has the conversation; resume it in a terminal:\n  devplane run attach {run}"
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
            let Ok(f) = serde_json::from_str::<StreamFrame>(data) else {
                continue;
            };
            match f.frame.as_deref() {
                Some("message") => print_message(
                    f.role.as_deref().unwrap_or("agent"),
                    f.text.as_deref().unwrap_or(""),
                    thinking,
                ),
                // The turn ending marks the answer complete, not paused.
                Some("event") if f.event_type() == Some("turn_ended") => {
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
    let run = &ids::run(&crate::local::Reader::Host(c.clone()), run).await?;
    let v = fetch_run(&crate::local::Reader::Host(c.clone()), run).await?;
    let v = &v.run;
    let dir = v.worktree.as_deref().unwrap_or(&v.cwd);
    let session = v.session_id.as_str();
    let mode = v.mode.as_str();

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
            &format!(
                "{} {} (in {})",
                bin.display(),
                args.join(" "),
                dir.display()
            )
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
pub async fn cmd_open(port: u16) -> Result<()> {
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
    super::cmd_serve(port).await
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

/// `devplane run watch [run]` — every event, or one driven run's conversation.
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
            let Ok(f) = serde_json::from_str::<StreamFrame>(data) else {
                continue;
            };
            // Two kinds of frame, told apart on the wire rather than guessed at.
            let at = f.at.as_deref().unwrap_or("");
            let what = match f.frame.as_deref() {
                Some("message") => format!(
                    "{:<8} {}",
                    f.role.as_deref().unwrap_or(""),
                    clip(f.text.as_deref().unwrap_or("").trim(), 60)
                ),
                _ => f.event_type().unwrap_or("?").to_string(),
            };
            println!(
                "{} {} {}",
                paint(DIM, at.get(11..19).unwrap_or("")),
                paint(render::MAGENTA, &clip(f.run_id.as_deref().unwrap_or(""), 8)),
                what
            );
        }
    }
    Ok(())
}

/// One `data:` line of `/api/stream`: a [`crate::core::Frame`], read only as
/// far as a terminal prints it. Every field is optional, so a line that is not
/// a frame (a `resync`) still reads.
#[derive(serde::Deserialize)]
struct StreamFrame {
    /// `message` or `event`.
    #[serde(default)]
    frame: Option<String>,
    #[serde(default)]
    at: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    event: Option<EventTag>,
}

/// An event, by its `type` alone.
#[derive(serde::Deserialize)]
struct EventTag {
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

impl StreamFrame {
    fn event_type(&self) -> Option<&str> {
        self.event.as_ref()?.kind.as_deref()
    }
}

/// The sentence naming the vendors this machine cannot watch, composed from
/// the table the board reads so `devplane status` and the interface agree.
fn unwatched_note() -> String {
    let w = crate::core::vendors::watching();
    if w.driven_only.is_empty() {
        return String::new();
    }
    let (verb, them) = match w.driven_only.len() {
        1 => ("appears", "it"),
        _ => ("appear", "them"),
    };
    format!(
        "{} {verb} only when Devplane starts {them}.\nA session you opened yourself in one of \
         those is not listed here.\n`devplane doctor` says what is watched, per channel.",
        w.driven_only.join(", "),
    )
}

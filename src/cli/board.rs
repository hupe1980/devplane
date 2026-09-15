//! Seeing what is running: the board, a run, its transcript, and reaching it.

use super::{fetch_run, raw, urlencode};
use crate::focus;
use crate::render::{BOLD, DIM, ago, clip, paint, state_marker, surface};
use crate::{client, render};
use anyhow::{Context, Result};

fn print_board(board: &render::BoardResponse) {
    // Grouped by project, because that is the unit the person cares about. A
    // machine with nine sessions open on one repository otherwise prints nine
    // near-identical rows that differ only in a hash, and reading them means
    // matching prefixes by eye to work out that it is all one project.
    //
    // The board is already sorted by recency, so first appearance decides a
    // project's place and the rows keep their order inside it.
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
        println!(
            "{}{}",
            paint(BOLD, project),
            if rows.len() > 1 {
                paint(DIM, &format!("  ·  {} sessions", rows.len()))
            } else {
                String::new()
            }
        );

        for r in rows {
            let ctx = r
                .context_percent
                .map(|p| format!("{p:3.0}%"))
                .unwrap_or_else(|| "   -".into());
            let cost = if r.cost_usd > 0.0 {
                format!("${:.2}", r.cost_usd)
            } else {
                "-".into()
            };
            // The project is on the header line, so the row carries only what
            // tells one of its sessions from another.
            let label = session_label(r, project);
            // Trimmed: a working run with nothing to say would otherwise leave
            // two spaces at the end of every line, which shows up the moment
            // anyone pipes the board into a file.
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
        }
    }
}

/// What distinguishes one session of a project from another.
///
/// Session names are `<project>-<hash>`, and the project is already the header
/// above the row, so repeating it costs ten columns to say nothing.
fn session_label(run: &render::RunView, project: &str) -> String {
    let name = run.name.as_deref().unwrap_or("");
    let short = name
        .strip_prefix(project)
        .and_then(|rest| rest.strip_prefix('-'))
        .unwrap_or(name);
    if short.is_empty() {
        // Nothing to strip and nothing to show: fall back to the run's own id,
        // which is what `vibeplane show` and `focus` take anyway.
        return run.id.chars().take(8).collect();
    }
    clip(short, 10)
}

pub async fn cmd_ls(all: bool, project: Option<&str>, needs_you: bool, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    // A filter asks about sessions that may well be dormant — `--project x`
    // meaning "and nothing in x" would be a lie — so either one widens the
    // fetch and then narrows it here.
    let all = all || project.is_some() || needs_you;
    let path = if all {
        "/api/board?all=true"
    } else {
        "/api/board"
    };
    let mut board: render::BoardResponse = c.get(path).await?;

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
                paint(DIM, "vibeplane ls --all lists every project.")
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
                    "{} session(s) exist but have never reported — editor tabs left open.",
                    board.summary.dormant
                )
            ),
            paint(DIM, "vibeplane ls --all shows them.")
        );
        return Ok(());
    }

    if board.runs.is_empty() {
        // Two very different situations look identical here, and telling the
        // user the wrong one wastes their afternoon.
        match crate::observe::locate::claude_binary() {
            Some(bin) => println!(
                "No Claude Code sessions are running.\n\n\
                 {}\n\n\
                 Start one, and it appears here. For live state — what each session is \n\
                 doing, what it costs, what it is blocked on — run {}.",
                paint(DIM, &format!("using {}", bin.display())),
                paint(BOLD, "vibeplane connect claude")
            ),
            None => println!(
                "{}\n\n\
                 Sessions are discovered by running `claude agents --json`, and no \n\
                 `claude` binary was found on PATH, in ~/.claude/local, or in a VS Code\n\
                 extension.\n\n\
                 Point at it with {} if it lives somewhere else.",
                paint(render::YELLOW, "Claude Code was not found on this machine."),
                paint(BOLD, "VIBEPLANE_CLAUDE_BIN=/path/to/claude")
            ),
        }
        return Ok(());
    }

    // A filtered view must not keep the whole machine's header: "7 projects ·
    // 22 sessions" above two rows of one project is a summary of something
    // else.
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
    println!(
        "{} · {} · {} working · {} need you · {} idle{}",
        paint(BOLD, &format!("{} projects", s.projects)),
        format_args!("{} sessions", s.runs),
        s.working,
        if s.needs_you > 0 {
            paint(render::YELLOW, &s.needs_you.to_string())
        } else {
            "0".into()
        },
        s.idle,
        if s.cost_usd > 0.0 {
            paint(DIM, &format!(" · ${:.2}", s.cost_usd))
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
                    "{} dormant (never reported) — vibeplane ls --all",
                    s.dormant
                )
            )
        );
    }
    // Sessions are discovered from Claude Code's own roster, which carries no
    // cost and no context figure — so on a machine that has never run
    // `connect`, two columns are `-` on every row and nothing says why. The
    // empty-board path already explains this; the populated one did not, which
    // is the path somebody with twenty sessions open is actually on.
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
                "cost and context are blank — vibeplane connect claude adds them",
            )
        );
    }
    println!();

    print_board(&board);

    Ok(())
}

pub async fn cmd_show(run: &str, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
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

    // The last of the conversation, for a run that has one. `vibeplane tail`
    // is the live version; this is the glance.
    if v["mode"] == "driven" {
        let said: Vec<render::Message> = c
            .get(&format!("/api/runs/{run}/messages?limit=6"))
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
            println!("  {}", paint(DIM, &format!("vibeplane tail {run}")));
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
    // Indented under its speaker so a wrapped paragraph still reads as one
    // person talking.
    let body = text
        .trim_end()
        .lines()
        .collect::<Vec<_>>()
        .join("\n       ");
    println!("{} {}", paint(colour, &format!("{label:>5}")), body);
}

/// Follows a driven run's conversation.
pub async fn cmd_tail(run: &str, thinking: bool, history: i64) -> Result<()> {
    let c = client::Client::connect_or_start().await?;

    // What kind of run this is decides whether there is anything to follow, and
    // saying so beats printing nothing for ever.
    let detail = fetch_run(&c, run).await?;
    let mode = detail["mode"].as_str().unwrap_or("observed");
    if mode != "driven" {
        anyhow::bail!(
            "that is a session Vibeplane watches, not one it drives, and the documented \n             channels carry no transcript: hooks report lifecycle and tool inputs, and \n             telemetry redacts prompts and responses.\n\n             Its own window already has the conversation:\n  vibeplane focus {run}"
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
        // Server-sent events are newline-framed and a chunk can split one, so
        // the last partial line is kept rather than parsed and dropped.
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
                // The turn ending is worth a line: it is the moment the answer
                // is complete rather than merely paused.
                Some("event") if v["event"]["type"] == "turn_ended" => {
                    println!("{}", paint(DIM, "— turn ended —"));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub async fn cmd_focus(run: &str) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = fetch_run(&c, run).await?;
    let dir = v["worktree"]
        .as_str()
        .or_else(|| v["cwd"].as_str())
        .context("that run has no working directory")?;

    match focus::focus_path(std::path::Path::new(dir))? {
        focus::Focused::Editor { app, pid } => {
            println!("Raised {app} (pid {pid}) for {dir}");
        }
        focus::Focused::Nothing => {
            // Saying "focused" when nothing happened would be worse than
            // admitting it, so the fallback is what always works: the command,
            // and — on the surfaces that make a URL clickable — the handler
            // Claude Code registers with the operating system.
            println!(
                "No editor window has {dir} open.\n\nResume it in a terminal:\n  claude --resume {run}"
            );
            println!(
                "\nOr open it as a VS Code tab:\n  {}",
                crate::core::deeplink::vscode_session(run)
            );
        }
    }
    Ok(())
}

/// Hands the terminal to Claude Code, resuming the session in its own
/// directory.
///
/// This is the escape hatch the whole product is built around: Vibeplane is
/// not a terminal, and when a session needs more than a decision the right
/// answer is the real thing, in the right place, with one keystroke.
pub async fn cmd_attach(run: &str) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = fetch_run(&c, run).await?;
    let dir = v["worktree"]
        .as_str()
        .or_else(|| v["cwd"].as_str())
        .unwrap_or(".");
    let session = v["session_id"].as_str().unwrap_or(run);
    let mode = v["mode"].as_str().unwrap_or("observed");

    let bin = crate::observe::locate::claude_binary()
        .context("no claude binary found; set VIBEPLANE_CLAUDE_BIN")?;

    // A background session is owned by Claude's daemon, which has its own way
    // in; resuming it as a fresh conversation would fork the transcript.
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
        // Replace this process rather than nesting one inside it: the user
        // wanted Claude Code, not Vibeplane holding a pipe to it.
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
        std::process::exit(status.code().unwrap_or(1));
    }
}

pub async fn cmd_open() -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let url = format!("{}/?token={}", c.base_url(), c.token());
    println!("{}", paint(DIM, c.base_url()));
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
            .arg(&url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("opening a browser")?;
    }
    Ok(())
}

pub async fn cmd_watch() -> Result<()> {
    let c = client::Client::connect_or_start().await?;
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

//! Vibeplane — the local-first control plane for AI coding agents.
//!
//! One binary. `vibeplane serve` is the daemon that receives hooks and
//! telemetry; every other subcommand is a client of it and starts it if it is
//! not already running, so the observer is never something the user has to
//! remember.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use vibeplane::render::{BOLD, DIM, ago, clip, level_marker, paint, state_marker, surface};
use vibeplane::{client, config, daemon, focus, poller, render};

#[derive(Parser)]
#[command(
    name = "vibeplane",
    version,
    about = "The control plane for AI coding agents",
    long_about = "Vibeplane watches every Claude Code session on this machine — in a terminal, \
                  in VS Code, in the desktop app — and tells you which ones need you.\n\n\
                  Start with `vibeplane connect claude`, then `vibeplane ls`."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Print machine-readable JSON instead of a table.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon in the foreground.
    Serve {
        #[arg(long, env = "VIBEPLANE_PORT", default_value_t = config::DEFAULT_PORT)]
        port: u16,
    },
    /// Show every session on this machine.
    #[command(visible_alias = "ps")]
    Ls,
    /// Show what needs a human, most urgent first.
    Inbox,
    /// Show one run in detail.
    Show { run: String },
    /// Search tool calls, questions and errors across every session.
    Search { query: String },
    /// Raise the editor window that owns a run.
    Focus { run: String },
    /// Attach a terminal to a run, resuming its session.
    Attach { run: String },
    /// Hide a run's inbox items for a while.
    Snooze {
        run: String,
        /// Minutes to stay quiet. `0` un-snoozes.
        #[arg(long, default_value_t = 60)]
        minutes: i64,
    },
    /// Open the board in a browser.
    Open,
    /// Follow events as they arrive.
    Watch,
    /// Channel health, latency and daemon status.
    #[command(visible_alias = "doctor")]
    Diagnostics,
    /// Install Vibeplane's hooks and telemetry into Claude Code.
    Connect {
        #[command(subcommand)]
        what: ConnectTarget,
        /// Also wrap the status line, which is the only source of subscription
        /// rate limits. Off by default because it touches a command you
        /// configured yourself.
        #[arg(long)]
        statusline: bool,
    },
    /// Remove everything `connect` installed.
    Disconnect {
        #[command(subcommand)]
        what: ConnectTarget,
    },
    /// Stop the running daemon.
    Stop,
    /// Read a hook payload on stdin and forward it to the daemon.
    ///
    /// Used for `SessionStart`, the one hook event that does not accept HTTP
    /// hooks. Always exits 0: a hook that fails is a hook that interrupts the
    /// user's session, and an observer has no business doing that.
    #[command(hide = true)]
    Hook,
    /// Read a status-line payload on stdin and forward it, then run the
    /// command that was there before. Used by the optional status-line shim.
    Statusline {
        /// The user's original status-line command, run after forwarding.
        #[arg(long)]
        then: Option<String>,
    },
}

#[derive(Subcommand, Clone, Copy)]
enum ConnectTarget {
    /// Claude Code, through its user-scope settings.
    Claude,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Serve { port }) => cmd_serve(port).await,
        Some(Command::Ls) | None => cmd_ls(cli.json).await,
        Some(Command::Inbox) => cmd_inbox(cli.json).await,
        Some(Command::Show { run }) => cmd_show(&run, cli.json).await,
        Some(Command::Search { query }) => cmd_search(&query, cli.json).await,
        Some(Command::Focus { run }) => cmd_focus(&run).await,
        Some(Command::Attach { run }) => cmd_attach(&run).await,
        Some(Command::Snooze { run, minutes }) => cmd_snooze(&run, minutes, cli.json).await,
        Some(Command::Open) => cmd_open().await,
        Some(Command::Watch) => cmd_watch().await,
        Some(Command::Diagnostics) => cmd_diagnostics(cli.json).await,
        Some(Command::Connect { what, statusline }) => {
            cmd_connect(what, statusline, cli.json).await
        }
        Some(Command::Disconnect { what }) => cmd_disconnect(what, cli.json).await,
        Some(Command::Stop) => cmd_stop().await,
        Some(Command::Hook) => cmd_hook().await,
        Some(Command::Statusline { then }) => cmd_statusline(then).await,
    }
}

// ---------------------------------------------------------------------------

async fn cmd_serve(port: u16) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("VIBEPLANE_LOG")
                .unwrap_or_else(|_| "vibeplane=info,warn".into()),
        )
        .init();

    if let Some(info) = config::read_daemon_info()?
        && poller::process_alive(info.pid)
        && info.pid != std::process::id()
    {
        anyhow::bail!(
            "a daemon is already running (pid {}, port {}). Stop it with `vibeplane stop`.",
            info.pid,
            info.port
        );
    }

    let token = config::load_or_create_token()?;
    let policy = load_policy();
    let state = daemon::AppState::new(config::db_path()?, token, policy).await?;
    poller::reconcile_at_startup(&state).await;
    // Fill the board before anyone can ask for it.
    poller::initial_poll(&state).await;
    daemon::serve(state, port).await
}

/// The policy, from `~/.vibeplane/policy.toml` if it exists.
///
/// Until project configuration lands, the default is empty: no rule matches, so
/// every permission prompt reaches the human exactly as it does today. A policy
/// that guessed on the user's behalf would be a policy that approved something
/// nobody chose.
fn load_policy() -> vibeplane_core::Policy {
    let Ok(home) = config::home() else {
        return vibeplane_core::Policy::default();
    };
    let path = home.join("policy.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return vibeplane_core::Policy::default();
    };
    let mut allow = Vec::new();
    let mut deny = Vec::new();
    let mut section = "";
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if l.starts_with('[') {
            section = "";
            continue;
        }
        if let Some(rest) = l.strip_prefix("auto_allow") {
            section = "allow";
            collect_rules(rest, &mut allow);
        } else if let Some(rest) = l.strip_prefix("never_auto") {
            section = "deny";
            collect_rules(rest, &mut deny);
        } else if section == "allow" {
            collect_rules(l, &mut allow);
        } else if section == "deny" {
            collect_rules(l, &mut deny);
        }
    }
    tracing::info!(
        allow = allow.len(),
        deny = deny.len(),
        path = %path.display(),
        "policy loaded"
    );
    vibeplane_core::Policy::new(&allow, &deny)
}

fn collect_rules(s: &str, out: &mut Vec<String>) {
    for part in s.split(['[', ']', ',']) {
        let t = part.trim().trim_matches('"').trim();
        if !t.is_empty() && t != "=" && !t.starts_with('=') {
            out.push(t.to_string());
        }
    }
}

async fn cmd_ls(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let board: render::BoardResponse = c.get("/api/board").await?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&raw(&c, "/api/board").await?)?
        );
        return Ok(());
    }

    if board.runs.is_empty() {
        // Two very different situations look identical here, and telling the
        // user the wrong one wastes their afternoon.
        match vibeplane_observe::locate::claude_binary() {
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
    println!();

    for r in &board.runs {
        let ctx = r
            .context_percent
            .map(|p| format!("{p:3.0}%"))
            .unwrap_or_else(|| "   -".into());
        let cost = if r.cost_usd > 0.0 {
            format!("${:.2}", r.cost_usd)
        } else {
            "-".into()
        };
        let label = r
            .name
            .clone()
            .or_else(|| r.project_name.clone())
            .unwrap_or_else(|| "?".into());
        println!(
            "{} {:<18} {:<8} {:>5} {:>7} {:>5}  {}",
            state_marker(&r.state),
            paint(BOLD, &clip(&label, 18)),
            paint(DIM, surface(r)),
            ctx,
            cost,
            paint(DIM, &ago(r.idle_seconds)),
            clip(
                r.summary.as_deref().unwrap_or(match r.state.as_str() {
                    "waiting" => "needs you",
                    "idle" => "waiting for a prompt",
                    _ => "",
                }),
                60
            )
        );
        if let Some(w) = &r.worktree {
            println!("  {}", paint(DIM, &format!("worktree {}", clip(w, 70))));
        }
    }
    Ok(())
}

async fn cmd_inbox(json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let items: Vec<render::InboxItem> = c.get("/api/inbox").await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&raw(&c, "/api/inbox").await?)?
        );
        return Ok(());
    }
    if items.is_empty() {
        println!("{}", paint(render::GREEN, "Nothing needs you."));
        return Ok(());
    }
    for i in &items {
        println!(
            "{} {} {}",
            level_marker(&i.level),
            paint(BOLD, &i.title),
            paint(DIM, &format!("[{}]", i.kind))
        );
        if let Some(d) = &i.detail {
            println!("     {}", clip(d, 100));
        }
        for (n, o) in i.options.iter().enumerate() {
            println!("     {}. {}", n + 1, o);
        }
        println!(
            "     {}",
            paint(
                DIM,
                &format!("run {} · {}", clip(&i.run_id, 12), i.actions.join(", "))
            )
        );
    }
    Ok(())
}

async fn cmd_show(run: &str, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, &format!("/api/runs/{run}")).await?;
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
    Ok(())
}

async fn cmd_search(query: &str, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, &format!("/api/search?q={}", urlencode(query))).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    let empty = vec![];
    for hit in v.as_array().unwrap_or(&empty) {
        println!(
            "{}  {}",
            paint(DIM, &clip(hit["run_id"].as_str().unwrap_or(""), 12)),
            hit["text"].as_str().unwrap_or("")
        );
    }
    Ok(())
}

async fn cmd_focus(run: &str) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, &format!("/api/runs/{run}")).await?;
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
            // admitting it, so the fallback is the command that always works.
            println!(
                "No editor window has {dir} open.\n\nResume it in a terminal:\n  claude --resume {run}"
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
async fn cmd_attach(run: &str) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v = raw(&c, &format!("/api/runs/{run}")).await?;
    let dir = v["worktree"]
        .as_str()
        .or_else(|| v["cwd"].as_str())
        .unwrap_or(".");
    let session = v["session_id"].as_str().unwrap_or(run);
    let mode = v["mode"].as_str().unwrap_or("observed");

    let bin = vibeplane_observe::locate::claude_binary()
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

async fn cmd_snooze(run: &str, minutes: i64, json: bool) -> Result<()> {
    let c = client::Client::connect_or_start().await?;
    let v: serde_json::Value = c
        .post(&format!("/api/runs/{run}/snooze?minutes={minutes}"))
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else if minutes == 0 {
        println!(
            "{} — its items are back in the inbox.",
            paint(BOLD, "Un-snoozed")
        );
    } else {
        println!("Quiet for {minutes} minutes.");
    }
    Ok(())
}

async fn cmd_open() -> Result<()> {
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

async fn cmd_watch() -> Result<()> {
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
            println!(
                "{} {} {}",
                paint(
                    DIM,
                    v["at"].as_str().unwrap_or("").get(11..19).unwrap_or("")
                ),
                paint(
                    render::MAGENTA,
                    &clip(v["run_id"].as_str().unwrap_or(""), 8)
                ),
                v["event"]["type"].as_str().unwrap_or("?")
            );
        }
    }
    Ok(())
}

async fn cmd_diagnostics(json: bool) -> Result<()> {
    let settings_path = vibeplane_observe::connect::settings_path()?;
    let settings = vibeplane_observe::connect::read_settings(&settings_path).unwrap_or_default();
    let state = vibeplane_observe::connect::inspect(&settings, &settings_path);

    let daemon = config::read_daemon_info()?;
    let diag = match client::Client::connect() {
        Ok(c) => raw(&c, "/api/diagnostics").await.ok(),
        Err(_) => None,
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "daemon": daemon,
                "connect": state,
                "diagnostics": diag,
            }))?
        );
        return Ok(());
    }

    println!("{}", paint(BOLD, "daemon"));
    match &daemon {
        Some(d) if poller::process_alive(d.pid) => println!(
            "  running   pid {} · port {} · v{}",
            d.pid, d.port, d.version
        ),
        Some(d) => println!(
            "  {} (stale record, pid {} is gone)",
            paint(render::RED, "not running"),
            d.pid
        ),
        None => println!("  {}", paint(render::RED, "not running")),
    }

    println!("\n{}", paint(BOLD, "claude code"));
    println!("  settings  {}", state.settings_path.display());
    if state.hooks_installed.is_empty() {
        println!(
            "  hooks     {} — run `vibeplane connect claude`",
            paint(render::RED, "not installed")
        );
    } else {
        println!(
            "  hooks     {} ({} events)",
            paint(render::GREEN, "installed"),
            state.hooks_installed.len()
        );
    }
    if state.allowlist_blocks_us {
        println!(
            "  {}",
            paint(
                render::RED,
                "allowedHttpHookUrls does not permit loopback — hooks will not run"
            )
        );
    }
    match (&state.telemetry_endpoint, state.telemetry_is_ours) {
        (Some(e), true) => println!("  telemetry {} → {}", paint(render::GREEN, "configured"), e),
        (Some(e), false) => println!(
            "  telemetry {} → {} (cost and context unavailable)",
            paint(render::YELLOW, "external"),
            e
        ),
        (None, _) => println!("  telemetry {}", paint(render::RED, "not configured")),
    }

    if let Some(d) = diag {
        println!("\n{}", paint(BOLD, "channels"));
        let empty = vec![];
        let channels = d["channels"].as_array().unwrap_or(&empty);
        if channels.is_empty() {
            println!("  {}", paint(DIM, "nothing received yet"));
        }
        for ch in channels {
            println!(
                "  {:<14} {:>6} events · worst {:>6} µs · last {}",
                ch["channel"].as_str().unwrap_or("?"),
                ch["count"].as_u64().unwrap_or(0),
                ch["worst_micros"].as_u64().unwrap_or(0),
                ch["last_seen_at"]
                    .as_str()
                    .unwrap_or("")
                    .get(11..19)
                    .unwrap_or("")
            );
        }
    }
    Ok(())
}

async fn cmd_connect(_what: ConnectTarget, statusline: bool, json: bool) -> Result<()> {
    // Start the daemon first: the hooks we are about to install point at it,
    // and a port in the settings file that nothing answers is worse than no
    // hooks at all.
    let c = client::Client::connect_or_start().await?;
    let token = config::load_or_create_token()?;
    let path = vibeplane_observe::connect::settings_path()?;
    let mut settings = vibeplane_observe::connect::read_settings(&path)?;
    let exe = std::env::current_exe().context("finding the vibeplane binary")?;
    let mut report = vibeplane_observe::connect::connect(&mut settings, c.base_url(), &token, &exe);
    if statusline {
        report
            .notes
            .push(vibeplane_observe::connect::wrap_status_line(
                &mut settings,
                &exe,
            ));
    }
    vibeplane_observe::connect::write_settings(&path, &settings)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!(
        "{} {} hook events into {}",
        paint(render::GREEN, "installed"),
        report.hooks_added,
        path.display()
    );
    match report.telemetry {
        vibeplane_observe::connect::TelemetryStatus::Configured => {
            println!(
                "{} telemetry → {}",
                paint(render::GREEN, "configured"),
                c.base_url()
            )
        }
        vibeplane_observe::connect::TelemetryStatus::External => println!(
            "{} telemetry already goes elsewhere; left alone",
            paint(render::YELLOW, "skipped")
        ),
        _ => {}
    }
    for n in &report.notes {
        println!("  {}", paint(DIM, n));
    }
    println!(
        "\n{}\n  Existing sessions pick the hooks up on their next turn.\n  New sessions are seen immediately. Try {}.",
        paint(BOLD, "Done."),
        paint(BOLD, "vibeplane ls")
    );
    Ok(())
}

async fn cmd_disconnect(_what: ConnectTarget, json: bool) -> Result<()> {
    let path = vibeplane_observe::connect::settings_path()?;
    let mut settings = vibeplane_observe::connect::read_settings(&path)?;
    let report = vibeplane_observe::connect::disconnect(&mut settings);
    vibeplane_observe::connect::write_settings(&path, &settings)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{} {} hook entries from {}",
            paint(render::GREEN, "removed"),
            report.hooks_removed,
            path.display()
        );
    }
    Ok(())
}

async fn cmd_stop() -> Result<()> {
    let Some(info) = config::read_daemon_info()? else {
        println!("No daemon is running.");
        return Ok(());
    };
    #[cfg(unix)]
    unsafe {
        libc_kill(info.pid as i32, 15);
    }
    config::clear_daemon_info().ok();
    println!("Stopped daemon pid {}.", info.pid);
    Ok(())
}

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

/// The hook shim.
///
/// Reads the payload on stdin and posts it to the daemon. Never fails: the exit
/// code of a hook is something the user's session reacts to, so a daemon that
/// is not running must cost them nothing at all.
async fn cmd_hook() -> Result<()> {
    use std::io::Read;
    let mut body = String::new();
    std::io::stdin().read_to_string(&mut body).ok();

    if let Ok(Some(info)) = config::read_daemon_info()
        && let Ok(token) = config::load_or_create_token()
    {
        let _ = reqwest::Client::new()
            .post(format!("{}/vibeplane/hook", info.base_url()))
            .bearer_auth(token)
            .header("content-type", "application/json")
            .body(body)
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await;
    }
    Ok(())
}

/// The status-line shim: forward the sample, then run whatever the user had
/// configured so their status line is unchanged.
async fn cmd_statusline(then: Option<String>) -> Result<()> {
    use std::io::Read;
    let mut body = String::new();
    std::io::stdin().read_to_string(&mut body).ok();

    if let (Ok(info), Ok(token)) = (config::read_daemon_info(), config::load_or_create_token())
        && let Some(info) = info
    {
        // Best effort and short: the status line runs on every update, and a
        // slow shim is a slow prompt.
        let _ = reqwest::Client::new()
            .post(format!("{}/vibeplane/statusline", info.base_url()))
            .bearer_auth(token)
            .header("content-type", "application/json")
            .body(body.clone())
            .timeout(std::time::Duration::from_millis(300))
            .send()
            .await;
    }

    if let Some(cmd) = then {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(body.as_bytes()).ok();
                }
                child.wait_with_output()
            });
        if let Ok(out) = out {
            print!("{}", String::from_utf8_lossy(&out.stdout));
        }
    }
    Ok(())
}

async fn raw(c: &client::Client, path: &str) -> Result<serde_json::Value> {
    c.get(path)
        .await
        .with_context(|| format!("fetching {path}"))
}

fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

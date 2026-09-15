//! The command line: one module per thing a person is trying to do.
//!
//! This lives in the library rather than beside `main` for the reason the crate
//! docs give: a seam that can only be exercised through a subprocess is a seam
//! nobody exercises. `main.rs` is the argument parser it claims to be — it
//! parses and calls [`run`].

use crate::render::{DIM, paint};
use crate::{client, config, daemon, poller};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod admin;
mod board;
mod inbox;
mod work;

use admin::{cmd_agents, cmd_audit, cmd_connect, cmd_diagnostics, cmd_disconnect, cmd_search};
use board::{cmd_attach, cmd_focus, cmd_ls, cmd_open, cmd_show, cmd_tail, cmd_watch};
use inbox::{cmd_attention, cmd_decide, cmd_inbox, cmd_say, cmd_snooze};
use work::{cmd_check, cmd_dispatch, cmd_trust, cmd_work};

#[derive(Parser)]
#[command(
    name = "vibeplane",
    version,
    about = "The control plane for AI coding agents",
    long_about = "Vibeplane watches every Claude Code session on this machine — in a terminal, \
                  in VS Code, in the desktop app — and tells you which ones need you.\n\n\
                  Start with `vibeplane connect claude`, then `vibeplane ls`."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Print machine-readable JSON instead of a table.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the daemon in the foreground.
    Serve {
        #[arg(long, env = "VIBEPLANE_PORT", default_value_t = config::DEFAULT_PORT)]
        port: u16,
    },
    /// Show what is happening: sessions in play, and anything asking for you.
    #[command(visible_alias = "ps")]
    Ls {
        /// Include sessions that exist but have never reported anything —
        /// editor tabs left open, usually for days.
        #[arg(long, short)]
        all: bool,
        /// Only this project. Matches on any part of the name, so `mat` finds
        /// `matter-kit`.
        #[arg(long, short)]
        project: Option<String>,
        /// Only sessions that are waiting on a human.
        #[arg(long = "needs-you")]
        needs_you: bool,
    },
    /// Show what needs a human, most urgent first.
    Inbox,
    /// Show one run in detail.
    Show { run: String },
    /// Follow what a driven agent is saying, like `tail -f`.
    ///
    /// Only for runs Vibeplane drives: they have no window of their own, which
    /// is why this exists. A session you started in a terminal or an editor is
    /// already showing you its own transcript — use `vibeplane focus` to raise
    /// the window that has it.
    Tail {
        run: String,
        /// Include the agent's reasoning, where it streams any.
        #[arg(long)]
        thinking: bool,
        /// How much of the conversation so far to print first.
        #[arg(long, default_value_t = 40)]
        history: i64,
    },
    /// Search tool calls, questions and errors across every session.
    Search { query: String },
    /// Show what Vibeplane decided, and on whose authority.
    ///
    /// Answers the two questions the event log cannot: why a command ran
    /// without anybody being asked, and why there is a pull request on a branch.
    Audit {
        /// Narrow to one run or one piece of work.
        about: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// Show whether the inbox is worth reading, per kind.
    ///
    /// The product is a filter, and this is the only thing that measures it:
    /// how often each kind of item was acted on, dismissed, or resolved
    /// somewhere else. A kind that is mostly dismissed is costing you the
    /// credibility of every item beside it.
    Attention {
        /// How many days back to look.
        #[arg(long, default_value_t = 7)]
        days: i64,
    },
    /// Raise the editor window that owns a run.
    Focus { run: String },
    /// Attach a terminal to a run, resuming its session.
    Attach { run: String },
    /// Start an agent on a project and give it something to do.
    Dispatch {
        /// What to ask for.
        prompt: Vec<String>,
        /// Which agent: `claude`, `codex`, `opencode`, `gemini`, or a command.
        #[arg(long, default_value = "claude")]
        agent: String,
        /// Where it runs. Defaults to the current directory.
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    /// Send another prompt to a run Vibeplane drives.
    Say { run: String, prompt: Vec<String> },
    /// Answer a permission request from a driven run.
    Decide {
        run: String,
        /// `allow` or `deny`. Omit to refuse.
        #[arg(long, default_value = "deny")]
        decision: String,
        /// An exact option id from the inbox item, when the agent offers more
        /// than the usual two.
        #[arg(long)]
        option: Option<String>,
        #[arg(long)]
        request: String,
    },
    /// List the agents Vibeplane can drive.
    Agents,
    /// Read this repository's vibeplane.toml and say what it will do.
    ///
    /// Answers the three questions a committed config raises: does it parse,
    /// does everything it names exist, and is anything in it unsafe.
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Ask the gate what it would decide about one call, and why.
    ///
    /// Offline: it reads the rules a directory is governed by and answers
    /// without starting a daemon or an agent, so a rule can be tested before it
    /// is committed.
    ///
    ///   vibeplane explain 'pnpm test && rm -rf /'
    ///   vibeplane explain --tool Read .env
    ///   vibeplane explain --tool Agent --input '{"isolation":"worktree"}'
    ///   vibeplane explain --replay
    ///
    /// `--replay` asks the same question of every call already observed and
    /// names the rule that would stop the interruptions.
    Explain {
        /// The call, for a tool with a plain specifier: a command for `Bash`,
        /// a path for `Read` and `Edit`, a URL for `WebFetch`.
        #[arg(trailing_var_arg = true)]
        call: Vec<String>,
        #[arg(long, default_value = "Bash")]
        tool: String,
        /// The whole tool input as JSON, for a call a specifier cannot express.
        #[arg(long)]
        input: Option<String>,
        /// The directory the agent would be working in, which decides whose
        /// rules apply.
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Replay every tool call already observed against the current rules,
        /// and say which rule would answer the ones that reached you.
        #[arg(long, conflicts_with_all = ["call", "input"])]
        replay: bool,
        /// How many of the most recent calls to replay.
        #[arg(long, default_value_t = 5000)]
        limit: i64,
    },
    /// Allow Vibeplane to start agents in a repository.
    ///
    /// A headless agent runs that repository's own hooks and MCP servers
    /// without asking, so this is a deliberate act rather than a default.
    Trust {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Start work: an isolated checkout, an agent in it, and the project's
    /// gates when the agent says it is finished.
    Work {
        #[command(subcommand)]
        what: WorkCmd,
    },
    /// Hide a run's or a piece of work's inbox items for a while.
    Snooze {
        /// A run id, or a work id from `vibeplane inbox`.
        id: String,
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
    /// Install Vibeplane's hooks into a provider.
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
    Hook {
        /// Answer a provider's permission hook rather than only reporting it.
        ///
        /// Only GitHub Copilot needs this: its HTTP `preToolUse` hook falls
        /// through to the default permission flow on any error, so the one hook
        /// that carries a prohibition has to be a `command` hook, which fails
        /// closed. Claude Code answers over HTTP and never reaches here.
        #[arg(long, value_name = "PROVIDER")]
        gate: Option<String>,
    },
    /// Read a status-line payload on stdin and forward it, then run the
    /// command that was there before. Used by the optional status-line shim.
    Statusline {
        /// The user's original status-line command, run after forwarding.
        #[arg(long)]
        then: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum WorkCmd {
    /// Begin a new piece of work.
    Start {
        /// What to do. Becomes the branch name and the first prompt.
        title: Vec<String>,
        #[arg(long, default_value = "quick")]
        kind: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Work in the repository itself rather than an isolated checkout.
        #[arg(long)]
        no_worktree: bool,
        /// Start from a GitHub issue. Its title and body become the work.
        #[arg(long)]
        issue: Option<u64>,
    },
    /// Show every piece of work.
    #[command(visible_alias = "ls")]
    List,
    /// Show the issues this repository is offering as work.
    Issues {
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Only issues with this label. Defaults to `[github].ready_label`.
        #[arg(long)]
        label: Option<String>,
    },
    /// Run the project's gates now.
    Verify { work: String },
    /// Release a pipeline that is waiting at a declared human step.
    Approve { work: String },
    /// Hand the failures back to the agent once more, past the project's bound.
    ///
    /// The bound stops the machine arguing with a test suite for ever. It was
    /// never meant to stop you deciding that one more go is worth it.
    Retry { work: String },
    /// Pick work back up after a restart, against the same agent session.
    ///
    /// A daemon restart takes the agent processes with it; the branch, the
    /// worktree and the conversation the agent kept all survive. This
    /// reconnects to that conversation rather than starting a new one, so the
    /// work continues instead of being paid for twice.
    Resume { work: String },
    /// Show one piece of work: its phase, its runs, and what its checks said.
    Show { work: String },
    /// Mark work finished, optionally removing its checkout.
    Finish {
        work: String,
        #[arg(long)]
        remove_worktree: bool,
        /// Discard uncommitted changes in the checkout.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Clone, Copy, PartialEq, Eq)]
pub enum ConnectTarget {
    /// Claude Code, through its user-scope settings.
    Claude,
    /// GitHub Copilot, through one file in `~/.copilot/hooks/`.
    ///
    /// Its permission gate runs as a `command` hook rather than over HTTP,
    /// because an HTTP `preToolUse` hook there falls through to the default
    /// permission flow on any error — a prohibition that disappears under load
    /// is not one. Telemetry is not installed: Copilot reads it from the
    /// environment, which is the person's to set.
    Copilot,
}

/// Runs the parsed command.
pub async fn run(_cli: Cli) -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Serve { port }) => cmd_serve(port).await,
        Some(Command::Ls {
            all,
            project,
            needs_you,
        }) => cmd_ls(all, project.as_deref(), needs_you, cli.json).await,
        None => cmd_ls(false, None, false, cli.json).await,
        Some(Command::Inbox) => cmd_inbox(cli.json).await,
        Some(Command::Show { run }) => cmd_show(&run, cli.json).await,
        Some(Command::Tail {
            run,
            thinking,
            history,
        }) => cmd_tail(&run, thinking, history).await,
        Some(Command::Search { query }) => cmd_search(&query, cli.json).await,
        Some(Command::Audit { about, limit }) => cmd_audit(about.as_deref(), limit, cli.json).await,
        Some(Command::Attention { days }) => cmd_attention(days, cli.json).await,
        Some(Command::Focus { run }) => cmd_focus(&run).await,
        Some(Command::Attach { run }) => cmd_attach(&run).await,
        Some(Command::Dispatch { prompt, agent, cwd }) => {
            cmd_dispatch(&agent, cwd, prompt.join(" "), cli.json).await
        }
        Some(Command::Say { run, prompt }) => cmd_say(&run, prompt.join(" ")).await,
        Some(Command::Decide {
            run,
            decision,
            option,
            request,
        }) => cmd_decide(&run, &request, &decision, option).await,
        Some(Command::Agents) => cmd_agents(cli.json).await,
        Some(Command::Check { path }) => cmd_check(path, cli.json),
        Some(Command::Explain {
            call,
            tool,
            input,
            dir,
            replay,
            limit,
        }) => {
            if replay {
                crate::cli::work::cmd_replay(dir, limit, cli.json).await
            } else {
                crate::cli::work::cmd_explain(dir, tool, call, input, cli.json)
            }
        }
        Some(Command::Trust { path }) => cmd_trust(path, cli.json).await,
        Some(Command::Work { what }) => cmd_work(what, cli.json).await,
        Some(Command::Snooze { id, minutes }) => cmd_snooze(&id, minutes, cli.json).await,
        Some(Command::Open) => cmd_open().await,
        Some(Command::Watch) => cmd_watch().await,
        Some(Command::Diagnostics) => cmd_diagnostics(cli.json).await,
        Some(Command::Connect { what, statusline }) => {
            cmd_connect(what, statusline, cli.json).await
        }
        Some(Command::Disconnect { what }) => cmd_disconnect(what, cli.json).await,
        Some(Command::Stop) => cmd_stop().await,
        Some(Command::Hook { gate }) => cmd_hook(gate).await,
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
    let home = config::home()?;
    let state = daemon::AppState::new(config::db_path()?, token, policy, home).await?;
    poller::reconcile_at_startup(&state).await;
    // Fill the board before anyone can ask for it.
    poller::initial_poll(&state).await;
    daemon::serve(state, port).await
}

/// The machine-wide rules, from `~/.vibeplane/policy.toml`.
///
/// Empty by default: no rule matches, so every permission prompt reaches the
/// human exactly as it does today. A policy that guessed on the user's behalf
/// would be a policy that approved something nobody chose.
fn load_policy() -> crate::core::Policy {
    let Ok(home) = config::home() else {
        return crate::core::Policy::default();
    };
    match crate::core::GlobalConfig::load(&home) {
        Ok(g) => {
            let policy = g.policy();
            tracing::info!(
                allow = policy.allow_rules().len(),
                deny = policy.deny_rules().len(),
                "policy loaded"
            );
            policy
        }
        Err(e) => {
            // Refusing to start is worse than starting with no machine-wide
            // rules — the projects' own rules still apply, and the daemon is an
            // observer first. Saying so loudly is the obligation.
            tracing::error!(error = %e, "the machine-wide policy was not applied");
            crate::core::Policy::default()
        }
    }
}

/// Prints the runs grouped by project.
async fn raw(c: &client::Client, path: &str) -> Result<serde_json::Value> {
    c.get(path)
        .await
        .with_context(|| format!("fetching {path}"))
}

/// Fetches one run, or explains that there is no such run.
///
/// A mistyped id is the commonest thing that goes wrong with any command that
/// takes one, and "fetching /api/runs/xyz returned 404 Not Found" tells the
/// person nothing they can act on.
async fn fetch_run(c: &client::Client, run: &str) -> Result<serde_json::Value> {
    c.get(&format!("/api/runs/{run}")).await.map_err(|_| {
        anyhow::anyhow!(
            "no run `{run}` on the board.\n\n  {}",
            paint(DIM, "vibeplane ls --all lists every session, ids included.")
        )
    })
}

/// Percent-encodes a query value.
///
/// Over UTF-8 *bytes*, not chars: `format!("%{:02X}", c as u32)` on `é`
/// produced `%E9` (its codepoint, not its encoding) and on an emoji produced
/// `%1F680`, which is not percent-encoding at all. Searching for anything but
/// ASCII silently looked for something else.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
async fn cmd_hook(gate: Option<String>) -> Result<()> {
    use std::io::Read;
    let mut body = String::new();
    std::io::stdin().read_to_string(&mut body).ok();

    let Ok(Some(info)) = config::read_daemon_info() else {
        return Ok(());
    };
    let Ok(token) = config::load_or_create_token() else {
        return Ok(());
    };

    // The gate variant: post, and print whatever the daemon decided, because
    // the provider reads this process's stdout as the verdict.
    if gate.as_deref() == Some("copilot") {
        let reply = reqwest::Client::new()
            .post(format!("{}/vibeplane/copilot/gate", info.base_url()))
            .bearer_auth(token)
            .header("content-type", "application/json")
            .body(body)
            // Under Copilot's own timeout, which is 5 s as registered. A slow
            // answer there is fail-open anyway, so being slower than the
            // provider's patience buys nothing.
            .timeout(std::time::Duration::from_millis(3000))
            .send()
            .await;
        match reply {
            Ok(r) => println!("{}", r.text().await.unwrap_or_else(|_| "{}".into())),
            // **Nothing, and exit zero.** A command `preToolUse` hook is
            // fail-closed on a non-zero exit, so erroring out here would deny
            // every tool call on the machine the moment the daemon is not
            // running — turning an observer that is merely absent into one that
            // breaks the agent it was installed to watch. Saying nothing puts
            // the call back into the provider's own permission flow, which is
            // exactly where it would be with no hook installed at all.
            Err(_) => println!("{{}}"),
        }
        return Ok(());
    }

    let _ = reqwest::Client::new()
        .post(format!("{}/vibeplane/hook", info.base_url()))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(body)
        .timeout(std::time::Duration::from_millis(500))
        .send()
        .await;
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

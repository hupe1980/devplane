//! The command line: one module per thing a person is trying to do.
//!
//! It lives in the library so tests can drive a subcommand without a
//! subprocess; `main.rs` only parses and calls [`crate::cli::run`].

use crate::render::{DIM, paint};
use crate::{client, config, host, poller};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod admin;
mod board;
pub mod completions;

/// The argument the completion scripts call back with.
///
/// Not a clap subcommand: `clap_complete` offers even hidden subcommands, so
/// `main` answers this before clap parses and it never appears in a script.
pub const COMPLETE_ARG: &str = "__complete";
mod change;
mod github;
mod inbox;
mod report;

use admin::{
    cmd_agents, cmd_audit, cmd_connect, cmd_diagnostics, cmd_disconnect, cmd_rewind, cmd_search,
};
use board::{cmd_attach, cmd_ls, cmd_open, cmd_show, cmd_watch};
use change::{cmd_change, cmd_check, cmd_gate_run, cmd_speckit_install, cmd_trust};
use inbox::{cmd_answer, cmd_attention, cmd_inbox, cmd_inbox_all, cmd_snooze};

/// The five errands, and every non-hidden subcommand assigned to exactly one.
///
/// The one place the grouping is decided; `site/content/docs/cli.md` carries a
/// copy and a test holds the two together.
pub const COMMAND_GROUPS: &[(&str, &[&str])] = &[
    (
        "See what is happening",
        &["ls", "show", "watch", "search", "open"],
    ),
    (
        "What needs you, and what happened without you",
        &[
            "inbox",
            "answer",
            "attention",
            "audit",
            "modes",
            "forge",
            "snooze",
        ],
    ),
    (
        "Start and steer work",
        &["change", "report", "attach", "gate", "rewind"],
    ),
    (
        "Set up a project",
        &[
            "connect",
            "disconnect",
            "trust",
            "check",
            "explain",
            "speckit",
            "agents",
            "doctor",
            "login",
            "logout",
            "completions",
        ],
    ),
    ("The host", &["serve", "quit", "app"]),
];

/// The grouped command listing, which replaces clap's flat one.
///
/// Each description is the command's own `about`, read from a bare
/// `augment_subcommands` tree (`Cli::command()` would recurse through this), so
/// `--help` and `help <command>` cannot disagree. Plain text: it is often piped.
#[must_use]
pub fn groups_block() -> String {
    use clap::Subcommand;
    let tree = Command::augment_subcommands(clap::Command::new("devplane"));
    // A command this build hides (`app` without its feature) is not listed.
    let hidden = |name: &str| {
        tree.get_subcommands()
            .find(|c| c.get_name() == name)
            .is_some_and(clap::Command::is_hide_set)
    };
    let about = |name: &str| -> String {
        tree.get_subcommands()
            .find(|c| c.get_name() == name)
            .and_then(|c| c.get_about().map(ToString::to_string))
            .unwrap_or_default()
    };

    // One column width across every group, so descriptions line up screen-wide.
    let widest = COMMAND_GROUPS
        .iter()
        .flat_map(|(_, cs)| cs.iter())
        .map(|c| c.len())
        .max()
        .unwrap_or(0);

    let mut out = String::from("What you came here to do:\n");
    for (name, commands) in COMMAND_GROUPS {
        out.push_str(&format!("\n  {name}\n"));
        for c in commands.iter().filter(|c| !hidden(c)) {
            match about(c) {
                d if d.is_empty() => out.push_str(&format!("    {c}\n")),
                d => out.push_str(&format!("    {c:<widest$}  {d}\n")),
            }
        }
    }
    out.push_str("\nRun `devplane help <command>` for one of them.");
    out
}

#[derive(Parser)]
#[command(
    name = "devplane",
    version,
    about = "Know when an agent's work is actually done",
    long_about = "Devplane — know when an agent's work is actually done. Run any ACP agent in \
                  its own worktree, verify its change with your own checks run outside the \
                  agent, see the weakened tests first, and keep the record of who decided what \
                  while you were not looking.\n\n\
                  Start with `devplane open` (or `devplane app`), then \
                  `devplane change start \"<what to do>\"`.",
    after_help = groups_block(),
    after_long_help = groups_block(),
    // No `{subcommands}` or `{all-args}` (which carries them back in): the
    // grouped block replaces clap's flat list, placed above the options.
    help_template = "\
{before-help}{about-with-newline}
{usage-heading} {usage}{after-help}

Options:
{options}"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the host in the foreground.
    Serve {
        #[arg(long, env = "DEVPLANE_PORT", default_value_t = config::DEFAULT_PORT)]
        port: u16,
    },
    /// Show what is happening: sessions in play, and anything asking for you.
    #[command(visible_alias = "ps")]
    Ls {
        /// Include sessions that have never reported anything (idle editor tabs).
        #[arg(long, short)]
        all: bool,
        /// Only this project. Matches any part of the name: `mat` finds `matter-kit`.
        #[arg(long, short)]
        project: Option<String>,
        /// Only sessions that are waiting on a human.
        #[arg(long = "needs-you")]
        needs_you: bool,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Show what needs a human, most urgent first.
    ///
    /// Filters are never remembered between runs, and a narrowed list always
    /// says how many items it is not showing. `--all` lists everything an
    /// agent has asked you instead, open ones first, and what ended each: you,
    /// a clock your project set, or nobody.
    Inbox {
        /// Only this project. Matches any part of the name, as `ls --project` does.
        #[arg(long, short)]
        project: Option<String>,
        /// Only what can be answered from here (a question, not a red gate).
        #[arg(long = "needs-you")]
        needs_you: bool,
        /// Every question and permission an agent asked, settled ones too.
        #[arg(long, conflicts_with_all = ["project", "needs_you"])]
        all: bool,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Open GitHub issues and pull requests across every registered project,
    /// what needs you first.
    Forge {
        #[command(subcommand)]
        what: ForgeCmd,
        /// Print machine-readable JSON instead of text.
        #[arg(long, global = true)]
        json: bool,
    },
    /// Show one run in detail.
    Show {
        /// The run, or a unique prefix of it, as `devplane ls` prints it.
        run: String,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Search tool calls, questions and errors across every session.
    Search {
        /// Words to find in commands, questions and errors.
        query: String,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Which of a run's files the vendor's checkpoint will not bring back.
    ///
    /// Claude Code's `/rewind` restores only files its editing tools touched;
    /// this lists files Bash calls named for writing. The gate sees a call
    /// before it runs, so it says `named for writing`, never `changed`.
    Rewind {
        /// The run, or a unique prefix of it, as `devplane ls` prints it.
        run: String,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Show what Devplane decided, and on whose authority.
    ///
    /// Why a command ran without anybody being asked, and why a branch has a
    /// pull request.
    Audit {
        /// Narrow to one run or one change.
        about: Option<String>,
        /// Only what was decided instead of you: by a rule, a clock, or nobody.
        #[arg(long = "without-me")]
        without_me: bool,
        /// How many of the most recent rows to show.
        #[arg(long, default_value_t = 50)]
        limit: i64,
        /// Write the rows as OpenTelemetry GenAI log records instead.
        ///
        /// `gen_ai.tool.call.decision` events as OTLP/JSON on stdout, with the
        /// deciding authority under an application prefix (absent when
        /// unknown). Nothing is sent anywhere.
        #[arg(long)]
        otel: bool,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Write a shell completion script.
    ///
    ///   devplane completions zsh  > ~/.zsh/completions/_devplane
    ///   devplane completions bash > /etc/bash_completion.d/devplane
    ///   devplane completions fish > ~/.config/fish/completions/devplane.fish
    ///
    /// Ids (questions, sessions, changes) complete from a running host or the
    /// store, never starting one; zsh and fish show what each id is.
    #[command(verbatim_doc_comment)]
    Completions {
        /// bash, zsh or fish.
        shell: String,
    },
    /// Which projects are deciding without you, and what mode each is in.
    ///
    /// Live sessions only, least-supervised first; a session that has not
    /// reported a mode is shown as unknown.
    Modes {
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Show whether the inbox is worth reading, per kind.
    ///
    /// How often each kind of item was acted on, dismissed, or resolved
    /// somewhere else.
    Attention {
        /// How many days back to look.
        #[arg(long, default_value_t = 7)]
        days: i64,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Attach a terminal to a run, resuming its session.
    Attach {
        /// The run, or a unique prefix of it, as `devplane ls` prints it.
        run: String,
    },
    /// Answer something an agent asked you — a permission or a question.
    ///
    /// The id is the one `devplane inbox` prints, not a session id: it outlives
    /// the asking process, so a late answer reaches the agent through a resumed
    /// session. There is no dismiss.
    Answer {
        /// The ask, from `devplane inbox`.
        ask: String,
        /// Allow it — for a permission.
        #[arg(long, conflicts_with_all = ["deny", "custom", "option", "field"])]
        allow: bool,
        /// Refuse it — for a permission.
        #[arg(long, conflicts_with_all = ["allow", "custom", "option", "field"])]
        deny: bool,
        /// An exact option the agent offered, as it wrote it. Says the whole
        /// answer, so it cannot be combined with `--allow` or `--deny`.
        #[arg(long)]
        option: Option<String>,
        /// Your own words, where the agent offered an "Other" box. Wins over `--option`.
        #[arg(long)]
        custom: Option<String>,
        /// Which question, when the agent asked several at once.
        #[arg(long)]
        field: Option<String>,
    },
    /// Run this repository's own gates and report what they exited with.
    ///
    /// Decides on exit codes alone. Exits 0 only when the checks passed; no
    /// declared checks, or a config that will not parse, exits non-zero.
    Gate {
        /// Which repository. Defaults to the working directory.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// One named gate from `devplane.toml` instead of the whole `check`.
        /// A named gate never makes a change verified.
        #[arg(long)]
        name: Option<String>,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Register Devplane's gate as a Spec Kit extension hook.
    ///
    /// Spec Kit runs hooks from `.specify/extensions.yml`; this one runs the
    /// project's gates and reports what they exited with.
    Speckit {
        /// Which hook point. Defaults to `after_implement`.
        #[arg(long)]
        event: Option<String>,
        /// Print and write nothing.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Write the hook even where the repository declares no gate to run.
        #[arg(long)]
        anyway: bool,
    },
    /// List the agents Devplane can drive.
    Agents {
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Read this repository's devplane.toml and say what it will do.
    ///
    /// Does it parse, does everything it names exist, and is anything unsafe.
    Check {
        /// The repository to read. Defaults to the working directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Ask the gate what it would decide about one call, and why.
    ///
    /// Offline: reads the rules governing a directory, so a rule can be tested
    /// before it is committed.
    ///
    ///   devplane explain 'pnpm test && rm -rf /'
    ///   devplane explain --tool Read .env
    ///   devplane explain --tool Agent --input '{"isolation":"worktree"}'
    ///   devplane explain --replay
    ///
    /// `--replay` asks it of every observed call and names the rule that would
    /// stop the interruptions.
    #[command(verbatim_doc_comment)]
    Explain {
        /// The call, for a tool with a plain specifier: a command for `Bash`,
        /// a path for `Read` and `Edit`, a URL for `WebFetch`.
        #[arg(trailing_var_arg = true)]
        call: Vec<String>,
        /// The tool being called: `Bash`, `Read`, `Edit`, `WebFetch`, …
        #[arg(long, default_value = "Bash")]
        tool: String,
        /// The whole tool input as JSON, for a call a specifier cannot express.
        #[arg(long)]
        input: Option<String>,
        /// The directory the agent would work in, which decides whose rules apply.
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Replay every tool call already observed against the current rules,
        /// and say which rule would answer the ones that reached you.
        #[arg(long, conflicts_with_all = ["call", "input"])]
        replay: bool,
        /// How many of the most recent calls to replay.
        #[arg(long, default_value_t = 5000)]
        limit: i64,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Allow Devplane to start agents in a repository.
    ///
    /// A headless agent runs the repository's own hooks and MCP servers without
    /// asking, so this prints what those are first. `--dry-run` prints them and
    /// trusts nothing.
    Trust {
        /// The repository to trust. Defaults to the working directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Trust without asking. For scripts and for a directory you wrote.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Print what is there and trust nothing.
        #[arg(long)]
        dry_run: bool,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Start a change: an isolated checkout, an agent in it, and the project's
    /// gates when the agent says it is finished.
    Change {
        #[command(subcommand)]
        what: ChangeCmd,
        /// Print machine-readable JSON instead of text.
        #[arg(long, global = true)]
        json: bool,
    },
    /// File a finding about another project, and answer the ones filed here.
    ///
    /// It reaches that project's person, not its agent, unless its
    /// `[reports] deliver_from` names this one. A GitHub target gets a draft
    /// that nothing sends until you open it.
    Report {
        #[command(subcommand)]
        what: ReportCmd,
        /// Print machine-readable JSON instead of text.
        #[arg(long, global = true)]
        json: bool,
    },
    /// Hide a run's or a change's inbox items for a while.
    Snooze {
        /// A run id, or a change id from `devplane inbox`.
        id: String,
        /// Minutes to stay quiet. `0` un-snoozes.
        #[arg(long, default_value_t = 60)]
        minutes: i64,
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Open the workbench in a browser, hosting here if nothing is running.
    Open,
    /// Follow events as they arrive, or one driven run's conversation.
    ///
    /// Given a run Devplane drives, prints its conversation like `tail -f`. For
    /// a session you started yourself, `devplane attach` resumes it in a terminal.
    Watch {
        /// One run to follow, as `devplane ls` prints it.
        run: Option<String>,
        /// Include the agent's reasoning, where it streams any.
        #[arg(long, requires = "run")]
        thinking: bool,
        /// How much of the conversation so far to print first.
        #[arg(long, default_value_t = 40, requires = "run")]
        history: i64,
    },
    /// Channel health, latency, whether a host answers, and each GitHub
    /// host's sign-in.
    #[command(visible_alias = "diagnostics")]
    Doctor {
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Sign in to GitHub: a code to enter at GitHub's device page, and the
    /// token kept only in the OS credential store.
    Login {
        #[command(subcommand)]
        what: LoginTarget,
        /// Print machine-readable JSON instead of text.
        #[arg(long, global = true)]
        json: bool,
    },
    /// Sign out of GitHub: the stored token is deleted, and where to revoke
    /// the grant at GitHub is said.
    Logout {
        #[command(subcommand)]
        what: LogoutTarget,
        /// Print machine-readable JSON instead of text.
        #[arg(long, global = true)]
        json: bool,
    },
    /// Install Devplane's hooks into a provider.
    Connect {
        #[command(subcommand)]
        what: ConnectTarget,
        /// Also wrap the status line, the only source of subscription rate
        /// limits. Off by default: it touches a command you configured.
        // Global, so the flag works after the target as well as before it.
        #[arg(long, global = true)]
        statusline: bool,
        /// Write the shown changes without asking. Required when stdin is
        /// not a terminal, and under `--json`.
        #[arg(short = 'y', long, global = true)]
        yes: bool,
        /// The devplane binary the hooks run. Defaults to this one, which is
        /// refused when it lives somewhere temporary (npx's cache, a mounted
        /// disk image).
        #[arg(long, global = true, value_name = "PATH")]
        bin: Option<PathBuf>,
        /// Print machine-readable JSON instead of text.
        #[arg(long, global = true)]
        json: bool,
    },
    /// Remove everything `connect` installed.
    Disconnect {
        #[command(subcommand)]
        what: ConnectTarget,
        /// Write the shown changes without asking.
        #[arg(short = 'y', long, global = true)]
        yes: bool,
        /// Print machine-readable JSON instead of text.
        #[arg(long, global = true)]
        json: bool,
    },
    /// Quit the running host, saying what that ends before it ends it.
    Quit {
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Open the window: the same host, with a tray item, notifications, one
    /// global shortcut and links that open the right change.
    ///
    /// Closing the window keeps hosting; quitting from the tray says what it
    /// stops. Nothing starts at login.
    #[cfg_attr(
        not(feature = "app"),
        command(
            hide = true,
            about = "Not in this build: built without the app feature (cargo install devplane --features app)"
        )
    )]
    App {
        /// The port to bind. Overrides `[app] port` in `~/.devplane/app.toml`
        /// (default 0: pick one).
        #[arg(long)]
        port: Option<u16>,
    },
    /// Serve Devplane's read-only surface to an agent over MCP, on stdio.
    ///
    /// Five tools — `inbox`, `change`, `explain`, `audit`, `reports` — none of
    /// which mutates. Register it as a `command` MCP server running
    /// `devplane mcp`. Hidden: an agent runs it, not a person.
    #[command(hide = true)]
    Mcp,
    /// Read a hook payload on stdin, decide it here and write it to the store.
    ///
    /// What a vendor runs on every hook event; `devplane connect` installs it.
    #[command(hide = true)]
    Hook {
        /// Answer this provider's permission hook (Copilot's payload shape).
        #[arg(long, value_name = "PROVIDER")]
        gate: Option<String>,
        /// Record this provider's observation hook rather than deciding it.
        #[arg(long, value_name = "PROVIDER")]
        observe: Option<String>,
        #[arg(long, value_name = "EVENT")]
        event: Option<String>,
        /// The vendor, for one speaking Claude Code's payloads verbatim (Codex).
        #[arg(long, value_name = "VENDOR")]
        vendor: Option<String>,
    },
    /// Record a status-line payload from stdin, then run the command that was
    /// there before. Called by the shim `devplane connect claude` writes.
    #[command(hide = true)]
    Statusline {
        /// The user's original status-line command, run after forwarding.
        #[arg(long)]
        then: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum LoginTarget {
    /// Sign in to GitHub (github.com, or `[github] host` in app.toml).
    Github {
        /// A GitHub Enterprise host, instead of the configured one.
        #[arg(long)]
        host: Option<String>,
        /// Read a token from stdin instead of the device flow — never from
        /// an argument. For a host with no registered app, and for CI.
        #[arg(long)]
        with_token: bool,
    },
}

#[derive(Subcommand)]
pub enum LogoutTarget {
    /// Sign out of GitHub.
    Github {
        /// A GitHub Enterprise host, instead of the configured one.
        #[arg(long)]
        host: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ForgeCmd {
    /// Every open issue, what needs you first.
    ///
    /// `--ready` narrows it to one repository's issues carrying
    /// `[github].ready_label`: the list `change start --issue` picks from.
    Issues {
        /// Only the issues this repository offers as work.
        #[arg(long)]
        ready: bool,
        /// The repository to ask. Defaults to the working directory. Implies `--ready`.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Only issues with this label. Defaults to `[github].ready_label`. Implies `--ready`.
        #[arg(long)]
        label: Option<String>,
    },
    /// Every open pull request, what needs you first.
    Prs,
}

#[derive(Subcommand, Debug)]
pub enum ChangeCmd {
    /// Begin a new change — in this repository, or the same prompt in several.
    ///
    /// Every `--project` is checked first (trusted, clean, config readable);
    /// one refusal starts none of them.
    Start {
        /// What to do. Becomes the branch name and the first prompt.
        title: Vec<String>,
        /// Which agent to start, as `devplane agents` lists it. Defaults to the
        /// project's `[agent] default`.
        #[arg(long)]
        agent: Option<String>,
        /// A project to start it in: a registered name or a path. Repeatable,
        /// to send one prompt to several. Defaults to the current directory.
        #[arg(long = "project", value_name = "NAME|PATH")]
        project: Vec<String>,
        /// Work in the repository itself rather than an isolated checkout.
        #[arg(long)]
        no_worktree: bool,
        /// Start from a GitHub issue. Its title and body become the change.
        #[arg(long)]
        issue: Option<u64>,
        /// The specification this change answers — a file or folder, relative
        /// to the repository. Stamped onto the done certificate with its task
        /// list's `- [ ]` progress at each gate run.
        #[arg(long)]
        spec: Option<String>,
        /// Which tasks of `--spec` to send, repeatable: a requirement token
        /// (every task line citing it) or `file:line`. A selector matching
        /// nothing refuses.
        #[arg(long = "task", requires = "spec")]
        task: Vec<String>,
    },
    /// Show every change.
    #[command(visible_alias = "ls")]
    List,
    /// Run the project's gates now.
    Verify {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
    },
    /// Hand the failures back to the agent once more, past the project's bound.
    Retry {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
    },
    /// Pick a change back up after a restart, against the same agent session.
    ///
    /// Reconnects to the conversation the agent kept rather than starting anew.
    Resume {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
    },
    /// Show one change: its state, its runs, and what its checks said.
    Show {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
    },
    /// Print the done certificate: what was checked, against which commit, and
    /// how to check it yourself.
    ///
    /// Meant for a pull request: a reviewer can check out the commit and run
    /// the commands. `--json` gives an in-toto statement.
    Export {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
    },
    /// Accept a change as finished. Records the basis; removes nothing.
    Finish {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
    },
    /// Decide what to do about a specification that moved under a run.
    ///
    /// `--tell` prompts the run with the changed files (resuming it if needed);
    /// `--accept` moves the change's start to what the run saw.
    Drift {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
        /// The run the drift names.
        #[arg(long)]
        run: String,
        /// Accept the drift: move the change's start to what the run saw.
        #[arg(long, conflicts_with = "tell")]
        accept: bool,
        /// Tell the run which specification files changed.
        #[arg(long)]
        tell: bool,
    },
    /// Take a branch somebody made by hand and make it a change.
    ///
    /// The title defaults to the first commit's subject; the worktree is an
    /// existing checkout of the branch or a new one under `.claude/worktrees/`.
    Adopt {
        branch: String,
        /// The repository root. Defaults to the current directory's.
        #[arg(long)]
        project: Option<PathBuf>,
        /// The specification this change answers, relative to the repository.
        #[arg(long)]
        spec: Option<String>,
        /// The change's title. Defaults to the first commit's subject.
        #[arg(long)]
        title: Option<String>,
    },
    /// Remove the worktree and keep the record.
    ///
    /// Refuses uncommitted or unmerged work unless forced. The branch is kept
    /// unless asked, and deleted only when its base has every commit.
    Archive {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
        /// Delete the branch too. Refused while it has commits its base does
        /// not, unless its pull request merged.
        #[arg(long)]
        delete_branch: bool,
        /// Remove the worktree even with uncommitted or untracked files in it.
        #[arg(long)]
        discard_uncommitted: bool,
        /// With --delete-branch: delete it even with unmerged commits.
        #[arg(long, requires = "delete_branch")]
        force: bool,
    },
    /// Push the branch and open the pull request — or, when `[github]
    /// pull_request` does not say Devplane may, print the push command and
    /// the address that opens the pull request in a browser.
    Offer {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
    },
    /// Read a change in review order: files ordered by `[review] roles`, each
    /// with its role, test coverage, decisions and task, and the gate standing.
    ///
    /// `--by intent` groups files by the run that wrote them and its tasks.
    ///
    /// `--seen <path>` marks every current *checks weakened or changed* row at
    /// that path read, as yours; an offer waits until every such row is.
    Review {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it.
        change: String,
        #[arg(long, value_parser = ["risk", "intent"], default_value = "risk")]
        by: String,
        /// Mark the weakened rows at this path seen (repeatable).
        #[arg(long, value_name = "PATH")]
        seen: Vec<String>,
    },
    /// Send a message to a change's agent without stopping it. Mid-turn, it is
    /// queued until the turn ends.
    Prompt {
        /// A change — its latest run is the one told — or a run.
        run: String,
        text: Vec<String>,
    },
    /// Stop a run. Says first what survives, then asks on a terminal;
    /// anywhere else it needs `--yes`.
    Stop {
        run: String,
        /// Stop without asking. Required when stdin is not a terminal.
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReportCmd {
    /// File a report. The origin comes from `DEVPLANE_RUN` or
    /// `CLAUDE_CODE_SESSION_ID`, never an argument; a person says `--as-person`.
    File {
        /// A registered project, `owner/name`, or a GitHub URL.
        #[arg(long)]
        to: String,
        /// Draft an issue on the registered project's GitHub remote instead
        /// of reaching its person here.
        #[arg(long)]
        forge: bool,
        /// A target that is neither stays on the change it came from.
        #[arg(long)]
        keep: bool,
        /// `defect`, `request`, `question` or `breaking`.
        #[arg(long)]
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        finding: String,
        /// The command that shows it.
        #[arg(long)]
        command: Option<String>,
        /// What that command printed. Up to 4 KiB: keep the lines that show it.
        #[arg(long)]
        output_file: Option<PathBuf>,
        #[arg(long)]
        stack_file: Option<PathBuf>,
        /// `a..b`.
        #[arg(long)]
        commits: Option<String>,
        /// A file the finding is about, in the filing project. Repeatable.
        #[arg(long)]
        path: Vec<String>,
        /// You are filing this yourself, from the project this directory is in.
        #[arg(long)]
        as_person: bool,
    },
    /// Reports waiting for an answer — `--all` for every one.
    Ls {
        /// Only the ones filed against the project this directory is in.
        #[arg(long, conflicts_with = "from_me")]
        to_me: bool,
        /// Only the ones the project this directory is in filed.
        #[arg(long)]
        from_me: bool,
        #[arg(long)]
        all: bool,
    },
    /// One report: where it came from, what became of it, and the report
    /// itself, quoted.
    Show { id: String },
    /// Start a change in the target project from a report.
    Start {
        id: String,
        #[arg(long)]
        agent: Option<String>,
    },
    /// Refuse a report. The project that filed it is told why.
    Reject {
        id: String,
        #[arg(long)]
        reason: String,
    },
    /// Put a report off. The project that filed it is told why.
    Defer {
        id: String,
        #[arg(long)]
        reason: String,
    },
    /// Answer a report as fixed by hand.
    Fixed {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Open a GitHub draft as an issue under your own sign-in. The draft is
    /// shown and asked about first on a terminal; anywhere else it needs
    /// `--yes`.
    Open {
        id: String,
        /// Open it without asking. Required when stdin is not a terminal.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Throw a GitHub draft away. Nothing was ever sent.
    Discard { id: String },
}

#[derive(Subcommand, Clone, Copy, PartialEq, Eq)]
pub enum ConnectTarget {
    /// Claude Code, through its user-scope settings.
    Claude,
    /// Codex, through `~/.codex/hooks.json`. The entries wait for your
    /// approval in Codex's own dialog.
    Codex,
    /// GitHub Copilot, through one file in `~/.copilot/hooks/`. Telemetry is
    /// not installed: Copilot reads it from the environment.
    Copilot,
}

impl Cli {
    /// Whether this command was asked for JSON, so an error is printed the
    /// same way its output would have been.
    #[must_use]
    pub fn json(&self) -> bool {
        match &self.command {
            Some(
                Command::Ls { json, .. }
                | Command::Inbox { json, .. }
                | Command::Forge { json, .. }
                | Command::Show { json, .. }
                | Command::Search { json, .. }
                | Command::Rewind { json, .. }
                | Command::Audit { json, .. }
                | Command::Modes { json }
                | Command::Attention { json, .. }
                | Command::Gate { json, .. }
                | Command::Agents { json }
                | Command::Check { json, .. }
                | Command::Explain { json, .. }
                | Command::Trust { json, .. }
                | Command::Change { json, .. }
                | Command::Report { json, .. }
                | Command::Snooze { json, .. }
                | Command::Doctor { json }
                | Command::Login { json, .. }
                | Command::Logout { json, .. }
                | Command::Connect { json, .. }
                | Command::Disconnect { json, .. }
                | Command::Quit { json },
            ) => *json,
            _ => false,
        }
    }
}

/// A command that has already said everything it had to and wants this exit
/// status. Returned rather than calling `std::process::exit`, so destructors
/// (the host lock, flushed output) run and a test can drive the command.
#[derive(Debug)]
pub struct Exit(pub i32);

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit status {}", self.0)
    }
}

impl std::error::Error for Exit {}

/// Prints a command's error — as `{"error": …}` on stdout under `--json`, as
/// text on stderr otherwise — and returns the process's exit status. An
/// [`Exit`] has already been reported and prints nothing.
#[must_use]
pub fn report_error(e: &anyhow::Error, json: bool) -> i32 {
    if let Some(Exit(code)) = e.downcast_ref::<Exit>() {
        return *code;
    }
    if json {
        println!("{}", serde_json::json!({ "error": format!("{e:#}") }));
    } else {
        eprintln!("Error: {e:?}");
    }
    1
}

/// Prints a host's answer as the `--json` document. An answer that carries
/// `error` is printed the same and fails the command: a script reading the
/// exit status must not take a refusal for success.
pub(crate) fn print_json(v: &serde_json::Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    if v.get("error").is_some_and(|e| !e.is_null()) {
        return Err(Exit(1).into());
    }
    Ok(())
}

/// One rule for every question the CLI asks before it acts: on a terminal it
/// asks on stderr, so `--json` output stays one document; anywhere else it
/// refuses unless `--yes` said so, because nobody is there to answer and a
/// pipe must not read as consent.
pub(crate) fn confirm(question: &str, yes: bool) -> Result<bool> {
    use std::io::IsTerminal;
    confirm_with(
        question,
        yes,
        std::io::stdin().is_terminal(),
        &mut std::io::stdin().lock(),
        &mut std::io::stderr(),
    )
}

fn confirm_with(
    question: &str,
    yes: bool,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    prompt: &mut dyn std::io::Write,
) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !terminal {
        anyhow::bail!(
            "nothing was done: stdin is not a terminal, so nobody can answer \"{question}\" — \
             re-run with `--yes`"
        );
    }
    write!(prompt, "  {question} [y/N] ")?;
    prompt.flush()?;
    let mut answer = String::new();
    if input.read_line(&mut answer)? == 0 {
        writeln!(prompt)?;
        return Ok(false);
    }
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

/// Runs the parsed command. Takes a `Cli` so a test can drive it in-process.
pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Serve { port }) => cmd_serve(port).await,
        Some(Command::Ls {
            all,
            project,
            needs_you,
            json,
        }) => cmd_ls(all, project.as_deref(), needs_you, json).await,
        None => cmd_ls(false, None, false, false).await,
        Some(Command::Inbox {
            all: true, json, ..
        }) => cmd_inbox_all(json).await,
        Some(Command::Inbox {
            project,
            needs_you,
            json,
            ..
        }) => cmd_inbox(json, project.as_deref(), needs_you).await,
        Some(Command::Forge { what, json }) => match what {
            ForgeCmd::Issues { ready, cwd, label } => {
                if ready || cwd.is_some() || label.is_some() {
                    crate::cli::change::cmd_ready_issues(cwd, label, json).await
                } else {
                    crate::cli::board::cmd_forge_issues(json).await
                }
            }
            ForgeCmd::Prs => crate::cli::board::cmd_forge_prs(json).await,
        },
        Some(Command::Show { run, json }) => cmd_show(&run, json).await,
        Some(Command::Search { query, json }) => cmd_search(&query, json).await,
        Some(Command::Rewind { run, json }) => cmd_rewind(&run, json).await,
        Some(Command::Audit {
            about,
            without_me,
            limit,
            otel,
            json,
        }) => cmd_audit(about.as_deref(), without_me, limit, json, otel).await,
        Some(Command::Completions { shell }) => crate::cli::completions::cmd_completions(&shell),
        Some(Command::Modes { json }) => crate::cli::inbox::cmd_modes(json).await,
        Some(Command::Attention { days, json }) => cmd_attention(days, json).await,
        Some(Command::Attach { run }) => cmd_attach(&run).await,
        Some(Command::Answer {
            ask,
            allow,
            deny,
            option,
            custom,
            field,
        }) => cmd_answer(&ask, allow, deny, option, custom, field).await,
        Some(Command::Gate { cwd, name, json }) => cmd_gate_run(cwd, name, json).await,
        Some(Command::Speckit {
            event,
            dry_run,
            anyway,
        }) => cmd_speckit_install(event, dry_run, anyway),
        Some(Command::Agents { json }) => cmd_agents(json).await,
        Some(Command::Check { path, json }) => cmd_check(path, json).await,
        Some(Command::Explain {
            call,
            tool,
            input,
            dir,
            replay,
            limit,
            json,
        }) => {
            if replay {
                crate::cli::change::cmd_replay(dir, limit, json).await
            } else {
                crate::cli::change::cmd_explain(dir, tool, call, input, json)
            }
        }
        Some(Command::Trust {
            path,
            yes,
            dry_run,
            json,
        }) => cmd_trust(path, yes, dry_run, json).await,
        Some(Command::Change { what, json }) => cmd_change(what, json).await,
        Some(Command::Report { what, json }) => report::cmd_report(what, json).await,
        Some(Command::Snooze { id, minutes, json }) => cmd_snooze(&id, minutes, json).await,
        Some(Command::Open) => cmd_open().await,
        Some(Command::Watch {
            run,
            thinking,
            history,
        }) => cmd_watch(run.as_deref(), thinking, history).await,
        Some(Command::Doctor { json }) => cmd_diagnostics(json).await,
        Some(Command::Login {
            what: LoginTarget::Github { host, with_token },
            json,
        }) => github::cmd_login(host, with_token, json).await,
        Some(Command::Logout {
            what: LogoutTarget::Github { host },
            json,
        }) => github::cmd_logout(host, json).await,
        Some(Command::Connect {
            what,
            statusline,
            yes,
            bin,
            json,
        }) => cmd_connect(what, statusline, yes, json, bin).await,
        Some(Command::Disconnect { what, yes, json }) => cmd_disconnect(what, yes, json).await,
        Some(Command::Quit { json }) => cmd_quit(json).await,
        #[cfg(feature = "app")]
        Some(Command::App { port }) => crate::app::run(port).await,
        #[cfg(not(feature = "app"))]
        Some(Command::App { .. }) => anyhow::bail!(
            "this binary was built without the app feature, so there is no window to open. \
             Install one with `cargo install devplane --features app`, or run `devplane open` \
             for the same page in a browser."
        ),
        Some(Command::Mcp) => crate::mcp::Server::run().await,
        Some(Command::Hook {
            gate,
            observe,
            event,
            vendor,
        }) => crate::hook::run(gate, observe, event, vendor).await,
        Some(Command::Statusline { then }) => crate::hook::statusline(then).await,
    }
}

// ---------------------------------------------------------------------------

pub(crate) async fn cmd_serve(port: u16) -> Result<()> {
    init_tracing();
    // Held until this function returns — the host's whole life.
    let _lock = refuse_if_hosting().await?;
    let state = boot_state().await?;
    drain_decision_spool(&state).await;
    poller::reconcile_at_startup(&state).await;
    // Fill the board before anyone can ask for it.
    poller::initial_poll(&state).await;
    host::serve(state, port).await
}

/// The host's log, on stderr, filtered by `DEVPLANE_LOG`.
pub(crate) fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("DEVPLANE_LOG")
                .unwrap_or_else(|_| "devplane=info,warn".into()),
        )
        .init();
}

/// Refuses to start a second host, and clears a record nothing answers on.
///
/// `~/.devplane/host.lock` is taken exclusively before anything boots, so two
/// hosts started at once cannot both win; `host.json` only names the holder in
/// the refusal. A `host.json` found while the lock is free is stale. The caller
/// (`serve` or the app) keeps the returned lock for as long as it hosts.
pub(crate) async fn refuse_if_hosting() -> Result<config::HostLock> {
    let home = config::home()?;
    let lock = match config::lock_host_at(&home) {
        Ok(lock) => lock,
        Err(config::LockRefused::Held) => match config::read_host()? {
            Some(info) => anyhow::bail!(
                "a host is already running (pid {}, port {}, up {}). Stop it with `devplane quit`.",
                info.pid,
                info.port,
                uptime_of(&info.started_at)
            ),
            None => anyhow::bail!(
                "another host is starting in {} right now (it holds host.lock).",
                home.display()
            ),
        },
        Err(config::LockRefused::Io(e)) => {
            return Err(anyhow::Error::from(e)
                .context(format!("taking {}", home.join("host.lock").display())));
        }
    };
    if let Some(info) = config::read_host()? {
        tracing::warn!(
            port = info.port,
            "host.json names a host that no longer holds the lock; the last host did not shut \
             down cleanly. Ignoring it."
        );
        config::clear_host().ok();
    }
    Ok(lock)
}

/// The host's state, restored from the store with the machine-wide policy.
pub(crate) async fn boot_state() -> Result<std::sync::Arc<host::AppState>> {
    let token = config::load_or_create_token()?;
    let policy = load_policy();
    let home = config::home()?;
    host::AppState::new(config::db_path()?, token, policy, home).await
}

/// How long ago something started, in the roughest unit still useful. An
/// unparseable or future timestamp is named, never a negative duration.
fn uptime_of(started_at: &str) -> String {
    let Ok(then) = started_at.parse::<jiff::Timestamp>() else {
        return "start time unknown".into();
    };
    let secs = (jiff::Timestamp::now() - then).get_seconds();
    match secs {
        s if s < 0 => "start time in the future".into(),
        s if s < 90 => format!("{s}s"),
        s if s < 5400 => format!("{}m", s / 60),
        s if s < 172_800 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}

/// Files the decisions the hook spooled because it could not write the store,
/// so `devplane audit` is not missing them.
pub(crate) async fn drain_decision_spool(state: &std::sync::Arc<host::AppState>) {
    let pending = config::drain_spool();
    if pending.is_empty() {
        return;
    }
    tracing::info!(
        count = pending.len(),
        "filing decisions the hook could not write down"
    );
    for row in pending {
        match serde_json::from_value::<crate::core::DecidedEnvelope>(row.clone()) {
            Ok(env) => {
                if let Err(e) = crate::record::decided(&state.store, env).await {
                    tracing::warn!(error = %e, "a spooled decision could not be filed");
                }
            }
            // A row nobody can read is a decision the ledger lost: say so, with
            // the row, rather than dropping it in silence.
            Err(e) => tracing::error!(
                error = %e,
                row = %row,
                "a spooled decision would not parse and is not in the ledger"
            ),
        }
    }
}

/// The machine-wide rules, from `~/.devplane/policy.toml`.
///
/// Empty by default, so every permission prompt reaches the human. A file that
/// will not load means every call is asked, never no rules; the host still
/// starts and logs an error.
fn load_policy() -> crate::core::Policy {
    let (policy, _) = crate::policy_cache::global_policy();
    match policy.load_error() {
        Some(e) => tracing::error!(
            error = %e,
            "the machine-wide policy would not load; every call is asked until it does"
        ),
        None => tracing::info!(
            deny = policy.deny_rules().len(),
            ask = policy.ask_rules().len(),
            "policy loaded"
        ),
    }
    policy
}

/// One read, from wherever answers: a running host, or the store.
async fn raw(c: &crate::local::Reader, path: &str) -> Result<serde_json::Value> {
    c.get(path).await
}

/// Fetches one run, or says there is no such run and how to list them.
async fn fetch_run(c: &crate::local::Reader, run: &str) -> Result<serde_json::Value> {
    c.get(&format!("/api/runs/{run}")).await.map_err(|_| {
        anyhow::anyhow!(
            "no run `{run}` on the board.\n\n  {}",
            paint(DIM, "devplane ls --all lists every session, ids included.")
        )
    })
}

/// Percent-encodes a query value — the one encoder this repository has.
fn urlencode(s: &str) -> String {
    crate::core::text::url_escape(s)
}

/// Quit the host: say what this ends, then end it, then confirm it is gone.
///
/// The inventory is printed before the stop, because a person cannot consent
/// to what they were not told. The stop is an authenticated request, never a
/// signal to a pid from `host.json` (which may be stale or reused), and it
/// reports stopped only once the process has exited.
async fn cmd_quit(json: bool) -> Result<()> {
    let Some(info) = config::read_host()? else {
        if json {
            println!("{}", serde_json::json!({"running": false}));
        } else {
            println!("Nothing is running.");
        }
        return Ok(());
    };

    // A stale record: clear it and say which port it named.
    let client = client::Client::connect()?;
    if !client.healthy().await {
        config::clear_host().ok();
        if json {
            println!(
                "{}",
                serde_json::json!({"running": false, "cleared_stale_port": info.port})
            );
        } else {
            println!(
                "Nothing is running. Cleared a record left behind by a host on port {}.",
                info.port
            );
        }
        return Ok(());
    }

    // Unreadable is not empty: a failed read must not print as "stops nothing".
    let held = client
        .get::<crate::core::reduce::facts::Quitting>("/api/quitting")
        .await;

    if json {
        let inventory = match &held {
            Ok(q) => serde_json::to_value(q)?,
            Err(e) => serde_json::json!({"unreadable": e.to_string()}),
        };
        println!(
            "{}",
            serde_json::json!({"running": true, "pid": info.pid, "stops": inventory})
        );
    } else {
        match &held {
            Ok(q) => print!("{}", q.says()),
            Err(e) => println!(
                "Could not read what this would stop ({e}). Quitting anyway — \
                 anything it started ends with it."
            ),
        }
    }

    client.stop_and_wait().await;

    // The port going quiet is not the host having gone: `stop_and_wait`
    // returns before `AppState::shutdown` has reaped agents (up to 10 s). So
    // wait for the process itself, with margin for a loaded machine.
    let gone = {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if !poller::process_alive(info.pid) {
                break true;
            }
            if std::time::Instant::now() >= deadline {
                break false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    };

    if json {
        println!("{}", serde_json::json!({"stopped": gone, "pid": info.pid}));
    } else if gone {
        println!("Stopped.");
    } else {
        // Not an error: the stop was accepted but has not finished.
        println!(
            "Asked it to stop, but pid {} is still running after 15s. \
             It may still be waiting for an agent to exit.",
            info.pid
        );
    }
    Ok(())
}

#[cfg(test)]
mod uptime_tests {
    use super::uptime_of;

    fn ago(secs: i64) -> String {
        (jiff::Timestamp::now() - jiff::SignedDuration::from_secs(secs)).to_string()
    }

    #[test]
    fn the_unit_is_the_roughest_one_still_useful() {
        assert_eq!(uptime_of(&ago(30)), "30s");
        assert_eq!(uptime_of(&ago(600)), "10m");
        assert_eq!(uptime_of(&ago(7200)), "2h");
        assert_eq!(uptime_of(&ago(9 * 86_400)), "9d");
    }

    #[test]
    fn an_unusable_start_time_is_named_rather_than_computed() {
        assert_eq!(uptime_of("not a timestamp"), "start time unknown");
        assert_eq!(uptime_of(""), "start time unknown");
        let future = (jiff::Timestamp::now() + jiff::SignedDuration::from_secs(600)).to_string();
        assert_eq!(uptime_of(&future), "start time in the future");
    }
}

#[cfg(test)]
mod confirm_tests {
    use super::confirm_with;

    fn ask(yes: bool, terminal: bool, typed: &str) -> (anyhow::Result<bool>, String) {
        let mut input = std::io::Cursor::new(typed.as_bytes().to_vec());
        let mut prompt = Vec::new();
        let r = confirm_with("Stop it?", yes, terminal, &mut input, &mut prompt);
        (r, String::from_utf8(prompt).unwrap())
    }

    #[test]
    fn a_json_refusal_is_printed_and_fails_the_command() {
        let e = super::print_json(&serde_json::json!({"error": "no such change"})).unwrap_err();
        assert_eq!(e.downcast_ref::<super::Exit>().map(|x| x.0), Some(1));
        assert!(super::print_json(&serde_json::json!({"error": null, "ok": true})).is_ok());
        assert!(super::print_json(&serde_json::json!({"state": "finished"})).is_ok());
    }

    #[test]
    fn without_a_terminal_only_yes_proceeds() {
        // A pipe is not consent: `y` piped in is refused the same as nothing.
        let (r, said) = ask(false, false, "y\n");
        assert!(r.unwrap_err().to_string().contains("--yes"));
        assert!(said.is_empty(), "nothing is asked of nobody");
        assert!(ask(true, false, "").0.unwrap());
    }

    #[test]
    fn on_a_terminal_the_question_is_asked_and_no_is_the_default() {
        let (r, said) = ask(false, true, "y\n");
        assert!(r.unwrap());
        assert!(said.contains("Stop it? [y/N]"));
        assert!(!ask(false, true, "\n").0.unwrap());
        assert!(!ask(false, true, "").0.unwrap(), "end of input is no");
    }
}

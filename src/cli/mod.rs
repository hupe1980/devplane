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
mod agent;
mod board;
mod change;
pub mod completions;
mod conf;
mod doctor;
mod forge;
mod gate;
mod github;
mod ids;
mod inbox;
mod project;
mod verify;

/// The argument the completion scripts call back with.
///
/// Not a clap subcommand: `clap_complete` offers even hidden subcommands, so
/// `main` answers this before clap parses and it never appears in a script.
pub const COMPLETE_ARG: &str = "__complete";

/// What the process was started to do, read before clap parses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// Opened by a `devplane://` link: Linux and Windows start the binary
    /// with the link as its only argument when the app is not running.
    Link(String),
    /// Anything else is the command line.
    Cli,
}

/// Reads the arguments (program name first) for a link launch. Only a lone
/// argument counts, as the operating system passes it; clap would refuse it as
/// an unknown command.
pub fn launch_of<I, S>(args: I) -> Launch
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut args = args.into_iter().skip(1);
    match (args.next(), args.next()) {
        (Some(one), None) => match one.as_ref().to_str() {
            Some(url)
                if url
                    .get(..11)
                    .is_some_and(|s| s.eq_ignore_ascii_case("devplane://")) =>
            {
                Launch::Link(url.to_string())
            }
            _ => Launch::Cli,
        },
        _ => Launch::Cli,
    }
}

/// Whether an app build should open the window rather than run the command
/// line: started with no argument at all and no terminal on stdin, as Finder,
/// the Start menu and a desktop launcher start it. A bare `devplane` typed in
/// a terminal still runs `status`. (A pipe into `devplane` with no command
/// also has no terminal; a script names its command.)
pub fn opens_window<S: AsRef<std::ffi::OsStr>>(args: &[S], stdin_is_terminal: bool) -> bool {
    args.len() <= 1 && !stdin_is_terminal
}

/// Starts the window, as the operating system's launch of the app does. A
/// binary without the window never gets here: `main` asks only in an app build.
pub async fn open_window() -> Result<()> {
    #[cfg(feature = "app")]
    {
        crate::app::run(None).await
    }
    #[cfg(not(feature = "app"))]
    anyhow::bail!("{NO_WINDOW}")
}

/// What a binary without the window says when asked for one.
pub const NO_WINDOW: &str = "this binary was built without the app feature, so there is no \
     window to open. Download the desktop app from https://github.com/hupe1980/devplane/releases, \
     or run `devplane up` for the same page in a browser.";

/// Starts the window on a link. The deep-link plugin reads the link from this
/// process's arguments, so the window opens on it as it first shows.
pub async fn open_link(url: &str) -> Result<()> {
    #[cfg(feature = "app")]
    {
        tracing::debug!(url, "started by a link");
        crate::app::run(None).await
    }
    #[cfg(not(feature = "app"))]
    anyhow::bail!(
        "`{url}` is a link for the Devplane window, and this binary was built without the \
         app feature. Download the desktop app from https://github.com/hupe1980/devplane/releases, \
         or run `devplane up` and open the change from the browser."
    )
}

/// The five errands, and every non-hidden subcommand assigned to exactly one.
///
/// The one place the grouping is decided; `site/content/docs/cli.md` carries a
/// copy and a test holds the two together.
pub const COMMAND_GROUPS: &[(&str, &[&str])] = &[
    ("See what is happening", &["status", "run", "search"]),
    (
        "What needs you, and what happened without you",
        &["inbox", "audit", "pr", "issue"],
    ),
    ("Start and steer work", &["change", "gate", "verify"]),
    (
        "Set up a project",
        &[
            "project",
            "config",
            "agent",
            "auth",
            "doctor",
            "completions",
        ],
    ),
    ("The host", &["up", "down"]),
];

/// The names the command line had before it was reorganised by noun, and
/// what each is now. Refused, never aliased: this only words the refusal.
const RENAMED: &[(&str, &str)] = &[
    ("ps", "devplane status"),
    ("open", "devplane up"),
    ("serve", "devplane up --no-browser"),
    ("app", "devplane up --window"),
    ("quit", "devplane down"),
    ("answer", "devplane inbox answer <ask>"),
    ("snooze", "devplane inbox snooze <id>"),
    ("show", "devplane run show <run>"),
    ("watch", "devplane run watch [run]"),
    ("attach", "devplane run attach <run>"),
    ("forge", "devplane pr list, or devplane issue list"),
    ("speckit", "devplane gate install --speckit"),
    ("check", "devplane config check"),
    ("explain", "devplane config explain"),
    ("trust", "devplane project trust"),
    ("agents", "devplane agent list"),
    ("connect", "devplane agent connect <claude|codex|copilot>"),
    (
        "disconnect",
        "devplane agent disconnect <claude|codex|copilot>",
    ),
    ("login", "devplane auth login"),
    ("logout", "devplane auth logout"),
    ("diagnostics", "devplane doctor"),
];

/// The sentence for a command that was renamed, when `word` is one.
#[must_use]
pub fn renamed(word: &str) -> Option<String> {
    RENAMED
        .iter()
        .find(|(old, _)| *old == word)
        .map(|(old, new)| format!("`devplane {old}` is now `{new}`"))
}

/// The grouped command listing, which replaces clap's flat one.
///
/// Each description is the command's own `about`, read from a bare
/// `augment_subcommands` tree (`Cli::command()` would recurse through this), so
/// `--help` and `help <command>` cannot disagree. Plain text: it is often piped.
#[must_use]
pub fn groups_block() -> String {
    use clap::Subcommand;
    let tree = Command::augment_subcommands(clap::Command::new("devplane"));
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

/// What `devplane --help` says above the listing. The window is named only
/// in a build that has it.
#[cfg(feature = "app")]
const LONG_ABOUT: &str = "Devplane — know when an agent's work is actually done. Run any ACP \
     agent in its own worktree, verify its change with your own checks run outside the agent, \
     see the weakened tests first, and keep the record of who decided what while you were not \
     looking.\n\n\
     Start with `devplane up` (or `devplane up --window`), then \
     `devplane change start \"<what to do>\"`.";
#[cfg(not(feature = "app"))]
const LONG_ABOUT: &str = "Devplane — know when an agent's work is actually done. Run any ACP \
     agent in its own worktree, verify its change with your own checks run outside the agent, \
     see the weakened tests first, and keep the record of who decided what while you were not \
     looking.\n\n\
     Start with `devplane up`, then `devplane change start \"<what to do>\"`.";

#[derive(Parser)]
#[command(
    name = "devplane",
    version,
    about = "Know when an agent's work is actually done",
    long_about = LONG_ABOUT,
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
    /// Print machine-readable JSON instead of text
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show what is happening: sessions in play, and anything asking for you
    ///
    /// Bare `devplane` runs this.
    #[command(visible_alias = "ls")]
    Status {
        /// Include sessions that have never reported anything (idle editor tabs)
        #[arg(long, short)]
        all: bool,
        /// Only this project. Matches any part of the name: `mat` finds `matter-kit`
        #[arg(long, short)]
        project: Option<String>,
        /// Only sessions that are waiting on a human
        #[arg(long = "needs-you")]
        needs_you: bool,
        /// Which projects are deciding without you: each live session's
        /// permission mode, least-supervised first, and the question clock
        #[arg(long, conflicts_with_all = ["all", "project", "needs_you"])]
        modes: bool,
    },
    /// Read one run: its detail, its conversation, or its terminal
    Run {
        #[command(subcommand)]
        what: RunCmd,
    },
    /// Search tool calls, questions and errors across every session
    Search {
        /// Words to find in commands, questions and errors
        query: String,
    },
    /// Show what needs a human, most urgent first
    ///
    /// Filters are never remembered between runs, and a narrowed list always
    /// says how many items it is not showing. `--all` lists everything an
    /// agent has asked you instead, open ones first, and what ended each: you,
    /// a clock your project set, or nobody.
    #[command(args_conflicts_with_subcommands = true)]
    Inbox {
        #[command(subcommand)]
        what: Option<InboxCmd>,
        /// Only this project. Matches any part of the name, as `status --project` does
        #[arg(long, short)]
        project: Option<String>,
        /// Only what can be answered from here (a question, not a red gate)
        #[arg(long = "needs-you")]
        needs_you: bool,
        /// Every question and permission an agent asked, settled ones too
        #[arg(long, conflicts_with_all = ["project", "needs_you"])]
        all: bool,
    },
    /// Show what Devplane decided, and on whose authority
    ///
    /// Why a command ran without anybody being asked, and why a branch has a
    /// pull request.
    Audit {
        /// Narrow to one run or one change (a unique prefix is enough)
        about: Option<String>,
        /// Only what was decided instead of you: by a rule, a clock, or nobody
        #[arg(long = "without-me")]
        without_me: bool,
        /// How many of the most recent rows to show
        #[arg(long, default_value_t = 50)]
        limit: i64,
        /// Write the rows as OpenTelemetry GenAI log records instead
        ///
        /// `gen_ai.tool.call.decision` events as OTLP/JSON on stdout, with the
        /// deciding authority under an application prefix (absent when
        /// unknown). Nothing is sent anywhere.
        #[arg(long)]
        otel: bool,
    },
    /// List open pull requests across every registered project
    Pr {
        #[command(subcommand)]
        what: PrCmd,
    },
    /// List open issues across every registered project, or those ready for work
    Issue {
        #[command(subcommand)]
        what: IssueCmd,
    },
    /// Start a change and steer it: its own checkout, an agent, your gates
    ///
    /// An isolated checkout, an agent in it, and the project's gates when the
    /// agent says it is finished.
    Change {
        #[command(subcommand)]
        what: ChangeCmd,
    },
    /// Run this repository's own gates and report what they exited with
    ///
    /// Decides on exit codes alone. Exits 0 only when the checks passed; no
    /// declared checks, or a config that will not parse, exits non-zero.
    #[command(args_conflicts_with_subcommands = true)]
    Gate {
        #[command(subcommand)]
        what: Option<GateCmd>,
        /// Which repository. Defaults to the working directory
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// One named gate from `devplane.toml` instead of the whole `check`.
        /// A named gate never makes a change verified
        #[arg(long)]
        name: Option<String>,
    },
    /// Run the base's `check` on this change, leading with the checks it weakened
    ///
    /// Compares the checkout with its merge base against `--base`, and runs the
    /// `check` declared in `devplane.toml` **at that merge base** — never the
    /// checkout's own. Needs only git: no host, store or network. Writes the
    /// Markdown summary to `--summary`, else appends it to
    /// `$GITHUB_STEP_SUMMARY` when set. `--json` prints one JSON document and
    /// nothing else on stdout.
    ///
    /// Exit codes: 0 verified (every command exited 0 and the tree stayed put);
    /// 1 failed (a command failed, timed out or never started, or the tree
    /// moved); 2 could not run (no base, no shared history, no `check` at the
    /// base, an unreadable diff, or `check` passed and its evidence could not be
    /// written). Weakened checks never change the code.
    #[command(verbatim_doc_comment)]
    Verify(verify::VerifyArgs),
    /// Decide what Devplane may do in a repository
    Project {
        #[command(subcommand)]
        what: ProjectCmd,
    },
    /// Read this repository's devplane.toml, or ask what its rules decide
    Config {
        #[command(subcommand)]
        what: ConfigCmd,
    },
    /// The agents Devplane can drive, and its hooks in each vendor's settings
    Agent {
        #[command(subcommand)]
        what: AgentCmd,
    },
    /// Sign in to GitHub, or out of it
    Auth {
        #[command(subcommand)]
        what: AuthCmd,
    },
    /// Channel health, latency, whether a host answers, and each GitHub
    /// host's sign-in
    Doctor,
    /// Write a shell completion script
    ///
    ///   devplane completions zsh  > ~/.zsh/completions/_devplane
    ///   devplane completions bash > /etc/bash_completion.d/devplane
    ///   devplane completions fish > ~/.config/fish/completions/devplane.fish
    ///
    /// Ids (questions, sessions, changes) complete from a running host or the
    /// store, never starting one; zsh and fish show what each id is.
    #[command(verbatim_doc_comment)]
    Completions {
        /// bash, zsh or fish
        shell: String,
    },
    /// Start the host and open the workbench
    ///
    /// With a host already running, only opens the workbench. Otherwise this
    /// process becomes the host until ctrl-c.
    Up {
        /// The port to bind. Defaults to 47831; with `--window`, to `[app] port`
        /// in `~/.devplane/app.toml` where that is set
        #[arg(long, env = "DEVPLANE_PORT")]
        port: Option<u16>,
        /// Host in the foreground without opening a browser
        #[arg(long, conflicts_with = "window")]
        no_browser: bool,
        /// Open the window instead of a browser: the same host, with a tray
        /// item, notifications, one global shortcut and links that open the
        /// right change. Closing the window keeps hosting; quitting from the
        /// tray says what it stops. Nothing starts at login
        #[cfg_attr(
            not(feature = "app"),
            arg(
                hide = true,
                help = "Not in this build: built without the app feature (cargo install devplane --features app)"
            )
        )]
        #[arg(long)]
        window: bool,
    },
    /// Stop the running host, saying what that ends before it ends it
    Down,
    /// Serve Devplane's read-only surface to an agent over MCP, on stdio
    ///
    /// Four tools — `inbox`, `change`, `explain`, `audit` — none of
    /// which mutates. Register it as a `command` MCP server running
    /// `devplane mcp`. Hidden: an agent runs it, not a person.
    #[command(hide = true)]
    Mcp,
    /// Read a hook payload on stdin, decide it here and write it to the store
    ///
    /// What a vendor runs on every hook event; `devplane agent connect` installs it.
    #[command(hide = true)]
    Hook {
        /// Answer this provider's permission hook (Copilot's payload shape)
        #[arg(long, value_name = "PROVIDER")]
        gate: Option<String>,
        /// Record this provider's observation hook rather than deciding it
        #[arg(long, value_name = "PROVIDER")]
        observe: Option<String>,
        #[arg(long, value_name = "EVENT")]
        event: Option<String>,
        /// The vendor, for one speaking Claude Code's payloads verbatim (Codex)
        #[arg(long, value_name = "VENDOR")]
        vendor: Option<String>,
    },
    /// Record a status-line payload from stdin, then run the command that was
    /// there before
    ///
    /// Called by the shim `devplane agent connect claude --statusline` writes.
    #[command(hide = true)]
    Statusline {
        /// The user's original status-line command, run after forwarding
        #[arg(long)]
        then: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum RunCmd {
    /// List the sessions in play, grouped by project
    #[command(visible_alias = "ls")]
    List {
        /// Include sessions that have never reported anything (idle editor tabs)
        #[arg(long, short)]
        all: bool,
        /// Only this project. Matches any part of the name
        #[arg(long, short)]
        project: Option<String>,
        /// Only sessions that are waiting on a human
        #[arg(long = "needs-you")]
        needs_you: bool,
    },
    /// Show one run in detail
    Show {
        /// The run, or a unique prefix of it, as `devplane status` prints it
        run: String,
    },
    /// Follow events as they arrive, or one driven run's conversation
    ///
    /// Given a run Devplane drives, prints its conversation like `tail -f`. For
    /// a session you started yourself, `devplane run attach` resumes it in a
    /// terminal.
    Watch {
        /// One run to follow, or a unique prefix of it
        run: Option<String>,
        /// Include the agent's reasoning, where it streams any
        #[arg(long, requires = "run")]
        thinking: bool,
        /// How much of the conversation so far to print first
        #[arg(long, default_value_t = 40, requires = "run")]
        history: i64,
    },
    /// Attach a terminal to a run, resuming its session
    Attach {
        /// The run, or a unique prefix of it, as `devplane status` prints it
        run: String,
    },
}

#[derive(Subcommand)]
pub enum InboxCmd {
    /// Answer something an agent asked you — a permission or a question
    ///
    /// The id is the one `devplane inbox` prints, not a session id: it outlives
    /// the asking process, so a late answer reaches the agent through a resumed
    /// session. There is no dismiss.
    Answer {
        /// The ask, or a unique prefix of it, from `devplane inbox`
        ask: String,
        /// Allow it — for a permission
        #[arg(long, conflicts_with_all = ["deny", "custom", "option", "field"])]
        allow: bool,
        /// Refuse it — for a permission
        #[arg(long, conflicts_with_all = ["allow", "custom", "option", "field"])]
        deny: bool,
        /// An exact option the agent offered, as it wrote it. Says the whole
        /// answer, so it cannot be combined with `--allow` or `--deny`
        #[arg(long)]
        option: Option<String>,
        /// Your own words, where the agent offered an "Other" box. Wins over `--option`
        #[arg(long)]
        custom: Option<String>,
        /// Which question, when the agent asked several at once
        #[arg(long)]
        field: Option<String>,
    },
    /// Hide a run's or a change's inbox items for a while
    Snooze {
        /// A run, or a change (`c-…`), or a unique prefix of either
        id: String,
        /// Minutes to stay quiet. `0` un-snoozes
        #[arg(long, default_value_t = 60)]
        minutes: i64,
        /// Stay quiet until the run or change next reports activity, rather
        /// than for minutes
        #[arg(long, conflicts_with = "minutes")]
        until_activity: bool,
    },
}

#[derive(Subcommand)]
pub enum PrCmd {
    /// Every open pull request, what needs you first
    #[command(visible_alias = "ls")]
    List,
}

#[derive(Subcommand)]
pub enum IssueCmd {
    /// Every open issue, what needs you first
    ///
    /// `--ready` narrows it to one repository's issues carrying
    /// `[github].ready_label`: the list `change start --issue` picks from.
    #[command(visible_alias = "ls")]
    List {
        /// Only the issues this repository offers as work
        #[arg(long)]
        ready: bool,
        /// The repository to ask. Defaults to the working directory. Implies `--ready`
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Only issues with this label. Defaults to `[github].ready_label`. Implies `--ready`
        #[arg(long)]
        label: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum GateCmd {
    /// Register the gate where a workflow runs it: a Spec Kit extension hook
    ///
    /// Spec Kit runs hooks from `.specify/extensions.yml`; this one runs the
    /// project's gates and reports what they exited with. Refused while the
    /// repository declares no gate, since the hook would have nothing to run.
    Install {
        /// Register it with Spec Kit, the one workflow this installs into
        #[arg(long, required = true)]
        speckit: bool,
        /// Which hook point. Defaults to `after_implement`
        #[arg(long)]
        event: Option<String>,
        /// Print and write nothing
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Write the hook even where the repository declares no gate to run,
        /// or the agent cannot reach the skill
        #[arg(long)]
        anyway: bool,
    },
}

#[derive(Subcommand)]
pub enum ProjectCmd {
    /// Allow Devplane to start agents in a repository
    ///
    /// A headless agent runs the repository's own hooks and MCP servers without
    /// asking, so this prints what those are first. `--dry-run` prints them and
    /// trusts nothing.
    Trust {
        /// The repository to trust. Defaults to the working directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Trust without asking. For scripts and for a directory you wrote
        #[arg(long, short = 'y')]
        yes: bool,
        /// Print what is there and trust nothing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Read this repository's devplane.toml and say what it will do
    ///
    /// Does it parse, does everything it names exist, and is anything unsafe.
    Check {
        /// The repository to read. Defaults to the working directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Ask the gate what it would decide about one call, and why
    ///
    /// Offline: reads the rules governing a directory, so a rule can be tested
    /// before it is committed.
    ///
    ///   devplane config explain 'pnpm test && rm -rf /'
    ///   devplane config explain --tool Read .env
    ///   devplane config explain --tool Agent --input '{"isolation":"worktree"}'
    ///   devplane config explain --replay
    ///
    /// `--replay` asks it of every observed call and names the rule that would
    /// stop the interruptions.
    #[command(verbatim_doc_comment)]
    Explain {
        /// The call, for a tool with a plain specifier: a command for `Bash`,
        /// a path for `Read` and `Edit`, a URL for `WebFetch`
        #[arg(trailing_var_arg = true)]
        call: Vec<String>,
        /// The tool being called: `Bash`, `Read`, `Edit`, `WebFetch`, …
        #[arg(long, default_value = "Bash")]
        tool: String,
        /// The whole tool input as JSON, for a call a specifier cannot express
        #[arg(long)]
        input: Option<String>,
        /// The directory the agent would work in, which decides whose rules apply
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Replay every tool call already observed against the current rules,
        /// and say which rule would answer the ones that reached you
        #[arg(long, conflicts_with_all = ["call", "input"])]
        replay: bool,
        /// How many of the most recent calls to replay
        #[arg(long, default_value_t = 5000)]
        limit: i64,
    },
}

#[derive(Subcommand)]
pub enum AgentCmd {
    /// List the agents Devplane can drive
    #[command(visible_alias = "ls")]
    List,
    /// Install Devplane's hooks into a vendor's own settings
    Connect {
        /// Which vendor
        vendor: Vendor,
        /// Also wrap the status line, the only source of subscription rate
        /// limits. Off by default: it touches a command you configured
        #[arg(long)]
        statusline: bool,
        /// Write the shown changes without asking. Required when stdin is
        /// not a terminal, and under `--json`
        #[arg(short = 'y', long)]
        yes: bool,
        /// The devplane binary the hooks run. Defaults to this one, which is
        /// refused when it lives somewhere temporary (npx's cache, a mounted
        /// disk image)
        #[arg(long, value_name = "PATH")]
        bin: Option<PathBuf>,
    },
    /// Remove everything `agent connect` installed
    Disconnect {
        /// Which vendor
        vendor: Vendor,
        /// Write the shown changes without asking
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum AuthCmd {
    /// Sign in to GitHub: a code to enter at GitHub's device page, and the
    /// token kept only in the OS credential store
    Login {
        /// A GitHub Enterprise host, instead of the configured one
        #[arg(long)]
        host: Option<String>,
        /// Read a token from stdin instead of the device flow — never from
        /// an argument. For a host with no registered app, and for CI
        #[arg(long)]
        with_token: bool,
    },
    /// Sign out of GitHub: the stored token is deleted, and where to revoke
    /// the grant at GitHub is said
    Logout {
        /// A GitHub Enterprise host, instead of the configured one
        #[arg(long)]
        host: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ChangeCmd {
    /// Begin a new change — in this repository, or the same prompt in several
    ///
    /// Every `--project` is checked first (trusted, clean, config readable);
    /// one refusal starts none of them.
    Start {
        /// What to do. Becomes the branch name and the first prompt
        title: Vec<String>,
        /// Which agent to start, as `devplane agent list` lists it. Defaults to
        /// the project's `[agent] default`
        #[arg(long)]
        agent: Option<String>,
        /// A project to start it in: a registered name or a path. Repeatable,
        /// to send one prompt to several. Defaults to the current directory
        #[arg(long = "project", value_name = "NAME|PATH")]
        project: Vec<String>,
        /// Work in the repository itself rather than an isolated checkout
        #[arg(long)]
        no_worktree: bool,
        /// Start from a GitHub issue. Its title and body become the change
        #[arg(long)]
        issue: Option<u64>,
        /// The specification this change answers — a file or folder, relative
        /// to the repository. Stamped onto the done certificate with its task
        /// list's `- [ ]` progress at each gate run
        #[arg(long)]
        spec: Option<String>,
        /// Which tasks of `--spec` to send, repeatable: a requirement token
        /// (every task line citing it) or `file:line`. A selector matching
        /// nothing refuses
        #[arg(long = "task", requires = "spec")]
        task: Vec<String>,
    },
    /// Show every change
    #[command(visible_alias = "ls")]
    List,
    /// Show one change: its state, its runs, and what its checks said
    Show {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
    },
    /// Read a change in review order
    ///
    /// Files ordered by `[review] roles`, each with its role, test coverage,
    /// decisions and task, and the gate standing. `--by intent` groups files by
    /// the run that wrote them and its tasks.
    ///
    /// `--seen <path>` marks every current *checks weakened or changed* row at
    /// that path read, as yours; an offer waits until every such row is.
    Review {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
        #[arg(long, value_parser = ["risk", "intent"], default_value = "risk")]
        by: String,
        /// Mark the weakened rows at this path seen (repeatable)
        #[arg(long, value_name = "PATH")]
        seen: Vec<String>,
    },
    /// Run the project's gates now
    Verify {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
        /// Then, if `check` passed, the base checks: the base's tests on this
        /// change and its new tests on the base. The exit code stays `check`'s.
        #[arg(long)]
        base_checks: bool,
    },
    /// Hand the failures back to the agent once more, past the project's bound
    Retry {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
    },
    /// Pick a change back up after a restart, against the same agent session
    ///
    /// Reconnects to the conversation the agent kept rather than starting anew.
    Resume {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
    },
    /// Send a message to a change's agent without stopping it
    ///
    /// Mid-turn, it is queued until the turn ends.
    Prompt {
        /// A change — its latest run is the one told — or a run, or a unique
        /// prefix of either
        run: String,
        text: Vec<String>,
    },
    /// Stop a run, saying first what survives
    ///
    /// Asks on a terminal; anywhere else it needs `--yes`.
    Stop {
        /// A change — its latest run is the one stopped — or a run, or a
        /// unique prefix of either
        run: String,
        /// Stop without asking. Required when stdin is not a terminal
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Decide what to do about a specification that moved under a run
    ///
    /// `--tell` prompts the run with the changed files (resuming it if needed);
    /// `--accept` moves the change's start to what the run saw.
    Drift {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
        /// The run the drift names
        #[arg(long)]
        run: String,
        /// Accept the drift: move the change's start to what the run saw
        #[arg(long, conflicts_with = "tell")]
        accept: bool,
        /// Tell the run which specification files changed
        #[arg(long)]
        tell: bool,
    },
    /// Take a branch somebody made by hand and make it a change
    ///
    /// The title defaults to the first commit's subject; the worktree is an
    /// existing checkout of the branch or a new one under `.claude/worktrees/`.
    Adopt {
        branch: String,
        /// The repository root. Defaults to the current directory's
        #[arg(long)]
        project: Option<PathBuf>,
        /// The specification this change answers, relative to the repository
        #[arg(long)]
        spec: Option<String>,
        /// The change's title. Defaults to the first commit's subject
        #[arg(long)]
        title: Option<String>,
    },
    /// Commit the verified tree, push the branch and open the pull request
    ///
    /// Writes one commit whose tree is exactly the one the last passing check
    /// ran against, and refuses when the tree has moved since. When `[github]
    /// pull_request` does not say Devplane may push, prints the push command
    /// and the address that opens the pull request in a browser instead.
    Offer {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
    },
    /// Accept a change as finished: records the basis and removes nothing
    Finish {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
    },
    /// Remove the worktree and keep the record
    ///
    /// Refuses uncommitted or unmerged work unless forced. The branch is kept
    /// unless asked, and deleted only when its base has every commit.
    Archive {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
        /// Delete the branch too. Refused while it has commits its base does
        /// not, unless its pull request merged
        #[arg(long)]
        delete_branch: bool,
        /// Remove the worktree even with uncommitted or untracked files in it
        #[arg(long)]
        discard_uncommitted: bool,
        /// With --delete-branch: delete it even with unmerged commits
        #[arg(long, requires = "delete_branch")]
        force: bool,
    },
    /// Mark the agent's open stop claim read
    ///
    /// The claim leaves what asks for attention and stays on the change under
    /// its run. Refused inside an agent's session.
    ClaimRead {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
    },
    /// Print the done certificate: what was checked, against which commit,
    /// and how to check it yourself
    ///
    /// Meant for a pull request: a reviewer can check out the tree and run
    /// the commands. `--json` gives the in-toto statement (Statement v1, Test
    /// Result predicate) instead; `--sign ssh` signs that statement with your
    /// own SSH key. Devplane never signs and never makes a key.
    Export {
        /// The change, or a unique prefix of its id, as `devplane change list` prints it
        change: String,
        /// Write to this file instead of printing; refused if it exists
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Replace the `--out` file (and its `.sig`) if it exists
        #[arg(long)]
        force: bool,
        /// Sign the in-toto statement with the key your global git
        /// `user.signingkey` names, by `ssh-keygen -Y sign`; writes
        /// `<out>.sig` beside it and prints how anyone checks it
        #[arg(long, value_enum, requires = "out")]
        sign: Option<Signer>,
    },
}

/// Who signs an exported statement. Only the person, with SSH.
#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signer {
    /// `ssh-keygen -Y sign` with your global `user.signingkey`
    Ssh,
}

/// The vendors whose own settings `agent connect` writes to.
#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vendor {
    /// Claude Code, through its user-scope settings
    Claude,
    /// Codex, through `~/.codex/hooks.json`. The entries wait for your
    /// approval in Codex's own dialog
    Codex,
    /// GitHub Copilot, through one file in `~/.copilot/hooks/`. Telemetry is
    /// not installed: Copilot reads it from the environment
    Copilot,
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
    let json = cli.json;
    let Some(command) = cli.command else {
        return board::cmd_status(board::Filter::default(), json).await;
    };
    match command {
        Command::Status { modes: true, .. } => inbox::cmd_modes(json).await,
        Command::Status {
            all,
            project,
            needs_you,
            ..
        } => {
            board::cmd_status(
                board::Filter {
                    all,
                    project,
                    needs_you,
                },
                json,
            )
            .await
        }
        Command::Run { what } => match what {
            RunCmd::List {
                all,
                project,
                needs_you,
            } => {
                board::cmd_run_list(
                    board::Filter {
                        all,
                        project,
                        needs_you,
                    },
                    json,
                )
                .await
            }
            RunCmd::Show { run } => board::cmd_show(&run, json).await,
            RunCmd::Watch {
                run,
                thinking,
                history,
            } => board::cmd_watch(run.as_deref(), thinking, history).await,
            RunCmd::Attach { run } => board::cmd_attach(&run).await,
        },
        Command::Search { query } => admin::cmd_search(&query, json).await,
        Command::Inbox {
            what:
                Some(InboxCmd::Answer {
                    ask,
                    allow,
                    deny,
                    option,
                    custom,
                    field,
                }),
            ..
        } => inbox::cmd_answer(&ask, allow, deny, option, custom, field, json).await,
        Command::Inbox {
            what:
                Some(InboxCmd::Snooze {
                    id,
                    minutes,
                    until_activity,
                }),
            ..
        } => inbox::cmd_snooze(&id, minutes, until_activity, json).await,
        Command::Inbox { all: true, .. } => inbox::cmd_inbox_all(json).await,
        Command::Inbox {
            project, needs_you, ..
        } => inbox::cmd_inbox(json, project.as_deref(), needs_you).await,
        Command::Audit {
            about,
            without_me,
            limit,
            otel,
        } => admin::cmd_audit(about.as_deref(), without_me, limit, json, otel).await,
        Command::Pr { what: PrCmd::List } => forge::cmd_prs(json).await,
        Command::Issue {
            what: IssueCmd::List { ready, cwd, label },
        } => match ready || cwd.is_some() || label.is_some() {
            true => forge::cmd_ready_issues(cwd, label, json).await,
            false => forge::cmd_issues(json).await,
        },
        Command::Change { what } => change::cmd_change(what, json).await,
        Command::Gate {
            what:
                Some(GateCmd::Install {
                    event,
                    dry_run,
                    anyway,
                    ..
                }),
            ..
        } => gate::cmd_speckit_install(event, dry_run, anyway),
        Command::Gate { cwd, name, .. } => gate::cmd_gate_run(cwd, name, json).await,
        Command::Verify(args) => verify::cmd_verify(args, json).await,
        Command::Project {
            what: ProjectCmd::Trust { path, yes, dry_run },
        } => project::cmd_trust(path, yes, dry_run, json).await,
        Command::Config { what } => match what {
            ConfigCmd::Check { path } => conf::cmd_check(path, json).await,
            ConfigCmd::Explain {
                replay: true,
                dir,
                limit,
                ..
            } => conf::cmd_replay(dir, limit, json).await,
            ConfigCmd::Explain {
                call,
                tool,
                input,
                dir,
                ..
            } => conf::cmd_explain(dir, tool, call, input, json),
        },
        Command::Agent { what } => match what {
            AgentCmd::List => agent::cmd_agents(json).await,
            AgentCmd::Connect {
                vendor,
                statusline,
                yes,
                bin,
            } => agent::cmd_connect(vendor, statusline, yes, json, bin).await,
            AgentCmd::Disconnect { vendor, yes } => agent::cmd_disconnect(vendor, yes, json).await,
        },
        Command::Auth { what } => match what {
            AuthCmd::Login { host, with_token } => github::cmd_login(host, with_token, json).await,
            AuthCmd::Logout { host } => github::cmd_logout(host, json).await,
        },
        Command::Doctor => doctor::cmd_doctor(json).await,
        Command::Completions { shell } => completions::cmd_completions(&shell),
        Command::Up {
            port,
            no_browser,
            window,
        } => cmd_up(port, no_browser, window).await,
        Command::Down => cmd_down(json).await,
        Command::Mcp => crate::mcp::Server::run().await,
        Command::Hook {
            gate,
            observe,
            event,
            vendor,
        } => crate::hook::run(gate, observe, event, vendor).await,
        Command::Statusline { then } => crate::hook::statusline(then).await,
    }
}

/// `devplane up`: the workbench, hosting here when nothing is running.
async fn cmd_up(port: Option<u16>, no_browser: bool, window: bool) -> Result<()> {
    if window {
        #[cfg(feature = "app")]
        return crate::app::run(port).await;
        #[cfg(not(feature = "app"))]
        anyhow::bail!("{NO_WINDOW}");
    }
    let port = port.unwrap_or(config::DEFAULT_PORT);
    match no_browser {
        true => cmd_serve(port).await,
        false => board::cmd_open(port).await,
    }
}

// ---------------------------------------------------------------------------

pub(crate) async fn cmd_serve(port: u16) -> Result<()> {
    init_tracing();
    // Held until this function returns — the host's whole life.
    let _lock = refuse_if_hosting().await?;
    let state = boot_state().await?;
    // A debug build can be handed the window's key, so the UX bars can drive
    // the window's page in a browser and trust from it as the app's webview
    // does. A release build never reads it: there, only the app holds a key.
    #[cfg(debug_assertions)]
    if let Some(key) = std::env::var("DEVPLANE_WINDOW_KEY")
        .ok()
        .filter(|k| k.len() >= 32)
    {
        let _ = state.window_key.set(key);
    }
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
                "a host is already running (pid {}, port {}, up {}). Stop it with `devplane down`.",
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
            paint(
                DIM,
                "devplane status --all lists every session, ids included."
            )
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
async fn cmd_down(json: bool) -> Result<()> {
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

#[cfg(test)]
mod launch_tests {
    use super::{Launch, launch_of};

    #[test]
    fn a_lone_devplane_link_is_a_link_launch() {
        assert_eq!(
            launch_of(["devplane", "devplane://change/abc"]),
            Launch::Link("devplane://change/abc".into())
        );
        assert_eq!(
            launch_of(["devplane", "DEVPLANE://inbox"]),
            Launch::Link("DEVPLANE://inbox".into())
        );
    }

    #[test]
    fn everything_else_is_the_command_line() {
        assert_eq!(launch_of(["devplane"]), Launch::Cli);
        assert_eq!(launch_of(["devplane", "status"]), Launch::Cli);
        assert_eq!(launch_of(["devplane", "up", "--window"]), Launch::Cli);
        assert_eq!(launch_of(["devplane", "https://example.com"]), Launch::Cli);
        assert_eq!(launch_of(["devplane", "devplane:"]), Launch::Cli);
        // A link among other arguments is not how the system launches it.
        assert_eq!(
            launch_of(["devplane", "devplane://change/abc", "--json"]),
            Launch::Cli
        );
        assert_eq!(launch_of(Vec::<String>::new()), Launch::Cli);
    }

    /// The operating system starts the app with no argument and no terminal;
    /// a person typing `devplane` has a terminal and gets `status`.
    #[test]
    fn only_a_launch_with_no_argument_and_no_terminal_opens_the_window() {
        use super::opens_window;
        assert!(opens_window(&["devplane"], false));
        assert!(opens_window(&Vec::<String>::new(), false));
        assert!(!opens_window(&["devplane"], true));
        assert!(!opens_window(&["devplane", "status"], false));
        assert!(!opens_window(&["devplane", "doctor", "--json"], false));
    }
}

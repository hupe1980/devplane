//! The command line: one module per thing a person is trying to do.
//!
//! This lives in the library rather than beside `main` for the reason the crate
//! docs give: a seam that can only be exercised through a subprocess is a seam
//! nobody exercises. `main.rs` is the argument parser it claims to be — it
//! parses and calls [`crate::cli::run`].

use crate::render::{DIM, paint};
use crate::{client, config, daemon, poller};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod admin;
mod batch;
mod board;
pub mod completions;

/// The argument the completion scripts call back with.
///
/// **Not a clap subcommand**, deliberately: `hide = true` keeps a command off
/// the help screen and not out of `clap_complete`'s output, so a hidden one was
/// offered in every generated script. `main` answers this before clap parses,
/// and a command clap does not know about cannot leak into what clap generates.
pub const COMPLETE_ARG: &str = "__complete";
mod inbox;
mod library;
mod rules;
mod work;

use admin::{
    cmd_agents, cmd_audit, cmd_connect, cmd_diagnostics, cmd_disconnect, cmd_rewind, cmd_search,
};
use board::{cmd_attach, cmd_focus, cmd_ls, cmd_open, cmd_show, cmd_tail, cmd_watch};
use inbox::{cmd_answer, cmd_asks, cmd_attention, cmd_inbox, cmd_say, cmd_snooze};
use work::{cmd_check, cmd_dispatch, cmd_gate_run, cmd_speckit_install, cmd_trust, cmd_work};

/// The five errands, and every non-hidden subcommand assigned to exactly one.
///
/// **Errands rather than categories.** A stranger arriving at a thirty-six-row
/// flat list is reading an inventory; these are the five reasons somebody opens
/// this binary at all, and the command they want is under one of them.
///
/// This is the one place the grouping is decided. `site/content/docs/cli.md`
/// carries a copy for people who never run `--help`, and a test holds the two
/// together — they were two hand-maintained lists once and nothing compared
/// them.
pub const COMMAND_GROUPS: &[(&str, &[&str])] = &[
    (
        "See what is happening",
        &["ls", "show", "tail", "watch", "search", "open"],
    ),
    (
        "What needs you, and what happened without you",
        &[
            "inbox",
            "asks",
            "answer",
            "attention",
            "audit",
            "modes",
            "issues",
            "prs",
            "snooze",
        ],
    ),
    (
        "Start and steer work",
        &[
            "work", "dispatch", "batch", "say", "attach", "focus", "gate", "rewind", "library",
        ],
    ),
    (
        "Set up a project",
        &[
            "connect",
            "disconnect",
            "trust",
            "check",
            "explain",
            "rules",
            "speckit",
            "agents",
            "doctor",
            "completions",
        ],
    ),
    ("The daemon", &["serve", "stop"]),
];

/// The grouped listing — **the only listing**.
///
/// **It printed under clap's flat one for the life of the feature.** The
/// specification asks that the commands be *presented* under five names; what
/// shipped was thirty-five in a flat list and then the same thirty-five in
/// groups, which is the problem this feature exists to fix, twice, on one
/// screen. The doc comment here even said *"printed above clap's own listing"*
/// while `after_help` prints below it. Nothing caught it, because the test
/// asserts the groups are present and a screen nobody reads can carry both.
///
/// `help_template` now drops `{subcommands}`, so this is what a reader sees.
///
/// **The descriptions come from clap and are not a second copy.** Each line is
/// the command's own `about`, read back out of the augmented command tree, so
/// `devplane --help` and `devplane help <command>` cannot disagree. Built with
/// `augment_subcommands` on a bare command rather than `Cli::command()`, which
/// would recurse through this function.
///
/// Plain text with no colour: this is the surface most likely to be piped into
/// a file or a pager, and colour may never be the only thing that carries a
/// distinction.
#[must_use]
pub fn groups_block() -> String {
    use clap::Subcommand;
    let tree = Command::augment_subcommands(clap::Command::new("devplane"));
    let about = |name: &str| -> String {
        tree.get_subcommands()
            .find(|c| c.get_name() == name)
            .and_then(|c| c.get_about().map(ToString::to_string))
            .unwrap_or_default()
    };

    // One column width across every group, so the descriptions line up down the
    // whole screen rather than per block.
    let widest = COMMAND_GROUPS
        .iter()
        .flat_map(|(_, cs)| cs.iter())
        .map(|c| c.len())
        .max()
        .unwrap_or(0);

    let mut out = String::from("What you came here to do:\n");
    for (name, commands) in COMMAND_GROUPS {
        out.push_str(&format!("\n  {name}\n"));
        for c in *commands {
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
    about = "Records who decided, when nobody asked you",
    long_about = "Devplane records who decided, when nobody asked you — a person, a rule, a \
                  classifier, a timer, or nobody — across every project and every coding agent on \
                  this machine. One page for what needs you, what went red after the agent \
                  stopped, and which finished work can prove its checks passed.\n\n\
                  It watches the sessions already running, drives any agent that speaks the Agent \
                  Client Protocol, and never approves a tool call.\n\n\
                  Start with `devplane connect claude`, then `devplane ls`.",
    after_help = groups_block(),
    after_long_help = groups_block(),
    // **`{subcommands}` is deliberately absent, and `{all-args}` with it.**
    // Clap's flat list of thirty-five is the thing the grouped block replaces,
    // and printing both put the problem on the screen twice — which is what
    // shipped, because `after_help` renders *below* the listing it was written
    // to replace.
    //
    // `{options}` rather than `{all-args}`: the latter carries the subcommands
    // back in. And `{after-help}` is placed by hand so the grouped list sits
    // where the flat one used to, above the options, rather than after them.
    help_template = "\
{before-help}{about-with-newline}
{usage-heading} {usage}{after-help}

Options:
{options}"
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
        #[arg(long, env = "DEVPLANE_PORT", default_value_t = config::DEFAULT_PORT)]
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
    ///
    /// **A narrowing is a view, not a preference.** Nothing is remembered
    /// between runs: a filter that persists is one somebody forgets they set,
    /// and the next morning they are reading a subset of what needs them and do
    /// not know it. A narrowed list always says how many it is not showing.
    Inbox {
        /// Only this project. Matches on any part of the name, so `mat` finds
        /// `matter-kit` — the same rule `devplane ls --project` uses.
        #[arg(long, short)]
        project: Option<String>,
        /// Only what can be answered from here.
        ///
        /// *Has an answer path*, as `ls --needs-you` has it — not *a person is
        /// required*. A red gate needs somebody and cannot be answered from a
        /// list; a question with a reply can.
        #[arg(long = "needs-you")]
        needs_you: bool,
    },
    /// Every open GitHub issue across every registered project, what needs you first.
    ///
    /// `--ready` narrows it to one repository's issues that are *offered as
    /// work* — the ones carrying `[github].ready_label` — which is the list
    /// `devplane work start --issue` picks from.
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
    /// Every open pull request across every registered project, what needs you first.
    Prs,
    /// Show one run in detail.
    Show { run: String },
    /// Follow what a driven agent is saying, like `tail -f`.
    ///
    /// Only for runs Devplane drives: they have no window of their own, which
    /// is why this exists. A session you started in a terminal or an editor is
    /// already showing you its own transcript — use `devplane focus` to raise
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
    /// Which of a run's files the vendor's checkpoint will not bring back.
    ///
    /// Claude Code snapshots the files its own editing tools touch and
    /// `/rewind` restores them. Its documentation is explicit that files
    /// modified by bash commands are not tracked — and that is the one class
    /// the decision log has a complete record of.
    ///
    /// It says `named for writing`, never `changed`: the gate sees a call
    /// before the tool runs, so claiming the second would be a confident answer
    /// this evidence does not support.
    Rewind {
        /// The run, or a unique prefix of it, as `devplane ls` prints it.
        run: String,
    },
    /// Show what Devplane decided, and on whose authority.
    ///
    /// Answers the two questions the event log cannot: why a command ran
    /// without anybody being asked, and why there is a pull request on a branch.
    Audit {
        /// Narrow to one run or one piece of work.
        about: Option<String>,
        /// Only what was decided **instead of** you — a rule, a clock, or
        /// nobody. Your own answers and Devplane running a gate are left out,
        /// because those are the rows you already know about.
        #[arg(long = "without-me")]
        without_me: bool,
        #[arg(long, default_value_t = 50)]
        limit: i64,
        /// Write the rows as OpenTelemetry GenAI log records instead.
        ///
        /// `gen_ai.tool.call.decision`, the event
        /// `open-telemetry/semantic-conventions-genai` #535 proposes — **plus
        /// the attribute it leaves out**. That proposal's own note says the
        /// event is recorded when *"a framework, harness, application policy,
        /// or human approval flow"* decides, and none of its three attributes
        /// says which of the four it was.
        ///
        /// The authority rides under an application-specific prefix, because a
        /// producer may not mint a normative `gen_ai.*` name. An authority
        /// nobody can establish is **absent**, never defaulted.
        ///
        /// OTLP/JSON on standard output. Nothing is sent anywhere: Devplane
        /// receives telemetry and exports none of its own.
        #[arg(long)]
        otel: bool,
    },
    /// Write a shell completion script.
    ///
    /// Generated from this command tree, so a command that exists completes and
    /// a hidden one is not offered. **Needs no daemon**: it writes a script and
    /// talks to nothing.
    ///
    ///   devplane completions zsh  > ~/.zsh/completions/_devplane
    ///   devplane completions bash > /etc/bash_completion.d/devplane
    ///   devplane completions fish > ~/.config/fish/completions/devplane.fish
    ///
    /// Completing an id — a waiting question, a session, a project — asks the
    /// daemon and **is silent when there is none**, because pressing Tab must
    /// not start one. zsh and fish show the sentence beside each id; bash
    /// completes the id alone, which is all bash reads.
    Completions {
        /// bash, zsh or fish.
        shell: String,
    },
    /// Which of your repositories is missing a rule.
    ///
    /// The fleet half of *answer once*: `devplane explain` composes a rule for
    /// one call on one machine, and this asks the same question across every
    /// registered project. Both files are read and never conflated — a
    /// `devplane.toml` prohibition is what this product refuses, a
    /// `permissions.deny` entry is what the agent refuses, and a person with six
    /// repositories needs the second at least as much.
    ///
    /// **It writes nothing, and there is no apply-to-all.** Across 15 549
    /// agentic pull requests in 148 projects, adding instruction files helped in
    /// 27.7% and hurt in 26.35% — what separated them was what the rules said,
    /// not that they were there.
    ///
    /// With no rule, it reports what the projects disagree about.
    Rules {
        /// The rule, as you would write it: `Bash(curl:*)`, `Read(./.env)`.
        rule: Option<String>,
        /// Ask rather than deny, which changes the key the paste names.
        #[arg(long)]
        ask: bool,
    },
    /// Which projects are deciding without you, and what mode each is in.
    ///
    /// A person with six repositories cannot find this out from anything else
    /// on the machine. Live sessions only, least-supervised first; a session
    /// that has not reported a mode is shown as unknown rather than hidden,
    /// because the hook that fires on every tool call does not carry one.
    Modes,
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
    /// Start an agent on a project — or send one prompt to several.
    ///
    /// With `--to`, this is a fan-out: one intent, many repositories, one
    /// reviewable row. **Draft is chosen for you above three targets**, and no
    /// position merges.
    Dispatch {
        /// What to ask for.
        prompt: Vec<String>,
        /// Which agent: `claude`, `codex`, `opencode`, `gemini`, or a command.
        #[arg(long, default_value = "claude")]
        agent: String,
        /// Where it runs. Defaults to the current directory.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Project names, comma-separated. Turns this into a fan-out.
        #[arg(long, value_delimiter = ',')]
        to: Vec<String>,
        /// How far it may go without you: `draft`, `gate`, `pr`.
        ///
        /// Defaults to `draft`, and stays draft above three targets whatever
        /// you pass — six terminals holding a readable prompt is a better first
        /// version than six running agents.
        #[arg(long)]
        mode: Option<String>,
        /// A library artefact this fan-out starts from, by name.
        ///
        /// Recorded on the batch, and read for the one thing a fan-out can say
        /// about it that a single dispatch cannot: **which of its frontmatter
        /// fields the documented distribution paths reject**. A warning and
        /// never a refusal — the artefact still works in the tool that wrote
        /// it, and the documented error is about leaving it.
        #[arg(long)]
        template: Option<String>,
        /// Without this, the preflight prints and nothing is sent or opened.
        #[arg(long)]
        apply: bool,
    },
    /// A fan-out: one row, one outcome per target.
    ///
    /// **No aggregate.** Four green, one red and one asking a question is what
    /// a fan-out looks like; a percentage over that hides the one that needs
    /// you.
    Batch {
        /// One batch, or the most recent when omitted.
        id: Option<String>,
    },
    /// Send another prompt to a run Devplane drives.
    Say { run: String, prompt: Vec<String> },
    /// Answer something an agent asked you — a permission or a question.
    ///
    /// The id is the one `devplane inbox` prints, and it is **not** a session
    /// id: it outlives the process that asked, so an answer given tomorrow
    /// morning still reaches the agent, through a resumed session where the
    /// original one is gone.
    ///
    /// There is no way to dismiss one. An agent that asked and was told nothing
    /// proceeds on nothing, which is what this exists to prevent.
    Answer {
        /// The ask, from `devplane inbox`.
        ask: String,
        /// Allow it — for a permission.
        #[arg(long, conflicts_with_all = ["deny", "custom"])]
        allow: bool,
        /// Refuse it — for a permission. The default where neither is given,
        /// because a refusal is the safe end of the range.
        #[arg(long, conflicts_with_all = ["allow", "custom"])]
        deny: bool,
        /// An exact option the agent offered, as it wrote it.
        #[arg(long)]
        option: Option<String>,
        /// Your own words, where the agent offered an "Other" box. Wins over
        /// `--option`, which is the agent's own rule rather than ours.
        #[arg(long)]
        custom: Option<String>,
        /// Which question, when the agent asked several at once.
        #[arg(long)]
        field: Option<String>,
    },
    /// Run this repository's own gates and report what they exited with.
    ///
    /// The verdict half of `check`: that one says what the file will do, this
    /// one says what the commands in it just said. It decides on exit codes and
    /// nothing else — no specification is opened and no task list is parsed.
    ///
    /// Exits 0 only when the checks passed. A repository that declares none,
    /// and one whose configuration will not parse, both exit non-zero: a caller
    /// that reads "nothing was checked" as success is the failure this exists
    /// to prevent.
    Gate {
        #[command(subcommand)]
        what: GateCmd,
    },
    /// Register Devplane's gate as a Spec Kit extension hook.
    ///
    /// Spec Kit's commands look in `.specify/extensions.yml` for a hook to
    /// invoke and wait for. Devplane's runs this project's gates and reports
    /// what they said, which is the one thing that whole workflow has no way to
    /// do: its own analysers report and none of them decides.
    Speckit {
        #[command(subcommand)]
        what: SpeckitCmd,
    },
    /// Everything an agent has asked you, and what became of each one.
    ///
    /// Open ones first, oldest first among those — a queue of what is owed to
    /// you rather than a feed. Settled ones follow with the sentence that says
    /// what ended them: you, a clock your project set, or nobody.
    Asks,
    /// List the agents Devplane can drive.
    Agents,
    /// Read this repository's devplane.toml and say what it will do.
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
    ///   devplane explain 'pnpm test && rm -rf /'
    ///   devplane explain --tool Read .env
    ///   devplane explain --tool Agent --input '{"isolation":"worktree"}'
    ///   devplane explain --replay
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
    /// Allow Devplane to start agents in a repository.
    ///
    /// A headless agent runs that repository's own hooks and MCP servers
    /// without asking, so this is a deliberate act rather than a default — and
    /// it prints what those are before it asks. Answering a question about a
    /// directory you have not looked inside is a consent dialog, not a
    /// decision.
    ///
    /// `--dry-run` prints the same thing and trusts nothing, which is the form
    /// worth running on somebody else's repository before you clone it.
    Trust {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Trust without asking. For scripts and for a directory you wrote.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Print what is there and trust nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Start work: an isolated checkout, an agent in it, and the project's
    /// gates when the agent says it is finished.
    Work {
        #[command(subcommand)]
        what: WorkCmd,
    },
    /// Prompts and skills you reuse across projects.
    ///
    /// Five verbs over artefacts **in the vendors' own formats, unmodified**.
    /// Devplane owns the verbs and none of the nouns: nothing here invents a
    /// format, rewrites an artefact, or translates one vendor's fields into
    /// another's.
    //
    // This said "blueprints" and "four verbs" until 2026-09-20, and both were
    // wrong in the direction that costs the most: it advertised a noun this
    // command does not have — a blueprint carries a shell hook and needs its own
    // safety argument, which is why it is not in the feature — and then
    // miscounted the verbs it does have, with all five listed underneath. The
    // help text is the first page anybody reads.
    Library {
        #[command(subcommand)]
        what: LibraryCmd,
    },
    /// Hide a run's or a piece of work's inbox items for a while.
    Snooze {
        /// A run id, or a work id from `devplane inbox`.
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
    ///
    /// Named `doctor` because that is what every page of the documentation,
    /// every quickstart and every error message in this product already called
    /// it while the command was spelled `diagnostics`. The canonical name being
    /// the one nobody writes is a small thing that costs somebody a search
    /// every time.
    #[command(visible_alias = "diagnostics")]
    Doctor,
    /// Install Devplane's hooks into a provider.
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
    /// Serve Devplane's read-only surface to an agent over MCP, on stdio.
    ///
    /// Four questions — `inbox`, `work`, `explain`, `audit` — and nothing that
    /// acts. The surface is read-only because it implements no mutating tool,
    /// which is a property of the code rather than of a `readOnlyHint` a client
    /// may ignore.
    ///
    /// Register it with your agent as a `command` MCP server running
    /// `devplane mcp`.
    ///
    /// **Hidden**: an agent runs this, not a person, and a listing a person
    /// reads is shorter and truer without it. It is documented on the site and
    /// in `llms.txt`, where whoever is wiring it up will be looking.
    #[command(hide = true)]
    Mcp,
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
    ///
    /// **Hidden**, for the reason `mcp` is: `devplane connect claude` writes
    /// the shim that calls this, and nobody types it.
    #[command(hide = true)]
    Statusline {
        /// The user's original status-line command, run after forwarding.
        #[arg(long)]
        then: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum SpeckitCmd {
    /// Add the hook to `.specify/extensions.yml`, or print it where one exists.
    Install {
        /// Which hook point. Defaults to `after_implement` — where code has
        /// just been written, so a gate has something to check.
        #[arg(long)]
        event: Option<String>,
        /// Print and write nothing.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Write the hook even where the repository has no gate to run.
        ///
        /// A hook that calls a gate nothing declares fails every time it fires,
        /// which teaches people to ignore it. So this refuses by default and
        /// says what to declare first.
        #[arg(long)]
        anyway: bool,
    },
}

#[derive(Subcommand)]
pub enum GateCmd {
    /// Run this repository's gates now and report the verdict.
    Run {
        /// Which repository. Defaults to the working directory.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// One named gate from `devplane.toml`, rather than the whole check.
        ///
        /// Without it the repository's `check` runs, which is the definition of
        /// done here. A named gate is a different question — *does this one
        /// suite pass* — and some are declared `expect = "fail"`, so the two
        /// cannot share a verdict.
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum LibraryCmd {
    /// Every artefact this machine can reach.
    List,
    /// Which of your copies drifted, which projects lack it, and what a
    /// distribution path will reject. **Reports; changes nothing.**
    Diff {
        /// One artefact, or every one when omitted.
        artefact: Option<String>,
    },
    /// What it will be allowed to do, and where it came from.
    ///
    /// Never a verdict. Not *safe*, not *risky*, not a tick.
    Report { artefact: String },
    /// Copy it into projects, byte for byte, into vendor-documented paths only.
    ///
    /// Every refusal is named **before the first byte is written**.
    Install {
        artefact: String,
        /// Project names, comma-separated. Every registered project when omitted.
        #[arg(long, value_delimiter = ',')]
        to: Vec<String>,
        /// Replace a copy that differs. Never overrides an untrusted target.
        #[arg(long)]
        force: bool,
    },
    /// Bring one copy into line, in a direction you name.
    Sync {
        artefact: String,
        /// `library` (library → project) or `project` (project → library).
        #[arg(long)]
        from: String,
        /// The project.
        #[arg(long)]
        to: String,
        /// Without this, it prints what would change and writes nothing.
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand, Debug)]
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
        /// The specification this work answers — a file, or the folder your
        /// spec tool wrote — relative to the repository.
        ///
        /// Stamped onto the done certificate, with what its task list said when
        /// each gate ran. No methodology is learned: the outline is the
        /// Markdown headings, the progress is the `- [ ]` boxes, and the
        /// `[gates]` commands you declare are what actually check the work.
        #[arg(long)]
        spec: Option<String>,
    },
    /// Show every piece of work.
    #[command(visible_alias = "ls")]
    List,
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
    /// Print the done certificate: what was checked, against which commit, and
    /// how to check it yourself.
    ///
    /// The artifact is meant to be pasted into a pull request. Everything a
    /// reviewer needs to re-derive the outcomes is in it, and none of it
    /// requires trusting Devplane — they check out the commit and run the
    /// commands. `--json` gives the same facts as an in-toto statement for
    /// another tool to read.
    Export { work: String },
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
///
/// Takes the `Cli` rather than parsing one, which is the whole reason this
/// lives in the library: a test can drive a subcommand without a subprocess.
/// It used to re-parse the process's own argv and ignore the argument, so such
/// a test would have run against the harness's command line.
pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Serve { port }) => cmd_serve(port).await,
        Some(Command::Ls {
            all,
            project,
            needs_you,
        }) => cmd_ls(all, project.as_deref(), needs_you, cli.json).await,
        None => cmd_ls(false, None, false, cli.json).await,
        Some(Command::Inbox { project, needs_you }) => {
            cmd_inbox(cli.json, project.as_deref(), needs_you).await
        }
        Some(Command::Issues { ready, cwd, label }) => {
            if ready || cwd.is_some() || label.is_some() {
                crate::cli::work::cmd_ready_issues(cwd, label, cli.json).await
            } else {
                crate::cli::board::cmd_forge_issues(cli.json).await
            }
        }
        Some(Command::Prs) => crate::cli::board::cmd_forge_prs(cli.json).await,
        Some(Command::Show { run }) => cmd_show(&run, cli.json).await,
        Some(Command::Tail {
            run,
            thinking,
            history,
        }) => cmd_tail(&run, thinking, history).await,
        Some(Command::Search { query }) => cmd_search(&query, cli.json).await,
        Some(Command::Rewind { run }) => cmd_rewind(&run, cli.json).await,
        Some(Command::Audit {
            about,
            without_me,
            limit,
            otel,
        }) => cmd_audit(about.as_deref(), without_me, limit, cli.json, otel).await,
        Some(Command::Completions { shell }) => crate::cli::completions::cmd_completions(&shell),
        Some(Command::Rules { rule, ask }) => {
            crate::cli::rules::cmd_rules(rule, ask, cli.json).await
        }
        Some(Command::Modes) => crate::cli::inbox::cmd_modes(cli.json).await,
        Some(Command::Library { what }) => match what {
            LibraryCmd::List => crate::cli::library::cmd_list(cli.json).await,
            LibraryCmd::Diff { artefact } => {
                crate::cli::library::cmd_diff(artefact, cli.json).await
            }
            LibraryCmd::Report { artefact } => {
                crate::cli::library::cmd_report(artefact, cli.json).await
            }
            LibraryCmd::Install {
                artefact,
                to,
                force,
            } => crate::cli::library::cmd_install(artefact, to, force, cli.json).await,
            LibraryCmd::Sync {
                artefact,
                from,
                to,
                apply,
            } => crate::cli::library::cmd_sync(artefact, from, to, apply, cli.json).await,
        },
        Some(Command::Attention { days }) => cmd_attention(days, cli.json).await,
        Some(Command::Focus { run }) => cmd_focus(&run).await,
        Some(Command::Attach { run }) => cmd_attach(&run).await,
        Some(Command::Dispatch {
            prompt,
            agent,
            cwd,
            to,
            mode,
            template,
            apply,
        }) => {
            if to.is_empty() {
                cmd_dispatch(&agent, cwd, prompt.join(" "), cli.json).await
            } else {
                crate::cli::batch::cmd_fan_out(
                    &agent,
                    to,
                    prompt.join(" "),
                    mode.as_deref(),
                    template.as_deref(),
                    apply,
                    cli.json,
                )
                .await
            }
        }
        Some(Command::Batch { id }) => crate::cli::batch::cmd_batch(id, cli.json).await,
        Some(Command::Say { run, prompt }) => cmd_say(&run, prompt.join(" ")).await,
        Some(Command::Answer {
            ask,
            allow,
            deny,
            option,
            custom,
            field,
        }) => cmd_answer(&ask, allow, deny, option, custom, field).await,
        Some(Command::Asks) => cmd_asks(cli.json).await,
        Some(Command::Gate {
            what: GateCmd::Run { cwd, name },
        }) => cmd_gate_run(cwd, name, cli.json).await,
        Some(Command::Speckit {
            what:
                SpeckitCmd::Install {
                    event,
                    dry_run,
                    anyway,
                },
        }) => cmd_speckit_install(event, dry_run, anyway),
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
        Some(Command::Trust { path, yes, dry_run }) => {
            cmd_trust(path, yes, dry_run, cli.json).await
        }
        Some(Command::Work { what }) => cmd_work(what, cli.json).await,
        Some(Command::Snooze { id, minutes }) => cmd_snooze(&id, minutes, cli.json).await,
        Some(Command::Open) => cmd_open().await,
        Some(Command::Watch) => cmd_watch().await,
        Some(Command::Doctor) => cmd_diagnostics(cli.json).await,
        Some(Command::Connect { what, statusline }) => {
            cmd_connect(what, statusline, cli.json).await
        }
        Some(Command::Disconnect { what }) => cmd_disconnect(what, cli.json).await,
        Some(Command::Stop) => cmd_stop().await,
        Some(Command::Mcp) => crate::mcp::Server::run().await,
        Some(Command::Hook { gate }) => cmd_hook(gate).await,
        Some(Command::Statusline { then }) => cmd_statusline(then).await,
    }
}

// ---------------------------------------------------------------------------

async fn cmd_serve(port: u16) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("DEVPLANE_LOG")
                .unwrap_or_else(|_| "devplane=info,warn".into()),
        )
        .init();

    // **A live pid is not a running daemon.** `daemon.json` outlives a daemon
    // that was killed, and the operating system reuses the pid — after which
    // this guard refused to start for ever, naming somebody else's process.
    // Three outcomes, and the middle one is the one that was missing.
    if let Some(info) = config::read_daemon_info()?
        && info.pid != std::process::id()
        && poller::process_alive(info.pid)
    {
        match crate::observe::procs::is_daemon(info.pid) {
            // It is there and it is us. The original, correct refusal.
            Some(true) => anyhow::bail!(
                "a daemon is already running (pid {}, port {}). Stop it with `devplane stop`.",
                info.pid,
                info.port
            ),
            // The pid is alive and belongs to something else, so the record is
            // stale and the pid has come round again. Clearing it is the whole
            // repair, and saying so beats leaving somebody to guess.
            Some(false) => {
                tracing::warn!(
                    pid = info.pid,
                    "daemon.json names a pid that belongs to something else; the last daemon did not shut down cleanly. Ignoring it."
                );
                config::clear_daemon_info().ok();
            }
            // The process table could not be read, so this cannot tell a stale
            // record from a live daemon. **Refuse**, because two daemons on one
            // database both poll, both reconcile and both start agents, which
            // is worse than a refusal — and name the file, because at this
            // point a person has to decide.
            None => anyhow::bail!(
                "a daemon may already be running (pid {}, port {}), and the process table could not be read to confirm it.\nIf you are sure it is not, delete {} and try again.",
                info.pid,
                info.port,
                config::home()?.join("daemon.json").display()
            ),
        }
    }

    let token = config::load_or_create_token()?;
    let policy = load_policy();
    let home = config::home()?;
    let state = daemon::AppState::new(config::db_path()?, token, policy, home).await?;
    drain_decision_spool(&state).await;
    poller::reconcile_at_startup(&state).await;
    // Fill the board before anyone can ask for it.
    poller::initial_poll(&state).await;
    daemon::serve(state, port).await
}

/// The machine-wide rules, from `~/.devplane/policy.toml`.
///
/// Empty by default: no rule matches, so every permission prompt reaches the
/// human exactly as it does today. A policy that guessed on the user's behalf
/// would be a policy that approved something nobody chose.
/// Writes down the decisions the `command` hook took while no daemon was
/// listening.
///
/// The hook decides in its own process, so a stopped daemon costs the *record*
/// and not the enforcement. This is the other half of that trade: without it,
/// `devplane audit` would be missing exactly the refusals that happened when
/// nobody was watching, and would not say so.
async fn drain_decision_spool(state: &std::sync::Arc<daemon::AppState>) {
    let pending = config::drain_spool();
    if pending.is_empty() {
        return;
    }
    tracing::info!(
        count = pending.len(),
        "filing decisions taken while the daemon was down"
    );
    for row in pending {
        if let Ok(env) = serde_json::from_value::<crate::core::DecidedEnvelope>(row) {
            crate::api::record_decided(state, env).await;
        }
    }
}

fn load_policy() -> crate::core::Policy {
    let Ok(home) = config::home() else {
        return crate::core::Policy::default();
    };
    match crate::core::GlobalConfig::load(&home) {
        Ok(g) => {
            let policy = g.policy();
            tracing::info!(
                deny = policy.deny_rules().len(),
                ask = policy.ask_rules().len(),
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
            paint(DIM, "devplane ls --all lists every session, ids included.")
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

/// The `command` hook: decide here, report afterwards.
///
/// The verdict is reached in this process, with no daemon and no network —
/// evaluation is pure, synchronous and reads two small local files, and a cold
/// process answers in about 26 ms. Claude Code walks past an unreachable HTTP
/// hook (*"Connection failure: non-blocking error, execution continues"*), so a
/// gate that needs a socket is absent whenever the daemon is.
///
/// The daemon is still what *sees*: board, transcript, decision log. That is
/// reported afterwards, best-effort, and never blocks the answer.
/// **Deciding must not depend on a network; seeing may.**
///
/// Never exits non-zero: a hook's exit code is something the user's session
/// reacts to.
async fn cmd_hook(gate: Option<String>) -> Result<()> {
    use std::io::Read;
    let mut body = String::new();
    std::io::stdin().read_to_string(&mut body).ok();

    match gate.as_deref() {
        Some("copilot") => decide_copilot(&body).await,
        Some(_) | None => decide_claude(&body).await,
    }
}

/// Claude Code's `PreToolUse` and `PermissionRequest`, answered here.
async fn decide_claude(body: &str) -> Result<()> {
    use crate::observe::hook::{HookPayload, PermissionResponse};

    // An unparseable payload is not a decision. Saying nothing puts the call
    // back into the provider's own permission flow, which is where it would be
    // with no hook installed at all.
    let Ok(payload) = serde_json::from_str::<HookPayload>(body) else {
        println!(
            "{}",
            serde_json::to_string(&PermissionResponse::undecided())?
        );
        return Ok(());
    };
    let tool = payload.tool_name.clone().unwrap_or_default();
    let input = payload
        .tool_input
        .clone()
        .unwrap_or(serde_json::Value::Null);

    let (cache, _) = crate::core::PolicyCache::from_disk();
    let pre = payload.hook_event_name == "PreToolUse";
    // `PreToolUse` fires on every call in every mode and may carry only a
    // prohibition. `PermissionRequest` fires when a person was going to be
    // asked, so the full verdict applies.
    let verdict = match (&payload.cwd, pre) {
        (Some(dir), true) => cache.restrictive(std::path::Path::new(dir), &tool, &input),
        (Some(dir), false) => cache.restrictive(std::path::Path::new(dir), &tool, &input),
        // No directory means no honest project answer, so the machine-wide
        // rules decide alone. Falling back to this process's own directory
        // would let one repository's rules answer another's session.
        (None, true) => cache.restrictive_global_only(&tool, &input),
        (None, false) => cache.restrictive_global_only(&tool, &input),
    };

    // Answer first. Everything below is bookkeeping the session is not waiting
    // for, and a failure in it may not change what was decided.
    if pre {
        println!(
            "{}",
            serde_json::to_string(&crate::observe::hook::pre_tool_use_reply(&verdict))?
        );
    } else {
        // **A permission a person could answer from anywhere, if this project
        // asked for it.**
        //
        // Ordered deliberately: a prohibition is already decided above and
        // never reaches here, so a hold can never turn a refusal into a
        // question. Only a verdict of `ask` is held — a call the project's own
        // `always_ask` rules matched — because a second selector beside
        // `always_ask` would be a second thing to keep in step, and because
        // holding every routine call would freeze an unattended agent.
        let held = match (&verdict, &payload.cwd) {
            (crate::core::Verdict::Ask { .. }, Some(dir)) => {
                hold_for_a_person(&payload, dir, &tool, &input).await
            }
            _ => None,
        };
        let reply = match held.as_deref() {
            // The person's own selection, carried. Not a verdict: the policy
            // engine never saw this and there is no `Verdict::Allow` for it to
            // have returned.
            Some("allow") => crate::observe::hook::PermissionResponse::allow(),
            Some("deny") => crate::observe::hook::PermissionResponse::deny(
                "denied by the person, from Devplane",
            ),
            // **Lapsed, or never held.** Identical to the behaviour with no
            // hold configured: the vendor shows its own dialog and answers it
            // where the person already is.
            _ => crate::observe::hook::permission_reply(&verdict),
        };
        println!("{}", serde_json::to_string(&reply)?);
    }
    report(
        body,
        &verdict,
        &payload.session_id,
        &tool,
        crate::observe::hook::describe_call(&tool, &input),
        // A `PermissionRequest` nobody had a rule about means Claude Code is
        // asking a person right now. `PreToolUse` fires on every call and
        // implies nothing of the sort.
        !pre && verdict.rule().is_none(),
    )
    .await;
    Ok(())
}

/// Waits, for as long as this project said, for somebody to answer.
///
/// **Returns `None` for every reason except an answer**, and that is the whole
/// safety argument. No hold configured, no daemon, a daemon that does not
/// answer, a value that will not parse, a hold that ran out — all of them lapse
/// into the vendor's own dialog, which is exactly what happens today.
///
/// **It connects, and never starts.** A hook that launched a daemon would turn
/// a permission prompt into a several-second pause on a machine where Devplane
/// was deliberately not running.
async fn hold_for_a_person(
    payload: &crate::observe::hook::HookPayload,
    dir: &std::path::Path,
    tool: &str,
    input: &serde_json::Value,
) -> Option<String> {
    let cfg = crate::core::ProjectConfig::load(dir).ok()?;
    // A value that will not parse is a problem `devplane check` reports, and
    // **not a hold**: guessing what somebody meant by a typo is how an agent
    // ends up frozen for a duration nobody wrote.
    let hold = cfg.questions.hold().ok().flatten()?;

    let c = crate::client::Client::connect().ok()?;
    let call = crate::observe::hook::describe_call(tool, input);
    let body = serde_json::json!({
        "session": payload.session_id,
        "cwd": dir.display().to_string(),
        "tool": tool,
        "call": call,
        "message": format!("{tool} · {call}"),
        "wait_ms": hold.0.as_millis() as u64,
    });

    // **The client's own timeout must outlast the hold**, or the hook gives up
    // on a daemon that is still waiting for the person and the answer arrives
    // nowhere. A little longer, so the daemon is always the thing that decides
    // the hold is over.
    let reply: serde_json::Value = c
        .post_json_within(
            "/devplane/hold",
            &body,
            hold.0 + std::time::Duration::from_secs(5),
        )
        .await
        .ok()?;
    reply
        .get("behavior")
        .and_then(|b| b.as_str())
        .map(str::to_string)
}

/// GitHub Copilot's `preToolUse`, answered here. Its hook vocabulary differs;
/// its verdict does not — one policy engine, in this process, and never a rule
/// translated into a vendor's own configuration file.
async fn decide_copilot(body: &str) -> Result<()> {
    use crate::core::Verdict;
    use crate::observe::copilot::{GateReply, HookPayload};

    let Ok(payload) = serde_json::from_str::<HookPayload>(body) else {
        println!("{}", GateReply::undecided().to_json());
        return Ok(());
    };
    let tool = payload.tool();
    let input = payload.input();
    let (cache, _) = crate::core::PolicyCache::from_disk();
    let verdict = match payload.cwd.as_deref() {
        Some(dir) => cache.restrictive(std::path::Path::new(dir), &tool, &input),
        None => cache.restrictive_global_only(&tool, &input),
    };
    let reply = match &verdict {
        Verdict::Deny { rule } => GateReply::deny(format!("denied by Devplane policy rule {rule}")),
        Verdict::Ask { rule } => GateReply::ask(format!("{rule} asks that a person decides this")),
        _ => GateReply::undecided(),
    };
    println!("{}", reply.to_json());
    report(
        body,
        &verdict,
        &payload.session_id,
        &tool,
        crate::observe::hook::describe_call(&tool, &input),
        false,
    )
    .await;
    Ok(())
}

/// Hands the payload and the verdict to the daemon so the board, the transcript
/// and the decision log see them — and spools the decision when there is no
/// daemon to hand it to.
///
/// **The spool exists because this hook now decides without one.** A decision
/// taken and not written down is the audit trail quietly acquiring holes, which
/// is the same class of failure as a rule that quietly does not fire: nothing
/// errors, and the gap is invisible from the inside. One append-only line per
/// decision, drained at the next daemon start.
///
/// Only *decisions* are spooled, never observations: a tool call nobody had a
/// rule about is re-derivable from the provider, and a board that missed an
/// hour is a smaller loss than a log that cannot account for a refusal.
async fn report(
    body: &str,
    verdict: &crate::core::Verdict,
    session: &str,
    tool: &str,
    subject: String,
    blocked: bool,
) {
    // `devplane doctor` runs this gate to check that it answers. The verdict
    // is real and the call is not, so it is reported nowhere: a diagnostic that
    // writes to the append-only log makes the log worse every time somebody
    // checks the tool is working.
    if session == crate::observe::hook::PROBE_SESSION {
        return;
    }
    let env = crate::core::DecidedEnvelope {
        session: session.to_string(),
        verdict: verdict.as_str().to_string(),
        rule: verdict.rule().map(str::to_string),
        server_source: None,
        why: verdict.why().map(str::to_string),
        subject,
        tool: tool.to_string(),
        at: Some(jiff::Timestamp::now()),
        late: false,
        blocked,
        payload: serde_json::from_str(body).ok(),
    };
    if post_decided(&env).await {
        return;
    }
    // No daemon. A decision that was enforced and never written down is an
    // audit trail quietly acquiring holes, which is the same class of failure
    // as a rule that quietly does not fire — nothing errors, and the gap is
    // invisible from the inside. Observations are not spooled: a tool call
    // nobody had a rule about is re-derivable from the provider, and a board
    // missing an hour is a smaller loss than a log that cannot account for a
    // refusal.
    if env.rule.is_some() {
        let mut late = env;
        late.late = true;
        if let Ok(v) = serde_json::to_value(&late) {
            let _ = crate::config::spool_decision(&v);
        }
    }
}

/// True when the daemon accepted it.
async fn post_decided(env: &crate::core::DecidedEnvelope) -> bool {
    let (Ok(Some(info)), Ok(token)) = (config::read_daemon_info(), config::load_or_create_token())
    else {
        return false;
    };
    let Ok(body) = serde_json::to_string(env) else {
        return false;
    };
    reqwest::Client::new()
        .post(format!("{}/devplane/decided", info.base_url()))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(body)
        .timeout(std::time::Duration::from_millis(500))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
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
            .post(format!("{}/devplane/statusline", info.base_url()))
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

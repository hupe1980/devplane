+++
title = "CLI reference"
description = "Every Vibeplane command. All of them take --json, and all of them start the daemon if it is not already running."
weight = 20
[extra]
group = "reference"
+++

Two things are true of every command below: it takes `--json`, and it starts the daemon if one is
not already running. There is nothing to launch first.

Installing puts one binary on your `PATH`, named `vibeplane`. If you type it often enough to
want it shorter, alias it yourself — there is no second binary to install:

```sh
alias vp=vibeplane
```

### Naming a run

Anywhere a command takes `<run>`, you can give it **whatever the board printed** — Vibeplane
resolves it the way `git` resolves a short commit: the full id, any unambiguous prefix of it, or the
session's name.

```console
$ vibeplane ls
core-lib
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace

$ vibeplane show a1        # the label on the row is enough
```

If what you typed matches more than one session it says so and lists them, rather than picking one —
the actions here include stopping an agent, and a wrong guess is expensive. A session that has never
reported loses to a live one, so a week-old editor tab sharing a prefix does not cost today's session
its short name.

Work ids resolve by prefix the same way.

## Asking

### `vibeplane connect copilot`

```sh
vibeplane connect copilot      # writes ~/.copilot/hooks/vibeplane.json
vibeplane disconnect copilot   # deletes it
```

One file, and two lines printed for you to export — Copilot reads telemetry from the environment.
See [Observing sessions](/docs/observe/#github-copilot).

### `vibeplane explain`

What the gate would decide about one call, and which rule says so. Offline: no daemon, no agent, no
bill — which is what you want while you are still writing the rule.

```console
$ vibeplane explain 'pnpm test --run'
allow      Bash
        by Bash(pnpm test *)

$ vibeplane explain 'pnpm test && rm -rf /'
deny       Bash
        by Bash(rm -rf *)

$ vibeplane explain 'echo x | tee /etc/hosts'
undecided  Bash
        no rule answers this one, so the provider's own dialog decides and it reaches your inbox
```

The third answer is the interesting one. A rule *did* match — `Bash(echo *)` — and still did not
speak for the call, because an allow rule covers the command and not the file it writes. Those cases
are invisible from reading a rule list, and they are where a permission layer is usually wrong.

| Flag | What |
|---|---|
| `--tool <name>` | the tool, default `Bash`. `Read`, `Edit` and `WebFetch` take their specifier directly: `vibeplane explain --tool Read .env` |
| `--input '<json>'` | the whole tool input, for a call a specifier cannot express: `--tool Agent --input '{"isolation":"worktree"}'` |
| `--dir <path>` | the directory the agent would be working in, which decides whose rules apply. Default `.` |
| `--replay` | ask the same question of every call already observed, and name the rule that would answer the ones that reached you |
| `--limit <n>` | how many of the most recent calls `--replay` reads. Default 5000 |
| `--json` | the verdict, the rule, and what a `PreToolUse` hook would answer — which is a different question, and the one that holds in auto mode |

### `vibeplane explain --replay`

Which rule to write next, from the calls this machine has already made. It replays every observed
tool call against the rules **as they are now**, and groups the ones that reached you by the rule
that would have answered them.

```console
$ vibeplane explain --replay
1284 tool calls in saas replayed against the rules as they are now

     912   71%  allow
      14    1%  ask
     358   28%  reached you

one rule each, most interruptions first
   118×  Bash(pnpm typecheck)
    94×  Bash(git status)
    62×  Bash(cargo test *)
    41×  Read(src/**)
    12×  WebFetch(docs.rs)

315 of the 358 calls that reached you would stop asking · paste into [policy] auto_allow, then vibeplane check
```

Each suggestion is in the vocabulary that tool's rules use: a command prefix with the `*` after the
subcommand, a directory glob for a path rule, a domain for `WebFetch`.

- **Offline**, like the rest of `explain`: it opens the store read-only and asks no agent anything.
- **Nothing is written for you.** Paste what you want into `[policy] auto_allow`.
- A rule is offered only where one covers the set, and only after a command has interrupted you
  **three times** — which is what keeps `Bash(rm -rf node_modules)` off the list.

`--dir` scopes it to one project; the default is the repository you are standing in.

## Looking

### `vibeplane ls`

The working set — sessions in play, and anything asking for you. Alias: `ps`. This is also what
plain `vibeplane` does.

| Flag | What |
|---|---|
| `--all`, `-a` | include sessions nothing has been heard from for hours — editor tabs, usually |
| `--project <name>`, `-p` | one project; matches any part of the name, so `mat` finds `matter-kit` |
| `--needs-you` | only what is waiting on a human |

### `vibeplane inbox`

What needs a human, most urgent first. Derived from state rather than stored, so it is correct after
a restart. Ranked by level, then age, oldest first.

A permission item also names **the rule that would have answered it**:

```console
 ! Permission: Bash [permission]
     pnpm test --run
     1. Yes
     2. No
     never asked again: auto_allow = ["Bash(pnpm test --run)"]
```

That is the **exact call**, never a pattern: one interruption says nothing about the shape of the
calls like it. [`vibeplane explain --replay`](#vibeplane-explain-replay) is where a pattern comes
from, where the evidence is a count. Nothing is written for you either way.

One item is about the machine rather than about any run, and it is the only thing Vibeplane raises
about **itself**:

```console
 ! The permission gate is installed and not answering [gate_down]
     No rule in any project is being enforced right now.
     it will not start: No such file or directory
```

A broken gate and a quiet machine look identical from the outside: no hook arrives either way. The
daemon runs the installed gate on a timer for that reason. Critical, and no action offered — the fix
is `vibeplane connect claude`, and this is not a product that rewrites your `settings.json` from a
list.

### `vibeplane issues` · `vibeplane prs`

Every open issue, and every open pull request, across **every registered project** — read through
your own `gh`, grouped by project, with what needs you first:

```console
$ vibeplane prs
as hupe1980 · what needs you first

saas
  ◆ #142   Fix the flaky login test                     red  yours
    https://github.com/acme/saas/pull/142
  ◆ #150   Add rate limiting                            approved · green  review asked of you
    https://github.com/acme/saas/pull/150

core-lib
  ○ #31    Bump tokio                                   pending
    https://github.com/acme/core-lib/pull/31
```

The board has the same two lists behind `g`, and the counts in a project heading open them.

`◆` marks what is waiting on you: an issue assigned to you, a review requested from you, or your own
pull request that is red, has changes requested, or is approved and unmerged. A draft of your own is
never one of them — you marked it unfinished — but a review requested of you, or changes requested on
your own, reaches you through a draft. These are inbox items at normal urgency, so none of them
raises a desktop notification.

The daemon reads GitHub a few seconds after it starts and every five minutes after that. A project it
could not read keeps its last good numbers and is marked `· stale`. A project with no GitHub remote
is ruled out and asked again an hour later. `vibeplane doctor` says whose `gh` this is, when it last
read, and which projects were ruled out and why.

**Nothing here writes to GitHub** — every action is a link, and `vibeplane work start --issue <n>` is
how an issue becomes work.

`vibeplane issues --ready` asks a different question of one repository: which of its issues are
*offered* as work — the ones carrying `[github].ready_label` — which is the list
`work start --issue` picks from. It reads that repository live rather than serving the poller's
cached counts.

| Flag | What |
|---|---|
| `--ready` | only what this repository offers as work |
| `--label <l>` | default: `[github].ready_label`; implies `--ready` |
| `--cwd <path>` | which repository; implies `--ready` |

A project whose directory has no GitHub remote is asked once and then left alone.

### `vibeplane show <run>`

One run in detail: state, agent, model, surface, cost, tool calls, what it is blocked on, the agent's
own plan, recent tools, and — for a driven run — the last few things it said.

### `vibeplane tail <run>`

Follow what a driven agent is saying, like `tail -f`.

| Flag | What |
|---|---|
| `--thinking` | include its reasoning, where it streams any |
| `--history <n>` | how much of the conversation to print first (default 40) |

Only for runs Vibeplane drives. A session you started in a terminal or an editor is already showing
you its own transcript — `vibeplane focus` raises that window, and `tail` says so rather than
printing nothing for ever.

### `vibeplane search <query>`

Full-text search over tool commands, questions and errors across every session. What you type is a
phrase, not a query language.

The board has the same search — press <kbd>/</kbd>, or click the box in the header. A match names the
session it came from and opens it.

### `vibeplane audit [id]`

What Vibeplane decided, and on whose authority. Narrow to a run or a piece of work by id. See
[the decision log](/docs/decisions/).

| Flag | What |
|---|---|
| `--limit <n>` | how many rows (default 50) |

### `vibeplane attention`

Whether the inbox is worth reading, per kind. Every item Vibeplane raises is recorded, and every
resolution says what became of it.

```console
$ vibeplane attention
kind               raised   acted  dismissed  elsewhere   open   acted
permission             41      36          0          4      1     90%
gate_failed             9       8          1          0      0     89%
refused                 3       3          0          0      0    100%
context_high           23       1         17          5      0      4%
stalled                12       0          2         10      0      0%
```

| Outcome | What it means |
|---|---|
| `acted` | you used one of the item's own actions — answered, allowed, denied, approved, retried, resumed |
| `dismissed` | you snoozed it. The clearest evidence a kind is too loud |
| `elsewhere` | it stopped asking on its own: the agent unblocked, the checks went green, you answered in a terminal |

Three numbers, not one score: `elsewhere` is ambiguous — the item was right that you were needed and
wrong about where — so it is reported rather than averaged away. A kind that is mostly `dismissed` is
a threshold to change.

| Flag | What |
|---|---|
| `--days <n>` | how far back to look (default 7) |

### `vibeplane watch`

Follow events as they arrive. Two kinds of frame, told apart on the wire: state changes, and
fragments of what a driven agent is saying.

### `vibeplane open`

Open the board in a browser. The token is handed over once in the URL and stripped from the address
bar, so it cannot end up in a screenshot or a bookmark.

### `vibeplane doctor`

Whether the tool itself is telling you the truth: hook latency, when telemetry was last seen, the
roster, each channel's last error **with the date it happened**, and any project whose
`vibeplane.toml` will not parse — whose permission rules are therefore not in force.

**It runs the gate rather than reading about it.** No hook can enforce its own presence, so a
settings file containing the right line is evidence about a settings file. `doctor` writes a
throwaway project with one deny rule, runs the **installed command** with a real `PreToolUse`
payload, and says whether a refusal came back:

```console
claude code
  settings  ~/.claude/settings.json
  hooks     installed (22 events)
  gate      answering (21 ms, measured just now)
```

or, when something is wrong with it:

```console
  gate      INSTALLED AND NOT ANSWERING — every prohibition on this machine is inert
            /usr/local/bin/vibeplane hook
            it will not start: No such file or directory
            a hook that does not answer never blocks — the provider carries on
```

The probe is recorded nowhere: a diagnostic that files decisions would make the audit trail worse
every time you checked the tool was working.

If the gate has been deciding while no daemon was running, `doctor` says how many decisions are
waiting to be filed. Starting the daemon files them.

It starts with **which model provider you are on**, because that decides which of Claude Code's own
supervision surfaces exist here at all. On Bedrock, Google Cloud's Agent Platform, Microsoft Foundry,
an Anthropic Console key or a corporate gateway they are unavailable, while hooks, OpenTelemetry,
workflows and skills all still work:

```console
provider
  Amazon Bedrock  (CLAUDE_CODE_USE_BEDROCK is set)
  Vibeplane is the only gate on this machine.
  off here   Remote Control · Routines (/schedule) · ultrareview · Code Review · Channels · …
  partial    auto mode — fewer models, and sessions start in Manual
  still on   hooks · OpenTelemetry metrics · workflows · skills and commands · sandboxing · …
```

On a claude.ai sign-in it is one line. It names the variable that decided, and a variable exported
empty reads as unset.

It also reads back **Claude Code's own auto-mode classifier**, because in that mode the thing
actually deciding is configured somewhere Vibeplane does not write:

```console
auto mode (Claude Code's own classifier)
  21 trusted entries · 17 allow · 70 soft deny · 1 hard deny
  your deny and ask rules resolve before it; it cannot override them
```

Vibeplane reads this and never writes it. If nothing is configured it says so: an unconfigured
classifier trusts only the working repository and its remotes, which is the usual cause of denials
people blame on the agent. `/auto-mode-setup` in Claude Code drafts the entries.

## Acting

### `vibeplane focus <run>`

Raise the editor window that owns a run's directory. When no window has it open, says so and prints
the resume command rather than claiming success.

### `vibeplane attach <run>`

Hand the terminal to the real agent, resuming its session — `claude --resume <id>`, or
`claude attach <id>` for a background session, whose daemon has its own way in. Replaces this process
rather than nesting one inside it.

### `vibeplane dispatch <prompt…>`

Start an agent and give it something to do.

| Flag | What |
|---|---|
| `--agent <id>` | `claude`, `codex`, `opencode`, `gemini`, anything in `agents.toml`, or a command line |
| `--cwd <path>` | where it runs (default: here) |

### `vibeplane say <run> <prompt…>`

Send another prompt to a run Vibeplane drives.

### `vibeplane decide <run> --request <id>`

Answer a permission request from a driven run.

| Flag | What |
|---|---|
| `--decision <allow\|deny>` | default `deny`; omitting it refuses |
| `--option <id>` | an exact option id from the inbox item, when the agent offers more than two |

Option ids belong to the agent — one calls it `allow`, another `proceed_once` — so `--decision` is
resolved against the options it actually offered rather than a guessed string.

### `vibeplane snooze <id>`

Hide a run's — or a piece of work's — inbox items for a while. Takes either id; the inbox prints
whichever one an item is about.

**A snooze covers only the kinds being asked when you take it.** Something new turns up, you see it.

A snoozed run stays on the board and a snoozed piece of work keeps running: this hides a request, not
the thing itself. Both ids are accepted because the items Work produces outlive their sessions — a
pull request that goes red hours later has no run left to quieten.

| Flag | What |
|---|---|
| `--minutes <n>` | default 60; `0` un-snoozes |

## Work

### `vibeplane work start <title…>`

Make an isolated checkout, prepare it, put an agent in it, and run the project's gates when the agent
says it is finished.

| Flag | What |
|---|---|
| `--kind <k>` | `quick` (default), `chore`, `bug`, `feature` — the kind chooses the pipeline |
| `--agent <id>` | overrides `[project] default_agent` |
| `--cwd <path>` | which repository |
| `--no-worktree` | work in the repository itself rather than an isolated checkout |
| `--issue <n>` | start from a GitHub issue; its body arrives marked as an untrusted report |

### `vibeplane work list`

Where everything is. Alias: `ls`.

### `vibeplane work show <id>`

Phase, pipeline stepper, runs, cost, and what each check actually said.

### `vibeplane work verify <id>`

Run the project's gates now. A read: it never changes a phase a person asked for.

### `vibeplane work approve <id>`

Release a pipeline waiting at a declared human step.

### `vibeplane work retry <id>`

Hand the failures back to the agent once more, past the project's bound. Offered only while the
session that wrote the code still exists; recorded as a human decision, and counted.

### `vibeplane work resume <id>`

Pick work back up after the daemon that was running it stopped, **against the same agent-side
conversation**.

A restart takes every agent process with it. The Work row, the branch and the worktree all survive
one, and a resumable agent keeps the conversation — so this reconnects to that conversation rather
than starting a new one. The difference is not cosmetic: a fresh session would pay a second time to
rediscover what the first one already knew, and would look at the half-finished code in the worktree
without the context that produced it.

It is offered in the inbox on the `interrupted` item, and only when it can actually work: the run
recorded the id its agent answers `session/resume` on, and Vibeplane is not already holding a session
for it. Where the agent cannot resume — it does not advertise the capability, or it has forgotten the
session — this says so instead of silently starting again.

Deliberately not automatic on startup. Resuming spends money and runs an agent in a repository, and
doing either because a machine rebooted is a decision nobody made.

### `vibeplane work finish <id>`

| Flag | What |
|---|---|
| `--remove-worktree` | remove the isolated checkout too |
| `--force` | discard uncommitted changes in it |

Without `--force`, removal refuses to destroy uncommitted or unpushed work.

## Setup and health

### `vibeplane trust [path]`

Allow Vibeplane to start agents in a repository. Required once per repository, because a headless
agent runs that repository's own hooks and MCP servers without asking.

### `vibeplane check [path]`

Read the repository's `vibeplane.toml` and say what it will do — and refuse what cannot work. Offline
and daemon-free, so it runs in CI. Exits non-zero on an error.

### `vibeplane agents`

The agents Vibeplane can drive.

### `vibeplane connect claude` · `disconnect claude`

Install or remove hooks and telemetry, in your user settings, with a backup.

| Flag | What |
|---|---|
| `--statusline` | also wrap your status line — the only source of subscription rate limits |

### `vibeplane serve`

Run the daemon in the foreground. Every other command starts it in the background as needed.

| Flag | What |
|---|---|
| `--port <n>` | default `47831`; `0` asks the OS for any free port |

When the default port is taken by something else, the daemon takes another one and clients follow via
`~/.vibeplane/daemon.json`. A port you asked for explicitly is never silently swapped.

### `vibeplane stop`

Ask the daemon to stop — over its own API, with the bearer token, reaching the same graceful path as
ctrl-C. **The agents it started are stopped first**, and waited for. It does not signal a pid: a
record left behind by a crash names a pid the operating system may since have given to something
else.

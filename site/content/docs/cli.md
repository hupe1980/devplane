+++
title = "CLI reference"
description = "Every Devplane command. All of them take --json; reading works with nothing running, and a command that needs the host says so."
weight = 20
[extra]
group = "reference"
+++

Every command takes `--json`. Colour follows `NO_COLOR`, is off when output is not a terminal, and
`CLICOLOR_FORCE=1` forces it on.

**Reading needs no host.** `ls`, `inbox`, `asks`, `audit`, `show`, `search`, `change list`, `change
show`, `modes`, `agents`, `rules` and `attention` ask a running host if one answers, and otherwise
read the store and print one dim line about what only a host can see (the runs it drives, GitHub,
the gate probe). A command that starts, steers or stops an agent needs the host and says *no host is
running — start one with `devplane serve`, or `devplane open` for the workbench*. `devplane answer`
reaches a held permission with nothing running, because the hook that raised it polls the store.

## Naming a run or a change

Wherever a command takes `<run>`, give what `devplane ls` printed: the full id, any unambiguous
prefix, or the session's name.

```console
$ devplane ls
core-lib
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace

$ devplane show a1
```

An ambiguous prefix is an error that lists the matches. A live session wins over one that never
reported. Change ids (`<change>`) resolve by prefix the same way. `devplane change prompt` and
`devplane snooze` take either a change or a run.

## The groups

`devplane --help` lists the commands under five groups:

| Group | Commands |
|---|---|
| **See what is happening** | `ls` `show` `watch` `search` `open` |
| **What needs you, and what happened without you** | `inbox` `asks` `answer` `attention` `audit` `modes` `issues` `prs` `snooze` |
| **Start and steer work** | `change` `report` `attach` `focus` `gate` `rewind` |
| **Set up a project** | `connect` `disconnect` `trust` `check` `explain` `rules` `speckit` `agents` `doctor` `completions` |
| **The host** | `serve` `quit` `app` |

Three commands are for machines and are not listed: `devplane mcp` (below), and `devplane hook` and
`devplane statusline`, which `devplane connect` installs.

## See what is happening

### `devplane ls`

Sessions in play, and anything asking for you. Alias `ps`; plain `devplane` does the same.

| Flag | What |
|---|---|
| `-a`, `--all` | include sessions that have never reported anything — usually editor tabs left open |
| `-p`, `--project <name>` | one project; matches any part of the name, so `mat` finds `matter-kit` |
| `--needs-you` | only sessions waiting on a person |

### `devplane show <run>`

One run: state, agent, model, surface, cost, tool calls, what it is blocked on, the agent's plan,
recent tools and, for a driven run, its last lines. The `tasks` line says which tasks the run was
sent, that it was sent none, or that it was watched rather than started.

### `devplane watch [RUN]`

With no run: follow events as they arrive. With a run: print what that driven agent says, like
`tail -f`. Needs the host. Only driven runs have a conversation here; for a session you started,
`devplane focus` raises its window.

| Flag | What |
|---|---|
| `--thinking` | include the agent's reasoning, where it streams any |
| `--history <n>` | how much of the conversation to print first (default 40) |

### `devplane search <query>`

Full-text search over tool calls, questions and errors across every session. The text is a phrase,
not a query language.

### `devplane open`

Open the [workbench](@/docs/workbench.md) in a browser. With no host running, this **is** the host,
in the foreground until ctrl-c. The token is handed over once in the URL and stripped from the
address bar.

## What needs you, and what happened without you

### `devplane inbox`

What needs a person, most urgent first: by band, then oldest first. Derived from state, so it is
correct after a restart.

| Band | What is in it |
|---|---|
| stops without you | a question or permission an agent is waiting on |
| already stopped | an abandoned question, a run that is gone |
| broke after the fact | a gate red after the agent finished; a run that failed |
| ready to decide | a verified change waiting to be finished or offered |
| owed by you | a review requested, an issue assigned, an open question in a specification |
| worth knowing | drift, overlap, a cost anomaly, the machine's own health |

| Flag | What |
|---|---|
| `-p`, `--project <name>` | one project, matched like `ls --project` |
| `--needs-you` | only what can be answered from here — a question with a reply, not a red gate |

A narrowing is never remembered, and a narrowed list says how many items it hides. Above twelve
rows, interchangeable items (issues assigned, stalled sessions) fold into one counted row; questions,
permissions, drift and reports never fold.

```console
$ devplane inbox
since you last looked · 16h

 ! Keep the legacy /v1/login route? [question] saas 3m new
   Gate failed after the agent claimed completion [gate_failed] core-lib 5h
```

`new` marks what was raised since you last read the inbox. When nothing needs you, it says how many
decisions were taken in your name and by whom.

A permission item names the narrowest rule that would stop it being asked again, and the settings
file to paste it into. Nothing is written for you.

Two items are about Devplane itself: `gate_down` (the installed permission gate is not answering) and
`config_broken` (a `devplane.toml` will not load, so every gated call there goes to a person).

### `devplane asks`

Everything an agent asked you: open ones first, oldest first, then settled ones with what ended them
— **you**, **a clock** set in [`[questions] deadline`](@/docs/configuration.md#questions), or
**nobody**. Then questions an agent moved past without an answer, with what it did instead.

### `devplane answer <ask>`

Answer a permission or a question. The id comes from `devplane inbox`, not a session id: it outlives
the process that asked, so an answer given tomorrow reaches the agent through a resumed session.

| Flag | What |
|---|---|
| `--allow` | allow it — a permission |
| `--deny` | refuse it — a permission; the default when neither is given |
| `--option <text>` | one of the options the agent offered, as it wrote it |
| `--custom <text>` | your own words, where the agent offered an *Other* box; wins over `--option` |
| `--field <id>` | which question, when the agent asked several at once |

**`--allow` is refused inside an agent session** (`CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`,
`DEVPLANE_RUN`, or the Codex or Copilot session variables set). This stops the easy path, not a
hostile process; see [Security](@/docs/security.md). There is no dismiss. The first answer wins; a
second is told who gave the first.

### `devplane attention`

Per inbox kind: how often it was raised, acted on, dismissed (snoozed) or resolved elsewhere. A kind
that is mostly dismissed is too loud. No number measures you.

| Flag | What |
|---|---|
| `--days <n>` | how far back to look (default 7) |

### `devplane audit [ABOUT]`

What Devplane decided, and on whose authority: `person`, `rule`, `timer`, `nobody` or `devplane`.
`ABOUT` narrows to one run or change. See [the decision log](@/docs/decisions.md).

| Flag | What |
|---|---|
| `--without-me` | only what was decided instead of you — a rule, a clock, or nobody |
| `--limit <n>` | how many rows (default 50) |
| `--otel` | the rows as OpenTelemetry GenAI `gen_ai.tool.call.decision` log records (OTLP/JSON on stdout), with the authority as an extra attribute. Nothing is sent |

A row for an MCP tool call says where that server was defined (`project`, `user`, `plugin`, `sdk`),
or that it was not reported.

### `devplane modes`

Which live sessions decide without you, and in what permission mode, least supervised first. An
unrecognised mode shows as unknown. *Seen* is when Devplane first heard the session at that mode. It
also reports a question auto-continue timer (`askUserQuestionTimeout`, or `CLAUDE_AFK_TIMEOUT_MS` in
a session's environment) and who set it. Devplane never sets a mode.

### `devplane issues`

Every open GitHub issue across registered projects, read through your own `gh`, what needs you
first. The host refreshes it every five minutes; with no host it says so.

| Flag | What |
|---|---|
| `--ready` | only this repository's issues carrying `[github] ready_label`: the list `change start --issue` picks from |
| `--cwd <path>` | which repository; implies `--ready` |
| `--label <label>` | default `[github] ready_label`; implies `--ready` |

### `devplane prs`

Every open pull request across registered projects. `◆` marks what waits on you: a review requested
of you, or your own pull request that is red, has changes requested, or is approved and unmerged.
Writes nothing to GitHub.

### `devplane snooze <id>`

Hide a run's or a change's inbox items for a while. It covers only the kinds showing when you snooze;
anything new still shows. The run keeps running.

| Flag | What |
|---|---|
| `--minutes <n>` | default 60; `0` un-snoozes |

## Start and steer work

### `devplane change`

A change is one isolated checkout, an agent in it, and the project's `[gates] check` run when the
agent says it is finished. Its state is `drafted`, `isolated`, `in flight`, `verified`, `offered` or
`archived`. See [Verified done](@/docs/verified-done.md).

#### `devplane change start [TITLE]...`

```sh
devplane change start "add rate limiting to /login"
devplane change start --spec specs/001-password-reset --task REQ-3 "password reset"
devplane change start --project api --project web "bump tokio to 1.40"
```

The title becomes the branch name and the first prompt.

| Flag | What |
|---|---|
| `--agent <id>` | overrides `[project] default_agent`: `claude`, `codex`, `opencode`, `copilot`, `gemini`, anything in `agents.toml`, or a command |
| `--project <name\|path>` | a registered project or a path; repeatable, to send one prompt to several. Default: the current directory |
| `--no-worktree` | work in the repository itself; the change is marked *in place* and the diff is against the working tree |
| `--issue <n>` | start from a GitHub issue; its title and body become the change, marked as an untrusted report |
| `--spec <path>` | the specification this change answers, a file or folder relative to the repository; stamped onto the certificate |
| `--task <selector>` | repeatable; needs `--spec`. A requirement token (`REQ-3`: every task line citing it) or `file:line` |

Every project is checked first (trusted, clean, `devplane.toml` readable, the prompt resolvable) and
**every refusal is reported at once**; one refused project starts none. A `--task` matching nothing
is refused the same way, naming the tasks that exist. The preflight names what `[workspace] setup`
installs, or warns that a lockfile is present and no setup is declared. See
[Specifications](@/docs/specs.md).

#### `devplane change list`

Every change, with its state and what it is waiting on (*installing · `pnpm install` · 1m 12s*,
*gates running*, *feedback round 2*, *needs you*). Alias `ls`.

#### `devplane change show <change>`

State, gate standing (*verified*, *stale* with both tree digests, *failed*, *not run*, *no gates
declared*), runs, cost against the budget, and what each check said. With a specification, task
counts and any drift:

```console
tasks      11 ticked · 9 verified
REQ-3      3 tasks · 2 ticked · 0 verified
drift      the specification changed 18m into run r-9f2 and the run never saw it
           devplane change drift c-01a0 --run r-9f2 --accept | --tell
```

#### `devplane change verify <change>`

Run `[gates] check` now and record the report. The change is verified only while that report passed
and its tree digest equals the working tree's.

#### `devplane change retry <change>`

Hand the failures back to the agent once more, past `max_feedback_rounds`. Recorded as your decision.

#### `devplane change resume <change>`

Reconnect to the same agent conversation after the host stopped, so the work is not paid for twice.
Offered in the inbox only when the agent can resume. Never automatic.

#### `devplane change review <change>`

The change read for a merge decision: **checks weakened or changed** first (added skip markers,
deleted test files, edits to the gates or CI config), then the gate standing, then every file in the
order `[review] roles` declares, each with its role, whether a declared test covers it, the decisions
taken while it was written, and its task. No score.

| Flag | What |
|---|---|
| `--by risk` | declared role order (the default) |
| `--by intent` | grouped by the run that wrote each file and the tasks it was sent; files nobody asked for last |

#### `devplane change export <change>`

Print the done certificate: what was checked, against which commit and tree digest, and the commands
to re-run it with git and a shell. It flags a commit no remote has, which attempt passed, unticked
tasks beside a green gate, and whether the change edited a check.

```sh
devplane change export c-01a0 > cert.md
```

`--json` gives the same facts as an unsigned [in-toto statement](https://in-toto.io/Statement/v1).

#### `devplane change finish <change>`

Accept a change as finished and record the basis: verified, stale, no gates declared, or your
decision anyway. Removes nothing.

#### `devplane change drift <change> --run <run> (--accept | --tell)`

Decide about a specification that moved under a run. `--tell` prompts the run with the changed files
and resumes it if its agent is gone; `--accept` moves the change's start forward to what the run saw.
Recorded as your decision. Needs the host.

#### `devplane change adopt <branch>`

Make a hand-made branch a change. Refuses a missing branch and the base branch. The worktree is the
checkout that already has the branch, or a fresh one under `.claude/worktrees/`.

| Flag | What |
|---|---|
| `--project <root>` | the repository; default the current directory's |
| `--spec <path>` | the specification it answers, relative to the repository |
| `--title <text>` | default: the branch's first commit subject |

#### `devplane change archive <change>`

Remove the worktree and keep the record. Refuses uncommitted or unmerged work unless told otherwise;
a refusal changes nothing.

| Flag | What |
|---|---|
| `--delete-branch` | delete the branch too; refused while it has commits its base lacks, unless its pull request merged |
| `--discard-uncommitted` | remove the worktree even with uncommitted or untracked files |
| `--force` | with `--delete-branch`: delete it even with unmerged commits |

#### `devplane change offer <change>`

Push the branch and open the pull request when `[github] pull_request = true` in your checkout.
Otherwise it pushes nothing and prints the `git push` and `gh pr create` lines.

#### `devplane change prompt <change|run> [TEXT]...`

Send the agent a message without stopping it. Given a change, its latest run is told. Mid-turn it is
queued until the turn ends; it is recorded as sent either way.

#### `devplane change stop <run>`

Stop a run. It says first what survives (the change, worktree, branch and record), then asks on a
terminal and proceeds in a script.

### `devplane report`

A finding about another project. It reaches that project's person, never its agent, unless that
project's [`[reports] deliver_from`](@/docs/configuration.md#reports) names this one. See
[Reports between projects](@/docs/reports.md).

| Subcommand | Does |
|---|---|
| `devplane report file` | file a report (flags below) |
| `devplane report ls` | reports waiting for an answer; `--all`, `--to-me`, `--from-me` |
| `devplane report show <id>` | where it came from, where it went, what became of it, the report quoted |
| `devplane report start <id>` | start a change in the target project, the report quoted as the prompt; `--agent`. Offering or finishing it answers the report as fixed |
| `devplane report reject <id> --reason <text>` | refuse it; the filing project is told why |
| `devplane report defer <id> --reason <text>` | put it off; the filing project is told why |
| `devplane report fixed <id> [--reason <text>]` | answer it as fixed by hand |
| `devplane report open <id>` | show a GitHub draft, ask, then open it with your own `gh` — the only command that writes to a forge |
| `devplane report discard <id>` | throw a GitHub draft away; nothing was sent |

```sh
devplane report file --to core-lib --kind defect --title "client retries on 4xx" \
  --finding "retry() does not check the status" --command "cargo test -p client" \
  --output-file out.txt --path src/client.rs
```

The origin is read from the environment (`DEVPLANE_RUN`, then `CLAUDE_CODE_SESSION_ID`), never from
an argument. With neither set, `--as-person` files as you; otherwise it is refused as anonymous.

| Flag | What |
|---|---|
| `--to <target>` | required: a registered project, `owner/name`, or a GitHub URL |
| `--kind <kind>` | required: `defect`, `request`, `question` or `breaking` |
| `--title`, `--finding` | required |
| `--forge` | draft an issue on the registered project's GitHub remote instead |
| `--keep` | a target that is neither stays on the change it came from |
| `--command`, `--output-file`, `--stack-file`, `--commits a..b`, `--path` | evidence; `--path` is repeatable. 4 KiB a field, paths inside the source project |
| `--as-person` | you are filing this yourself |

### `devplane attach <run>`

Hand this terminal to the agent, resuming its session (`claude --resume <id>`, or `claude attach
<id>` for a background session). Replaces this process rather than nesting.

### `devplane focus <run>`

Raise the editor window that owns a run's directory. When none has it open, it says so and prints the
resume command.

### `devplane gate run`

Run this repository's gates and report what they exited with. Needs no host, records nothing, reads
no specification. A [Spec Kit hook](@/docs/specs.md) calls it.

| Flag | What |
|---|---|
| `--cwd <path>` | which repository; default the working directory |
| `--name <gate>` | run one `[gates.named.<gate>]` instead of `check` |

| `state` | Exit | Means |
|---|---|---|
| `verified` | 0 | every command passed |
| `failed` | 1 | one did not, and it is named |
| `no_gates` | 1 | the repository declares no checks |
| `config_unreadable` | 1 | `devplane.toml` would not parse, so nothing ran |
| `unknown_gate` | 1 | `--name` asked for a gate nothing declares; the declared names are printed |

A named gate never makes a change verified; only `check` defines done. `devplane change verify`
records a report on a change.

```console
$ devplane gate run --name bench
ok        cargo bench --no-run exit 0

bench passed
```

### `devplane rewind <run>`

Which of a Claude Code session's files its `/rewind` checkpoint will **not** restore: files a shell
command named for writing. It says *named for writing*, never *changed*, because the gate sees a call
before it runs.

## Set up a project

### `devplane connect <provider>`

Install Devplane's hooks: `claude` (hooks and telemetry in your user settings, with a backup), `codex`
(merged into `~/.codex/hooks.json`) or `copilot` (one file in `~/.copilot/hooks/`). It shows the diff
and writes only when you confirm. Nothing is started.

| Flag | What |
|---|---|
| `--statusline` | also wrap your status line, the only source of subscription rate limits: `devplane connect --statusline claude` |
| `-y`, `--yes` | write without asking |

Codex runs a hook only after you approve it in its own dialog. For Copilot, `connect copilot` prints
three environment lines to export. See [Watching sessions](@/docs/observe.md).

### `devplane disconnect <provider>`

Remove exactly what `connect` installed for `claude`, `codex` or `copilot`. `-y` writes without
asking.

### `devplane trust [path]`

Allow Devplane to start agents in a repository. It first prints the repository's own hooks and MCP
servers, which a headless agent runs without asking:

```console
$ devplane trust .
  starting an agent here loads this repository's own:

  hook    ./scripts/guard.sh
          runs on every PreToolUse in this repository
  mcp     notes
          `notes-mcp` has no version pin, so starting an agent fetches and
          runs whatever is published at that name today

  Trust this repository? [y/N]
```

| Flag | What |
|---|---|
| `--dry-run` | print the same and trust nothing |
| `-y`, `--yes` | trust without asking |

Without `--yes`, a non-interactive stdin is an error.

### `devplane check [path]`

Read `devplane.toml` and say what it will do: does it parse, does everything it names exist, is
anything unsafe. It also names a rule that covers nothing, a rule that grants more than it reads as
granting, and a path denied for reading that is still writable. Offline, and exits non-zero on an
error, so run it in CI. See [Configuration](@/docs/configuration.md).

### `devplane explain [CALL]...`

What the permission gate would decide about one call, and which rule says so. Offline: no host, no
agent, nothing recorded.

```console
$ devplane explain 'git push --force origin main'
ask  Bash
        by Bash(git push *)

$ devplane explain 'rm$IFS-rf node_modules'
ask — unreadable  Bash
        because the program is named by a variable the shell expands; …

$ devplane explain 'cargo test'
undecided  Bash
        its rules loaded and none answers this one, so the provider's own dialog decides and it reaches your inbox
```

The answers are `deny`, `ask`, `ask — unreadable`, `undecided`, and `unresolved` when the project's
rules will not load. There is no `allow`. See [Permissions](@/docs/permissions.md).

| Flag | What |
|---|---|
| `--tool <name>` | default `Bash`. `Read`, `Edit` and `WebFetch` take their specifier directly: `devplane explain --tool Read .env` |
| `--input <json>` | the whole tool input: `--tool Agent --input '{"isolation":"worktree"}'` |
| `--dir <path>` | the directory the agent would work in, which decides whose rules apply (default `.`) |
| `--replay` | replay every observed call against the current rules |
| `--limit <n>` | how many recent calls `--replay` reads (default 5000) |

`--replay` groups the calls no rule decides by the allow rule that would cover them, most
interruptions first, for your agent's `permissions.allow`. A rule is offered only after a command
interrupted you three times. It writes nothing.

### `devplane rules [RULE]`

Which registered projects are missing a rule, in `devplane.toml` (what Devplane refuses) and in the
agent's `.claude/settings.json` (what the agent refuses), reported separately.

```console
$ devplane rules 'Bash(curl:*)'
Bash(curl:*)

  saas      devplane  missing     /Users/you/saas/devplane.toml
  saas      agent     has it      /Users/you/saas/.claude/settings.json
  core-lib  devplane  covered     by `Bash(*)`
  ai-tool   devplane  unreadable  expected `=` at line 4 — run `devplane check`
```

It ends with what to paste into which file. `covered` means a wider rule already answers every call
yours names. With no rule, it reports the rules some projects have and others lack. It writes nothing.

| Flag | What |
|---|---|
| `--ask` | the rule belongs in the ask list, which changes the key the paste names |

### `devplane speckit install`

Register `devplane gate run` as a Spec Kit extension hook in `.specify/extensions.yml`. It writes the
file only when there is none; otherwise it prints the entry and where to add it. See
[Specifications](@/docs/specs.md).

| Flag | What |
|---|---|
| `--event <hook>` | which hook point; default `after_implement` |
| `--dry-run` | print and write nothing |
| `--anyway` | write the hook even though the repository declares no gate |

### `devplane agents`

The agents Devplane can drive and, for those it has started, what each advertised at its handshake
(`resume`, `load`, `list`, modes, sign-in) with the date. See [Driving agents](@/docs/agents.md).

### `devplane doctor`

Whether Devplane is working. Alias `diagnostics`. It reports:

- the host: running, with port and version, or not;
- which model provider you are on, which decides which Claude Code surfaces exist;
- the hook gate, **run** with a real `PreToolUse` payload against a throwaway deny rule, with its
  latency, or `INSTALLED AND NOT ANSWERING`;
- what is watched per vendor and channel: `read`, `unproved`, `unbuilt`, `not published`, `not
  checked`;
- the Claude Code release the rule syntax was modelled on;
- any project whose `devplane.toml` will not load, and `app.toml` if it will not parse.

The probe is recorded nowhere.

### `devplane completions <shell>`

```sh
devplane completions zsh  > ~/.zsh/completions/_devplane
devplane completions bash > /etc/bash_completion.d/devplane
devplane completions fish > ~/.config/fish/completions/devplane.fish
```

Needs no host; completing an id asks a running host and is silent without one. See
[Install](@/docs/install.md#shell-completion).

### `devplane mcp`

Hidden from `--help`. Serves Devplane's **read-only** surface to an agent over MCP on stdio; the
Devplane plugin registers it. To register it yourself:

```json
{ "mcpServers": { "devplane": { "command": "devplane", "args": ["mcp"] } } }
```

| Tool | Answers |
|---|---|
| `inbox` | what needs a person right now, across every project |
| `change` | a change's state, gate verdicts and specification; all open changes without an `id` |
| `explain` | what the gate would decide about a call, and which rule decides it |
| `audit` | what Devplane decided, and on whose authority |
| `reports` | what was filed against (`to`) and from (`from`) a project, and what became of each |

No tool acts. It asks a host if one answers, and otherwise reads the store. An `explain` asked here
**is** recorded in `devplane audit`. Filing a report goes through `devplane report file` in the shell,
where the origin can be checked.

## The host

The host is the long-lived process: it drives agents, polls GitHub, runs deadlines and serves the
workbench and the API. One runs per home. It publishes `~/.devplane/host.json` and is alive when
`/healthz` answers on that port. Nothing starts it automatically.

### `devplane serve`

Run the host in the foreground, until ctrl-c.

| Flag | What |
|---|---|
| `--port <n>` | default `47831`, or `DEVPLANE_PORT`; `0` asks the OS for a free port |

When the default port is held by something that is not a Devplane host, `serve` takes another and
records it; a port you name is never swapped. A second host on the same home is refused with the
running one's port and uptime.

### `devplane quit`

Quit the running host, saying first what that ends: agents Devplane started are named and stopped;
sessions you started are counted and left alone; waiting questions are counted and survive.

```text
Quitting stops 2 agents Devplane started:
  r-4f2a9c1e  claude  ~/code/api
  r-8b31d07a  gemini  ~/code/web
It leaves 3 sessions running in their own terminals untouched.
Stopped.
```

`Stopped.` prints once the port has gone quiet. With no host: *Nothing is running.*

### `devplane app`

The host with a native window: a tray item with the *needs you* count, desktop notifications, one
global shortcut (`[app] shortcut` in `~/.devplane/app.toml`), and `devplane://` links that open a
change, review, ask or run. Closing the window keeps hosting; quitting says what it stops. Nothing
starts at login.

| Flag | What |
|---|---|
| `--port <n>` | the port the host binds; overrides `[app] port` in `~/.devplane/app.toml` (default `0`, a free port) |

Only in a build with the `app` feature (`cargo install devplane --features app`). Without it the
command is listed as *Not in this build* and exits pointing at `devplane open`.

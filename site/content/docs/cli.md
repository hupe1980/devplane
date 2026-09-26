+++
title = "CLI reference"
description = "Every Devplane command and flag. Reading works with nothing running; a command that needs the host says so."
weight = 20
[extra]
group = "reference"
+++

A command that prints something a script would read takes `--json`, after the command's name
(`devplane inbox --json`, `devplane change show <id> --json`). `watch`, `open`, `answer`, `attach`,
`speckit`, `completions`, `serve` and `app` do not. Under `--json` a failure is one object on
stdout, `{"error": "…"}`, with a non-zero exit. Colour follows `NO_COLOR`, is off when
output is not a terminal, and `CLICOLOR_FORCE=1` forces it on.

**Reading needs no host.** `ls`, `inbox`, `audit`, `show`, `search`, `change list`, `change
show`, `modes`, `agents` and `attention` ask a running host if one answers, and otherwise
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
| **What needs you, and what happened without you** | `inbox` `answer` `attention` `audit` `modes` `forge` `snooze` |
| **Start and steer work** | `change` `report` `attach` `gate` `rewind` |
| **Set up a project** | `connect` `disconnect` `trust` `check` `explain` `speckit` `agents` `doctor` `login` `logout` `completions` |
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
| `--json` | the same, as JSON |

### `devplane show <run>`

One run: state, agent, model, surface, cost, tool calls, what it is blocked on, the agent's plan,
recent tools and, for a driven run, its last lines. The `tasks` line says which tasks the run was
sent, that it was sent none, or that it was watched rather than started.

### `devplane watch [RUN]`

With no run: follow events as they arrive. With a run: print what that driven agent says, like
`tail -f`. Needs the host. Only driven runs have a conversation here; for a session you started,
`devplane attach` resumes it in a terminal. No `--json`.

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
| ready to decide | a verified change waiting to be reviewed; never offered from the inbox |
| owed by you | a review requested, an issue assigned, an open question in a specification |
| worth knowing | drift, overlap, a cost anomaly, the machine's own health |

| Flag | What |
|---|---|
| `-p`, `--project <name>` | one project, matched like `ls --project` |
| `--needs-you` | only what can be answered from here — a question with a reply, not a red gate |
| `--all` | everything an agent asked you instead, settled ones too (below) |

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

#### `devplane inbox --all`

Everything an agent asked you: open ones first, oldest first, then settled ones with what ended them
— **you**, **a clock** set in [`[questions] deadline`](@/docs/configuration.md#questions), or
**nobody**. Then questions an agent moved past without an answer, with what it did instead.

### `devplane answer <ask>`

Answer a permission or a question. The id comes from `devplane inbox`, not a session id: it outlives
the process that asked, so an answer given tomorrow reaches the agent through a resumed session.

| Flag | What |
|---|---|
| `--allow` | allow it — a permission |
| `--deny` | refuse it — a permission |
| `--option <text>` | one of the options the agent offered, as it wrote it. It is the whole answer, so it cannot be combined with `--allow` or `--deny` |
| `--custom <text>` | your own words, where the agent offered an *Other* box; wins over `--option` |
| `--field <id>` | which question, when the agent asked several at once. Questions only: `--allow` and `--deny` refuse it |

One of `--allow`, `--deny`, `--option` or `--custom` is required; with none, nothing is sent. No
`--json`.

**`--allow` is refused inside an agent session** (`CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`,
`CLAUDE_CODE_ENTRYPOINT`, `DEVPLANE_RUN`, or the Codex or Copilot session variables set), and so is an
`--option` given without `--field`, which could be a permission's allow. This stops the easy path, not a
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

### `devplane forge`

Open GitHub issues and pull requests across registered projects, read with your GitHub sign-in, what
needs you first. The host refreshes them every five minutes, and at once after a sign-in; with no
host it says so. Writes nothing to GitHub. Each project is listed under its own heading with the
state its GitHub is in — not signed in to its host, expired, rate limited, unreachable, stale, or not
a GitHub repository — and how many more are open past the page that was read. Both take `--json` and
print only their own list, with that per-project state: `devplane forge prs --json`.

#### `devplane forge issues`

Every open issue across registered projects.

| Flag | What |
|---|---|
| `--ready` | only this repository's issues carrying `[github] ready_label`: the list `change start --issue` picks from. Without it, every project's open issues |
| `--cwd <path>` | which repository; implies `--ready` |
| `--label <label>` | default `[github] ready_label`; implies `--ready` |

#### `devplane forge prs`

Every open pull request across registered projects. `◆` marks what waits on you: a review requested
of you, or your own pull request that is red, has changes requested, or is approved and unmerged.

### `devplane snooze <id>`

Hide a run's or a change's inbox items for a while. It covers only the kinds showing when you snooze;
anything new still shows. The run keeps running.

| Flag | What |
|---|---|
| `--minutes <n>` | default 60; `0` un-snoozes. Anything past 30 days is taken as 30 days |

## Start and steer work

### `devplane change`

A change is one isolated checkout, an agent in it, and the project's `[gates] check` run when the
agent says it is finished. Its state is `drafted`, `isolated`, `in flight`, `verified`, `offered` or
`archived`. See [Verified done](@/docs/verified-done.md).

#### `devplane change start [TITLE]...`

```sh
devplane change start "add rate limiting to /login"
devplane change start --spec specs/001-password-reset --task REQ-3 \
  "password reset"
devplane change start --project api --project web "bump tokio to 1.40"
```

The title becomes the first prompt and, slugged, the branch: `change/<slug>` (for example
`change/add-rate-limiting-to-login-3f9a1c`). Every `change` subcommand takes `--json`.

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
tasks      11 ticked · 9 seen by a passing check
REQ-3      3 tasks · 2 ticked · 0 seen by a passing check
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

The change read for a merge decision: **checks weakened or changed** first (skip markers, removed
assertions, deleted tests, suppressions, edits to the gates, CI or a checker's configuration — each
with what it matched and whether it was read), then the gate standing, then every file in the order
`[review] roles` declares, each with its role, whether a declared test covers it, the decisions taken
while it was written, and its task. No score.

| Flag | What |
|---|---|
| `--by risk` | declared role order (the default) |
| `--by intent` | grouped by the run that wrote each file and the tasks it was sent; files nobody asked for last |
| `--seen <path>` | mark every weakened row at that path read, as yours (repeatable); refused for a path with none, and from inside an agent's session |

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

Push the branch and open the pull request when `[github] pull_request = true` in your checkout; a
branch that already has an open pull request records that one instead. Otherwise it pushes nothing and prints the `git push` line and the address of GitHub's own compare
page for the branch, where you open the pull request yourself. Refused, with exit
status `3`, while any weakened-check row is unseen: it names every row and the `change review --seen`
line that marks one read (`--json` prints `refused`, `rows`, `seen_with`). Refused from inside an
agent's session, as `finish` and `archive` are.

#### `devplane change prompt <change|run> [TEXT]...`

Send the agent a message without stopping it. Given a change, its latest run is told. Mid-turn it is
queued until the turn ends; it is recorded as sent either way.

#### `devplane change stop <run>`

Stop a run. It says first what survives (the change, worktree, branch and record), then asks on a
terminal. Anywhere else (a script, a pipe) it refuses unless `--yes` says so.

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
| `devplane report open <id>` | show a GitHub draft, ask, then open it as an issue under your GitHub sign-in; `--yes` where nobody can answer |
| `devplane report discard <id>` | throw a GitHub draft away; nothing was sent |

```sh
devplane report file --to core-lib --kind defect \
  --title "client retries on 4xx" \
  --finding "retry() does not check the status" \
  --command "cargo test -p client" \
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

### `devplane gate`

Run this repository's gates and report what they exited with. Needs no host, records nothing, reads
no specification. A [Spec Kit hook](@/docs/specs.md) calls it.

| Flag | What |
|---|---|
| `--cwd <path>` | which repository; default the working directory |
| `--name <gate>` | run one `[gates.named.<gate>]` instead of `check` |
| `--json` | `state`, `passed`, `summary` and `commands` |

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
$ devplane gate --name bench
ok        cargo bench --no-run exit 0

bench passed
Not recorded: this ran in a repository rather than against a change. `devplane change verify <id>` is the one that leaves a row.
```

### `devplane rewind <run>`

Which of a Claude Code session's files its `/rewind` checkpoint will **not** restore: files a shell
command named for writing. It says *named for writing*, never *changed*, because the gate sees a call
before it runs.

## Set up a project

### `devplane connect <provider>`

Install Devplane's hooks: `claude` (hooks and telemetry in your user settings, with a backup), `codex`
(merged into `~/.codex/hooks.json`) or `copilot` (one file in `~/.copilot/hooks/`). Nothing is
started.

It shows the diff first. On a terminal it asks; anywhere else (a script, a pipe, an agent's shell)
and under `--json` it writes nothing unless you pass `--yes`.

Every hook runs the binary by its full path, so `connect` refuses a binary that lives somewhere
temporary: npx's package cache, a macOS App Translocation path, a mounted volume under `/Volumes`,
or an AppImage's mount. Install `devplane` somewhere permanent and connect from there, or name one
with `--bin`. After moving or reinstalling the binary, run `connect` again.

| Flag | What |
|---|---|
| `--statusline` | also wrap your status line, the only source of subscription rate limits: `devplane connect --statusline claude` |
| `-y`, `--yes` | write without asking; required when stdin is not a terminal, and under `--json` |
| `--bin <path>` | the `devplane` binary the hooks run; default this one |
| `--json` | the diff and what was written |

Codex runs a hook only after you approve it in its own dialog. For Copilot, `connect copilot` prints
three environment lines to export. See [Watching sessions](@/docs/observe.md).

### `devplane disconnect <provider>`

Remove exactly what `connect` installed for `claude`, `codex` or `copilot`. It shows the diff, asks on
a terminal, and needs `-y` (`--yes`) anywhere else. Hooks you configured yourself are left alone.

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

The table prints `deny`, `ask`, `ask — unreadable` (a line that hides what it runs, or rules that
will not load) or `undecided`; `--json` calls the unreadable case `unresolved`. There is no `allow`.
See [Permissions](@/docs/permissions.md).

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

### `devplane speckit`

Register Devplane's gate as a Spec Kit extension hook in `.specify/extensions.yml`. The hook names the
`devplane-gate` skill, which runs `devplane gate`. It needs Spec Kit initialised (`.specify/`), and it
writes the file only when there is none; otherwise it prints the entry and where to add it. It refuses
when the repository declares no gate, or when no `devplane-gate` skill is at
`.claude/skills/devplane-gate/SKILL.md` in the repository or your home; `--dry-run` applies the same
refusals. See [Specifications](@/docs/specs.md#the-gate-inside-spec-kit-s-workflow).

| Flag | What |
|---|---|
| `--event <hook>` | which hook point; default `after_implement` |
| `--dry-run` | print and write nothing |
| `--anyway` | write the hook even though the repository declares no gate |

### `devplane agents`

The agents Devplane can drive and, for those it has started, what each advertised at its handshake
(`resume`, `load`, `list`, modes, sign-in) with the date. See [Driving agents](@/docs/agents.md).

### `devplane login github`

Sign in to GitHub by its device flow: Devplane prints a short code and GitHub's device address, opens
the address where it can, and waits while you enter the code there. It never sees your password. The
token goes into the operating system's credential store (Keychain, Credential Manager, Secret
Service) under service `devplane`, account `github:<host>`, and nowhere else; with no credential
store, sign-in refuses rather than write a file. When a host runs, the window's **Setup** shows the
same code.

| Flag | |
|---|---|
| `--host <host>` | a GitHub Enterprise server instead of the configured host (`[github] host` in `~/.devplane/app.toml`, default `github.com`) |
| `--with-token` | read a token from stdin — never an argument — verify it and store it: for a server with no registered app, and for CI |
| `--json` | a `{ "state": "pending", "user_code", "verification_uri", "expires_at" }` line first, then `{ "host", "login", "scopes" }` |

With no GitHub app registered for the host, the device flow is refused and the refusal names the way
that works without one: a token on stdin.

```sh
gh auth token | devplane login github --with-token   # GitHub CLI's token
devplane login github --with-token < pat.txt         # a fine-grained personal access token
```

An Enterprise server registers its own app; name it per host in `~/.devplane/app.toml` as
`[github.hosts."ghe.corp"] client_id = "…"` (or `[github] client_id` for the configured host). A
build's own app is used for `github.com` only. A sign-in finished here while a host runs is handed to
it, and the Forge is read at once.

### `devplane logout github`

Deletes the token from the credential store and every copy Devplane holds; the Forge view reads *not
signed in* at its next poll. A host nobody signed in to has nothing to delete, and the command says
so. `--host` for an Enterprise server. The grant still exists at GitHub,
and the command says where to revoke it (`https://<host>/settings/applications`).

### `devplane doctor`

Whether Devplane is working. Alias `diagnostics`. It reports:

- the host: running, with port and version, or not;
- which model provider you are on, which decides which Claude Code surfaces exist;
- the hook gate, **run** with a real `PreToolUse` payload against a throwaway deny rule, with its
  latency, or `INSTALLED AND NOT ANSWERING`;
- **gate off** for any hook event whose binary no longer exists (an npx cache cleared, a binary
  moved), and **out of date** when the installed hooks are not the ones this build writes;
- per GitHub host, whom it is signed in as, the scopes and when GitHub last answered — never the
  token — and each project that is not a GitHub repository, with the reason (`--json` carries the
  hosts as `github`, whether or not a host runs);
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

Needs no host; completing an id asks a running host, or else reads the store (never starting a host),
and gives up after 150 ms. See
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

A port held by another process is a refusal, never a quiet move elsewhere: the agents' settings send
the bearer token to that port, so a host on another one would leave the token going to whatever holds
it. The refusal names the holder where it can. Stop that process, or pick a free port and run
`devplane connect` again so the settings follow. A second host on the same home is refused with the
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

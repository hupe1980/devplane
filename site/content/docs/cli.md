+++
title = "CLI reference"
description = "Every Devplane command and flag. Reading works with nothing running; a command that needs the host says so."
weight = 20
[extra]
group = "reference"
+++

Commands are nouns with verbs: `devplane run show`, `devplane inbox answer`. Plain `devplane` runs
`devplane status`. A verb typed without its noun (`devplane connect`) is refused, naming the right
command.

- **`--json`** works on every command, before or after its name. A failure is `{"error": "…"}` on
  stdout with a non-zero exit. It changes nothing for `run watch`, `run attach`, `gate install`,
  `completions` and `up`.
- **Colour** follows `NO_COLOR` and is off when output is not a terminal; `CLICOLOR_FORCE=1` forces it.
- **Reading needs no host.** `status`, `run list`, `run show`, `inbox`, `audit`, `search`, `change
  list`, `change show` and `agent list` read the store when no host answers, and say what only a host
  sees. Starting, steering or stopping an agent needs the host. `inbox answer` works without one.
- **Exit codes**: `0` success, `1` failure, `2` a usage error, `3` a refusal by `change draft` or
  `change offer`. [`devplane verify`](#devplane-verify) and [`devplane gate`](#devplane-gate) have
  their own.

## Naming a run, a change or an ask

Every id takes the full id or any unique prefix; a run also takes its session name.

```console
$ devplane status
core-lib
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace

$ devplane run show a1
```

An ambiguous prefix is an error listing the matches; a live session wins over one that never
reported. `change prompt`, `change stop` and `inbox snooze` take a change or a run.

## The groups

`devplane --help` lists the commands under five groups:

| Group | Commands |
|---|---|
| **See what is happening** | `status` `run` `search` |
| **What needs you, and what happened without you** | `inbox` `audit` `pr` `issue` |
| **Start and steer work** | `change` `gate` `verify` |
| **Set up a project** | `project` `config` `agent` `auth` `doctor` `completions` |
| **The host** | `up` `down` |

Hidden, for machines: `devplane mcp` ([below](#devplane-mcp)), and `devplane hook` and `devplane
statusline`, which `devplane agent connect` installs.

## See what is happening

### `devplane status`

Sessions in play, and anything asking for you.

| Flag | What |
|---|---|
| `-a`, `--all` | include sessions that never reported anything (idle editor tabs) |
| `-p`, `--project <name>` | one project; matches any part of the name (`mat` finds `matter-kit`) |
| `--needs-you` | only sessions waiting on a person |
| `--modes` | each live session's permission mode, least supervised first, with when it was first seen in that mode and any question auto-continue timer (`askUserQuestionTimeout`, `CLAUDE_AFK_TIMEOUT_MS`) and who set it |

### `devplane run`

| Subcommand | What |
|---|---|
| `run list` | sessions in play, grouped by project. Alias `ls`. Takes `-a`, `-p` and `--needs-you` as `status` does |
| `run show <run>` | state, agent, model, surface, cost, tool calls, blocker, plan, recent tools, tasks sent, and a driven run's last lines |
| `run watch [run]` | without a run, follow events; with a driven run, print its conversation like `tail -f`. Needs the host |
| `run attach <run>` | hand this terminal to the agent: `claude --resume <id>`, or `claude attach <id>` for a background session. Replaces this process. Needs the host |

`run watch` flags: `--thinking` includes the agent's reasoning where streamed; `--history <n>` is how
much conversation to print first (default 40).

### `devplane search <query>`

Full-text search over tool calls, questions and errors in every session. The query is a phrase.

## What needs you, and what happened without you

### `devplane inbox`

What needs a person, by band, then oldest first. Derived from state, so correct after a restart.

| Band | What is in it |
|---|---|
| stops without you | a question or permission an agent is waiting on |
| already stopped | an abandoned question, a run that is gone |
| broke after the fact | a gate red after the agent finished; a failed run |
| ready to decide | a verified change waiting for review; never offered from the inbox |
| owed by you | a review requested, an issue assigned, an open question in a spec |
| worth knowing | drift, overlap, a cost anomaly, the machine's health |

| Flag | What |
|---|---|
| `-p`, `--project <name>` | one project, matched like `status --project` |
| `--needs-you` | only what can be answered from here (a question, not a red gate) |
| `--all` | every question and permission an agent asked, settled ones too |

```console
$ devplane inbox
since you last looked · 16h

 ! Keep the legacy /v1/login route? [question] saas 3m new
   Gate failed after the agent claimed completion [gate_failed] core-lib 5h
```

- `new` marks what arrived since you last read the inbox. An empty inbox counts the decisions taken
  in your name and by whom.
- A narrowed list counts what it hides. Filters are not remembered.
- Above twelve rows, issues and stalled sessions fold into a counted row. Questions, permissions and
  drift never fold.
- A permission item names the narrowest rule that would stop the ask. Nothing is written for you.
- `gate_down`: the permission gate is not answering. `config_broken`: a `devplane.toml` will not load.

`--all` lists open asks first, then settled ones with what ended them: **you**, **a clock**
([`[questions] deadline`](@/docs/configuration.md#questions)) or **nobody**, then questions an
agent moved past.

#### `devplane inbox answer <ask>`

Answer a permission or a question by the id `devplane inbox` prints. The id outlives the asking
process; a late answer reaches the agent through a resumed session.

| Flag | What |
|---|---|
| `--allow` | allow a permission |
| `--deny` | refuse a permission |
| `--option <text>` | one of the agent's options, as written; the whole answer, so not with `--allow`/`--deny` |
| `--custom <text>` | your own words, where the agent offered *Other*; wins over `--option` |
| `--field <id>` | which question, when several were asked; not with `--allow`/`--deny` |

- One of `--allow`, `--deny`, `--option` or `--custom` is required.
- `--allow`, and `--option` without `--field`, are refused inside an agent session
  ([Security](@/docs/security.md#an-agent-cannot-approve-itself-mostly)).
- There is no dismiss. The first answer wins; a second is told who gave the first.

#### `devplane inbox snooze <id>`

Hide a run's or a change's current inbox items for a while; new kinds still show, and the run keeps
running. A question or a permission is never snoozed. `devplane inbox` counts what is snoozed.

| Flag | What |
|---|---|
| `--minutes <n>` | default 60, at most 30 days; `0` un-snoozes |
| `--until-activity` | until the run next reports activity (a change: a run event, a check result or a moved tree); not with `--minutes` |

### `devplane audit [about]`

What Devplane decided, and on whose authority: `person`, `rule`, `timer`, `nobody` or `devplane`.
`about` narrows to one run or change. How to read it: [the decision log](@/docs/decisions.md).

| Flag | What |
|---|---|
| `--without-me` | only what a rule, a clock or nobody decided, plus the calls nobody can be named for |
| `--limit <n>` | rows (default 50) |
| `--otel` | the rows as OpenTelemetry GenAI `gen_ai.tool.call.decision` log records (OTLP/JSON on stdout). Nothing is sent |

An MCP tool call's row says where its server was defined (`project`, `user`, `plugin`, `sdk`), or
that it was not reported.

### `devplane pr list` and `devplane issue list`

Open pull requests and issues across registered projects, read with your GitHub sign-in, what needs
you first. Alias `ls`. Nothing is written to GitHub.

- The host refreshes them every five minutes and after a sign-in.
- Each project shows its GitHub state (not signed in, expired, rate limited, unreachable, stale, not a
  GitHub repository) and how many are open past the page read.
- In `pr list`, `◆` marks what waits on you: a review requested of you, or your own pull request
  that is red, has changes requested, or is approved and unmerged.

| `issue list` flag | What |
|---|---|
| `--ready` | only this repository's issues with `[github] ready_label`: what `change start --issue` picks from |
| `--cwd <path>` | which repository; implies `--ready` |
| `--label <label>` | default `[github] ready_label`; implies `--ready` |

## Start and steer work

### `devplane change`

A change is an isolated checkout, an agent in it, and `[gates] check` run when the agent finishes.
Its states are `drafted`, `isolated`, `in flight`, `verified`, `offered` and `archived`
([Verified done](@/docs/verified-done.md)). Every `<change>` is an id or unique prefix as `change
list` prints it.

Refused inside an agent session: `change resume`, `login`, `claim-read`, `draft`, `draft-accept`,
`draft-reject`, `offer`, and `review --seen`.

#### `devplane change start [title]...`

```sh
devplane change start "add rate limiting to /login"
devplane change start --spec specs/001-password-reset --task REQ-3 "password reset"
devplane change start --project api --project web "bump tokio to 1.40"
```

The title is the first prompt and, slugged, the branch (`change/add-rate-limiting-to-login-3f9a1c`).

| Flag | What |
|---|---|
| `--agent <id>` | default `[agent] default`: `claude`, `codex`, `opencode`, `copilot`, `gemini`, an `agents.toml` entry, or a command |
| `--project <name\|path>` | a registered project or a path; repeatable. Default: the current directory |
| `--no-worktree` | work in the repository itself; marked *in place*, diffed against the working tree |
| `--issue <n>` | start from a GitHub issue; its body is marked as an untrusted report |
| `--spec <path>` | the spec file or folder this change answers, relative to the repository |
| `--task <selector>` | repeatable, needs `--spec`: a requirement token (`REQ-3`) or `file:line`. A selector matching nothing refuses |

Every project is checked first (trusted, clean, readable `devplane.toml`, matching `--task`); one
refusal starts none. The preflight names `[workspace] setup`, or warns of a lockfile without one.

#### Reading a change

| Command | What |
|---|---|
| `change list` | every change, its state and what it waits on (*installing · `pnpm install` · 1m 12s*, *gates running*, *feedback round 2*, *needs you*). Alias `ls` |
| `change show <change>` | state; the agent's open spec-and-tests claim, quoted; gate standing (*verified*, *stale* with both tree digests, *failed*, *not run*, *no gates declared*); runs; cost against budget; each check's output; with a spec, task counts and drift |
| `change review <change>` | the **checks weakened or changed** ([kinds](@/docs/verified-done.md#review-before-you-merge)), the gate standing, then every file in `[review] roles` order with its role, covering test, decisions and task |
| `change export <change>` | the [done certificate](@/docs/verified-done.md#the-done-certificate) |

```console
tasks      11 ticked · 9 seen by a passing check
REQ-3      3 tasks · 2 ticked · 0 seen by a passing check
drift      the specification changed 18m into run r-9f2 and the run never saw it
           devplane change drift c-01a0 --run r-9f2 --accept | --tell
```

`change review` flags:

| Flag | What |
|---|---|
| `--by risk` | declared role order (default) |
| `--by intent` | grouped by the run that wrote each file and its tasks; unrequested files last |
| `--seen <path>` | mark every weakened row at that path read, as yours (repeatable). An offer waits until every row is. Refused for a path with nothing in the change |

`change export` flags and exits:

| Flag | What |
|---|---|
| `--json` | the [in-toto statement](@/docs/verified-done.md#the-statement-a-machine-reads): Statement v1, Test Result predicate |
| `--out <file>` | write to `<file>` instead of printing; refused if it exists |
| `--force` | replace `<file>` and `<file>.sig` |
| `--sign ssh` | needs `--out`. Sign the statement with your global git `user.signingkey` by `ssh-keygen -Y sign`, write `<file>.sig`, and print the `ssh-keygen -Y verify` lines anyone can run. Devplane never makes a key |

Exit 1, writing nothing, when the change has no statement (no `check` run, no tree recorded), when
signing fails, or when `--out` exists. Exit 2 for `--sign` without `--out`.

```sh
devplane change export c-01a0 > cert.md
devplane change export c-01a0 --sign ssh --out c-01a0.intoto.json
```

#### Steering a change

| Command | What |
|---|---|
| `change verify <change>` | run `[gates] check` now and record it. `--base-checks` follows a pass with the [base checks](@/docs/verified-done.md#base-checks); the exit code stays `check`'s |
| `change retry <change>` | one more feedback round past `max_feedback_rounds`, recorded as yours |
| `change prompt <change\|run> [text]...` | message the agent without stopping it (a change: its latest run); queued mid-turn |
| `change stop <change\|run>` | stop a run (a change: its latest). Says what survives (change, worktree, branch, record), then asks on a terminal; elsewhere it needs `-y` |
| `change resume <change> [prompt]...` | reconnect to the same agent conversation after the host stopped; never automatic. `--agent <id>` continues with another agent in the same worktree, told only your words, never the earlier transcript. Refused while a run of the change is live |
| `change login <change>` | for an agent that said it needs signing in. Without `--method <id>`, list its methods. A terminal method runs the agent's login here, then starts the change again; an agent method asks the agent to sign itself in. Never asks for, reads or stores a credential |
| `change claim-read <change>` | mark the agent's open spec-and-tests claim read; it stays on the change under its run. No decision is recorded |
| `change drift <change> --run <run> --accept\|--tell` | a spec moved under a run. `--tell` prompts the run with the changed files, resuming it if needed; `--accept` moves the change's start to what the run saw. Recorded as yours. Needs the host |
| `change adopt <branch>` | make a hand-made branch a change, in the checkout that has it or a new one under `.claude/worktrees/`. Refuses a missing branch and the base branch. `--project <root>`, `--spec <path>`, `--title <text>` (default: the first commit's subject) |

#### Drafting a specification

| Command | What |
|---|---|
| `change draft <name> -- <intent>...` | start a change whose agent is sent the project's own spec command (Spec Kit's `/speckit-specify` as that agent spells it, or OpenSpec's installed propose workflow) followed by your words, and nothing else. Refused with exit `3` before anything is made |
| `change draft-accept <change>` | the drafted folder becomes the spec the change answers |
| `change draft-reject <change>` | the worktree goes back to where it was; the change, run and decisions stay on the record |
| `change clarify <change> <path>:<line> -- <answer>...` | replace one bracketed open-question marker with your words, exactly. Prints the diff, writes on `--yes` or a `y` at the terminal. An unbracketed line is refused: use `change prompt` |

`change draft` flags: `--agent <id>`, `--methodology speckit|openspec` (needed only where the project
has both), `--project <name|path>`, `--change <change>` (draft again into a change whose draft was
rejected), `--dry-run` (print what would be sent). Accept and reject are refused while the agent is
mid-turn. Full rules: [Specifications](@/docs/specs.md#drafting-a-spec-with-the-project-s-own-tool).

#### Ending a change

| Command | What |
|---|---|
| `change offer <change>` | commit exactly the verified tree (the certificate's `gitTree`) on the change's branch, then push and open a draft pull request when `[github] pull_request = true`, else print the `git push` line and GitHub's compare address |
| `change finish <change>` | accept the change as finished, recording the basis (verified, stale, no gates declared, or by hand). Removes nothing |
| `change archive <change>` | remove the worktree, keep the record. Refuses uncommitted or unmerged work unless told; a refusal changes nothing |

`change offer` is refused with exit `3` while a weakened row is unseen (`--json`: `refused`, `rows`,
`seen_with`), and when the tree moved since the last pass
([rules](@/docs/verified-done.md#offering-finishing-archiving)).

| `change archive` flag | What |
|---|---|
| `--delete-branch` | also delete the branch; refused while it has commits its base lacks, unless its pull request merged |
| `--discard-uncommitted` | remove the worktree despite uncommitted or untracked files |
| `--force` | with `--delete-branch`: delete despite unmerged commits |

### `devplane gate`

Run this repository's gates and report the exit codes. No host, no record, no spec. A
[Spec Kit hook](@/docs/specs.md) or a `verify` skill calls it.

| Flag | What |
|---|---|
| `--cwd <path>` | which repository; default the working directory |
| `--name <gate>` | run `[gates.<gate>]` instead of `check`. A named gate never makes a change verified |

| `state` (`--json`) | Exit | Means |
|---|---|---|
| `passed` | 0 | every command passed |
| `failed` | 1 | one did not, and it is named |
| `no_gates` | 1 | no checks declared |
| `config_unreadable` | 1 | `devplane.toml` did not parse; nothing ran |
| `unknown_gate` | 1 | `--name` names no declared gate; the declared names are printed |

`--json` also prints `passed`, `summary` and `commands`.

#### `devplane gate install`

Put the gate where a workflow runs it. One of `--speckit` or `--skill` is required. Refused while the
repository declares no gate. Setup: [Specifications](@/docs/specs.md#the-gate-inside-spec-kit-s-workflow)
and [Driving agents](@/docs/agents.md#before-an-agent-commits).

| Flag | What |
|---|---|
| `--speckit` | install the Devplane extension into this repository's Spec Kit, through `specify extension add` when `specify` is on `PATH`, else the hand-written hook in `.specify/extensions.yml` |
| `--skill` | offer the gate as the Claude Code skill `verify` in `.claude/skills/`, shown as a diff and written when you confirm |
| `--user` | with `--skill`: in `~/.claude/skills/`, for every repository |
| `-y`, `--yes` | with `--skill`: write without asking |
| `--event <hook>` | with `--speckit`: the hook point; default `after_implement` |
| `--dry-run` | print, write and run nothing |
| `--anyway` | install with no gate declared, no reachable skill for the hand-written hook, or over a `verify` skill Devplane did not write |

### `devplane verify`

Run the base's `check` on this change, leading with the checks it weakened. For CI
([CI verify](@/docs/ci.md)) or before you push. Needs only git: no host, store or network unless
`--approvals` is passed.

- The change is the diff from the merge base with `--base` to the working tree, untracked files
  included.
- `check`, its timeout and review roles come from `devplane.toml` **at the merge base**. A change
  that edits `check` is judged by the base's, and the edit is the summary's first row.

| Flag | What |
|---|---|
| `--base <ref>` | any commit-ish, tried as `origin/<ref>` then `<ref>`. Default: `origin/HEAD`'s branch, else `main` |
| `--summary <path>` | write the Markdown summary there; otherwise appended to `$GITHUB_STEP_SUMMARY` if set |
| `--statement <path>` | write the in-toto statement (never signed) when `check` ran |
| `--no-base-checks` | skip the base checks that run after a pass |
| `--sarif <path>` | write the weakened rows and failing named tests as SARIF 2.1.0; nothing is uploaded |
| `--approvals` | read who approved the pull request the run's event names, with `$GITHUB_TOKEN` from `$GITHUB_API_URL`: two to twelve `GET` requests, nothing written ([details](@/docs/ci.md#who-approved-the-pull-request)) |

| Exit | Means |
|---|---|
| 0 | verified: every command exited 0 and the tree stayed put |
| 1 | failed: a command failed, timed out or never started, or the tree moved |
| 2 | could not run: no base, no shared history, no `check` at the base, an unreadable diff, or evidence that could not be written |

Weakened checks never change the exit code. `--json` prints one document: `verdict`, `exit`, `base`,
`tree`, weakened rows and each command's outcome. Under GitHub Actions, weakened rows and failing
named tests are also [annotations](@/docs/ci.md#annotations-and-sarif).

## Set up a project

### `devplane project trust [path]`

Allow agents in a repository (default: here), after printing the hooks and MCP servers an agent
would load there:

```console
$ devplane project trust .
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
| `--dry-run` | print, trust nothing |
| `-y`, `--yes` | trust without asking; required when stdin is not a terminal |

Refused inside an agent session. It writes `~/.devplane/trusted.toml` and has a running host re-read
it.

### `devplane config`

Read `devplane.toml`, ask what its rules decide, and manage your grants. Offline.

| Subcommand | What |
|---|---|
| `config check [path]` | does the file parse, does everything it names exist, is anything unsafe. Also names a rule that covers nothing, one that grants more than it reads, a read-denied path that is still writable, and a removed key with its replacement. Exits non-zero on an error ([Configuration](@/docs/configuration.md)) |
| `config explain [call]...` | what the permission gate would decide about one call, and which rule says so |
| `config grants` | the [grants](@/docs/permissions.md#grants-your-own-rule-per-project) in `~/.devplane/policy.toml`, with what each decided |
| `config revoke <rule>` | remove a grant, exactly as the file has it, and list every decision it made. `--dir <path>`: a directory in the project the grant is for (default `.`) |

```console
$ devplane config explain 'git push --force origin main'
ask  Bash
        by Bash(git push *)

$ devplane config explain 'rm$IFS-rf node_modules'
ask — unreadable  Bash
        because the program is named by a variable the shell expands; …

$ devplane config explain 'cargo test'
undecided  Bash
        its rules loaded and none answers this one, so the provider's own dialog decides and it reaches your inbox
```

`explain` verdicts are `deny`, `ask`, `ask — unreadable` (`unresolved` in `--json`), `undecided`, or
`allow` only for your own grant. See [Permissions](@/docs/permissions.md).

| `explain` flag | What |
|---|---|
| `--tool <name>` | default `Bash`. `Read`, `Edit` and `WebFetch` take the specifier directly: `--tool Read .env` |
| `--input <json>` | the whole tool input: `--tool Agent --input '{"isolation":"worktree"}'` |
| `--dir <path>` | the agent's working directory, which decides whose rules apply (default `.`) |
| `--replay` | replay every observed call against the current rules, grouping undecided calls by the allow rule that would cover them; a rule is offered after three interruptions |
| `--limit <n>` | how many recent calls `--replay` reads (default 5000) |

### `devplane agent`

| Subcommand | What |
|---|---|
| `agent list` | the agents Devplane can drive and, once started, what each advertised (`resume`, `load`, `list`, modes, sign-in), with the date. Alias `ls`. See [Driving agents](@/docs/agents.md) |
| `agent connect <vendor>` | install Devplane's hooks: `claude` (hooks and telemetry in user settings, with a backup), `codex` (merged into `~/.codex/hooks.json`) or `copilot` (one file in `~/.copilot/hooks/`). Starts nothing |
| `agent disconnect <vendor>` | remove exactly what `connect` installed; your own hooks stay |

- Both show the diff and ask on a terminal. Elsewhere, and under `--json`, they write only with `-y`.
- Hooks run the binary by full path, so a temporary binary is refused (npx's cache, macOS App
  Translocation, `/Volumes`, an AppImage mount). Run `connect` again after moving or reinstalling.
- Codex runs a hook only after you approve it in its own dialog. For Copilot, `connect` prints three
  environment lines to export. See [Watching sessions](@/docs/observe.md).

| `connect` flag | What |
|---|---|
| `--statusline` | also wrap your status line, the only source of subscription rate limits |
| `-y`, `--yes` | write without asking |
| `--bin <path>` | the binary the hooks run; default this one |

### `devplane auth`

| Subcommand | What |
|---|---|
| `auth login` | GitHub's device flow: prints a code and an address, then waits. The token goes only into the OS credential store (service `devplane`, account `github:<host>`); with no store, sign-in refuses |
| `auth logout` | delete the token everywhere Devplane holds it, and name `https://<host>/settings/applications` to revoke the grant at GitHub |

| Flag | What |
|---|---|
| `--host <host>` | a GitHub Enterprise server instead of `[github] host` in `~/.devplane/app.toml` (default `github.com`) |
| `--with-token` | `login` only: read a token from stdin, never an argument; for a host with no registered app, and CI |

```sh
gh auth token | devplane auth login --with-token  # GitHub CLI's token
devplane auth login --with-token < pat.txt        # a fine-grained personal access token
```

`login --json` prints `{ "state": "pending", "user_code", "verification_uri", "expires_at" }`, then
`{ "host", "login", "scopes" }`. A build's own OAuth app covers `github.com` only; an Enterprise app
is set per host in [`app.toml`](@/docs/configuration.md#machine-wide-files).

### `devplane doctor`

Whether Devplane works on this machine. The probe is recorded nowhere.

| Section | Reports |
|---|---|
| host | running (port, version, binary) or not |
| app | the window, signing, update checks, notifications, shortcut and port |
| watched | per vendor and channel: `read`, `unproved`, `not published` or `not checked`, with the date last read |
| gate | the rule-syntax baseline, and a timed `PreToolUse` probe |
| only gate | *Devplane is the only gate on this machine* when a provider variable (Bedrock, Vertex, Foundry, a gateway, an API key) turns Claude Code's supervision off |
| claude code | hooks, `INSTALLED AND NOT ANSWERING`, **gate off** (binary gone), **out of date**, telemetry |
| this project | a JUnit path git does not ignore |
| github | per host: login, scopes, last answer (never the token); projects that are not GitHub repositories, with why |
| channels | which are arriving, how many, worst latency, last error |
| inbox | per kind over a week: raised, acted on, dismissed, resolved elsewhere |
| auto mode | Claude Code's own auto-mode classifier rules, read, never written |
| unreadable configuration | a `devplane.toml` or `app.toml` that will not load |
| unwritten | events and decisions the store refused |
| leaked agents | an agent a killed host left running, with the `kill -TERM -<pgid>` |
| unreadable rows | rows this build cannot decode |

### `devplane completions <shell>`

`bash`, `zsh` or `fish`. Ids complete from a running host or the store, never starting one.

```sh
devplane completions zsh  > ~/.zsh/completions/_devplane
devplane completions bash > /etc/bash_completion.d/devplane
devplane completions fish > ~/.config/fish/completions/devplane.fish
```

See [Install](@/docs/install.md#shell-completion).

### `devplane mcp`

Hidden. Devplane over MCP on stdio: four questions and one claim. The plugin registers it, and every
agent Devplane drives is offered it. By hand:

```json
{ "mcpServers": { "devplane": { "command": "devplane", "args": ["mcp"] } } }
```

| Tool | Answers |
|---|---|
| `inbox` | what needs a person, across every project |
| `change` | a change's state, gate verdicts and spec; without an `id`, the change Devplane started the agent for, or every open change for an agent it did not start |
| `explain` | what the gate would decide about a call, and which rule; recorded in `devplane audit` |
| `audit` | what Devplane decided, and on whose authority |
| `spec_and_tests_conflict` | **not a question**: the calling agent says its change's specification and tests cannot both be satisfied. Records the claim and stops the run ([what follows](@/docs/agents.md#when-the-spec-and-the-tests-conflict)) |

The questions ask a host, else read the store, and act on nothing.

`spec_and_tests_conflict` takes exactly `reason` (1 to 1024 bytes) and `paths` (at most 10, each at
most 512 bytes, inside the project or the change's worktree). The run, change and a per-session key
come from the server's environment, and the host checks them. Anything else (a session Devplane did
not start, no host, another run's server, a second open claim, an argument out of bounds) is refused
with a sentence and nothing recorded. Devplane never allows the call itself.

## The host

The one long-lived process ([Architecture](@/docs/architecture.md#one-host-per-home)). Nothing starts
it automatically.

### `devplane up`

Start the host and open the [workbench](@/docs/workbench.md). With a host running, only open it;
otherwise this process is the host until ctrl-c.

| Flag | What |
|---|---|
| `--port <n>` | default `47831`, or `DEVPLANE_PORT`; `0` picks a free port. With `--window`, `[app] port` in `~/.devplane/app.toml` if set |
| `--no-browser` | host without opening a browser |
| `--window` | the native window instead of a browser |

A port held by another process is refused, naming the holder
([Troubleshooting](@/docs/troubleshooting.md#the-port-is-taken)). A second host on the same home is
refused with the running one's port and uptime.

#### `devplane up --window`

The host with a native window: a tray item with the *need you* count, notifications, one global
shortcut (`[app] shortcut`), and `devplane://` links to a change, review, ask or run. Closing the
window keeps hosting; quitting from the tray says what it stops. Nothing starts at login. Needs a
build with the `app` feature (`cargo install devplane --features app`); otherwise the flag is hidden
and refused.

### `devplane down`

Stop the host, first saying what ends: driven agents stop; your own sessions and waiting questions
stay.

```text
Quitting stops 2 agents Devplane started:
  r-4f2a9c1e  claude  ~/code/api
  r-8b31d07a  gemini  ~/code/web
It leaves 3 sessions running in their own terminals untouched.
Stopped.
```

`Stopped.` prints once the port is quiet. With no host: *Nothing is running.*

+++
title = "CLI reference"
description = "Every Devplane command and flag. Reading works with nothing running; a command that needs the host says so."
weight = 20
[extra]
group = "reference"
+++

Commands are nouns with verbs: `devplane run show`, `devplane inbox answer`. Plain `devplane` is
`devplane status`. A verb typed without its noun (`devplane connect`) is refused with the right command.

- **`--json`** works on every command, before or after its name. It changes nothing where nothing is
  printed for a script (`run watch`, `run attach`, `gate install`, `completions`, `up`). A failure is
  `{"error": "…"}` on stdout with a non-zero exit.
- **Colour** follows `NO_COLOR`, is off when output is not a terminal; `CLICOLOR_FORCE=1` forces it.
- **Reading needs no host.** `status`, `run list`, `run show`, `inbox`, `audit`, `search`, `change
  list`, `change show` and `agent list` read the store when no host answers, noting what only a host
  sees. Starting, steering or stopping an agent needs the host and says so. `devplane inbox answer`
  works without one: the waiting hook polls the store.

## Naming a run, a change or an ask

Every id takes the full id or any unique prefix; a run also takes its session name.

```console
$ devplane status
core-lib
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace

$ devplane run show a1
```

An ambiguous prefix is an error listing the matches; a live session wins over one that never reported.
`change prompt`, `change stop` and `inbox snooze` take a change or a run.

## The groups

`devplane --help` lists the commands under five groups:

| Group | Commands |
|---|---|
| **See what is happening** | `status` `run` `search` |
| **What needs you, and what happened without you** | `inbox` `audit` `pr` `issue` |
| **Start and steer work** | `change` `gate` `verify` |
| **Set up a project** | `project` `config` `agent` `auth` `doctor` `completions` |
| **The host** | `up` `down` |

Hidden, for machines: `devplane mcp` (below), and `devplane hook` and `devplane statusline`, which
`devplane agent connect` installs.

## See what is happening

### `devplane status`

Sessions in play and anything asking for you. Plain `devplane` does the same.

| Flag | What |
|---|---|
| `-a`, `--all` | include sessions that never reported anything (usually idle editor tabs) |
| `-p`, `--project <name>` | one project; matches any part of the name (`mat` finds `matter-kit`) |
| `--needs-you` | only sessions waiting on a person |
| `--modes` | live sessions least supervised first, with permission mode |

`--modes` also shows *Seen* (first seen in that mode) and any question auto-continue timer
(`askUserQuestionTimeout`, `CLAUDE_AFK_TIMEOUT_MS`) with who set it.

### `devplane run`

One run: its detail, conversation or terminal.

#### `devplane run list`

Sessions in play, grouped by project. Alias `ls`. Takes `-a`, `-p` and `--needs-you` as `status`.

#### `devplane run show <run>`

State, agent, model, surface, cost, tool calls, blocker, plan, recent tools, tasks sent, and a driven
run's last lines.

#### `devplane run watch [run]`

No run: follow events. With a run: print what that driven agent says, like `tail -f`. Needs the host.
For a session you started, use `run attach`.

| Flag | What |
|---|---|
| `--thinking` | include the agent's reasoning, where streamed |
| `--history <n>` | conversation to print first (default 40) |

#### `devplane run attach <run>`

Hand this terminal to the agent (`claude --resume <id>`, or `claude attach <id>` for a background
session). Replaces this process.

### `devplane search <query>`

Full-text search over tool calls, questions and errors in every session. The text is a phrase.

## What needs you, and what happened without you

### `devplane inbox`

What needs a person: by band, then oldest first. Derived from state, so correct after a restart.

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
| `--needs-you` | only what can be answered from here |
| `--all` | everything an agent asked you, settled ones too |

```console
$ devplane inbox
since you last looked · 16h

 ! Keep the legacy /v1/login route? [question] saas 3m new
   Gate failed after the agent claimed completion [gate_failed] core-lib 5h
```

- `new`: since you last read it. Empty, it counts decisions taken in your name and by whom.
- A narrowed list counts what it hides; narrowing is not remembered.
- Above twelve rows, issues and stalled sessions fold into a counted row; questions, permissions and
  drift never fold.
- A permission item names the narrowest rule that would stop the ask; nothing is written for you.
- `gate_down`: the permission gate is not answering. `config_broken`: a `devplane.toml` will not load.

#### `devplane inbox --all`

Open asks first, oldest first, then settled ones with what ended them: **you**, **a clock**
([`[questions] deadline`](@/docs/configuration.md#questions)) or **nobody**. Then questions an agent
moved past, with what it did instead.

#### `devplane inbox answer <ask>`

Answer a permission or question. The id is from `devplane inbox`; it outlives the asking process, so
an answer given tomorrow reaches the agent through a resumed session.

| Flag | What |
|---|---|
| `--allow` | allow a permission |
| `--deny` | refuse a permission |
| `--option <text>` | one of the agent's options, as written; the whole answer, so not with `--allow`/`--deny` |
| `--custom <text>` | your own words, where the agent offered *Other*; wins over `--option` |
| `--field <id>` | which question, when several were asked; refused with `--allow`/`--deny` |

- One of `--allow`, `--deny`, `--option` or `--custom` is required.
- `--allow`, and `--option` without `--field`, are refused inside an agent session
  ([Security](@/docs/security.md#an-agent-cannot-approve-itself-mostly)).
- No dismiss. The first answer wins; a second is told who gave the first.

#### `devplane inbox snooze <id>`

Hide a run's or change's current inbox items for a while; new kinds still show. The run keeps running.
A question or a permission is never snoozed: the host refuses, naming it, and hides nothing.

| Flag | What |
|---|---|
| `--minutes <n>` | default 60; `0` un-snoozes; capped at 30 days |
| `--until-activity` | until the session next reports activity (for a change: a run event, a check result or a moved tree); not with `--minutes` |

`devplane inbox` counts what is snoozed and lists each item with when it comes back.

### `devplane audit [about]`

What Devplane decided and on whose authority: `person`, `rule`, `timer`, `nobody` or `devplane`.
`about` narrows to one run or change. See [the decision log](@/docs/decisions.md).

| Flag | What |
|---|---|
| `--without-me` | only what a rule, a clock or nobody decided |
| `--limit <n>` | rows (default 50) |
| `--otel` | rows as OpenTelemetry GenAI `gen_ai.tool.call.decision` log records (OTLP/JSON on stdout), authority as an attribute. Nothing is sent |

An MCP tool call's row says where the server was defined (`project`, `user`, `plugin`, `sdk`) or that
it was not reported.

### `devplane pr` and `devplane issue`

Open pull requests and issues across registered projects, read with your GitHub sign-in, what needs
you first. Nothing is written to GitHub.

- The host refreshes every five minutes and after a sign-in.
- Per project: its GitHub state (not signed in, expired, rate limited, unreachable, stale, not a GitHub
  repository) and how many more are open past the page read. `devplane pr list --json` includes it.

#### `devplane pr list`

Every open pull request. `◆` marks what waits on you: a review requested of you, or your own PR that
is red, has changes requested, or is approved and unmerged. Alias `ls`.

#### `devplane issue list`

Every open issue. Alias `ls`.

| Flag | What |
|---|---|
| `--ready` | only this repository's issues with `[github] ready_label` (what `change start --issue` picks from) |
| `--cwd <path>` | which repository; implies `--ready` |
| `--label <label>` | default `[github] ready_label`; implies `--ready` |

## Start and steer work

### `devplane change`

A change is an isolated checkout, an agent in it, and `[gates] check` run when the agent finishes.
States: `drafted`, `isolated`, `in flight`, `verified`, `offered`, `archived`. See
[Verified done](@/docs/verified-done.md).

#### `devplane change start [title]...`

```sh
devplane change start "add rate limiting to /login"
devplane change start --spec specs/001-password-reset --task REQ-3 \
  "password reset"
devplane change start --project api --project web "bump tokio to 1.40"
```

The title is the first prompt and, slugged, the branch (`change/add-rate-limiting-to-login-3f9a1c`).

| Flag | What |
|---|---|
| `--agent <id>` | overrides `[agent] default`: `claude`, `codex`, `opencode`, `copilot`, `gemini`, an `agents.toml` entry, or a command |
| `--project <name\|path>` | a registered project or path; repeatable. Default: the current directory |
| `--no-worktree` | work in the repository itself; marked *in place*, diffed against the working tree |
| `--issue <n>` | start from a GitHub issue; its body is marked as an untrusted report |
| `--spec <path>` | the spec file or folder this change answers, relative to the repository |
| `--task <selector>` | repeatable; needs `--spec`. A requirement token (`REQ-3`) or `file:line` |

Every project is checked first (trusted, clean, readable `devplane.toml`, resolvable prompt, matching
`--task`); all refusals are reported and none starts. The preflight names `[workspace] setup`, or
warns of a lockfile without one.

#### `devplane change list`

Every change, its state and what it waits on (*installing · `pnpm install` · 1m 12s*, *gates
running*, *feedback round 2*, *needs you*). Alias `ls`.

#### `devplane change show <change>`

State, gate standing (*verified*, *stale* with both tree digests, *failed*, *not run*, *no gates
declared*), runs, cost against budget, and each check's output. An agent's open claim that the spec
and the tests conflict is printed after the state and before the gates, quoted and attributed. With a
spec, task counts and drift:

```console
tasks      11 ticked · 9 seen by a passing check
REQ-3      3 tasks · 2 ticked · 0 seen by a passing check
drift      the specification changed 18m into run r-9f2 and the run never saw it
           devplane change drift c-01a0 --run r-9f2 --accept | --tell
```

#### `devplane change review <change>`

**Checks weakened or changed** first ([the kinds](@/docs/verified-done.md#review-before-you-merge)),
each with what it matched and whether it was read; then gate standing; then every file in `[review]
roles` order with its role, covering test, decisions and task. No score.

| Flag | What |
|---|---|
| `--by risk` | declared role order (default) |
| `--by intent` | grouped by the run that wrote each file and its tasks; unrequested files last |
| `--seen <path>` | mark every hunk at that path seen, as yours (repeatable); a weakened row is read when its hunks are. Refused for a path with nothing in the change, and inside an agent session (an advisory check) |

#### `devplane change verify <change>`

Run `[gates] check` now and record it. `--base-checks` follows a pass with the
[base checks](@/docs/verified-done.md#base-checks). The exit code stays `check`'s.

#### `devplane change retry <change>`

One more feedback round past `max_feedback_rounds`. Recorded as your decision.

#### `devplane change resume <change>`

Reconnect to the same agent conversation after the host stopped. Offered only when the agent can
resume; never automatic.

#### `devplane change prompt <change|run> [text]...`

Message the agent without stopping it (a change: its latest run). Queued mid-turn.

#### `devplane change claim-read <change>`

Mark the agent's open claim that the spec and the tests conflict read. It stays on the change under
its run; no decision is recorded. Refused inside an agent session (an advisory check). To answer the
agent instead: `devplane change resume <change>`, then `devplane change prompt <change> "…"`.

#### `devplane change stop <change|run>`

Stop a run (a change: its latest). Says what survives (change, worktree, branch, record), then asks on
a terminal; elsewhere it needs `-y` (`--yes`).

#### `devplane change drift <change> --run <run> (--accept | --tell)`

A spec moved under a run. `--tell` prompts the run with the changed files (resuming it if gone);
`--accept` moves the change's start to what the run saw. Recorded as your decision. Needs the host.

#### `devplane change adopt <branch>`

Make a hand-made branch a change. Refuses a missing branch and the base branch. Uses the checkout that
has the branch, or a fresh one under `.claude/worktrees/`.

| Flag | What |
|---|---|
| `--project <root>` | the repository; default the current one |
| `--spec <path>` | the spec it answers |
| `--title <text>` | default: the branch's first commit subject |

#### `devplane change offer <change>`

Commit exactly the verified tree (the certificate's `gitTree`) on the change's branch, then open a
draft pull request with `[github] pull_request = true`, or print the `git push` line and GitHub's
compare address. Refused with exit `3` while a weakened-check row is unseen (`--json`: `refused`,
`rows`, `seen_with`), when the tree moved since the last pass, and inside an agent session.
[Full rules](@/docs/verified-done.md#offering-finishing-archiving).

#### `devplane change finish <change>`

Accept a change as finished, recording the basis: verified, stale, no gates declared, or by hand.
Removes nothing.

#### `devplane change archive <change>`

Remove the worktree, keep the record. Refuses uncommitted or unmerged work unless told; a refusal
changes nothing. After `offer` the worktree is clean.

| Flag | What |
|---|---|
| `--delete-branch` | also delete the branch; refused while it has commits its base lacks, unless its PR merged |
| `--discard-uncommitted` | remove the worktree despite uncommitted or untracked files |
| `--force` | with `--delete-branch`: delete despite unmerged commits |

#### `devplane change export <change>`

Print the [done certificate](@/docs/verified-done.md#the-done-certificate). `--json` gives an
unsigned [in-toto statement](https://in-toto.io/Statement/v1).

```sh
devplane change export c-01a0 > cert.md
```

### `devplane gate`

Run this repository's gates and report the exit. No host, records nothing, reads no spec. A
[Spec Kit hook](@/docs/specs.md) calls it.

| Flag | What |
|---|---|
| `--cwd <path>` | which repository; default the working directory |
| `--name <gate>` | run `[gates.<gate>]` instead of `check` |

`--json` prints `state`, `passed`, `summary` and `commands`.

| `state` | Exit | Means |
|---|---|---|
| `passed` | 0 | every command passed |
| `failed` | 1 | one did not, and it is named |
| `no_gates` | 1 | no checks declared |
| `config_unreadable` | 1 | `devplane.toml` did not parse; nothing ran |
| `unknown_gate` | 1 | `--name` names no declared gate; the declared names are printed |

A named gate never verifies a change; only `check` defines done.

```console
$ devplane gate --name bench
ok        cargo bench --no-run exit 0

bench passed
Not recorded: this ran in a repository rather than against a change. `devplane change verify <id>` is the one that leaves a row.
```

#### `devplane gate install --speckit`

Register the gate as a Spec Kit hook in `.specify/extensions.yml`, naming the `devplane-gate` skill.
Setup and refusals: [Specifications](@/docs/specs.md#the-gate-inside-spec-kit-s-workflow).

| Flag | What |
|---|---|
| `--speckit` | required; Spec Kit is the one workflow supported |
| `--event <hook>` | hook point; default `after_implement` |
| `--dry-run` | print, write nothing (same refusals) |
| `--anyway` | write even with no gate declared or no reachable skill |

### `devplane verify`

Run the base's `check` on this change, leading with the checks it weakened. For CI
([CI verify](@/docs/ci.md)) or before you push. Needs only git and the repository: no host, store or
network.

- The change is the diff from the merge base with `--base` to the working tree, untracked included.
- `check`, its timeout and review roles come from `devplane.toml` **at the merge base**. A change that
  edits `check` is judged by the base's, and the edit is the summary's first row.

| Flag | What |
|---|---|
| `--base <ref>` | any commit-ish, tried as `origin/<ref>` then `<ref>`. Default: `origin/HEAD`'s branch, else `main` |
| `--summary <path>` | write the Markdown summary there; otherwise appended to `$GITHUB_STEP_SUMMARY` if set |
| `--statement <path>` | write the in-toto statement when `check` ran |
| `--no-base-checks` | skip the base checks that run after a pass |

`--json` prints one document: `verdict`, `exit`, `base`, `tree`, weakened rows, each command's outcome.
Verdicts and exit codes: [CI verify](@/docs/ci.md#exit-codes). Weakened checks never change the exit
code.

## Set up a project

### `devplane project trust [path]`

Allow agents in a repository (default: here), after printing its hooks and MCP servers:

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

Refused inside an agent's session (the same variables as `inbox answer --allow`). It writes the store
itself and has a running host re-read it; the host's trust route takes only the window.

### `devplane config`

Read `devplane.toml`, or ask what its rules decide. Offline.

#### `devplane config check [path]`

Does it parse, does everything it names exist, is anything unsafe. Also names a rule that covers
nothing, one that grants more than it reads, and a read-denied path that is still writable. A retired
key is refused with its replacement. Exits non-zero on error; run it in CI. See
[Configuration](@/docs/configuration.md).

#### `devplane config explain [call]...`

What the permission gate would decide about one call, and which rule says so. Records nothing.

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

Verdicts: `deny`, `ask`, `ask — unreadable` (`unresolved` in `--json`) or `undecided`. Never `allow`.
See [Permissions](@/docs/permissions.md).

| Flag | What |
|---|---|
| `--tool <name>` | default `Bash`. `Read`, `Edit`, `WebFetch` take the specifier directly: `devplane config explain --tool Read .env` |
| `--input <json>` | the whole tool input: `--tool Agent --input '{"isolation":"worktree"}'` |
| `--dir <path>` | the agent's working directory, which decides whose rules apply (default `.`) |
| `--replay` | replay every observed call against the current rules |
| `--limit <n>` | recent calls `--replay` reads (default 5000) |

`--replay` groups undecided calls by the allow rule that would cover them, most interruptions first;
a rule is offered after three interruptions. Writes nothing.

### `devplane agent`

The agents Devplane drives, and its hooks in each vendor's settings.

#### `devplane agent list`

Drivable agents and, once started, what each advertised (`resume`, `load`, `list`, modes, sign-in)
with the date. Alias `ls`. See [Driving agents](@/docs/agents.md).

#### `devplane agent connect <vendor>`

Install hooks: `claude` (hooks and telemetry in user settings, with a backup), `codex` (merged into
`~/.codex/hooks.json`) or `copilot` (one file in `~/.copilot/hooks/`). Starts nothing.

- Shows the diff; asks on a terminal. Elsewhere, and under `--json`, writes only with `--yes`.
- Hooks run the binary by full path, so a temporary binary is refused (npx cache, macOS App
  Translocation, `/Volumes`, an AppImage mount). Re-run after moving or reinstalling.
- Codex runs a hook only after you approve it in its dialog. For Copilot it prints three environment
  lines to export. See [Watching sessions](@/docs/observe.md).

| Flag | What |
|---|---|
| `--statusline` | also wrap your status line, the only source of subscription rate limits |
| `-y`, `--yes` | write without asking |
| `--bin <path>` | the binary the hooks run; default this one |

#### `devplane agent disconnect <vendor>`

Remove exactly what `connect` installed for `claude`, `codex` or `copilot`; your own hooks stay. Diff,
then asks; `-y` elsewhere.

### `devplane auth`

Sign in to GitHub, or out.

#### `devplane auth login`

GitHub's device flow: prints a code and address, waits. The token goes only into the OS credential
store as service `devplane`, account `github:<host>`; with no store, sign-in refuses.

| Flag | What |
|---|---|
| `--host <host>` | a GitHub Enterprise server instead of `[github] host` in `~/.devplane/app.toml` (default `github.com`) |
| `--with-token` | read a token from stdin, verify and store it: for a host with no registered app, and CI |

`--json` prints `{ "state": "pending", "user_code", "verification_uri", "expires_at" }`, then `{
"host", "login", "scopes" }`. A host with no registered app needs a token:

```sh
gh auth token | devplane auth login --with-token  # GitHub CLI's token
devplane auth login --with-token < pat.txt        # a fine-grained personal access token
```

An Enterprise app is set per host in `~/.devplane/app.toml` (`[github.hosts."ghe.corp"] client_id`,
or `[github] client_id`). A build's own app covers `github.com` only.

#### `devplane auth logout`

Delete the token everywhere Devplane holds it; `--host` for Enterprise. The grant stays at GitHub;
the command names `https://<host>/settings/applications` to revoke it.

### `devplane doctor`

Whether Devplane works. The probe is recorded nowhere.

| Section | Reports |
|---|---|
| host | running (port, version, binary) or not |
| only gate | *Devplane is the only gate on this machine* when a provider variable (Bedrock, Vertex, Foundry, a gateway, an API key) turns Claude Code's own supervision off |
| claude code | the hook gate, **run** with a real `PreToolUse` probe and timed, or `INSTALLED AND NOT ANSWERING`; **gate off** for an event whose binary is gone; **out of date** for old hooks; the Claude Code release the rule syntax was modelled on |
| github | per host: login, scopes, last answer (never the token); projects that are not GitHub repositories, with why (`--json`: `github`) |
| watched | per vendor and channel: `read`, `unproved`, `unbuilt`, `not published`, `not checked` |
| channels | which are arriving, how fast, worst latency |
| inbox | per kind over a week: raised, acted on, dismissed, resolved elsewhere, folded rows acted on. A mostly dismissed kind is too loud |
| unreadable configuration | a `devplane.toml` or `app.toml` that will not load |
| unwritten | events and decisions the store refused |
| leaked agents | an agent a killed host left running; also a critical inbox item with the `kill -TERM -<pgid>` |
| unreadable rows | rows this build cannot decode |
| window | whether this build has it |

### `devplane completions <shell>`

```sh
devplane completions zsh  > ~/.zsh/completions/_devplane
devplane completions bash > /etc/bash_completion.d/devplane
devplane completions fish > ~/.config/fish/completions/devplane.fish
```

See [Install](@/docs/install.md#shell-completion).

### `devplane mcp`

Hidden. Devplane's surface over MCP on stdio: four questions and one claim. The plugin registers it,
and every agent Devplane drives is offered it. By hand:

```json
{ "mcpServers": { "devplane": { "command": "devplane", "args": ["mcp"] } } }
```

| Tool | Answers |
|---|---|
| `inbox` | what needs a person, across every project |
| `change` | a change's state, gate verdicts and spec; all open changes without an `id` |
| `explain` | what the gate would decide about a call, and which rule |
| `audit` | what Devplane decided, and on whose authority |
| `spec_and_tests_conflict` | **not a question**: the calling agent says the specification and the tests of its change cannot both be satisfied |

The questions act on nothing: they ask a host, else read the store. What the server writes is a
short list — an `explain` here **is** recorded in `devplane audit`, and `spec_and_tests_conflict`
records the agent's claim on its own change and stops its run
([what follows](@/docs/agents.md#when-the-spec-and-the-tests-conflict)).

`spec_and_tests_conflict` takes exactly `reason` (1 to 1024 bytes) and `paths` (at most 10, each at
most 512 bytes, inside the project or the change's worktree); no argument names a run, a change or
anybody. The run, its change and a per-session key come from the environment Devplane gave the
server when it offered it, and the host checks them. Refused, with a sentence and nothing recorded:
a session Devplane did not start (*tell your person in your reply instead*), no host running, a run
this host does not drive, a run with no change, a server started for another run's session or
change, a run already stopping, a second claim before you answer the first, an empty or oversized
field, a path outside both roots, an argument it does not take. Devplane never allows the call on
its own; where the agent's own permission prompt asks, a person (or their rule) answers it.

## The host

The one long-lived process ([Architecture](@/docs/architecture.md#one-host-per-home)). Nothing starts
it automatically.

### `devplane up`

Start the host and open the [workbench](@/docs/workbench.md); with a host running, only open it.
Otherwise this process is the host until ctrl-c.

| Flag | What |
|---|---|
| `--port <n>` | default `47831`, or `DEVPLANE_PORT`; `0` picks a free port. With `--window`, `[app] port` in `~/.devplane/app.toml` if set |
| `--no-browser` | host without opening a browser |
| `--window` | the native window instead of a browser |

A port held by another process is refused, naming the holder
([Troubleshooting](@/docs/troubleshooting.md#the-port-is-taken)). A second host on the same home is
refused with the running one's port and uptime.

#### `devplane up --window`

The host with a native window: the tray's *need you* count (the inbox badge's number), notifications, one global shortcut (`[app]
shortcut`), and `devplane://` links to a change, review, ask or run. Closing the window keeps hosting;
quitting from the tray says what it stops. Nothing starts at login. Needs a build with the `app`
feature (`cargo install devplane --features app`); otherwise the flag is hidden and refused.

### `devplane down`

Stop the host, first saying what ends: driven agents stop; your sessions and waiting questions stay.

```text
Quitting stops 2 agents Devplane started:
  r-4f2a9c1e  claude  ~/code/api
  r-8b31d07a  gemini  ~/code/web
It leaves 3 sessions running in their own terminals untouched.
Stopped.
```

`Stopped.` prints once the port is quiet. No host: *Nothing is running.*

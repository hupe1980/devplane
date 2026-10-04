+++
title = "Driving agents"
description = "Start, prompt, watch, stop and resume any agent that speaks the Agent Client Protocol — and add one that is not built in with one entry in a file."
weight = 11
[extra]
group = "guide"
+++

A **driven** session, started by `devplane change start`, can be answered, prompted, stopped and
resumed. A **watched** session, one you started yourself, is only shown. Both share the board and
the inbox, and `[policy]` decides first.

```sh
devplane project trust .                             # once per repository
devplane agent list                                  # what can be driven
devplane change start "add rate limiting to /login"  # Claude Code
devplane change start --agent codex "review the auth change"
devplane run watch <run>                             # follow what it says
devplane change prompt <change> "use the existing middleware"
devplane inbox answer <ask> --allow                  # <ask>: from the inbox
```

## Trust comes first

```sh
devplane project trust --dry-run ../someone-elses-repo   # show it, trust nothing
devplane project trust .
```

No agent starts in an untrusted repository. `project trust` shows the repository's hooks and MCP
servers first ([Security](@/docs/security.md#the-trust-gate)).

## Which agents

```console
$ devplane agent list
claude     Claude Code    npx -y @agentclientprotocol/claude-agent-acp@0.85.1
codex      Codex          npx -y @agentclientprotocol/codex-acp@2.1.1
opencode   OpenCode       opencode acp
copilot    GitHub Copilot npx -y @github/copilot@1.0.91 --acp
gemini     Gemini CLI     npx -y @google/gemini-cli@0.62.0 --acp
```

- The `npx` agents are pinned to exact versions and need Node and npm access on first start.
  OpenCode runs the `opencode` on your `PATH`.
- Capabilities (`resume`, `load`, modes, sign-in) are listed only for agents that have run, with the
  date. *Not probed* is not *not supported*.
- An agent that needs signing in lists its own sign-in methods on the change. `devplane change login
  <change>` lists them; `--method <id>` runs a terminal method in your terminal and starts the change
  again, or asks the agent to sign itself in. A method of an unknown kind is shown and never acted
  on. Devplane never asks for, reads or stores a credential.

## Adding an agent

Any other agent that speaks the protocol is an entry in `~/.devplane/agents.toml`:

```toml
# ~/.devplane/agents.toml
[agents.kimi]
name    = "Kimi"
command = "npx -y @moonshot/kimi-acp@1.2.0"
```

| Key | Meaning |
|---|---|
| `name` | the display name; default the id |
| `command` | how to start it; optional for a built-in id |
| `mode` | the permission mode to ask for at set-up ([below](#permission-modes)) |

- A built-in id replaces the built-in, to re-pin it.
- A file that does not parse, or an entry with no `command` for an unknown id, adds nothing; the
  host logs why.
- Or name a command directly: `devplane change start --agent '/opt/my-agent --acp' "…"`. An unknown
  bare word is an error, never a launch.

## Permission modes

An agent in a mode that decides without you (Claude Code's *Auto*, Codex's *Auto review*, any
agent's *full access*) stops asking, so an empty inbox proves nothing. Devplane reads the mode each
session opens in and shows it on the change, in `devplane change show` and in `devplane status
--modes`.

| Situation | What Devplane does |
|---|---|
| You set no mode | Before the first prompt it asks the agent for its ask-first mode (Codex's *Read-only*, Claude Code's *Manual*, Gemini CLI's *Default*). Codex starts in *Auto review* unless told otherwise, so this is the usual Codex case |
| You set a mode in the agent's own settings (`permissions.defaultMode`, `INITIAL_AGENT_MODE`, Gemini CLI's approval mode) | Keeps it, and says whether it decides without you |
| You set `mode` in `agents.toml` | Asks the agent for exactly that value at set-up, if the agent offers it |

```toml
# ~/.devplane/agents.toml
[agents.codex]          # a built-in: `command` may be left out
mode = "workspace-write"
```

Set `mode` too when you configured exactly the mode an adapter starts in by itself (for example
`INITIAL_AGENT_MODE=agent`); from outside that looks like no choice, and Devplane would ask for
ask-first.

Asking for a mode decides nothing and records no decision. Devplane asks once, at set-up. If the
agent or another client changes the mode later, the change follows it. Modes are named in the
agent's own words, and only agents whose advertisement is recorded at their pinned version are read
as asking or not.

| The change says | Meaning |
|---|---|
| *asked ‹agent› to ask before acting — you set no mode* | Devplane asked for the ask-first mode, and the agent agreed |
| *asks you: ‹mode›* | the session opened in its ask-first mode |
| *you chose ‹mode›* | your `agents.toml` `mode` |
| *you chose ‹value›; ‹agent› does not offer it* | your `mode` is not one this agent advertises; nothing was asked |
| *‹agent› is in ‹mode› — you set it* | set in the agent's own settings, and it asks you |
| *decides without you: ‹mode› (‹what›) — you set it* | set in the agent's own settings, and it decides *what* without you |
| *‹agent› moved to ‹mode› — …* | the mode changed mid-session |
| *asked ‹agent› to ask before acting; it refused: ‹reason›* / *it did not answer* | the request failed (no answer within ten seconds); the current mode is shown |
| *‹agent› advertises no mode Devplane knows asks you first* | nothing was asked |
| *‹agent› states no permission mode* | the agent advertises none |
| *‹agent› is in ‹mode› — not known whether it asks you* | an agent or version Devplane has no record of |

### What a rule can see

Project rules match a call's kind and input (the command, the file, the URL), never its title. An
agent may report a call in pieces, so Devplane joins every report of a call before a rule reads it.
Gemini CLI 0.62.0 sends no input for a call: where a rule about commands is in force, each of its
command calls goes to you, and the inbox says *the agent did not send what it means to run*.

## Steering a run

| Command | Does |
|---|---|
| `devplane change start --agent <id> "<prompt>"` | worktree, agent, gates when it finishes |
| `devplane change start … --project a --project b` | one prompt, several projects |
| `devplane change prompt <change\|run> <text>` | message it without stopping; queued mid-turn |
| `devplane run watch <run>` | the conversation live; `--thinking` adds reasoning |
| `devplane run show <run>` | detail and last lines |
| `devplane change stop <run>` | stop; says what survives, then asks (`--yes` in scripts) |
| `devplane inbox answer <ask>` | `--allow` / `--deny`, or `--option` / `--custom` |

An ask outlives its agent. It survives quitting the host, and the next host delivers the answer by
resuming the session. Only [`[questions] deadline`](@/docs/configuration.md#questions) ends it
unanswered. A question the agent withdraws leaves the inbox.

## What a driven agent is given

- **Devplane's MCP server** ([`devplane mcp`](@/docs/cli.md#devplane-mcp)). Asked `change` with no
  id, it answers about the agent's own change: phase, gate verdicts, specification and task lines,
  framed as a report, not instructions. When the server could not be offered, the change says so.
- **Its specification folder** as an additional directory, when the folder is in your checkout but
  not on the base branch. An agent that cannot take one ends before any prompt, saying why.
- **A way to say the spec and the tests conflict.** Every implementation prompt and every round of
  gate feedback ends with one sentence telling it to call `spec_and_tests_conflict` rather than
  change the tests, or to say so plainly without that tool.
- **`DEVPLANE_RUN`** in its environment, marking an agent session.
- **A clean ending**: `session/close` where offered, before its processes end. The change says
  whether the session was closed that way.

What the agent addresses to you as a notice is shown on the change, quoted. The diff an agent
reports for an edit is kept beside the decision on that call, labelled as the agent's own report;
the gates read the worktree, not the report. With `[agent] keep_transcripts = false` no diff is
kept, and the ledger row says so.

## When the spec and the tests conflict

A driven agent calls `spec_and_tests_conflict` with a reason and the files involved. Then:

- **The claim lands on the change as the agent's**: *the agent says the spec and the tests
  conflict*, the reason quoted, the agent and run named. It shows after the weakened checks and
  before the gates (change header, change list, `devplane change show`) and stays in the change's
  history once closed.
- **The run stops.** Its turn is cancelled and its process ended; its open asks end under *nobody*.
  No decision is written for the claim or the stop.
- **`check` runs once** on the tree the agent left. No feedback round follows; the change waits for
  you.
- **It is neither a failure nor evidence.** The change is not marked failed, and *verified* reads
  exactly what the gate says.

Answer it by telling the agent (resume, then `devplane change prompt`) or by marking it read
(`devplane change claim-read`). It also closes when a run goes back to work on the change, or when
the change is finished or archived. The claim's text is never sent to any agent.

Limits: a reason of 1 to 1024 bytes, at most 10 paths of at most 512 bytes inside the project or
the worktree, one open claim per run, and only from an agent Devplane started for a change. Anything
else is refused with a sentence and nothing is recorded ([details](@/docs/cli.md#devplane-mcp)).

## Before an agent commits

Claude Code runs a project or user skill named `verify` right before it commits (except docs-only
and tests-only commits). Devplane can be that skill:

```sh
devplane gate install --skill --dry-run   # the diff, and nothing written
devplane gate install --skill             # the diff, then: Write this? [y/N]
devplane gate install --skill --user      # in ~/.claude/skills/, for every repository
```

The skill runs `devplane gate --json` and reports what each check exited with. It says *passed*,
never *verified*, which only `check` against a recorded tree earns
([Verified done](@/docs/verified-done.md)). It fixes, re-runs and records nothing.

- It is written to `.claude/skills/verify/SKILL.md`, its first line naming where it came from.
  Delete the directory to remove it.
- Where nobody can answer (a script, an agent's shell, `--json`), nothing is written without
  `--yes`.
- It is refused with no `[gates] check`. A `verify` skill you wrote is left alone unless you pass
  `--anyway`, which shows the diff that replaces it.

## Transcripts and cost

Devplane keeps what a driven agent says, on this machine. To decline:

```toml
# devplane.toml
[agent]
keep_transcripts = false
```

Cost is recorded as deltas of the reported total; context is the agent's own level and window size.

> [!WARNING]
> Cost is **optional** in ACP. An agent that never reports it cannot be stopped by `[budget] max_usd`;
> `change show` prints *not reported by this agent — a [budget] ceiling cannot bite*.

## Resuming after a restart

Quitting the host ends driven agents. The branch, worktree and conversation survive, and the run is
`interrupted`. To continue:

```sh
devplane change resume <change>
```

Devplane uses `session/resume`, else `session/load` with replay suppressed (GitHub Copilot). With
neither it says so; it never passes a fresh session off as a resume, and never resumes on its own.

To continue with a different agent:

```sh
devplane change resume <change> --agent codex "Finish the rate limiter; the tests are in tests/login.test.ts"
```

The new agent starts in the change's worktree and is told only your words and Devplane's standing
sentence, nothing from the earlier conversation. It reads the specification, tasks and worktree as
files and through the MCP server. Both runs stay on the change, the earlier one resumable with its
own agent, and the switch is recorded as yours. It is refused while a run of the change is live.

## Handing over

```sh
devplane run attach <run>    # this process becomes `claude --resume <session>`
```

On Unix it `exec`s, so the terminal behaves natively; on Windows it starts `claude` and waits. It
needs `claude` ([finding it](@/docs/install.md#finding-the-claude-binary)). For your own sessions,
see [Watching sessions](@/docs/observe.md).

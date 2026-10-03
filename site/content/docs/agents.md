+++
title = "Driving agents"
description = "Start, prompt, watch, stop and resume any agent that speaks the Agent Client Protocol — and add one that is not built in with one entry in a file."
weight = 11
[extra]
group = "guide"
+++

A **driven** session (started by `devplane change start`) can be answered, prompted, stopped and
resumed; a **watched** one only shown. Both share the board and inbox; `[policy]` decides first.

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

No agent starts in an untrusted repository; `project trust` shows its hooks and MCP servers first
([Security](@/docs/security.md#the-trust-gate)).

## Which agents

```console
$ devplane agent list
claude     Claude Code    npx -y @agentclientprotocol/claude-agent-acp@0.85.1
codex      Codex          npx -y @agentclientprotocol/codex-acp@2.1.1
opencode   OpenCode       opencode acp
copilot    GitHub Copilot npx -y @github/copilot@1.0.91 --acp
gemini     Gemini CLI     npx -y @google/gemini-cli@0.62.0 --acp
```

- `npx` agents are pinned and need Node and npm access the first time; OpenCode runs the `opencode`
  on your `PATH`.
- Capabilities (`resume`, `load`, `modes`, sign-in) are listed only for agents that have run, with the
  date. *Not probed* differs from *not supported*.
- Agents sign in in their own terminal; a failed start shows the advertised methods (*Run `copilot
  login` in the terminal*).

## Adding an agent

Anything else that speaks the protocol is an entry in `~/.devplane/agents.toml`:

```toml
# ~/.devplane/agents.toml
[agents.kimi]
name    = "Kimi"
command = "npx -y @moonshot/kimi-acp@1.2.0"
```

A built-in id replaces the built-in (to re-pin it). An unparsable file adds nothing and logs why. Or
name a command:

```sh
devplane change start --agent '/opt/my-agent --acp' "…"
```

An unknown bare word is an error, never a launch.

## Permission modes

A driven agent can run in a mode that decides without you — Claude Code's *Auto*, Codex's *Auto
review* (a reviewer model answers), any agent's *full access* — and then it stops asking, so an empty
inbox would look like an agent with nothing to ask. Devplane reads the mode each session opens in, as
the agent advertises it, and says it on the change, on `devplane change show` and in `devplane status --modes`:

- **You set no mode.** The session opened where the adapter starts on its own, so Devplane asks the
  agent for its own ask-first mode before the first prompt — Codex's *Read-only*, Claude Code's
  *Manual*, Gemini CLI's *Default* — and the change says *asked Codex to ask before acting — you set
  no mode*. Codex starts every session in *Auto review* unless told otherwise, so this is the usual
  Codex case.
- **You set a mode** in the agent's own settings (Claude Code's `permissions.defaultMode`, Codex's
  `INITIAL_AGENT_MODE`, Gemini CLI's approval mode). It is kept. When it decides without you the
  change says so whole: *decides without you: Full access (everything) — you set it*.
- **You chose one for Devplane** with `mode` in `agents.toml`. Devplane asks the agent for exactly
  that value at set-up, if the agent offers it:

  ```toml
  # ~/.devplane/agents.toml
  [agents.codex]          # a built-in: `command` may be left out
  mode = "workspace-write"
  ```

  Use this too if you configured exactly the mode an adapter starts in by itself (for example
  `INITIAL_AGENT_MODE=agent`): from the outside that looks like no choice at all, and Devplane would
  ask for ask-first.

Asking for the ask-first mode decides nothing: no decision, no grant. Devplane asks once, at
set-up, and never after; if the agent or another client changes the mode later, the change follows
it and Devplane does not move it back. A mode is named in the agent's own words, and only agents
whose advertisement Devplane has recorded at their pinned version are read as asking or not.

| The change says | Meaning |
|---|---|
| *asked ‹agent› to ask before acting — you set no mode* | Devplane asked for the agent's ask-first mode, and it agreed |
| *asks you: ‹mode›* | the session opened in its ask-first mode |
| *you chose ‹mode›* | your `agents.toml` `mode` |
| *you chose ‹value›; ‹agent› does not offer it* | your `mode` names a value this agent does not advertise; nothing was asked |
| *‹agent› is in ‹mode› — you set it* | configured in the agent's own settings, and it asks you |
| *decides without you: ‹mode› (‹what›) — you set it* | configured, and it decides *what* without you |
| *‹agent› moved to ‹mode› — …* | the mode changed mid-session |
| *asked ‹agent› to ask before acting; it refused: ‹reason›* / *it did not answer* | the request failed (after ten seconds for no answer); the mode it is in is shown |
| *‹agent› advertises no mode Devplane knows asks you first* | nothing was asked |
| *‹agent› states no permission mode* | the agent advertises none |
| *‹agent› is in ‹mode› — not known whether it asks you* | an agent or version Devplane has no record of |

### What a rule can see

Project rules match a call's kind and its input — the command, the file, the URL — never its title.
Agents now report a call in pieces (an update may carry only what changed, and a permission request
may name the call by its id alone), so Devplane joins every report of a call before a rule reads it.
Gemini CLI 0.62.0 sends no input for a call at all: where a rule about commands is in force, each of
its command calls goes to you, and the inbox says *the agent did not send what it means to run*.

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

**An ask outlives its agent.** It survives quitting the host; the next host delivers the answer by
resuming the session. Only [`[questions] deadline`](@/docs/configuration.md#questions) ends it
unanswered.

## What a driven agent is given

- **Devplane's MCP server** ([`devplane mcp`](@/docs/cli.md#devplane-mcp)): four questions — inbox,
  change, explain, audit — and one claim, `spec_and_tests_conflict` (below).
- **A way to say the spec and the tests conflict.** Every implementation prompt and every round of
  gate feedback ends with one sentence: if the specification and the tests cannot both be
  satisfied, call `spec_and_tests_conflict` with what conflicts and leave the tests as they are, and
  without that tool, say so plainly.
- **`DEVPLANE_RUN`** in its environment, marking an agent session.
- **A clean ending**: `session/close` where offered, before its processes end.
- **Withdrawn questions leave the inbox** when the agent cancels a request.

## When the spec and the tests conflict

An agent that cannot satisfy both the specification and the tests has two moves: say so, or make
the checks pass another way. Devplane makes the first one cheap. A driven agent calls
`spec_and_tests_conflict` with a reason and the files involved, and:

- **The claim lands on the change as the agent's**: *the agent says the spec and the tests
  conflict*, the reason quoted as plain text, the agent and run named. It is read after the
  weakened checks and before the gates — in the change's header, the change list and `devplane
  change show` — and kept in the change's history once it closes.
- **The run stops.** The agent's turn is cancelled and its process ended; its open asks end
  under *nobody*. Devplane writes no decision for the claim or the stop.
- **`check` runs once** on the tree the agent left, and nothing is handed back: no feedback round
  follows a claim. The change waits for you.
- **It is not a failure, and it is not evidence.** The change is not marked failed, and
  *verified* reads exactly what the gate says, with or without a claim.

You answer it by telling the agent — your own words, sent after its conversation is resumed — or by
marking it read (`devplane change claim-read`). A claim closes when you do either, when a run goes
back to work on the change, or when the change is finished or archived. The claim's text is never
sent to any agent.

The claim is bounded: a reason of 1 to 1024 bytes, at most 10 paths of at most 512 bytes, each
inside the project or the change's worktree. Only an agent Devplane started for a change can make
one, about its own run, once until you answer; anything else is refused with a sentence and nothing
is recorded.

## Before an agent commits

Claude Code runs a project or user skill named `verify` right before it commits (docs-only and
tests-only commits excepted). Devplane can be that skill:

```sh
devplane gate install --skill --dry-run   # the diff, and nothing written
devplane gate install --skill             # the diff, then: Write this? [y/N]
devplane gate install --skill --user      # in ~/.claude/skills/, for every repository
```

The skill runs `devplane gate --json` and reports what each of the project's checks exited with.
Its pass is the agent's own run, so it says *passed* — never *verified*, which only `check` against
a recorded tree earns ([Verified done](@/docs/verified-done.md)). It does not fix, re-run or record
anything; a red gate is reported before the commit and the person decides.

- Written to `.claude/skills/verify/SKILL.md`, with a first line naming where it came from. Delete the
  directory to remove it.
- Shown as a diff first, and written only when you answer yes. Where nobody can answer — a script, an
  agent's shell, `--json` — nothing is written without `--yes`.
- Refused in a repository with no `[gates] check`. A `verify` skill you wrote yourself is left alone
  unless you pass `--anyway`, and then you are shown the diff that replaces it.

## Transcripts

Devplane keeps what a driven agent says, on this machine. To decline:

```toml
# devplane.toml
[agent]
keep_transcripts = false
```

## Cost and context

Cost is recorded as deltas of the reported total; context is the agent's own level and window size.

> [!WARNING]
> Cost is **optional** in ACP. An agent that never reports it cannot be stopped by `[budget] max_usd`;
> `change show` prints *not reported by this agent — a [budget] ceiling cannot bite*.

## Resuming after a restart

Quitting the host ends driven agents; branch, worktree and conversation survive, and the run is
`interrupted`. To continue:

```sh
devplane change resume <change>
```

Uses `session/resume`, else `session/load` with replay suppressed (GitHub Copilot). With neither it
says so; a fresh session is never passed off as a resume. Never automatic.

## Handing over

```sh
devplane run attach <run>    # this process becomes `claude --resume <session>`
```

On Unix it `exec`s, so terminal behaviour is native; on Windows it starts `claude` and waits. Needs
`claude` ([finding it](@/docs/install.md#finding-the-claude-binary)). Watching your own sessions:
[Watching sessions](@/docs/observe.md).

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
claude     Claude Code    npx -y @agentclientprotocol/claude-agent-acp@0.81.2
codex      Codex          npx -y @agentclientprotocol/codex-acp@1.13.1
opencode   OpenCode       opencode acp
copilot    GitHub Copilot npx -y @github/copilot@1.0.88 --acp
gemini     Gemini CLI     npx -y @google/gemini-cli@0.61.0 --acp
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

- **The read-only MCP server** ([`devplane mcp`](@/docs/cli.md#devplane-mcp)): inbox, change,
  explain, audit.
- **`DEVPLANE_RUN`** in its environment, marking an agent session.
- **A clean ending**: `session/close` where offered, before its processes end.
- **Withdrawn questions leave the inbox** when the agent cancels a request.

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

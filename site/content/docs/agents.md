+++
title = "Driving agents"
description = "Start, prompt, watch, stop and resume any agent that speaks the Agent Client Protocol — and add one that is not built in with one entry in a file."
weight = 11
[extra]
group = "guide"
+++

A session Devplane **watches** is one it can show you. A session it **drives**, started with
`devplane change start`, is one it can also answer, prompt, stop and resume. Both appear on the same
board and in the same inbox; for a driven run the project's `[policy]` rules decide first.

```sh
devplane trust .                                     # once per repository
devplane agents                                      # what can be driven
devplane change start "add rate limiting to /login"  # Claude Code
devplane change start --agent codex "review the auth change"
devplane watch <run>                                 # follow what it says
devplane change prompt <change> "use the existing middleware"
devplane answer <ask> --allow                        # <ask>: from the inbox
```

Each change runs in an isolated worktree. The `<ask>` id is the one `devplane inbox` prints.

## Trust comes first

```sh
devplane trust --dry-run ../someone-elses-repo   # show it, trust nothing
devplane trust .
```

A headless agent runs the repository's own hooks and MCP servers without asking, so Devplane starts
none in a directory you have not trusted. `trust` prints what is there before it asks. A worktree
inherits the trust of the checkout that owns it.

## Which agents

```console
$ devplane agents
claude     Claude Code    npx -y @agentclientprotocol/claude-agent-acp@0.81.2
codex      Codex          npx -y @agentclientprotocol/codex-acp@1.13.1
opencode   OpenCode       opencode acp
copilot    GitHub Copilot npx -y @github/copilot@1.0.88 --acp
gemini     Gemini CLI     npx -y @google/gemini-cli@0.61.0 --acp
```

The agents launched through `npx` are pinned to exact versions, and need Node (with `npx`) and
network access to npm the first time; OpenCode runs the `opencode` on your `PATH`. What an agent supports (`resume`, `load`, `modes`, sign-in) is
advertised when it starts, so `devplane agents` lists it only for agents that have run, with the date.
*Not probed* and *not supported* are different facts.

Most agents sign in through their own terminal. When starting a session fails, Devplane shows the
sign-in methods the agent advertised (for example, *Run `copilot login` in the terminal*).

## Adding an agent

Anything else that speaks the protocol is an entry in `~/.devplane/agents.toml`:

```toml
# ~/.devplane/agents.toml
[agents.kimi]
name    = "Kimi"
command = "npx -y @moonshot/kimi-acp@1.2.0"
```

An entry with a built-in id replaces the built-in, which is how you re-pin one. A file that will not
parse adds nothing and says so in the log. Or point straight at a command:

```sh
devplane change start --agent '/opt/my-agent --acp' "…"
```

A bare word that is not a known id is an error, never a launch attempt.

## Steering a run

| Command | Does |
|---|---|
| `devplane change start --agent <id> "<prompt>"` | a change: an isolated checkout, the agent in it, the gates when it says it is done |
| `devplane change start … --project a --project b` | the same prompt in several projects; every refusal is reported before anything is created |
| `devplane change prompt <change\|run> <text>` | another message, without stopping it; queued until the turn ends |
| `devplane watch <run>` | the conversation as it arrives; `--thinking` includes the reasoning |
| `devplane show <run>` | the run in detail, with its last lines |
| `devplane change stop <run>` | stop it; says first what survives (the change, worktree, branch and record), then asks — `--yes` where nobody can answer |
| `devplane answer <ask>` | answer a permission (`--allow` / `--deny`) or a question (`--option`, `--custom`) |

**An ask outlives the agent that made it.** Quit the host with a question waiting and it stays in the
inbox, answerable with `devplane answer`; the next host delivers the answer by resuming the session.
Only [`[questions] deadline`](@/docs/configuration.md#questions) ends an unanswered ask.

## What a driven agent is given

- **Devplane's read-only MCP server.** Every session is offered `devplane mcp` (the same five tools as
  the [plugin](@/docs/cli.md#devplane-mcp): inbox, change, explain, audit, reports), so the agent can
  read the inbox or its own change's standing. None of them acts.
- **`DEVPLANE_RUN`** in its environment, which `devplane report file` reads as the origin.
- **A clean ending.** Stopping a run, or quitting the host, closes the session over the protocol
  (`session/close`) where the agent offers it, before its processes are ended.
- **Withdrawn questions leave the inbox.** When the agent cancels a permission request it had put to
  you (it moved on, or the turn ended), the item goes away rather than waiting for an answer nothing
  will read.

## Transcripts

A driven run has no window of its own, so Devplane keeps what the agent says on this machine. A
repository can decline:

```toml
# devplane.toml
[transcripts]
keep = false
```

## Cost and context

The protocol reports a running cost total and a context-window level. Devplane records the deltas,
so cost adds up and context does not. The window size is the agent's own figure.

> [!WARNING]
> The protocol makes cost **optional**. An agent that never reports it cannot be stopped by
> `[budget] usd`, so `devplane change show` prints *not reported by this agent — a [budget] ceiling
> cannot bite* rather than `$0.00`.

## Resuming after a restart

Quitting the host ends every agent process it drives; the branch, the worktree and the agent's own
conversation survive, and the run is marked `interrupted`. To continue the same conversation:

```sh
devplane change resume <change>
```

Devplane uses `session/resume` where the agent offers it, and `session/load` (replay suppressed)
where it offers only that, as GitHub Copilot does. An agent that offers neither is told so; a fresh
session is never passed off as a resume. Resuming is never automatic: it spends money.

## Handing over

```sh
devplane attach <run>    # this process becomes `claude --resume <session>`
```

On Unix `attach` uses `exec`, so ctrl-C, resize and the alternate screen behave as without Devplane;
on Windows it starts `claude` and waits. It needs the `claude` binary
([finding it](@/docs/install.md#finding-the-claude-binary)).

Watching sessions you started yourself covers a different set of vendors: see
[Watching sessions](@/docs/observe.md).

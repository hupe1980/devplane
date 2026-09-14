+++
title = "Driving agents"
description = "Dispatch, prompt and answer any agent that speaks the Agent Client Protocol — and add one that isn't built in without waiting for a release."
weight = 11
[extra]
group = "guide"
+++

A session Vibeplane **watches** is one it can show you and raise the window for. A session it
**drives** is one it can answer.

```sh
vibeplane agents                                   # what can be driven
vibeplane dispatch "add rate limiting to /login"   # starts Claude Code here
vibeplane dispatch --agent codex --cwd ../core-lib "review the auth change"
vibeplane say <run> "use the existing middleware"  # another turn
vibeplane decide <run> --request <id> --decision allow
```

A driven run is a run like any other: same board, same project grouping, same inbox. The difference
is that its permission requests can be *answered* from Vibeplane rather than only looked at — and
the same `[policy]` rules that auto-decide a hook decide these first.

## Trust comes first

```sh
vibeplane trust .
```

Required before any agent starts in a repository, because a headless agent runs *that repository's*
own hooks and MCP servers with no dialog of its own. The decision has to be one somebody made
deliberately, once, for that directory. A worktree inherits the trust of the checkout that owns it.

## Which agents

Five are built in, each pinned to an exact version:

| Id | Agent | Launched as |
|---|---|---|
| `claude` | Claude Code | `npx -y @agentclientprotocol/claude-agent-acp@0.76` |
| `codex` | Codex | `npx -y @agentclientprotocol/codex-acp@1.11` |
| `opencode` | OpenCode | `opencode acp` |
| `copilot` | GitHub Copilot | `npx -y @github/copilot@1.0.83 --acp` |
| `gemini` | Gemini CLI | `npx -y @google/gemini-cli@0.59.0 --acp` |

### If an agent needs signing in

Most agents authenticate in their own terminal, and Vibeplane does not try to do it for them — a
login is device codes, browser redirects and a keychain, all of which already work there.

What it does instead is repeat what the agent said. Agents advertise their sign-in methods during
the handshake, so when starting a session fails you get that back rather than a protocol error:

```console
$ vibeplane dispatch "add rate limiting" --agent copilot
could not start a session: Invalid request.
This agent may need signing in first. It offers:
  • Log in with Copilot CLI — Run `copilot login` in the terminal
```

### Continuing a conversation after a restart

A driven run's conversation survives a daemon restart, and the protocol offers **two** ways to carry
it — advertised separately by each agent:

| Method | What it does | Vibeplane |
|---|---|---|
| `session/resume` | continues without replaying history | preferred where offered |
| `session/load` | continues *and* replays the conversation | the fallback, with the replay suppressed |

The replay is suppressed because Vibeplane already wrote that conversation down — a driven run has no
window of its own, so its transcript is kept as the agent speaks, and taking the replay too would
duplicate every sentence. A plan, a usage total or a tool call replayed alongside it *is* kept: those
are state rather than speech.

An agent that offers neither is told so. Vibeplane will not open a fresh conversation and call it a
resume.

> [!NOTE]
> GitHub Copilot advertises `session/load` and not `session/resume`, which is exactly why both are
> implemented.

> [!NOTE]
> **Driving an agent and watching one are different things.** Every agent here can be *started* by
> Vibeplane. A session **you** started, in your own terminal, is only visible where its vendor
> publishes a channel for it — hooks, telemetry, a roster. Today that means Claude Code, and GitHub
> Copilot next. The [observe](/docs/observe/) page says which channels each one has.

Pinned on purpose: an agent that silently upgrades under a conformance suite is an agent whose suite
proves nothing, and `npx` will happily fetch a new major version overnight otherwise.

## Adding an agent

The argument for speaking a standard is that a new agent costs no code. So anything else that speaks
the protocol is four lines in `~/.vibeplane/agents.toml`:

```toml
# ~/.vibeplane/agents.toml
[agents.kimi]
name    = "Kimi"
command = "npx -y @moonshot/kimi-acp@1.2.0"
```

The same file re-pins a built-in agent without waiting for a release — an entry whose id matches
replaces it rather than shadowing it. A file that will not parse adds **nothing** rather than half,
and says so in the log.

Or point straight at a command:

```sh
vibeplane dispatch --agent '/opt/my-agent --acp' "..."
```

A bare word that is not a known id stays an error rather than becoming a launch attempt, so a typo
reports “unknown agent” instead of “no such binary”.

## What a driven run says

A driven run has no window of its own, so Vibeplane is the only place its conversation can appear —
and without it `dispatch` is a black box that can tell you a tool ran and not one word about why.
The protocol streams the answer, the reasoning and the agent's own plan to the client:

```sh
vibeplane tail <run>              # follow it live
vibeplane tail <run> --thinking   # including its reasoning
vibeplane show <run>              # the last few lines, with everything else
```

Chunks are joined into fragments rather than stored one row per syllable, and they live in their own
table: run state is a pure reduction over the event log, and a sentence reduces to nothing.

Transcripts are kept per repository and never leave the machine. An agent's prose can quote
something it read, so a repository can decline:

```toml
[transcripts]
keep = false     # default is true; it reads either way
```

## Cost and context

The protocol reports a **running total** for cost and a **level** for the context window, not
per-request figures. Vibeplane keeps what it has already counted and records the delta, and treats
the window level as a level — without the first a driven run shows `$0.00` for ever, and without the
second the token count becomes the sum of every level ever reported.

The window size is the agent's own figure, which is better than any table of model names.

> [!WARNING]
> The protocol makes the cost field **optional**. An agent that never reports one can never be
> stopped by a `[budget]` ceiling, so `vibeplane work show` prints “not reported by this agent”
> rather than `$0.00` and letting you assume you are covered.

## A driven session outlives the daemon, if the agent lets it

Restarting the daemon ends every agent *process* it was driving — a protocol connection cannot
survive the thing holding it. The branch and worktree are untouched, and work that was mid-flight
appears in the inbox as `interrupted` rather than sitting on the board looking busy.

What is new is that the **conversation** can be picked back up. When a driven run starts, Vibeplane
records the id the agent knows the session by; after a restart, the `interrupted` item offers
**resume**, which reconnects to that same session rather than opening a new one:

```sh
vibeplane work resume <id>
```

That matters more than it sounds. A fresh session in the same worktree would pay again to work out
what the last one already knew, and would read half-finished code without the context that produced
it. Resuming continues the turn the restart interrupted.

It is offered only when it will actually work — the agent advertises `session/resume`, the run
recorded a session id, and Vibeplane is not already holding a session for it. An agent that cannot
resume, or has forgotten the session, gets an honest refusal rather than a silent restart wearing a
resume's name. It is never automatic: resuming spends money and runs an agent in a repository, and
neither should happen because a machine rebooted.

| Agent | Resume |
|---|---|
| Anything advertising `agentCapabilities.sessionCapabilities.resume` | offered |
| Anything that does not | the item says so, and the work is started again by hand |

## Handing over

Vibeplane is not a terminal. When a session needs more than a decision, the right answer is the real
thing, in the right place:

```sh
vibeplane attach <run>    # replaces this process with `claude --resume <id>`
vibeplane focus <run>     # raises the editor window that owns its directory
```

`attach` uses `exec` rather than holding a pseudo-terminal open, so ctrl-C, resize and the alternate
screen behave exactly as they do without Vibeplane in the way.

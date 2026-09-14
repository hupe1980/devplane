+++
title = "Watching sessions"
description = "How Vibeplane sees sessions it did not start: the roster, hooks, OpenTelemetry and the optional status line."
weight = 10
[extra]
group = "guide"
+++

Vibeplane's first job is to know what is happening without changing how you work. It watches
sessions it did not start, on documented interfaces only, through four channels — each of which
answers a different question.

## The four channels

| Channel | Answers | Needs `connect`? |
|---|---|---|
| **Session roster** (`claude agents --json`) | which sessions exist, where, since when | no |
| **Hooks** (HTTP) | what a session is doing, and when it is blocked | yes |
| **OpenTelemetry** | cost, tokens, context usage, tool decisions | yes |
| **Status line** (optional shim) | subscription rate limits, exact context percentage | `--statusline` |

Connecting subscribes to twenty-two of the thirty-three documented hook events. Four are worth naming:

| Event | What it gives you |
|---|---|
| `ElicitationResult` | Clears an inbox item when you answer the question in your own terminal |
| `ConfigChange` | Tells `doctor` that your settings changed — including a managed policy that blocks loopback hooks |
| `TaskCreated` · `TaskCompleted` | A watched session's own task list, on the board |
| `PreModelSwitch` | The new model's context window, before the next turn is measured against the old one |

`WorktreeCreate` is deliberately never installed: configuring it *replaces* Claude Code's own
`git worktree` logic, which would break `claude --worktree`, subagent isolation and background
sessions in every repository on the machine.

## Every blocked state reaches the inbox

`claude agents --json` reports `waitingFor` only while a session is waiting, so every value means a
person is being waited on. Vibeplane raises an item for all five documented ones, and for any value
Claude Code adds later — titled in its own words rather than dropped for being unfamiliar.

Discovery is free. `vibeplane ls` is useful the moment it is installed, because the roster lists
**every live session on the machine** — interactive ones included, not only the background sessions
the Agent View shows.

## Connecting

```sh
vibeplane connect claude
vibeplane doctor            # is anything actually arriving?
vibeplane disconnect claude
```

`connect` writes to `~/.claude/settings.json` — your **user** settings, so every project is covered —
after taking a backup, and adds only two things.

**Hook entries** pointing at `http://127.0.0.1:47831` with a bearer token, merged alongside hooks you
already have. Sixteen events over HTTP plus one that cannot be:

- `SessionStart` is a **command** hook running `vibeplane hook`, because that event accepts only
  `command` and `mcp_tool` hooks. An HTTP entry there is written happily into the settings file and
  then never runs, which looks exactly like a session that started without telling anyone.
- **Two are synchronous**, and they answer different questions. `PermissionRequest` is the instant
  signal that a session is blocked, and it carries the full verdict — it fires only when Claude Code
  is about to ask you, so an allow there skips a prompt that was already coming.
- `PreToolUse` is the other, and it exists because of **auto mode**: there a classifier approves
  routine calls with no prompt, so `PermissionRequest` never fires and a prohibition answered only
  there would not run at all. `PreToolUse` fires before every tool call in every mode. It answers
  with a deny, an ask, or nothing — never an allow, which would skip the classifier too.
- Everything else is `async`, so no hook can make Claude Code feel slower. The two that block answer
  in microseconds and never wait for a person.

**OpenTelemetry variables** pointing at the same loopback port, merged per variable and shown before
they are written. If you already export telemetry somewhere, Vibeplane leaves it alone and says so.

> [!IMPORTANT]
> The flags that would put your prompts or the agent's responses into telemetry —
> `OTEL_LOG_USER_PROMPTS`, `OTEL_LOG_ASSISTANT_RESPONSES`, `OTEL_LOG_TOOL_CONTENT` — are never set.
> Prompt and response text stays redacted, and nothing leaves the machine regardless. The same holds
> for the other dialect: `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT` is never set either,
> and it is off by default.

### Two dialects, one receiver

The telemetry endpoint reads both shapes agents send:

| | Claude Code | Agents on the GenAI semantic conventions |
|---|---|---|
| Signal | log records named `claude_code.*` | **traces** — `invoke_agent` with `chat` and `execute_tool` spans beneath it |
| Endpoint | `/vibeplane/otel/v1/logs` | `/vibeplane/otel/v1/traces` |
| Wire protocol | `http/json` | `http/json` |
| Carries cost | yes — `cost_usd` is Claude Code's own extension | **no.** Those conventions have no notion of money |

Both become the same events, so the board does not care which arrived. The one difference you will
see is the cost column: an agent reporting only GenAI-convention traces shows tokens and no dollars,
because there is nowhere in that schema to put a price. A spending ceiling cannot bind such a run,
and `work show` says so rather than printing `$0.00`.

## GitHub Copilot

`vibeplane connect copilot` writes **one file**, `~/.copilot/hooks/vibeplane.json`, and
`vibeplane disconnect copilot` deletes it. Nothing of yours is merged into or picked back out of —
Copilot loads every `*.json` in that directory, which is a cleaner mechanism than editing a settings
file you own.

Three things differ from Claude Code, and each is Copilot's rather than a preference:

- **The permission gate runs as a `command` hook**, not over HTTP. An HTTP `preToolUse` hook there is
  documented to *fail open* — a timeout or a non-2xx reply falls through to the default permission
  flow — and a prohibition that disappears under load is not one. The informational events stay on
  HTTP, where a process per tool call would cost something for nothing.
- **If the daemon is not running, the hook says nothing and exits zero.** A `command` hook is
  fail-*closed* on an error, so erroring would deny every tool call on your machine the moment
  Vibeplane is stopped. An observer that is absent must not become one that breaks your agent.
- **Telemetry is not installed for you.** Copilot reads it from the environment, and its settings
  equivalent is a managed (organisation) key. `connect` prints the two lines instead:

```sh
export COPILOT_OTEL_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:47831/vibeplane/otel
```

> [!NOTE]
> **There is no session roster for Copilot.** `claude agents --json` is what makes `vibeplane ls`
> work before anything is configured; Copilot publishes no equivalent, so the sequence here is
> connect first, then see. A Copilot session appears once it does something.

Your `vibeplane.toml` rules govern it unchanged. Copilot's own tool names are mapped to the ones the
rules use — `view` is `Read`, `create` is `Write`, `bash` is `Bash` — so `never_auto = ["Read(.env)"]`
stops a Copilot `view` of `.env` and names itself in `vibeplane audit`.

## What `connect` will not do

Two refusals worth knowing about, because both protect something that is easy to break by accident.

**It never creates `allowedHttpHookUrls`.** Defining that key restricts *every* HTTP hook on your
machine to the patterns it lists. Vibeplane appends `http://127.0.0.1:*` when the key already
exists, and leaves it absent otherwise. `doctor` reports when a managed allowlist is blocking
loopback, because the symptom is otherwise indistinguishable from “Claude Code is quiet”.

**It never installs a `WorktreeCreate` hook.** Configuring that event *replaces* Claude Code's own
`git worktree` logic entirely, which would break `claude --worktree`, subagent isolation and
background sessions in every repository on the machine. Worktrees are learned from `CwdChanged`, the
status line and the roster instead.

## The status line

Optional, and off by default, because it wraps a command you configured yourself:

```sh
vibeplane connect claude --statusline
```

The shim runs your original status-line command with the same input, so what you see is unchanged,
and `disconnect` puts it back exactly. It is the only channel that carries **subscription rate
limits** — a five-hour or seven-day window past 90 % becomes a `rate_limit` item in the inbox,
before an agent finds out mid-turn.

## What the board shows

The **working set**, not the inventory.

A machine that has been running agents all week accumulates editor tabs whose processes are still
alive. On the machine this was developed against, 23 sessions were listed and five were in play.
Listing all of them is technically complete and practically useless, so a session that has never
reported anything is counted rather than shown:

```console
8 projects · 23 sessions · 5 working · 2 need you · 16 idle · $4.18
17 dormant (never reported) — vibeplane ls --all
```

A dormant session that starts asking for something joins the working set immediately. This is about
noise, never about silencing a question.

Rows are **grouped by project**, and a run's age is the **session's own start time**, not the moment
the daemon first noticed it — otherwise every session discovered in one poll shows the same age and
sorting by recency sorts by nothing.

## What `doctor` will tell you that nothing else does

Three failures are invisible on the board, because in each one the symptom is an
*absence* — and an absence looks exactly like nothing having happened:

| Reported as | What it means |
|---|---|
| `channels` | which observation channels are arriving, how fast, and the worst latency seen. A channel that stopped is silence, and silence is what a quiet machine looks like too |
| `unreadable configuration` | a repository whose `vibeplane.toml` will not load. The rules it had stay cached — but a restarted daemon has none to cache, so that project's `never_auto` list is simply not in force |
| `unreadable rows` | stored runs or work this build can no longer decode. The schema changes here without migrations on purpose, so a changed shape makes rows vanish from the board. A **work** row is the one to read first: it names a branch and a worktree, so losing it orphans a checkout nobody is left to tell you about. Observations rebuild from the providers, so deleting the database is safe |

## Who wins when channels disagree

A roster poll runs every two seconds; a hook is the session speaking. The poll must never overwrite
what a session said about itself, or a blocked run would flick back to “working” and drop out of the
inbox. So:

- a **background** row carries `state`, and the provider's daemon owns that process — its verdict wins;
- an **interactive** row contributes identity (pid, name, cwd, entrypoint) and sets state only for a
  session no hook has ever spoken for.

## Transcripts

A session **you** started has no transcript here, and that is not a gap waiting to be filled. Hooks
carry lifecycle and tool inputs; telemetry redacts prompts and responses; the transcript files are
documented as internal, which is the one dependency this project refuses. The conversation is
already on your screen in the window that owns it, and `vibeplane focus <run>` raises it.

A run Vibeplane **drives** is the opposite case in every respect — see [Driving agents](/docs/agents/).

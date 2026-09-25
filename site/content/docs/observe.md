+++
title = "Watching sessions"
description = "How Devplane sees agent sessions it did not start: the roster, command hooks, OpenTelemetry and the optional status line — per vendor, and what each state means."
weight = 10
[extra]
group = "guide"
+++

Devplane watches sessions you started yourself, in a terminal or an editor, through documented
interfaces only. The hooks write straight to the store, so nothing has to be running for the record
to be kept. Sessions Devplane starts are covered in [Driving agents](@/docs/agents.md). The
workbench's **Sight** panel shows these channels live.

## The channels

| Channel | Answers | Needs `connect`? | Needs a host? |
|---|---|---|---|
| **Roster** (`claude agents --json`) | which Claude Code sessions exist, where, since when | no | no — `ls` reads it itself |
| **Command hooks** (`devplane hook`) | what a session is doing, when it is blocked, and the permission gate | yes | no — the hook writes to the store |
| **OpenTelemetry** | cost, tokens, context use | yes | yes — it arrives on a socket |
| **Status line** (optional) | rate limits and when they reset, model, context window size, cost, lines changed | `--statusline` | no — the shim writes to the store |

A running host tails the hook-written rows every 500 ms. With no host, `devplane ls` and `devplane
inbox` replay them in memory.

## Per vendor

| Vendor | How it is watched | State |
|---|---|---|
| **Claude Code** | roster, hooks, telemetry, status line | proved end to end |
| **GitHub Copilot** | hooks, telemetry you export yourself | implemented, not yet proved on a live session |
| **Codex** | hooks, in Claude Code's payload shapes | implemented, not yet proved on a live session; needs approval in Codex's own dialog |
| **OpenCode** | its own event feed, when you point Devplane at `opencode serve` | read only; not yet proved on a live server |
| **Gemini CLI** | not watched | can only be driven |

`devplane doctor` prints this table for your machine.

### Claude Code

```sh
devplane connect claude      # diffs ~/.claude/settings.json, then asks
devplane doctor              # is anything arriving?
devplane disconnect claude   # removes exactly what connect added
```

`connect` edits your **user** settings, so every project is covered, and merges beside hooks you
already have:

- **22 command hooks**, each running `devplane hook` with a five-second bound. Two decide:
  `PermissionRequest` (a session is about to ask you) and `PreToolUse` (every tool call, including in
  auto mode). They answer deny, ask, or nothing — **never allow**. Every other hook is `async`.
- **OpenTelemetry variables** pointing at the host's loopback port, with the bearer token in
  `OTEL_EXPORTER_OTLP_HEADERS`. An exporter you already configured is left alone.

The flags that would put prompt or response text into telemetry (`OTEL_LOG_USER_PROMPTS`,
`OTEL_LOG_ASSISTANT_RESPONSES`, `OTEL_LOG_TOOL_CONTENT`) are never set.

`connect` does not install `WorktreeCreate` (it would replace Claude Code's own worktree logic) or
create `allowedHttpHookUrls` (it restricts every HTTP hook on the machine; `doctor` reports when a
managed allowlist blocks loopback).

### GitHub Copilot

```sh
devplane connect copilot     # writes ~/.copilot/hooks/devplane.json
devplane disconnect copilot  # deletes it
```

One file, nothing merged. Every event is a `command` hook, because an HTTP `preToolUse` hook in
Copilot fails open. Copilot's tool names are mapped to the ones rules use (`view` → `Read`, `create` → `Write`, `str_replace_editor` → `Edit`, `bash` →
`Bash`, `web_fetch` → `WebFetch`), so `never_auto = ["Read(.env)"]` stops a Copilot `view` of `.env`.

Copilot reads telemetry settings from the environment, so `connect` prints the lines to add:

```sh
export COPILOT_OTEL_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:47831/devplane/otel
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer $(cat ~/.devplane/token)"
```

Copilot publishes no roster, so a session appears once it does something.

### Codex

```sh
devplane connect codex       # merges entries into ~/.codex/hooks.json
devplane disconnect codex    # picks exactly those back out
```

With `$CODEX_HOME` set, the file is `$CODEX_HOME/hooks.json`.

Codex hooks take Claude Code's payloads and answers, so the same `devplane hook` serves, registered
as `devplane hook --vendor codex`. Ten of its twelve events are registered. No telemetry is
installed.

> [!IMPORTANT]
> **Codex silently skips any hook you have not approved in its own trust dialog.** Until you approve
> them, nothing is watched or gated.

### OpenCode

```sh
opencode serve --port 4096
export DEVPLANE_OPENCODE_URL=http://127.0.0.1:4096
```

Opt-in: Devplane never scans for a server. The host subscribes to the feed and lists sessions from
it. A `question.rejected` event is recorded with the authority **nobody**, because it names no cause.
Questions are shown but cannot be answered from Devplane. The feed does not replay, so a dropped
connection is a gap; Devplane reports whether it is connected.

## The status line

Optional, and off by default because it wraps your own status-line command:

```sh
devplane connect --statusline claude
```

The shim runs your original command with the same input, so what you see is unchanged;
`disconnect claude` restores it. It is the only source of **subscription rate limits** (a five-hour,
seven-day or spend window past 90 % becomes a `rate_limit` inbox item), and also carries the model,
context window size, cost, lines changed and the Claude Code version:

```console
$ devplane show 7c
  model      claude-opus-5
  cost       $2.5000 over 41 requests
  changed    +156 −23 lines
  limits     5-hour 88% used, resets in 39m
  harness    Claude Code 2.1.276
```

Nothing depends on it; every fact falls back to another channel, or to absent.

## What the Sessions list shows

The **working set**. A session that is neither doing nor asking for something is counted, not
listed:

```console
8 projects · 23 sessions · 5 working · 2 need you · 4 idle · $4.18
12 quiet (nothing heard for hours) — devplane ls --all
```

A session that is **working** or **asking** is never counted away, however old. Rows are grouped by
project.

- **Blocked states always reach the inbox.** Every `waitingFor` value the roster reports becomes an
  item. An item for a session Devplane did not start has no Allow or Deny; the action is to open its
  window (`devplane focus <run>`).
- **An agent waiting on a command it started** (usually a test suite) is not waiting on you. Devplane
  reads the process table and shows it as `running a command it started`, off the inbox.

## What each state means

| | State | Meaning | Counted as |
|---|---|---|---|
| `●` | `working` · `starting` | the agent is generating | working |
| `◆` | `waiting` | **asking you something** — a permission, a question, a plan to approve | needs you |
| `○` | `idle` | alive, waiting for a prompt | idle |
| `✓` | `completed` | the turn ended on its own | idle |
| `✗` | `failed` | the turn ended with an error | failed |
| `?` | `lost` | Devplane expected a process and could not find it at startup | failed |
| `◌` | `stopped` | **you** stopped it | idle |
| `⊘` | `interrupted` | **Devplane** stopped it, because the host was quitting | idle |

`interrupted` means the work did not finish. Its branch and worktree are untouched, and where the
agent can resume, the inbox offers `devplane change resume`.

## Modes

```sh
devplane modes
```

Which projects are deciding without you, least supervised first. For Claude Code it reads the modes
from the settings files and says who set each. An ACP agent's mode is shown in its own words. A
session that reported no mode shows as unknown. Devplane never sets a mode.

## GitHub

```sh
devplane issues          # every project's open issues, what needs you first
devplane issues --ready  # this repo's issues with [github] ready_label
devplane prs             # every project's open pull requests
```

A running host reads them through your own `gh` after it starts and every five minutes. *Needs you*
means assigned to you, a review requested from you, or your own pull request that is red, has changes
requested, or is approved and waiting. Each is also an inbox item. Nothing here writes to GitHub.

## `devplane doctor`

The failures that look like a quiet machine:

| Section | Tells you |
|---|---|
| host | whether a host answers, and which binary it runs |
| watched | each vendor channel: read, unproved, not published or not checked |
| claude code | whether the hooks are installed and the gate **answers** — it runs the installed gate with a probe and times it |
| channels | which channels are arriving, how fast, the worst latency |
| unreadable configuration | a `devplane.toml` that will not load — every gated call there is put in front of a person |
| unwritten | events and decisions the store refused; the record is missing that much |
| leaked agents | an agent a killed host left running, still spending. Also a critical inbox item, with the `kill -TERM -<pgid>` to end it |
| unreadable rows | rows this build cannot decode; read the `change` count first |

## Transcripts

A session **you** started has no transcript in Devplane: hooks carry lifecycle and tool inputs,
telemetry redacts prompts and responses, and the vendor's transcript files are internal. The
conversation is in the window that owns it: `devplane focus <run>` raises it, and `devplane rewind
<run>` lists the files its shell commands named for writing, which Claude Code's `/rewind` does not
restore. Driven runs keep transcripts; see [Driving agents](@/docs/agents.md).

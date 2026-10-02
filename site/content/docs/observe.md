+++
title = "Watching sessions"
description = "How Devplane sees agent sessions it did not start: the roster, command hooks, OpenTelemetry and the optional status line — per vendor, and what each state means."
weight = 10
[extra]
group = "guide"
+++

Devplane watches sessions you started (terminal or editor) through documented interfaces only. Hooks
write straight to the store, so nothing has to be running. Sessions Devplane starts:
[Driving agents](@/docs/agents.md). The workbench's **Sight** panel shows these channels live.

## The channels

| Channel | Answers | Needs `agent connect`? | Needs a host? |
|---|---|---|---|
| **Roster** (`claude agents --json`) | which Claude Code sessions exist, where, since when | no | no — `status` reads it itself |
| **Command hooks** (`devplane hook`) | what a session is doing, when it is blocked, and the permission gate | yes | no — the hook writes to the store |
| **OpenTelemetry** | cost, tokens, context use | yes | yes — it arrives on a socket |
| **Status line** (optional) | rate limits and when they reset, model, context window size, cost, lines changed | `--statusline` | no — the shim writes to the store |

## Per vendor

| Vendor | How it is watched | State |
|---|---|---|
| **Claude Code** | roster, hooks, telemetry, status line | proved end to end |
| **GitHub Copilot** | hooks, telemetry you export yourself | untested against a live session |
| **Codex** | hooks, in Claude Code's payload shapes | untested against a live session; needs approval in Codex's own dialog |
| **OpenCode** | its own event feed, when you point Devplane at `opencode serve` | read only; untested against a live server |
| **Gemini CLI** | not watched | can only be driven |

`devplane doctor` prints this table for your machine.

### Claude Code

```sh
devplane agent connect claude     # diffs ~/.claude/settings.json, then asks (--yes in a script)
devplane agent disconnect claude  # removes exactly what connect added
```

`agent connect` edits your **user** settings (every project), beside hooks you already have:

- **22 command hooks** running `devplane hook` by full path. `PermissionRequest` and `PreToolUse`
  decide (deny, ask or nothing — **never allow**); the rest are `async`. Bound: 5 s each, except
  `PermissionRequest` at 150 s, since [`[questions] hold`](@/docs/configuration.md#questions) may wait
  up to two minutes.
- A moved or reinstalled binary leaves hooks running nothing (**gate off** in `doctor`); run
  `agent connect` again ([the gate is off](@/docs/troubleshooting.md#the-gate-is-off)).
- **OpenTelemetry variables** for the host's port (the running host's, else 47831) with the
  telemetry-only token ([Security](@/docs/security.md#loopback-plus-a-bearer-token)). An exporter you
  configured is left alone. Prompt and response logging flags are never set.
- Not installed: `WorktreeCreate` (would replace Claude Code's worktree logic) and
  `allowedHttpHookUrls` (machine-wide); `doctor` reports a managed allowlist blocking loopback.

### GitHub Copilot

```sh
devplane agent connect copilot     # writes ~/.copilot/hooks/devplane.json
devplane agent disconnect copilot  # deletes it
```

- One file, nothing merged; every event is a `command` hook (Copilot's HTTP `preToolUse` fails open).
- Tool names map to rule names (`view` → `Read`, `create` → `Write`, `str_replace_editor` → `Edit`,
  `bash` → `Bash`, `web_fetch` → `WebFetch`), so `deny = ["Read(.env)"]` stops a `view` of `.env`.
- Telemetry comes from the environment; `agent connect` prints the lines:

```sh
export COPILOT_OTEL_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:47831/devplane/otel
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer $(cat ~/.devplane/telemetry-token)"
```

No roster: a session appears once it does something.

### Codex

```sh
devplane agent connect codex     # merges entries into ~/.codex/hooks.json
devplane agent disconnect codex  # picks exactly those back out
```

- `$CODEX_HOME/hooks.json` when `$CODEX_HOME` is set.
- Same payloads as Claude Code: `devplane hook --vendor codex`, ten of twelve events. No telemetry.
- **On Codex, a question is a refusal.** Codex runs a call answered `ask`, so where Devplane would ask
  (an `ask` rule, an unreadable line, a broken `devplane.toml`) it answers `deny` and says why.

> [!IMPORTANT]
> **Codex silently skips any hook you have not approved in its own trust dialog.** Until you approve
> them, nothing is watched or gated.

### OpenCode

```sh
opencode serve --port 4096
export DEVPLANE_OPENCODE_URL=http://127.0.0.1:4096
```

- Opt-in; Devplane never scans for a server. The host lists sessions from the feed.
- `question.rejected` is recorded with authority **nobody** (it names no cause).
- Questions are shown, not answerable from Devplane.
- The feed does not replay: a dropped connection is a gap; the connection state is reported.

## The status line

Optional; it wraps your status-line command:

```sh
devplane agent connect --statusline claude
```

Your command still runs with the same input; `agent disconnect claude` restores it. It is the only
source of **subscription rate limits** (a five-hour, seven-day or spend window past 90 % raises
`rate_limit`), plus model, context size, cost, lines changed and Claude Code version:

```console
$ devplane run show 7c
  model      claude-opus-5
  cost       $2.5000 over 41 requests
  changed    +156 −23 lines
  limits     5-hour 88% used, resets in 39m
  harness    Claude Code 2.1.276
```

## What the Sessions list shows

The **working set**: sessions neither doing nor asking are counted, not listed.

```console
8 projects · 23 sessions · 5 working · 2 need you · 4 idle · $4.18
12 quiet (nothing heard for hours) — devplane status --all
```

- A **working** or **asking** session is always listed. Rows group by project.
- Every roster `waitingFor` becomes an inbox item. A watched session's item has Allow/Deny only with a
  [hold](@/docs/permissions.md#answering-a-watched-session-s-permission); otherwise `devplane run
  attach <run>` or its own window.
- An agent waiting on its own command (a test suite) shows `running a command it started`, off the
  inbox.

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

`interrupted`: branch and worktree untouched; the inbox offers `devplane change resume` where the agent
can resume.

## Modes

```sh
devplane status --modes
```

Least supervised first. Claude Code modes come from settings files, with who set each; ACP agents'
in their own words; none reported shows unknown. Devplane never sets a mode.

## GitHub

```sh
devplane issue list          # every project's open issues, what needs you first
devplane issue list --ready  # this repo's issues with [github] ready_label
devplane pr list             # every project's open pull requests
```

- A running host reads them with your sign-in on start, after a sign-in, and every five minutes: one
  request per repository plus one search per host.
- *Needs you*: assigned to you, review requested, or your own PR red, changes requested, or approved
  and waiting. Found by search, so old or past-first-page items count. Each is an inbox item.
- Nothing is written to GitHub.

## Transcripts

A session **you** started has no transcript in Devplane (hooks carry lifecycle and tool inputs;
telemetry is redacted). The conversation is in its own window, or `devplane run attach <run>`. Driven
runs keep transcripts ([Driving agents](@/docs/agents.md)).

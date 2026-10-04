+++
title = "Watching sessions"
description = "How Devplane sees agent sessions it did not start: the roster, command hooks, OpenTelemetry and the optional status line — per vendor, and what each state means."
weight = 10
[extra]
group = "guide"
+++

Devplane watches sessions you started yourself, in a terminal or an editor, through documented
interfaces only. Hooks write straight to the store, so nothing has to be running. Sessions Devplane
starts are [driven](@/docs/agents.md) and report in full. The workbench's **Sight** panel shows these
channels live.

## The channels

| Channel | Answers | Needs `agent connect`? | Needs a host? |
|---|---|---|---|
| **Roster** (`claude agents --json`) | which Claude Code sessions exist, where, since when | no | no |
| **Command hooks** (`devplane hook`) | what a session is doing, when it is blocked, and the permission gate | yes | no: the hook writes to the store |
| **OpenTelemetry** | cost, tokens, context use | yes | yes: it arrives on a socket |
| **Status line** (optional) | rate limits and resets, model, context size, cost, lines changed | `--statusline` | no |

## Per vendor

| Vendor | How it is watched | State |
|---|---|---|
| **Claude Code** | roster, hooks, telemetry, status line | proved end to end |
| **GitHub Copilot** | hooks, telemetry you export yourself | untested against a live session |
| **Codex** | hooks, in Claude Code's payload shapes | untested against a live session; needs approval in Codex's own dialog |
| **OpenCode** | its event feed, when you point Devplane at `opencode serve` | untested against a live server |
| **Gemini CLI** | not watched | can only be driven |

`devplane doctor` prints this for your machine, with the date each row was last read against the
vendor's documentation.

### Claude Code

```sh
devplane agent connect claude     # diffs ~/.claude/settings.json, then asks (--yes in a script)
devplane agent disconnect claude  # removes exactly what connect added
```

`agent connect` edits your **user** settings, beside hooks you already have:

- **22 command hooks** running `devplane hook` by full path. `PreToolUse` and `PermissionRequest`
  decide: deny, ask, or nothing — and allow only through a
  [grant you wrote](@/docs/permissions.md#grants-your-own-rule-per-project), on `PermissionRequest`.
  The rest are `async`. Each is bounded at 5 s, `PermissionRequest` at 150 s so a
  [`[questions] hold`](@/docs/configuration.md#questions) of up to two minutes fits.
- **OpenTelemetry variables** for the host's port (the running host's, else 47831) with the
  telemetry-only token ([Security](@/docs/security.md#loopback-plus-a-bearer-token)). An exporter you
  configured is left alone; prompt and response logging are never turned on.
- Not installed: `WorktreeCreate` (it would replace Claude Code's worktree logic) and
  `allowedHttpHookUrls`. `doctor` reports a managed allowlist that blocks loopback.

A moved or reinstalled binary leaves the hooks running nothing: `doctor` says **gate off**; connect
again ([the gate is off](@/docs/troubleshooting.md#the-gate-is-off)).

### GitHub Copilot

```sh
devplane agent connect copilot     # writes ~/.copilot/hooks/devplane.json
devplane agent disconnect copilot  # deletes it
```

- One file, nothing merged; every event is a `command` hook (Copilot's HTTP `preToolUse` fails open).
- Tool names map to rule names (`view` → `Read`, `create` → `Write`, `str_replace_editor` → `Edit`,
  `bash` → `Bash`, `web_fetch` → `WebFetch`), so `deny = ["Read(.env)"]` stops a `view` of `.env`.
- No roster: a session appears once it does something.
- Telemetry comes from the environment; `agent connect` prints the lines:

```sh
export COPILOT_OTEL_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:47831/devplane/otel
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer $(cat ~/.devplane/telemetry-token)"
```

### Codex

```sh
devplane agent connect codex     # merges entries into ~/.codex/hooks.json ($CODEX_HOME/hooks.json if set)
devplane agent disconnect codex  # takes exactly those back out
```

- `devplane hook --vendor codex` on ten of Codex's twelve events. No telemetry.
- **On Codex, a question is a refusal.** Codex runs a call answered `ask`, so where Devplane would ask
  (an `ask` rule, an unreadable line, a broken `devplane.toml`) it answers `deny` and says why.

> [!IMPORTANT]
> **Codex silently skips any hook you have not approved in its own trust dialog.** Until you approve
> them, nothing is watched or gated.

### OpenCode

```sh
opencode serve --port 4096
export DEVPLANE_OPENCODE_URL=http://127.0.0.1:4096   # before starting the host
```

- Opt-in; Devplane never scans for a server. The host lists sessions from the feed.
- Questions are shown, not answerable from Devplane. `question.rejected` is recorded with authority
  **nobody**.
- The feed does not replay: a dropped connection is a gap, and the connection state is reported.

## The status line

Optional; it wraps your status-line command, which still runs with the same input:

```sh
devplane agent connect --statusline claude
```

`agent disconnect claude` restores it. It is the only source of **subscription rate limits** (a
window past 90 % raises `rate_limit`), plus model, context size, cost, lines changed and the Claude
Code version:

```console
$ devplane run show 7c
  model      claude-opus-5
  cost       $2.5000 over 41 requests
  changed    +156 −23 lines
  limits     5-hour 88% used, resets in 39m
  harness    Claude Code 2.1.276
```

## What the Sessions list shows

The **working set**: sessions neither working nor asking are counted, not listed.

```console
8 projects · 23 sessions · 5 working · 2 need you · 4 idle · $4.18
12 quiet (nothing heard for hours) — devplane status --all
```

- Every roster `waitingFor` becomes an inbox item. A watched session's permission has Allow/Deny only
  with a [hold](@/docs/permissions.md#answering-a-watched-session-s-permission); otherwise answer it
  in its own window or with `devplane run attach <run>`.
- An agent waiting on its own command (a test suite) shows *running a command it started*, off the
  inbox.

## What each state means

| | State | Meaning | Counted as |
|---|---|---|---|
| `●` | `working` · `starting` | the agent is generating | working |
| `◆` | `waiting` | **asking you something**: a permission, a question, a plan to approve | needs you |
| `○` | `idle` | alive, waiting for a prompt | idle |
| `✓` | `completed` | the turn ended on its own | idle |
| `✗` | `failed` | the turn ended with an error | failed |
| `?` | `lost` | Devplane expected a process and could not find it at startup | failed |
| `◌` | `stopped` | **you** stopped it | idle |
| `⊘` | `interrupted` | **Devplane** stopped it, because the host was quitting | idle |

After `interrupted`, the branch and worktree are untouched, and the inbox offers `devplane change
resume` where the agent can resume.

## Modes

```sh
devplane status --modes
```

Least supervised first. Claude Code's modes come from its settings files, with who set each; ACP
agents report theirs in their own words; none reported shows unknown. Devplane never sets a mode.

## GitHub

```sh
devplane issue list          # every project's open issues, what needs you first
devplane issue list --ready  # this repo's issues with [github] ready_label
devplane pr list             # every project's open pull requests
```

- A running host reads them with your sign-in at start, after a sign-in, and every five minutes.
- *Needs you*: assigned to you, review requested, or your own pull request red, with changes
  requested, or approved and waiting. Each is an inbox item.
- Nothing is written to GitHub.

## Transcripts

A session **you** started has no transcript in Devplane: hooks carry lifecycle and tool inputs, and
telemetry is redacted. The conversation is in its own window, or `devplane run attach <run>`. Driven
runs keep transcripts ([Driving agents](@/docs/agents.md)).

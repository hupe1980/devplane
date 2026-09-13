# Vibeplane

**The local-first control plane for AI coding agents.** One binary that sees every Claude Code
session on your machine — in a terminal, in VS Code, in the desktop app — and tells you which ones
need you.

```console
$ vibeplane ls
8 projects · 23 sessions · 5 working · 2 need you · 16 idle · $4.18

◆ saas-7c             vscode     62%   $1.04   3m  Keep the legacy /v1/login route?
● core-lib-a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace
◆ mobile-04           cli         –    $0.12   8m  Permission: Bash rm -rf node_modules
○ blog-e2             vscode     12%   $0.02  41m  waiting for a prompt
```

> **Status: early.** The observer (M0) runs, and has been verified against Claude Code 2.1.270 on a
> machine with 23 live sessions. Driving agents, work items and gates are designed, not built.

## Why

Five VS Code windows, five agents, and no way to know which one is stuck. Claude Code's own
`claude agents` shows background sessions in one directory; the VS Code Agents window shows what VS
Code started; neither spans your terminal, your editor and the desktop app at once, and none of them
knows what *done* means for your project.

Vibeplane watches all of them, on documented interfaces, and stays out of the way.

## Install

```sh
cargo install vibeplane      # or: brew install hupe1980/tap/vibeplane   (coming with 0.1)
```

## Use

```sh
vibeplane ls                 # every session on this machine — works immediately, no setup
vibeplane connect claude     # add live state: hooks + telemetry, into your user settings
vibeplane open               # the board in a browser, updating live

vibeplane inbox              # only what needs a human, most urgent first
vibeplane focus <run>        # raise the editor window that owns a session
vibeplane attach <run>       # hand the terminal to Claude Code, resuming that session
vibeplane snooze <run>       # not this one, not now
vibeplane doctor             # is anything actually arriving?
```

`vibeplane ls` works before you connect anything: sessions are discovered from Claude Code's own
roster. Connecting is what adds cost, context usage, blocking and the permission gate.

Every command starts the daemon if it is not already running, and every command takes `--json`.

### The board

`vibeplane open` serves a single page from the daemon — no build step, no CDN, no account. It
updates over server-sent events, groups sessions by project, and puts what needs you at the top.
`j`/`k` to move, `f` to raise the editor window, `a` to copy the attach command, `s` to snooze.

It works on a laptop with no network, and the token is handed over once in the URL and then stripped
from the address bar so it cannot end up in a screenshot.

### Notifications

A session that blocks on a question or a permission raises a desktop notification — once, never
twice for the same thing, and only for what genuinely needs a person. `VIBEPLANE_NOTIFY=0` turns
them off.

### What `connect` changes

It writes to `~/.claude/settings.json`, keeping a backup, and adds only:

- **hook entries** pointing at `http://127.0.0.1:47831` with a bearer token — merged alongside hooks
  you already have, and removed exactly by `vibeplane disconnect claude`;
- **OpenTelemetry variables** pointing at the same loopback port, so cost and token counts arrive.
  If you already export telemetry somewhere, Vibeplane leaves it alone and says so;
- with `--statusline`, a **wrapper around your status line** — the only source of subscription rate
  limits. It runs your original command with the same input, so what you see is unchanged, and
  `disconnect` puts it back exactly.

It never sets the flags that would put your prompts or the agent's responses into telemetry, never
creates `allowedHttpHookUrls` (creating it would restrict every other HTTP hook on your machine),
and never installs a `WorktreeCreate` hook (configuring one replaces Claude Code's own worktree
logic).

Nothing leaves the machine. There is no account, no relay and no telemetry of our own.

## How it works

```
  Claude Code sessions                     vibeplane serve (the daemon)
  ────────────────────                     ───────────────────────────
  terminal ─┐                          ┌─ hooks    → lifecycle, blocking, policy
  VS Code  ─┼─ hooks + OpenTelemetry ─►├─ OTLP     → cost, tokens, entrypoint
  desktop  ─┘                          ├─ roster   → discovery, background state
  claude --bg ── claude agents --json ─┘└─ SQLite  → events, runs, search
                                             │
                                    board · inbox · SSE
                                             │
                                    CLI  ·  browser  ·  your scripts
```

State is a pure reduction over observed events, so the board is rebuildable and the inbox is derived
rather than stored. Details in `concepts/` (not published — see below).

## More

Source, architecture notes and roadmap: <https://github.com/hupe1980/vibeplane>

## License

MIT OR Apache-2.0

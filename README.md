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
vibeplane inbox              # only what needs a human, most urgent first
vibeplane focus <run>        # raise the editor window that owns a session
vibeplane doctor             # is anything actually arriving?
```

`vibeplane ls` works before you connect anything: sessions are discovered from Claude Code's own
roster. Connecting is what adds cost, context usage, blocking and the permission gate.

Every command starts the daemon if it is not already running, and every command takes `--json`.

### What `connect` changes

It writes to `~/.claude/settings.json`, keeping a backup, and adds only:

- **hook entries** pointing at `http://127.0.0.1:47831` with a bearer token — merged alongside hooks
  you already have, and removed exactly by `vibeplane disconnect claude`;
- **OpenTelemetry variables** pointing at the same loopback port, so cost and token counts arrive.
  If you already export telemetry somewhere, Vibeplane leaves it alone and says so.

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

## Layout

| Path | What |
|---|---|
| `crates/vibeplane` | The binary: daemon, receivers, HTTP API, CLI |
| `crates/vibeplane-domain` | Types and pure functions. No I/O |
| `crates/vibeplane-core` | The reducer, the attention engine, the permission policy |
| `crates/vibeplane-observe` | Hooks, OpenTelemetry, roster, status line, connect, discovery |
| `crates/vibeplane-store` | SQLite: events, runs, full-text search |
| `scripts/` | Spec fetching and the checks that keep the notes honest |

Architecture notes live in `concepts/` and third-party specifications in `specs/`; both are
gitignored. `scripts/fetch-specs.sh` rebuilds `specs/`, and `scripts/verify-claims.sh` checks every
integration claim in the notes against it.

## Roadmap

| | | |
|---|---|---|
| **M0 Sight** | see every session, on every surface | ⏳ building |
| **M1 Hands** | drive any agent that speaks the [Agent Client Protocol](https://agentclientprotocol.com) | next |
| **M2 Proof** | work items, verification gates, GitHub — *verified done*, not *agent says done* | designed |
| **M3 Pipelines** | implement → review → verify, with agents checking agents | designed |

## Development

```sh
cargo test                   # 93 tests, no network, no provider needed
cargo run -- serve           # the daemon in the foreground
VIBEPLANE_HOME=/tmp/vp cargo run -- ls    # an isolated instance
```

`VIBEPLANE_CLAUDE_BIN` points at a `claude` binary if yours is not on `PATH` — which is common, since
the VS Code extension ships its own copy and installs nothing.

## License

MIT OR Apache-2.0

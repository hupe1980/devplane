# Vibeplane

**The local-first control plane for AI coding agents.** One binary that sees every Claude Code
session on your machine — in a terminal, in VS Code, in the desktop app — tells you which ones need
you, and drives any agent that speaks the [Agent Client Protocol](https://agentclientprotocol.com).

```console
$ vibeplane ls
8 projects · 23 sessions · 5 working · 2 need you · 16 idle · $4.18

◆ saas-7c             vscode     62%   $1.04   3m  Keep the legacy /v1/login route?
● core-lib-a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace
◆ mobile-04           cli         –    $0.12   8m  Permission: Bash rm -rf node_modules
○ blog-e2             vscode     12%   $0.02  41m  waiting for a prompt
```

> **Status: early, but the whole loop runs.** Watching is verified against Claude Code 2.1.270 on a
> machine with 23 live sessions. Driving works for any ACP agent. And work is *verified*: an agent
> that says it is done but fails the project's own checks does not get to be done. GitHub, the
> durable runtime and declared pipelines are designed, not built.

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

### Driving an agent

```sh
vibeplane agents                                   # what can be driven
vibeplane dispatch "add rate limiting to /login"   # starts Claude Code here
vibeplane dispatch --agent codex --cwd ../core-lib "review the auth change"
vibeplane say <run> "use the existing middleware"  # another turn
vibeplane decide <run> --request <id> --option allow
```

A driven run is a run like any other: same board, same project grouping, same inbox. The difference
is that its permission requests can be *answered* from Vibeplane rather than only looked at — and
the same `[policy]` rules that auto-decide a hook decide these first.

### Verified done

This is the part that earns the tool.

```sh
vibeplane trust .                                     # once per repository
vibeplane work start "fix the flaky login test" --kind bug
vibeplane work list
```

`work start` makes an isolated checkout at `.claude/worktrees/<slug>` on its own branch, runs your
setup command, copies the gitignored files you name, and puts an agent in it. **When the agent says
it is finished, Vibeplane runs your checks.** Green means a human should look; red means the
failures go back to that same session, bounded, and then you are asked.

An agent that claims success without earning it reaches `failed`, never `review`.

```toml
# vibeplane.toml — committed, so the definition of done is the project's, not the agent's
[gates]
check   = ["pnpm typecheck", "pnpm test -- --run"]
timeout = "10m"
on_fail = "feedback"        # feedback | escalate | ignore
max_feedback_rounds = 2

[workspace]
setup   = "pnpm install --frozen-lockfile"
include = [".env"]

[policy]
auto_allow = ["Read", "Bash(pnpm test *)"]
never_auto = ["Bash(git push *)"]
```

Gates run as children of the daemon, never through the agent — letting the thing being checked
choose the check is the one mistake this whole layer exists to avoid. No `vibeplane.toml` is fine
too: nothing is verified, and nothing pretends to be.

`vibeplane trust` is required before any agent starts in a repository, because a headless agent
runs *that repository's* hooks and MCP servers without asking.

Agents come from the ACP registry, pinned to versions the conformance suite has run against; any
command that speaks the protocol works too:

```sh
vibeplane dispatch --agent '/opt/my-agent --acp' "..."
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

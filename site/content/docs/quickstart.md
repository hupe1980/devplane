+++
title = "Quickstart"
description = "From nothing to a live board, a driven agent and a verified piece of work — in about five minutes."
weight = 2
[extra]
group = "start"
+++

## 1. See what is already running

```sh
vibeplane ls
```

```console
8 projects · 23 sessions · 5 working · 2 need you · 4 idle · $4.18
12 quiet (nothing heard for hours) — vibeplane ls --all

saas
  ◆ 7c         vscode     62%   $1.04   3m  Keep the legacy /v1/login route?

core-lib  ·  2 sessions
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace
  ○ 4f         vscode     12%   $0.02  41m  waiting for a prompt
```

No setup was needed for that: sessions are discovered from Claude Code's own roster.

Rows are **grouped by project**, because that is the unit you think in — nine sessions on one
repository are one line of context, not nine rows that differ by a hash. Sessions that have never
reported anything are **counted, not listed**: a machine that has been running agents all week has
editor tabs whose processes are still alive. One that starts asking for something joins the working
set immediately.

```sh
vibeplane ls --all           # including the quiet ones
vibeplane ls --project saas  # one project; matches any part of the name
vibeplane ls --needs-you     # only what is waiting on a human
```

## 2. Add live state

Discovery is free. Cost, context usage, blocking and the permission gate need Claude Code to talk to
Vibeplane:

```sh
vibeplane connect claude
```

This merges hook entries and OpenTelemetry variables into your **user** settings, keeping a backup,
and removes exactly those entries again on `disconnect`. It never touches your prompts: the flags
that would put prompt or response text into telemetry are never set.

```sh
vibeplane doctor    # is anything actually arriving?
```

See [Watching sessions](/docs/observe/) for what each channel provides.

## 3. Open the board

```sh
vibeplane open
```

One page served from the daemon on loopback — no build step, no CDN, no account. It updates live,
groups by project, and puts what needs you at the top. It is a document rather than a canvas: every
state has a word as well as a colour, the sections are lists, and one polite live region says how
many things need you — so it reads aloud, and it survives being screenshotted in greyscale.

| Key | What |
|---|---|
| `j` `k` · `enter` | move · open what a session is saying |
| `1`–`9` | pick one of the answers the agent offered |
| `y` `n` · `r` | allow · deny a permission · reply |
| `?` | **why is this here** — the decision log for the row under the cursor |
| `,` | **what is configured** — this machine, and every repository's `vibeplane.toml` read back |
| `g` | **every open issue and pull request**, across every registered project — `i` and `p` switch |
| `⌘K` | jump to any project, piece of work or session by name |
| `⌘N` | dispatch work: prompt, project, kind, and the project's own templates |
| `f` · `s` · `/` | raise the editor window · snooze · search |

The counts are the same door: `4 issues · 2 PRs` in the header, and the same figures on a project
heading, are buttons. A count you cannot open is a number telling you to go and look somewhere else.

`⌘N` opens with the cursor in the prompt and everything else already decided by the project, and it
tells you what it will do before it does it. `⌘K` matches by subsequence, so `crlb` finds
`core-lib`.

## 4. Answer what needs you

```sh
vibeplane inbox
```

The inbox is derived from state, never stored, so it is correct after a restart. Every item carries
at least one action, and every action is one the surface can actually perform — a permission on a
session Vibeplane only *watches* offers **Focus**, because the dialog belongs to Claude Code and the
honest thing to do is raise the window that has it.

## 5. Make “done” mean something

This is the part that earns the tool.

```sh
cd ~/code/saas
vibeplane trust .                  # once per repository
```

Trust is deliberate: a headless agent runs *that repository's* own hooks and MCP servers without
asking. The command lists them — and any unpinned MCP server, shell-granting skill or overbroad
`[policy]` rule — before it asks.

Write a definition of done:

```toml
# vibeplane.toml
[gates]
check = ["cargo clippy -- -D warnings", "cargo test"]
```

```sh
vibeplane check                    # what will this file actually do?
vibeplane work start "fix the flaky login test" --kind bug
```

`work start` makes an isolated checkout at `.claude/worktrees/<slug>` on its own branch, runs your
setup command, copies the files you named, and puts an agent in it. **When the agent says it is
finished, Vibeplane runs your commands.** Green means a person should look; red means the failures
go back to that same session, bounded, and then you are asked — with the same failing lines the
agent was handed.

An agent that claims success without earning it reaches `failed`, never `review`.

```sh
vibeplane work list
vibeplane work show <id>     # where it got to, what it cost, what the checks said
```

Next: [Verified done](/docs/verified-done/) in full.

## 6. See what GitHub is holding

```sh
vibeplane issues
```

```console
as hupe1980 · what needs you first

saas
  ◆ #212   Login fails on Safari 17                       bug · assigned to you
    https://github.com/acme/saas/issues/212
  ○ #209   Document the rate limits                       docs
    https://github.com/acme/saas/issues/209
```

Every open issue and pull request across every registered project, read through your own `gh`. On
the board, press `g` for the same two lists, or click the counts in a project heading. `◆` is what
is waiting on you — an issue assigned to you, a review requested from you, your own pull request
that is red, contested, or approved and unmerged. Those are inbox items too.

**Nothing here writes to GitHub.** Every action is a link.

## 7. Ask afterwards

```sh
vibeplane audit
```

```console
2026-09-13T18:04:11 daemon  gh:pr.create     https://github.com/acme/app/pull/142
                     ↳ gates passed; opened as a draft
2026-09-13T17:58:40 policy  agent:tool.use   Bash: pnpm test -- --run
                     ↳ Bash(pnpm test *)
```

Every verdict names the rule or the check behind it. “Auto-approved” is not an answer.

Every command takes `--json`, and every command starts the daemon if it is not already running.

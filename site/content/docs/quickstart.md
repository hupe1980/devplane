+++
title = "Quickstart"
description = "From nothing to a live board, a driven agent and a verified piece of work — in about five minutes."
weight = 2
[extra]
group = "start"
+++

## 1. See what is already running

```sh
devplane ls
```

```console
8 projects · 23 sessions · 5 working · 2 need you · 4 idle · $4.18
12 quiet (nothing heard for hours) — devplane ls --all

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
devplane ls --all           # including the quiet ones
devplane ls --project saas  # one project; matches any part of the name
devplane ls --needs-you     # only what is waiting on a human
```

## 2. Add live state

Discovery is free. Cost, context usage, blocking and the permission gate need Claude Code to talk to
Devplane:

```sh
devplane connect claude
```

This merges hook entries and OpenTelemetry variables into your **user** settings, keeping a backup,
and removes exactly those entries again on `disconnect`. It never touches your prompts: the flags
that would put prompt or response text into telemetry are never set.

```sh
devplane doctor    # is anything actually arriving?
```

See [Watching sessions](/docs/observe/) for what each channel provides.

## 3. Open the board

```sh
devplane open
```

One page served from the daemon on loopback — no CDN, no account, nothing fetched. It updates live,
groups by project, and puts what needs you at the top. It is a document rather than a canvas: every
state has a word as well as a colour, the sections are lists, and one polite live region says how
many things need you — so it reads aloud, and it survives being screenshotted in greyscale.

It opens on **Inbox**. The sidebar holds the rest, under the same four errands `devplane --help`
sorts its commands into — so a heading names the errand and each item under it names the thing.

| Errand | Surface | What |
|---|---|---|
| **What needs you** | **Inbox** | one list across every project, ordered by what is waiting |
| | **Decisions** | what was decided for you, on whose authority, for one session or Work |
| | **Finished work** | a finished Work's certificate, and one button that copies it |
| **See what is happening** | **Sessions** | every session, grouped by project, with cost and context |
| **Start and steer work** | **Start an agent** | a prompt, the projects to send it to, and what would happen in each before anything is written |
| | **Issues and PRs** | every open one, across every registered project |
| **Set up a project** | **Setup** | this machine, and every repository's `devplane.toml` read back |
| | **Library** | one row per prompt or skill, one column per repository, and which copy drifted |

Each item is a noun; the heading above it is the question. The page it opens carries that question as
its own heading. **A number beside an item says what is in it**, so an empty one reads as empty rather
than broken.

**Two surfaces are not in the sidebar, because neither is a place.** **Search** is the field at the top
of it: a command, a question or an error across every session, with its results as their own page. And
**Changes** — a Work's diff against its base branch — is reached from that Work, under *Finished work*.

**Every action is a button, and there are no keyboard shortcuts.** A session row opens its decision
log; the counts in a project heading open the lists behind them. A count you cannot open is a number
telling you to go and look somewhere else.

A permission also carries **the rule that stops it being asked again**, with the file to paste it
into — see [Permissions](/docs/permissions/#which-rule-to-write-next). Nothing here writes it.

## 4. Answer what needs you

```sh
devplane inbox
```

The inbox is derived from state, never stored, so it is correct after a restart. Every item carries
at least one action, and every action is one the surface can actually perform — a permission on a
session Devplane only *watches* offers **Focus**, because the dialog belongs to Claude Code and the
honest thing to do is raise the window that has it. **Allow** and **deny** appear only on a session
Devplane drives, which is the only kind it can answer for.

A permission answer must name a decision or one of the agent's own options. One that names neither is
refused as a bad request — it is never read as a deny.

Where there is no yes-or-no, the row says why: a watched session has no request behind it that any
surface could answer, so the only way in is the agent's own window.

## 5. Make “done” mean something

This is the part that earns the tool.

```sh
cd ~/code/saas
devplane trust .                  # once per repository
```

Trust is deliberate: a headless agent runs *that repository's* own hooks and MCP servers without
asking. The command lists them — and any unpinned MCP server, shell-granting skill or overbroad
`[policy]` rule — before it asks.

Write a definition of done:

```toml
# devplane.toml
[gates]
check = ["cargo clippy -- -D warnings", "cargo test"]
```

```sh
devplane check                    # what will this file actually do?
devplane work start "fix the flaky login test" --kind bug
```

`work start` makes an isolated checkout at `.claude/worktrees/<slug>` on its own branch, runs your
setup command, copies the files you named, and puts an agent in it. **When the agent says it is
finished, Devplane runs your commands.** Green means a person should look; red means the failures
go back to that same session, bounded, and then you are asked — with the same failing lines the
agent was handed.

An agent that claims success without earning it reaches `failed`, never `review`.

```sh
devplane work list
devplane work show <id>     # where it got to, what it cost, what the checks said
```

Next: [Verified done](/docs/verified-done/) in full.

## 6. See what GitHub is holding

```sh
devplane issues
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
the board, the **github** surface holds the same two lists, and the counts in a project heading open them. `◆` is what
is waiting on you — an issue assigned to you, a review requested from you, your own pull request
that is red, contested, or approved and unmerged. Those are inbox items too.

**Nothing here writes to GitHub.** Every action is a link.

## 7. Ask afterwards

```sh
devplane audit
```

```console
2026-09-13T18:04:11 daemon  gh:pr.create     https://github.com/acme/app/pull/142
                     ↳ gates passed; opened as a draft
2026-09-13T17:58:40 policy  agent:tool.use   Bash: rm -rf /tmp/build
                     ↳ refused by Bash(rm -rf *)
```

Every verdict names the rule or the check behind it. “Refused” is not an answer.

Every command takes `--json`, and every command starts the daemon if it is not already running.

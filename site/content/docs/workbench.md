+++
title = "The workbench"
description = "A tour of Devplane's interface: the regions, a change's six views, the review, the inbox, the palette and the keys that matter."
weight = 3
[extra]
group = "start"
+++

One page served by the host on loopback, laid out like an editor. Nothing is fetched from the
internet; no account.

```sh
devplane up           # in your browser
devplane up --window  # in its own window (the desktop app, or --features app)
```

`?` lists the keys bound where you are.

![A change open in the workbench: the changes by project in the sidebar, the change as a document with its lifecycle, six views and the evidence cards](../../workbench.png)

## The regions

| Region | What it holds |
|---|---|
| **Title bar** | where you are, the find box (`⌘K` / `Ctrl+K`), **New change**, sidebar and panel toggles |
| **Activity bar** | one icon per surface, each with a count (none at zero) |
| **Sidebar** | the current surface's list |
| **Editor** | tabs of what you opened |
| **Bottom panel** | **Activity** and **Sight** (`⌘J`) |
| **Status bar** | the host connection, sessions working, projects, what needs you, the theme |

| Surface | Key | What it lists |
|---|---|---|
| **Inbox** | `g i` | what needs a person, most urgent first, across every project |
| **Changes** | `g c` | every change, grouped by project, with its state |
| **Sessions** | `g b` | every agent session on the machine, watched or driven |
| **Specifications** | `g s` | each project's spec folders, task counts and open questions |
| **Ledger** | `g l` | every decision, and on whose authority it was made |
| **Forge** | `g f` | open GitHub issues and pull requests across your projects |
| **Setup** | `g ,` | this machine's channels, and each repository's `devplane.toml` read back |

A click opens a **preview tab** (italic) that the next click replaces; double-click or `Enter` keeps
it. Open tabs stay in this browser tab only. The address names what is showing, so a link or reload
reopens it.

## First run

With nothing set up, the workbench opens on Setup's checklist: sessions already running, update
checks, connect an agent, trust a repository, declare a check, the spec layout, and a first change.
Each step reads what is true on this machine, so closing the window loses nothing. Any step can be
skipped and brought back. A step that writes configuration shows the exact diff and writes only on
**Confirm**. Trusting a repository needs the desktop window; a browser tab gives you the
`devplane project trust` command instead.

A project with no gates says so wherever gates are shown — *no gates declared* — and offers
**Declare a check** instead of **Run gates**.

## A change

Open it from **Changes**, the palette, or a `devplane://change/<id>` link.

- **Header**: title, state, and the actions that apply; the rest under **More**. Below it, the gate
  standing as a sentence (*`check` exited zero against the working tree as it stands*).
- **Actions**: **Review**, **Run gates**, **Offer as pull request** (verified only); **More** holds
  **Finish**, **Archive**, **Editor** and **Terminal**.
- **A weakened check** leads the header — **1 check weakened · verified** — and hides the green
  sentence until read. While a weakened row is unseen, **Review** is the primary action and the offer
  is refused, with a button to each row. Files nobody opened are named beside the offer; they never
  refuse it.
- `Alt+1` … `Alt+6` (`⌥1` … `⌥6`) switch the six views:

| View | What it shows |
|---|---|
| **Overview** | cards for gates, tasks, ledger and agent; the facts; the history |
| **Tasks** | ticked and seen-by-a-passing-check counts, requirements and the tasks citing them, spec drift with **Tell the run** / **Accept what it saw** |
| **Review** | the diff, below |
| **Gates** | every gate run with its commands and exit codes, **Run base checks**, the certificate with **Copy as markdown** |
| **Ledger** | this change's decisions, filterable by authority |
| **Agent** | the latest run's conversation, a composer to prompt it, **Stop…**, the files it wrote and commands it ran |

What the states and standing mean: [Verified done](@/docs/verified-done.md).

## The review

![The review: a skipped test listed first under checks weakened or changed, the file tree beside the diff](../../review.png)

Every hunk of every file in one column against the merge base (uncommitted and untracked files
included), with the file tree beside it. Each file's header stays on screen while its hunks scroll:
path, lines, role, test coverage, what it was asked for, and facts with the command to check each.

- **Checks weakened or changed** come first ([the kinds](@/docs/verified-done.md#review-before-you-merge)).
  The line that made a row carries ⚠ in the gutter. A row is read when its hunks are marked seen.
- Then files in `[review] roles` order (path order without roles, and the view says so). **By
  intent** groups them by the run that wrote them; files nobody asked for get their own heading.
- Formatting-only hunks are collapsed and counted. The tree counts unread hunks per file.

| Key | Does |
|---|---|
| `j` / `k` | next / previous hunk, across files |
| `n` / `p` | next / previous file |
| `s` | mark the hunk seen |
| `f` | start a comment on the hunk |
| `⌘Enter` / `Ctrl+Enter` | send the pending review |
| `x` | expand this file's formatting-only hunks |
| `v` | unified or side by side |
| `1` / `2` | by risk / by intent |

- **Seen** is kept by the host, keyed by the hunk's content: a hunk the agent rewrites is unread
  again. The CLI, the inbox and the offer read the same marks. Scrolling marks nothing.
- **Comments**: press a line number (or `f`). Comments wait in a pending review and survive a reload.
  **Send N comments** sends them as one turn to the change's agent, each with its path, line and the
  tree it was written against, and says whether the agent reads it now, after its turn, or was resumed
  for it. After the agent's next turn each sent comment says whether its hunk changed.
- `devplane://review/<id>` opens this view; the CLI is `devplane change review <id> [--by intent]`.

## The inbox

![What needs you: the ranked list beside the item in full](../../inbox.png)

Each key is printed on the control it presses; one that does not apply says why. The same keys work
in the desktop app's answer window.

| Key | Does |
|---|---|
| `1`–`9` | choose that option (on a form, for the first unanswered question) |
| `t` · `⌘Enter` / `Ctrl+Enter` | type an answer · send it |
| `a` · `d` | allow once · deny, for a permission Devplane holds |
| `w` | a dialog the vendor owns: open its session, or copy its resume command |
| `s` | snooze until the session or change next reports activity (an hour for a pull request or issue) |
| `o` · `r` | open what the item is about · open the change's review |
| `z` | take back an answer before it is sent, or bring a snoozed item back |

- **An answer is held for 5 seconds** before it reaches the agent, with a countdown; `z` takes it back
  with nothing sent or recorded. Answering another item sends the held one at once. Quitting the host
  cancels it, and the question is waiting again at the next start. `devplane inbox answer` sends at
  once.
- Snoozed items leave the list and are counted at its foot, with **list them** and **bring back**.
- The inbox badge, the status bar's *need you* and the tray count are one number.

## Sessions

Every session, watched or driven, in one grid; state counts are filters, grouped by project or state.
What each state means: [Watching sessions](@/docs/observe.md#what-each-state-means).

![Every session in one grid, grouped by project](../../sessions.png)

## New change, the palette, the bottom panel

- **New change** (`Alt+N`): projects, optional spec, title (also the branch), first prompt, agent, and
  worktree or in place. It shows each project's preflight and every refusal before creating anything.
- **The palette** (`⌘K` / `Ctrl+K`) finds changes, surfaces, actions and projects; `>` lists only
  actions; other text also searches every session's tool calls, questions and errors. Over an inbox
  item or a change, its own actions come first.
- **Activity**: every session's events, newest first.
- **Sight**: what this machine can and cannot see — vendors watched end to end, wired but unproved,
  only seen when Devplane starts them, and projects whose configuration cannot be read. An empty
  Sessions list is not proof that nothing runs.

## Keys

| Key | Does |
|---|---|
| `⌘K` / `Ctrl+K` | command palette |
| `?` | the keys bound here |
| `Esc` | close the palette, help or new-change form |
| `Alt+N` | new change |
| `⌘B` / `Ctrl+B` | show or hide the sidebar |
| `⌘J` / `Ctrl+J` | show or hide the bottom panel |
| `Alt+W` · `Alt+]` · `Alt+[` | close tab · next tab · previous tab |
| `j` `k` / `↓` `↑` · `Enter` | move through a list · open |
| `g g` · `G` | first · last |
| `r` | review, on a change |

No key fires while you type in a field, except `Esc` and the `⌘`/`Ctrl` chords.

## The window

`devplane up --window` adds desktop notifications when an agent asks or gives up, a tray count, and a
global shortcut (`⌘⇧Space`; `[app] shortcut`) that shows the top item over whatever you are doing.
Install: [The window](@/docs/install.md#the-window).

A permission or question can be [answered from its notification](@/docs/permissions.md#answering-from-the-notification):

| | macOS (installed app) | Windows (installed app) | Linux |
|---|---|---|---|
| Allow / Deny on a permission | yes | yes | where the notification service reports actions |
| A question's options | up to 4 | up to 5 | up to 3 |
| A typed reply | yes | yes | no; a click opens the answer window |
| Withdrawn when answered elsewhere | yes | yes | yes |

A build run from source, a Windows build run from `target/`, or a desktop without a notification
service gets a plain notification with no actions. `devplane doctor` says which you have. `devplane
up` without a window notifies through the system notifier, plainly.

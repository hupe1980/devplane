+++
title = "The workbench"
description = "A tour of Devplane's interface: the regions, a change's six views, the review, the palette and the keys that matter."
weight = 3
[extra]
group = "start"
+++

One page served by the host on loopback, laid out like an editor. Nothing is fetched from the internet;
no account.

```sh
devplane up           # in your browser
devplane up --window  # in its own window (--features app)
```

`?` lists the keys bound here.

![A change open in the workbench: the changes by project in the sidebar, the change as a document with its lifecycle, six views and the evidence cards](../../workbench.png)

## The regions

| Region | What it holds |
|---|---|
| **Title bar** | where you are, the find box (`⌘K` / `Ctrl+K`), **New change**, sidebar and panel toggles |
| **Activity bar** | one icon per surface, each with a count |
| **Sidebar** | the current surface's list: changes by project, inbox items, sessions |
| **Editor** | tabs of what you opened |
| **Bottom panel** | **Activity** and **Sight** (`⌘J`) |
| **Status bar** | the host connection, sessions working, projects, what needs you, the theme |

![What needs you: the ranked list beside the item in full](../../inbox.png)

### The activity bar

| Surface | Go-to key | What it lists |
|---|---|---|
| **Inbox** | `g i` | what needs a person, most urgent first, across every project |
| **Changes** | `g c` | every change, grouped by project, with its state |
| **Sessions** | `g b` | every agent session on the machine, watched or driven |
| **Specifications** | `g s` | each project's spec folders, task counts and open questions |
| **Ledger** | `g l` | every decision, and on whose authority it was made |
| **Forge** | `g f` | open GitHub issues and pull requests across registered projects |
| **Setup** | `g ,` | this machine's channels, and each repository's `devplane.toml` read back |

A zero count shows nothing; a count is always a number, never colour alone.

### Tabs

- A click opens a **preview tab** (italic title) that the next click replaces; double-click or
  `Enter` pins it.
- Tabs are kept per browser tab and sent nowhere. The address names what is showing, so a link
  reopens it.

## A change

Open from **Changes**, the palette, or a `devplane://change/<id>` link (in the app).

- **Header**: one line — title, state, and the actions that can do something; the rest under
  **More**. The state opens where the change is in its life and its facts (project, branch, spec
  folder, size). Outside the review a second line carries the gate standing as a sentence
  (*`check` exited zero against the working tree as it stands*).
- **Weakened check**: the header reads **1 check weakened · verified** — the weakening first, verified
  as its qualifier — and the green standing sentence is hidden while a weakened row is unread. The
  Changes list, palette, Specifications page and inbox use the same words.
- **Actions**: **Review**, **Run gates**, **Offer as pull request** (verified only); **More** holds
  **Finish**, **Archive**, **Editor** and **Terminal**. While a weakened row is unseen, Review is
  primary and Offer is refused, naming each row with a button to open it. Beside the offer, the
  files nobody opened are named: *2 files not opened*, then their paths — never a share of the
  change, and never a reason to refuse.
- `Alt+1` … `Alt+6` (`⌥1` … `⌥6` on a Mac) switch the six views; the address names the view, and in
  the review the hunk under the cursor, so a reload returns to the same place.

| View | What it shows |
|---|---|
| **Overview** | cards for gates, tasks, ledger and agent; the facts; the history |
| **Tasks** | ticked and seen-by-a-passing-check counts, tasks ticked but sent to nobody, requirements and the tasks citing them, spec drift with **Tell the run** / **Accept what it saw** |
| **Review** | the diff, below |
| **Gates** | every gate run with its commands and exit codes, **Run base checks**, and the certificate with **Copy as markdown** |
| **Ledger** | this change's decisions, filterable by authority |
| **Agent** | the latest run's conversation, a composer to prompt it, **Stop…**, and the files it wrote and commands it ran |

Only *verified* is green. See [Verified done](@/docs/verified-done.md).

## The review

![The review: a skipped test listed first under checks weakened or changed, the file tree beside the diff](../../review.png)

Every hunk of every file in one column, against the merge base (uncommitted and untracked files
included), with the file tree beside it. Each file's facts are one strip that stays at the top while
its hunks scroll: path, lines, role, coverage, what it was asked for, and **facts** with the commands
to check each one yourself.

- **Checks weakened or changed** first ([the kinds](@/docs/verified-done.md#review-before-you-merge)).
  The line that made a row weakened carries ⚠ in the gutter and a note naming the gate that ran over
  it; the Gates view links each row back to it. A row is read when its hunks are marked seen; an
  offer waits for every one.
- Then files in `[review] roles` order, each with whether a declared test covers it; without roles,
  path order, and the view says so.
- **By risk** / **By intent**: intent groups by the run that wrote each file and its tasks; unrequested
  files get their own heading.
- Formatting-only hunks are collapsed and counted.
- The tree says how many hunks of each file are unread, or *all seen*; the Review tab says how many
  in the change.

| Key | Does |
|---|---|
| `j` / `k` | next / previous hunk, across files |
| `n` / `p` | next / previous file |
| `s` | mark the hunk seen |
| `f` | request a fix: start a comment on the hunk |
| `⌘Enter` | send the pending review (in a comment: add it) |
| `x` | expand this file's collapsed formatting-only hunks |
| `v` | unified or side by side |
| `1` / `2` | by risk / by intent |

- **Seen** is kept by Devplane, keyed by the hunk's content: a hunk the agent rewrites is unread
  again, and the file says it changed since you marked it. The CLI, the inbox and the offer read the
  same marks. Scrolling marks nothing. It decides nothing either.
- **Comments**: press a line number (or `f`) to comment on that line. Comments wait in a pending
  review, counted in the review's bar, and survive a reload. **Send N comments** sends them all as
  one turn to the change's agent — each naming its path and line, the tree and hunk it was written
  against, the line, and your words — and says whether the agent is reading it now, will read it
  when its turn ends, or was resumed or started for it. A comment whose line has since changed says
  so; after the agent's next turn each sent comment says whether its hunk changed.
- `#change/<id>/review` or `devplane://review/<id>` opens this view. CLI: `devplane change review <id>
  [--by intent]`.

## Sessions

Every session, watched or driven, in one grid. State counts are filters; group by project or state.

![Every session in one grid, grouped by project](../../sessions.png)

## The first run

A window with nothing set up opens on Setup's first-run checklist: the sessions already running, update
checks, connect an agent, trust a repository, declare a check, the spec dialect, and a first change.
Each step reads what is actually true on this machine — hooks present, a repository trusted, a `check`
declared, a change verified — so closing the window loses nothing. Every step can be skipped and
brought back; a step that writes configuration shows the exact diff first and writes only on
**Confirm**. The checklist ends when the first change reads *verified*, and stays reachable from Setup.

A project that declares no gates says so wherever its gates are shown — *no gates declared — changes
here cannot become verified* — and offers **Declare a check** instead of *Run gates*. New change says
the same before anything is created.

## Starting a change

**New change** (title bar, `Alt+N`): projects, optional spec, title (also the branch), first prompt,
agent, and worktree or in place. Before creating anything it shows each project's preflight and every
refusal, as `devplane change start` does.

## The palette

`⌘K` / `Ctrl+K` finds changes, surfaces, actions and projects. `>` lists only actions. Other text
also searches every session's tool calls, questions and errors, like `devplane search`.

## The bottom panel

**Activity**: every session, newest event first: time, project, agent (driven or not), state, action,
context used.

**Sight**: what this machine can and cannot see. An empty Sessions list is not proof nothing runs.

- **Watched end to end**: vendors proved followable.
- **Read, not proved against a live session**: wired, unverified.
- **Seen only when Devplane starts them**: drivable, not watchable.
- **Projects whose configuration cannot be read**, if any.

See [Watching sessions](@/docs/observe.md).

## Keys

| Key | Does |
|---|---|
| `⌘K` / `Ctrl+K` | command palette |
| `?` | the keys bound here |
| `Esc` | close the palette or help, or go back to the list |
| `Alt+N` | new change |
| `⌘B` / `Ctrl+B` | show or hide the sidebar |
| `⌘J` / `Ctrl+J` | show or hide the bottom panel |
| `Alt+W` · `Alt+]` · `Alt+[` | close tab · next tab · previous tab |
| `j` `k` / `↓` `↑` | move through a list |
| `Enter` | open |
| `g g` · `G` | first · last |
| `r` | review, on a change |

No key fires while you type in a field, except `Esc` and the `⌘`/`Ctrl` chords.

### On an inbox item

The same keys work in the answer window. Each is printed on the control it presses; one that does
not apply to the item says why instead of doing nothing.

| Key | Does |
|---|---|
| `1`–`9` | choose that option (on a form, for the first question not yet answered) |
| `t` · `⌘↵` / `Ctrl+Enter` | type an answer in your own words · send it |
| `a` · `d` | allow, once · deny — a permission Devplane holds |
| `w` | a dialog the vendor owns: open its session (VS Code) or copy its resume command |
| `s` | snooze for an hour — never a question or a permission |
| `o` · `r` | open what the item is about · open the change's review |
| `z` | take back the answer before it is sent, or put a snoozed item back |

An answer is held for 5 seconds before it reaches the agent: the inbox says what was answered and
counts down, and `z` takes it back with nothing sent and nothing recorded. Answering another item
sends the held one at once. Closing the page does not stop it being sent; quitting does — the quit
question says so — and the question is waiting again at the next start. `devplane inbox answer`
sends at once.

## The window

`devplane up --window` adds a notification when an agent asks or gives up, a menu-bar count, and a
global shortcut (`⌘⇧Space`; `[app] shortcut`) that shows the top item over whatever you are doing;
`Esc` returns focus. See [Install](@/docs/install.md#the-window).

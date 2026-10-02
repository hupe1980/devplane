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

- **Header**: title, state, project, branch, spec folder, and the gate standing as a sentence
  (*`check` exited zero against the working tree as it stands*).
- **Weakened check**: the header reads **1 check weakened · verified** — the weakening first, verified
  as its qualifier — and the green standing sentence is hidden while a weakened row is unread. The
  Changes list, palette, Specifications page and inbox use the same words.
- **Toolbar**: **Review**, **Run gates**, **Offer as pull request** (verified only), **Finish**,
  **Archive**, **Editor** / **Terminal**. While a weakened row is unseen, Review is primary and Offer
  is refused, naming each row with a button to open it.

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

A file tree beside a diff against the merge base, uncommitted and untracked files included.

- **Checks weakened or changed** first ([the kinds](@/docs/verified-done.md#review-before-you-merge)),
  each with what it matched and whether you read it. **I have read this** (or marking a hunk in that
  file seen) records it; an offer waits for every one.
- Then files in `[review] roles` order, each with whether a declared test covers it; without roles,
  path order, and the view says so.
- **By risk** / **By intent**: intent groups by the run that wrote each file and its tasks; unrequested
  files get their own heading.
- Formatting-only hunks are collapsed and counted.

| Key | Does |
|---|---|
| `j` / `k` | next / previous hunk |
| `n` / `p` | next / previous file |
| `s` | mark the hunk seen |
| `f` | request a fix: a prompt to the change's latest run, quoting the hunk |
| `x` | expand this file's collapsed formatting-only hunks |
| `v` | unified or side by side |
| `1` / `2` | by risk / by intent |

- A hunk's *seen* mark lives in this browser, keyed by content (a rewritten hunk is unseen again), and
  decides nothing. Only a weakened-check row's mark is kept by the host.
- `#change/<id>/review` or `devplane://review/<id>` opens this view. CLI: `devplane change review <id>
  [--by intent]`.

## Sessions

Every session, watched or driven, in one grid. State counts are filters; group by project or state.

![Every session in one grid, grouped by project](../../sessions.png)

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

No key fires while you type in a field, except `Esc`.

## The window

`devplane up --window` adds a notification when an agent asks or gives up, a menu-bar count, and a
global shortcut (`⌘⇧Space`; `[app] shortcut`) that shows the top item over whatever you are doing;
`Esc` returns focus. See [Install](@/docs/install.md#the-window).

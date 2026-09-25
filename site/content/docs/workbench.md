+++
title = "The workbench"
description = "A tour of Devplane's interface: the regions, a change's six views, the review, the palette and the keys that matter."
weight = 3
[extra]
group = "start"
+++

The workbench is one page served by the host on loopback, laid out like an editor: a title bar, an
activity bar, a sidebar list, tabs, a bottom panel and a status bar. Nothing is fetched from the
internet and nothing needs an account.

```sh
devplane open     # in your browser
devplane app      # in its own window (--features app)
```

Press `?` anywhere to list the keys bound on the current surface.

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
| **Forge** | | open GitHub issues and pull requests across registered projects |
| **Reports** | `g r` | findings one project filed about another |
| **Setup** | | this machine's channels, and each repository's `devplane.toml` read back |

A zero count shows nothing; a count is always a number, never a colour alone.

### Tabs

A click on a list row opens a **preview tab** (title in italics) that the next click replaces.
Double-click or `Enter` pins it. Tabs are kept per browser tab and sent nowhere. The address always
names what is showing, so a link opens the same view.

## A change

Open a change from **Changes**, the palette, or a `devplane://change/<id>` link (in the app). The
header shows the title, state, project, branch, spec folder and the gate standing in one sentence,
for example *every declared gate exited zero against the tree as it stands*. The toolbar holds
**Review**, **Run gates**, **Offer as pull request** (only when verified), **Finish**, **Archive**, and
**Editor** / **Terminal** to open the worktree.

| View | What it shows |
|---|---|
| **Overview** | cards for gates, tasks, ledger and agent; the facts; the history; reports it filed |
| **Tasks** | ticked and verified counts, tasks ticked but sent to nobody, requirements and the tasks citing them, spec drift with **Tell the run** / **Accept what it saw** |
| **Review** | the diff, below |
| **Gates** | every gate run with its commands and exit codes, and the certificate with **Copy as markdown** |
| **Ledger** | this change's decisions, filterable by authority |
| **Agent** | the latest run's conversation, a composer to prompt it, **Stop…**, and the files it wrote and commands it ran |

Only *verified* is green. See [Verified done](@/docs/verified-done.md).

## The review

![The review: a skipped test listed first under checks weakened or changed, the file tree beside the diff](../../review.png)

A file tree beside a diff against the merge base, uncommitted and untracked files included.

- **Checks weakened or changed** comes first when there is anything under it: an added skip marker, a
  deleted test file, an edit to the gates' definition or to CI configuration. Each row says what it
  matched.
- Then the files in the order `[review] roles` declares (shared, logic, security, integration,
  wiring, tests), each with whether a declared test covers it. With no roles declared, files are in
  path order and the view says so.
- **By risk** / **By intent** switches to grouping by the run that wrote each file and the tasks it
  was sent. Files no run was asked for sit under their own heading.
- Formatting-only hunks are collapsed and counted.

| Key | Does |
|---|---|
| `j` / `k` | next / previous hunk |
| `n` / `p` | next / previous file |
| `s` | mark the hunk seen |
| `a` | accept the hunk |
| `f` | request a fix: a prompt to the change's latest run, quoting the hunk |
| `x` | expand this file's collapsed formatting-only hunks |
| `v` | unified or side by side |
| `1` / `2` | by risk / by intent |

*Seen* and *accepted* are marks in this browser only; they are sent nowhere and decide nothing. The
same pane is a change's Review view and `#review/<id>`. On the command line: `devplane change review
<id> [--by intent]`.

## Sessions

Every session on the machine, watched or driven, in one grid. The state counts above it are its
filters; group by project or by state; select a row for its detail.

![Every session in one grid, grouped by project](../../sessions.png)

## Starting a change

**New change** (title bar, or `Alt+N`) opens a form: one or more projects, an optional specification,
a title (also the branch name), the first prompt, the agent, and whether to use an isolated worktree.
Before anything is created it shows what would happen in each project, including every refusal: an
untrusted repository, a dirty checkout, a `devplane.toml` that will not load. It is the same preflight
`devplane change start` runs.

## The palette

`⌘K` (or `Ctrl+K`) finds changes, surfaces (*Go to …*), actions and projects by name. Start with `>`
to list only actions. Any other text can also be searched across every session's tool calls,
questions and errors, as `devplane search` does.

## The bottom panel

**Activity** is every session, newest event first, one line each: time, project, agent (and whether
Devplane drives it), state, what it is doing, context used. Click a line to open the session.

**Sight** is the limit of what this machine can see:

- **Watched end to end**: vendors whose sessions Devplane has proved it can follow.
- **Read, not proved against a live session**: channels that are wired but unverified.
- **Seen only when Devplane starts them**: agents it can drive but not watch.
- **Projects whose configuration cannot be read**, when there are any.

An empty Sessions list does not mean nothing is running; Sight says what is not watched. See
[Watching sessions](@/docs/observe.md).

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

`devplane app` hosts in-process and adds a notification when an agent asks or gives up, a count in
the menu bar, and one global shortcut (`⌘⇧Space` by default; `[app] shortcut` in
`~/.devplane/app.toml`) that shows the topmost thing that needs you over whatever you are doing.
Answer it, press `Esc`, and focus goes back. Closing the window keeps hosting; quitting from the tray
says what it stops first. Nothing starts at login. See [Install](@/docs/install.md#the-window).

+++
title = "Quickstart"
description = "From install to a verified change in about five minutes: see your sessions, open the workbench, start a change, and read what was decided."
weight = 2
[extra]
group = "start"
+++

You need `devplane` [installed](@/docs/install.md) and at least one coding agent. Claude Code is the
one Devplane watches best.

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

No setup and no host: sessions come from Claude Code's roster and `ls` reads the store directly.

```sh
devplane ls --needs-you     # only what waits on a person
devplane ls --project saas  # one project; any part of the name matches
```

## 2. Connect your agent

Cost, context, questions and the permission gate need the agent's hooks:

```sh
devplane connect claude     # also: codex, copilot
devplane doctor             # is anything arriving?
```

`connect` shows the diff to your **user** settings, keeps a backup, and writes when you confirm
(`--yes` skips the question). The hooks decide and write to the store in their own process, so
nothing needs to be running. `devplane disconnect claude` removes exactly what was added. See
[Watching sessions](@/docs/observe.md).

## 3. Open the workbench

```sh
devplane open
```

With no host running, this terminal becomes the host until `ctrl-c`. `devplane serve` is the host
without the browser; `devplane app` is the host in its own window.

It opens on the **Inbox**: what needs you across every project, most urgent first. `⌘K` finds
anything; `?` lists the keys. [The workbench](@/docs/workbench.md) is the full tour.

## 4. Answer what needs you

```sh
devplane inbox
devplane answer <ask> --allow           # a permission
devplane answer <ask> --option "Yes"    # a question, with one of the agent's options
```

The id is the one `inbox` prints. It outlives the process that asked, so an answer given tomorrow
still reaches the agent. For a session you started yourself, the inbox offers **Focus** (raise its
window) unless the project sets [`[questions] hold`](@/docs/configuration.md#questions).

## 5. Make “done” mean something

In a repository of yours:

```sh
devplane trust .
```

A headless agent runs the repository's own hooks and MCP servers without asking, so `trust` lists
them and asks first. Then commit a definition of done:

```toml
# devplane.toml
[gates]
check = ["cargo clippy -- -D warnings", "cargo test"]
```

```sh
devplane check                                   # what this file will do
devplane change start "fix the flaky login test"
devplane watch <run>                             # follow the agent, like tail -f
devplane change show <id>                        # state, cost, what the checks said
```

`change start` needs the host. It makes an isolated worktree on its own branch, starts an agent in it,
and **runs your `check` when the agent says it is finished**. Red goes back to the same session a
bounded number of times, then to you. Green against the tree as it stands is **verified**; touch a
file and it is stale.

The same prompt can go to several repositories; every refusal is reported before anything starts:

```sh
devplane change start "bump the MSRV to 1.90" --project core-lib --project saas
```

See [Verified done](@/docs/verified-done.md).

## 6. Review and offer it

```sh
devplane change review <id>     # weakened checks first, then files by risk
devplane change offer <id>      # push and open a draft PR, or print the two commands
devplane change export <id>     # the certificate, for the PR body
```

## 7. Ask afterwards

```sh
devplane audit                # everything, newest first
devplane audit --without-me   # only what a rule, a clock or nobody decided instead of you
```

```console
2026-09-13T18:04:11 devplane  gate:run          saas · fix the flaky login test
                      ↳ check passed
2026-09-13T17:58:40 rule      agent:tool.use    Bash: rm -rf /tmp/build
                      ↳ refused by Bash(rm -rf *)
```

Every decision names its authority (`person`, `rule`, `timer`, `nobody` or `devplane`) and the rule
or check behind it. See [The decision log](@/docs/decisions.md).

Every command takes `--json`. Reading works with nothing running; a command that starts, steers or
answers a driven agent needs the host and says so.

+++
title = "Quickstart"
description = "From nothing to a verified change, step by step: install, start the host, make a practice repository with a failing test, let an agent fix it, and read how it was verified."
weight = 2
[extra]
group = "start"
+++

This walks one change from nothing to **verified** in a practice repository, so nothing of yours is
at stake. About ten minutes once your agent is signed in.

## What you need

- **git** and a POSIX shell (macOS or Linux; see the [platform table](@/docs/install.md#platforms)).
- **Node 18 or later, with `npx`.** The built-in agents are npm packages Devplane starts with `npx`,
  so the first start needs network access to npm.
- **An agent, signed in.** This page uses Claude Code: install it and run `claude` once to log in.
  Any [other agent](@/docs/agents.md) works with `--agent`.
- **Two terminals.** The first runs the host; the second is where you work.
- A GitHub sign-in (`devplane login github`), only if you want the Forge view or pull requests. Not
  needed here.

## 1. Install and check

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
devplane --version
devplane doctor
```

Other ways in (npm, cargo, Windows) are on [Install](@/docs/install.md). `doctor` will say *host: not
running* and *hooks: not installed*. Both are expected at this point.

## 2. Start the host (terminal 1)

```sh
devplane open
```

With no host running, this terminal **becomes** the host until `ctrl-c`, and your browser opens the
workbench on the **Inbox**. Leave it running. Everything that starts or steers an agent needs it.

If it says the port is taken, see [Troubleshooting](@/docs/troubleshooting.md#the-port-is-taken).

## 3. Make a practice repository (terminal 2)

A two-line program and a test that fails:

```sh
mkdir devplane-demo && cd devplane-demo
git init -b main

cat > greet.sh <<'EOF'
#!/bin/sh
echo "Hello"
EOF

cat > test.sh <<'EOF'
#!/bin/sh
[ "$(sh greet.sh)" = "Hello, world" ] || { echo "no name: got '$(sh greet.sh)'"; exit 1; }
[ "$(sh greet.sh Ada)" = "Hello, Ada" ] || { echo "Ada: got '$(sh greet.sh Ada)'"; exit 1; }
echo ok
EOF
```

## 4. Say what “done” means

```toml
# devplane.toml
[gates]
check = ["sh test.sh"]
```

Save that as `devplane.toml`, then commit everything: a change starts from a clean checkout, and the
gate is read from what you committed, not from the agent's branch.

```sh
git add -A && git commit -m "a greeting and its test"
devplane check      # what the file will do
devplane gate       # run the check here, now
```

```console
$ devplane gate
failed    sh test.sh              exit 1

check failed: sh test.sh
```

Red, as it should be: that is the work.

## 5. Trust the repository

```sh
devplane trust .
```

A headless agent runs a repository's own hooks and MCP servers without asking, so Devplane starts no
agent in a repository you have not trusted. `trust` lists what is there (here: nothing), asks, and
needs the host from step 2.

## 6. Start a change

```sh
devplane change start "make greet.sh print 'Hello, <name>' for its first argument, and 'Hello, world' without one, so sh test.sh passes"
```

```console
started make greet.sh print 'Hello, <name>' …
  branch    change/make-greet-sh-print-hello-3f9a1c
  worktree  …/devplane-demo/.claude/worktrees/make-greet-sh-print-hello-3f9a1c
  devplane change verify c-3f9a…
```

Devplane made an isolated worktree on its own branch, started Claude Code in it with your sentence
as the prompt, and will **run your `check` when the agent says it is finished**. Your own checkout is
untouched. The last line carries the change's id; any unambiguous prefix of it works below, and
`devplane change list` shows it again.

## 7. Supervise

The agent may ask before it edits a file or runs a command. Those questions land in the Inbox in the
browser, and in the terminal:

```sh
devplane inbox
devplane answer <ask> --allow          # the id is the one `inbox` prints
devplane change show <id>              # state, runs, what the checks said
devplane watch <run>                   # follow the agent, like tail -f (the run id is in `change show`)
```

If the check is red when the agent stops, the failing lines go back to the same agent, a bounded
number of times, then to you.

## 8. Verified

```console
$ devplane change show c-3f9a
make greet.sh print 'Hello, <name>' …  c-3f9a…
  state      ✓ verified
  gates      `check` exited zero against the working tree as it stands
  branch     change/make-greet-sh-print-hello-3f9a1c
```

**Verified** means your `check` passed, and the tree it ran against (uncommitted files included) is
the tree now. The agent saying it is done is not enough. Edit any file in the worktree and it reads
*stale* until the check runs again (`devplane change verify <id>`). The state line says
**verified · 1 check weakened** if the agent's diff loosened a test instead of fixing the code. See
[Verified done](@/docs/verified-done.md).

## 9. Review, offer, finish

```sh
devplane change review <id>     # weakened checks first, then every file
devplane change offer <id>      # opens a pull request, or prints the push and the address
devplane change export <id>     # the certificate: what ran, against which commit
devplane change finish <id>     # accept it; removes nothing
devplane change archive <id>    # remove the worktree; keep the branch and the record
```

The practice repository has no remote and no `[github] pull_request = true`, so `offer` pushes
nothing and prints the `git push` line and the address that opens the pull request. In the workbench the same change is under
**Changes**, with its Review, Gates and Ledger.

## 10. Ask afterwards

```sh
devplane audit                # everything, newest first
devplane audit --without-me   # only what a rule, a clock or nobody decided instead of you
```

```console
2026-09-26T18:04:11 devplane gate:run          sh test.sh
                      ↳ check passed
2026-09-26T18:02:40 person   agent:tool.use    Edit: greet.sh
```

Every decision names its authority (`person`, `rule`, `timer`, `nobody` or `devplane`). See
[The decision log](@/docs/decisions.md).

## Next

- **Your own repositories.** The same steps: a `devplane.toml` with your real `check`, committed,
  then `trust` and `change start`. Add prohibitions with [`[policy]`](@/docs/permissions.md).
- **Sessions you start yourself.** `devplane connect claude` installs hooks into your Claude Code
  user settings (it shows the diff and asks), so `devplane ls` and the Inbox show your terminal and
  editor sessions too, with cost and context. See [Watching sessions](@/docs/observe.md).
- **A spec.** `devplane change start --spec specs/001-… --task REQ-3 "…"`. See
  [Working to a specification](@/docs/specs.md).
- **The window, surface by surface.** [The workbench tour](@/docs/workbench.md) — the Inbox,
  Changes, Sessions, Specifications, the Ledger, Forge, Reports and Setup.
- **Something wrong?** [Troubleshooting](@/docs/troubleshooting.md).

Reading commands (`ls`, `inbox`, `audit`, `change show`, …) work with the host stopped; a command
that starts, steers or answers a driven agent needs it and says so.

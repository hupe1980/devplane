+++
title = "Quickstart"
description = "From nothing to a verified change, step by step: install, start the host, make a practice repository with a failing test, let an agent fix it, and read how it was verified."
weight = 2
[extra]
group = "start"
+++

One change from nothing to **verified**, in a practice repository. About ten minutes.

## What you need

- **git** and a POSIX shell (macOS or Linux; see the [platform table](@/docs/install.md#platforms)).
- **Node 18+ with `npx`**, and npm access on first start.
- **An agent, signed in.** Here Claude Code (run `claude` once); [others](@/docs/agents.md) via `--agent`.
- **Two terminals**: one for the host, one to work in.

## 1. Install and check

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
devplane --version
devplane doctor
```

Other methods: [Install](@/docs/install.md). `doctor` says *host: not running* and *hooks: not
installed*; both are expected.

## 2. Start the host (terminal 1)

```sh
devplane up
```

This terminal **becomes** the host until `ctrl-c`, and the browser opens the workbench. Leave it
running. Port taken? [Troubleshooting](@/docs/troubleshooting.md#the-port-is-taken).

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

Commit everything: a change needs a clean checkout, and the gate is read from your commit.

```sh
git add -A && git commit -m "a greeting and its test"
devplane config check  # what the file will do
devplane gate          # run the check here, now
```

```console
$ devplane gate
failed    sh test.sh              exit 1

check failed: sh test.sh
```

Red: that is the work.

## 5. Trust the repository

```sh
devplane project trust .
```

It lists the repository's hooks and MCP servers (here none), then asks.

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

A worktree on its own branch, Claude Code in it with your sentence, and **your `check` when it
finishes**. Your checkout is untouched. Any unique prefix of the change id works; `devplane change
list` shows it again.

## 7. Supervise

The agent's questions land in the browser's Inbox and here:

```sh
devplane inbox
devplane inbox answer <ask> --allow  # the id is the one `inbox` prints
devplane change show <id>            # state, runs, what the checks said
devplane run watch <run>             # follow the agent, like tail -f (the run id is in `change show`)
```

A red check goes back to the agent a bounded number of times, then to you.

## 8. Verified

```console
$ devplane change show c-3f9a
make greet.sh print 'Hello, <name>' …  c-3f9a…
  state      ✓ verified
  gates      `check` exited zero against the working tree as it stands
  branch     change/make-greet-sh-print-hello-3f9a1c
```

**Verified**: your `check` passed and the tree it ran against (uncommitted files included) is the tree
now. Edit a file and it is *stale* until `devplane change verify <id>`. If the diff loosened a test
instead of fixing the code, it reads **1 check weakened · verified**. See
[Verified done](@/docs/verified-done.md).

## 9. Review, offer, finish

```sh
devplane change review <id>   # weakened checks first, then every file
devplane change offer <id>    # commits the verified tree, then opens a pull request or prints how
devplane change export <id>   # the certificate: what ran, against which commit
devplane change finish <id>   # accept it; removes nothing
devplane change archive <id>  # remove the worktree; keep the branch and the record
```

With no remote and no `[github] pull_request = true`, `offer` commits and prints the `git push` line.
The workbench shows the change under **Changes**.

## 10. Ask afterwards

```sh
devplane audit               # everything, newest first
devplane audit --without-me  # only what a rule, a clock or nobody decided instead of you
```

```console
2026-09-26T18:04:11 devplane gate:run          sh test.sh
                      ↳ check passed
2026-09-26T18:02:40 person   agent:tool.use    Edit: greet.sh
```

See [The decision log](@/docs/decisions.md).

## 11. Stop

`devplane down` in any terminal, or **Quit** (⌘Q or the tray) in the desktop window. Either says what it stops first; a
run it stops resumes with `devplane change resume`.

## Next

- **Your repositories**: the same steps with your real `check`; prohibitions in [`[policy]`](@/docs/permissions.md).
- **Your own sessions**: `devplane agent connect claude` ([Watching sessions](@/docs/observe.md)).
- **A spec**: [Working to a specification](@/docs/specs.md).
- **The interface**: [the workbench tour](@/docs/workbench.md).
- **Something wrong**: [Troubleshooting](@/docs/troubleshooting.md).

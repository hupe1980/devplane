+++
title = "Working to a specification"
description = "Name the spec folder a change answers, send an agent the tasks you choose, and make your own gates the verdict inside Spec Kit's workflow."
weight = 13
[extra]
group = "guide"
+++

Write the specification with Spec Kit, OpenSpec, Kiro, or a folder of Markdown. Devplane runs the
agent, watches it and decides whether the result is done, and adopts no spec format.

```sh
devplane change start "password reset" --spec specs/001-password-reset
devplane change show <id>
```

## What Devplane reads

- the spec is **Markdown**, committed to the repository;
- the unit is a **folder**;
- the outline is the **headings**;
- progress is `- [ ]` / `- [x]` in the folder's `tasks.md` only (a `checklists/` folder is not
  progress).

No section name is recognised. Layouts found automatically:

| Tool | Detected by | Spec folders |
|---|---|---|
| Spec Kit | `.specify/` | `specs/NNN-name/` |
| OpenSpec | `openspec/config.yaml` | `openspec/changes/<id>/` (not `archive/`) |
| Kiro | `.kiro/` | `.kiro/specs/<feature>/` |

Any other layout is one line of configuration:

```toml
# devplane.toml
[spec]
plans          = "docs/plans"  # where this repository keeps its specs
open_questions = ["NEEDS CLARIFICATION", "TBD"]
```

`open_questions` are the words that mark an unanswered question, matched per line,
case-insensitively; there is no default. The **Specifications** view in the
[workbench](@/docs/workbench.md) lists each project's spec folders, task counts, and the lines still
carrying one of those words.

## Naming the spec a change answers

```sh
devplane change start "password reset" --spec specs/001-password-reset
devplane change adopt feat/password-reset --spec specs/001-password-reset
```

The path is relative to the repository and must exist in the base the worktree branches from; an
uncommitted spec folder is not in the agent's checkout. Every gate run stamps a fingerprint of the
folder's Markdown onto the change, so the [certificate](@/docs/verified-done.md#the-done-certificate)
says *checked against `specs/001-password-reset` at `3f9a…`*.

```console
$ devplane change show c-3f9a
  spec       specs/001-password-reset at 3f9a1c40b7e2d518 over 4 documents
  tasks      11 ticked · 9 verified
```

## Sending chosen tasks

`--task` picks which lines of the task list go to the agent. Repeat it, or mix the two forms:

```sh
# every task line that cites the requirement token REQ-3
devplane change start "reset: token expiry" \
  --spec specs/001-password-reset --task REQ-3

# one exact line, file:line relative to the spec folder
devplane change start "reset: email copy" \
  --spec specs/001-password-reset --task tasks.md:14
```

The selected lines travel in the prompt and on the run's record. A selector that matches nothing
refuses before anything is created.

**Requirement tokens** are a prefix followed by digits: by default `FR-`, `NFR-`, `SC-`, `REQ-`,
`US-` and `AC-`; `[spec] tokens` replaces the list. A token in a heading and in a task line is an
edge; a token on only one side is reported as a gap. Spec Kit's `[US1]` marker and Kiro's
`_Requirements: 1.1, 2.3_` line are read as citations.

## Ticked is not verified

A ticked box is the agent's claim about its own work, so a change that names a spec reports two
counts and never combines them:

```console
  tasks      11 ticked · 9 verified
             ticked, sent to nobody: tasks.md:31, tasks.md:32
  REQ-3      3 tasks · 2 ticked · 0 verified
```

- **ticked** — `- [x]` in the task file, read in the change's worktree.
- **verified** — ticked, sent to a run of this change with `--task`, and that run finished before the
  change's latest passing `check` gate ran, so the gate saw its work.
- A box ticked by hand is listed as *ticked, sent to nobody*. With no gates declared the line reads
  *11 ticked · no gates declared*, never *0 verified*. No percentage is computed anywhere.

## When the spec moves under a run

If the spec folder is edited while an agent works from it, the inbox says so when the run ends
(*the specification changed 18m into run r-9f2 and the run never saw it*), with two answers:

```sh
devplane change drift <id> --run r-9f2 --tell     # tell it what changed
devplane change drift <id> --run r-9f2 --accept   # work to what the run saw
```

`--tell` prompts the run with the changed files named; `--accept` makes the change work to what the
run saw. Either is recorded as your decision.

## A spec tool's CLI is already a gate

If your spec tool validates from the command line, put it beside your tests:

```toml
# devplane.toml
[gates]
check = ["openspec validate --strict", "pnpm test -- --run"]
```

That checks the specs against each other; your tests compare spec and code.

## The gate inside Spec Kit's workflow

Spec Kit's commands look in `.specify/extensions.yml` for hooks, and run and wait for a hook marked
`optional: false`. Devplane's gate goes there:

```sh
devplane speckit install              # writes .specify/extensions.yml
devplane speckit install --dry-run    # print it, write nothing
devplane gate run                     # what the hook runs
```

If `.specify/extensions.yml` already exists, `speckit install` prints the entry instead.
The hook goes on `after_implement` unless you pass `--event`. `speckit install` refuses when the
repository declares no gate (declare `[gates] check` first, or pass `--anyway`), and when nothing
defines the `devplane-gate` skill the hook resolves to: install the
[Claude Code plugin](@/docs/install.md#inside-claude-code-as-a-plugin), or put a skill of that name in
the project's or your own `.claude/skills/`.

`devplane gate run` runs `[gates] check` and exits 0 only when it passed; *no gates declared* and an
unreadable `devplane.toml` exit non-zero. It records nothing, and the hook never waits on a person.
States and flags: [`devplane gate run`](@/docs/cli.md#devplane-gate-run).

## Next

- [Verified done](@/docs/verified-done.md) — what the gate verdict means, and the certificate.
- [Configuration: `[spec]`](@/docs/configuration.md#spec) — every key.

+++
title = "Working to a specification"
description = "Name the spec folder a change answers, send an agent the tasks you choose, and make your own gates the verdict inside Spec Kit's workflow."
weight = 13
[extra]
group = "guide"
+++

Write specs with Spec Kit, OpenSpec, Kiro or plain Markdown; Devplane adopts no spec format.

```sh
devplane change start "password reset" --spec specs/001-password-reset
devplane change show <id>
```

## What Devplane reads

Committed **Markdown**, one **folder** per spec, **headings** as outline, and progress as `- [ ]` /
`- [x]` in `tasks.md` only (not `checklists/`). No section names. Detected layouts:

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

`open_questions` has no default. The **Specifications** view lists each project's spec folders, task
counts, and lines carrying those words.

## Naming the spec a change answers

```sh
devplane change start "password reset" --spec specs/001-password-reset
devplane change adopt feat/password-reset --spec specs/001-password-reset
```

- The path is repository-relative and must be committed on the base (an uncommitted folder is not in
  the agent's checkout).
- Each gate run stamps a fingerprint of the folder's Markdown on the change, so the
  [certificate](@/docs/verified-done.md#the-done-certificate) says *checked against
  `specs/001-password-reset` at `3f9a…`*.

```console
$ devplane change show c-3f9a
  spec       specs/001-password-reset at 3f9a1c40b7e2d518 over 4 documents
  tasks      11 ticked · 9 seen by a passing check
```

## Sending chosen tasks

`--task` picks task lines to send; repeat it and mix forms:

```sh
# every task line that cites the requirement token REQ-3
devplane change start "reset: token expiry" \
  --spec specs/001-password-reset --task REQ-3

# one exact line, file:line relative to the spec folder
devplane change start "reset: email copy" \
  --spec specs/001-password-reset --task tasks.md:14
```

- Selected lines go in the prompt and on the run's record. A selector matching nothing refuses first.
- **Requirement tokens** are a prefix plus digits (default `FR-`, `NFR-`, `SC-`, `REQ-`, `US-`, `AC-`;
  [`[spec] tokens`](@/docs/configuration.md#spec) replaces them). A token in a heading and a task line
  is an edge; one side only is a gap. Spec Kit's `[US1]` and Kiro's `_Requirements: 1.1, 2.3_` are
  citations.

## Ticked is not verified

A ticked box is the agent's claim. Two counts are reported, never combined, and neither is *verified*.

```console
  tasks      11 ticked · 9 seen by a passing check
             ticked, sent to nobody: tasks.md:31, tasks.md:32
  REQ-3      3 tasks · 2 ticked · 0 seen by a passing check
```

- **ticked** — `- [x]` in the task file, read in the change's worktree.
- **seen by a passing check** — ticked, sent to a run with `--task`, and that run finished before a
  passing `check`. It stays true after a later failure, so it is never called verified.
- A box ticked by hand is *ticked, sent to nobody*. With no gates: *11 ticked · no gates declared*.
  No percentage anywhere.

## When the spec moves under a run

An edit to the spec folder during a run is raised when the run ends (*the specification changed 18m
into run r-9f2 and the run never saw it*):

```sh
devplane change drift <id> --run r-9f2 --tell    # tell it what changed
devplane change drift <id> --run r-9f2 --accept  # work to what the run saw
```

Either is recorded as your decision.

## A spec tool's CLI is already a gate

Put a spec validator beside your tests:

```toml
# devplane.toml
[gates]
check = ["openspec validate --strict", "pnpm test -- --run"]
```



## The gate inside Spec Kit's workflow

Spec Kit runs and waits for hooks in `.specify/extensions.yml` marked `optional: false`. Devplane's
hook names the `devplane-gate` skill, which runs `devplane gate`. Copy the skill into the project (or
`~/.claude/skills/`) first:

```sh
mkdir -p .claude/skills/devplane-gate
curl -LsSf -o .claude/skills/devplane-gate/SKILL.md   https://raw.githubusercontent.com/hupe1980/devplane/main/plugin/skills/devplane-gate/SKILL.md

devplane gate install --speckit            # writes .specify/extensions.yml
devplane gate install --speckit --dry-run  # print it, write nothing
devplane gate                              # what the skill runs
```

- Needs Spec Kit initialised (`.specify/`). An existing `extensions.yml` is not edited: the entry and
  where to add it are printed.
- Hook point `after_implement` unless `--event`.
- Refused with no `[gates] check`, or no skill at `.claude/skills/devplane-gate/SKILL.md` in the
  repository or home. The [plugin](@/docs/install.md#inside-claude-code-as-a-plugin)'s copy does not
  count. `--anyway` overrides; `--dry-run` applies the same refusals.
- `devplane gate` exits 0 only on a pass, records nothing, and never waits on a person.
  [States and flags](@/docs/cli.md#devplane-gate).

## Next

- [Verified done](@/docs/verified-done.md) — what the gate verdict means, and the certificate.
- [Configuration: `[spec]`](@/docs/configuration.md#spec) — every key.

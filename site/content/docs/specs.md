+++
title = "Working to a specification"
description = "Name the spec folder a change answers, send an agent the tasks you choose, and make your own gates the verdict inside Spec Kit's workflow."
weight = 13
[extra]
group = "guide"
+++

Write specs with Spec Kit, OpenSpec, Kiro, GSD or plain Markdown; Devplane adopts no spec format.

```sh
devplane change start "password reset" --spec specs/001-password-reset
devplane change show <id>
```

## What Devplane reads

Committed **Markdown**, one **folder** per spec, **headings** as outline, and progress as the boxes in
the tool's task file, counted the tool's way (not `checklists/`). No section names. Detected layouts,
each by its tool's own marker:

| Tool | Detected by | Spec folders | Progress |
|---|---|---|---|
| Spec Kit | `.specify/` | `specs/NNN-name/` | `- [ ]` / `- [x]` in `tasks.md` |
| OpenSpec | `openspec/config.yaml` (or `config.yml`, or the older `openspec/project.md`) | `openspec/changes/<id>/` (not `archive/`) | `tasks.md`, as `openspec list` counts it |
| Kiro | `.kiro/specs/` | `.kiro/specs/<feature>/` | `- [ ]` / `- [x]` in `tasks.md` |
| GSD | `.planning/STATE.md` | `.planning/phases/<phase>/` | none claimed |

Two in one repository is normal: each folder is read by the rules of the tool whose folder it is
under, so `specs/` reads as Spec Kit and `openspec/changes/` as OpenSpec. `devplane config check`
lists every layout found and how many folders each holds.

- **Kiro** is recognised by `.kiro/specs/`, not `.kiro/`: `openspec init --tools kiro` writes
  `.kiro/skills/` and `.kiro/prompts/`, and steering files are not specifications.
- **OpenSpec** changes and counts are OpenSpec's own. A folder that only wraps other change folders
  (`openspec/changes/mobile/refresh-token/`) is not a change, as `openspec list` reports it; a
  dot-named folder is one, as OpenSpec lists it. Every line of `tasks.md` is read, fenced blocks and
  comments included, and a box holding `x` or `X` with any spacing is done while `[~]`, `[-]` and
  `[]` are open tasks — so Devplane's counts equal `openspec list --json` on the same folder, and its
  task lines are the ones `openspec instructions apply --json` locates. A change under a schema other
  than `spec-driven` shows no count and says so.
- **OpenSpec in a store**: when `openspec/config.yaml` names a `store:` and the project's
  `openspec/` holds neither `specs/` nor `changes/`, the Specifications view and `devplane config
  check` say the changes live in that store, outside this project, instead of an empty list. Nothing
  outside the project is read.
- **GSD** keeps no task file: a phase is done when its summary file exists, not when a box is
  ticked. Every phase folder is listed — `01-auth`, inserted `02.1-hotfix`, prefixed `ABC-03-setup`,
  bracket-convention `GSD.02-05-export` — with no count, percentage or bar, and the sentence *this
  layout keeps no task file, so Devplane claims no progress for it*. Workstreams under
  `.planning/workstreams/` are not listed. When `.planning/` is gitignored, a change's worktree does
  not have it, and the change says to name it in `.worktreeinclude`.

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

Devplane ships a Spec Kit extension, `devplane`. It adds one command, `speckit.devplane.gate` (in
Claude Code, `/speckit-devplane-gate`), which runs `devplane gate` and reports what the project's own
checks exited with — no interpreting, no fixing, no re-running. It registers that command as a
mandatory `after_implement` hook, so an agent following `/speckit-implement` runs your gates before
it writes its completion report, and a red gate is in front of the agent that just wrote the code.

```sh
devplane gate install --speckit --dry-run  # what Spec Kit will be asked to install
devplane gate install --speckit            # runs `specify extension add` for you
specify extension list                     # Devplane is listed
```

With `specify` on `PATH`, Spec Kit's own command installs the extension and writes
`.specify/extensions.yml`; Devplane edits nothing there by hand. Without it, Devplane falls back to
the hand-written hook: it creates `extensions.yml` when absent, or prints the entry to paste when it
exists, and the hook names the `devplane-gate` skill, which must then be in `.claude/skills/` (the
[plugin](@/docs/install.md#inside-claude-code-as-a-plugin)'s copy does not count).

- Needs Spec Kit initialised (`.specify/`) and a `[gates] check`. `--anyway` overrides the gate
  check; `--dry-run` applies the same refusals and runs nothing.
- Hook point `after_implement` unless `--event`.
- `devplane gate` exits 0 only on a pass, records nothing, and never waits on a person.
  [States and flags](@/docs/cli.md#devplane-gate).

### When the agent skips the hook

A hook runs only if the agent follows Spec Kit's text. The extension also carries a **workflow
overlay** for Spec Kit's `speckit` workflow: a `shell` step after `implement` that runs `devplane
gate --json`. The workflow engine runs it, not the model, so the gate runs every time:

```sh
specify workflow overlay add .specify/extensions/devplane/overlays/devplane-gate.yml
specify workflow resolve speckit           # devplane-gate now follows implement
specify workflow run speckit -i spec="..."
```

A red gate fails the run at `devplane-gate` (`specify workflow run` exits 1), and the gate's own JSON
is kept with the run, under `steps.devplane-gate.output` in
`.specify/workflows/runs/<run id>/state.json`.

Remove it with `specify workflow overlay remove speckit devplane-gate` and `specify extension
remove devplane`.

## Next

- [Verified done](@/docs/verified-done.md) — what the gate verdict means, and the certificate.
- [Configuration: `[spec]`](@/docs/configuration.md#spec) — every key.

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
counts, and lines carrying those words; each such line in an open change's spec is
[its own inbox item](#answering-an-open-question).

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

## Drafting a spec with the project's own tool

Two sentences of intent can start the spec itself. Devplane runs the command your project already
uses, through the agent you choose, in the change's own worktree, and writes none of the spec:

```sh
devplane change draft rate-limit --agent claude -- \
  Limit login attempts per account. Lock the account after five tries in a row.
devplane change review <id>          # the drafted folder, file by file
devplane change draft-accept <id>    # or: devplane change draft-reject <id>
```

- **What is sent** is the tool's own command followed by your words, verbatim, and nothing else.
  For **Spec Kit** the command is spelled the way that agent invokes it, from the project's
  `.specify/integration.json`: `/speckit-specify` in skills mode, `/speckit.specify` in command mode,
  `$speckit-specify` for Codex and `/skill:speckit-specify` for Kimi. For **OpenSpec** it is the
  propose workflow OpenSpec installed for that agent — `/openspec-propose` (or `$openspec-propose`
  for Codex) where the skill is installed, else `/opsx:propose` or `/opsx-propose`. `--dry-run`
  prints it and starts nothing.
- **Spec Kit** is told the folder to write: `SPECIFY_FEATURE_DIRECTORY` names
  `specs/<next number>-<name>` inside the worktree, numbered the project's way
  (`.specify/init-options.json`) and never a number another open change already holds, and
  `SPECIFY_FEATURE_NO_PERSIST=1` keeps Spec Kit's scripts from recording an active feature (Spec Kit
  1.0.11 and later; older versions record it inside the worktree, and the change says which version
  it found). **OpenSpec**'s propose flow names its own change under `openspec/changes/`.
- **Refused before anything starts**, naming what to do: no words; an agent with no Spec Kit
  integration installed (`specify integration install <agent>`); OpenSpec's workflow missing for that
  agent (`openspec init --tools <agent>`); a project with both tools and no `--methodology`; a project
  whose only layout is Kiro or GSD, which drafting does not dispatch.
- **It is a driven run**: every permission the agent asks reaches you or a rule you wrote, and is on
  the record. While the draft waits for your decision, no gate, base check or drift check runs on it.
- **Reviewed as a diff**: the drafted folder is what the change asked for, and any other file the run
  touched is listed under *not asked for*. **Accept** makes the folder the spec the change answers;
  Devplane commits nothing. **Reject** puts the worktree back where it was at dispatch and removes the
  folder, even where git ignores it; the run and every decision stay on the record. A change answers
  one spec, so a second draft waits until the first is rejected.

Devplane writes no line into the drafted folder: every file in it is the agent's.

### Answering an open question

Each line in an open change's spec carrying one of your `open_questions` words is its own inbox item,
naming the file and line and quoting it — never folded, and gone when the line is. A line in
`checklists/` is not a question.

```sh
devplane change clarify <id> specs/002-rate-limit/spec.md:42 -- fifteen minutes
```

- **Bracketed** (`[NEEDS CLARIFICATION: which timeout applies?]`): your words replace exactly the
  bracket. The one-line diff is shown first and written only on `--yes`, a `y` at the terminal, or
  *write it* in the inbox; a line that changed since it was read is refused.
- **Unbracketed**: Devplane does not edit it. Send your answer to the change's agent
  (`devplane change prompt <id> -- <answer>`, or *send to the agent* in the inbox), and its edit comes
  back as a diff.
- **No words declared**: after a Spec Kit draft the change offers `open_questions =
  ["NEEDS CLARIFICATION"]` — the marker Spec Kit's own template writes — as a diff to
  `devplane.toml`, fenced by a comment naming Devplane and written only when you accept it. OpenSpec
  defines no such word, so after an OpenSpec draft the change names the setting and offers none.

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

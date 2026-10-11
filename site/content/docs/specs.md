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
the tool's task file, counted the tool's way (`checklists/` excluded). Each layout is detected by its
tool's own marker:

| Tool | Detected by | Spec folders | Progress |
|---|---|---|---|
| Spec Kit | `.specify/` | `specs/NNN-name/` | `- [ ]` / `- [x]` in `tasks.md` |
| OpenSpec | `openspec/config.yaml` (or `config.yml`, or `openspec/project.md`) | `openspec/changes/<id>/` (not `archive/`) | `tasks.md`, as `openspec list` counts it |
| Kiro | `.kiro/specs/` | `.kiro/specs/<feature>/` | `- [ ]` / `- [x]` in `tasks.md` |
| GSD | `.planning/STATE.md` | `.planning/phases/<phase>/` | none claimed |

Several in one repository is fine: each folder is read by the rules of the tool it sits under.
`devplane config check` lists every layout found and how many folders each holds.

- **OpenSpec** counts equal `openspec list --json` on the same folder: every line of `tasks.md` is
  read, `x` or `X` in a box is done, `[~]`, `[-]` and `[]` are open. A folder that only wraps other
  change folders is not a change. A change under a schema other than `spec-driven` shows no count and
  says so. When `openspec/config.yaml` names a `store:` and the project holds no `specs/` or
  `changes/`, Devplane says the changes live in that store and reads nothing outside the project.
- **GSD** keeps no task file, so every phase folder is listed with no count and the sentence *this
  layout keeps no task file, so Devplane claims no progress for it*. Workstreams under
  `.planning/workstreams/` are not listed. If `.planning/` is gitignored, name it in
  `.worktreeinclude` so the change's worktree has it.
- **Kiro** is recognised by `.kiro/specs/`, not `.kiro/`.

Any other layout is one line:

```toml
# devplane.toml
[spec]
plans          = "docs/plans"  # where this repository keeps its specs
open_questions = ["NEEDS CLARIFICATION", "TBD"]
```

`open_questions` has no default. Each line carrying one of those words in an open change's spec is
[its own inbox item](#answering-an-open-question). Every key: [`[spec]`](@/docs/configuration.md#spec).

## Naming the spec a change answers

```sh
devplane change start "password reset" --spec specs/001-password-reset
devplane change adopt feat/password-reset --spec specs/001-password-reset
```

- The path is repository-relative and must be committed on the base; an uncommitted folder is not in
  the agent's checkout.
- Each gate run stamps a fingerprint of the folder's Markdown on the change, so the
  [certificate](@/docs/verified-done.md#the-done-certificate) says which version was checked.

```console
$ devplane change show c-019a
  spec       specs/001-password-reset at 3f9a1c40b7e2d518 over 4 documents
  tasks      11 ticked · 9 seen by a passing check
```

## Drafting a spec with the project's own tool

Devplane runs the drafting command your project already uses, through the agent you choose, in the
change's worktree. It writes none of the spec.

```sh
devplane change draft rate-limit --agent claude -- \
  Limit login attempts per account. Lock the account after five tries in a row.
devplane change review <id>          # the drafted folder, file by file
devplane change draft-accept <id>    # or: devplane change draft-reject <id>
```

- **What is sent**: the tool's own command, then your words verbatim, nothing else. `--dry-run`
  prints it.
  - **Spec Kit**: spelled the way that agent invokes it, from `.specify/integration.json` —
    `/speckit-specify`, `/speckit.specify`, `$speckit-specify` (Codex) or `/skill:speckit-specify`
    (Kimi). `SPECIFY_FEATURE_DIRECTORY` names `specs/<next number>-<name>`, numbered the project's way
    and never reusing another open change's number; `SPECIFY_FEATURE_NO_PERSIST=1` keeps Spec Kit
    (1.0.11 or later) from recording an active feature.
  - **OpenSpec**: the propose workflow installed for that agent — `/openspec-propose`
    (`$openspec-propose` for Codex), else `/opsx:propose` or `/opsx-propose`. It names its own change
    folder.
- **Refused before anything starts**, saying what to do: no words; no Spec Kit integration for that
  agent (`specify integration install <agent>`); no OpenSpec workflow for it (`openspec init --tools
  <agent>`); both tools and no `--methodology`; a project whose only layout is Kiro or GSD.
- **A driven run**: every permission reaches you or a rule you wrote, and is recorded. While the
  draft waits for you, no gate, base check or drift check runs on it.
- **Reviewed as a diff**: other files the run touched are listed under *not asked for*. **Accept**
  makes the folder the change's spec and commits nothing. **Reject** puts the worktree back as it was
  at dispatch and removes the folder; the run and its decisions stay on the record. One draft per
  change at a time.

### Answering an open question

Each line carrying one of your `open_questions` words is its own inbox item, quoting the file and line,
and gone when the line is. Lines in `checklists/` are not questions.

```sh
devplane change clarify <id> specs/002-rate-limit/spec.md:42 -- fifteen minutes
```

- **Bracketed** (`[NEEDS CLARIFICATION: which timeout applies?]`): your words replace the bracket.
  The one-line diff is shown first and written only on `--yes`, a `y` at the terminal, or *write it*
  in the inbox; a line that changed since it was read is refused.
- **Unbracketed**: Devplane does not edit it. Send your answer to the agent
  (`devplane change prompt <id> -- <answer>`, or *send to the agent* in the inbox).
- **No words declared**: after a Spec Kit draft the change offers `open_questions =
  ["NEEDS CLARIFICATION"]` as a diff to `devplane.toml`, written only when you accept. OpenSpec has no
  such marker, so none is offered.

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

- Selected lines go in the prompt and on the run's record. A selector that matches nothing is refused.
- **Requirement tokens** are a prefix plus digits (`FR-`, `NFR-`, `SC-`, `REQ-`, `US-`, `AC-` unless
  [`[spec] tokens`](@/docs/configuration.md#spec) says otherwise). A token in both a heading and a task
  line is an edge; on one side only, a gap. Spec Kit's `[US1]` and Kiro's `_Requirements: 1.1, 2.3_`
  count as citations.

## Ticked is not verified

A ticked box is the agent's claim. Two counts are reported, never combined, and neither is *verified*:

```console
  tasks      11 ticked · 9 seen by a passing check
             ticked, sent to nobody: tasks.md:31, tasks.md:32
  REQ-3      3 tasks · 2 ticked · 0 seen by a passing check
```

- **ticked**: `- [x]` in the task file, read in the change's worktree.
- **seen by a passing check**: ticked, sent to a run with `--task`, and that run finished before a
  passing `check`. It stays true after a later failure.
- A box ticked without being sent is *ticked, sent to nobody*. With no gates: *11 ticked · no gates
  declared*.

## When the spec moves under a run

An edit to the spec folder during a run is raised when the run ends:

```sh
devplane change drift <id> --run r-9f2 --tell    # tell it what changed
devplane change drift <id> --run r-9f2 --accept  # work to what the run saw
```

Either is recorded as your decision.

## A spec tool's validator is a gate

```toml
# devplane.toml
[gates]
check = ["openspec validate --strict", "pnpm test -- --run"]
```

## The gate inside Spec Kit's workflow

Devplane ships a Spec Kit extension, `devplane`, with one command, `speckit.devplane.gate`
(`/speckit-devplane-gate` in Claude Code). It runs `devplane gate` and reports what your checks exited
with. It is registered as a mandatory `after_implement` hook, so an agent following
`/speckit-implement` runs your gates before its completion report.

```sh
devplane gate install --speckit --dry-run  # what Spec Kit will be asked to install
devplane gate install --speckit            # runs `specify extension add` for you
specify extension list                     # devplane is listed
```

- Needs `.specify/` and a `[gates] check` (`--anyway` skips the second). `--event` picks another hook
  point.
- With `specify` on `PATH`, Spec Kit installs the extension and writes `.specify/extensions.yml`.
  Without it, Devplane creates `extensions.yml` when absent or prints the entry to paste, and the hook
  names the `devplane-gate` skill, which must be in `.claude/skills/` (the
  [plugin](@/docs/install.md#inside-claude-code-as-a-plugin)'s copy does not count).
- `devplane gate` exits 0 only on a pass, records nothing, and never waits on a person
  ([flags](@/docs/cli.md#devplane-gate)).

### When the agent skips the hook

A hook runs only if the agent follows Spec Kit's text. The extension also carries a **workflow
overlay**: a `shell` step after `implement` that runs `devplane gate --json`. Spec Kit's workflow
engine runs it, not the model:

```sh
specify workflow overlay add .specify/extensions/devplane/overlays/devplane-gate.yml
specify workflow resolve speckit           # devplane-gate now follows implement
specify workflow run speckit -i spec="..."
```

A red gate fails the run at `devplane-gate` (exit 1), with the gate's JSON under
`steps.devplane-gate.output` in `.specify/workflows/runs/<run id>/state.json`. Remove it with
`specify workflow overlay remove speckit devplane-gate` and `specify extension remove devplane`.

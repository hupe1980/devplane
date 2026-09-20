---
name: devplane-gate
description: Run this repository's own checks and report what they exited with, without interpreting the result. Use when a workflow asks for a verdict on work just written — a Spec Kit hook, a definition-of-done check, or a user asking whether this project's gates pass right now. Reports only; never fixes, never re-runs.
---

# Devplane gate

Run the commands this repository committed under `[gates]` in `devplane.toml`
and report what they said. **Nothing here reads a specification, grades prose or
scores coverage.** The contribution is that something outside the agent ran the
project's own checks and can say no.

## How to run it

```sh
devplane gate run --json
```

From the repository root. It answers with `state`, `passed`, `summary` and
`commands`.

## How to report it

Report `summary` **verbatim**, then list every command whose outcome is not a
pass, each with its exit code. A reader who is told only that checks failed will
guess at which one.

Four states, and only one of them is a pass:

| `state` | What to say |
|---|---|
| `verified` | This project's checks passed, naming the commands that ran. |
| `failed` | Which command failed and what it exited with. |
| `no_gates` | This repository declares no checks, so nothing was verified. |
| `config_unreadable` | `devplane.toml` could not be read — quote the error — and nothing ran. |

**`no_gates` and `config_unreadable` are not passes.** Reporting either as one
is the failure this exists to prevent: a workflow that reads "nothing was
checked" as success has verified nothing while appearing to.

## What not to do

**Do not interpret the result.** The exit code is the verdict. It does not need
a judgement about whether the failure matters.

**Do not offer to fix it, and do not fix it.** This usually fires just after
code was written, and an agent that reads a red gate and starts repairing it has
turned a verdict into a prompt — which is the one thing a gate must never
become. Report it; the person decides.

**Do not re-run it.** A second run that passes does not retract the first, and
running until green is how a gate stops meaning anything.

**Do not answer for the person.** A red gate is theirs to act on, from
`devplane inbox` where every other decision waits.

## What it does not record

`devplane gate run` reports; it writes no row to the decision log, because it
runs against a repository rather than against a piece of work. The command that
leaves a record is `devplane work verify <id>`. Do not tell the user the result
was recorded.

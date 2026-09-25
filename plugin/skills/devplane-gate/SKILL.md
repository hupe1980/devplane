---
name: devplane-gate
description: Run this repository's own checks and report what they exited with, without interpreting the result. Use when a workflow asks for a verdict on code just written — a Spec Kit hook, a definition-of-done check, or a user asking whether this project's gates pass right now. Reports only; never fixes, never re-runs.
---

# Devplane gate

Run the commands this repository committed under `[gates]` in `devplane.toml` and report what they
said. Nothing here reads a specification, grades prose or scores coverage.

## How to run it

```sh
devplane gate run --json
```

From the repository root. It answers with `state`, `passed`, `summary` and `commands`.

## How to report it

Report `summary` **verbatim**, then list every command whose outcome is not a pass, each with its exit
code.

| `state` | What to say |
|---|---|
| `verified` | This project's checks passed, naming the commands that ran. |
| `failed` | Which command failed and what it exited with. |
| `no_gates` | This repository declares no checks, so nothing was verified. |
| `config_unreadable` | `devplane.toml` could not be read — quote the error — and nothing ran. |

Only `verified` is a pass. **Never report `no_gates` or `config_unreadable` as one.**

## What not to do

- **Do not interpret the result.** The exit code is the verdict.
- **Do not offer to fix it, and do not fix it.** Report it; the person decides.
- **Do not re-run it.** A second run that passes does not retract the first.
- **Do not answer for the person.** A red gate is theirs to act on, from `devplane inbox`.

## What it does not record

`devplane gate run` writes no row to the decision log. The command that records a result is
`devplane change verify <change>`. Do not tell the user the result was recorded.

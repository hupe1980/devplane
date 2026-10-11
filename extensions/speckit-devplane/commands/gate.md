---
description: "Run this project's own gates and report what they exited with, without interpreting the result"
---

# Devplane gate

Run the commands this repository committed under `[gates]` in `devplane.toml` and report what they
exited with. Nothing here reads a specification, grades prose or scores coverage: the exit code is
the verdict.

## User Input

```text
$ARGUMENTS
```

If the input names a gate (a single word such as `lint`), run that gate with `--name <gate>`.
Otherwise run the project's `check` gate.

## Run it

From the repository root:

```sh
devplane gate --json
```

It answers with `state`, `passed`, `summary`, `says` and `commands` (each with its exit and the tail
of its output), and `error` when the configuration could not be read. It exits `0` only when `state`
is `passed`.

If `devplane` is not installed, say so and stop: nothing was checked. Do not substitute a command of
your own for the project's gates.

## Report it

Report `summary` **verbatim**, then list every command whose outcome is not a pass, each with its exit
code.

| `state` | What to say |
|---|---|
| `passed` | This project's checks passed, naming the commands that ran. |
| `failed` | Which command failed and what it exited with. |
| `no_gates` | This repository declares no checks, so nothing was checked. |
| `config_unreadable` | `devplane.toml` could not be read — quote `error` — and nothing ran. |
| `unknown_gate` | No gate of that name is declared; name the ones that are. |

Only `passed` is a pass. **Never report `no_gates`, `config_unreadable` or `unknown_gate` as one.**
A pass here is the agent's own run of the gates: call it *passed*, never *verified*.

## Do not

- **Interpret the result.** The exit code is the verdict.
- **Fix it, or offer to.** Report it; the person decides what happens next.
- **Re-run it.** A second run that passes does not retract the first.
- **Say it was recorded.** `devplane gate` writes no record; `devplane change verify <change>` is the
  command that does.

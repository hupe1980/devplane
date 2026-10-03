# Devplane for Spec Kit

Spec Kit's commands read a specification and report on it. None of them decides whether the code is
done. This extension adds the one thing that can: **the project's own checks, run after
`/speckit-implement`, judged by their exit codes.**

It reads no `tasks.md`, scores no coverage and grades no prose. It runs the commands your repository
already declares under `[gates]` in `devplane.toml` and says what each one exited with.

## What it installs

| Piece | What it does |
|---|---|
| `speckit.devplane.gate` | A command (in Claude Code, the `/speckit-devplane-gate` skill) that runs `devplane gate --json` and reports the verdict verbatim. It never fixes, never re-runs, never calls a pass *verified*. |
| An `after_implement` hook | Mandatory (`optional: false`) and without a `condition`, so every agent that follows Spec Kit's hook text runs the gate before it writes its completion report. |
| `overlays/devplane-gate.yml` | A workflow overlay for the `speckit` workflow: a `shell` step after `implement` that runs `devplane gate --json`. The workflow engine runs it, so the gate runs whether or not the agent followed the hook — and a red gate fails the run. |

## Requirements

- [Devplane](https://hupe1980.github.io/devplane/docs/install/) 0.12.0 or later, on `PATH`.
- Spec Kit 1.0.7 or later.
- A `devplane.toml` declaring the checks:

  ```toml
  [gates]
  check = ["cargo test", "cargo clippy -- -D warnings"]
  ```

## Install

```sh
specify extension add devplane --from <release archive URL>
specify extension list            # shows Devplane
```

or, from a checkout of this repository:

```sh
specify extension add --dev extensions/speckit-devplane
```

`devplane gate install --speckit` does the same through `specify` when it is on `PATH`, after showing
what it will run.

To have the workflow engine run the gate as well — the deterministic route — add the overlay:

```sh
specify workflow overlay add .specify/extensions/devplane/overlays/devplane-gate.yml
specify workflow resolve speckit  # devplane-gate now follows implement
```

## What a run says

| `state` | Exit | Meaning |
|---|---|---|
| `passed` | 0 | Every declared command exited zero. |
| `failed` | 1 | At least one did not; each is named with its exit code. |
| `no_gates` | 1 | The repository declares no checks. Nothing was checked, and that is not a pass. |
| `config_unreadable` | 1 | `devplane.toml` would not parse. Nothing ran, and that is not a pass. |

When the overlay's step fails, the run stops at `devplane-gate` and the gate's own JSON is kept with
the run, under `steps.devplane-gate.output.stdout` in `.specify/workflows/runs/<run id>/state.json`.
`devplane gate` prints the same verdict for a person.

## Remove

```sh
specify workflow overlay remove speckit devplane-gate
specify extension remove devplane
```

Your `devplane.toml` stays: it holds commands you wrote, runnable by hand or by CI without Devplane.

## License

MIT or Apache-2.0, at your option.

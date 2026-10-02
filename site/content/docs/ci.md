+++
title = "CI verify"
description = "Run the base's own check on every pull request, and lead the job summary with the checks the change weakened. One workflow step; nothing posted to the pull request."
weight = 15
[extra]
group = "guide"
+++

`devplane verify` runs the `check` your `devplane.toml` declares **at the base branch** on the pull
request's tree, and writes a job summary led by every skipped test, deleted test and edited gate.

```sh
devplane verify --base main
```

Needs only git and the repository: no host, no store, nothing in your Devplane home, no network.

## The workflow step

```yaml
name: verify
on:
  pull_request:          # not pull_request_target: that runs with the base's secrets on the pull request's code

permissions:
  contents: read

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0   # the merge base must be in the checkout
      # Set up what your `check` needs here — toolchains, dependencies, caches.
      - uses: actions/setup-node@v4
        with: { node-version: "22" }
      - run: npm ci
      - uses: hupe1980/devplane@v0.12.0   # pin a release tag
```

The action downloads the released binary's archive, checks it against the release's `.sha256`, then runs `devplane verify --base <the pull request's base
branch>`; its exit status is `verify`'s. Runners: Linux (x64, ARM64) and Apple Silicon macOS; others
stop with a sentence naming those.

| Input | Default | What |
|---|---|---|
| `base` | the pull request's base branch | what the change is compared against; empty uses `verify`'s default |
| `version` | the release the action shipped with (`v0.12.0`) | which release to install, such as `v0.12.0`; `latest` is refused |
| `statement` | empty | a path to write the in-toto statement to; nothing is written when no tree was recorded, and the log says why |
| `install` | `true` | `false` uses a `devplane` already on `PATH` |

Outputs: `verdict` (`verified`, `failed`, `could_not_run`) and `statement` (path or empty). Keep the
statement with `actions/upload-artifact`.

## Whose `check` runs

- The one in `devplane.toml` **at the merge base**. A PR that edits `check` (to `["true"]`, say) is
  judged by the base's; the edit is the summary's first row, both command lists shown. Review roles
  come from the base too.
- The PR that adds `devplane.toml` exits 2. Make the job required once the file is on the base.
- `[workspace] setup` and named gates do not run; prepare the runner before the action.

## Exit codes

| Exit | `verdict` | When |
|---|---|---|
| 0 | `verified` | every command of the base's `check` exited 0, and the tree was the same before and after |
| 1 | `failed` | a command failed, timed out or never started, or `check` changed the tree it ran on |
| 2 | `could_not_run` | the base did not resolve, the checkout is shallow (use `fetch-depth: 0`), the merge base has no `devplane.toml` or no `check`, the diff could not be read, or `check` passed and the summary or statement could not be written |

**A weakened check never fails the job**; it is read first, not a verdict. Whether the job must pass
is your **branch protection**'s call. To block on weakened rows, add a step reading `devplane verify
--json`.

## What the summary says

In order:

1. the verdict and the base;
2. **the checks this change weakened**, by kind, or that it weakened none;
3. each command's outcome, failures named, output folded;
4. the [base checks](@/docs/verified-done.md#base-checks), after a pass: a base test failing on the
   change joins the weakened checks; a new test passing without the change is listed; test counts
   beside the verdict. Neither changes the exit code; `--no-base-checks` skips them;
5. what `verify` did not do, and its limits.

- Paths, lines and commands are literal text, never markup.
- The summary stays under the step-summary size limit, counting rows left out; `--json` lists all.
- Nothing is posted to the pull request (no comment, review, label or status).

## Limits

- The summary shows the commands ended as recorded against this tree, not that the work is correct.
- `check` runs code from the PR's tree, including edited tests.
- On `pull_request`, the workflow file and action also come from the PR's tree; guard
  `.github/workflows/` with branch protection and code owners.

## Locally

Before you push, on uncommitted work:

```sh
devplane verify                      # against origin/HEAD, else main
devplane verify --summary out.md     # the job summary, as a file
devplane verify --json               # everything, as one document
```

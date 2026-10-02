+++
title = "CI verify"
description = "Run the base's own check on every pull request, and lead the job summary with the checks the change weakened. One workflow step; nothing posted to the pull request."
weight = 15
[extra]
group = "guide"
+++

The workbench shows the checks a change weakened before it says *verified*. `devplane verify` puts
the same verdict where the merge button is: the pull request's CI job. It runs the `check` your
`devplane.toml` declares **at the base branch**, on the pull request's tree, and writes a job summary
whose first section is every skipped test, deleted test and edited gate in the diff.

```sh
devplane verify --base main
```

It needs only git and the repository's files. It starts no host, opens no store, creates nothing
under your Devplane home, and makes no network request.

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
      - uses: hupe1980/devplane@v0.9.0   # pin a release tag
```

The action installs the released `devplane` binary with the project's own installer, then runs
`devplane verify --base <the pull request's base branch>`. Its exit status is `verify`'s. It runs on
Linux (x64 and ARM64) and Apple Silicon macOS runners, which the installer supports; on any other
runner it stops with a sentence naming those.

| Input | Default | What |
|---|---|---|
| `base` | the pull request's base branch | what the change is compared against; empty uses `verify`'s default |
| `version` | `latest` | which release to install, such as `0.9.0` |
| `statement` | empty | a path to write the in-toto statement to |
| `install` | `true` | `false` uses a `devplane` already on `PATH` |

The outputs are `verdict` (`verified`, `failed` or `could_not_run`) and `statement` (the path
written, or empty). A workflow that wants the statement kept adds `actions/upload-artifact` itself.

## Whose `check` runs

The one in `devplane.toml` **at the merge base**. A pull request that edits `check` — to `["true"]`,
say — is still judged by the base's, and its edit is the first row of the summary, with the base's
commands shown beside the ones the change declares. The same goes for the review roles that decide
what counts as a test: a change cannot move its own tests out of view.

So the pull request that adds `devplane.toml` exits 2: a change cannot supply its own gate. Make the
job required after that file is on the base branch.

`verify` does not run `[workspace] setup` or any named gate. Preparing the runner is the workflow's
job, before the action.

## Exit codes

| Exit | `verdict` | When |
|---|---|---|
| 0 | `verified` | every command of the base's `check` exited 0, and the tree was the same before and after |
| 1 | `failed` | a command failed, timed out or never started, or `check` changed the tree it ran on |
| 2 | `could_not_run` | the base did not resolve, the checkout is shallow (use `fetch-depth: 0`), the merge base has no `devplane.toml` or no `check`, the diff could not be read, or `check` passed and the summary or statement could not be written |

**A weakened check never fails the job.** Refactoring a test legitimately edits it; the rows are
what a reviewer reads first, not a verdict. Whether this job must pass to merge is decided by your
**branch protection**, not by `verify`. A repository that wants a weakened row to block can add a
step of its own reading `devplane verify --json`.

## What the summary says

In a fixed order: a one-line heading with the verdict and the base; **the checks this change
weakened**, grouped by kind, or a sentence saying it weakened none; the gate that ran and each
command's outcome, failures named and output folded; the base checks; what `verify` did not do;
and its limits.

**Base checks** run after `check` passes, each in a throwaway checkout that is removed afterwards:
the base's own tests on this change (with the test files the change edited put back as the base
has them), and this change's new tests on the base. A base test that fails on the change joins the
weakened checks, by name; a new test that passes without the change is listed. Neither changes the
exit code. How many tests ran sits beside the verdict. `--no-base-checks` skips them. Every path, matched line and command is shown as
literal text, never as markup, and the summary stays under the step-summary size limit, saying how
many rows it left out — `--json` always lists them all.

Nothing is posted to the pull request: no comment, review, label or status. The summary is on the
run's page.

## Limits

The summary is evidence that these commands ended as recorded against this tree. It is not evidence
that the work is correct or that the commands check the right things. `check` runs code in the pull
request's tree, including tests the change edited. On `pull_request`, the workflow file and the
action are read from the pull request's tree too, so branch protection and code owners on
`.github/workflows/` are what guard them — not `verify`.

## Locally

The same command works before you push, against uncommitted work:

```sh
devplane verify                      # against origin/HEAD, else main
devplane verify --summary out.md     # the job summary, as a file
devplane verify --json               # everything, as one document
```

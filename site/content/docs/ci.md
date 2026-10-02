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
| `sarif` | empty | a path to write the [SARIF file](#annotations-and-sarif) to; nothing is uploaded |
| `install` | `true` | `false` uses a `devplane` already on `PATH` |

Outputs: `verdict` (`verified`, `failed`, `could_not_run`), `statement` and `sarif` (path or empty). Keep the
statement with `actions/upload-artifact`.

The statement is the [same kind the certificate gives](@/docs/verified-done.md#the-statement-a-machine-reads):
an in-toto Statement v1 with the Test Result predicate. Its subject is the tree `check` ran on, its
`result` is the base's `check` exit codes, `configuration` names those commands and the base's
`devplane.toml` blob, and `predicate.devplane.verify` names the base. It is never signed — CI holds
no person's key — and it is never the verdict: `verify`'s exit status is.

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
| 2 | `could_not_run` | the base did not resolve, the checkout is shallow (use `fetch-depth: 0`), the merge base has no `devplane.toml` or no `check`, the diff could not be read, or `check` passed and the summary, statement or SARIF file could not be written |

**A weakened check never fails the job**; it is read first, not a verdict. Whether the job must pass
is your **branch protection**'s call. To block on weakened rows, add a step reading `devplane verify
--json`.

## What the summary says

In order:

1. the verdict and the base;
2. **the checks this change weakened**, by kind, or that it weakened none — including edits to the
   agent's controls, to ownership and to the supply chain
   ([the files](@/docs/verified-done.md#what-governs-the-agent-and-what-the-build-trusts)), each
   counted under its own key in `--json`; none of them changes the exit code;
3. each command's outcome, failures named, output folded;
4. the [base checks](@/docs/verified-done.md#base-checks), after a pass: a base test failing on the
   change joins the weakened checks; a new test passing without the change is listed; test counts
   beside the verdict. Neither changes the exit code; `--no-base-checks` skips them;
5. what `verify` did not do, and its limits.

- Paths, lines and commands are literal text, never markup.
- The summary stays under the step-summary size limit, counting rows left out; `--json` lists all.
- Nothing is posted to the pull request (no comment, review, label or status).

## Annotations and SARIF

When `GITHUB_ACTIONS` is set, `verify` also prints each weakened row — every kind: skip, deleted
test, gate, agent controls, ownership, supply chain, base tests — as a **warning** annotation, and
each test that failed in `check`, named from the JUnit files `[gates] junit` declares, as an
**error** annotation:

```text
::warning file=tests/login.test.js,line=1,title=Devplane verify — check weakened::adds a skip marker `it.skip(` — it.skip("rejects an expired token", async () => {
```

- The line is the added line the row quotes, or where a removed line stood. A row that matched no
  single line (an edit to a whole gate definition, a deleted file) is annotated on its file; a row
  with no file (the base checks as a whole, a truncated diff) on none, its path in the message.
- GitHub shows at most 10 warning and 10 error annotations per step. `verify` prints that many of
  each and the summary says how many it left out.
- Annotations change no exit code. A failing test's annotation stands beside the `failed` it
  already caused.
- The rest of the log is printed with workflow commands paused, so no line the change wrote is read
  as a command.

`--sarif <path>` (the action's `sarif` input) writes the same rows, none left out, as a SARIF 2.1.0
file. A row with no file is placed on `devplane.toml`, since code scanning needs a location.
**Nothing is uploaded.** To show the rows in code scanning, add your own step; it needs
`security-events: write`, which the verify job above does not ask for:

```yaml
      - uses: hupe1980/devplane@v0.12.0
        with:
          sarif: verify.sarif
      - uses: github/codeql-action/upload-sarif@v3   # optional, your own step
        if: always()
        with:
          sarif_file: verify.sarif
          category: devplane-verify
```

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

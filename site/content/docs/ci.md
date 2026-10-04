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

It needs only git and the repository: no host, no store, and no network unless you ask it to read
[who approved the pull request](#who-approved-the-pull-request).

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

The action downloads the release's `devplane-<target>.tar.xz`, checks it against its `.sha256`, and
runs `devplane verify --base <the pull request's base branch>`; the step's exit status is `verify`'s.
Runners: Linux (x64, ARM64) and Apple Silicon macOS; any other stops with exit 2, naming those.

| Input | Default | What |
|---|---|---|
| `base` | the pull request's base branch | what the change is compared against; empty uses `verify`'s default |
| `version` | the release the action shipped with (`v0.12.0`) | the release to install; `latest` is refused |
| `statement` | empty | a path to write the in-toto statement to |
| `sarif` | empty | a path to write the [SARIF file](#annotations-and-sarif) to; nothing is uploaded |
| `install` | `true` | `false` uses a `devplane` already on `PATH` |
| `approvals` | `false` | `true` reads [who approved the pull request](#who-approved-the-pull-request) with the job's token |

Outputs: `verdict` (`verified`, `failed`, `could_not_run`), `statement` and `sarif` (the path this run
wrote, or empty). Keep the statement with `actions/upload-artifact`.

The statement is the [certificate's kind](@/docs/verified-done.md#the-statement-a-machine-reads): its
subject is the tree `check` ran on, its `result` the base's `check` exit codes, and
`predicate.devplane.verify` names the base. It is never signed, and never the verdict: the exit
status is.

## Whose `check` runs

- The one in `devplane.toml` **at the merge base**. A pull request that edits `check` (to `["true"]`,
  say) is judged by the base's, and the edit is the summary's first row. Review roles come from the
  base too.
- The pull request that adds `devplane.toml` exits 2. Make the job required once the file is on the
  base.
- `[workspace] setup` and named gates do not run; prepare the runner before the action.

## Exit codes

| Exit | `verdict` | When |
|---|---|---|
| 0 | `verified` | every command of the base's `check` exited 0, and the tree was the same before and after |
| 1 | `failed` | a command failed, timed out or never started, or `check` changed the tree it ran on |
| 2 | `could_not_run` | the base did not resolve, the checkout is shallow (use `fetch-depth: 0`), the merge base has no `devplane.toml` or no `check`, the diff could not be read, or `check` passed and the summary, statement or SARIF file could not be written |

**A weakened check never fails the job.** Whether the job must pass is your branch protection's call.
To block on weakened rows, add a step that reads `devplane verify --json`.

## What the summary says

1. The verdict and the base.
2. **The checks this change weakened**, by kind
   ([the kinds](@/docs/verified-done.md#review-before-you-merge)), or that it weakened none.
3. Each command's outcome, failures named, output folded.
4. After a pass, the [base checks](@/docs/verified-done.md#base-checks): a base test failing on the
   change joins the weakened rows; a new test passing without the change is listed.
   `--no-base-checks` skips them.
5. What `verify` did not do.

None of the rows changes the exit code. Paths and commands are literal text, never markup. The summary
stays under GitHub's size limit and counts what it left out; `--json` lists everything. Nothing is
posted to the pull request.

## Who approved the pull request

GitHub lets Copilot code review submit an approval that counts toward required approvals (since
September 2026), shown as the same green *Approved*. With the `approvals` input on, the summary gains
an **Approvals** section saying whom the required approvals came from:

```text
1 of 1 required approvals: 1 could not be attributed — a model decided, and the record has no word
for it yet (Copilot code review, copilot-pull-request-reviewer[bot]); 0 from a person's account.
1 dismissed: a person's account (alice), on 3f2a9c1.
```

```yaml
on:
  pull_request:
  pull_request_review:
    types: [submitted, dismissed]   # refresh the summary when a review lands or is dismissed
permissions:
  contents: read
  pull-requests: read
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }
      # Set up what your `check` needs here, as above.
      - uses: hupe1980/devplane@v0.12.0
        with: { approvals: "true" }
```

- **Read-only.** `GET` requests to `$GITHUB_API_URL` with the job's token, which reaches only the
  verify step and only with the input on. Nothing is written to the pull request, and the exit code
  does not change.
- **A reviewer's kind comes from what GitHub documents**, never from a login's look: a person is an
  account of type `User`; a model is an account a vendor documents as its model reviewer
  (`copilot-pull-request-reviewer[bot]` on github.com, listed by `devplane doctor` with its source).
  Any other bot is *a bot account*. A model's approval is never counted as a person's.
- **A dismissed approval** is said as dismissed and not counted; an approval on an older commit is
  counted and said as such.
- **The count required** comes from the base branch's rulesets, else classic branch protection. A
  job's token usually cannot read branch protection; then the sentence says so, never *0 required*.
- The section is as of the run, hence `pull_request_review` in the triggers. Which approvals GitHub
  counts toward its rule is GitHub's to decide.

`devplane verify --approvals` does the same outside the action, given `GITHUB_EVENT_PATH`,
`GITHUB_REPOSITORY`, `GITHUB_API_URL` and `GITHUB_TOKEN`.

## Annotations and SARIF

With `GITHUB_ACTIONS` set, `verify` prints each weakened row as a **warning** annotation, and each
test that failed in `check` (named from the JUnit files `[gates] junit` declares) as an **error**:

```text
::warning file=tests/login.test.js,line=1,title=Devplane verify — check weakened::adds a skip marker `it.skip(` — it.skip("rejects an expired token", async () => {
```

- A row is annotated on the line it quotes, else on its file, else with no file and its path in the
  message.
- GitHub shows at most 10 warnings and 10 errors per step; the summary counts the rest.
- Annotations change no exit code. The rest of the log is printed with workflow commands paused, so
  no line the change wrote is read as a command.

`--sarif <path>` (the `sarif` input) writes every row as a SARIF 2.1.0 file; a row with no file is
placed on `devplane.toml`. **Nothing is uploaded.** To show the rows in code scanning, add your own
step with `security-events: write`:

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
- `check` runs code from the pull request's tree, including edited tests.
- On `pull_request`, the workflow file also comes from the pull request's tree; guard
  `.github/workflows/` with branch protection and code owners.

## Locally

Before you push, on uncommitted work:

```sh
devplane verify                      # against origin/HEAD, else main
devplane verify --summary out.md     # the job summary, as a file
devplane verify --json               # everything, as one document
```

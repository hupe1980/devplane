+++
title = "Verified done"
description = "A change is an isolated checkout, an agent in it, and your own gate as the definition of done. Verified means that gate passed against the tree exactly as it stands."
weight = 12
[extra]
group = "guide"
+++

A **change** is an isolated checkout, an agent in it, and your own gate as the definition of done.

```sh
devplane project trust .
devplane change start "fix the flaky login test"
devplane change show <id>
```

## What `verified` means

A change is **verified** when the latest `[gates] check` passed **and** the tree it ran against is
the tree now.

- "The tree" is the git tree of the working tree, **uncommitted and untracked files included**
  (ignored files excluded), hashed as bytes on disk with no clean filter or end-of-line conversion.
  Without filters, `git add -A && git write-tree` in a copy of the checkout gives the same id.
- **Edit any file and it is stale**, both digests shown. Changing `check` in your checkout's
  `devplane.toml` makes a pass stale too.
- **A gate that edits the tree it checks cannot verify**: use `cargo fmt --check`, not `cargo fmt`.
- **Only `check` decides.** Named gates (`[gates.<name>]`) are evidence, never verification.

Gate standing is one of **verified**, **stale**, **failed**, **not run**, **no gates declared** or
**configuration unreadable**. Only *verified* is green.

## Declare the definition of done

```toml
# devplane.toml
[gates]
check               = ["pnpm typecheck", "pnpm lint", "pnpm test -- --run"]
timeout             = "10m"   # the whole gate, not per command
max_feedback_rounds = 2       # failures back to the agent, then to you
```

```sh
devplane config check  # does it parse, does all it names exist, is it safe
devplane gate          # run check here, now, and exit on the verdict
```

Gates run as children of the host, **never through the agent**, and are read from **your checkout's**
`devplane.toml`, not the agent's branch. They stop at the first failure, are never green when empty,
and keep the last 8&nbsp;KiB of output (the byte count and digest cover all of it). Every key:
[Configuration](@/docs/configuration.md#gates).

## The loop

`devplane change start`:

1. **Checks first**: trust, a clean checkout, a readable `devplane.toml`, the spec path; with several
   `--project`s, all before anything is created.
2. **Makes a worktree** at `.claude/worktrees/<slug>` on branch `change/<slug>` (`--no-worktree` works
   in place).
3. **Prepares it**: copies the files `.worktreeinclude` names, runs `[workspace] setup`.
4. **Starts the agent.**
5. **Runs `check` when the agent stops.** A failure goes back to the same session up to
   `max_feedback_rounds` times, then to your inbox. `0` stops at the first failure.

## States

Computed from the record and the tree; none is stored.

| State | Glyph | What is true |
|---|---|---|
| **drafted** | `·` | a record exists; no branch, no worktree, nothing has run |
| **isolated** | `⎇` | a branch and a worktree exist; nothing has run |
| **in flight** | `▶` | at least one run has worked on it |
| **verified** | `✓` | the latest `check` passed against the tree as it stands |
| **offered** | `↗` | a pull request exists |
| **archived** | `▣` | the worktree is gone; the record is kept |

A change may also be **waiting**: on the gates, a feedback round, or a person.

| Inbox item | What happened | What to do |
|---|---|---|
| `gate_failed` | `check` stayed red past `max_feedback_rounds` | `devplane change retry <id>` for one more round, or take over |
| `cost_spike` | the change passed a [`[budget]`](@/docs/configuration.md#budget) bound | raise the bound, then retry |
| `change_broken` | it cannot continue: its worktree is gone | archive it or start again |
| `conflict` | another change in the repository touches the same files | read both before offering either |
| `ci_red` | the offered pull request's checks failed | read the CI log; prompt the agent |

`change retry` needs the session that wrote the code. After a host restart, `devplane change resume
<id>` reconnects to the same conversation.

## Review before you merge

```sh
devplane change review <id>              # by risk, per [review] roles
devplane change review <id> --by intent  # grouped by the run that wrote it
```

The review leads with **checks weakened or changed**: signals from lines the change added or removed,
never a verdict.

| Kind | What the change's own diff did |
|---|---|
| check weakened | added a skip marker (`it.skip(`, `#[ignore`, `@pytest.mark.skip`, …); in a test file, removed an assertion with none in its place, added one that cannot fail (`assert True`, `expect(true)`), or changed a tolerance (`rel=`, `atol=`, `toBeCloseTo(`, …); outside prose, added a lint or type suppression (`@ts-ignore`, `eslint-disable`, `# noqa`, `# type: ignore`, `#[allow(`, `//nolint`) |
| test deleted | deleted a test file, or removed a test from a file that stays |
| gate changed | edited what defines passing: `devplane.toml`, CI, `justfile`, `Makefile`, pre-commit, test runner, linter, type checker or coverage configuration, `package.json` scripts, a `Cargo.toml` profile, a script a declared gate runs — or masked a failure there (`\|\| true`, `continue-on-error: true`, `--skip`, `-k "not`) |
| agent controls | edited what governs the agent: its hooks, settings, instructions, skills or MCP servers |
| ownership | edited who must review a path: `CODEOWNERS`, branch protection or a ruleset as code |
| supply chain | edited what the build trusts: a lockfile (naming the packages added, removed or changed) or a manifest's dependency declarations |

- **Verified stays the `check` exit.** A weakened row qualifies it: **1 check weakened · verified**
  in the workbench, `✓ verified · 1 check weakened` in `change show`.
- **An unseen row holds the offer.** Mark it read in the review, or
  `devplane change review <id> --seen tests/login.test.ts`. The mark records that a person read it
  (not when) and lapses if the matched text changes.
- Then every file in `[review] roles` order, each with whether a declared test covers it. The
  [workbench](@/docs/workbench.md#the-review) shows the same review as a diff.

### What governs the agent and what the build trusts

One row per file and kind. Nothing is scored or refused; the offer waits for a person to read it.

| Kind | Files |
|---|---|
| agent controls | at the repository root: `.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json`, `.codex/hooks.json`, `.codex/config.toml`, `.gemini/settings.json`, anything under `.github/hooks/`, `.claude/skills/` or `.agents/skills/`; `CLAUDE.md` and `AGENTS.md` at any depth |
| ownership | `CODEOWNERS` at the root, in `.github/` or in `docs/`; `.github/settings.yml`; a `.tf` hunk naming `github_branch_protection`, `github_branch_protection_v3` or `github_repository_ruleset` |
| supply chain | lockfiles at any depth: `Cargo.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `go.sum`, `poetry.lock`, `uv.lock`, `Pipfile.lock`, `pdm.lock`, `Gemfile.lock`, `composer.lock`, `flake.lock`; any edit to `go.mod`, `Gemfile`, `Pipfile`, `requirements*.txt`, `requirements*.in`; a hunk of `Cargo.toml`, `package.json`, `pyproject.toml` or `composer.json` that edits its dependencies |

- Created, deleted and renamed-away files count. One manifest can be two rows: a `[lints]` hunk of
  `Cargo.toml` is **gate changed**, a `[dependencies]` hunk **supply chain**.
- A lockfile row counts packages added, removed and changed, read from the diff, never a registry;
  for `bun.lock`, `Pipfile.lock`, `pdm.lock`, `composer.lock` and `flake.lock` it says the counts were
  not read.
- **Limits.** A script a hook runs is not matched by its own path; the settings file naming it is.
  Skills are matched at the root only. An ignored file is in neither the diff nor the digest.

## Offering, finishing, archiving

```sh
devplane change offer <id>    # commit the verified tree, push, open the PR
devplane change finish <id>   # accept; records the basis, removes nothing
devplane change archive <id>  # remove the worktree; keep branch and record
```

**`offer`** writes one commit on the change's branch whose tree is exactly the tree the last passing
`check` ran against.

- With `[github] pull_request = true` it pushes and opens a **draft** pull request under your
  sign-in. Without it, it prints the `git push` line and GitHub's compare address.
- Refused, exit `3`, while a weakened row is unseen, naming each row and the command that marks it
  read. Refused when the tree moved since the last pass (`devplane change verify` first).
- An offered pull request's checks are polled; a red one raises `ci_red`.

**`finish`** records the basis: *gates passed*, *gates passed, stale*, *no gate declared*, or
*finished by hand*.

**`archive`** refuses uncommitted or unmerged work unless told (`--discard-uncommitted`,
`--delete-branch --force`), and deletes the branch only with `--delete-branch`. A merged pull request
counts as merged whatever the merge strategy.

`offer`, `finish`, `archive` and `review --seen` are refused inside an agent session
([Security](@/docs/security.md#an-agent-cannot-approve-itself-mostly)).

## Base checks

The agent can edit tests and the scripts that run them, so Devplane can run `check` twice more, in a
throwaway checkout at the merge base:

| Run | What | Effect |
|---|---|---|
| **The base's tests on this change** | the verified tree, with every file that defines the checks (the `tests` role, runner, build, CI and linter configuration, files a gate names, `devplane.toml`) put back as the base has it | a failing test is named, joins the weakened rows and holds the offer until seen; so does a timeout |
| **This change's new tests on the base** | the base, with the change's `tests`-role files on it | a new test that passes here would pass without the change; said in the header, never holds the offer |

- Neither run is an input to *verified*.
- Test counts come from the runner's summary (*ran 12 → 3*); a count that could not be read is said,
  never shown as zero. Declare `[gates] junit` so failing tests are named.
- `[gates] base_checks`: `"offer"` (the default) runs them before an offer when the tree has none;
  `"auto"` after every pass; `"off"` never, and every surface says so.

```sh
devplane change verify <id> --base-checks   # check, then both runs
```

The Gates view has **Run base checks**.

## The done certificate

```sh
devplane change export <id> > cert.md  # paste into the pull request
devplane change export <id> --json     # the same facts as an in-toto statement
```

It names the repository, commit, tree digest, each command with its exit status and output digest,
the spec's fingerprint and whether the change edited a check, then how to re-run it without Devplane:

```
## Check this yourself

    git clone git@github.com:acme/widgets.git && cd widgets
    git checkout 4f2a9c1e8b3d7a5069fe2c14b8d93a70e5c6f182
    pnpm typecheck
    pnpm lint
    pnpm test -- --run
```

- It shows the commands ended as recorded against this tree, not that the work is correct. It says
  when it cannot be re-run: the commit is on no remote, or the gate ran on uncommitted files.
- Failed, timed out, never started and could not be collected are distinct; only *failed* is a verdict
  on your code. The Gates view's **Copy as markdown** gives the same bytes.

### Who approved the merge

For a change with a pull request, the certificate's *Merge approvals* section says whom the
required approvals came from — a person's account, a model a vendor documents as its reviewer, or
another bot — with each reviewer's latest review, its commit, and the `curl` that re-reads them. A
model's approval reads *could not be attributed* and is never counted as a person's. The same reading
is in `devplane verify --approvals` ([CI verify](@/docs/ci.md#who-approved-the-pull-request)).

### The statement a machine reads

`--json` prints an [in-toto Statement v1](https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md)
with the standard
[Test Result](https://github.com/in-toto/attestation/blob/main/spec/predicates/test-result.md)
predicate, so a policy that has never heard of Devplane can read it:

| Field | What |
|---|---|
| `subject[0].digest.gitTree` | the working tree `check` ran on; `gitCommit` beside it only when that tree was clean and committed |
| `predicate.result` | `PASSED` when every `check` command exited 0, otherwise `FAILED`; nothing else moves it |
| `predicate.configuration[0]` | the `check` commands, by SHA-256 of each command plus one NUL byte; `annotations.recompute` reproduces it |
| `predicate.configuration[1]` | `devplane.toml` by its git blob id, when the checked tree is committed and holds one |
| `predicate.passedTests`, `failedTests` | the commands, by outcome |
| `predicate.devplane` | what Test Result has no field for: weakened checks (`checksAltered`), base checks, exit codes and timing, decisions by authority, the steps to re-run it, and what it does not establish |

A weakened check is never folded into `result`. Two exports of an unchanged change are
byte-identical. With no tree to name — no `check` has run, or the tree moved while it ran — the
command exits 1 and says why.

### Signing it with your own key

Devplane never signs and never makes a key. To sign with the SSH key git already signs commits with:

```sh
git config --global user.signingkey ~/.ssh/id_ed25519.pub   # once, if you do not sign with SSH
devplane change export <id> --sign ssh --out cert.intoto.json
```

This writes `cert.intoto.json` and `cert.intoto.json.sig` (both or neither) with `ssh-keygen -Y sign`
under the namespace `certificate@devplane.dev`, which no commit signature uses. Only your **global**
`user.signingkey` is read, never one set in the repository. It then prints the check, which needs
only `ssh-keygen`:

```sh
printf '%s\n' 'you@example.com namespaces="certificate@devplane.dev" ssh-ed25519 AAAA…' > allowed_signers
ssh-keygen -Y verify -f allowed_signers -I 'you@example.com' -n certificate@devplane.dev -s 'cert.intoto.json.sig' < 'cert.intoto.json'
```

The reader compares the key with a copy of your public key from elsewhere, such as your forge.

## Next

- [CI verify](@/docs/ci.md): the same verdict on every pull request.
- [Working to a specification](@/docs/specs.md): ticked tasks against checked ones.
- [Configuration](@/docs/configuration.md): every key used above.

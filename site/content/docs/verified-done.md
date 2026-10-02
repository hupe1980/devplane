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

- "The tree" is the git tree of the working tree, **including uncommitted and untracked files**
  (ignored files excluded), computed with a temporary index. Files are hashed as bytes on disk: no
  clean filter, end-of-line conversion or stat cache is used. Without filters, re-derive it in a copy
  of the checkout with `git add -A && git write-tree`.
- **Uncommitted work can be verified.** `change offer` writes the one commit, from the tree that passed.
- **Touch any file and it is stale**, both digests shown. Changing `check` in your checkout's
  `devplane.toml` also makes a pass stale; deleting `check`, or a `devplane.toml` that will not load,
  means nothing is verified.
- **A gate that edits the tree it checks cannot verify**: the pass is recorded without a digest.
- **Only `check` decides.** Named gates (`[gates.<name>]`) are evidence, never verification.

Gate standing is one of **verified**, **stale**, **failed**, **not run** or **no gates declared**.

## Declare the definition of done

```toml
# devplane.toml
[gates]
check               = ["pnpm typecheck", "pnpm lint", "pnpm test -- --run"]
timeout             = "10m"   # the whole gate, not per command
max_feedback_rounds = 2       # failures back to the agent, then the change stops
```

```sh
devplane config check  # does it parse, does all it names exist, is it safe
devplane gate          # run check here, now, and exit on the verdict
```

Gates run as host children, **never through the agent**; are read from **your checkout's**
`devplane.toml`, not the agent's branch; stop at the first failure; are never green when empty; and
keep the last 8&nbsp;KiB of output (byte count and digest cover all of it).

## The loop

`devplane change start`:

1. **Checks everything first**: trust, clean checkout, readable `devplane.toml`, spec path. With
   several `--project`s every refusal is reported before anything is created.
2. **Makes a worktree** at `.claude/worktrees/<slug>` on its own branch (`--no-worktree` works in
   place, marked everywhere).
3. **Prepares it**: copies files `.worktreeinclude` names, runs `[workspace] setup`.
4. **Starts the agent.**
5. **Runs `check` when the agent stops.** Red goes back to the same session up to
   `max_feedback_rounds`, then to you in the inbox. `0` stops at the first red.

```toml
# devplane.toml
[workspace]
setup = "pnpm install --frozen-lockfile"
```

Setup is never a gate. See [`[workspace]`](@/docs/configuration.md#workspace).

## The six states

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

## When a change stops

| Inbox item | What happened | What to do |
|---|---|---|
| `gate_failed` | `check` stayed red past `max_feedback_rounds` | `devplane change retry <id>` for one more round, or take over |
| `cost_spike` | the change passed a `[budget]` bound | raise the bound in `devplane.toml`, then retry |
| `change_broken` | it cannot continue — its worktree is gone | archive it or start again |

`change retry` needs the session that wrote the code. After a host restart, `devplane change resume
<id>` reconnects to the same conversation.

## Budget

`[budget]` bounds a change by `max_usd`, `max_turns` and `max_runtime`; passing one raises
`cost_spike`. See [Configuration](@/docs/configuration.md#budget).

## Review before you merge

```sh
devplane change review <id>              # by risk, per [review] roles
devplane change review <id> --by intent  # grouped by the run that wrote it
```

The review leads with **checks weakened or changed**. Each row says what it matched:

| Kind | What the change's own diff did |
|---|---|
| check weakened | added a skip marker (`it.skip(`, `#[ignore`, `@pytest.mark.skip`, …); in a test file, removed an assertion with none in its place, added one that cannot fail (`assert True`, `expect(true)`), or changed a tolerance (`rel=`, `atol=`, `toBeCloseTo(`, …); anywhere but prose, added a lint or type suppression (`@ts-ignore`, `eslint-disable`, `# noqa`, `# type: ignore`, `#[allow(`, `//nolint`) |
| test deleted | deleted a test file, or removed a test from a file that stays |
| gate changed | edited what defines passing: `devplane.toml`, CI, `justfile`, `Makefile`, pre-commit, the test runner's, a linter's, the type checker's or coverage configuration, `package.json` scripts, a `Cargo.toml` profile, or a script a declared gate runs — or masked a failure there (`\|\| true`, `continue-on-error: true`, `--skip`, `-k "not`) |
| agent controls | edited what governs the agent: its hook and settings files, its instructions, its skills or its MCP servers ([the files](#what-governs-the-agent-and-what-the-build-trusts)) |
| ownership | edited who must review a path: `CODEOWNERS`, or branch protection or a ruleset as code |
| supply chain | edited what the build trusts: a lockfile (with the packages it added, removed or changed), or a manifest's dependency declarations |

- Only lines the change added or removed count; these are signals from the diff, not verdicts.
- *Verified* stays the `check` exit. With a weakened row, every surface leads with it:
  **1 check weakened · verified**. While a row is unseen, **Review** is the primary action and
  offering is refused.
- Mark a row read in the review, or `devplane change review <id> --seen tests/login.test.ts`. The mark
  records that a person read it (not when) and lapses if the matched text changes.
- Then the files in declared order, each with whether a declared test covers it. The
  [workbench](@/docs/workbench.md#the-review) shows the same review as a diff.
- Two changes in one repository touching the same files raise `conflict` when either reaches review.

### What governs the agent and what the build trusts

The agent can edit its own hooks, permissions and instructions, and what the build pulls in, as
easily as it edits a test. These files are read first too, one row per file and kind, each naming the
file and what it governs. **No row says the edit is good or bad**, nothing is scored, and nothing is
refused: the offer waits for a person to have read the row, exactly as for a skipped test.

| Kind | Files |
|---|---|
| agent controls | `.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json`, `.codex/hooks.json`, `.codex/config.toml`, `.gemini/settings.json`, any file under `.github/hooks/` (Copilot's repository hooks), `.claude/skills/` or `.agents/skills/` — all at the repository root; `CLAUDE.md` and `AGENTS.md` at any depth |
| ownership | `CODEOWNERS` at the root, in `.github/` or in `docs/`; `.github/settings.yml`; a hunk of a `.tf` file naming `github_branch_protection`, `github_branch_protection_v3` or `github_repository_ruleset` |
| supply chain | lockfiles at any depth: `Cargo.lock`, `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `go.sum`, `poetry.lock`, `uv.lock`, `Pipfile.lock`, `pdm.lock`, `Gemfile.lock`, `composer.lock`, `flake.lock`; manifests: any edit to `go.mod`, `Gemfile`, `Pipfile`, `requirements*.txt`, `requirements*.in`, and a hunk of `Cargo.toml`, `package.json`, `pyproject.toml` or `composer.json` that edits its dependency declarations |

- A created, deleted or renamed-away file is a row too. `devplane.toml` stays a **gate changed** row.
- A lockfile row says how many packages were added, removed, changed version, or changed only their
  checksum or source, with names (eight per class, then how many more) — read from the diff text,
  never from a registry. An edit whose package name lies outside the diff's lines is counted as
  *name not in the diff*. For `bun.lock`, `Pipfile.lock`, `pdm.lock`, `composer.lock`, `flake.lock`,
  or a lockfile the diff shows no lines of, the row says the counts were not read and why — never
  *0 packages*.
- A manifest file can be two rows: a `[lints]` hunk of `Cargo.toml` is **gate changed**, a
  `[dependencies]` hunk **supply chain**, each read on its own.
- **Limits.** A script a hook command runs (a `.claude/hooks/check.sh` that `settings.json` names) is
  not matched by its path: the row is the settings file that names it. Skills are matched at the
  repository root only. A manifest edit is recognised by its dependency keys and by lines shaped as a
  dependency declaration; one with neither is not a row (its lockfile, if any, still is). An ignored
  file is in neither the diff nor the digest, so it is not a row.

## Offering, finishing, archiving

```sh
devplane change offer <id>    # commit the verified tree, push, open the PR
devplane change finish <id>   # accept; records the basis, removes nothing
devplane change archive <id>  # remove the worktree; keep branch and record
```

**`offer`** writes one commit on the change's branch whose tree is exactly the tree the last passing
check ran against (the certificate's `gitTree`).

- With `[github] pull_request = true` in your checkout, it pushes and opens a **draft** pull request
  under your sign-in (or records the one already open). Without it, it prints the `git push` line and
  GitHub's compare address.
- Refused first while any weakened-check row is unseen: exit `3`, naming every row and the command
  that marks it read. Then refused when the tree moved since the last pass (run `devplane change
  verify`). The certificate states a weakened check either way.

**`finish`** records the basis: *gates passed*, *gates passed, stale*, *no gate declared*, or
*finished by hand*, each rendered differently.

**`archive`** refuses uncommitted or unmerged work unless told (`--discard-uncommitted`,
`--delete-branch --force`) and deletes the branch only with `--delete-branch`. After `offer` neither
is needed. A merged pull request counts as merged whatever the strategy. The record stays.

`offer`, `finish`, `archive` and `review --seen` are refused inside an agent session
([Security](@/docs/security.md#an-agent-cannot-approve-itself-mostly)).

```toml
# devplane.toml
[github]
pull_request = true
ready_label  = "devplane:ready"
```

- An offered pull request's checks are polled; a red one becomes `ci_red`, even after every session
  ended.
- `devplane change start --issue 7` starts from an issue; its body reaches the agent as untrusted text.
- A hand-made branch joins with `devplane change adopt <branch>`.

## Base checks

The agent can edit tests, runner configuration and scripts, so Devplane can run `check` twice more in
one throwaway checkout at the merge base (under `.claude/worktrees/`, removed afterwards):

| Run | What | Effect |
|---|---|---|
| **The base's tests on this change** | the verified tree, with every file defining the checks put back as the base has it: the `tests` role (or the scan's test-file names), runner, build, CI and linter configuration, files a gate command names, and `devplane.toml` | a failing test is named, joins the weakened group and holds `offer` until seen. A timeout, or setup failing with the change's files, holds it too |
| **This change's new tests on the base** | the base, with the change's `tests`-role files placed on it (nothing without a `tests` role) | a new test that passes here would pass without the change; the header says so. Never holds the offer |

- Test counts come from the runner's summary: *ran 12 → 3* is base tests on the change, then its own
  `check`. An unread count is said, never shown as zero; counts are not added across runners.
- Neither run is an input to *verified*.
- They run once per tree, and not by default:

```sh
devplane change verify <id> --base-checks   # check, then both runs
devplane change offer <id>                  # runs them first when the tree has no result
```

The Gates view has *Run base checks*. `[gates] base_checks = "auto"` runs them after every pass in the
background; `"off"` never, and every surface says so. Declare `[gates] junit` so tests are named
rather than counted.

## The done certificate

```sh
devplane change export <id> > cert.md  # paste into the pull request
devplane change export <id> --json     # same facts as an in-toto statement
```

The Gates view's **Copy as markdown** gives the same bytes. It names the repository, commit,
working-tree digest, every command with exit status and output digest and size, the spec folder's
fingerprint, and whether the change edited a check. Then:

```
## Check this yourself

    git clone git@github.com:acme/widgets.git && cd widgets
    git checkout 4f2a9c1e8b3d7a5069fe2c14b8d93a70e5c6f182
    pnpm typecheck
    pnpm lint
    pnpm test -- --run
```

- A reviewer re-runs it without Devplane. It shows the commands ended as recorded against this tree,
  not that the work is correct.
- It says when it cannot be checked: the commit is on no remote, or the gate ran on uncommitted files.
- Failed, timed out, never started and could not be collected are distinct; only *failed* is a verdict
  on your code.

### The statement a machine reads

`--json` prints an [in-toto Statement v1](https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md)
whose predicate is the standard
[Test Result](https://github.com/in-toto/attestation/blob/main/spec/predicates/test-result.md)
(`https://in-toto.io/attestation/test-result/v0.1`), so a policy that has never heard of Devplane
can read it:

| Field | What |
|---|---|
| `subject[0].digest.gitTree` | the working tree `check` ran on; `gitCommit` beside it only when that tree was clean and committed |
| `predicate.result` | `PASSED` when every `check` command exited 0, otherwise `FAILED`; never `WARNED`, and nothing else moves it |
| `predicate.configuration[0]` | the `check` commands that ran, by SHA-256 of each command followed by one NUL byte; `annotations.recompute` is the `printf … \| shasum -a 256` line that reproduces it |
| `predicate.configuration[1]` | `devplane.toml` by its git blob id, when the checked tree is committed and holds one |
| `predicate.passedTests`, `failedTests` | the commands, by outcome |
| `predicate.devplane` | everything Test Result has no field for: the weakened checks (rows, or *unknown* and why), the base checks, each command's exit code and timing, how many tool calls each authority decided, the steps to re-run it, and what it does not establish |

`result` is `check`'s exit codes and nothing more: a weakened check is listed under
`predicate.devplane.checksAltered`, never folded into the result. Deleting `predicate.devplane`
leaves a statement that means exactly what it meant. Two exports of an unchanged change are
byte-identical.

With no tree to name — no `check` has run, the gate ran outside a repository, or the tree moved while
it ran — there is no statement: the command exits 1 and says why.

```sh
devplane change export <id> --json --out cert.intoto.json
```

### Signing it with your own key

Devplane never signs and never makes a key. You can put your name to the statement with the SSH key
git already signs your commits with:

```sh
git config --global user.signingkey ~/.ssh/id_ed25519.pub   # once, if you do not sign commits with SSH
devplane change export <id> --sign ssh --out cert.intoto.json
```

This writes `cert.intoto.json` and `cert.intoto.json.sig` (both or neither) with
`ssh-keygen -Y sign` under the namespace `certificate@devplane.dev` — not git's, so the signature
cannot pass as a commit signature. Only your **global** `user.signingkey` is read: a key or signing
program set in the repository, where an agent can write, is ignored. Then it prints what a stranger
runs with nothing but `ssh-keygen`, in the directory holding both files:

```sh
printf '%s\n' 'you@example.com namespaces="certificate@devplane.dev" ssh-ed25519 AAAA…' > allowed_signers
ssh-keygen -Y verify -f allowed_signers -I 'you@example.com' -n certificate@devplane.dev -s 'cert.intoto.json.sig' < 'cert.intoto.json'
```

The allowed-signers line is the key the signature claims; a reader checks it against a copy of your
public key they obtained themselves, such as the keys your forge lists. Change one byte of the
statement and the check fails. The statement's own `signed` field stays `false`: the signature is
the file beside it.

## The same verdict in CI

`devplane verify --base main` runs the base's `check` on a pull request, weakened checks first in the
job summary. See [CI verify](@/docs/ci.md).

## Next

- [Working to a specification](@/docs/specs.md) — ticked tasks against checked ones, and the Spec Kit hook.
- [CI verify](@/docs/ci.md) — the same verdict on every pull request.
- [Configuration](@/docs/configuration.md) — every key used above.

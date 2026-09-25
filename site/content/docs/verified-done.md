+++
title = "Verified done"
description = "A change is an isolated checkout, an agent in it, and your own gate as the definition of done. Verified means that gate passed against the tree exactly as it stands."
weight = 12
[extra]
group = "guide"
+++

“The agent says it is done” is a claim; your test suite exiting zero is evidence. A **change** is an
isolated checkout, an agent in it, and your own gate as the definition of done.

```sh
devplane trust .
devplane change start "fix the flaky login test"
devplane change show <id>
```

## What `verified` means

A change is **verified** when both are true:

1. the latest run of your `[gates] check` passed, and
2. the tree it ran against is the tree now.

“The tree” is the git tree of the working tree, **including uncommitted and untracked files**
(ignored files are left out), computed with a temporary index. Re-derive it in a copy of the
checkout:

```sh
git add -A && git write-tree
```

- **Uncommitted work can be verified.** Agents need not commit.
- **Touch any file and it is stale**, with both digests shown.
- **A gate that edits the tree it checks cannot verify.** If the digest differs before and after the
  commands, the pass is recorded without a digest.
- **Only `check` decides.** Named gates (`[gates.named.*]`, `devplane gate run --name`) are evidence
  and never make a change verified. Put everything that defines done in `check`.

The gate standing a change shows is one of **verified**, **stale**, **failed**, **not run** or **no
gates declared**.

## Declare the definition of done

Committed, so it is reviewed like code:

```toml
# devplane.toml
[gates]
check               = ["pnpm typecheck", "pnpm lint", "pnpm test -- --run"]
timeout             = "10m"        # the whole gate, not per command
on_fail             = "feedback"   # feedback | escalate | ignore
max_feedback_rounds = 2
```

```sh
devplane check        # does it parse, does all it names exist, is it safe
devplane gate run     # run check here, now, and exit on the verdict
```

Gates:

- **run as children of the host, never through the agent**, so they still run when the agent has
  crashed;
- **are read from your checkout's `devplane.toml`, not the agent's branch**, so a `check = ["true"]`
  committed on the branch is not what runs;
- **stop at the first failure**;
- **are never green when empty**: no gates reads *no gates declared*;
- keep the last 8&nbsp;KiB of output; the recorded byte count and digest cover all of it.

## The loop

`devplane change start` does five things:

1. **Checks everything first** — trust, a clean checkout, a readable `devplane.toml`, the spec path.
   With several `--project`s, every refusal is reported before anything is created.
2. **Makes an isolated checkout** at `.claude/worktrees/<slug>` on its own branch (`--no-worktree`
   works in place instead, and every surface marks it).
3. **Prepares it** — copies the files `.worktreeinclude` names, then runs `[workspace] setup`.
4. **Starts the agent** in it.
5. **When the agent stops, runs `check`.** Red: the failing lines go back to the same session, up to
   `max_feedback_rounds`, then to you in the inbox.

```toml
# devplane.toml
[workspace]
setup = "pnpm install --frozen-lockfile"
```

Setup is recorded separately and is never a gate. A project with a lockfile and no `setup` is warned
before the change starts. In a Rust project, `share = ["cargo"]` lets every change reuse one build
cache.

## The six states

Computed from the record and the tree when read; none is stored.

| State | Glyph | What is true |
|---|---|---|
| **drafted** | `·` | a record exists; no branch, no worktree, nothing has run |
| **isolated** | `⎇` | a branch and a worktree exist; nothing has run |
| **in flight** | `▶` | at least one run has worked on it |
| **verified** | `✓` | the latest `check` passed against the tree as it stands |
| **offered** | `↗` | a pull request exists |
| **archived** | `▣` | the worktree is gone; the record is kept |

Beside the state a change may be **waiting** — on the gates, on a feedback round, or on a person.

## When a change stops

| Inbox item | What happened | What to do |
|---|---|---|
| `gate_failed` | `check` stayed red past `max_feedback_rounds` | `devplane change retry <id>` for one more round, or take over |
| `cost_spike` | the change passed a `[budget]` bound | raise the bound in `devplane.toml`, then retry |
| `change_broken` | it cannot continue — its worktree is gone | nothing an agent can fix; archive it or start again |

`change retry` is offered only while the session that wrote the code still exists. After a host
restart, `devplane change resume <id>` reconnects to the same agent conversation.

## Budget

`[budget]` bounds a change by `usd`, `max_turns` and `max_runtime`; passing one raises `cost_spike`.
Turns and runtime bind every agent; `usd` only an agent that reports cost. See
[Configuration](@/docs/configuration.md#budget).

## Review before you merge

```sh
devplane change review <id>              # by risk, per [review] roles
devplane change review <id> --by intent  # grouped by the run that wrote it
```

The review leads with **checks weakened or changed** (added skip markers, deleted test files, edits
to the gates or CI configuration), because an agent stuck on a red test can make the test go away.
Then the files in the declared order, each with whether a declared test covers it. The
[workbench](@/docs/workbench.md#the-review) shows the same review as a diff.

Two changes in one repository that touch the same files raise a `conflict` item when either reaches
review.

## Offering, finishing, archiving

```sh
devplane change offer <id>    # push and open the PR, or print the commands
devplane change finish <id>   # accept; records the basis, removes nothing
devplane change archive <id>  # remove the worktree; keep branch and record
```

- **`offer`**: with `[github] pull_request = true` in your checkout, pushes and runs `gh pr create`
  (a draft by default); without it, prints the exact `git push` and `gh pr create` lines. It refuses a
  worktree with uncommitted changes or a branch with no commits past its base.
- **`finish`** records why the change counts as done: *gates passed*, *gates passed, stale*, *no gate
  declared*, or *finished by hand*. They never render alike.
- **`archive`** refuses uncommitted or unmerged work unless told otherwise (`--discard-uncommitted`,
  `--delete-branch --force`), and deletes the branch only with `--delete-branch`. A merged pull
  request counts as merged whatever the merge strategy. The record stays.

```toml
# devplane.toml
[github]
pull_request = true
draft        = true
ready_label  = "devplane:ready"
```

Checks on an offered pull request are polled through your own `gh`; a red one becomes a `ci_red` item
even after every session has ended. `devplane change start --issue 7` starts from an issue, whose
body reaches the agent marked as untrusted text.

A branch you made by hand joins the same loop with `devplane change adopt <branch>`.

## The done certificate

```sh
devplane change export <id> > cert.md  # paste into the pull request
devplane --json change export <id>     # same facts as an in-toto statement
```

The Gates view's **Copy as markdown** gives the same bytes. The certificate names the repository, the
commit, the working-tree digest, every command with its exit status and the digest and size of its
output, the spec folder's fingerprint, and whether the change edited a check. Then:

```
## Check this yourself

    git clone git@github.com:acme/widgets.git && cd widgets
    git checkout 4f2a9c1e8b3d7a5069fe2c14b8d93a70e5c6f182
    pnpm typecheck
    pnpm lint
    pnpm test -- --run
```

A reviewer runs that without Devplane, so the certificate is unsigned. It is evidence that these
commands ended as recorded against this tree, not that the work is correct. It says when you cannot
check it: the commit is on no remote, or the gate ran against uncommitted files. Failed, timed out,
never started and could not be collected are four different outcomes; only the first is a verdict
about your code.

## Next

- [Working to a specification](@/docs/specs.md) — ticked vs verified tasks, and the Spec Kit hook.
- [Configuration](@/docs/configuration.md) — every key used above.

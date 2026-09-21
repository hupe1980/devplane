+++
title = "Verified done"
description = "Work that outlives sessions: an isolated checkout, your own commands as the definition of done, and a bounded feedback loop before a person is asked."
weight = 12
[extra]
group = "guide"
+++

“The agent says it is done” is a claim. `cargo test` is evidence. This is the part of Devplane that
earns it.

```sh
devplane trust .
devplane work start "fix the flaky login test" --kind bug
devplane work list
devplane work show <id>
```

## The agent's account, beside what was measured

When a gate goes red, the board and `devplane work show` print what the agent last said — under the
verdict, quiet, in its own line:

```console
  rate limiter    implement  feat/ratelimit   $0.44   specs/limits.md   gates red
  │ All tests pass and the rate limiter is complete. I ran the suite and everything is green.
```

**Both, and no judgement.** An agent's end-of-task report references about one action in eleven across
5,851 measured sessions, and drifts toward its *plan* as execution leaves it — so the report is worth
very little on its own and is the whole point next to an exit code that contradicts it. Devplane does
not decide which is right: no model separates a truthful trajectory report from an untruthful one
better than a bag-of-words detector does. It puts the sentence and the exit code on one screen.

It appears **only beside a failed gate**. Alone it reads as a summary and is not one. It is also absent
when no transcript was kept — a session Devplane only watched, or `[transcripts] keep = false` — and
that is *nothing was recorded*, never *the agent said nothing*.

## Approving what you can see

Tabbing to a work row and pressing `enter` — or clicking it — opens what is being approved: the change its branch made against the
base, every gate command with its exit code, the failing lines that were handed to the agent, and —
when there is one — the agent's account beside them. The release control is on that screen, so the
decision is made while looking at the evidence rather than after it.

- **The diff is against the merge base**, so commits that landed on `main` since the worktree was
  made are not reported as this work's doing. Uncommitted and untracked files are included, because
  an agent that has not committed has still changed the checkout.
- **A large change says what it withheld** and prints the `git` command that shows the rest. A silent
  subset is worse than no diff.
- **Four absences read differently**: nothing changed, the checkout is gone, a binary file, and a
  change too large to render. A gate that passed over nothing verified nothing, and the view says so.

## What `work start` does

1. **Validates the configuration** — before a worktree exists, so a broken chain costs nothing.
2. **Makes an isolated checkout** at `.claude/worktrees/<slug>` on `fix/<slug>`, using Claude Code's
   own convention so its cleanup sweep understands the worktree too.
3. **Prepares it** — runs `[workspace] setup`, copies the gitignored files `include` names.
4. **Puts an agent in it**, with that repository's rules.
5. **When the agent stops, runs your commands.**

Green means a person should look. Red means the failures go back to that same session — the one that
still has the context that wrote the code — bounded by `max_feedback_rounds`. When the budget is
spent, **you are asked**, in the inbox, with the same failing lines the agent was handed.

An agent that claims success without earning it reaches `failed`, never `review`.

## Phases

```
ready → implement → verify → review ─┐
                        │            ├→ done
                        ├→ human ────┘
                        └→ failed
```

`review` is where work lands when it is simply finished. `human` is where a declared pipeline
*asked* to stop — see [Pipelines](/docs/pipelines/).

## The definition of done

Your commands, committed to the repository, so they are reviewed like anything else and never
something an agent wrote for itself:

```toml
[gates]
check   = ["pnpm typecheck", "pnpm lint", "pnpm test -- --run"]
timeout = "10m"                # the whole gate, not per command
on_fail = "feedback"           # feedback | escalate | ignore
max_feedback_rounds = 2
```

Five rules govern how they run:

- **Gates run as children of the daemon, never through the agent.** Letting the thing being checked
  choose the check is the one mistake this layer exists to avoid. It also means a gate still runs
  when the agent has crashed — which is exactly when the answer matters.
- **Commands stop at the first failure.** If the type check fails, the test run after it fails for
  the same reason, and reporting both hides the cause.
- **An empty gate is not a pass.** A definition of done with no checks in it proves nothing, and
  neither does asking for a gate nobody declared — that is an error, not a silent green.
- **The configuration that governs a worktree is the one committed on its branch**, not the working
  copy of your main checkout. A project's definition of done should not change because somebody has
  an unsaved edit in another window.
- **When no runner is recognised, nothing is claimed.** Failures are extracted for cargo, `tsc`,
  jest/vitest, pytest and generic panics; otherwise the report falls back to the log and says so. A
  wrong guess sends the agent after the wrong line while hiding the real one.

Output is bounded to the last 8&nbsp;KiB, and a gate that times out still reports what the command
printed before it was killed — which is the only clue there is about where it got stuck.

## Setup is not a gate

```toml
[workspace]
setup   = "pnpm install --frozen-lockfile"
include = [".env", ".env.local"]
```

`pnpm install` says nothing about whether the work is done, so its report is recorded separately from
the gates rather than counting as one.

`include` takes repository-relative **paths, not globs**, and a path that leaves the repository is
refused: the file is committed, so it arrives with somebody else's code, and
`include = ["../../.ssh/id_rsa"]` would otherwise copy a private key into a directory an agent is
about to read.

## Reproduction gates

The one check whose success is a failure. For a bug, the test that demonstrates it must **fail**
before the fix:

```toml
[gates.named.repro]
run    = ["pnpm test -- --run tests/repro"]
expect = "fail"
```

Every command runs rather than stopping at the first failure — the failure *is* the result — and a
suite that passes is reported as *not having reproduced the problem*, with feedback that says
exactly that rather than a log of passing tests that reads as good news.

## When the bound is spent

Exhaustion is the end of the machine's half of the argument, not the end of the argument.

```sh
devplane work retry <id>    # one more round, past the bound
```

A deliberate gesture, recorded as a human decision, and counted — so the next automatic round
respects the budget from where you left it. It is offered only while the session that wrote the code
still exists: a fresh agent would have to learn all of it again, which is exactly what the feedback
loop exists to avoid. The inbox does not show the button when it cannot keep it.

## Money

Counting bounds stop an agent and a test suite arguing for ever. They do nothing about one turn
going a long way on its own:

```toml
[budget]
default_usd = 10
feature_usd = 25
```

Checked **before each step and before each further round**, because a ceiling only looked at when the
work finishes never stopped anything. Passing it raises a `cost_spike` item — deliberately not a
`gate_failed` one, because “should I spend more” and “is the code right” want different answers, and
blaming the checks for a bill sends somebody after the wrong thing.

Raise the ceiling in `devplane.toml` and `work retry` works again.

> [!WARNING]
> **A ceiling only bites when the cost is reported.** The protocol makes that field optional, and the
> GenAI semantic conventions — what Copilot and Codex emit — have no notion of money at all. So
> `devplane check` warns whenever a budget is set, and `work show` prints *"not reported by this
> agent"* rather than `$0.00`. A guard that may not fire has to say so.

## Two pieces of work, one file

An isolated checkout per piece of work stops two agents overwriting each other **while they run**,
and does nothing about the failure they actually have: two branches that are each locally correct
and cannot both land.

So when work reaches review — or a human step — Devplane compares what every in-flight worktree in
that repository has touched, and raises a `conflict` item naming the other work and the shared files:

```console
$ devplane inbox
conflict   rename the auth module is editing the same files as add oauth
           Both are in flight and both are locally correct; they cannot both land unchanged.
           src/auth.rs
```

Nothing is refused and nothing has to be declared in advance. It is one `git diff` per worktree at a
phase change — never on a timer — and it is at *normal* level, because nothing has failed. The point
is to hear it while it is still cheap rather than at the merge.

`max_parallel_runs` is the other half and it answers a different question: how many agents may run at
once. Two agents in different parts of a repository never collide, and two in one file always do, so
a limit on the count was never going to be the answer to this.

## Why work stopped

Four unrelated things stop a piece of work, and each is a different item asking a different question:

| Item | What happened | What `work retry` hands back |
|---|---|---|
| `gate_failed` | the project's checks stayed red past `max_feedback_rounds` | the failing lines the agent was already given |
| `review_exhausted` | a reviewing step kept finding things until its `max` was spent | **what the reviewer found** |
| `cost_spike` | the work passed the ceiling `[budget]` sets for its kind | nothing, until you raise the ceiling |
| `pipeline_broken` | the chain could not continue — a step no longer declared, a gate nobody wrote | nothing; there is no Retry, because no agent can fix a config file |

A reviewer's findings travel on the work item, so they survive the file they were written to and are
what goes back to the agent if you decide one more round is worth it.

`devplane work show <id>` prints the reason and, where there is one, the findings.

## GitHub

Off by default, because pushing a branch is the first thing Devplane does that other people can
see.

```toml
[github]
pull_request = true
draft        = true
ready_label  = "devplane:ready"
```

A pull request is opened **only after the gates pass**, and as a draft unless you say otherwise. One
opened earlier tells other people something is ready when it is not; one that looks finished summons
reviewers to work nobody has read.

The body carries the evidence — which checks ran, that they passed, and how many times the failures
were handed back first — so a reviewer can see the verification rather than taking the description's
word for it.

Checks are **polled**, not pushed: a webhook needs a public address and this is a local tool. A red
check becomes a `ci_red` item minutes or hours after the session ended — which is why work is the
durable unit and the session is not. An approved, green pull request asks for nothing and stays out
of the inbox.

```sh
devplane issues --ready    # what this repository labels as ready
devplane work start --issue 7 --kind bug
```

An issue's body reaches the agent **marked as an untrusted report** and bounded. It is text written
by anyone on the internet arriving at something that can run commands: evidence about a problem, not
a description of what to do.

## Finishing

```sh
devplane work verify <id>                    # run the gates now
devplane work finish <id> --remove-worktree
```

`verify` is a read — it says what the project thinks of the code right now. It never changes a phase
a person asked for, so re-running the checks on a pipeline parked at a human step leaves it parked.

`finish --remove-worktree` refuses to destroy uncommitted or unpushed work unless you add `--force`.

## The done certificate

`done` is a claim, and until it is checkable by somebody else it is a claim you have to take on
trust. That is the shape this product refuses everywhere else — the permission gate does not ask a
model what a rule means, the compatibility floor does not move by editing a constant — and it was
true of the one sentence Devplane is named for.

```sh
devplane work export <id> > cert.md          # for a pull request body
devplane --json work export <id>             # for another tool
```

**And it is on the page.** Opening a piece of work on the board
shows the basis, the commands with their outcomes, where the evidence came from, and one button that
puts the whole certificate on the clipboard as markdown — the same bytes `work export` writes. Two
clicks from a finished Work to a pull request body.

Every sentence there is composed by the daemon. The page renders and words nothing, because a
certificate described twice is a certificate that can disagree with itself, and nothing would notice.

**Where the predicate came from is named**, under the vocabulary the OpenTelemetry GenAI conventions
have open for it:

| Value | What carries it |
|---|---|
| `externally_observed` | the gate transcript — commands this tool ran, and the codes they ended on |
| `self_reported` | the agent's own account, carried as a claim and never as the predicate |
| *absent* | not known — **never defaulted**, because the only value anybody would default to is the flattering one |

The attribute is `gen_ai.evidence.origin`. The proposal is open rather than published, so the name is
adopted and nothing here claims it is a standard yet.

**All four ways a Work reaches done render as a sentence**, and *no gate was declared* is one of
them rather than an empty block — a blank reads as *nothing to show* where it means *this project
never said what done means, and nothing was checked*. A Work that finished before Devplane kept a
record says **that**, which is neither *unfinished* nor an empty certificate.

The certificate carries the gate commands as they were run, each one's outcome, the commit they ran
against, a digest of each command's output, and the fingerprint of the specification the work names.
Then it carries the part that makes it worth reading:

```
## Check this yourself

    git clone git@github.com:acme/widgets.git && cd widgets
    git checkout 4f2a9c1e8b3d7a5069fe2c14b8d93a70e5c6f182
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test
```

A reviewer runs that. Nothing in it passes through Devplane — which is the whole point, and the
reason the certificate is unsigned. A signature would say *this came from a producer you trust*,
which is the reading it exists to avoid.

### Every route to done says which route it was

Four bases, and they read differently on purpose:

| Basis | What it means |
|---|---|
| **gates passed** | The project declares gates and the run this names passed |
| **reproduced** | A gate declared to expect failure did fail — which is its pass |
| **no gate declared** | This project never said what done means. **Nothing was checked** |
| **finished by hand** | The gates did not pass and a person decided anyway |

All four are legitimate; people finish work a gate cannot judge. What must never happen is that the
record renders them alike, because then *nothing was checked* is indistinguishable from *everything
passed*.

### Four states, not a tick and a cross

A command that exited non-zero, one that ran out of time, one the shell never started, and one whose
result could not be collected are four different sentences, and only the first is a verdict about
your code. A missing binary is a broken gate, not a broken change.

### What it refuses to claim

The certificate states its limits in its own text, so they travel with the paste. It is evidence that
these commands ended as recorded against this commit. It is **not** evidence that the work is
correct, that the commands check the right things, or that the record was never altered — the digests
detect change, not forgery.

It also tells you when you cannot check it:

- **The commit is on no remote.** You cannot fetch it, so the steps will not work for you. Said where
  the steps are.
- **The tree was dirty.** The commit does not fully describe what was checked. Said where the commit
  is.
- **Unticked tasks beside a green gate.** *Gates green · 28 of 31 ticked* is a sentence neither the
  exit code nor the agent can produce alone. Both numbers go in; Devplane judges neither.

And the agent's own account, when it appears, appears beside an outcome and never instead of one —
for the reason [the top of this page](#the-agent-s-account-beside-what-was-measured) already gives.

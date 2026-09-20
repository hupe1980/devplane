+++
title = "The decision log"
description = "Why a command ran without anybody being asked, and why there is a pull request on this branch — answerable months later, with the rule or the check named."
weight = 14
[extra]
group = "guide"
+++

Two questions have to be answerable months later, in front of a repository you half remember:

> Why did that command run without anybody being asked? Why is there a pull request on this branch?

Neither is answerable from an event log, because an event log records what happened **to** Devplane.
These are facts about what Devplane **did**, or refused.

```sh
devplane audit
devplane audit <run-or-work-id>
```

```console
2026-09-13T18:04:11 daemon  gh:pr.create     https://github.com/acme/app/pull/142
                     ↳ gates passed; opened as a draft
2026-09-13T18:04:09 daemon  git:push         fix/flaky-login-a1b2c3
                     ↳ the project's gates passed
2026-09-13T18:04:02 daemon  gate:run         pnpm typecheck && pnpm test -- --run
                     ↳ check passed
2026-09-13T17:58:40 rule    agent:tool.use   Bash: rm -rf /tmp/build
                     ↳ refused by Bash(rm -rf *)
2026-09-13T17:58:31 person  agent:tool.use   rm -rf node_modules
2026-09-13T17:41:09 timer   agent:tool.use   Bash: pnpm publish
                     ↳ a clock refused it after 4h — set in devplane.toml
2026-09-13T17:22:55 nobody  agent:question   req-7c03
                     ↳ the daemon stopped while the question was waiting
```

![The audit surface: what Devplane decided, when, on whose authority, and the rule behind each verdict](/audit.png)

## The field the table exists for

Each row carries the **authority** — *on whose authority this happened* — the **action** in the same
vocabulary the permission rules speak (`agent:tool.use`, `gate:run`, `git:push`, `gh:pr.create`,
`work:advance`), the **subject**, the **outcome**, and the **reason**.

“Refused” is not an answer. “Refused by `Bash(rm -rf *)`” is.

### The five authorities

| | What it means |
|---|---|
| `person` | somebody was asked and answered — through the inbox, the board or the CLI |
| `rule` | one of your `never_auto` or `always_ask` rules matched. The rule text is in the reason, and re-evaluating it must reproduce the verdict |
| `timer` | **a clock decided**, because nobody answered before the deadline your project set in [`[questions]`](/docs/configuration/#questions). The duration and the file are in the reason — Devplane has no clock of its own |
| `nobody` | **asked, never answered, and the moment passed** — the run ended under the question, or Devplane was killed while it was waiting. Nobody decided.<br>**Stopping Devplane cleanly does not produce this row**: the question survives, and it is still yours to answer when you start it again |
| `daemon` | Devplane itself, carrying out something your project wrote down: a gate ran, a pipeline advanced, a pull request opened |

**`classifier` is deliberately not on that list.** Devplane has no channel that attributes an
individual call to a model's approval, so a variant for it would be one nothing could ever produce.
Which of your sessions are running under a classifier is a different question, and
[`devplane modes`](/docs/cli/#devplane-modes) answers it.

**And there is no `unknown`.** A row whose authority cannot be established is not a row with a sixth
kind of authority — it is a row Devplane does not write. A log that guesses is worse than one with
gaps.

## Observations are pruned; decisions are not

Both live in one SQLite file, and the asymmetry is the point rather than an accident of storage:

| | Nature | Retention | Rebuildable from |
|---|---|---|---|
| Runs, events, telemetry, transcripts | things that happened **to** Devplane | pruned on a timer, with the search index | the providers |
| Decisions | what Devplane **did**, or refused | appended, never pruned | nothing |
| Asks | what an agent put to **you**, and what became of it | kept while open, then with the decision that closed it | nothing |

An observation can be re-derived from the provider. A decision cannot be re-derived from anything —
so pruning it would leave a pull request nobody can account for. Nor can an ask: losing one loses
the question itself, which is why it survives a restart rather than living in memory.

It is written wherever something is decided — the permission hook, the protocol's permission and
question handlers, the deadline sweep, the gate runner, the pipeline, and the git and GitHub
mutations — and read from one place, which is why every row looks the same whatever wrote it.

## An undecided request is not a decision

When no rule matches, Devplane replies with *no decision* and Claude Code shows its own dialog. The
human answering that dialog is not something Devplane saw, so nothing is written. Recording it
would be recording a guess.

## A standing grant says so

"Allow always" lives inside **the agent's** session: every later call it covers is approved there,
and no request for those reaches Devplane. It is the one decision whose consequences this log cannot
show you, so two rules apply.

- **Devplane never chooses one**, because it never approves a call at all. Every `allow_always` in
  this log was chosen by a person.
- **When you choose one, the log names it** — outcome `allow_always`, not `allow`.

```console
$ devplane audit
2026-09-14T11:02:07 human   agent:tool.use   Bash: pnpm build
                     ↳ allow_always — a standing choice made by a person: the agent
                       applies it to later matching calls itself, and Devplane sees
                       no request for those
```

## What this deliberately is not

A hash-chained, tamper-evident journal in a second store.

That defends against an adversary with write access to the same laptop as the agents themselves —
which is not a threat this product has. The threat it actually faces is *“I cannot remember why that
happened”*, and one append-only table answers it. Claiming tamper evidence it does not have would be
worse than not having it.

## The boundary of the claim

> **Devplane's own actions are accounted for; the agents' actions are supervised.**

Nothing here governs what Claude Code, Codex or OpenCode do. Their tool calls are theirs. Devplane
sees a permission request for a run it drives, or a hook for a session it merely watches, and
answers — and *that answer* is a decision with a rule behind it.

No runtime, anywhere, can make an external agent's `rm -rf` at-most-once from outside the process
that runs it.

## Repeating an action is safe

Everything Devplane does to the world is idempotent, free to repeat, or checked against the remote
first: a gate is a read, pushing a branch twice is one branch, and `gh pr create` reads the branch's
pull request back because it needs the number. A pipeline step resumes rather than repeats, because
the cursor is a column on the work row.

So a crash costs you nothing but the decision log — and that is the one thing that is never pruned.

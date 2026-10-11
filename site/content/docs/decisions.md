+++
title = "The decision log"
description = "What Devplane did or refused, on whose authority, and with which rule or check — kept after everything else is pruned."
weight = 14
[extra]
group = "guide"
+++

The decision log holds what Devplane did or refused, and on whose authority. Use it to answer *why
did that command run unasked?* or *why is there a pull request on this branch?*

```sh
devplane audit                  # the latest 50 decisions
devplane audit <run-or-change>  # one run or change
devplane audit --without-me     # only what was decided instead of you
devplane audit --otel           # OpenTelemetry GenAI log records on stdout
```

```console
2026-09-13T18:04:11 person   github:pr.create https://github.com/acme/app/pull/142
                     ↳ offered as a draft
2026-09-13T18:04:09 person   git:push         change/fix-the-flaky-login-test-a1b2c3
                     ↳ offered the change
2026-09-13T18:04:02 devplane gate:run         pnpm typecheck && pnpm test -- --run
                     ↳ check passed
2026-09-13T17:58:40 rule     agent:tool.use   Bash: rm -rf /tmp/build
                     ↳ refused by Bash(rm *)
2026-09-13T17:41:09 timer    agent:tool.use   Bash: pnpm publish
                     ↳ a clock refused it after 4h — set in devplane.toml
2026-09-13T17:22:55 nobody   agent:question   req-7c03
                     ↳ the run ended under the question
```

The workbench's **Ledger** and each change's Ledger tab show the same rows.

![The ledger: what Devplane recorded as decided, when, on whose authority, and the rule behind each verdict](../../audit.png)

## Reading a row

A row has an **authority**, an **action**, a **subject**, an **outcome** and a **reason**.

| Authority | Means |
|---|---|
| `person` | somebody was asked and answered. The reason names the path: *the window*, *the notification*, or *the cli* for anything else holding the control token, a browser tab included ([where an answer came from](@/docs/permissions.md#where-an-answer-came-from)) |
| `rule` | a `deny` or `ask` rule matched, or your own [grant](@/docs/permissions.md#grants-your-own-rule-per-project) allowed the call. The reason names the rule or the grant. A line the gate could not read is `rule` with outcome `unresolved` |
| `timer` | nobody answered before your project's [`[questions] deadline`](@/docs/configuration.md#questions); the reason names the duration and file |
| `nobody` | asked, never answered, and the moment passed: the run ended under the question, or a held permission lapsed. Quitting the host cleanly leaves the question open |
| `devplane` | Devplane carrying out what your project wrote down: a gate run, a budget stop |

A row without an established authority is not written; there is no `unknown`. A push and a pull
request are `person`, because you offered the change.

| Action | Recorded for |
|---|---|
| `agent:tool.use`, `agent:question` | a tool call or a question an agent put |
| `gate:run` | a gate run on a change |
| `git:push`, `github:pr.create` | an offered change |
| `change:…` | what you did to a change: `adopt`, `retry`, `resume`, `stop`, `done`, `archive`, `draft`, `draft.accept`, `draft.reject`, `drift.accepted`, `drift.told`, `agent.switch`, `agent.sign_in` |
| `run:resume`, `run:stop` | the same for a run |
| `spec:clarify` | an open question answered in a specification |
| `config:open_questions` | `[spec] open_questions` written into `devplane.toml` at your word |
| `policy:grant`, `policy:revoke`, `policy:explain` | a grant written or removed; an `explain` asked of the host, such as over MCP |

- `--without-me` drops `person` and `devplane` rows and adds the **Not attributed** list below.
- `--otel` writes the authority on each `gen_ai.tool.call.decision` record. Nothing is sent.
- [`devplane status --modes`](@/docs/cli.md#devplane-status) shows the sessions that never prompt.

## A session you started yourself

With Claude Code's telemetry connected (`devplane agent connect claude`), every permission decision
Claude Code reports for your own session is recorded, with its own word beside the authority
(`↳ Claude Code: user_reject`). An authority is written only where that word has one reading:

| Claude Code reports | Recorded as |
|---|---|
| `user_permanent`, `user_temporary`, `user_abort`, `user_reject`, in the terminal CLI | `person` |
| the same in an Agent SDK, `-p`, IDE or desktop session | not attributed: there a saved rule's match is reported the same way |
| `config`, with a rule in your Claude Code settings that matches the call | `rule`, naming the rule and its file |
| `config` otherwise | not attributed |
| `hook` | Devplane's own row for that call; another program's hook is not attributed |
| no source, or a value Claude Code does not document | not attributed, with the value as received |

`config` reads `rule` only when the permission mode at that moment is known, is not auto mode and
does not allow the call by itself, and a readable settings rule matches with the same effect.

`devplane audit --without-me` lists every other reported call under **Not attributed**:

```console
Not attributed (3)
2026-09-30T10:01:00 Bash     accept  could not be attributed — auto mode was on
                     ↳ Claude Code: config
```

| Reason | When |
|---|---|
| *auto mode was on* | a rule, an earlier grant or Claude Code's classifier may have decided |
| *the permission mode was not known* | nothing Claude Code sent names the mode at that moment |
| *the mode allowed it* | the mode allows the call by itself (accept edits, bypass permissions) |
| *Claude Code's settings decided and do not say how* | no readable rule matches the call |
| *Claude Code reports a saved rule's match as the person's in this kind of session* | a `user_*` report outside the terminal CLI |
| *a hook decided — no hook decision Devplane recorded names this call* | another program's hook, or Devplane's `PermissionRequest` hook, which carries no call id |
| *the vendor sent no source* / *sent "…"* | the source was missing or undocumented |

- The mode in force is read from Claude Code's own timestamps and order, not from arrival time.
- Telemetry never answers a permission; only the hook's or the protocol's answer clears one.
- `devplane doctor` names any undocumented source value Claude Code sent, with when it was first seen.
- The Not attributed list lives with the session's events and is pruned with them.

No decision is recorded as a model's. Codex's auto-review is a reviewer model deciding, and no
channel Devplane reads carries those decisions, so none is recorded. A driven Codex session in Auto
review says *decides without you: a reviewer model decides*.

## What the log keeps

- Decisions and asks are never pruned. Events, telemetry and transcripts are pruned after 30 days,
  finished runs after 7.
- The hook writes decisions directly, with no host. When the store will not open they go to
  `~/.devplane/pending-decisions.jsonl`; `devplane doctor` counts them and the next host files
  them, marked late.
- A store schema change starts a new store with an empty Ledger; the old file stays readable
  ([Troubleshooting](@/docs/troubleshooting.md#the-ledger-is-empty-after-an-upgrade)).

## What is not in the log

- **An undecided call**: your agent's own dialog answered it.
- **Calls under an agent's "allow always"**: the row that chose it says `allow_always`; later
  matching calls do not appear.
- **A Claude Code mod that refuses a call**, and **an approval given in the chat** rather than at
  the prompt: Claude Code reports no source for either.
- **A watched call with telemetry off**: nothing is reported, and nothing is inferred.

## Limits

- The log is an append-only local SQLite file. It is not tamper-evident ([Security](@/docs/security.md)).
- Devplane's own actions are safe to repeat: a gate is a read, a branch pushed twice is one branch,
  and an existing pull request is looked up first.

+++
title = "The decision log"
description = "Why a command ran without anybody being asked, and why there is a pull request on this branch — answerable months later, with the authority and the rule or check named."
weight = 14
[extra]
group = "guide"
+++

The event log records what happened **to** Devplane. The decision log records what Devplane **did**
or refused, and on whose authority, so *why did that command run without anybody being asked?* and
*why is there a pull request on this branch?* stay answerable months later.

```sh
devplane audit                   # the latest 50 decisions
devplane audit <run-or-change>   # narrowed to one run or change
devplane audit --without-me      # only what was decided instead of you
devplane audit --otel            # OpenTelemetry GenAI log records on stdout
```

```console
2026-09-13T18:04:11 person   gh:pr.create     https://github.com/acme/app/pull/142
                     ↳ offered as a draft
2026-09-13T18:04:09 person   git:push         fix/flaky-login-a1b2c3
                     ↳ offered the change
2026-09-13T18:04:02 devplane gate:run         pnpm typecheck && pnpm test -- --run
                     ↳ check passed
2026-09-13T17:58:40 rule     agent:tool.use   Bash: rm -rf /tmp/build
                     ↳ refused by Bash(rm *)
2026-09-13T17:58:31 person   agent:tool.use   rm -rf node_modules
2026-09-13T17:41:09 timer    agent:tool.use   Bash: pnpm publish
                     ↳ a clock refused it after 4h — set in devplane.toml
2026-09-13T17:22:55 nobody   agent:question   req-7c03
                     ↳ the run ended under the question
```

The same rows are in the workbench's **Ledger**, and on each change's Ledger tab.

![The ledger: what Devplane recorded as decided, when, on whose authority, and the rule behind each verdict](../../audit.png)

## What a row says

The **authority**, the **action** (`agent:tool.use`, `agent:question`, `gate:run`, `git:push`,
`gh:pr.create`, `change:archive`), the **subject**, the **outcome**, and the **reason**: not
"refused" but "refused by `Bash(rm *)`".

| Authority | What it means |
|---|---|
| `person` | somebody was asked and answered — in the workbench or with `devplane answer` |
| `rule` | a `never_auto` or `always_ask` rule matched. The rule is in the reason, and re-evaluating it reproduces the verdict |
| `timer` | nobody answered before the deadline your project set in [`[questions]`](@/docs/configuration.md#questions); the duration and file are in the reason |
| `nobody` | asked, never answered, and the moment passed — the run ended under the question, or a held permission lapsed. Quitting the host cleanly does **not** produce this: the question survives |
| `devplane` | Devplane doing what your project wrote down: running a gate, stopping a change at its budget. A push and a pull request are `person`: you offered the change |

There is no `unknown`: a row whose authority cannot be established is not written. `--without-me`
leaves out `person` and `devplane` rows. With `--otel`, the authority rides on each
`gen_ai.tool.call.decision` record; nothing is sent anywhere.
[`devplane modes`](@/docs/cli.md#devplane-modes) shows which sessions run with no prompts at all.

## Decisions are never pruned

Runs, events, telemetry and transcripts are pruned on a timer. Decisions and asks are appended and
kept.

The hook writes its decision straight to the store, so no host has to be running. If the store will
not open, the decision goes to `~/.devplane/pending-decisions.jsonl`; `devplane doctor` counts what
is waiting there, and the next host files it, marked as filed late.

## What became of a report

A report writes up to three actions on the record of the change that filed it:

| Action | Authority | When |
|---|---|---|
| `report:resolved` | `person` | answered: fixed, rejected or deferred with a reason, or a GitHub draft discarded |
| `report:opened` | `person` | a GitHub draft opened with your own `gh`; the issue address is the reason |
| `report:delivered` | `rule` | the target's `[reports] deliver_from` named the source, so the report was handed to its live run |

Filing a report writes no decision. See [Reports between projects](@/docs/reports.md).

## What is not in the log

- **An undecided call.** When no rule matches, your agent shows its own dialog. Devplane does not see
  your answer there, so it writes nothing.
- **Calls under a standing "allow always".** That choice lives in the agent's session. When you choose
  it in a run Devplane drives, the row's outcome says `allow_always`, so you know later matching calls
  will not appear.

## Limits

Devplane's own actions are accounted for; the agents' actions are supervised. The log is
append-only in a local SQLite file, not tamper-evident. See [Security](@/docs/security.md).

Everything Devplane does to the world is safe to repeat: a gate is a read, pushing a branch twice is
one branch, and an existing pull request is looked up before one is opened.

+++
title = "The decision log"
description = "Why a command ran without anybody being asked, and why there is a pull request on this branch — answerable months later, with the authority and the rule or check named."
weight = 14
[extra]
group = "guide"
+++

What Devplane **did** or refused, and on whose authority: *why did that command run unasked?* and *why
is there a pull request on this branch?*, answerable months later.

```sh
devplane audit                  # the latest 50 decisions
devplane audit <run-or-change>  # narrowed to one run or change
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
2026-09-13T17:58:31 person   agent:tool.use   rm -rf node_modules
2026-09-13T17:41:09 timer    agent:tool.use   Bash: pnpm publish
                     ↳ a clock refused it after 4h — set in devplane.toml
2026-09-13T17:22:55 nobody   agent:question   req-7c03
                     ↳ the run ended under the question
```

The same rows are in the workbench's **Ledger** and each change's Ledger tab.

![The ledger: what Devplane recorded as decided, when, on whose authority, and the rule behind each verdict](../../audit.png)

## What a row says

**Authority**, **action** (`agent:tool.use`, `agent:question`, `gate:run`, `git:push`,
`github:pr.create`, `change:archive`), **subject**, **outcome**, and **reason** ("refused by
`Bash(rm *)`").

| Authority | What it means |
|---|---|
| `person` | somebody was asked and answered — in the workbench or with `devplane inbox answer`. The reason says *answered from the window* only for the app's window, which proves it per question; anything else holding the token, a browser tab on `devplane up` included, is *the cli* |
| `rule` | a `deny` or `ask` rule matched. The rule is in the reason, and re-evaluating it reproduces the verdict |
| `timer` | nobody answered before the deadline your project set in [`[questions]`](@/docs/configuration.md#questions); the duration and file are in the reason |
| `nobody` | asked, never answered, and the moment passed — the run ended under the question, or a held permission lapsed. Quitting the host cleanly does **not** produce this: the question survives |
| `devplane` | Devplane doing what your project wrote down: running a gate, stopping a change at its budget. A push and a pull request are `person`: you offered the change |

- No `unknown`: a row without an established authority is not written.
- `--without-me` drops `person` and `devplane` rows. `--otel` puts the authority on each
  `gen_ai.tool.call.decision` record; nothing is sent.
- [`devplane status --modes`](@/docs/cli.md#devplane-status) shows sessions that never prompt.

## Decisions are never pruned

- Events, telemetry and transcripts are pruned after 30 days, finished runs after 7; decisions and
  asks are kept.
- A schema change starts a new store and the Ledger starts empty; the old log stays readable
  ([Troubleshooting](@/docs/troubleshooting.md#the-ledger-is-empty-after-an-upgrade)).
- The hook writes decisions directly, no host needed. If the store will not open they go to
  `~/.devplane/pending-decisions.jsonl`; `doctor` counts them and the next host files them, marked
  late.

## What is not in the log

- **An undecided call**: your agent's own dialog answers it, unseen.
- **Calls under "allow always"**: in a driven run the choosing row says `allow_always`; later matching
  calls do not appear.

## Limits

- Append-only in a local SQLite file, not tamper-evident ([Security](@/docs/security.md)).
- Devplane's actions are safe to repeat: a gate is a read, a branch pushed twice is one branch, an
  existing pull request is looked up first.

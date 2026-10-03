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
| `person` | somebody was asked and answered — in the workbench, from the app's notification, or with `devplane inbox answer`. The reason names the path: *answered from the window* only for the app's window, which proves it per question; *from the notification* only for an action pressed on the app's own notification; anything else holding the token, a browser tab on `devplane up` included, is *the cli* ([where an answer came from](@/docs/permissions.md#where-an-answer-came-from)) |
| `rule` | a `deny` or `ask` rule matched. The rule is in the reason, and re-evaluating it reproduces the verdict |
| `timer` | nobody answered before the deadline your project set in [`[questions]`](@/docs/configuration.md#questions); the duration and file are in the reason |
| `nobody` | asked, never answered, and the moment passed — the run ended under the question, or a held permission lapsed. Quitting the host cleanly does **not** produce this: the question survives |
| `devplane` | Devplane doing what your project wrote down: running a gate, stopping a change at its budget. A push and a pull request are `person`: you offered the change |

- No `unknown`: a row without an established authority is not written.
- `--without-me` drops `person` and `devplane` rows, and lists the calls nobody can be named for
  apart from them (below). `--otel` puts the authority on each
  `gen_ai.tool.call.decision` record; nothing is sent.
- [`devplane status --modes`](@/docs/cli.md#devplane-status) shows sessions that never prompt.

## A session you started yourself

When Claude Code's telemetry is connected (`devplane agent connect claude`), every permission
decision it reports for a session you started is recorded — with Claude Code's own word for who
decided kept beside the authority (`↳ Claude Code: user_reject`). Devplane writes an authority only
where Claude Code's definition of that word allows no other reading:

| Claude Code reports | Recorded as |
|---|---|
| `user_permanent`, `user_temporary`, `user_abort`, `user_reject` in the terminal CLI | `person` |
| the same in an Agent SDK or `-p` session, or one whose kind is not known | not attributed — Claude Code also reports a saved rule's match that way there |
| `config`, and Devplane finds the allow or deny rule in your Claude Code settings that matches the call | `rule`, naming the rule and its file |
| `config` otherwise | not attributed — with the reason below |
| `hook` | Devplane's own row for that call, with the report beside it; any other hook is not attributed |
| no source, or a value Claude Code does not document | not attributed, with the value as received |

A session's kind comes from Claude Code's own launch surface: `cli` is the terminal, `sdk-*` is the
Agent SDK and `-p`, and a session Devplane drives is never interactive. Anything else — an IDE, the
desktop app — is *not known*, so its `user_*` reports are not credited to you.

`config` covers your settings' rules, managed policy, command-line flags, the permission mode, a
grant you made earlier in the session, tools that are always safe, and a prompt that failed — and
Claude Code does not say which. So a call reads `rule` only when the permission mode at that moment
is known, is not auto mode, and does not allow the call by itself, **and** a rule in a settings file
Devplane can read matches the call with the same effect.

Every other reported call is listed apart from the rows by `devplane audit --without-me`, under
**Not attributed**, with its reason and no authority:

```console
Not attributed (3)
2026-09-30T10:01:00 Bash     accept  could not be attributed — auto mode was on
                     ↳ Claude Code: config
```

| Reason | When |
|---|---|
| *could not be attributed — auto mode was on* | the call was decided while auto mode was on: a rule, your earlier grant or Claude Code's classifier may have decided, and nothing says which |
| *could not be attributed — the permission mode was not known* | nothing Claude Code sent says which mode was in force at that moment |
| *could not be attributed — the mode allowed it* | the mode allows the call by itself (accepting edits for an edit, bypassing permissions) |
| *Claude Code's settings decided and do not say how — …* | no rule Devplane can read matches the call |
| *Claude Code reports a saved rule's match as the person's in this kind of session* | a `user_*` report outside the terminal CLI |
| *a hook decided — no hook decision Devplane recorded names this call* | another program's hook, or Devplane's own `PermissionRequest` hook, which Claude Code gives no call id |
| *not attributed — the vendor sent no source* / *sent "…"* | the source was missing, or is not one Claude Code documents |

The mode in force is read by Claude Code's own timestamps and ordering, not by when telemetry
arrived. Telemetry never answers a permission: a waiting session is cleared by the hook's or the
agent protocol's own answer, never by a report arriving later. `devplane doctor` names any source
value Claude Code sent that its documentation does not define, with when it was first seen and the
date the documented values were last read; calls carrying it stay unattributed. The unattributed
list is kept with the session's events, so it is pruned after 30 days; decision rows are not.

**No decision is recorded as a model's.** A model decider is never mapped to `person`, `rule` or
`nobody`. OpenAI documents Codex's auto-review as a reviewer model deciding approvals, but no channel
Devplane reads carries that decision per call, so nothing is recorded for it. A Codex session
Devplane drives says so when you choose Auto review: *decides without you: a reviewer model
decides*.

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
- **A Claude Code mod that refuses a call**, and **an approval you give in the chat** rather than
  at the prompt: Claude Code reports no source for either, so neither is recorded as decided by
  anybody.
- **A watched call when telemetry is off**: nothing is reported, and nothing is inferred.

## Limits

- Append-only in a local SQLite file, not tamper-evident ([Security](@/docs/security.md)).
- Devplane's actions are safe to repeat: a gate is a read, a branch pushed twice is one branch, an
  existing pull request is looked up first.

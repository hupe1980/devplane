+++
title = "Architecture"
description = "One binary, one store, one rule: the pure half may not reach the outside world. The event bus, the reducers, the API and what recovery does."
weight = 23
[extra]
group = "reference"
+++

## One binary

```
┌───────────────┐  ┌────────────────┐
│ browser tab   │  │ vibeplane CLI  │
│ (embedded UI) │  │ (same binary)  │
└──────┬────────┘  └──────┬─────────┘
       │  HTTP + SSE      │ HTTP + SSE
┌──────▼──────────────────▼───────────────────────────────────┐
│                 vibeplane serve (the daemon)                 │
│ api      REST + SSE, bearer token, loopback only             │
│ core     domain, reducers, attention engine, policy (pure)   │
│ acp      protocol client; every agent that speaks it         │
│ observe  hooks (HTTP), OTLP/HTTP, the session roster         │
│ work     worktrees, gates, declared chains, findings         │
│ git/gh   the git CLI, the gh CLI                             │
│ web      one embedded HTML file on loopback                  │
│ store    SQLite (WAL, FTS5): observations + decisions        │
└──────────────────────────────────────────────────────────────┘
   ▲ hooks / telemetry from any session   ▲ stdio to agents
```

Install is one binary with nothing else to run. The daemon is started by any client command and
found through `~/.vibeplane/daemon.json`, which records the pid and the port it actually bound.
Notifications and window raising shell out to the platform, so neither needs a GUI framework.

A **second instance is supported rather than an error**: `VIBEPLANE_HOME=/tmp/vp vibeplane ls` gets
its own database, token and daemon record, and — because the usual port is taken by the real one — its
own port.

## Events, reducers, state

- `EventEnvelope { id, at, run_id, project_id?, source, event }`. Observations are append-only.
- Reducers are **pure functions** `(Run, Event) -> Run`, unit-tested by replay.
- The board subscribes to state rather than reconstructing it from events, which is the difference
  between a dashboard that survives a high-volume session and one that melts.
- The live stream carries **two kinds of frame, tagged**: `event` for a state change, `message` for a
  fragment of what a driven agent said. A reader never has to inspect a payload to find out which it
  was handed, and `?run=` narrows it.

Three claims rest on that purity and on nothing else: the board is rebuildable, because run state is
a replayable reduction; the inbox is correct after a restart, because it is derived and never stored;
and the permission policy cannot fail open, because it cannot wait on anything.

## One rule, enforced

`src/core/` may not reach the outside world: no `async fn`, no `.await`, no runtime, no database, no
HTTP. A test reads every file in that directory and fails the build on a violation — with the
sentence explaining the rule rather than only the token that broke it, and a second test asserting
the matcher itself, because a guard that cannot fail is a guard nobody should trust.

The one thing deliberately allowed there is a **synchronous** read of a small local file, which is how
a project's rules are read on the permission hook and is bounded in a way a network call is not.

## One store, three kinds of row

SQLite, WAL, FTS5, in one file.

It holds **observations** — events and the run and work rows derived from them. Query-heavy, and
rebuildable from the providers, which is why losing the file costs history rather than correctness.
It holds **transcripts**: what driven agents said, pruned on the same sweep because that is history
too. And it holds the **decision log**, which is the exception to every rule above: appended, never
updated, never pruned, outside the sweep.

The retention sweep prunes the event log **and its search index together** — an index that outlives
what it indexes is both the larger half of the file and a search that finds things that are no longer
there.

It is one file, and `sqlite3 ~/.vibeplane/vibeplane.db` opens it — the search, the run history and
the decision log are all queryable with a tool you already have.

## The API

HTTP + JSON on loopback, with Server-Sent Events for live updates, and a bearer token on everything.
One transport, not two: the hook receivers need an HTTP listener regardless, a hook can only post to
a URL, and SSE is one line in a browser and needs no client library.

| | |
|---|---|
| **Read** | `/api/board` (`?all=true`), `/api/inbox`, `/api/runs/{id}`, `/api/runs/{id}/events`, `/api/runs/{id}/messages`, `/api/agents`, `/api/work`, `/api/decisions`, `/api/search`, `/api/diagnostics`, `/api/attention`, `/api/projects`, `/api/stream`, `/healthz` |
| **Write** | `/api/dispatch`, `/api/work`, `/api/work/{id}/{verify,finish,approve,retry}`, `/api/issues`, `/api/projects/trust`, `/api/runs/{id}/{prompt,decide,stop,snooze,focus}`, `/api/shutdown` |
| **Receivers** | `/vibeplane/hook`, `/vibeplane/policy`, `/vibeplane/statusline`, `/vibeplane/otel/v1/{logs,metrics}` |

Everything under `/api` and `/vibeplane` requires the token, except `/healthz` — which proves the port
is ours without revealing what is on it — and the telemetry endpoints, because the exporter cannot be
given a per-signal credential without sending it to every other collector you configure. Those accept
observations only, never commands.

`/api/work` serves each item with its last gate **already judged** (`gate.passed`, `gate.summary`).
There is one definition of a passing gate, it lives in the domain, and it is the one on the wire — so
no surface can read a reproduction gate, where failing *is* passing, backwards.

## Recovery

On daemon start: load projects, work and runs → reconcile against the session roster and live process
ids → mark `lost` where appropriate → re-arm stall timers → rebuild the inbox → open the API.

**Restoring is not believing.** A run recorded as working is a claim about a process that may have
died while the daemon was down. But “we could not ask” and “nothing is running” are different
answers, and conflating them would mark every restored run lost at once — so a roster that cannot be
read is *no evidence*, and the process-id check carries the weight alone.

Work that was mid-flight produces an **`interrupted`** item rather than sitting on the board looking
busy for ever. Its branch and worktree are untouched, and nothing will move it on its own — but the
item carries a **resume** where one is possible, because the id the agent knows its session by is
recorded when the run starts and comes back with it. See [Agents](/docs/agents/).

A driven run has no process id, so reconciliation rightly leaves its row alone — but no session
survives a restart. **“Can this be driven” is therefore a question about sessions**, answered from
the set the daemon still holds and never from run state.

## Shutdown

Every agent Vibeplane started is a protocol connection holding a child process group. A daemon that
simply exits leaves each of them re-parented to init and still spending — a model with a subscription
attached, running, with nothing left on the machine that knows it is there.

`serve` stops them and waits, bounded, before it returns. `vibeplane stop` reaches the same path.

## Technology

| Area | Choice |
|---|---|
| Core | Rust stable, Tokio, serde, tracing |
| Agents | the Agent Client Protocol Rust SDK — one client for every agent that speaks it |
| Store | SQLite via `sqlx`, WAL, FTS5 |
| Telemetry ingest | a hand-written OTLP/HTTP **JSON** reader behind `axum` — the exporter is configured for JSON, so the four record shapes are about sixty lines of serde rather than a protobuf toolchain |
| Git / GitHub | the `git` and `gh` CLIs, for parity with what agents and people run by hand |
| UI | one embedded HTML file, no build step — about three hundred lines of plain JavaScript against the same JSON the CLI reads |
| Notifications | the platform's own notifier: `osascript`, `notify-send`, PowerShell toast |

The UI has no build step on purpose. A dashboard that cannot be opened without `npm install` rots the
first time the toolchain moves, and a Rust build that depends on a JavaScript build is a Rust build
that breaks for everyone. The day it needs a terminal emulator and a diff viewer is the day a bundler
earns its place.

## Performance

Targets, each asserted by a test where a test can assert it.

- Daemon start → API ready < 1 s.
- Permission check **< 200 µs**, path rules included, over 10 000 lookups.
- Permission hook round trip over loopback **< 50 ms**, worst of 50 consecutive requests.
- Session roster polled at most once every 2 s while anything is busy, 10 s when idle.
- Binary ≤ 25 MB; measured 12 MB on macOS arm64, release, stripped.

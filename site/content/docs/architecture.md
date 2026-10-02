+++
title = "Architecture"
description = "One binary and one store: the hook writes, the host folds, the pure core may not reach the outside world. The API route table, recovery, shutdown and the window."
weight = 23
[extra]
group = "reference"
+++

## One binary, and the store is the bus

```
  Claude Code / Codex / Copilot session
        │ command hooks
        ▼
  ┌──────────────────┐
  │  devplane hook   │  decides in its own process, answers on stdout
  └────────┬─────────┘
           │ appends events, decisions, held asks
           ▼
  ┌─────────────────────────────────────────────┐
  │     ~/.devplane/devplane.db   (SQLite)      │ ◀── read commands, with no host
  └─────────────────────────────────────────────┘
           ▲ tails past the projection mark every 500 ms
           │
  ┌────────┴────────────────────────────────────┐
  │ the host: devplane up                       │
  │ api      REST + SSE, bearer token, loopback │
  │ core     domain, reducers, attention, policy│
  │ acp      the client for every driven agent  │
  │ observe  OTLP, the roster, OpenCode's feed  │
  │ change   worktrees, gates, offers           │
  │ git      the git CLI                        │
  │ github   GitHub's API, its own sign-in       │
  │ web      the workbench, embedded            │
  └─────────────────────────────────────────────┘
           ▲ HTTP + SSE
    browser tab · the window · devplane CLI
```

- **The hook writes.** `devplane hook` decides in its own process, appends the event, decision or held
  permission to SQLite, and exits. If the store will not open, the decision goes to
  `~/.devplane/pending-decisions.jsonl` for the next host to file.
- **The host folds.** `devplane up` is the one long-lived process and the only socket: it tails the
  hooks' rows, drives agents, receives telemetry, polls the roster and GitHub, serves the workbench.
  Nothing auto-starts it.
- **Read commands need no host.** Without one they replay the hooks' rows in memory, write nothing,
  and name what they cannot see (live driven sessions, GitHub, the gate probe).

## One host per home

- `~/.devplane/host.json` holds port, version, start time, binary and pid. **Alive means `/healthz`
  answers**, not that a pid exists; a record whose port is silent is cleared.
- A second host on the same home is refused with the running one's uptime.
- `devplane down` posts `/api/quit` with the token and waits for the port to go quiet; no signal.
- A second instance is a second home: `DEVPLANE_HOME=/tmp/vp devplane up --no-browser --port 47832`.

## Events and reducers

- `EventEnvelope { id, at, run_id, project_id?, source, event }`, append-only; `source` is the channel
  (hook, OpenTelemetry, roster, status line, vendor feed, driven agent) or `host`.
- Reducers are pure functions `(Run, Event) -> Run`, tested by replay.
- The inbox is derived from state, never stored.
- `/api/stream` carries `event` (state change) and `message` (driven agent output) frames; `?run=`
  narrows it.

## The purity rule

`src/core/` has no `async fn`, `.await`, runtime, database, HTTP, clock or file system; the edge
passes them in. `tests/purity.rs` fails the build on a violation.

## The store

SQLite with WAL and FTS5, one file (schema version 12), readable with `sqlite3 ~/.devplane/devplane.db`.
A schema change starts a new store
([after an upgrade](@/docs/troubleshooting.md#the-ledger-is-empty-after-an-upgrade)).

| Rows | What | Re-derivable? |
|---|---|---|
| `events`, `runs`, `projects` | observations and the runs folded from them; `events_fts` indexes them for search | largely, from the providers |
| `changes` | each change: its project, branch, worktree and agent | no |
| `messages` | what driven agents said | no; pruned with the events |
| `decisions` | the decision log. Appended, never updated, never pruned | no |
| `asks` | what an agent put to a person, and what became of it | no |
| `weakened_seen` | which weakened-check rows a person marked read | no |
| `projection`, `channel_health`, `attention_log`, `agent_capabilities`, `looks` | the tail's mark, channel latency, inbox outcomes, what each agent advertised, when you last looked | bookkeeping |

On host start, events (telemetry included), messages and their search index older than 30 days, and
finished runs older than 7 days, are pruned. Decisions and asks are kept.

## The API

HTTP + JSON on `127.0.0.1`, with Server-Sent Events. Tokens: [Security](@/docs/security.md#loopback-plus-a-bearer-token).

| Route | Method | Purpose |
|---|---|---|
| `/healthz` | GET | `ok <version>`; liveness |
| `/devplane/otel/v1/logs` | POST | Claude Code's OpenTelemetry log records |
| `/devplane/otel/v1/traces` | POST | GenAI-convention traces (Copilot, Codex) |
| `/api/board` | GET | the working set (`?all=true`: everything) |
| `/api/inbox` | GET | what needs a person |
| `/api/runs/{id}` | GET | one run |
| `/api/runs/{id}/messages` | GET | what a driven agent said |
| `/api/runs/{id}/snooze` | POST | quieten a run's inbox items |
| `/api/runs/{id}/prompt` | POST | message a driven run |
| `/api/asks` | GET | every ask and its outcome |
| `/api/asks/{id}/answer` | POST | answer a permission or a question; recorded from the `window` only with the window's proof for that ask, else `cli` |
| `/api/runs/{id}/stop` | GET | what stopping would leave |
| `/api/runs/{id}/stop` | POST | stop a driven run |
| `/api/agents` | GET | drivable agents and their capabilities |
| `/api/changes` | GET | every change with state and standing |
| `/api/changes` | POST | start a change |
| `/api/changes/preflight` | POST | what starting would do; writes nothing |
| `/api/changes/{id}` | GET | one change, its runs, tasks and drifts |
| `/api/changes/{id}/drift/accept` | POST | accept spec drift |
| `/api/changes/{id}/drift/tell` | POST | tell the run about spec drift |
| `/api/changes/{id}/verify` | POST | run the gates now |
| `/api/changes/{id}/base-checks` | POST | run the base checks |
| `/api/changes/{id}/finish` | POST | finish |
| `/api/changes/{id}/retry` | POST | one more feedback round |
| `/api/changes/{id}/snooze` | POST | quieten its inbox items |
| `/api/changes/{id}/review` | GET | the review |
| `/api/changes/{id}/review/seen` | POST | mark weakened rows read |
| `/api/changes/{id}/certificate` | GET | the done certificate |
| `/api/changes/{id}/open` | POST | open the worktree (window), else its path |
| `/api/changes/{id}/resume` | POST | reconnect to the agent's session |
| `/api/changes/adopt` | POST | make a hand-made branch a change |
| `/api/changes/{id}/archive` | POST | archive |
| `/api/changes/{id}/offer` | POST | offer |
| `/api/projects` | GET | registered projects |
| `/api/specs` | GET | specifications and requirement-to-task trace |
| `/api/projects/trust` | POST | trust a repository; the window only (the token alone is refused) |
| `/api/projects/reload` | POST | re-read trust `devplane project trust` wrote to the store |
| `/api/projects/{id}/snooze` | POST | quieten its GitHub items |
| `/api/issues` | POST | a repository's open issues, live |
| `/api/github` | GET | each host's sign-in state; never the token |
| `/api/github/login` | POST | start or return a device-flow sign-in |
| `/api/github/logout` | POST | delete a host's token |
| `/api/github/refresh` | POST | reload the token and read the forge |
| `/api/forge` | GET | the last GitHub poll, per project |
| `/api/decisions` | GET | the decision log |
| `/api/explain` | GET | what the gate would decide about one call |
| `/api/search` | GET | full-text search |
| `/api/diagnostics` | GET | what `devplane doctor` reports |
| `/api/setup` | GET | this machine and each `devplane.toml`, read back |
| `/api/modes` | GET | which sessions decide without you |
| `/api/quitting` | GET | what quitting would end |
| `/api/quit` | POST | quit the host |
| `/api/stream` | GET | Server-Sent Events |
| `/`, `/{file}` | GET | the embedded workbench |

There is no hook receiver: hooks write to the store.

## Recovery

On host start: file the spool → load projects, changes, runs → reconcile with the roster and live pids
→ mark `lost` where a process is gone → re-arm stall timers → open the API → tail catches up.

- An unreadable roster never marks a run lost.
- A mid-flight driven run becomes `interrupted`, branch and worktree untouched, with **resume** where
  supported ([Driving agents](@/docs/agents.md#resuming-after-a-restart)).

## The window

`devplane up --window` runs the host in the app's process and opens a WebView on the same URL a browser
gets. Same page, same API; no data crosses the window's bridge. It adds notifications, a tray count, a
global shortcut, `devplane://` links, and opening a worktree in an editor or terminal. Cargo feature
`app`, off by default, so the CLI carries no WebKit.

## Shutdown

`devplane down` (or Quit) says what it ends, stops answering and polling, stops each driven agent's
process group, and waits (bounded) for their endings to be written.

A killed host cannot. The next host raises each of its agents still running as a critical inbox item
with the command to end it; it does not kill them.

## Technology

| Area | Choice |
|---|---|
| Core | Rust (1.94+), Tokio, serde, tracing |
| Agents | the Agent Client Protocol Rust SDK — one client for every agent |
| HTTP | `axum` |
| Store | SQLite via `sqlx`, WAL, FTS5 |
| Telemetry | an OTLP/HTTP JSON reader |
| Git | the `git` CLI |
| GitHub | its documented GraphQL and REST APIs over `reqwest` with rustls; the token in the OS credential store via `keyring` |
| Workbench | Svelte 5 and Vite, embedded in the binary; fetches nothing from any other origin |
| Window | Tauri 2, behind the `app` feature |

`DEVPLANE_UI=/path/to/dist devplane up --no-browser` serves a built interface from disk.

## Performance

Asserted by a test or fixed in code:

- Deciding a prohibition costs under 60 ms over starting the binary at all.
- The host folds hook-written rows within 500 ms.
- The roster is polled every 2 s while anything is busy, 10 s when idle; GitHub every five minutes.
- The embedded workbench stays under 250 KB gzipped.

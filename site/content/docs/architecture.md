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
  │ the host: devplane serve · open · app       │
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

- **The hook writes.** Every hook is a `command` hook running `devplane hook`: it decides in its own
  process, appends the event, the decision or a held permission to SQLite, and exits. If the store
  will not open, the decision goes to `~/.devplane/pending-decisions.jsonl` and the next host files
  it.
- **The host folds.** `devplane serve`, `devplane open` or `devplane app` is the one long-lived
  process and the only one holding a socket. It tails what the hooks wrote, drives agents, receives
  telemetry, polls the roster and GitHub, and serves the workbench. Nothing auto-starts it.
- **Read commands need no host.** They ask a running host if one answers; otherwise they open the
  store, replay what the hooks wrote since the last projection in memory, write nothing, and name
  what they cannot see (live driven sessions, GitHub, the gate probe).

## One host per home

The host writes `~/.devplane/host.json`: port, version, start time, binary and pid. **A host is alive
when `/healthz` on that port answers**, not because a pid exists. A second `serve`, `open` or `app`
on the same home is refused with the running host's uptime; a record whose port answers nothing is
stale and cleared. `devplane quit` posts `/api/quit` with the token and waits for the port to go
quiet; it sends no signal.

A second instance is a second home: `DEVPLANE_HOME=/tmp/vp devplane serve --port 47832`.

## Events and reducers

- `EventEnvelope { id, at, run_id, project_id?, source, event }`, append-only. The source names the
  channel (a hook, OpenTelemetry, the roster, the status line, a vendor feed, a driven agent) or
  `host` for the host's own acts.
- Reducers are pure functions `(Run, Event) -> Run`, tested by replay.
- The inbox is derived from state, never stored.
- `/api/stream` carries two tagged frames: `event` for a state change and `message` for a fragment of
  what a driven agent said. `?run=` narrows it.

## The purity rule

`src/core/` may not reach the outside world: no `async fn`, no `.await`, no runtime, no database, no
HTTP, no clock and no file system. The clock and files are read at the edge and passed in.
`tests/purity.rs` fails the build on a violation.

This is what keeps state rebuildable by replay, the inbox correct after a restart, and the permission
policy unable to fail open by waiting.

## The store

SQLite with WAL and FTS5, in one file (schema version 11). A schema change moves the old file aside
as `devplane.v<n>.<time>.bak` instead of migrating it, and never deletes one that holds anything.
`sqlite3 ~/.devplane/devplane.db` opens it. `~/.devplane` is mode `0700`; the store, the token and
the other files in it are `0600`.

| Rows | What | Re-derivable? |
|---|---|---|
| `events`, `runs`, `projects` | observations and the runs folded from them; `events_fts` indexes them for search | largely, from the providers |
| `changes` | each change: its project, branch, worktree and agent | no |
| `messages` | what driven agents said | no; pruned with the events |
| `decisions` | the decision log. Appended, never updated, never pruned | no |
| `asks` | what an agent put to a person, and what became of it | no |
| `reports` | findings one project filed about another, with evidence and resolved origin | no |
| `projection`, `channel_health`, `attention_log`, `agent_capabilities`, `looks` | the tail's mark, channel latency, inbox outcomes, what each agent advertised, when you last looked | bookkeeping |

The retention sweep runs once when a host starts. Events (telemetry included) and driven agents'
messages older than 30 days are pruned, the search index with them, and finished runs older than 7
days. Decisions and asks are kept.

## The API

HTTP + JSON on `127.0.0.1`, with Server-Sent Events for live updates. Everything under `/api` needs
the bearer token from `~/.devplane/token`, in the `Authorization` header only. The telemetry routes
under `/devplane` also accept the telemetry-only token from `~/.devplane/telemetry-token`, which
opens nothing else. `/healthz` and the workbench's static files are open and carry no data.

| Route | Method | Purpose |
|---|---|---|
| `/healthz` | GET | `ok <version>`; proves the port is a Devplane host |
| `/devplane/otel/v1/logs` | POST | Claude Code's OpenTelemetry log records |
| `/devplane/otel/v1/traces` | POST | GenAI-convention traces (Copilot, Codex) |
| `/api/board` | GET | the working set (`?all=true` for everything) |
| `/api/inbox` | GET | what needs a person |
| `/api/runs/{id}` | GET | one run |
| `/api/runs/{id}/rewind-gap` | GET | files a shell command named for writing |
| `/api/runs/{id}/messages` | GET | what a driven agent said |
| `/api/runs/{id}/snooze` | POST | quieten a run's inbox items |
| `/api/runs/{id}/prompt` | POST | send a driven run a message; queued mid-turn |
| `/api/asks` | GET | everything asked, and what became of each |
| `/api/asks/{id}/answer` | POST | answer a permission or a question |
| `/api/runs/{id}/stop` | GET | what stopping would leave behind; stops nothing |
| `/api/runs/{id}/stop` | POST | stop a driven run |
| `/api/agents` | GET | the agents that can be driven, and what each advertised |
| `/api/changes` | GET | every change, with its state and gate standing |
| `/api/changes` | POST | start a change, in one project or several |
| `/api/changes/preflight` | POST | what starting would do in each project; writes nothing |
| `/api/changes/{id}` | GET | one change, its runs, tasks and drifts |
| `/api/changes/{id}/drift/accept` | POST | accept that the specification moved under a run |
| `/api/changes/{id}/drift/tell` | POST | resume the run with the changed files named |
| `/api/changes/{id}/verify` | POST | run the gates now |
| `/api/changes/{id}/finish` | POST | accept it as finished; removes nothing |
| `/api/changes/{id}/retry` | POST | one more feedback round past the bound |
| `/api/changes/{id}/snooze` | POST | quieten its inbox items |
| `/api/changes/{id}/review` | GET | the change for review: checks weakened or changed first, then files in role order |
| `/api/changes/{id}/review/seen` | POST | mark weakened-check rows read, as a person |
| `/api/changes/{id}/certificate` | GET | the done certificate |
| `/api/changes/{id}/open` | POST | open its worktree in an editor or terminal (the window); the path otherwise |
| `/api/changes/{id}/resume` | POST | reconnect to the agent's session |
| `/api/changes/adopt` | POST | make a hand-made branch a change |
| `/api/changes/{id}/archive` | POST | remove the worktree, keep the record |
| `/api/changes/{id}/offer` | POST | push and open the pull request, or answer with the commands |
| `/api/reports` | GET | reports, newest first, each with its quoted rendering |
| `/api/reports` | POST | file a report; the origin is resolved from the run |
| `/api/reports/{id}` | GET | one report |
| `/api/reports/{id}/start` | POST | start a change in the target from it |
| `/api/reports/{id}/resolve` | POST | reject, defer, mark fixed or discard |
| `/api/reports/{id}/open` | POST | open a GitHub draft under your sign-in; the only route that writes an issue |
| `/api/projects` | GET | registered projects |
| `/api/specs` | GET | every project's specifications and their requirement-to-task trace |
| `/api/projects/trust` | POST | trust a repository |
| `/api/projects/{id}/snooze` | POST | quieten a project's GitHub items |
| `/api/issues` | POST | a repository's open issues, read live from GitHub |
| `/api/github` | GET | each GitHub host's sign-in: state, login, scopes, a pending code; never the token |
| `/api/github/login` | POST | start the device-flow sign-in for a host, or return the one waiting |
| `/api/github/logout` | POST | delete a host's token; says where to revoke the grant at GitHub |
| `/api/github/refresh` | POST | a sign-in changed in another process: drop the held token, read the forge now |
| `/api/forge` | GET | what the last GitHub poll read, per project |
| `/api/decisions` | GET | the decision log |
| `/api/explain` | GET | what the gate would decide about one call |
| `/api/search` | GET | full-text search |
| `/api/diagnostics` | GET | channel health and latency, unwritten rows, leaked agents |
| `/api/setup` | GET | this machine and every project's `devplane.toml`, read back |
| `/api/attention` | GET | per inbox kind: raised, acted on, dismissed, resolved elsewhere |
| `/api/modes` | GET | which sessions decide without you |
| `/api/quitting` | GET | what quitting would end |
| `/api/quit` | POST | quit the host |
| `/api/stream` | GET | Server-Sent Events |
| `/`, `/{file}` | GET | the embedded workbench |

There is no hook receiver: hooks write to the store.

## Recovery

On host start: file what the spool holds → load projects, changes and runs → reconcile against the
roster and live process ids → mark `lost` where a recorded process is gone → re-arm stall timers →
open the API. The tail then catches up on what the hooks wrote while no host ran.

An unreadable roster never marks a run lost. A driven run that was mid-flight becomes `interrupted`,
branch and worktree untouched, with **resume** offered where the agent supports it
([Driving agents](@/docs/agents.md#resuming-after-a-restart)).

## The window

`devplane app` runs the host inside the app's own process and opens a WebView on
`http://127.0.0.1:<port>/?token=…`, the address `devplane open` gives a browser. The page takes the
token from the address once, removes it, and sends it as a header from then on. Same page, same API;
no data crosses the window's bridge. The window adds native notifications, a tray count, one global
shortcut, `devplane://` links, and opening a worktree in an editor or terminal. It is the cargo
feature `app`, off by default, so the CLI build carries no WebKit.

## Shutdown

`devplane quit` (or the window's Quit) says what quitting ends, then the host stops answering, stops
its pollers, stops every agent it drives (each agent's whole process group) and waits, bounded, for
their endings to be written. `Stopped.` prints once the port is quiet.

A killed host (`kill -9`, a crash, a power cut) cannot do this. The next host raises each agent it
had started that is still running as a critical inbox item with the command to end it; it does not
kill them for you.

## Technology

| Area | Choice |
|---|---|
| Core | Rust (1.90+), Tokio, serde, tracing |
| Agents | the Agent Client Protocol Rust SDK — one client for every agent |
| HTTP | `axum` |
| Store | SQLite via `sqlx`, WAL, FTS5 |
| Telemetry | an OTLP/HTTP JSON reader |
| Git | the `git` CLI |
| GitHub | its documented GraphQL and REST APIs over `reqwest` with rustls; the token in the OS credential store via `keyring` |
| Workbench | Svelte 5 and Vite, embedded in the binary; fetches nothing from any other origin |
| Window | Tauri 2, behind the `app` feature |

`DEVPLANE_UI=/path/to/dist devplane serve` serves a built interface from disk instead of the embedded
copy.

## Performance

Each is asserted by a test or fixed in code:

- Deciding a prohibition costs under 60 ms over starting the binary at all.
- The host folds hook-written rows within 500 ms.
- The roster is polled every 2 s while anything is busy, 10 s when idle; GitHub every five minutes.
- The embedded workbench stays under 250 KB gzipped.

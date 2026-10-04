+++
title = "Architecture"
description = "A map for contributors: one binary and one store, the hook writes, the host folds, the pure core reaches nothing outside. Processes, the store, the API routes, recovery and the window."
weight = 23
[extra]
group = "reference"
+++

## Processes

```
  Claude Code / Codex / Copilot session
        │ command hooks
        ▼
  devplane hook ── decides in its own process, answers on stdout
        │ appends events, decisions, held asks
        ▼
  ~/.devplane/devplane.db  (SQLite) ◀── read commands, with no host
        ▲ tailed every 500 ms
        │
  devplane up ── the host: the one long-lived process and the only socket
        ▲ HTTP + SSE on 127.0.0.1
  browser tab · desktop window · devplane CLI
```

- **The hook writes.** `devplane hook` decides in-process, appends to SQLite, and exits. If the store
  will not open, the decision goes to `~/.devplane/pending-decisions.jsonl` for the next host to file.
  There is no hook receiver route.
- **The host folds.** `devplane up` tails the hooks' rows, drives agents over ACP, receives telemetry,
  polls the roster and GitHub, runs gates, and serves the workbench. Nothing starts it automatically.
- **Read commands need no host.** Without one they replay the store in memory, write nothing, and say
  what they cannot see.

## One host per home

`~/.devplane/host.json` names the host's port, version, start time, binary and pid; alive means
`/healthz` answers, not that the pid exists. A second host on the same home is refused. `devplane
down` posts `/api/quit` and waits for the port to close. A second instance needs its own
`DEVPLANE_HOME` and port.

## Source map

| Path | What |
|---|---|
| `src/core/` | the pure domain: events, reducers, changes, policy, attention, review, certificate, verify. No `async`, runtime, database, HTTP, clock or file system; `tests/purity.rs` enforces it |
| `src/hook.rs`, `src/observe/` | the hook entry point; roster, OTLP, status line, OpenCode feed, `agent connect` |
| `src/acp/`, `src/driven.rs` | the Agent Client Protocol client for every driven agent |
| `src/change.rs`, `src/gates.rs`, `src/base_checks.rs`, `src/git.rs` | worktrees, gates, base checks, offers; git through its CLI |
| `src/github/` | GitHub's REST and GraphQL APIs, device-flow sign-in, reviews |
| `src/store.rs`, `src/schema.sql` | the SQLite store |
| `src/api.rs`, `src/host.rs`, `src/poller.rs` | the HTTP API, host start-up and shutdown, the tail and pollers |
| `src/cli/` | every command |
| `src/app/` | the desktop window (feature `app`) |
| `ui/` | the workbench: Svelte 5 and Vite, built into `ui/dist` and embedded in the binary |

Events are `EventEnvelope { id, at, run_id, project_id?, source, event }`, append-only. Reducers are
pure `(Run, Event) -> Run`, tested by replay. The inbox is derived from state, never stored.

## The store

SQLite with WAL and FTS5, one file, schema version 18. A schema change starts a new store and moves
the old one aside ([after an upgrade](@/docs/troubleshooting.md#the-ledger-is-empty-after-an-upgrade)).

| Tables | What | Kept |
|---|---|---|
| `events`, `events_fts`, `runs`, `projects` | observations, the runs folded from them, their search index | events 30 days, finished runs 7 days |
| `messages` | what driven agents said | 30 days |
| `changes` | each change: project, branch, worktree, agent | always |
| `decisions` | the decision log; appended, never updated | always |
| `asks` | what an agent put to a person, and what became of it | always |
| `seen`, `review_comments` | hunks a person marked seen (by content, no time); review comments | always |
| `projection`, `channel_health`, `attention_log`, `agent_capabilities`, `looks`, `announced` | the tail's mark, channel latency, inbox outcomes, what each agent advertised, when you last looked, which notifications were shown | bookkeeping |

Pruning runs once at host start.

## The API

HTTP + JSON on `127.0.0.1`, with Server-Sent Events on `/api/stream` (`event` and `message` frames;
`?run=` narrows it). Everything but `/healthz` and the static files needs a token
([Security](@/docs/security.md#loopback-plus-a-bearer-token)). Routes marked *window* need the desktop
window's proof as well.

| Route | Method | Purpose |
|---|---|---|
| `/healthz` | GET | `ok <version>`; liveness |
| `/devplane/otel/v1/logs`, `/devplane/otel/v1/traces` | POST | OpenTelemetry logs (Claude Code) and traces (Copilot, Codex) |
| `/api/board` | GET | the working set (`?all=true`: everything) |
| `/api/inbox` | GET | what needs a person |
| `/api/stream` | GET | Server-Sent Events |
| **Runs** | | |
| `/api/runs/{id}` | GET | one run |
| `/api/runs/{id}/messages` | GET | what a driven agent said |
| `/api/runs/{id}/prompt` | POST | message a driven run |
| `/api/runs/{id}/snooze` | POST | quieten a run's inbox items |
| `/api/runs/{id}/stop` | GET, POST | what stopping would leave; stop a driven run |
| `/api/runs/{id}/claim` | POST | the agent's stop claim (its spec and tests conflict) |
| `/api/agents` | GET | drivable agents and their capabilities |
| **Asks** | | |
| `/api/asks` | GET | every ask and its outcome |
| `/api/asks/{id}/answer` | POST | answer a permission or a question; recorded `window` only with the window's proof |
| `/api/asks/{id}/undo` | POST | take back a held answer |
| **Changes** | | |
| `/api/changes` | GET, POST | every change with state and standing; start one |
| `/api/changes/preflight` | POST | what starting would do; writes nothing |
| `/api/changes/adopt` | POST | make a hand-made branch a change |
| `/api/changes/draft` | POST | start a change that drafts a spec |
| `/api/changes/{id}` | GET | one change, its runs, tasks and drifts |
| `/api/changes/{id}/verify`, `/base-checks` | POST | run the gates; run the base checks |
| `/api/changes/{id}/retry`, `/resume`, `/login` | POST | one more feedback round; reconnect to the session; sign the agent in |
| `/api/changes/{id}/review` | GET | the review |
| `/api/changes/{id}/review/seen` | POST | mark a hunk, or every hunk at a path, seen |
| `/api/changes/{id}/comments` | GET, POST | review comments; add one |
| `/api/changes/{id}/comments/{cid}` | DELETE | delete an unsent comment |
| `/api/changes/{id}/comments/send` | POST | send every unsent comment as one prompt |
| `/api/changes/{id}/drift/accept`, `/drift/tell` | POST | decide spec drift |
| `/api/changes/{id}/draft/accept`, `/draft/reject` | POST | decide a draft |
| `/api/changes/{id}/clarify` | POST | answer an open-question line |
| `/api/changes/{id}/claim/read` | POST | mark the stop claim read |
| `/api/changes/{id}/certificate` | GET | the done certificate |
| `/api/changes/{id}/offer`, `/finish`, `/archive` | POST | offer, finish, archive |
| `/api/changes/{id}/snooze` | POST | quieten its inbox items |
| `/api/changes/{id}/open` | POST | open the worktree in an editor or terminal (window), else its path |
| **Projects and setup** | | |
| `/api/projects` | GET | registered projects |
| `/api/projects/trust` | POST | trust a repository (*window*) |
| `/api/projects/reload` | POST | re-read projects and `~/.devplane/trusted.toml` |
| `/api/projects/scan` | GET | what trusting a path would allow, before trust |
| `/api/projects/{id}/gates/candidates` | GET | commands the repository already has, for **Declare a check** |
| `/api/projects/{id}/gates` | POST | write `[gates] check` after confirmation |
| `/api/projects/{id}/config/open-questions` | POST | write the offered `open_questions` |
| `/api/projects/{id}/snooze` | POST | quieten its GitHub items |
| `/api/first-run` | GET | the first-run checklist |
| `/api/connect/{agent}`, `/api/disconnect/{agent}` | POST | connect or disconnect an agent's hooks |
| `/api/grants` | GET, POST | your grants; write an offered one (*window*) |
| `/api/grants/revoke` | POST | remove a grant (*window*) |
| `/api/specs` | GET | specifications and the requirement-to-task trace |
| `/api/setup` | GET | this machine and each `devplane.toml`, read back |
| **GitHub** | | |
| `/api/github` | GET | each host's sign-in state; never the token |
| `/api/github/login`, `/logout`, `/refresh` | POST | device-flow sign-in; delete a token; reload and re-read |
| `/api/forge` | GET | the last GitHub poll, per project |
| `/api/issues` | POST | a repository's open issues, live |
| **Record and diagnostics** | | |
| `/api/decisions` | GET | the decision log |
| `/api/decisions/unattributed` | GET | calls a vendor reported deciding that no authority can be named for |
| `/api/explain` | GET | what the gate would decide about one call |
| `/api/search` | GET | full-text search |
| `/api/diagnostics` | GET | what `devplane doctor` reports |
| `/api/modes` | GET | which sessions decide without you |
| **The app** | | |
| `/api/app/update`, `/api/app/updates` | GET, POST | update-check state; your answer to *may it check* |
| `/api/app/update/install`, `/dismiss` | POST | install and restart; dismiss an offer |
| `/api/quitting`, `/api/quit` | GET, POST | what quitting would end; quit the host |
| `/`, `/{file}` | GET | the embedded workbench |

## Recovery and shutdown

On host start: file the spool, load projects, changes and runs, reconcile with the roster and
live pids, mark `lost` where a process is gone, re-arm stall timers, open the API, and let the tail
catch up. An unreadable roster never marks a run lost. A driven run caught mid-flight becomes
`interrupted`, branch and worktree untouched, with **resume** where the agent supports it
([Driving agents](@/docs/agents.md#resuming-after-a-restart)).

`devplane down` (or Quit) says what it ends, stops answering and polling, stops each driven agent's
process group, and waits, bounded, for their endings to be written. A killed host cannot: the next
host raises each of its agents still running as a critical inbox item with the command to end it.

## The window

`devplane up --window` runs the host in the app's process and opens a WebView on the same URL a
browser gets: same page, same API, no data over the window's bridge. It adds notifications, a tray
count, a global shortcut, `devplane://` links, opening a worktree in an editor or terminal, and the
proof the *window* routes need. Tauri 2, behind the Cargo feature `app`, off by default, so the
command line links no WebKit.

`DEVPLANE_UI=/path/to/dist devplane up --no-browser` serves a built interface from disk.

## Technology

| Area | Choice |
|---|---|
| Core | Rust 1.94+, Tokio, serde, tracing |
| Agents | the Agent Client Protocol Rust SDK |
| HTTP | `axum` |
| Store | SQLite via `sqlx`, WAL, FTS5 |
| Telemetry | an OTLP/HTTP JSON reader |
| GitHub | REST and GraphQL over `reqwest` with rustls; the token in the OS credential store via `keyring` |
| Workbench | Svelte 5 and Vite; fetches nothing from any other origin |
| Window | Tauri 2 |

## Performance

Asserted by a test or fixed in code:

- Deciding a prohibition costs under 60 ms over starting the binary at all.
- The host folds hook-written rows within 500 ms.
- The roster is polled every 2 s while anything is busy, 10 s when idle; GitHub every five minutes.
- The embedded workbench stays under 250 KB gzipped.

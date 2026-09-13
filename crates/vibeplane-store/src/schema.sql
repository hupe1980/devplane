-- Vibeplane's observation store.
--
-- This holds observations only: events we received, and the run and project
-- rows derived from them. Everything here is rebuildable from the providers,
-- which is why it can be deleted without losing anything that matters, and why
-- the schema changes without a migration while the project is unreleased.
--
-- Facts Vibeplane *causes* — effects, approvals, decisions — do not live here.
-- They belong in the runtime journal, which is append-only and hash-chained.


CREATE TABLE IF NOT EXISTS projects (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    root         TEXT NOT NULL,
    trusted      INTEGER NOT NULL DEFAULT 0,
    repo_url     TEXT,
    auto_discovered INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL
);

-- Runs are a projection of the event log, stored so that the board is on
-- screen before a replay finishes rather than after it.
CREATE TABLE IF NOT EXISTS runs (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL,
    project_id   TEXT REFERENCES projects(id) ON DELETE SET NULL,
    agent        TEXT NOT NULL,
    mode         TEXT NOT NULL,
    state        TEXT NOT NULL,
    cwd          TEXT NOT NULL,
    worktree     TEXT,
    branch       TEXT,
    model        TEXT,
    entrypoint   TEXT,
    name         TEXT,
    pid          INTEGER,
    started_at   TEXT NOT NULL,
    last_event_at TEXT NOT NULL,
    -- The whole Run struct, so the board survives a schema the projection
    -- columns do not cover. The columns above exist for indexing and for
    -- queries a human runs by hand.
    payload      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS runs_by_activity ON runs(last_event_at DESC);
CREATE INDEX IF NOT EXISTS runs_by_project ON runs(project_id, last_event_at DESC);
CREATE INDEX IF NOT EXISTS runs_by_state ON runs(state);

CREATE TABLE IF NOT EXISTS events (
    id           TEXT PRIMARY KEY,
    at           TEXT NOT NULL,
    run_id       TEXT NOT NULL,
    project_id   TEXT,
    source       TEXT NOT NULL,
    kind         TEXT NOT NULL,
    payload      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS events_by_run ON events(run_id, id);
CREATE INDEX IF NOT EXISTS events_by_time ON events(at DESC);
CREATE INDEX IF NOT EXISTS events_by_kind ON events(kind, at DESC);

-- Full-text search over what a human would search for: the tool commands, the
-- questions, the summaries. Prompt and response text is deliberately absent —
-- Vibeplane never turns on the telemetry flags that would carry it.
CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
    text,
    run_id UNINDEXED,
    event_id UNINDEXED,
    tokenize = "unicode61"
);

-- Latency and liveness per observation channel, so diagnostics can answer
-- "is telemetry arriving?" without guessing from the absence of events.
CREATE TABLE IF NOT EXISTS channel_health (
    channel      TEXT PRIMARY KEY,
    last_seen_at TEXT NOT NULL,
    count        INTEGER NOT NULL DEFAULT 0,
    p99_micros   INTEGER NOT NULL DEFAULT 0,
    last_error   TEXT
);

-- Devplane's observation store.
--
-- This holds observations only: events we received, and the run and project
-- rows derived from them. Everything here is rebuildable from the providers,
-- which is why it can be deleted without losing anything that matters, and why
-- the schema changes without a migration while the project is unreleased.
--
-- Facts Devplane *causes* live here too, in `decisions`, and are the one table
-- that is append-only and never pruned: an observation can be re-derived from
-- the provider, a decision cannot be re-derived from anything.


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
-- Devplane never turns on the telemetry flags that would carry it.
CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
    text,
    run_id UNINDEXED,
    event_id UNINDEXED,
    tokenize = "unicode61"
);

-- Latency and liveness per observation channel, so diagnostics can answer
-- "is telemetry arriving?" without guessing from the absence of events.
-- `worst_micros` is a maximum, and is named one. It was `p99_micros`, which
-- keeping a real percentile would need a histogram per channel for — and the
-- number is only ever read by a person looking for a stall, who wants the worst
-- case rather than the ninety-ninth.
--
-- `last_error` is kept after the problem is fixed, because "this channel has
-- had a problem" is worth knowing — but it is kept **with its timestamp**,
-- because an error with no date on it reads as current, and a diagnostic that
-- cries wolf is one nobody reads twice.
CREATE TABLE IF NOT EXISTS channel_health (
    channel       TEXT PRIMARY KEY,
    last_seen_at  TEXT NOT NULL,
    count         INTEGER NOT NULL DEFAULT 0,
    worst_micros  INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    last_error_at TEXT
);

-- Work: the durable unit. Outlives the sessions that do it, which is the whole
-- reason it exists as a row rather than living in a run.
CREATE TABLE IF NOT EXISTS works (
    id           TEXT PRIMARY KEY,
    project_id   TEXT,
    kind         TEXT NOT NULL,
    phase        TEXT NOT NULL,
    title        TEXT NOT NULL,
    worktree     TEXT,
    branch       TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    payload      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS works_by_phase ON works(phase, updated_at DESC);
CREATE INDEX IF NOT EXISTS works_by_project ON works(project_id, updated_at DESC);

-- What Devplane decided, and on whose authority.
--
-- Append-only, and deliberately outside the retention sweep: the event log is
-- history you can lose, this is the answer to "why did that happen" months
-- later. Deleting it would leave a pull request nobody can account for.
CREATE TABLE IF NOT EXISTS decisions (
    id           TEXT PRIMARY KEY,
    at           TEXT NOT NULL,
    actor        TEXT NOT NULL,
    action       TEXT NOT NULL,
    subject      TEXT NOT NULL,
    outcome      TEXT NOT NULL,
    reason       TEXT,
    -- The tool the call was, where there was one. Recorded rather than inferred
    -- from `subject`, because the question `devplane rewind` asks -- which of
    -- these wrote a file through a *shell* -- is answerable exactly from the
    -- tool name and only guessable from the text.
    tool         TEXT,
    project_id   TEXT,
    run_id       TEXT,
    work_id      TEXT
);

CREATE INDEX IF NOT EXISTS decisions_by_time ON decisions(at DESC);
CREATE INDEX IF NOT EXISTS decisions_by_run ON decisions(run_id, at DESC);
CREATE INDEX IF NOT EXISTS decisions_by_work ON decisions(work_id, at DESC);

-- What a driven agent said.
--
-- Its own table, not the event log. Run state is a pure reduction over
-- `events`, and a sentence reduces to nothing: put here it would be most of
-- the rows, would slow every replay, and would make "the log" mean two things.
-- Pruned on the same sweep, because a transcript is history like any other
-- observation — unlike `decisions`, which is neither.
CREATE TABLE IF NOT EXISTS messages (
    id           TEXT PRIMARY KEY,
    run_id       TEXT NOT NULL,
    at           TEXT NOT NULL,
    role         TEXT NOT NULL,
    text         TEXT NOT NULL
);

-- Uuid v7 ids sort by time, so this index is both "one run's transcript" and
-- "in order".
CREATE INDEX IF NOT EXISTS messages_by_run ON messages(run_id, id);
-- The retention sweep deletes by age across every run at once, which is a full
-- scan without this.
CREATE INDEX IF NOT EXISTS messages_by_time ON messages(at);

-- What the inbox asked for, and what became of it.
--
-- The product is a filter and this is the only thing that measures it. One row
-- per raise: `resolved_at` NULL means the item is still asking. The resolution
-- is written by whoever closes it — the API when a person uses one of the
-- item's own actions, the sweeper when it simply went away — and never
-- overwritten, so an explicit `acted` always beats the sweeper that follows it.
--
-- An observation, not a decision: it records what Devplane *showed*, not what
-- it did. It is pruned with the event log.
CREATE TABLE IF NOT EXISTS attention_log (
    rowid_       INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id      TEXT NOT NULL,
    kind         TEXT NOT NULL,
    level        TEXT NOT NULL,
    project_id   TEXT,
    run_id       TEXT,
    work_id      TEXT,
    raised_at    TEXT NOT NULL,
    resolved_at  TEXT,
    resolution   TEXT
);

-- The open set, which the sweeper reads on every tick.
CREATE INDEX IF NOT EXISTS attention_open ON attention_log(item_id) WHERE resolved_at IS NULL;
CREATE INDEX IF NOT EXISTS attention_by_raised ON attention_log(raised_at DESC);

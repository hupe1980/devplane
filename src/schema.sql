-- Devplane's store: what it observed, and what it was told.
--
-- Rebuildable: `runs`, `events`, `events_fts`, `messages`, `channel_health`,
-- `attention_log`, and `projects` except its `trusted` flag. Each is an
-- observation or a projection of one; losing it costs history.
--
-- The record: `changes`, `asks`, `decisions`, `agent_capabilities`, `looks`,
-- `reports`, and the `trusted` flag. Nothing can say these again, so the
-- schema is versioned and an old file is moved aside rather than dropped.
--
-- `events` and `decisions` are append-only; tables updated in place say how.


CREATE TABLE IF NOT EXISTS projects (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    root         TEXT NOT NULL,
    trusted      INTEGER NOT NULL DEFAULT 0,
    repo_url     TEXT,
    auto_discovered INTEGER NOT NULL DEFAULT 1
);

-- A projection of the event log, stored so the board renders before a replay
-- finishes. `payload` is the whole Run; the other columns are the ones a query
-- reads (probe cleanup, rule scope, inbox vendors, order and retention).
CREATE TABLE IF NOT EXISTS runs (
    id           TEXT PRIMARY KEY,
    agent        TEXT NOT NULL,
    cwd          TEXT NOT NULL,
    last_event_at TEXT NOT NULL,
    payload      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS runs_by_activity ON runs(last_event_at DESC);

-- `seq` is the append order the projection tails. AUTOINCREMENT keeps it from
-- being reused after retention empties the table, which would otherwise hide
-- every later event behind the projection mark.
CREATE TABLE IF NOT EXISTS events (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    id           TEXT NOT NULL UNIQUE,
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

-- Full-text search over tool commands, questions and summaries. Prompt and
-- response text is absent: Devplane never enables the telemetry that carries it.
CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
    text,
    run_id UNINDEXED,
    event_id UNINDEXED,
    tokenize = "unicode61"
);

-- Latency and liveness per observation channel. `worst_micros` is a maximum.
-- `last_error` survives the fix but keeps its timestamp, so an old error does
-- not read as current.
CREATE TABLE IF NOT EXISTS channel_health (
    channel       TEXT PRIMARY KEY,
    last_seen_at  TEXT NOT NULL,
    count         INTEGER NOT NULL DEFAULT 0,
    worst_micros  INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    last_error_at TEXT
);

-- The durable unit of work; it outlives the sessions that do it. `payload` is
-- the struct and `updated_at` the only column read beside it, for order.
CREATE TABLE IF NOT EXISTS changes (
    id           TEXT PRIMARY KEY,
    updated_at   TEXT NOT NULL,
    payload      TEXT NOT NULL
);

-- What Devplane decided, and on whose authority. Append-only and outside the
-- retention sweep: it answers "why did that happen" months later.
CREATE TABLE IF NOT EXISTS decisions (
    id           TEXT PRIMARY KEY,
    at           TEXT NOT NULL,
    -- `person`, `rule`, `timer`, `nobody` or `devplane`. There is no
    -- `unknown`: a row whose authority cannot be established is not written.
    authority    TEXT NOT NULL,
    action       TEXT NOT NULL,
    subject      TEXT NOT NULL,
    outcome      TEXT NOT NULL,
    reason       TEXT,
    -- The tool the call was, recorded rather than inferred from `subject` so
    -- `devplane rewind` can tell exactly which calls wrote through a shell.
    tool         TEXT,
    -- Where the MCP server behind `tool` came from, as the vendor reported it
    -- (`plugin`, `sdk`, `user`, `project`, or an unseen value stored as is).
    -- `authority` says who decided a call; this says who installed what acted.
    -- NULL when the tool is not MCP or the vendor does not report it.
    server_source TEXT,
    project_id   TEXT,
    run_id       TEXT,
    change_id      TEXT
);

-- Ties on `at` are common (a gate finishing as its run ends), so readers
-- order by `at` then `rowid`, the append order. The uuid v7 id cannot break
-- ties: its tail within a millisecond is random.
CREATE INDEX IF NOT EXISTS decisions_by_time ON decisions(at DESC);
CREATE INDEX IF NOT EXISTS decisions_by_run ON decisions(run_id, at DESC);
CREATE INDEX IF NOT EXISTS decisions_by_change ON decisions(change_id, at DESC);

-- What a driven agent said. Kept out of `events` because run state is a
-- reduction over events and a sentence reduces to nothing. Pruned with the
-- event log, unlike `decisions`.
CREATE TABLE IF NOT EXISTS messages (
    id           TEXT PRIMARY KEY,
    run_id       TEXT NOT NULL,
    at           TEXT NOT NULL,
    role         TEXT NOT NULL,
    text         TEXT NOT NULL
);

-- Uuid v7 ids sort by time: one run's transcript, in order.
CREATE INDEX IF NOT EXISTS messages_by_run ON messages(run_id, id);
-- The retention sweep deletes by age across every run.
CREATE INDEX IF NOT EXISTS messages_by_time ON messages(at);

-- What the inbox raised, and what became of it: one row per raise,
-- `resolved_at` NULL while still asking. Updated in place at most twice, on
-- resolve and on first fold, each guarded on the column still being NULL, so
-- an explicit `acted` is never overwritten by the sweeper. Pruned with events.
CREATE TABLE IF NOT EXISTS attention_log (
    item_id      TEXT NOT NULL,
    kind         TEXT NOT NULL,
    run_id       TEXT,
    raised_at    TEXT NOT NULL,
    resolved_at  TEXT,
    resolution   TEXT,
    -- When the item was first folded into a summary, stamped only when the
    -- inbox is actually read (the board polls). Beside `resolution` it shows
    -- which kinds are folded and then acted on, i.e. folded wrongly.
    folded_at    TEXT
);

-- The open set, which the sweeper reads on every tick.
CREATE INDEX IF NOT EXISTS attention_open ON attention_log(item_id) WHERE resolved_at IS NULL;
CREATE INDEX IF NOT EXISTS attention_by_raised ON attention_log(raised_at DESC);

-- What an agent asked a person, outliving the process that asked it.
--
-- `id` is an opaque token answers are addressed to from any surface; the
-- protocol's `request_id` is kept but only means something while its
-- connection lives. `deadline_secs` NULL means it waits. `answer` is written
-- before delivery, so a crash in between replays as answered. Rewritten whole
-- on every state change (`INSERT OR REPLACE`).
CREATE TABLE IF NOT EXISTS asks (
    id             TEXT PRIMARY KEY,
    kind           TEXT NOT NULL,        -- permission | question
    run_id         TEXT NOT NULL,
    project_id     TEXT,
    request_id     TEXT NOT NULL,
    message        TEXT NOT NULL,
    payload        TEXT NOT NULL,
    asked_at       TEXT NOT NULL,
    deadline_secs  INTEGER,
    answer         TEXT,
    answered_at    TEXT,
    answered_from  TEXT,
    delivery       TEXT,
    ended          TEXT,
    ended_at       TEXT
);

-- The open set, which the inbox and the deadline sweep both read.
CREATE INDEX IF NOT EXISTS asks_open ON asks(asked_at) WHERE ended IS NULL;

-- What each agent advertised at `initialize` the last time Devplane ran it: a
-- measurement, never a claim. No row means not probed, not unsupported. Keyed
-- by command, since that is what ran and a registry rename is not a new agent.
CREATE TABLE IF NOT EXISTS agent_capabilities (
    command        TEXT PRIMARY KEY,
    agent_name     TEXT,
    resume         INTEGER NOT NULL,
    load_session   INTEGER NOT NULL,
    list_sessions  INTEGER NOT NULL,
    declares_modes INTEGER NOT NULL,
    needs_auth     INTEGER NOT NULL,
    measured_at    TEXT NOT NULL
);

-- When somebody last read the inbox on this machine. One row, overwritten.
-- Advanced only after a render was left alone long enough to be read, not on
-- every board repaint, which would erase the boundary it marks. Per machine.
CREATE TABLE IF NOT EXISTS looks (
    id        INTEGER PRIMARY KEY CHECK (id = 1),
    at        TEXT NOT NULL
);

-- How far the host has folded events other processes wrote into `runs`.
-- Hooks and shims append and exit, so `runs` has one writer: the host applies
-- events past this mark and moves it. Without a host, readers replay the same
-- rows in memory and leave the mark alone.
CREATE TABLE IF NOT EXISTS projection (
    id        INTEGER PRIMARY KEY CHECK (id = 1),
    through   INTEGER NOT NULL
);

-- A finding one project filed about another. `payload` is the whole report,
-- with provenance the host resolved from a run. `target_project` is NULL for
-- a GitHub draft or a report kept on its own change. Rewritten whole on every
-- state change; the resolution itself is a decision on the source change.
CREATE TABLE IF NOT EXISTS reports (
    id             TEXT PRIMARY KEY,     -- rp-<uuid7>
    filed_at       TEXT NOT NULL,
    source_project TEXT NOT NULL,
    target_project TEXT,
    state          TEXT NOT NULL,        -- open | accepted | fixed | rejected | deferred | drafted | opened | discarded
    payload        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS reports_by_target ON reports(target_project, state);
CREATE INDEX IF NOT EXISTS reports_by_source ON reports(source_project, state);

-- A person read a weakened-check row of a change's review: that the row was
-- seen, and by whom, never when — a time beside the change's ready time would
-- record how long somebody spent reviewing. Keyed by the row's matched text,
-- so a different skip marker is a different, unseen row. Guards the offer.
CREATE TABLE IF NOT EXISTS weakened_seen (
    change_id  TEXT NOT NULL,
    path       TEXT NOT NULL,
    matched    TEXT NOT NULL,
    authority  TEXT NOT NULL CHECK (authority = 'person'),
    PRIMARY KEY (change_id, path, matched)
);

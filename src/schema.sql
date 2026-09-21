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
    -- The fan-out this work belongs to, when it belongs to one. A member is a
    -- work row, not a separate entity.
    batch_id     TEXT,
    payload      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS works_by_phase ON works(phase, updated_at DESC);
CREATE INDEX IF NOT EXISTS works_by_project ON works(project_id, updated_at DESC);
-- Members of a fan-out. One index, because "show me this batch" is the only
-- question asked of it.
CREATE INDEX IF NOT EXISTS works_by_batch ON works(batch_id) WHERE batch_id IS NOT NULL;

-- A fan-out: one person's intent, sent once, to many targets.
--
-- `kind` is `drafted` or `dispatched` and the distinction is load-bearing. A
-- drafted batch records the composition and never expects members; a dispatched
-- one has one member per accepted target. Modelling a draft as "a dispatched
-- batch whose members have not arrived" would make a draft nobody sent
-- indistinguishable from six runs that failed to start.
--
-- `targets` holds **every** project chosen, including the ones the preflight
-- refused, because a record that dropped them would answer "what did I send
-- this to" with the subset that happened to work.
--
-- What is deliberately absent: whether the person went on to send a draft. The
-- vendor's window is the vendor's, and inferring a send from a later session is
-- manufactured attribution.
CREATE TABLE IF NOT EXISTS batches (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    position     TEXT NOT NULL,
    prompt       TEXT NOT NULL,
    template     TEXT,
    sent_at      TEXT NOT NULL,
    sent_by      TEXT NOT NULL,
    -- The findings, as JSON: one row per target with its refusal and the time
    -- the preflight was computed. A snapshot, not a promise.
    targets      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS batches_by_time ON batches(sent_at DESC);

-- What Devplane decided, and on whose authority.
--
-- Append-only, and deliberately outside the retention sweep: the event log is
-- history you can lose, this is the answer to "why did that happen" months
-- later. Deleting it would leave a pull request nobody can account for.
CREATE TABLE IF NOT EXISTS decisions (
    id           TEXT PRIMARY KEY,
    at           TEXT NOT NULL,
    -- **On whose authority**: `person`, `rule`, `timer`, `nobody`, `daemon`.
    --
    -- This column was `actor` with three values until 2026-09-19, and `daemon`
    -- carried three unrelated meanings: Devplane ran a gate, a clock refused a
    -- call nobody answered, and a question died unanswered when its run ended.
    -- The reason text told them apart and the queryable column did not — in the
    -- one table whose whole purpose is being queried by this dimension.
    --
    -- `classifier` is absent because nothing here can attribute an individual
    -- call to a model's approval, and `unknown` is absent because a row whose
    -- authority cannot be established is a row that must not be written.
    authority    TEXT NOT NULL,
    action       TEXT NOT NULL,
    subject      TEXT NOT NULL,
    outcome      TEXT NOT NULL,
    reason       TEXT,
    -- The tool the call was, where there was one. Recorded rather than inferred
    -- from `subject`, because the question `devplane rewind` asks -- which of
    -- these wrote a file through a *shell* -- is answerable exactly from the
    -- tool name and only guessable from the text.
    tool         TEXT,
    -- Where the MCP server behind `tool` came from, as the vendor reported it:
    -- `plugin`, `sdk`, `user`, `project`, or a value this build has never seen,
    -- stored as received.
    --
    -- **The adjacent column to `authority`.** That one says on whose authority
    -- a call was decided; this says what was acting and who put it there. A
    -- call into a server a cloned repository defined and one into a server the
    -- person installed themselves produced identical rows until this existed.
    --
    -- NULL for a tool that is not an MCP tool, for a vendor that reports no
    -- such thing, and for an agent older than the release that began sending
    -- it. Absent is not unknown, and no value here means unknown.
    server_source TEXT,
    project_id   TEXT,
    run_id       TEXT,
    work_id      TEXT
);

-- **Ordered by time and then by insertion**, because `at` ties.
--
-- Two decisions taken in the same second — a gate finishing and the run it was
-- about ending, which is the common case rather than a corner — sort
-- arbitrarily under `at` alone, and SQLite is free to return them in either
-- order. That surfaced as a test that passed alone and failed under parallel
-- load, which is the mild version; the real one is an audit page showing two
-- decisions in the wrong sequence, in the table whose whole purpose is saying
-- what happened in what order.
--
-- `rowid` is the append order of an append-only log, so it is the exact
-- tie-break. The row id cannot serve: it is a uuid v7, whose head is a
-- millisecond timestamp and whose tail is random, so two rows written in the
-- same millisecond sort by the random part.
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
    resolution   TEXT,
    -- When this item was first **folded** into a summary rather than listed.
    --
    -- Per item, not per render: the board polls every couple of seconds, so
    -- counting folds per render would measure polling. Stamped only when
    -- somebody actually reads the inbox, exactly like the `looks` mark.
    --
    -- What it is for: a kind that is always folded is one nobody needed, and a
    -- kind that is folded and then **acted on** once opened is one that is
    -- being folded wrongly. The second is the interesting number and it is only
    -- answerable with this column beside `resolution`.
    folded_at    TEXT
);

-- The open set, which the sweeper reads on every tick.
CREATE INDEX IF NOT EXISTS attention_open ON attention_log(item_id) WHERE resolved_at IS NULL;
CREATE INDEX IF NOT EXISTS attention_by_raised ON attention_log(raised_at DESC);

-- What an agent asked a person, as a row that outlives the process that asked
-- it.
--
-- **The one table here that is neither a pure observation nor a decision.** An
-- observation can be re-derived from the provider and a decision cannot be
-- re-derived from anything; an ask is a decision *waiting to be taken*, and
-- losing one is losing the question itself — which is what happened before this
-- table existed: the question lived in a map on a live connection, so a daemon
-- restart took the run, the agent and the question together and the inbox said
-- "Nothing needs you".
--
-- `id` is an **opaque token** and is what an answer is addressed to, from any
-- surface, however long afterwards. The protocol's own `request_id` is kept
-- beside it and is deliberately not the key: it only means anything while the
-- connection that carried it is alive.
--
-- `deadline_secs` is NULL by default and NULL means **it waits**. That is what
-- the agent's own vendor does — permission prompts "never auto-resolve on
-- idle" — and a product whose argument is that vendors end your questions on
-- clocks you did not set may not ship one.
--
-- `answer` is written **before** anything is delivered, so a crash between
-- answered and delivered replays as answered rather than as ask-again.
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
CREATE INDEX IF NOT EXISTS asks_by_run ON asks(run_id, asked_at DESC);

-- What each agent said it could do, the last time Devplane spoke to it.
--
-- **A measurement, never a claim.** Support for every session capability is
-- advertised per agent at `initialize`, so it is a runtime fact about that
-- agent at that version — not a property of this product, and not something the
-- schema can assert. An agent Devplane has never started has **no row here**,
-- and that is reported as *not probed* rather than as *not supported*: the two
-- are different facts and conflating them is how a table starts lying about a
-- field nobody has looked at.
--
-- Keyed by the command rather than by a friendly name, because that is what was
-- actually run — two registry entries can point at the same binary, and a
-- rename must not read as a new agent.
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

-- When somebody last **read** the inbox on this machine. One row, overwritten.
--
-- **Read, not rendered**, and the distinction is the whole of why this table
-- exists rather than a timestamp on a request. The board repaints every couple
-- of seconds; advancing the mark on every paint would erase the boundary it is
-- drawn to show, and the failure would be invisible — the hairline would simply
-- always say `0m`. The daemon advances it only after a render has been asked
-- for and then left alone long enough to have been read.
--
-- Per machine, because a person has one attention span: reading the board and
-- then the CLI is one look, not two. Not synchronised anywhere, so somebody
-- with two laptops has two boundaries and each is honest about its own.
CREATE TABLE IF NOT EXISTS looks (
    id        INTEGER PRIMARY KEY CHECK (id = 1),
    at        TEXT NOT NULL
);

//! The observation store: SQLite, WAL, rebuildable.
//!
//! Two rules shape everything here:
//!
//! * **Observations only.** Events, and the run and project rows derived from
//!   them. Anything Vibeplane causes belongs in the runtime journal instead, so
//!   there is never a question about which store owns a fact.
//! * **Rebuildable.** Losing this database costs history, not correctness: the
//!   providers are still the source of truth for what is running.

use crate::core::event::EventEnvelope;
use crate::core::ids::{ProjectId, RunId};
use crate::core::project::Project;
use crate::core::run::Run;
use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// A handle on the observation store.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens the store, creating it if needed.
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        // `filename`, not a `sqlite://…` URL built by string formatting: a home
        // directory with a `?` or a `#` in it turned the rest of the path into
        // query parameters and opened a database somewhere else entirely.
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await
            .with_context(|| format!("opening {}", path.display()))?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// An in-memory store, for tests.
    ///
    /// Configured exactly like the real one — foreign keys included. A test
    /// database with the constraints turned off proves nothing about the
    /// database the product ships.
    pub async fn open_in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Applies the schema. There is no migration machinery on purpose: almost
    /// everything here is a projection of the event log, and a row this build
    /// cannot decode is dropped and counted (`unreadable rows`) rather than
    /// migrated.
    ///
    /// **`decisions` is the exception, because it cannot be rebuilt.**
    /// `CREATE TABLE IF NOT EXISTS` leaves an existing table alone, so a column
    /// added to the file never reaches a database somebody already has and the
    /// first write naming it fails. Additive columns for that one table are
    /// applied explicitly below; SQLite has no `ADD COLUMN IF NOT EXISTS`, so a
    /// duplicate-column error is the success case.
    async fn migrate(&self) -> Result<()> {
        // `raw_sql` runs the whole file, comments and all, in one round trip.
        // Hand-splitting on `;` is how a schema loses the statement that a
        // comment happens to sit above.
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await
            .context("applying schema")?;
        // Columns added to `decisions` after a release. Ignored when already
        // present, which is what makes this safe to run on every open.
        for statement in ["ALTER TABLE decisions ADD COLUMN tool TEXT"] {
            let _ = sqlx::raw_sql(statement).execute(&self.pool).await;
        }
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Appends an event and indexes anything searchable in it.
    ///
    /// Both writes in one transaction, for the same reason the retention sweep
    /// is one: the index and the thing it indexes are two tables that have to
    /// agree, and a crash between two separate statements leaves an event that
    /// search can never find. The sweep was already transactional and this was
    /// not, which is the half of the invariant nobody had looked at.
    pub async fn append_event(&self, env: &EventEnvelope) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT OR IGNORE INTO events (id, at, run_id, project_id, source, kind, payload)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&env.id)
        .bind(env.at.to_string())
        .bind(env.run_id.as_str())
        .bind(env.project_id.as_ref().map(|p| p.as_str()))
        .bind(env.source.as_str())
        .bind(env.event.label())
        .bind(serde_json::to_string(&env.event)?)
        .execute(&mut *tx)
        .await?;

        if let Some(text) = searchable_text(&env.event) {
            // Indexed only when the event itself was new. The insert above
            // ignores a duplicate id; an unconditional index row beside it
            // meant a replayed event was findable twice.
            sqlx::query(
                "INSERT INTO events_fts (text, run_id, event_id)
                 SELECT ?, ?, ? WHERE NOT EXISTS
                   (SELECT 1 FROM events_fts WHERE event_id = ?)",
            )
            .bind(text)
            .bind(env.run_id.as_str())
            .bind(&env.id)
            .bind(&env.id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Every event for a run, oldest first. Uuid v7 ids sort by time, so this
    /// is the replay order without a join on the timestamp.
    pub async fn events_for_run(&self, run: &RunId, limit: i64) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            "SELECT id, at, run_id, project_id, source, payload FROM events
             WHERE run_id = ? ORDER BY id ASC LIMIT ?",
        )
        .bind(run.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_event).collect()
    }

    /// Every tool call this machine has observed, newest first, with the
    /// directory the session was working in.
    ///
    /// The directory is what decides whose rules apply, so it comes from the
    /// run rather than from the event. A run whose row has been pruned is
    /// dropped rather than evaluated against the wrong project's rules.
    ///
    /// Only the tools a path or command rule can speak for; a replay that
    /// counted `TodoWrite` would report a coverage figure nobody can act on.
    /// `under` scopes the query to one directory tree. It is applied **in the
    /// query** rather than after it, because a limit that runs first would
    /// return the most recent calls on the whole machine and then throw most of
    /// them away — so a person asking about one project on a busy laptop would
    /// be told they have no history.
    pub async fn observed_tool_calls(
        &self,
        under: Option<&Path>,
        limit: i64,
    ) -> Result<Vec<ObservedCall>> {
        // `LIKE` with the tree as a prefix. The escape keeps a `%` or `_` in a
        // real directory name from turning into a wildcard.
        let prefix = under.map(|p| {
            let mut t = p.to_string_lossy().into_owned();
            t = t
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            if !t.ends_with('/') {
                t.push('/');
            }
            format!("{t}%")
        });
        let rows = sqlx::query(
            "SELECT e.payload AS payload, r.cwd AS cwd
             FROM events e JOIN runs r ON r.id = e.run_id
             WHERE e.kind = 'tool_started'
               AND (?1 IS NULL OR r.cwd = ?2 OR r.cwd LIKE ?1 ESCAPE '\\')
             ORDER BY e.at DESC LIMIT ?3",
        )
        .bind(prefix)
        .bind(under.map(|p| p.to_string_lossy().into_owned()))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            let payload: serde_json::Value = serde_json::from_str(&r.get::<String, _>("payload"))?;
            let Some(tool) = payload.get("tool").and_then(|t| t.as_str()) else {
                continue;
            };
            if crate::core::policy::rule_content_field(tool).is_none() {
                continue;
            }
            out.push(ObservedCall {
                cwd: PathBuf::from(r.get::<String, _>("cwd")),
                tool: tool.to_string(),
                input: payload
                    .get("input")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            });
        }
        Ok(out)
    }

    /// Full-text search across tool commands, questions and summaries.
    ///
    /// What the user typed is a phrase, not an FTS5 expression. Passing it
    /// through raw meant `vibeplane search "a-b"` or a query containing a quote
    /// was a syntax error the user could not have predicted and the daemon
    /// reported as a 500.
    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<(RunId, String)>> {
        let phrase = fts_phrase(query);
        if phrase.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT run_id, text FROM events_fts WHERE events_fts MATCH ? ORDER BY rank LIMIT ?",
        )
        .bind(phrase)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (RunId::new(r.get::<String, _>("run_id")), r.get("text")))
            .collect())
    }

    /// Writes the run projection. Called after every applied event, which is
    /// cheap because it is one upsert of one row.
    pub async fn save_run(&self, run: &Run) -> Result<()> {
        sqlx::query(
            "INSERT INTO runs (id, session_id, project_id, agent, mode, state, cwd, worktree,
                               branch, model, entrypoint, name, pid, started_at, last_event_at, payload)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
               project_id=excluded.project_id, agent=excluded.agent, mode=excluded.mode,
               state=excluded.state, cwd=excluded.cwd, worktree=excluded.worktree,
               branch=excluded.branch, model=excluded.model, entrypoint=excluded.entrypoint,
               name=excluded.name, pid=excluded.pid, last_event_at=excluded.last_event_at,
               payload=excluded.payload",
        )
        .bind(run.id.as_str())
        .bind(run.session_id.as_str())
        .bind(run.project_id.as_ref().map(|p| p.as_str()))
        .bind(&run.agent)
        .bind(run.mode.as_str())
        .bind(run.state.as_str())
        .bind(run.cwd.to_string_lossy().to_string())
        .bind(run.worktree.as_ref().map(|p| p.to_string_lossy().to_string()))
        .bind(run.branch.as_deref())
        .bind(run.model.as_deref())
        .bind(run.entrypoint.as_deref())
        .bind(run.name.as_deref())
        .bind(run.pid.map(|p| p as i64))
        .bind(run.started_at.to_string())
        .bind(run.last_event_at.to_string())
        .bind(serde_json::to_string(run)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_runs(&self) -> Result<Vec<Run>> {
        let rows = sqlx::query("SELECT id, payload FROM runs ORDER BY last_event_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("run", &rows))
    }

    pub async fn save_project(&self, p: &Project) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, root, trusted, repo_url, auto_discovered, created_at)
             VALUES (?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, trusted=excluded.trusted, repo_url=excluded.repo_url,
               auto_discovered=excluded.auto_discovered",
        )
        .bind(p.id.as_str())
        .bind(&p.name)
        .bind(p.root.to_string_lossy().to_string())
        .bind(p.trusted as i32)
        .bind(p.repo_url.as_deref())
        .bind(p.auto_discovered as i32)
        .bind(jiff::Timestamp::now().to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_projects(&self) -> Result<Vec<Project>> {
        let rows =
            sqlx::query("SELECT id, name, root, trusted, repo_url, auto_discovered FROM projects")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .iter()
            .map(|r| Project {
                id: ProjectId::new(r.get::<String, _>("id")),
                name: r.get("name"),
                root: r.get::<String, _>("root").into(),
                trusted: r.get::<i64, _>("trusted") != 0,
                repo_url: r.get("repo_url"),
                auto_discovered: r.get::<i64, _>("auto_discovered") != 0,
            })
            .collect())
    }

    pub async fn save_work(&self, w: &crate::core::Work) -> Result<()> {
        sqlx::query(
            "INSERT INTO works (id, project_id, kind, phase, title, worktree, branch,
                                created_at, updated_at, payload)
             VALUES (?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
               phase=excluded.phase, title=excluded.title, worktree=excluded.worktree,
               branch=excluded.branch, updated_at=excluded.updated_at, payload=excluded.payload",
        )
        .bind(w.id.as_str())
        .bind(w.project_id.as_str())
        .bind(w.kind.as_str())
        .bind(w.phase.as_str())
        .bind(&w.title)
        .bind(w.worktree.as_ref().map(|p| p.to_string_lossy().to_string()))
        .bind(w.branch.as_deref())
        .bind(w.created_at.to_string())
        .bind(w.updated_at.to_string())
        .bind(serde_json::to_string(w)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_works(&self) -> Result<Vec<crate::core::Work>> {
        let rows = sqlx::query("SELECT id, payload FROM works ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("work", &rows))
    }

    /// How many stored rows this build can no longer read.
    ///
    /// Asked by `vibeplane doctor`, because the answer is otherwise invisible:
    /// the rows are simply absent from the board, which looks exactly like
    /// never having had them.
    pub async fn unreadable(&self) -> Result<Vec<(String, String)>> {
        // Literal queries, one per table: sqlx refuses a table name built by
        // `format!`, and it is right to — the rule that keeps one of these from
        // ever taking a parameter is worth more than the repetition it costs.
        let mut out = Vec::new();
        let runs = sqlx::query("SELECT id, payload FROM runs")
            .fetch_all(&self.pool)
            .await?;
        out.extend(unreadable_in::<Run>("run", &runs));
        let works = sqlx::query("SELECT id, payload FROM works")
            .fetch_all(&self.pool)
            .await?;
        out.extend(unreadable_in::<crate::core::Work>("work", &works));
        Ok(out)
    }

    /// Appends a fragment of a driven run's transcript.
    pub async fn append_message(&self, m: &crate::core::Message) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO messages (id, run_id, at, role, text) VALUES (?,?,?,?,?)",
        )
        .bind(&m.id)
        .bind(m.run_id.as_str())
        .bind(m.at.to_string())
        .bind(m.role.as_str())
        .bind(&m.text)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// One run's transcript, oldest first.
    ///
    /// `limit` takes the *newest* rows and hands them back in order, because a
    /// long conversation is read from the end — the same reason `tail` exists.
    pub async fn messages_for_run(
        &self,
        run: &RunId,
        limit: i64,
    ) -> Result<Vec<crate::core::Message>> {
        let rows = sqlx::query(
            "SELECT id, run_id, at, role, text FROM
               (SELECT * FROM messages WHERE run_id = ? ORDER BY id DESC LIMIT ?)
             ORDER BY id ASC",
        )
        .bind(run.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| crate::core::Message {
                id: r.get("id"),
                run_id: RunId::new(r.get::<String, _>("run_id")),
                at: r
                    .get::<String, _>("at")
                    .parse()
                    .unwrap_or_else(|_| jiff::Timestamp::now()),
                role: crate::core::Role::parse(&r.get::<String, _>("role")),
                text: r.get("text"),
            })
            .collect())
    }

    /// Appends a decision. Never updated, never pruned.
    pub async fn append_decision(&self, d: &crate::core::Decision) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO decisions
               (id, at, actor, action, subject, outcome, reason, tool, project_id, run_id, work_id)
             VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&d.id)
        .bind(d.at.to_string())
        .bind(d.actor.as_str())
        .bind(&d.action)
        .bind(&d.subject)
        .bind(&d.outcome)
        .bind(d.reason.as_deref())
        .bind(d.tool.as_deref())
        .bind(d.project_id.as_ref().map(|p| p.as_str()))
        .bind(d.run_id.as_ref().map(|r| r.as_str()))
        .bind(d.work_id.as_ref().map(|w| w.as_str()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records that the inbox is asking for something, once per asking.
    ///
    /// Idempotent against the *open* row rather than against the item id: the
    /// same question asked again after being answered is a new raise, because
    /// it is a new decision. That is the same rule the notifier follows when it
    /// decides whether to interrupt somebody twice.
    pub async fn attention_raise(&self, item: &crate::core::AttentionItem) -> Result<()> {
        sqlx::query(
            "INSERT INTO attention_log
               (item_id, kind, level, project_id, run_id, work_id, raised_at)
             SELECT ?,?,?,?,?,?,?
             WHERE NOT EXISTS (
               SELECT 1 FROM attention_log WHERE item_id = ? AND resolved_at IS NULL)",
        )
        .bind(item.id.0.as_str())
        .bind(item.kind.as_str())
        .bind(item.level.as_str())
        .bind(item.project_id.as_ref().map(|p| p.as_str()))
        .bind(item.run_id.as_ref().map(|r| r.as_str()))
        .bind(item.work_id.as_ref().map(|w| w.as_str()))
        .bind(item.since.to_string())
        .bind(item.id.0.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Closes the open row for these items, if any is still open.
    ///
    /// `WHERE resolved_at IS NULL` is what makes the measurement exact rather
    /// than heuristic: the API writes `Acted` the instant a person answers, the
    /// sweeper follows with `Elsewhere` for whatever merely vanished, and the
    /// first writer wins. No timing window, no correlation guess.
    pub async fn attention_resolve(
        &self,
        item_ids: &[String],
        resolution: crate::core::attention::Resolution,
    ) -> Result<u64> {
        if item_ids.is_empty() {
            return Ok(0);
        }
        // The id list rides as JSON through one bind rather than as a built
        // string of placeholders: the statement stays literal, which is what
        // `sqlx` asks for and what keeps an item id — a value derived from a
        // session id — away from the SQL itself.
        let ids = serde_json::to_string(item_ids)?;
        Ok(sqlx::query(
            "UPDATE attention_log SET resolved_at = ?, resolution = ?
             WHERE resolved_at IS NULL AND item_id IN (SELECT value FROM json_each(?))",
        )
        .bind(jiff::Timestamp::now().to_string())
        .bind(resolution.as_str())
        .bind(ids)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    /// Closes everything still open that is no longer being asked.
    ///
    /// One statement per tick rather than a read-then-write, so a run that
    /// unblocks between the two cannot be lost.
    /// Resolves every open item whose id starts with `prefix`.
    ///
    /// The forge items are keyed `gh:<project>:<kind>:<number>`, and a snooze
    /// dismisses a kind for a project — every number of it — so the match is
    /// on the prefix rather than on a list of ids the caller would have to
    /// reconstruct.
    pub async fn attention_resolve_prefix(
        &self,
        prefix: &str,
        resolution: crate::core::attention::Resolution,
    ) -> Result<u64> {
        Ok(sqlx::query(
            "UPDATE attention_log SET resolved_at = ?, resolution = ?
             WHERE resolved_at IS NULL AND item_id LIKE ? || '%'",
        )
        .bind(jiff::Timestamp::now().to_string())
        .bind(resolution.as_str())
        .bind(prefix)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    pub async fn attention_sweep(&self, still_open: &[String]) -> Result<u64> {
        let ids = serde_json::to_string(still_open)?;
        Ok(sqlx::query(
            "UPDATE attention_log SET resolved_at = ?, resolution = 'elsewhere'
             WHERE resolved_at IS NULL AND item_id NOT IN (SELECT value FROM json_each(?))",
        )
        .bind(jiff::Timestamp::now().to_string())
        .bind(ids)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    /// What the inbox did, per kind, since a point in time.
    pub async fn attention_stats(
        &self,
        since: jiff::Timestamp,
    ) -> Result<std::collections::BTreeMap<String, crate::core::attention::KindStats>> {
        let rows = sqlx::query(
            "SELECT kind,
                    COUNT(*) AS raised,
                    SUM(resolution = 'acted')     AS acted,
                    SUM(resolution = 'dismissed') AS dismissed,
                    SUM(resolution = 'elsewhere') AS elsewhere,
                    SUM(resolved_at IS NULL)      AS open
             FROM attention_log WHERE raised_at >= ? GROUP BY kind ORDER BY raised DESC",
        )
        .bind(since.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                use sqlx::Row;
                (
                    r.get::<String, _>("kind"),
                    crate::core::attention::KindStats {
                        raised: r.get("raised"),
                        acted: r.try_get("acted").unwrap_or(0),
                        dismissed: r.try_get("dismissed").unwrap_or(0),
                        elsewhere: r.try_get("elsewhere").unwrap_or(0),
                        open: r.try_get("open").unwrap_or(0),
                    },
                )
            })
            .collect())
    }

    /// The decision log, newest first, optionally about one run or one work
    /// item.
    pub async fn decisions(
        &self,
        about: Option<&str>,
        limit: i64,
    ) -> Result<Vec<crate::core::Decision>> {
        let rows = match about {
            Some(id) => sqlx::query(
                "SELECT * FROM decisions WHERE run_id = ? OR work_id = ?
                 ORDER BY at DESC LIMIT ?",
            )
            .bind(id)
            .bind(id)
            .bind(limit),
            None => sqlx::query("SELECT * FROM decisions ORDER BY at DESC LIMIT ?").bind(limit),
        }
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| crate::core::Decision {
                id: r.get("id"),
                at: r
                    .get::<String, _>("at")
                    .parse()
                    .unwrap_or_else(|_| jiff::Timestamp::now()),
                actor: match r.get::<String, _>("actor").as_str() {
                    "policy" => crate::core::Actor::Policy,
                    "human" => crate::core::Actor::Human,
                    _ => crate::core::Actor::Daemon,
                },
                action: r.get("action"),
                subject: r.get("subject"),
                outcome: r.get("outcome"),
                reason: r.get("reason"),
                tool: r.get("tool"),
                project_id: r.get::<Option<String>, _>("project_id").map(ProjectId::new),
                run_id: r.get::<Option<String>, _>("run_id").map(RunId::new),
                work_id: r
                    .get::<Option<String>, _>("work_id")
                    .map(crate::core::WorkId::new),
            })
            .collect())
    }

    /// Records that a channel delivered something, with how long the handler
    /// took. A hook that is slow is a hook the user feels, so the number is
    /// kept rather than inferred.
    pub async fn record_channel(
        &self,
        channel: &str,
        micros: u64,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO channel_health
               (channel, last_seen_at, count, worst_micros, last_error, last_error_at)
             VALUES (?, ?, 1, ?, ?, ?)
             ON CONFLICT(channel) DO UPDATE SET
               last_seen_at = excluded.last_seen_at,
               count = channel_health.count + 1,
               worst_micros = MAX(channel_health.worst_micros, excluded.worst_micros),
               last_error = COALESCE(excluded.last_error, channel_health.last_error),
               last_error_at = COALESCE(excluded.last_error_at, channel_health.last_error_at)",
        )
        .bind(channel)
        .bind(jiff::Timestamp::now().to_string())
        .bind(micros as i64)
        .bind(error)
        .bind(error.map(|_| jiff::Timestamp::now().to_string()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn channel_health(&self) -> Result<Vec<ChannelHealth>> {
        let rows = sqlx::query(
            "SELECT channel, last_seen_at, count, worst_micros, last_error, last_error_at
             FROM channel_health",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| ChannelHealth {
                channel: r.get("channel"),
                last_seen_at: r.get("last_seen_at"),
                count: r.get::<i64, _>("count") as u64,
                worst_micros: r.get::<i64, _>("worst_micros") as u64,
                last_error: r.get("last_error"),
                last_error_at: r.get("last_error_at"),
            })
            .collect())
    }

    /// Deletes events and transcripts older than `days`. Runs are kept: a row
    /// on the board costs nothing, and losing it would make a resumable session
    /// invisible.
    ///
    /// The search index goes with them, in the same transaction: it is the
    /// larger half of the file, and an index that outlives what it indexes is a
    /// search that finds things that are gone.
    /// Removes everything a gate probe left behind in an earlier build: the
    /// probe session's run, events and decisions, and the temporary project
    /// its working directory was discovered as. Run once at start; a store
    /// that never held any returns zero and costs a few statements.
    pub async fn forget_probe(&self, session: &str) -> Result<u64> {
        let mut n = 0;
        for sql in [
            "DELETE FROM events WHERE run_id = ?",
            "DELETE FROM decisions WHERE run_id = ?",
            "DELETE FROM runs WHERE id = ? OR session_id = ?",
        ] {
            let q = sqlx::query(sql).bind(session);
            let q = if sql.contains("session_id") {
                q.bind(session)
            } else {
                q
            };
            n += q.execute(&self.pool).await?.rows_affected();
        }
        n += sqlx::query("DELETE FROM projects WHERE root LIKE '%/vibeplane-probe-%'")
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(n)
    }

    pub async fn prune_events(&self, days: i64) -> Result<u64> {
        let cutoff =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * days)).to_string();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM events_fts WHERE event_id IN
               (SELECT id FROM events WHERE at < ?)",
        )
        .bind(&cutoff)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM messages WHERE at < ?")
            .bind(&cutoff)
            .execute(&mut *tx)
            .await?;
        // The attention log goes too, and this is where it was supposed to
        // have been going all along: it is an observation about what Vibeplane
        // *showed*, one row per item raised, and nothing was removing it.
        // `vibeplane attention` only ever looks back a fixed number of days,
        // and the table grew without bound on exactly the machines this product
        // is for — twenty agents raising items all day.
        //
        // Only rows that have been **resolved**. An item still open is a
        // question somebody has not answered, and age is not an answer.
        sqlx::query("DELETE FROM attention_log WHERE raised_at < ? AND resolved_at IS NOT NULL")
            .bind(&cutoff)
            .execute(&mut *tx)
            .await?;
        let r = sqlx::query("DELETE FROM events WHERE at < ?")
            .bind(&cutoff)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(r.rows_affected())
    }
}

/// The rows of one table this build cannot decode, and why.
fn unreadable_in<T: serde::de::DeserializeOwned>(
    kind: &str,
    rows: &[sqlx::sqlite::SqliteRow],
) -> Vec<(String, String)> {
    rows.iter()
        .filter_map(|r| {
            let payload: String = r.get("payload");
            serde_json::from_str::<T>(&payload).err().map(|e| {
                let id: String = r.get("id");
                (format!("{kind} {id}"), e.to_string())
            })
        })
        .collect()
}

/// Decodes stored payloads, saying so when one cannot be read.
///
/// The schema here changes without a migration on purpose — the store is
/// rebuildable and the project is unreleased — which makes *silence* the danger.
/// A `filter_map(…ok())` dropped any row this build could no longer parse, so a
/// changed struct made runs and work quietly disappear from the board, which
/// looks identical to never having had them. A Work row is the worst case: it
/// carries the branch and the worktree, so losing one orphans a checkout that
/// nobody is left to tell you about.
fn decode_rows<T: serde::de::DeserializeOwned>(
    kind: &str,
    rows: &[sqlx::sqlite::SqliteRow],
) -> Vec<T> {
    let mut out = Vec::with_capacity(rows.len());
    let mut lost = 0usize;
    for r in rows {
        let payload: String = r.get("payload");
        match serde_json::from_str(&payload) {
            Ok(v) => out.push(v),
            Err(e) => {
                lost += 1;
                // Bounded: a schema change makes *every* row unreadable, and a
                // line each would bury the one summary that matters.
                if lost <= 3 {
                    let id: String = r.get("id");
                    tracing::warn!(%kind, %id, error = %e, "a stored row could not be read");
                }
            }
        }
    }
    if lost > 0 {
        tracing::warn!(
            %kind,
            lost,
            kept = out.len(),
            "rows this build cannot read were left out of the board — \
             `vibeplane doctor` lists them; deleting the database rebuilds it from the providers"
        );
    }
    out
}

/// Liveness of one observation channel.
/// One tool call as it was observed, with the directory whose rules govern it.
#[derive(Debug, Clone)]
pub struct ObservedCall {
    pub cwd: PathBuf,
    pub tool: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelHealth {
    pub channel: String,
    pub last_seen_at: String,
    pub count: u64,
    /// The worst handler latency seen — a maximum, not a percentile, in the
    /// column of the same name. Keeping a real p99 would need a histogram per
    /// channel, and the number is only ever read by a person looking for a
    /// stall, who wants the worst case.
    pub worst_micros: u64,
    pub last_error: Option<String>,
    /// When that error happened. An error with no date on it reads as current.
    pub last_error_at: Option<String>,
}

/// Turns what a person typed into an FTS5 query they would recognise.
///
/// Each run of word characters becomes a quoted term, and the terms are ANDed:
/// `pnpm test` finds rows containing both. Everything else — quotes, hyphens,
/// `NEAR`, unbalanced parentheses — is punctuation to a human and a syntax
/// error to FTS5, so it is dropped rather than passed on. A trailing `*` on the
/// last term keeps prefix search, which is what makes typing feel live.
fn fts_phrase(query: &str) -> String {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\""))
        .collect();
    terms.join(" AND ")
}

fn row_to_event(row: &sqlx::sqlite::SqliteRow) -> Result<EventEnvelope> {
    let at: String = row.get("at");
    let project: Option<String> = row.get("project_id");
    Ok(EventEnvelope {
        id: row.get("id"),
        at: at.parse().unwrap_or_else(|_| jiff::Timestamp::now()),
        run_id: RunId::new(row.get::<String, _>("run_id")),
        project_id: project.map(ProjectId::new),
        source: match row.get::<String, _>("source").as_str() {
            "otel" => crate::core::event::Source::Otel,
            "agents_json" => crate::core::event::Source::AgentsJson,
            "statusline" => crate::core::event::Source::StatusLine,
            "daemon" => crate::core::event::Source::Daemon,
            _ => crate::core::event::Source::Hook,
        },
        event: serde_json::from_str(row.get::<String, _>("payload").as_str())?,
    })
}

/// What of an event is worth searching for. Deliberately narrow: commands,
/// questions and errors are what a human looks for months later.
fn searchable_text(e: &crate::core::event::Event) -> Option<String> {
    use crate::core::event::Event as E;
    match e {
        E::ToolStarted { tool, input } => {
            let detail = input
                .get("command")
                .or_else(|| input.get("file_path"))
                .or_else(|| input.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            Some(format!("{tool} {detail}"))
        }
        E::QuestionAsked { question, .. } => Some(question.clone()),
        E::TurnFailed { message } => Some(message.clone()),
        E::ApiError { error, .. } => Some(error.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_replay_reads_the_directory_from_the_run_not_the_event() {
        // The directory decides whose rules apply, and the event does not
        // carry one. A tool call whose run row is gone is dropped rather than
        // evaluated against some other project's rules, which would report a
        // coverage figure about a repository the call never touched.
        let s = Store::open_in_memory().await.unwrap();
        let run = crate::core::run::Run::new(
            crate::core::ids::SessionId::new("s1"),
            PathBuf::from("/repo"),
            crate::core::run::RunMode::Observed,
            "claude",
        );
        let run_id = run.id.to_string();
        s.save_run(&run).await.unwrap();
        for (rid, tool, input) in [
            (
                run_id.as_str(),
                "Bash",
                serde_json::json!({"command": "pnpm test"}),
            ),
            (
                run_id.as_str(),
                "TodoWrite",
                serde_json::json!({"todos": []}),
            ),
            ("gone", "Bash", serde_json::json!({"command": "rm -rf /"})),
        ] {
            sqlx::query(
                "INSERT INTO events (id, at, run_id, project_id, source, kind, payload)
                 VALUES (?, '2026-09-14T10:00:00Z', ?, NULL, 'hook', 'tool_started', ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(rid)
            .bind(serde_json::json!({"tool": tool, "input": input}).to_string())
            .execute(&s.pool)
            .await
            .unwrap();
        }

        let calls = s.observed_tool_calls(None, 100).await.unwrap();
        assert_eq!(calls.len(), 1);
        // Scoping happens in the query, so a limit cannot spend itself on other
        // projects' calls before the filter runs.
        assert_eq!(
            s.observed_tool_calls(Some(Path::new("/repo")), 100)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            s.observed_tool_calls(Some(Path::new("/elsewhere")), 100)
                .await
                .unwrap()
                .is_empty()
        );
        // The `TodoWrite` is dropped because no path or command rule can speak
        // for it, and counting it would make the coverage figure unactionable.
        // The orphan is dropped because its directory is unknown.
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "Bash");
        assert_eq!(calls[0].cwd, PathBuf::from("/repo"));
    }

    #[tokio::test]
    async fn a_channel_error_is_kept_with_the_date_it_happened() {
        // The error is kept after the problem is fixed, because "this channel
        // has had a problem" is worth knowing. It is kept *with its date*
        // because an error carrying none reads as current, and a diagnostic
        // that cries wolf is one nobody reads twice.
        let s = Store::open_in_memory().await.unwrap();
        s.record_channel("otel", 10, Some("record had no session"))
            .await
            .unwrap();
        let before = s.channel_health().await.unwrap();
        let ch = before.iter().find(|c| c.channel == "otel").unwrap();
        let stamped = ch.last_error_at.clone().expect("an error carries its date");
        assert_eq!(ch.last_error.as_deref(), Some("record had no session"));

        // A later success does not erase the error, and does not restamp it
        // either — otherwise a channel that failed once last week would look
        // like it failed a moment ago, every moment, for ever.
        s.record_channel("otel", 5, None).await.unwrap();
        let after = s.channel_health().await.unwrap();
        let ch = after.iter().find(|c| c.channel == "otel").unwrap();
        assert_eq!(ch.last_error.as_deref(), Some("record had no session"));
        assert_eq!(ch.last_error_at.as_deref(), Some(stamped.as_str()));
        assert_eq!(ch.count, 2, "and the channel is still counted as alive");
    }

    #[tokio::test]
    async fn the_schema_creates_every_table() {
        // Proves the schema applied, not that a string contains a word: a
        // half-applied schema only fails on the first query otherwise.
        let s = Store::open_in_memory().await.unwrap();
        let names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
                .fetch_all(s.pool())
                .await
                .unwrap();
        for table in [
            "projects",
            "runs",
            "events",
            "events_fts",
            "channel_health",
            "works",
            "decisions",
            "messages",
        ] {
            assert!(
                names.iter().any(|n| n == table),
                "schema did not create `{table}`; got {names:?}"
            );
        }
    }
    use crate::core::SessionId;
    use crate::core::event::{Event, Source};
    use crate::core::run::RunMode;
    use std::path::PathBuf;

    #[tokio::test]
    async fn events_round_trip_and_replay_in_order() {
        let s = Store::open_in_memory().await.unwrap();
        for i in 0..5 {
            let env = EventEnvelope::new(
                RunId::new("s1"),
                Source::Hook,
                Event::ToolStarted {
                    tool: format!("Tool{i}"),
                    input: serde_json::json!({"command": format!("cmd {i}")}),
                },
            );
            s.append_event(&env).await.unwrap();
        }
        let back = s.events_for_run(&RunId::new("s1"), 100).await.unwrap();
        assert_eq!(back.len(), 5);
        match &back[0].event {
            Event::ToolStarted { tool, .. } => assert_eq!(tool, "Tool0"),
            other => panic!("wrong order: {other:?}"),
        }
    }

    #[tokio::test]
    async fn search_finds_a_command() {
        let s = Store::open_in_memory().await.unwrap();
        s.append_event(&EventEnvelope::new(
            RunId::new("s1"),
            Source::Hook,
            Event::ToolStarted {
                tool: "Bash".into(),
                input: serde_json::json!({"command": "pnpm typecheck"}),
            },
        ))
        .await
        .unwrap();
        let hits = s.search("typecheck", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, RunId::new("s1"));
    }

    #[tokio::test]
    async fn runs_and_projects_survive_a_reopen() {
        let s = Store::open_in_memory().await.unwrap();
        let mut run = Run::new(
            SessionId::new("s1"),
            PathBuf::from("/repo"),
            RunMode::Observed,
            "claude",
        );
        run.totals.cost_usd = 1.25;
        s.save_run(&run).await.unwrap();
        s.save_project(&Project::from_root(PathBuf::from("/repo")))
            .await
            .unwrap();

        let runs = s.load_runs().await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].totals.cost_usd, 1.25);
        assert_eq!(s.load_projects().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_query_a_person_would_type_is_not_a_syntax_error() {
        // `vibeplane search "cargo test --workspace"` would otherwise reach FTS5 as an
        // expression and come back as a 500.
        let s = Store::open_in_memory().await.unwrap();
        s.append_event(&EventEnvelope::new(
            RunId::new("s1"),
            Source::Hook,
            Event::ToolStarted {
                tool: "Bash".into(),
                input: serde_json::json!({"command": "cargo test --workspace"}),
            },
        ))
        .await
        .unwrap();

        for q in [
            "cargo test",
            "cargo test --workspace",
            "\"quoted\"",
            "a-b",
            "NEAR(",
            "*",
        ] {
            s.search(q, 10).await.unwrap_or_else(|e| panic!("{q}: {e}"));
        }
        assert_eq!(s.search("cargo workspace", 10).await.unwrap().len(), 1);
        assert!(s.search("!!!", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn pruning_takes_the_search_index_with_it() {
        // Otherwise the index outgrows the log it indexes, and search keeps
        // finding events that are no longer there.
        let s = Store::open_in_memory().await.unwrap();
        s.append_event(&EventEnvelope::new(
            RunId::new("s1"),
            Source::Hook,
            Event::ToolStarted {
                tool: "Bash".into(),
                input: serde_json::json!({"command": "pnpm typecheck"}),
            },
        ))
        .await
        .unwrap();
        assert_eq!(s.search("typecheck", 10).await.unwrap().len(), 1);

        // Everything is older than "negative one day ago".
        s.prune_events(-1).await.unwrap();
        assert!(
            s.search("typecheck", 10).await.unwrap().is_empty(),
            "the index still points at a deleted event"
        );
    }

    #[tokio::test]
    async fn the_same_event_is_indexed_once() {
        let s = Store::open_in_memory().await.unwrap();
        let env = EventEnvelope::new(
            RunId::new("s1"),
            Source::Hook,
            Event::ToolStarted {
                tool: "Bash".into(),
                input: serde_json::json!({"command": "pnpm build"}),
            },
        );
        s.append_event(&env).await.unwrap();
        s.append_event(&env).await.unwrap();
        assert_eq!(s.search("build", 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_transcript_reads_back_in_order_and_from_the_end() {
        let s = Store::open_in_memory().await.unwrap();
        for (role, text) in [
            (crate::core::Role::User, "fix the login test"),
            (crate::core::Role::Thought, "I should read auth.rs"),
            (crate::core::Role::Agent, "Reading auth.rs"),
            (crate::core::Role::Agent, "Found it"),
        ] {
            s.append_message(&crate::core::Message::new(RunId::new("r1"), role, text))
                .await
                .unwrap();
        }
        s.append_message(&crate::core::Message::new(
            RunId::new("other"),
            crate::core::Role::Agent,
            "not this one",
        ))
        .await
        .unwrap();

        let all = s.messages_for_run(&RunId::new("r1"), 100).await.unwrap();
        assert_eq!(all.len(), 4, "one run's transcript, not the machine's");
        assert_eq!(all[0].text, "fix the login test");
        assert_eq!(all[3].text, "Found it");
        assert_eq!(all[1].role, crate::core::Role::Thought);

        // A long conversation is read from the end, and still in order.
        let tail = s.messages_for_run(&RunId::new("r1"), 2).await.unwrap();
        assert_eq!(
            tail.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(),
            ["Reading auth.rs", "Found it"]
        );
    }

    #[tokio::test]
    async fn a_transcript_ages_out_with_the_events_beside_it() {
        // History, like any other observation — unlike the decision log, which
        // is neither and stays.
        let s = Store::open_in_memory().await.unwrap();
        s.append_message(&crate::core::Message::new(
            RunId::new("r1"),
            crate::core::Role::Agent,
            "something",
        ))
        .await
        .unwrap();
        s.prune_events(-1).await.unwrap();
        assert!(
            s.messages_for_run(&RunId::new("r1"), 10)
                .await
                .unwrap()
                .is_empty()
        );
    }

    fn raised(id: &str, kind: crate::core::AttentionKind) -> crate::core::AttentionItem {
        crate::core::AttentionItem {
            id: crate::core::AttentionId::new(id),
            level: kind.default_level(),
            kind,
            run_id: Some(RunId::new("s1")),
            project_id: None,
            title: "t".into(),
            detail: None,
            options: vec![],
            actions: vec![],
            request_id: None,
            url: None,
            launch: None,
            work_id: None,
            suggested_rule: None,
            since: jiff::Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn an_answered_item_is_never_counted_as_one_that_went_away() {
        // The whole measurement rests on this. The API writes `acted` the
        // instant a person answers; the sweeper follows a moment later and
        // closes everything that merely stopped being asked. If the sweeper
        // could overwrite, every answered item would eventually be recorded as
        // noise and the number would say the opposite of the truth — so the
        // guard is `resolved_at IS NULL` and this test is what holds it.
        use crate::core::AttentionKind;
        use crate::core::attention::Resolution;
        let s = Store::open_in_memory().await.unwrap();

        let perm = raised("s1:permission", AttentionKind::Permission);
        let stall = raised("s1:stalled", AttentionKind::Stalled);
        s.attention_raise(&perm).await.unwrap();
        s.attention_raise(&stall).await.unwrap();

        // Raising twice while it is still open is the same asking.
        s.attention_raise(&perm).await.unwrap();

        // A person answers the permission; the stall just stops being true.
        s.attention_resolve(&["s1:permission".into()], Resolution::Acted)
            .await
            .unwrap();
        s.attention_sweep(&[]).await.unwrap();

        let stats = s
            .attention_stats(jiff::Timestamp::UNIX_EPOCH)
            .await
            .unwrap();
        let p = stats.get("permission").expect("permission was raised");
        assert_eq!((p.raised, p.acted, p.elsewhere), (1, 1, 0), "{p:?}");
        let st = stats.get("stalled").expect("stalled was raised");
        assert_eq!((st.raised, st.acted, st.elsewhere), (1, 0, 1), "{st:?}");

        // Asked again after being answered is a new decision, so a new row.
        s.attention_raise(&perm).await.unwrap();
        let stats = s
            .attention_stats(jiff::Timestamp::UNIX_EPOCH)
            .await
            .unwrap();
        let p = stats.get("permission").unwrap();
        assert_eq!((p.raised, p.open), (2, 1), "{p:?}");
        assert_eq!(
            p.acted_share(),
            Some(1.0),
            "one resolved item, and it was acted on"
        );

        // A kind that never fired and one that always gets ignored must not
        // print the same, which is why this is an Option rather than a zero.
        assert_eq!(
            crate::core::attention::KindStats::default().acted_share(),
            None
        );
    }

    #[tokio::test]
    async fn a_resolved_attention_row_is_pruned_and_an_open_one_is_not() {
        // One row per item raised, on a machine running twenty agents all day,
        // and nothing was removing them: this table was meant to be pruned with
        // the events and the sweep never touched it. A documented behaviour
        // with no code behind it, in the table that measures whether the inbox
        // is worth reading.
        //
        // Resolved rows only. An item still open is a question nobody has
        // answered, and age is not an answer.
        let s = Store::open_in_memory().await.unwrap();
        let mk = |id: &str| crate::core::AttentionItem {
            id: crate::core::AttentionId(id.to_string()),
            kind: crate::core::AttentionKind::Permission,
            level: crate::core::Level::High,
            title: "t".into(),
            detail: None,
            project_id: None,
            run_id: Some(RunId::new("r1")),
            work_id: None,
            suggested_rule: None,
            since: jiff::Timestamp::now(),
            options: Vec::new(),
            actions: Vec::new(),
            launch: None,
            request_id: None,
            url: None,
        };
        s.attention_raise(&mk("closed")).await.unwrap();
        s.attention_raise(&mk("open")).await.unwrap();
        s.attention_resolve(
            &["closed".to_string()],
            crate::core::attention::Resolution::Acted,
        )
        .await
        .unwrap();

        s.prune_events(-1).await.unwrap();
        let long_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * 3650);
        let stats = s.attention_stats(long_ago).await.unwrap();
        let raised: i64 = stats.values().map(|r| r.raised).sum();
        assert_eq!(raised, 1, "the resolved row went and the open one stayed");
    }

    #[tokio::test]
    async fn a_decision_log_written_by_an_older_release_still_opens_and_still_writes() {
        // `decisions` is the one table that cannot be rebuilt, and
        // `CREATE TABLE IF NOT EXISTS` leaves an existing one alone — so a
        // column added to `schema.sql` never reaches a database somebody
        // already has, and the first write naming it fails. Upgrading would
        // have cost the audit trail, which is the one thing here that is not
        // re-derivable from anything.
        let dir = std::env::temp_dir().join(format!("vp-upgrade-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("v.db");

        // A store as an older release left it: the table without `tool`, and a
        // row in it.
        {
            let s = Store::open(&path).await.unwrap();
            sqlx::raw_sql("DROP TABLE decisions")
                .execute(&s.pool)
                .await
                .unwrap();
            sqlx::raw_sql(
                "CREATE TABLE decisions (
                   id TEXT PRIMARY KEY, at TEXT NOT NULL, actor TEXT NOT NULL,
                   action TEXT NOT NULL, subject TEXT NOT NULL, outcome TEXT NOT NULL,
                   reason TEXT, project_id TEXT, run_id TEXT, work_id TEXT)",
            )
            .execute(&s.pool)
            .await
            .unwrap();
            sqlx::raw_sql(
                "INSERT INTO decisions (id, at, actor, action, subject, outcome)
                 VALUES ('d1', '2026-09-15T00:00:00Z', 'policy', 'agent:tool.use', 'cat a', 'allow')",
            )
            .execute(&s.pool)
            .await
            .unwrap();
        }

        // This release opens it, keeps the old row, and can write a new one.
        let s = Store::open(&path).await.unwrap();
        s.append_decision(
            &crate::core::Decision::new(
                crate::core::Actor::Policy,
                "agent:tool.use",
                "echo x > notes.md",
                "allow",
            )
            .by_tool("Bash")
            .for_run(&RunId::new("s1")),
        )
        .await
        .unwrap();

        let rows = s.decisions(None, 10).await.unwrap();
        assert_eq!(
            rows.len(),
            2,
            "the older release's row survives the upgrade"
        );
        assert!(
            rows.iter().any(|d| d.tool.as_deref() == Some("Bash")),
            "and this release's column is writable: {rows:?}"
        );
        assert!(
            rows.iter().any(|d| d.id == "d1" && d.tool.is_none()),
            "a row written before the column exists reads as having no tool"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn decisions_are_appended_and_survive_a_prune() {
        // The event log is history you can lose. The decision log is the answer
        // to "why is there a pull request on this branch", and losing it leaves
        // one nobody can account for.
        let s = Store::open_in_memory().await.unwrap();
        s.append_decision(
            &crate::core::Decision::new(
                crate::core::Actor::Policy,
                "agent:tool.use",
                "pnpm test -- --run",
                "allow",
            )
            .because("Bash(pnpm test *)")
            .for_run(&RunId::new("s1")),
        )
        .await
        .unwrap();
        s.append_decision(&crate::core::Decision::new(
            crate::core::Actor::Daemon,
            "gh:pr.create",
            "fix/login",
            "done",
        ))
        .await
        .unwrap();

        s.prune_events(-1).await.unwrap();
        let all = s.decisions(None, 10).await.unwrap();
        assert_eq!(all.len(), 2, "a prune must not touch the decision log");
        assert_eq!(all[0].action, "gh:pr.create", "newest first");

        let about = s.decisions(Some("s1"), 10).await.unwrap();
        assert_eq!(about.len(), 1);
        assert_eq!(about[0].reason.as_deref(), Some("Bash(pnpm test *)"));
    }

    #[tokio::test]
    async fn channel_health_accumulates() {
        let s = Store::open_in_memory().await.unwrap();
        s.record_channel("hook", 900, None).await.unwrap();
        s.record_channel("hook", 1500, None).await.unwrap();
        let h = s.channel_health().await.unwrap();
        assert_eq!(h[0].count, 2);
        assert_eq!(h[0].worst_micros, 1500);
    }

    #[tokio::test]
    async fn a_row_this_build_cannot_read_is_reported_rather_than_dropped() {
        // The schema changes here without a migration, on purpose. That makes
        // silence the danger: a row whose shape this build no longer
        // understands used to be filtered out, so it was absent from the board
        // in a way that looks exactly like never having existed. For a `work`
        // row that means an orphaned branch and worktree nobody is told about.
        let s = Store::open_in_memory().await.unwrap();

        let mut run = Run::new(
            crate::core::ids::SessionId::new("s-ok"),
            "/tmp/repo".into(),
            crate::core::run::RunMode::Observed,
            "claude",
        );
        run.summary = Some("readable".into());
        s.save_run(&run).await.unwrap();

        // A row written by some other version of this struct.
        sqlx::query(
            "INSERT INTO runs (id, session_id, agent, mode, state, cwd, started_at,
                               last_event_at, payload)
             VALUES ('s-bad','s-bad','claude','observed','working','/tmp/repo','t','t',
                     '{\"not\":\"a run\"}')",
        )
        .execute(&s.pool)
        .await
        .unwrap();

        let loaded = s.load_runs().await.unwrap();
        assert_eq!(loaded.len(), 1, "the readable row still loads");

        let bad = s.unreadable().await.unwrap();
        assert_eq!(bad.len(), 1, "and the unreadable one is named: {bad:?}");
        assert!(bad[0].0.contains("s-bad"), "{bad:?}");
        assert!(!bad[0].1.is_empty(), "with a reason somebody can act on");
    }
}

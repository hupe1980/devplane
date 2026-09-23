//! The observation store: SQLite, WAL, rebuildable.
//!
//! Two rules shape everything here:
//!
//! * **Observations only.** Events, and the run and project rows derived from
//!   them. Anything Devplane causes belongs in the runtime journal instead, so
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
// Only the in-memory store parses a connection string, and that is test-only.
#[cfg(test)]
use std::str::FromStr;

/// The shape of `schema.sql`, stamped into every database this build writes.
///
/// **Bump it whenever `schema.sql` changes in a way an older file would not
/// satisfy** — a renamed or added column, a changed type, a dropped table. A
/// file stamped with anything else is moved aside on open rather than migrated
/// (`Store::retire_if_stale`), which is affordable because everything here
/// except `decisions` is re-derivable from the event log or the provider.
///
/// **It is 2. The count started at 1 on 2026-09-20 and moved once, the same
/// day, when `decisions` grew a `server_source` column.** A database written by
/// the previous build is moved aside rather than migrated, which is what the
/// number is for. It had reached 5 by recording every
/// shape this schema passed through before anybody could have a database — and
/// with nothing released, none of those numbers described a file that exists.
/// A version is for telling *your* file from *another* one, not for keeping a
/// history; the history is in the changelog.
pub const SCHEMA_VERSION: i64 = 3;

/// The statement that stamps it. Written out rather than formatted, because
/// `PRAGMA user_version` accepts no bind parameter and a formatted string
/// would be a dynamic SQL string for a value that is a literal in this file.
const SCHEMA_VERSION_PRAGMA: &str = "PRAGMA user_version = 3";

/// A handle on the observation store.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

/// One `agent_capabilities` row as SQLite hands it back, before it becomes a
/// [`AgentCapabilityRecord`](crate::core::AgentCapabilityRecord).
///
/// Named rather than written inline because eight positional columns are a type
/// nobody can read at the call site, and the compiler says so.
type AgentCapabilityRow = (String, Option<String>, i64, i64, i64, i64, i64, String);

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
        // A database written by a different schema is moved aside before it is
        // opened, never migrated and never silently reused. See [`SCHEMA_VERSION`].
        Self::retire_if_stale(path).await?;
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await
            .with_context(|| format!("opening {}", path.display()))?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Renames a database written by a different schema out of the way.
    ///
    /// **There is no migration machinery and there is not going to be.** Every
    /// table here except `decisions` is a projection of the event log or of the
    /// provider, so the cost of starting again is a replay rather than a loss —
    /// and `decisions` is why the old file is *moved* rather than deleted. The
    /// user is told where it went.
    ///
    /// This replaces a loop of `ALTER TABLE … ADD COLUMN` statements whose
    /// success case was a duplicate-column error. That worked for exactly the
    /// change it was written for and silently did nothing for the next one: a
    /// renamed column, a changed type or a dropped table all leave a file that
    /// opens cleanly and fails on the first write naming the new shape, at
    /// runtime, in whichever surface happened to write first.
    async fn retire_if_stale(path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .busy_timeout(std::time::Duration::from_secs(5));
        let found: Option<i64> = match SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
        {
            Ok(pool) => {
                let v: Option<(i64,)> = sqlx::query_as("PRAGMA user_version")
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten();
                pool.close().await;
                v.map(|(n,)| n)
            }
            // Unreadable is not the same as stale. A file this build cannot
            // open at all is left exactly where it is, because moving it would
            // be this tool destroying evidence about its own failure.
            Err(_) => return Ok(()),
        };
        if found == Some(SCHEMA_VERSION) {
            return Ok(());
        }
        let found = found.unwrap_or(0);
        let aside = path.with_extension(format!("v{found}.bak"));
        std::fs::rename(path, &aside).with_context(|| {
            format!(
                "moving a database written by schema v{found} aside to {}",
                aside.display()
            )
        })?;
        // Both sidecars go with it or the new database inherits a stale journal.
        for suffix in ["-wal", "-shm"] {
            let from = PathBuf::from(format!("{}{suffix}", path.display()));
            if from.exists() {
                let _ = std::fs::rename(&from, format!("{}{suffix}", aside.display()));
            }
        }
        tracing::warn!(
            schema_found = found,
            schema_expected = SCHEMA_VERSION,
            moved_to = %aside.display(),
            "the database was written by a different schema and has been moved aside; \
             observations will be rebuilt from the providers, and the decision log in the \
             old file is the one thing that is not re-derivable"
        );
        Ok(())
    }

    /// An in-memory store, for tests.
    ///
    /// Configured exactly like the real one — foreign keys included. A test
    /// database with the constraints turned off proves nothing about the
    /// database the product ships.
    ///
    /// **Gated, because it was shipping.** Nothing outside this file's own test
    /// module has ever called it, so it was a test fixture compiled into every
    /// released binary — and invisible to the no-reader guard until that guard
    /// was widened past `src/core/`. Gating it is the honest form of the
    /// exemption: the compiler now enforces what the doc comment always said.
    #[cfg(test)]
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

    /// Applies the schema and stamps its version.
    ///
    /// There is no migration machinery on purpose: almost everything here is a
    /// projection of the event log, and a row this build cannot decode is
    /// dropped and counted (`unreadable rows`) rather than migrated. A file
    /// written by a different schema never reaches this function —
    /// [`Self::retire_if_stale`] has already moved it aside.
    async fn migrate(&self) -> Result<()> {
        // `raw_sql` runs the whole file, comments and all, in one round trip.
        // Hand-splitting on `;` is how a schema loses the statement that a
        // comment happens to sit above.
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await
            .context("applying schema")?;
        // Stamp the version this file was written by, so the next build can
        // tell whether it understands it. Written after the schema applies, so
        // a half-created file is not stamped as complete.
        // `PRAGMA user_version` takes no bind parameters, so the statement is
        // built from the constant — which is an integer literal in this source
        // file and can never be user input.
        sqlx::query(SCHEMA_VERSION_PRAGMA)
            .execute(&self.pool)
            .await
            .context("stamping the schema version")?;
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
    /// through raw meant `devplane search "a-b"` or a query containing a quote
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
                                created_at, updated_at, batch_id, payload)
             VALUES (?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
               phase=excluded.phase, title=excluded.title, worktree=excluded.worktree,
               branch=excluded.branch, updated_at=excluded.updated_at,
               batch_id=excluded.batch_id, payload=excluded.payload",
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
        .bind(w.batch_id.as_ref().map(|b| b.as_str()))
        .bind(serde_json::to_string(w)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records a fan-out.
    ///
    /// Written once and never updated: a batch's own facts — the prompt, the
    /// targets, the position, who sent it and when — are fixed the moment it is
    /// sent. What changes afterwards belongs to its members, which are work
    /// rows with their own lifecycle.
    pub async fn save_batch(&self, b: &crate::core::batch::Batch) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO batches
               (id, kind, position, prompt, template, sent_at, sent_by, targets)
             VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(b.id.as_str())
        .bind(match b.kind {
            crate::core::batch::Kind::Drafted => "drafted",
            crate::core::batch::Kind::Dispatched => "dispatched",
        })
        .bind(b.position.as_str())
        .bind(&b.prompt)
        .bind(b.template.as_deref())
        .bind(b.sent_at.to_string())
        .bind(&b.sent_by)
        .bind(serde_json::to_string(&b.targets)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Asks ────────────────────────────────────────────────────────────
    //
    // The one projection here that is *not* rebuildable from the providers. An
    // agent asks once; if the row is lost, the question is lost with it, and
    // nothing on the machine can re-derive what somebody was asked.

    /// Writes an ask, and every later state of it.
    ///
    /// Records what an agent said it could do, the last time it was started.
    ///
    /// **Overwrites on purpose.** The interesting value is the *current* one: an
    /// agent that gains `session/list` in a release should read as having it,
    /// not as two rows a surface has to choose between. The date says when the
    /// answer was true.
    pub async fn save_agent_capabilities(
        &self,
        c: &crate::core::AgentCapabilityRecord,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO agent_capabilities
                 (command, agent_name, resume, load_session, list_sessions,
                  declares_modes, needs_auth, measured_at)
             VALUES (?,?,?,?,?,?,?,?)",
        )
        .bind(&c.command)
        .bind(c.agent_name.as_deref())
        .bind(c.resume as i64)
        .bind(c.load_session as i64)
        .bind(c.list_sessions as i64)
        .bind(c.declares_modes as i64)
        .bind(c.needs_auth as i64)
        .bind(c.measured_at.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Notes that this agent declared a session mode.
    ///
    /// **A separate write because it is a separate observation.** The rest of
    /// the record comes from `initialize`; whether an agent declares a mode is
    /// only visible when a session is created, which is later and can fail. A
    /// single write covering both would have to choose one moment and be wrong
    /// about the other.
    pub async fn note_agent_declares_modes(&self, command: &str) -> Result<()> {
        sqlx::query("UPDATE agent_capabilities SET declares_modes = 1 WHERE command = ?")
            .bind(command)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Everything measured so far. An agent with no row has never been started,
    /// and the caller reports that as *not probed*.
    pub async fn agent_capabilities(&self) -> Result<Vec<crate::core::AgentCapabilityRecord>> {
        let rows: Vec<AgentCapabilityRow> = sqlx::query_as(
            "SELECT command, agent_name, resume, load_session, list_sessions,
                    declares_modes, needs_auth, measured_at
               FROM agent_capabilities",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| crate::core::AgentCapabilityRecord {
                command: r.0,
                agent_name: r.1,
                resume: r.2 != 0,
                load_session: r.3 != 0,
                list_sessions: r.4 != 0,
                declares_modes: r.5 != 0,
                needs_auth: r.6 != 0,
                measured_at: r.7.parse().unwrap_or_else(|_| jiff::Timestamp::now()),
            })
            .collect())
    }

    /// `INSERT OR REPLACE` on an opaque primary key, so the answer path is
    /// idempotent at the storage layer too: the row is the whole state and the
    /// pure [`Ask`](crate::core::ask::Ask) decides what may change.
    pub async fn save_ask(&self, a: &crate::core::ask::Ask) -> Result<()> {
        let deadline: Option<i64> = match a.deadline {
            crate::core::ask::Deadline::Never => None,
            crate::core::ask::Deadline::After(s) => Some(s as i64),
        };
        sqlx::query(
            "INSERT OR REPLACE INTO asks
               (id, kind, run_id, project_id, request_id, message, payload, asked_at,
                deadline_secs, answer, answered_at, answered_from, delivery, ended, ended_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(a.id.as_str())
        .bind(a.kind.as_str())
        .bind(a.run.as_str())
        .bind(a.project.as_ref().map(|p| p.as_str()))
        .bind(&a.request_id)
        .bind(&a.message)
        .bind(serde_json::to_string(&a.payload)?)
        .bind(a.asked_at.to_string())
        .bind(deadline)
        .bind(a.answer.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.answered_at.map(|t| t.to_string()))
        .bind(a.answered_from.as_deref())
        .bind(a.delivery.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.ended.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.ended_at.map(|t| t.to_string()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// One ask by its token.
    pub async fn ask(&self, id: &str) -> Result<Option<crate::core::ask::Ask>> {
        let row = sqlx::query("SELECT * FROM asks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().and_then(decode_ask))
    }

    /// Every ask still waiting for a person, oldest first.
    ///
    /// **Oldest first and not newest**, because this is a queue of things owed
    /// to somebody rather than a feed: the one that has been waiting longest is
    /// the one that has been failing longest.
    pub async fn open_asks(&self) -> Result<Vec<crate::core::ask::Ask>> {
        let rows = sqlx::query("SELECT * FROM asks WHERE ended IS NULL ORDER BY asked_at ASC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().filter_map(decode_ask).collect())
    }

    /// Recent asks whatever became of them, newest first — the surface that
    /// answers *who answered that one, and when*.
    pub async fn asks(&self, limit: i64) -> Result<Vec<crate::core::ask::Ask>> {
        let rows = sqlx::query("SELECT * FROM asks ORDER BY asked_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().filter_map(decode_ask).collect())
    }

    /// Every fan-out, newest first.
    ///
    /// A row this build cannot decode is **dropped and counted**, like every
    /// other projection here — not migrated, and not guessed at.
    pub async fn load_batches(&self, limit: i64) -> Result<Vec<crate::core::batch::Batch>> {
        let rows = sqlx::query("SELECT * FROM batches ORDER BY sent_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(crate::core::batch::Batch {
                    id: crate::core::BatchId::new(r.get::<String, _>("id")),
                    kind: match r.get::<String, _>("kind").as_str() {
                        "drafted" => crate::core::batch::Kind::Drafted,
                        "dispatched" => crate::core::batch::Kind::Dispatched,
                        _ => return None,
                    },
                    position: crate::core::batch::Position::parse(
                        r.get::<String, _>("position").as_str(),
                    )?,
                    prompt: r.get("prompt"),
                    template: r.get("template"),
                    sent_at: r.get::<String, _>("sent_at").parse().ok()?,
                    sent_by: r.get("sent_by"),
                    targets: serde_json::from_str(&r.get::<String, _>("targets")).ok()?,
                })
            })
            .collect())
    }

    /// One fan-out by id.
    pub async fn batch(&self, id: &str) -> Result<Option<crate::core::batch::Batch>> {
        Ok(self
            .load_batches(500)
            .await?
            .into_iter()
            .find(|b| b.id.as_str() == id))
    }

    pub async fn load_works(&self) -> Result<Vec<crate::core::Work>> {
        let rows = sqlx::query("SELECT id, payload FROM works ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("work", &rows))
    }

    /// How many stored rows this build can no longer read.
    ///
    /// Asked by `devplane doctor`, because the answer is otherwise invisible:
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
    /// The last thing the agent said on a run.
    ///
    /// For putting a claim beside the evidence: an agent's end-of-task report
    /// references about one action in eleven and drifts toward its plan as the
    /// run leaves it, so the report is worth reading **next to** a gate's exit
    /// code and worth very little on its own.
    ///
    /// `None` when there is no transcript — a run Devplane only watched, or a
    /// repository with `[transcripts] keep = false`. That is *nothing was
    /// recorded*, never *the agent said nothing*, and every surface that shows
    /// this has to keep the two apart.
    pub async fn last_agent_message(&self, run: &RunId) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT text FROM messages WHERE run_id = ? AND role = 'agent'
             ORDER BY id DESC LIMIT 1",
        )
        .bind(run.as_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.get::<String, _>("text")))
    }

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
               (id, at, authority, action, subject, outcome, reason, tool, server_source,
                project_id, run_id, work_id)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&d.id)
        .bind(d.at.to_string())
        .bind(d.authority.as_str())
        .bind(&d.action)
        .bind(&d.subject)
        .bind(&d.outcome)
        .bind(d.reason.as_deref())
        .bind(d.tool.as_deref())
        .bind(d.server_source.as_deref())
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
    /// How much of what happened in the person's name reached them.
    ///
    /// Three counts from two tables, and the seams matter:
    ///
    /// * **`unattended`** is tool calls — the `PreToolUse` half. Measured on
    ///   2026-09-19, this is everything: no session on the machine asked about
    ///   anything in forty-nine consecutive calls.
    /// * **`asked`** is the moments a person was actually put in the loop,
    ///   which arrive as a `blocked` event or a rule's `permission_decided`.
    /// * **`answered`** is what a person then did about it, from the decision
    ///   log rather than from the absence of a follow-up event — *nobody
    ///   answered* and *nothing was recorded* are different facts and only the
    ///   decision log can tell them apart.
    ///
    /// The window is a caller's, so `7 days` and `today` are the same query.
    pub async fn oversight(
        &self,
        since: jiff::Timestamp,
    ) -> Result<crate::core::attention::Oversight> {
        use sqlx::Row;
        let at = since.to_string();
        let unattended: i64 =
            sqlx::query("SELECT COUNT(*) AS n FROM events WHERE at >= ? AND kind = 'tool_started'")
                .bind(&at)
                .fetch_one(&self.pool)
                .await?
                .get("n");
        // **Both halves from one table, which is a correction.**
        //
        // `asked` was first counted from event kinds — `blocked` and
        // `permission_decided`. On a real machine that returned nought while
        // the attention log held sixty-nine raised permissions, and the
        // product printed *"not one was put in front of you"* underneath a
        // table showing sixty-nine. A driven run's permission never writes
        // those event kinds; it raises an item directly. The attention log is
        // where *a person was put in the loop* is actually recorded, and
        // counting the numerator and the denominator from two different
        // tables is how a ratio ends up contradicting the rows beneath it.
        let asked = sqlx::query(
            "SELECT COUNT(*) AS asked,
                    SUM(resolution = 'acted') AS answered
             FROM attention_log
             WHERE raised_at >= ? AND kind IN ('permission', 'question')",
        )
        .bind(&at)
        .fetch_one(&self.pool)
        .await?;
        Ok(crate::core::attention::Oversight {
            unattended,
            asked: asked.try_get("asked").unwrap_or(0),
            answered: asked.try_get("answered").unwrap_or(0),
        })
    }

    /// Which agents raised anything in the window.
    ///
    /// **Because a raise count is a fact about the model, not about the
    /// machine.** Implicit escalation thresholds differ markedly by model
    /// family and self-estimates are miscalibrated in model-specific ways
    /// ([arXiv:2604.08588]), so *how often did something need me* compared
    /// across a week when the vendor mix changed is comparing two escalation
    /// policies and calling it a trend.
    ///
    /// The surface says so only when it is true — when more than one agent is
    /// behind the numbers — because a caveat printed on every run is a caveat
    /// nobody reads.
    ///
    /// A row whose run has been pruned contributes nothing rather than an
    /// `unknown` agent: the question is *did more than one vendor produce
    /// these*, and a missing join cannot answer it either way.
    ///
    /// [arXiv:2604.08588]: https://arxiv.org/abs/2604.08588
    pub async fn agents_behind_attention(&self, since: jiff::Timestamp) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT r.agent FROM attention_log a
               JOIN runs r ON r.id = a.run_id
              WHERE a.raised_at >= ? AND r.agent != ''
              ORDER BY r.agent",
        )
        .bind(since.to_string())
        .fetch_all(&self.pool)
        .await?;
        use sqlx::Row;
        Ok(rows.iter().map(|r| r.get::<String, _>("agent")).collect())
    }

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
                    SUM(resolved_at IS NULL)      AS open,
                    SUM(folded_at IS NOT NULL)    AS folded,
                    SUM(folded_at IS NOT NULL AND resolution = 'acted') AS folded_then_acted
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
                        folded: r.try_get("folded").unwrap_or(0),
                        folded_then_acted: r.try_get("folded_then_acted").unwrap_or(0),
                    },
                )
            })
            .collect())
    }

    /// The decision log, newest first, optionally about one run or one work
    /// item.
    /// When somebody last read the inbox on this machine.
    /// Stamps the items a read folded, so folding can be measured.
    ///
    /// **Only on a read.** The board polls every couple of seconds; counting a
    /// fold per render would measure polling, which is the same mistake the
    /// boundary mark exists to avoid. First fold wins — `folded_at` is the
    /// moment a kind was first summarised rather than listed, and overwriting
    /// it on every read would turn a measurement into a timestamp of the last
    /// time anybody looked.
    pub async fn mark_folded(&self, ids: &[crate::core::AttentionId]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let list: Vec<String> = ids.iter().map(ToString::to_string).collect();
        sqlx::query(
            "UPDATE attention_log SET folded_at = ?
             WHERE resolved_at IS NULL AND folded_at IS NULL
               AND item_id IN (SELECT value FROM json_each(?))",
        )
        .bind(jiff::Timestamp::now().to_string())
        .bind(serde_json::to_string(&list)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn last_look(&self) -> Option<jiff::Timestamp> {
        sqlx::query("SELECT at FROM looks WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .and_then(|r| r.get::<String, _>("at").parse().ok())
    }

    /// Records that the inbox was read.
    ///
    /// **Called on a dwell, never on a render.** See the table's own comment:
    /// advancing this on every paint erases the boundary it exists to draw.
    pub async fn mark_look(&self, at: jiff::Timestamp) -> Result<()> {
        sqlx::query(
            "INSERT INTO looks (id, at) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET at = ?",
        )
        .bind(at.to_string())
        .bind(at.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// One day's decisions and the day's questions, for the close.
    ///
    /// **Bounded by the day rather than by a count**, because the tally is a
    /// statement about a day and a `LIMIT` would silently make it a statement
    /// about the most recent N of it. The index on `at` is what makes that
    /// cheap.
    pub async fn day(&self, since: &str) -> Result<(Vec<crate::core::Decision>, u32, u64)> {
        let rows =
            sqlx::query("SELECT * FROM decisions WHERE at >= ? ORDER BY at DESC, rowid DESC")
                .bind(since)
                .fetch_all(&self.pool)
                .await?;
        let decisions: Vec<crate::core::Decision> =
            rows.iter().filter_map(decode_decision).collect();

        // Questions that waited for somebody at any point today, and the
        // longest any one of them waited. An ask still open is counted with the
        // wait it has accrued so far, because a question that has been waiting
        // six hours is the most interesting row on the page.
        let asks = sqlx::query(
            "SELECT asked_at, ended_at FROM asks WHERE asked_at >= ? ORDER BY asked_at ASC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        let now = jiff::Timestamp::now();
        let mut longest = 0u64;
        for r in &asks {
            let Ok(start) = r.get::<String, _>("asked_at").parse::<jiff::Timestamp>() else {
                continue;
            };
            let end = r
                .get::<Option<String>, _>("ended_at")
                .and_then(|t| t.parse::<jiff::Timestamp>().ok())
                .unwrap_or(now);
            longest = longest.max(u64::try_from(end.as_second() - start.as_second()).unwrap_or(0));
        }
        Ok((
            decisions,
            u32::try_from(asks.len()).unwrap_or(u32::MAX),
            longest,
        ))
    }

    pub async fn decisions(
        &self,
        about: Option<&str>,
        limit: i64,
    ) -> Result<Vec<crate::core::Decision>> {
        let rows = match about {
            Some(id) => sqlx::query(
                "SELECT * FROM decisions WHERE run_id = ? OR work_id = ?
                 ORDER BY at DESC, rowid DESC LIMIT ?",
            )
            .bind(id)
            .bind(id)
            .bind(limit),
            None => sqlx::query("SELECT * FROM decisions ORDER BY at DESC, rowid DESC LIMIT ?")
                .bind(limit),
        }
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().filter_map(decode_decision).collect())
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
        n += sqlx::query("DELETE FROM projects WHERE root LIKE '%/devplane-probe-%'")
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
        // have been going all along: it is an observation about what Devplane
        // *showed*, one row per item raised, and nothing was removing it.
        // `devplane attention` only ever looks back a fixed number of days,
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
             `devplane doctor` lists them; deleting the database rebuilds it from the providers"
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
        E::ToolStarted { tool, input, .. } => {
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

/// One `asks` row back into the pure type.
///
/// A row this build cannot decode is **dropped**, like every other projection
/// here — never migrated and never guessed at. Dropping an ask loses a
/// question, which is why every field that decides *whether it is still open*
/// is required and only the descriptive ones are tolerant.
/// One `decisions` row as SQLite hands it back.
///
/// **An unrecognised authority drops the row rather than defaulting it.**
/// Reading it as `daemon` would put the most reassuring label on the least
/// known row, in the one table a person queries by exactly this column.
fn decode_decision(r: &sqlx::sqlite::SqliteRow) -> Option<crate::core::Decision> {
    use sqlx::Row as _;
    Some(crate::core::Decision {
        id: r.get("id"),
        at: r
            .get::<String, _>("at")
            .parse()
            .unwrap_or_else(|_| jiff::Timestamp::now()),
        // An unrecognised authority is **dropped**, not defaulted.
        // Reading it as `daemon` would put the most reassuring label
        // on the least known row, in the one table a person queries
        // by exactly this column.
        authority: crate::core::Authority::parse(r.get::<String, _>("authority").as_str())?,
        action: r.get("action"),
        subject: r.get("subject"),
        outcome: r.get("outcome"),
        reason: r.get("reason"),
        tool: r.get("tool"),
        server_source: r.get("server_source"),
        project_id: r.get::<Option<String>, _>("project_id").map(ProjectId::new),
        run_id: r.get::<Option<String>, _>("run_id").map(RunId::new),
        work_id: r
            .get::<Option<String>, _>("work_id")
            .map(crate::core::WorkId::new),
    })
}

fn decode_ask(r: &sqlx::sqlite::SqliteRow) -> Option<crate::core::ask::Ask> {
    use crate::core::ask::{Ask, Deadline};
    let deadline = match r.get::<Option<i64>, _>("deadline_secs") {
        None => Deadline::Never,
        Some(s) if s > 0 => Deadline::After(s as u32),
        // A stored zero or a negative is not a deadline anybody could have
        // written through the parser, so the row is dropped rather than read as
        // "already expired" — which would end somebody's question on a number
        // nothing produced.
        Some(_) => return None,
    };
    Some(Ask {
        id: crate::core::AskId::new(r.get::<String, _>("id")),
        kind: crate::core::ask::Kind::parse(r.get::<String, _>("kind").as_str())?,
        run: crate::core::RunId::new(r.get::<String, _>("run_id")),
        project: r
            .get::<Option<String>, _>("project_id")
            .map(crate::core::ProjectId::new),
        request_id: r.get("request_id"),
        message: r.get("message"),
        payload: serde_json::from_str(&r.get::<String, _>("payload")).ok()?,
        asked_at: r.get::<String, _>("asked_at").parse().ok()?,
        deadline,
        answer: r
            .get::<Option<String>, _>("answer")
            .and_then(|s| serde_json::from_str(&s).ok()),
        answered_at: r
            .get::<Option<String>, _>("answered_at")
            .and_then(|s| s.parse().ok()),
        answered_from: r.get("answered_from"),
        delivery: r
            .get::<Option<String>, _>("delivery")
            .and_then(|s| serde_json::from_str(&s).ok()),
        ended: r
            .get::<Option<String>, _>("ended")
            .and_then(|s| serde_json::from_str(&s).ok()),
        ended_at: r
            .get::<Option<String>, _>("ended_at")
            .and_then(|s| s.parse().ok()),
    })
}

#[cfg(test)]
mod tests {

    /// **The ratio and the table under it must come from the same place.**
    ///
    /// `asked` was first counted from event kinds. On a real machine that
    /// returned nought while the attention log held sixty-nine raised
    /// permissions, and the product printed *"not one of them was put in front
    /// of you"* directly above a table showing sixty-nine of them. A driven
    /// run's permission never writes those event kinds — it raises an item.
    ///
    /// Counting a numerator and a denominator from two tables is how a summary
    /// ends up contradicting its own rows, and a person who catches a product
    /// contradicting itself on screen is right to stop believing the rest.
    #[tokio::test]
    async fn the_oversight_ratio_agrees_with_the_table_beneath_it() {
        let s = Store::open_in_memory().await.unwrap();
        let long_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(1);

        // Three moments a person was put in the loop, through the path a
        // *driven* run uses — which writes no `blocked` event at all.
        let mut ids = Vec::new();
        for (n, kind) in [
            (0, crate::core::AttentionKind::Permission),
            (1, crate::core::AttentionKind::Permission),
            (2, crate::core::AttentionKind::Question),
        ] {
            let item = crate::core::AttentionItem {
                id: crate::core::AttentionId::from(format!("i{n}")),
                level: kind.default_level(),
                kind,
                run_id: None,
                project_id: None,
                title: "t".into(),
                detail: None,
                answer_in: None,
                ask: None,
                options: Vec::new(),
                actions: Vec::new(),
                request_id: None,
                form: None,
                url: None,
                launch: None,
                work_id: None,
                offer: None,
                no_offer: None,
                since: jiff::Timestamp::now(),
            };
            s.attention_raise(&item).await.unwrap();
            ids.push(format!("i{n}"));
        }
        // A person answered exactly one of them.
        s.attention_resolve(&ids[2..], crate::core::attention::Resolution::Acted)
            .await
            .unwrap();

        let o = s.oversight(long_ago).await.unwrap();
        let stats = s.attention_stats(long_ago).await.unwrap();

        let raised: i64 = stats.values().map(|k| k.raised).sum();
        let acted: i64 = stats.values().map(|k| k.acted).sum();
        assert_eq!(
            o.asked, raised,
            "the ratio says {} were put in front of a person and the table says {raised}",
            o.asked
        );
        assert_eq!(
            o.answered, acted,
            "the ratio says {} were answered and the table says {acted}",
            o.answered
        );
        assert_eq!(o.answered, 1);
        // And the unattended half is the tool calls, which nothing raised.
        assert_eq!(o.unattended, 0, "nothing ran, so nothing ran unattended");
        assert_eq!(o.total(), 3);
    }
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
                Event::tool_started(
                    format!("Tool{i}"),
                    serde_json::json!({"command": format!("cmd {i}")}),
                ),
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
            Event::tool_started("Bash", serde_json::json!({"command": "pnpm typecheck"})),
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
        // `devplane search "cargo test --workspace"` would otherwise reach FTS5 as an
        // expression and come back as a 500.
        let s = Store::open_in_memory().await.unwrap();
        s.append_event(&EventEnvelope::new(
            RunId::new("s1"),
            Source::Hook,
            Event::tool_started(
                "Bash",
                serde_json::json!({"command": "cargo test --workspace"}),
            ),
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
            Event::tool_started("Bash", serde_json::json!({"command": "pnpm typecheck"})),
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
            Event::tool_started("Bash", serde_json::json!({"command": "pnpm build"})),
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
            answer_in: None,
            options: vec![],
            actions: vec![],
            ask: None,
            request_id: None,
            form: None,
            url: None,
            launch: None,
            work_id: None,
            offer: None,
            no_offer: None,
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
            answer_in: None,
            project_id: None,
            run_id: Some(RunId::new("r1")),
            work_id: None,
            offer: None,
            no_offer: None,
            since: jiff::Timestamp::now(),
            options: Vec::new(),
            actions: Vec::new(),
            launch: None,
            ask: None,
            request_id: None,
            form: None,
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
    async fn a_database_from_another_schema_is_moved_aside_rather_than_migrated() {
        // **The hard cut, asserted.** This used to be a test that an older
        // file kept working: the schema grew columns through `ALTER TABLE`
        // statements whose success case was a duplicate-column error. That
        // handles exactly the change it was written for — an added, nullable
        // column — and silently does nothing for a renamed one, which is what
        // `actor` → `authority` is. The old file then opens cleanly and fails
        // on the first write, at runtime, in whichever surface wrote first.
        //
        // So: a file stamped with a different schema is renamed out of the way
        // and a fresh one takes its place. Observations are re-derivable; the
        // decision log is not, which is why the old file is **moved and not
        // deleted**, and why this test checks that it is still on disk.
        let dir = std::env::temp_dir().join(format!("devplane-schema-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("devplane.db");

        {
            let opts = SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(opts)
                .await
                .unwrap();
            sqlx::raw_sql(
                // **Any version but this build's.** It said `1`, which was a
                // real older shape at the time and is this build's own version
                // now — a fixture that collides with the thing it is testing.
                "PRAGMA user_version = 99;
                 CREATE TABLE decisions (
                   id TEXT PRIMARY KEY, at TEXT NOT NULL, actor TEXT NOT NULL,
                   action TEXT NOT NULL, subject TEXT NOT NULL, outcome TEXT NOT NULL,
                   reason TEXT, project_id TEXT, run_id TEXT, work_id TEXT);
                 INSERT INTO decisions (id, at, actor, action, subject, outcome)
                 VALUES ('d1', '2026-09-15T00:00:00Z', 'policy', 'agent:tool.use', 'cat a', 'allow');",
            )
            .execute(&pool)
            .await
            .unwrap();
            pool.close().await;
        }

        let s = Store::open(&path).await.unwrap();
        // The new database is usable immediately, under the new column.
        s.append_decision(
            &crate::core::Decision::new(
                crate::core::Authority::Nobody,
                "agent:question",
                "req-1",
                "unanswered",
            )
            .for_run(&RunId::new("s1")),
        )
        .await
        .unwrap();
        let rows = s.decisions(None, 10).await.unwrap();
        assert_eq!(rows.len(), 1, "the new file starts empty: {rows:?}");
        assert_eq!(rows[0].authority, crate::core::Authority::Nobody);

        assert!(
            dir.join("devplane.v99.bak").exists(),
            "the old file is kept — a decision log is the one thing here that \
             cannot be re-derived, so it is moved and never deleted"
        );

        // And opening again is idempotent: the file now carries this schema's
        // version, so nothing is moved a second time and the row just written
        // is still there.
        let s = Store::open(&path).await.unwrap();
        assert_eq!(s.decisions(None, 10).await.unwrap().len(), 1);

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
                crate::core::Authority::Rule,
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
            crate::core::Authority::Daemon,
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

    /// **Decisions taken in the same instant come back in the order they were
    /// taken.**
    ///
    /// `at` ties constantly — a gate finishing and the run it was about ending
    /// share a second — and under `ORDER BY at DESC` alone SQLite may return
    /// either first. This surfaced as the test above passing alone and failing
    /// under parallel load; the real cost is an audit page showing two
    /// decisions in the wrong sequence, in the one table whose purpose is
    /// saying what happened in what order.
    ///
    /// The tie-break is `rowid`, which is the append order. The row's own id
    /// cannot serve: it is a uuid v7 whose head is a millisecond and whose tail
    /// is random, so two rows written in the same millisecond sort by the
    /// random part — which is not an order at all.
    #[tokio::test]
    async fn decisions_taken_in_one_instant_keep_the_order_they_were_taken_in() {
        let s = Store::open_in_memory().await.unwrap();
        let at = jiff::Timestamp::now();
        for n in 0..8 {
            let mut d = crate::core::Decision::new(
                crate::core::Authority::Daemon,
                &format!("step:{n}"),
                "subject",
                "done",
            );
            // The same stamp on every row, which is what a fast sequence
            // produces anyway — this only makes it certain rather than likely.
            d.at = at;
            s.append_decision(&d).await.unwrap();
        }
        let back = s.decisions(None, 10).await.unwrap();
        let order: Vec<&str> = back.iter().map(|d| d.action.as_str()).collect();
        assert_eq!(
            order,
            [
                "step:7", "step:6", "step:5", "step:4", "step:3", "step:2", "step:1", "step:0"
            ],
            "eight decisions sharing one timestamp did not come back newest-first"
        );
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

#[cfg(test)]
mod ask_store_tests {
    use super::*;

    #[tokio::test]
    async fn an_ask_round_trips_and_open_means_unanswered() {
        let s = Store::open_in_memory().await.unwrap();
        let a = crate::core::ask::Ask::new(
            crate::core::AskId::new("a1"),
            crate::core::RunId::new("r1"),
            crate::core::ask::Asked {
                kind: crate::core::ask::Kind::Question,
                request_id: "req".into(),
                message: "Keep it?".into(),
                payload: serde_json::json!({"x": 1}),
                at: jiff::Timestamp::now(),
                deadline: crate::core::ask::Deadline::After(3600),
            },
        );
        s.save_ask(&a).await.unwrap();
        let back = s.ask("a1").await.unwrap().expect("stored");
        assert_eq!(back, a);
        assert_eq!(s.open_asks().await.unwrap().len(), 1);
    }
}

//! The store: SQLite, WAL, one file.
//!
//! It holds observations (events and the runs projected from them, which the
//! providers can say again) and the record (changes, asks, decisions, agent
//! capabilities, inbox reads), which nothing can say again; `schema.sql` says
//! which table is which. Timestamps are compared as text, so every one is
//! written through [`ts`] at a fixed width.

use crate::core::event::EventEnvelope;
use crate::core::ids::{ProjectId, RunId};
use crate::core::project::Project;
use crate::core::run::Run;
use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::str::FromStr;

/// The shape of `schema.sql`, stamped into every database this build writes.
///
/// Bump it whenever `schema.sql` changes in a way an older file would not
/// satisfy (a renamed or added column, a changed type, a dropped table). A file
/// stamped with anything else is moved aside on open (`Store::retire`), not
/// migrated.
pub const SCHEMA_VERSION: i64 = 11;

/// The statement that stamps it, written out because `PRAGMA user_version`
/// accepts no bind parameter.
const SCHEMA_VERSION_PRAGMA: &str = "PRAGMA user_version = 11";

/// The database and its sidecars readable by their owner alone, whoever
/// opened them first — a hook creates the file long before any `serve` would
/// tighten the home. Best effort: a file this process does not own is left.
fn owner_only(path: &Path) {
    #[cfg(unix)]
    for suffix in ["", "-wal", "-shm"] {
        use std::os::unix::fs::PermissionsExt;
        let file = PathBuf::from(format!("{}{suffix}", path.display()));
        let is_file = file
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_file() && m.permissions().mode() & 0o077 != 0);
        if is_file {
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Deletes a retired database that holds nothing: the file is empty and no
/// write-ahead log beside it holds a byte. Anything else is kept.
fn remove_if_empty(bak: &Path) {
    let len = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    let wal = PathBuf::from(format!("{}-wal", bak.display()));
    if len(bak) != Some(0) || len(&wal).is_some_and(|n| n > 0) {
        return;
    }
    if std::fs::remove_file(bak).is_ok() {
        tracing::info!(backup = %bak.display(), "an empty retired database was deleted");
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", bak.display()));
        }
    }
}

/// A handle on the observation store.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens the store, creating it if needed.
    pub async fn open(path: &Path) -> Result<Self> {
        Self::open_waiting(path, std::time::Duration::from_secs(5)).await
    }

    /// Opens the store, waiting at most `busy` for another writer's lock. The
    /// hook passes a short one: it runs under the vendor's timeout, and a locked
    /// database must cost the record (which is spooled), never the answer.
    pub async fn open_waiting(path: &Path, busy: std::time::Duration) -> Result<Self> {
        let store = Self::open_file(path, busy).await?;
        owner_only(path);
        Ok(store)
    }

    async fn open_file(path: &Path, busy: std::time::Duration) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        // `filename`, not a formatted `sqlite://` URL: a `?` or `#` in the path
        // would become query parameters.
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(busy);
        // Connecting sets the journal mode, which SQLite refuses at once —
        // without the busy handler — while another opener is re-creating the
        // file under the retire lock, and a file renamed away mid-connect
        // answers an I/O error; both are retried, within the same `busy` a
        // query would wait.
        let connect = || {
            let opts = opts.clone();
            async move {
                let until = std::time::Instant::now() + busy;
                loop {
                    match SqlitePoolOptions::new()
                        .max_connections(8)
                        .connect_with(opts.clone())
                        .await
                    {
                        Err(e)
                            if ["database is locked", "disk I/O error"]
                                .iter()
                                .any(|m| e.to_string().contains(m))
                                && std::time::Instant::now() < until =>
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        }
                        other => break other,
                    }
                }
            }
        };
        // The version is read on a read-only connection that neither creates
        // the file nor changes its journal: an opener racing a retire must not
        // touch the old file, or a fresh one appearing at its path.
        let probe = || {
            let opts = SqliteConnectOptions::new()
                .filename(path)
                .read_only(true)
                .busy_timeout(busy);
            async move {
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(opts)
                    .await
                    .ok()?;
                let v = sqlx::query_as::<_, (i64,)>("PRAGMA user_version")
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten()
                    .map(|(n,)| n);
                pool.close().await;
                v
            }
        };
        if path.exists() {
            match probe().await {
                // Complete: the stamp is written after the schema applies.
                Some(SCHEMA_VERSION) => {
                    let pool = connect()
                        .await
                        .with_context(|| format!("opening {}", path.display()))?;
                    return Ok(Self { pool });
                }
                // Written by a different schema: moved aside, never migrated
                // or reused. Parallel tool calls fire concurrent hooks; one
                // retires under an exclusive lock and the rest re-read after.
                Some(v) if v != 0 => {
                    let lock = Self::retire_lock(path).await?;
                    match probe().await {
                        Some(SCHEMA_VERSION) => {
                            drop(lock);
                            let pool = connect()
                                .await
                                .with_context(|| format!("opening {}", path.display()))?;
                            return Ok(Self { pool });
                        }
                        Some(v) if v != 0 => Self::retire(path, v)?,
                        _ => {}
                    }
                    let pool = connect()
                        .await
                        .with_context(|| format!("opening {}", path.display()))?;
                    let store = Self { pool };
                    // Applied under the lock too, so no opener sees a half-made file.
                    let migrated = store.migrate().await;
                    drop(lock);
                    migrated?;
                    return Ok(store);
                }
                // Empty, or being created by another opener: the schema is
                // idempotent.
                _ => {}
            }
        }
        let pool = connect()
            .await
            .with_context(|| format!("opening {}", path.display()))?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Takes the exclusive lock that serialises retiring, on a file next to
    /// the database. Released when the returned handle drops (and by the OS if
    /// the process dies).
    async fn retire_lock(path: &Path) -> Result<std::fs::File> {
        let at = PathBuf::from(format!("{}.lock", path.display()));
        tokio::task::spawn_blocking(move || -> Result<std::fs::File> {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&at)
                .with_context(|| format!("opening {}", at.display()))?;
            file.lock()
                .with_context(|| format!("locking {}", at.display()))?;
            Ok(file)
        })
        .await
        .context("waiting for the retire lock")?
    }

    /// Renames a database written by a different schema out of the way.
    ///
    /// There is no migration machinery. Observations cost a replay to rebuild;
    /// the record tables are why the old file is moved rather than deleted —
    /// moved aside, named by the schema version and the retire time, and never
    /// deleted afterwards. The user is told where it went. Called only under
    /// [`Self::retire_lock`].
    fn retire(path: &Path, found: i64) -> Result<()> {
        // A name no earlier retire used: renaming onto an existing backup
        // would destroy the only copy of an older record.
        let stamp = jiff::Timestamp::now()
            .strftime("%Y%m%dT%H%M%SZ")
            .to_string();
        let mut aside = path.with_extension(format!("v{found}.{stamp}.bak"));
        let mut n = 1;
        while aside.exists() {
            aside = path.with_extension(format!("v{found}.{stamp}-{n}.bak"));
            n += 1;
        }
        // Both sidecars go first, or the new database inherits a stale journal
        // — and while the old file still stands at `path`, no opener creates a
        // new one there whose journal this would carry off.
        for suffix in ["-wal", "-shm"] {
            let from = PathBuf::from(format!("{}{suffix}", path.display()));
            if from.exists() {
                let _ = std::fs::rename(&from, format!("{}{suffix}", aside.display()));
            }
        }
        std::fs::rename(path, &aside).with_context(|| {
            format!(
                "moving a database written by schema v{found} aside to {}",
                aside.display()
            )
        })?;
        tracing::warn!(
            schema_found = found,
            schema_expected = SCHEMA_VERSION,
            moved_to = %aside.display(),
            "the database was written by a different schema and has been moved aside; \
             observations will be rebuilt from the providers; the record tables — changes, \
             asks, decisions, reports — are in the old file and nowhere else"
        );
        Self::drop_empty_backups(path);
        Ok(())
    }

    /// Deletes the retired databases that hold nothing. Every other one is
    /// kept — moved aside, never deleted — whatever its age: the record tables
    /// in it are the only copy. Nothing is ranked, so no clock decides.
    fn drop_empty_backups(path: &Path) {
        let (Some(dir), Some(stem)) = (
            path.parent(),
            path.file_stem().map(|s| s.to_string_lossy().to_string()),
        ) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let prefix = format!("{stem}.v");
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".bak") {
                remove_if_empty(&entry.path());
            }
        }
    }

    /// An in-memory store for tests, configured exactly like the real one
    /// (foreign keys included).
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

    /// Applies the schema and stamps its version. A file written by a
    /// different schema never gets here: [`Self::retire`] moved it aside.
    async fn migrate(&self) -> Result<()> {
        // `raw_sql` runs the whole file in one round trip; splitting on `;`
        // by hand risks losing a statement.
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await
            .context("applying schema")?;
        // Stamped after the schema applies, so a half-created file is never
        // stamped as complete.
        sqlx::query(SCHEMA_VERSION_PRAGMA)
            .execute(&self.pool)
            .await
            .context("stamping the schema version")?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Appends an event and indexes anything searchable in it, in one
    /// transaction so a crash cannot leave an event search never finds.
    pub async fn append_event(&self, env: &EventEnvelope) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT OR IGNORE INTO events (id, at, run_id, project_id, source, kind, payload)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&env.id)
        .bind(ts(&env.at))
        .bind(env.run_id.as_str())
        .bind(env.project_id.as_ref().map(|p| p.as_str()))
        .bind(env.source.as_str())
        .bind(env.event.label())
        .bind(serde_json::to_string(&env.event)?)
        .execute(&mut *tx)
        .await?;

        if let Some(text) = searchable_text(&env.event) {
            // Indexed only when the event was new, so a replayed event is not
            // findable twice.
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

    /// Events past `after` from the sources short-lived processes write, oldest
    /// first: the rows the host has yet to fold into `runs`. Ordered by `seq`,
    /// not `at`, because two processes' clocks may disagree.
    pub async fn shim_events_since(
        &self,
        after: i64,
        limit: i64,
    ) -> Result<Vec<(i64, EventEnvelope)>> {
        let rows = sqlx::query(
            "SELECT seq, id, at, run_id, project_id, source, payload FROM events
             WHERE seq > ? AND source IN ('hook', 'copilot_hook', 'codex_hook', 'statusline')
             ORDER BY seq ASC LIMIT ?",
        )
        .bind(after)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("reading events to project")?;
        Ok(decode_rows("event", &rows, |row| {
            let seq: i64 = row.try_get("seq")?;
            Ok((seq, row_to_event(row)?))
        }))
    }

    /// The last event `seq` the host folded into `runs`. Zero on a fresh store.
    pub async fn projected_through(&self) -> Result<i64> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT through FROM projection WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .context("reading the projection mark")?;
        Ok(row.map(|(n,)| n).unwrap_or(0))
    }

    pub async fn set_projected_through(&self, seq: i64) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO projection (id, through) VALUES (1, ?)")
            .bind(seq)
            .execute(&self.pool)
            .await
            .context("writing the projection mark")?;
        Ok(())
    }

    /// Every event for a run, oldest first (uuid v7 ids sort by time).
    pub async fn events_for_run(&self, run: &RunId, limit: i64) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            "SELECT id, at, run_id, project_id, source, payload FROM events
             WHERE run_id = ? ORDER BY id ASC LIMIT ?",
        )
        .bind(run.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        // An unreadable row drops that row, not the whole replay.
        Ok(decode_rows("event", &rows, row_to_event))
    }

    /// Every tool call observed on this machine, newest first, with the
    /// directory whose rules govern it (from the run; calls whose run is pruned
    /// are dropped). Only tools a path or command rule can speak for.
    ///
    /// `under` scopes to one directory tree inside the query, so the limit is
    /// not spent on other projects' calls first.
    pub async fn observed_tool_calls(
        &self,
        under: Option<&Path>,
        limit: i64,
    ) -> Result<Vec<ObservedCall>> {
        // The escape keeps a `%` or `_` in a directory name from being a wildcard.
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

    /// Full-text search across tool commands, questions and summaries. The
    /// query is treated as words, not an FTS5 expression (see `fts_phrase`).
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

    /// Writes the run projection; one upsert, called after every applied event.
    pub async fn save_run(&self, run: &Run) -> Result<()> {
        sqlx::query(
            "INSERT INTO runs (id, agent, cwd, last_event_at, payload)
             VALUES (?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
               agent=excluded.agent, cwd=excluded.cwd,
               last_event_at=excluded.last_event_at, payload=excluded.payload",
        )
        .bind(run.id.as_str())
        .bind(&run.agent)
        .bind(run.cwd.to_string_lossy().to_string())
        .bind(ts(&run.last_event_at))
        .bind(serde_json::to_string(run)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_runs(&self) -> Result<Vec<Run>> {
        let rows = sqlx::query("SELECT id, payload FROM runs ORDER BY last_event_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("run", &rows, payload))
    }

    pub async fn save_project(&self, p: &Project) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, root, trusted, repo_url, auto_discovered)
             VALUES (?,?,?,?,?,?)
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
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records a project something saw (a hook resolving a directory, the host
    /// learning a remote) without touching `trusted`. Inserts a new row, and on
    /// an existing one only fills a missing remote, so a hook's stale read can
    /// never revoke a trust granted in between.
    pub async fn note_project(&self, p: &Project) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, root, trusted, repo_url, auto_discovered)
             VALUES (?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
               repo_url = COALESCE(projects.repo_url, excluded.repo_url)",
        )
        .bind(p.id.as_str())
        .bind(&p.name)
        .bind(p.root.to_string_lossy().to_string())
        .bind(p.trusted as i32)
        .bind(p.repo_url.as_deref())
        .bind(p.auto_discovered as i32)
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

    pub async fn save_change(&self, w: &crate::core::Change) -> Result<()> {
        sqlx::query(
            "INSERT INTO changes (id, updated_at, payload) VALUES (?,?,?)
             ON CONFLICT(id) DO UPDATE SET
               updated_at=excluded.updated_at, payload=excluded.payload",
        )
        .bind(w.id.as_str())
        .bind(ts(&w.updated_at))
        .bind(serde_json::to_string(w)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Asks ────────────────────────────────────────────────────────────
    //
    // Not rebuildable: if an ask's row is lost, the question is lost with it.

    /// Records what an agent said it could do, the last time it was started.
    /// Overwrites: the current answer is the interesting one, and the date
    /// says when it was true.
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
        .bind(ts(&c.measured_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Notes that this agent declared a session mode. A separate write because
    /// it is only visible when a session is created, later than `initialize`.
    pub async fn note_agent_declares_modes(&self, command: &str) -> Result<()> {
        sqlx::query("UPDATE agent_capabilities SET declares_modes = 1 WHERE command = ?")
            .bind(command)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Everything measured so far. An agent with no row was never started,
    /// which the caller reports as *not probed*.
    pub async fn agent_capabilities(&self) -> Result<Vec<crate::core::AgentCapabilityRecord>> {
        let rows = sqlx::query(
            "SELECT command AS id, command, agent_name, resume, load_session, list_sessions,
                    declares_modes, needs_auth, measured_at
               FROM agent_capabilities",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(decode_rows("agent_capabilities", &rows, |r| {
            Ok(crate::core::AgentCapabilityRecord {
                command: r.get("command"),
                agent_name: r.get("agent_name"),
                resume: r.get::<i64, _>("resume") != 0,
                load_session: r.get::<i64, _>("load_session") != 0,
                list_sessions: r.get::<i64, _>("list_sessions") != 0,
                declares_modes: r.get::<i64, _>("declares_modes") != 0,
                needs_auth: r.get::<i64, _>("needs_auth") != 0,
                measured_at: stamp(r, "measured_at")?,
            })
        }))
    }

    /// Writes an ask as it is raised. Not for closing one: that goes through
    /// [`Self::close_ask`], because a whole-row replace from a stale copy could
    /// overwrite an answer another surface just wrote.
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
        .bind(ts(&a.asked_at))
        .bind(deadline)
        .bind(a.answer.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.answered_at.as_ref().map(ts))
        .bind(a.answered_from.as_deref())
        .bind(a.delivery.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.ended.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.ended_at.as_ref().map(ts))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Closes an ask only if it is still open (compare-and-set). `true` when
    /// this call closed it, `false` when somebody else already had; so two
    /// surfaces answering get one answer and a sweep never overwrites one.
    pub async fn close_ask(&self, a: &crate::core::ask::Ask) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE asks SET answer = ?, answered_at = ?, answered_from = ?, ended = ?,
                    ended_at = ?, delivery = ?
              WHERE id = ? AND answer IS NULL AND ended IS NULL",
        )
        .bind(a.answer.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.answered_at.as_ref().map(ts))
        .bind(a.answered_from.as_deref())
        .bind(a.ended.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.ended_at.as_ref().map(ts))
        .bind(a.delivery.as_ref().map(serde_json::to_string).transpose()?)
        .bind(a.id.as_str())
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1)
    }

    /// Records what became of delivering an answer, on a row already closed.
    pub async fn set_ask_delivery(
        &self,
        id: &str,
        delivery: &crate::core::ask::Delivery,
    ) -> Result<()> {
        sqlx::query("UPDATE asks SET delivery = ? WHERE id = ?")
            .bind(serde_json::to_string(delivery)?)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// One ask by its token.
    pub async fn ask(&self, id: &str) -> Result<Option<crate::core::ask::Ask>> {
        let rows = sqlx::query("SELECT * FROM asks WHERE id = ?")
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("ask", &rows, decode_ask).pop())
    }

    /// Every ask still waiting for a person, oldest first: it is a queue of
    /// things owed, not a feed.
    pub async fn open_asks(&self) -> Result<Vec<crate::core::ask::Ask>> {
        let rows = sqlx::query("SELECT * FROM asks WHERE ended IS NULL ORDER BY asked_at ASC")
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("ask", &rows, decode_ask))
    }

    /// Recent asks whatever became of them, newest first.
    pub async fn asks(&self, limit: i64) -> Result<Vec<crate::core::ask::Ask>> {
        let rows = sqlx::query("SELECT * FROM asks ORDER BY asked_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("ask", &rows, decode_ask))
    }

    pub async fn load_changes(&self) -> Result<Vec<crate::core::Change>> {
        let rows = sqlx::query("SELECT id, payload FROM changes ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("change", &rows, payload))
    }

    // ── Reports ─────────────────────────────────────────────────────────

    /// Writes a report, and every later state of it.
    pub async fn save_report(&self, r: &crate::core::report::Report) -> Result<()> {
        let target = match &r.target {
            crate::core::report::Target::Project { project, .. } => Some(project.as_str()),
            _ => None,
        };
        sqlx::query(
            "INSERT OR REPLACE INTO reports
               (id, filed_at, source_project, target_project, state, payload)
             VALUES (?,?,?,?,?,?)",
        )
        .bind(r.id.as_str())
        .bind(ts(&r.provenance.at))
        .bind(r.provenance.project.as_str())
        .bind(target)
        .bind(r.state.as_str())
        .bind(serde_json::to_string(r)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// One report by id or id prefix, since people copy the part a table printed.
    pub async fn report(&self, id: &str) -> Result<Option<crate::core::report::Report>> {
        let rows = sqlx::query("SELECT id, payload FROM reports WHERE id = ?")
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        if let Some(r) = decode_rows("report", &rows, payload).pop() {
            return Ok(Some(r));
        }
        let rows = sqlx::query("SELECT id, payload FROM reports WHERE id LIKE ? || '%' LIMIT 2")
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        let mut hits = decode_rows("report", &rows, payload);
        Ok(match hits.len() {
            1 => hits.pop(),
            _ => None,
        })
    }

    /// Every report addressed to a project, newest first; only the open ones
    /// when asked.
    pub async fn reports_to(
        &self,
        project: &crate::core::ProjectId,
        open_only: bool,
    ) -> Result<Vec<crate::core::report::Report>> {
        let rows = sqlx::query(
            "SELECT id, payload FROM reports
              WHERE target_project = ? AND (? = 0 OR state = 'open')
              ORDER BY filed_at DESC",
        )
        .bind(project.as_str())
        .bind(open_only as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(decode_rows("report", &rows, payload))
    }

    /// Every report a project filed, newest first; only the unanswered ones
    /// when asked.
    pub async fn reports_from(
        &self,
        project: &crate::core::ProjectId,
        open_only: bool,
    ) -> Result<Vec<crate::core::report::Report>> {
        let rows = sqlx::query(
            "SELECT id, payload FROM reports
              WHERE source_project = ?
                AND (? = 0 OR state IN ('open', 'accepted', 'drafted'))
              ORDER BY filed_at DESC",
        )
        .bind(project.as_str())
        .bind(open_only as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(decode_rows("report", &rows, payload))
    }

    /// Every report, newest first, bounded — the inbox and the list read
    /// the whole set and ask the struct.
    pub async fn reports(&self, limit: i64) -> Result<Vec<crate::core::report::Report>> {
        let rows = sqlx::query("SELECT id, payload FROM reports ORDER BY filed_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        Ok(decode_rows("report", &rows, payload))
    }

    /// The reports a change raised or was started from, newest first. Read from
    /// the payload rather than a column.
    pub async fn reports_for_change(
        &self,
        change: &crate::core::ChangeId,
    ) -> Result<Vec<crate::core::report::Report>> {
        let rows = sqlx::query(
            "SELECT id, payload FROM reports
              WHERE json_extract(payload, '$.provenance.change') = ?
                 OR json_extract(payload, '$.state.change') = ?
              ORDER BY filed_at DESC",
        )
        .bind(change.as_str())
        .bind(change.as_str())
        .fetch_all(&self.pool)
        .await?;
        Ok(decode_rows("report", &rows, payload))
    }

    /// How many stored rows this build can no longer read, for `devplane
    /// doctor`: on the board they look exactly like rows that never existed.
    pub async fn unreadable(&self) -> Result<Vec<(String, String)>> {
        // Literal queries, one per table: sqlx refuses a `format!`ed table name.
        let mut out = Vec::new();
        let runs = sqlx::query("SELECT id, payload FROM runs")
            .fetch_all(&self.pool)
            .await?;
        out.extend(unreadable_in::<Run>("run", &runs));
        let changes = sqlx::query("SELECT id, payload FROM changes")
            .fetch_all(&self.pool)
            .await?;
        out.extend(unreadable_in::<crate::core::Change>("change", &changes));
        let reports = sqlx::query("SELECT id, payload FROM reports")
            .fetch_all(&self.pool)
            .await?;
        out.extend(unreadable_in::<crate::core::report::Report>(
            "report", &reports,
        ));
        Ok(out)
    }

    /// Appends a fragment of a driven run's transcript.
    pub async fn append_message(&self, m: &crate::core::Message) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO messages (id, run_id, at, role, text) VALUES (?,?,?,?,?)",
        )
        .bind(&m.id)
        .bind(m.run_id.as_str())
        .bind(ts(&m.at))
        .bind(m.role.as_str())
        .bind(&m.text)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The last thing the agent said on a run, to read beside a gate's exit
    /// code rather than on its own.
    ///
    /// `None` when nothing was recorded (a watched run, or `[transcripts] keep
    /// = false`), which surfaces must not show as *the agent said nothing*.
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

    /// One run's transcript, oldest first. `limit` keeps the newest rows,
    /// because a long conversation is read from the end.
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
        Ok(decode_rows("message", &rows, |r| {
            let role: String = r.get("role");
            Ok(crate::core::Message {
                id: r.get("id"),
                run_id: RunId::new(r.get::<String, _>("run_id")),
                at: stamp(r, "at")?,
                role: crate::core::Role::parse(&role)
                    .with_context(|| format!("unknown role {role:?}"))?,
                text: r.get("text"),
            })
        }))
    }

    /// Appends a decision. Never updated, never pruned.
    pub async fn append_decision(&self, d: &crate::core::Decision) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO decisions
               (id, at, authority, action, subject, outcome, reason, tool, server_source,
                project_id, run_id, change_id)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&d.id)
        .bind(ts(&d.at))
        .bind(d.authority.as_str())
        .bind(&d.action)
        .bind(&d.subject)
        .bind(&d.outcome)
        .bind(d.reason.as_deref())
        .bind(d.tool.as_deref())
        .bind(d.server_source.as_deref())
        .bind(d.project_id.as_ref().map(|p| p.as_str()))
        .bind(d.run_id.as_ref().map(|r| r.as_str()))
        .bind(d.change_id.as_ref().map(|w| w.as_str()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records that the inbox is asking for something. Idempotent against the
    /// open row, not the item id: asked again after an answer is a new raise.
    pub async fn attention_raise(&self, item: &crate::core::AttentionItem) -> Result<()> {
        sqlx::query(
            "INSERT INTO attention_log (item_id, kind, run_id, raised_at)
             SELECT ?,?,?,?
             WHERE NOT EXISTS (
               SELECT 1 FROM attention_log WHERE item_id = ? AND resolved_at IS NULL)",
        )
        .bind(item.id.0.as_str())
        .bind(item.kind.as_str())
        .bind(item.run_id.as_ref().map(|r| r.as_str()))
        .bind(ts(&item.since))
        .bind(item.id.0.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Closes the open row for these items, if still open. First writer wins
    /// (`WHERE resolved_at IS NULL`): the API's `Acted` is never overwritten by
    /// the sweeper's `Elsewhere`.
    pub async fn attention_resolve(
        &self,
        item_ids: &[String],
        resolution: crate::core::attention::Resolution,
    ) -> Result<u64> {
        if item_ids.is_empty() {
            return Ok(0);
        }
        // The ids ride as JSON through one bind, keeping the statement literal.
        let ids = serde_json::to_string(item_ids)?;
        Ok(sqlx::query(
            "UPDATE attention_log SET resolved_at = ?, resolution = ?
             WHERE resolved_at IS NULL AND item_id IN (SELECT value FROM json_each(?))",
        )
        .bind(ts(&jiff::Timestamp::now()))
        .bind(resolution.as_str())
        .bind(ids)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    /// Resolves every open item whose id starts with `prefix`: a snooze
    /// dismisses a kind for a project (`gh:<project>:<kind>:`), every number of it.
    pub async fn attention_resolve_prefix(
        &self,
        prefix: &str,
        resolution: crate::core::attention::Resolution,
    ) -> Result<u64> {
        Ok(sqlx::query(
            // An exact prefix, not `LIKE`: an id holding `_` or `%` would
            // otherwise match other projects' items.
            "UPDATE attention_log SET resolved_at = ?1, resolution = ?2
             WHERE resolved_at IS NULL AND substr(item_id, 1, length(?3)) = ?3",
        )
        .bind(ts(&jiff::Timestamp::now()))
        .bind(resolution.as_str())
        .bind(prefix)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    /// Closes everything still open that is no longer being asked, in one
    /// statement so a run that unblocks mid-sweep cannot be lost.
    pub async fn attention_sweep(&self, still_open: &[String]) -> Result<u64> {
        let ids = serde_json::to_string(still_open)?;
        Ok(sqlx::query(
            "UPDATE attention_log SET resolved_at = ?, resolution = 'elsewhere'
             WHERE resolved_at IS NULL AND item_id NOT IN (SELECT value FROM json_each(?))",
        )
        .bind(ts(&jiff::Timestamp::now()))
        .bind(ids)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    /// Which agents raised anything in the window.
    ///
    /// Escalation rates differ by model family ([arXiv:2604.08588]), so a raise
    /// count across a changing vendor mix is not a trend; surfaces caveat that
    /// only when more than one agent is behind the numbers. Rows whose run is
    /// pruned contribute nothing rather than an `unknown` agent.
    ///
    /// [arXiv:2604.08588]: https://arxiv.org/abs/2604.08588
    pub async fn agents_behind_attention(&self, since: jiff::Timestamp) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT DISTINCT r.agent FROM attention_log a
               JOIN runs r ON r.id = a.run_id
              WHERE a.raised_at >= ? AND r.agent != ''
              ORDER BY r.agent",
        )
        .bind(ts(&since))
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
        .bind(ts(&since))
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

    /// Stamps the items a read folded. Only on a read, since the board polls;
    /// the first fold wins, so `folded_at` stays a measurement.
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
        .bind(ts(&jiff::Timestamp::now()))
        .bind(serde_json::to_string(&list)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// When the inbox was last read, not polled.
    ///
    /// `None` folds *never looked* with *could not read*, unlike [`Self::day`]:
    /// here the failure over-reports what is new, which is the safe direction.
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
    /// Called on a dwell, never on a render: advancing it on every paint erases
    /// the boundary it draws.
    pub async fn mark_look(&self, at: jiff::Timestamp) -> Result<()> {
        sqlx::query(
            "INSERT INTO looks (id, at) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET at = ?",
        )
        .bind(ts(&at))
        .bind(ts(&at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records that a person read one weakened-check row of a change. No time
    /// is stored: only that it was seen, and by a person.
    pub async fn mark_weakened_seen(&self, change: &str, path: &str, matched: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO weakened_seen (change_id, path, matched, authority) \
             VALUES (?, ?, ?, 'person') ON CONFLICT DO NOTHING",
        )
        .bind(change)
        .bind(path)
        .bind(matched)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Every `(path, matched)` a person marked seen on this change.
    pub async fn weakened_seen(
        &self,
        change: &str,
    ) -> Result<std::collections::HashSet<(String, String)>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT path, matched FROM weakened_seen WHERE change_id = ?")
                .bind(change)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().collect())
    }

    /// One day's decisions and the count of the day's questions, for the close.
    /// Bounded by the day, not a count, so the tally is about the whole day.
    pub async fn day(&self, since: &str) -> Result<(Vec<crate::core::Decision>, u32)> {
        // Re-stamped through `ts` to compare in the stored format.
        let since: jiff::Timestamp = since
            .parse()
            .with_context(|| format!("the day's start {since:?} is not a timestamp"))?;
        let since = ts(&since);
        let rows =
            sqlx::query("SELECT * FROM decisions WHERE at >= ? ORDER BY at DESC, rowid DESC")
                .bind(&since)
                .fetch_all(&self.pool)
                .await?;
        let decisions = decode_rows("decision", &rows, decode_decision);

        // A count only: the longest wait would measure the person.
        let asks = sqlx::query("SELECT count(*) AS n FROM asks WHERE asked_at >= ?")
            .bind(&since)
            .fetch_all(&self.pool)
            .await
            // Unreadable is not empty: a failing store must not render as
            // *0 questions waited* on the summary a person trusts most.
            .context("reading today's asks")?;
        let waited: i64 = asks.first().map(|r| r.get("n")).unwrap_or(0);
        Ok((decisions, u32::try_from(waited).unwrap_or(u32::MAX)))
    }

    /// The decision log, newest first, optionally about one run or change.
    pub async fn decisions(
        &self,
        about: Option<&str>,
        limit: i64,
    ) -> Result<Vec<crate::core::Decision>> {
        let rows = match about {
            // Two indexed lookups rather than an `OR` that scans the log.
            // `rowid` is carried out explicitly as the same-second tie-break.
            Some(id) => sqlx::query(
                "SELECT rowid AS seq, * FROM decisions WHERE run_id = ?
                 UNION
                 SELECT rowid AS seq, * FROM decisions WHERE change_id = ?
                 ORDER BY at DESC, seq DESC LIMIT ?",
            )
            .bind(id)
            .bind(id)
            .bind(limit),
            None => sqlx::query("SELECT * FROM decisions ORDER BY at DESC, rowid DESC LIMIT ?")
                .bind(limit),
        }
        .fetch_all(&self.pool)
        .await?;

        Ok(decode_rows("decision", &rows, decode_decision))
    }

    /// Records that a channel delivered something, with the handler's latency.
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
        .bind(ts(&jiff::Timestamp::now()))
        .bind(micros as i64)
        .bind(error)
        .bind(error.map(|_| ts(&jiff::Timestamp::now())))
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

    /// Removes what a gate probe left behind: the probe session's run, events
    /// and decisions, and the temporary project it was discovered as.
    pub async fn forget_probe(&self, session: &str) -> Result<u64> {
        let mut n = 0;
        for sql in [
            "DELETE FROM events WHERE run_id = ?",
            "DELETE FROM decisions WHERE run_id = ?",
            // A run's id is its session's id.
            "DELETE FROM runs WHERE id = ?",
        ] {
            n += sqlx::query(sql)
                .bind(session)
                .execute(&self.pool)
                .await?
                .rows_affected();
        }
        n += sqlx::query("DELETE FROM projects WHERE root LIKE '%/devplane-probe-%'")
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(n)
    }

    /// Deletes events and transcripts older than `days`, with their search
    /// index in the same transaction. Runs are kept (see [`Self::prune_runs`]).
    pub async fn prune_events(&self, days: i64) -> Result<u64> {
        let cutoff = ts(&(jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * days)));
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
        // Resolved attention rows go too; an open item is unanswered, and age
        // is not an answer.
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

    /// Deletes runs that ended more than `days` ago, so restarts stop reloading
    /// them. Liveness is the struct's own `is_live`; a live run, a run of a
    /// change not yet archived (its answers and resumes still need the
    /// session) or an unreadable row is never pruned.
    pub async fn prune_runs(&self, days: i64) -> Result<u64> {
        let cutoff = ts(&(jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * days)));
        let rows = sqlx::query("SELECT id, payload FROM runs WHERE last_event_at < ?")
            .bind(&cutoff)
            .fetch_all(&self.pool)
            .await?;
        let changes = sqlx::query("SELECT id, payload FROM changes")
            .fetch_all(&self.pool)
            .await?;
        let kept: std::collections::HashSet<String> =
            decode_rows("change", &changes, payload::<crate::core::Change>)
                .into_iter()
                .filter(|c| c.archived_at.is_none())
                .flat_map(|c| c.runs.into_iter().map(|r| r.to_string()))
                .collect();
        let ended: Vec<String> = decode_rows("run", &rows, payload::<Run>)
            .into_iter()
            .filter(|r| !r.state.is_live() && !kept.contains(r.id.as_str()))
            .map(|r| r.id.to_string())
            .collect();
        if ended.is_empty() {
            return Ok(0);
        }
        Ok(
            sqlx::query("DELETE FROM runs WHERE id IN (SELECT value FROM json_each(?))")
                .bind(serde_json::to_string(&ended)?)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
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

/// Decodes stored payloads. Every reader goes through it: a row this build
/// cannot parse is dropped and counted, never defaulted or silently filtered,
/// because a missing Change row orphans a branch and worktree nobody mentions.
fn decode_rows<T>(
    kind: &str,
    rows: &[sqlx::sqlite::SqliteRow],
    decode: impl Fn(&sqlx::sqlite::SqliteRow) -> Result<T>,
) -> Vec<T> {
    let mut out = Vec::with_capacity(rows.len());
    let mut lost = 0usize;
    for r in rows {
        match decode(r) {
            Ok(v) => out.push(v),
            Err(e) => {
                lost += 1;
                // Bounded: a schema change makes every row unreadable.
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
            "rows this build cannot read were left out — \
             `devplane doctor` lists the run and change rows among them"
        );
    }
    out
}

/// The JSON `payload` column, as the struct it holds.
fn payload<T: serde::de::DeserializeOwned>(r: &sqlx::sqlite::SqliteRow) -> Result<T> {
    Ok(serde_json::from_str(&r.get::<String, _>("payload"))?)
}

/// A timestamp column, or the reason it is not one.
fn stamp(r: &sqlx::sqlite::SqliteRow, col: &str) -> Result<jiff::Timestamp> {
    let s: String = r.get(col);
    s.parse()
        .with_context(|| format!("{col} {s:?} is not a timestamp"))
}

/// A nullable timestamp column. Text that is not a timestamp is an error, never
/// `None`: for `ended_at` that decides whether a question is still open.
fn stamp_opt(r: &sqlx::sqlite::SqliteRow, col: &str) -> Result<Option<jiff::Timestamp>> {
    r.get::<Option<String>, _>(col)
        .map(|s| {
            s.parse()
                .with_context(|| format!("{col} {s:?} is not a timestamp"))
        })
        .transpose()
}

/// A nullable JSON column, on the same terms as [`stamp_opt`].
fn json_opt<T: serde::de::DeserializeOwned>(
    r: &sqlx::sqlite::SqliteRow,
    col: &str,
) -> Result<Option<T>> {
    r.get::<Option<String>, _>(col)
        .map(|s| serde_json::from_str(&s).with_context(|| format!("{col} would not parse")))
        .transpose()
}

/// How every timestamp column is written: fixed width, nine fractional digits,
/// `Z`. Columns are compared as text, and jiff's `Display` omits a zero
/// fraction, which sorts `10:00:00Z` after `10:00:00.5Z`.
fn ts(t: &jiff::Timestamp) -> String {
    format!("{t:.9}")
}

/// One tool call as it was observed, with the directory whose rules govern it.
#[derive(Debug, Clone)]
pub struct ObservedCall {
    pub cwd: PathBuf,
    pub tool: String,
    pub input: serde_json::Value,
}

/// Liveness of one observation channel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelHealth {
    pub channel: String,
    pub last_seen_at: String,
    pub count: u64,
    /// The worst handler latency seen: a maximum, not a percentile.
    pub worst_micros: u64,
    pub last_error: Option<String>,
    /// When that error happened. An error with no date on it reads as current.
    pub last_error_at: Option<String>,
}

/// Turns what a person typed into an FTS5 query: each run of word characters
/// becomes a quoted term, terms are ANDed, other punctuation is dropped, and
/// the last term keeps a trailing `*` for prefix search.
fn fts_phrase(query: &str) -> String {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\""))
        .collect();
    terms.join(" AND ")
}

fn row_to_event(row: &sqlx::sqlite::SqliteRow) -> Result<EventEnvelope> {
    let project: Option<String> = row.get("project_id");
    let source: String = row.get("source");
    Ok(EventEnvelope {
        id: row.get("id"),
        at: stamp(row, "at")?,
        run_id: RunId::new(row.get::<String, _>("run_id")),
        project_id: project.map(ProjectId::new),
        source: crate::core::event::Source::parse(&source)
            .with_context(|| format!("unknown source {source:?}"))?,
        event: payload(row)?,
    })
}

/// What of an event is worth searching for: commands, questions and errors.
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

/// One `decisions` row as SQLite hands it back.
///
/// An unrecognised authority drops the row: reading it as `devplane` would put
/// the most reassuring label on the least known row.
fn decode_decision(r: &sqlx::sqlite::SqliteRow) -> Result<crate::core::Decision> {
    let authority: String = r.get("authority");
    Ok(crate::core::Decision {
        id: r.get("id"),
        at: stamp(r, "at")?,
        authority: crate::core::Authority::parse(&authority)
            .with_context(|| format!("unknown authority {authority:?}"))?,
        action: r.get("action"),
        subject: r.get("subject"),
        outcome: r.get("outcome"),
        reason: r.get("reason"),
        tool: r.get("tool"),
        server_source: r.get("server_source"),
        project_id: r.get::<Option<String>, _>("project_id").map(ProjectId::new),
        run_id: r.get::<Option<String>, _>("run_id").map(RunId::new),
        change_id: r
            .get::<Option<String>, _>("change_id")
            .map(crate::core::ChangeId::new),
    })
}

/// One `asks` row back into the pure type.
///
/// Undecodable rows are dropped and counted, nullable columns included: an
/// unparseable `ended` read as none would reopen an answered question.
fn decode_ask(r: &sqlx::sqlite::SqliteRow) -> Result<crate::core::ask::Ask> {
    use crate::core::ask::{Ask, Deadline};
    let deadline = match r.get::<Option<i64>, _>("deadline_secs") {
        None => Deadline::Never,
        Some(s) if s > 0 => Deadline::After(s as u32),
        // Not a deadline the parser could produce; dropping beats "expired".
        Some(s) => anyhow::bail!("deadline_secs {s} is not a deadline"),
    };
    let kind: String = r.get("kind");
    Ok(Ask {
        id: crate::core::AskId::new(r.get::<String, _>("id")),
        kind: crate::core::ask::Kind::parse(&kind)
            .with_context(|| format!("unknown kind {kind:?}"))?,
        run: crate::core::RunId::new(r.get::<String, _>("run_id")),
        project: r
            .get::<Option<String>, _>("project_id")
            .map(crate::core::ProjectId::new),
        request_id: r.get("request_id"),
        message: r.get("message"),
        payload: payload(r)?,
        asked_at: stamp(r, "asked_at")?,
        deadline,
        answer: json_opt(r, "answer")?,
        answered_at: stamp_opt(r, "answered_at")?,
        answered_from: r.get("answered_from"),
        delivery: json_opt(r, "delivery")?,
        ended: json_opt(r, "ended")?,
        ended_at: stamp_opt(r, "ended_at")?,
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn a_replay_reads_the_directory_from_the_run_not_the_event() {
        // A call whose run row is gone has no directory, so it is dropped
        // rather than judged against another project's rules.
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
        // Scoped in the query, so the limit is not spent on other projects.
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
        // `TodoWrite` no rule speaks for, and the orphan has no directory.
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "Bash");
        assert_eq!(calls[0].cwd, PathBuf::from("/repo"));
    }

    #[tokio::test]
    async fn a_channel_error_is_kept_with_the_date_it_happened() {
        // The error outlives the fix, but keeps its own date.
        let s = Store::open_in_memory().await.unwrap();
        s.record_channel("otel", 10, Some("record had no session"))
            .await
            .unwrap();
        let before = s.channel_health().await.unwrap();
        let ch = before.iter().find(|c| c.channel == "otel").unwrap();
        let stamped = ch.last_error_at.clone().expect("an error carries its date");
        assert_eq!(ch.last_error.as_deref(), Some("record had no session"));

        // A later success neither erases nor restamps the error.
        s.record_channel("otel", 5, None).await.unwrap();
        let after = s.channel_health().await.unwrap();
        let ch = after.iter().find(|c| c.channel == "otel").unwrap();
        assert_eq!(ch.last_error.as_deref(), Some("record had no session"));
        assert_eq!(ch.last_error_at.as_deref(), Some(stamped.as_str()));
        assert_eq!(ch.count, 2, "and the channel is still counted as alive");
    }

    #[tokio::test]
    async fn the_schema_creates_every_table() {
        // Proves the schema applied: a half-applied one fails only on query.
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
            "changes",
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
        // Would otherwise reach FTS5 as an expression and fail.
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
        // The index must not outlive the events it indexes.
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

    /// A retention pass that empties `events` must not restart `seq` below
    /// the projection mark.
    #[tokio::test]
    async fn a_hook_event_after_retention_emptied_the_log_is_still_projected() {
        let s = Store::open_in_memory().await.unwrap();
        let hook = |cmd: &str| {
            EventEnvelope::new(
                RunId::new("s1"),
                Source::Hook,
                Event::tool_started("Bash", serde_json::json!({ "command": cmd })),
            )
        };
        for i in 0..3 {
            s.append_event(&hook(&format!("before {i}"))).await.unwrap();
        }
        let rows = s.shim_events_since(0, 100).await.unwrap();
        let through = rows.last().unwrap().0;
        s.set_projected_through(through).await.unwrap();

        s.prune_events(-1).await.unwrap();
        s.append_event(&hook("after")).await.unwrap();
        let rows = s
            .shim_events_since(s.projected_through().await.unwrap(), 100)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "the new event is invisible to the host");
        assert!(rows[0].0 > through, "a sequence number was reused");
    }

    /// Recording what a hook saw never revokes a trust granted in between.
    #[tokio::test]
    async fn noting_a_project_never_touches_trust() {
        let s = Store::open_in_memory().await.unwrap();
        let seen = Project::from_root(PathBuf::from("/repo"));
        assert!(!seen.trusted);
        s.note_project(&seen).await.unwrap();
        let mut trusted = seen.clone();
        trusted.trusted = true;
        s.save_project(&trusted).await.unwrap();
        s.note_project(&seen).await.unwrap();
        let back = s.load_projects().await.unwrap();
        assert!(back[0].trusted, "a hook revoked the trust");
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

        // Read from the end, still in order.
        let tail = s.messages_for_run(&RunId::new("r1"), 2).await.unwrap();
        assert_eq!(
            tail.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(),
            ["Reading auth.rs", "Found it"]
        );
    }

    #[tokio::test]
    async fn a_transcript_ages_out_with_the_events_beside_it() {
        // Transcripts are pruned like any observation; decisions stay.
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
            change_id: None,
            offer: None,
            no_offer: None,
            report: None,
            since: jiff::Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn an_answered_item_is_never_counted_as_one_that_went_away() {
        // The API writes `acted` when a person answers and the sweeper
        // follows; `resolved_at IS NULL` keeps the sweeper from overwriting it.
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
        // Resolved attention rows are pruned with the events; open ones are
        // kept whatever their age.
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
            change_id: None,
            offer: None,
            no_offer: None,
            report: None,
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

    fn backups(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("devplane.v") && n.ends_with(".bak"))
            .collect();
        v.sort();
        v
    }

    async fn write_old_schema(path: &Path, version: i64) {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "PRAGMA user_version = {version};
             CREATE TABLE decisions (id TEXT PRIMARY KEY);
             INSERT INTO decisions (id) VALUES ('only-copy');"
        )))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    /// Parallel hooks opening an old-schema database: exactly one retires it,
    /// the rest wait and open the new file, and the old record survives.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_opens_of_an_old_database_retire_it_once() {
        let dir = std::env::temp_dir().join(format!(
            "devplane-retire-race-{}-{}",
            std::process::id(),
            jiff::Timestamp::now().as_nanosecond()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("devplane.db");
        write_old_schema(&path, 97).await;

        let opens: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                tokio::spawn(async move { Store::open(&path).await })
            })
            .collect();
        for o in opens {
            o.await.unwrap().expect("every opener gets a usable store");
        }
        let baks = backups(&dir);
        assert_eq!(baks.len(), 1, "one retire, not one per opener: {baks:?}");
        // The retired file still holds the only copy of the record.
        let old = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(dir.join(&baks[0])))
            .await
            .unwrap();
        let (id,): (String,) = sqlx::query_as("SELECT id FROM decisions")
            .fetch_one(&old)
            .await
            .expect("the backup is the old database, not a fresh one");
        assert_eq!(id, "only-copy");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A store opened by anything (a hook, before any `serve`) is owner-only.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_store_is_owner_only_from_its_first_open() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "devplane-owner-only-{}-{}",
            std::process::id(),
            jiff::Timestamp::now().as_nanosecond()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("devplane.db");
        let store = Store::open(&path).await.unwrap();
        for suffix in ["", "-wal"] {
            let file = PathBuf::from(format!("{}{suffix}", path.display()));
            let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{}", file.display());
        }
        store.pool.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Retiring never overwrites or deletes an earlier backup, however many
    /// there are; only an empty one, which holds nothing, goes.
    #[tokio::test]
    async fn retiring_keeps_every_backup_and_drops_only_empty_ones() {
        let dir = std::env::temp_dir().join(format!(
            "devplane-retire-keep-{}-{}",
            std::process::id(),
            jiff::Timestamp::now().as_nanosecond()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("devplane.db");
        // A pre-existing backup with the old fixed name, and an empty one.
        std::fs::write(dir.join("devplane.v1.bak"), b"old record").unwrap();
        std::fs::write(dir.join("devplane.v2.bak"), b"").unwrap();
        // Empty, but its log is not: kept.
        std::fs::write(dir.join("devplane.v3.bak"), b"").unwrap();
        std::fs::write(dir.join("devplane.v3.bak-wal"), b"frames").unwrap();

        for v in [90, 91, 92, 93, 94] {
            for f in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{f}", path.display()));
            }
            write_old_schema(&path, v).await;
            Store::open(&path).await.unwrap().pool.close().await;
        }
        let baks: Vec<String> = backups(&dir)
            .into_iter()
            .filter(|b| b.ends_with(".bak"))
            .collect();
        assert!(!baks.iter().any(|b| b == "devplane.v2.bak"), "{baks:?}");
        for kept in ["devplane.v1.bak", "devplane.v3.bak"] {
            assert!(
                baks.iter().any(|b| b == kept),
                "{kept} was deleted: {baks:?}"
            );
        }
        for v in [90, 91, 92, 93, 94] {
            assert!(
                baks.iter()
                    .any(|b| b.starts_with(&format!("devplane.v{v}."))),
                "v{v} was deleted: {baks:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_database_from_another_schema_is_moved_aside_rather_than_migrated() {
        // A file stamped with a different schema is renamed aside and a fresh
        // one takes its place. The old file must still be on disk: the
        // decision log in it cannot be re-derived.
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
                // Any version but this build's.
                "PRAGMA user_version = 99;
                 CREATE TABLE decisions (
                   id TEXT PRIMARY KEY, at TEXT NOT NULL, actor TEXT NOT NULL,
                   action TEXT NOT NULL, subject TEXT NOT NULL, outcome TEXT NOT NULL,
                   reason TEXT, project_id TEXT, run_id TEXT, change_id TEXT);
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

        assert_eq!(
            backups(&dir).len(),
            1,
            "the old file is kept — a decision log is the one thing here that \
             cannot be re-derived, so it is moved and never deleted"
        );

        // Opening again moves nothing: the file carries this schema's version.
        let s = Store::open(&path).await.unwrap();
        assert_eq!(s.decisions(None, 10).await.unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn decisions_are_appended_and_survive_a_prune() {
        // Events are prunable history; decisions are not.
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
            crate::core::Authority::Devplane,
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

    /// Decisions stamped with the same instant come back in insertion order:
    /// `rowid` breaks `at` ties, since a uuid v7's tail is random within a
    /// millisecond.
    #[tokio::test]
    async fn decisions_taken_in_one_instant_keep_the_order_they_were_taken_in() {
        let s = Store::open_in_memory().await.unwrap();
        let at = jiff::Timestamp::now();
        for n in 0..8 {
            let mut d = crate::core::Decision::new(
                crate::core::Authority::Devplane,
                &format!("step:{n}"),
                "subject",
                "done",
            );
            // The same stamp on every row, to make the tie certain.
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
        // A row this build cannot decode is reported, not silently absent,
        // since a missing `change` row orphans its branch and worktree.
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
            "INSERT INTO runs (id, agent, cwd, last_event_at, payload)
             VALUES ('s-bad','claude','/tmp/repo','t','{\"not\":\"a run\"}')",
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

#[cfg(test)]
mod report_store_tests {
    use super::*;
    use crate::core::report::{Draft, Provenance, Report, State, Target};

    fn filed(from: &str, to: &str, change: Option<&str>) -> Report {
        Report::new(
            Draft {
                kind: "defect".into(),
                title: "client retries on 4xx".into(),
                finding: "retry() does not check status".into(),
                ..Default::default()
            },
            Target::Project {
                project: crate::core::ProjectId::new(to),
                name: to.trim_start_matches('/').into(),
            },
            Provenance::of_run(
                crate::core::ProjectId::new(from),
                from.trim_start_matches('/').into(),
                change.map(crate::core::ChangeId::new),
                RunId::new("acp-1"),
                "claude".into(),
                jiff::Timestamp::now(),
            ),
            |_| Ok(()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn a_report_is_found_from_both_ends_and_by_its_change() {
        let s = Store::open_in_memory().await.unwrap();
        let mut r = filed("/api", "/core-lib", Some("c-src"));
        s.save_report(&r).await.unwrap();
        let api = crate::core::ProjectId::new("/api");
        let core = crate::core::ProjectId::new("/core-lib");

        assert_eq!(s.report(r.id.as_str()).await.unwrap().as_ref(), Some(&r));
        assert_eq!(
            s.report(&r.id.as_str()[..10]).await.unwrap().map(|x| x.id),
            Some(r.id.clone()),
            "the start of an id is enough when it names one"
        );
        assert_eq!(s.reports_to(&core, true).await.unwrap().len(), 1);
        assert_eq!(s.reports_from(&api, true).await.unwrap().len(), 1);
        assert!(s.reports_to(&api, false).await.unwrap().is_empty());

        // Accepted into a change in the target: that change finds it too.
        r.state = State::Accepted {
            change: crate::core::ChangeId::new("c-dst"),
        };
        s.save_report(&r).await.unwrap();
        assert!(
            s.reports_to(&core, true).await.unwrap().is_empty(),
            "no longer open"
        );
        assert_eq!(s.reports_to(&core, false).await.unwrap().len(), 1);
        for c in ["c-src", "c-dst"] {
            let found = s
                .reports_for_change(&crate::core::ChangeId::new(c))
                .await
                .unwrap();
            assert_eq!(found.len(), 1, "{c}");
        }
        assert_eq!(
            s.reports(10).await.unwrap().len(),
            1,
            "rewritten, not appended"
        );
    }

    #[tokio::test]
    async fn an_unreadable_report_is_dropped_and_counted() {
        let s = Store::open_in_memory().await.unwrap();
        sqlx::query(
            "INSERT INTO reports (id, filed_at, source_project, target_project, state, payload)
             VALUES ('rp-bad', '2026-09-25T00:00:00.000000000Z', '/a', '/b', 'open', '{')",
        )
        .execute(s.pool())
        .await
        .unwrap();
        s.save_report(&filed("/a", "/b", None)).await.unwrap();
        assert_eq!(s.reports(10).await.unwrap().len(), 1);
        let lost = s.unreadable().await.unwrap();
        assert!(
            lost.iter().any(|(what, _)| what == "report rp-bad"),
            "{lost:?}"
        );
    }
}

/// A row this build cannot read is dropped and counted, never defaulted.
#[cfg(test)]
mod integrity_tests {
    use super::*;
    use crate::core::event::{Event, Source};
    use crate::core::run::RunMode;
    use std::path::PathBuf;

    fn ask(id: &str) -> crate::core::ask::Ask {
        crate::core::ask::Ask::new(
            crate::core::AskId::new(id),
            RunId::new("r1"),
            crate::core::ask::Asked {
                kind: crate::core::ask::Kind::Question,
                request_id: "req".into(),
                message: "Keep it?".into(),
                payload: serde_json::json!({}),
                at: jiff::Timestamp::now(),
                deadline: crate::core::ask::Deadline::Never,
            },
        )
    }

    /// An ending that will not parse is not the absence of one: read as
    /// `None`, an answered ask would reappear in the inbox.
    #[tokio::test]
    async fn an_ask_whose_ending_will_not_parse_is_dropped_rather_than_reopened() {
        let s = Store::open_in_memory().await.unwrap();
        let mut a = ask("a1");
        a.answer(
            serde_json::json!({"q": "keep"}),
            "board",
            jiff::Timestamp::now(),
        )
        .unwrap();
        s.save_ask(&a).await.unwrap();
        assert!(!s.ask("a1").await.unwrap().unwrap().is_open());

        for (column, corrupt) in [
            (
                "ended",
                "UPDATE asks SET ended = 'not json' WHERE id = 'a1'",
            ),
            (
                "answer",
                "UPDATE asks SET answer = 'not json' WHERE id = 'a1'",
            ),
            (
                "delivery",
                "UPDATE asks SET delivery = 'not json' WHERE id = 'a1'",
            ),
        ] {
            sqlx::query(corrupt).execute(&s.pool).await.unwrap();
            assert!(
                s.ask("a1").await.unwrap().is_none(),
                "an ask whose `{column}` cannot be read must not be handed back"
            );
            assert!(
                s.asks(10).await.unwrap().is_empty(),
                "nor listed ({column})"
            );
            s.save_ask(&a).await.unwrap();
        }
        // And the same for a stamp that is not one.
        sqlx::query("UPDATE asks SET ended_at = 'yesterday-ish' WHERE id = 'a1'")
            .execute(&s.pool)
            .await
            .unwrap();
        assert!(s.ask("a1").await.unwrap().is_none());
    }

    /// A source this build has no name for drops the row rather than
    /// borrowing another source's name.
    #[tokio::test]
    async fn a_feed_event_reads_back_as_a_feed_event_and_an_unknown_source_is_dropped() {
        let s = Store::open_in_memory().await.unwrap();
        s.append_event(&EventEnvelope::new(
            RunId::new("s1"),
            Source::Feed,
            Event::tool_started("Bash", serde_json::json!({"command": "ls"})),
        ))
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO events (id, at, run_id, project_id, source, kind, payload)
             VALUES ('e-odd', '2026-09-14T10:00:00.000000000Z', 's1', NULL, 'telepathy',
                     'tool_started', ?)",
        )
        .bind(serde_json::json!({"tool": "Bash", "input": {}}).to_string())
        .execute(&s.pool)
        .await
        .unwrap();

        let back = s.events_for_run(&RunId::new("s1"), 10).await.unwrap();
        assert_eq!(
            back.len(),
            1,
            "the unknown source is dropped, not defaulted"
        );
        assert_eq!(back[0].source, Source::Feed);
    }

    /// One stray row must not take its run's good events off the board.
    #[tokio::test]
    async fn one_unreadable_event_does_not_take_the_replay_with_it() {
        let s = Store::open_in_memory().await.unwrap();
        for i in 0..3 {
            s.append_event(&EventEnvelope::new(
                RunId::new("s1"),
                Source::Hook,
                Event::tool_started(format!("Tool{i}"), serde_json::json!({})),
            ))
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO events (id, at, run_id, project_id, source, kind, payload)
             VALUES ('e-bad', '2026-09-14T10:00:00.000000000Z', 's1', NULL, 'hook', 'x', '{')",
        )
        .execute(&s.pool)
        .await
        .unwrap();
        let back = s.events_for_run(&RunId::new("s1"), 10).await.unwrap();
        assert_eq!(back.len(), 3, "three good events survive one bad one");
    }

    /// Stamps are written at one width, so `10:00:00Z` sorts before
    /// `10:00:00.000000001Z` (jiff omits a zero fraction and `'Z'` > `'.'`).
    #[tokio::test]
    async fn a_whole_second_stamp_sorts_before_the_nanosecond_after_it() {
        let whole: jiff::Timestamp = "2026-09-24T10:00:00Z".parse().unwrap();
        let after = whole + jiff::SignedDuration::from_nanos(1);
        assert!(
            whole.to_string() > after.to_string(),
            "the premise: jiff's own spelling sorts the earlier instant later"
        );
        assert!(ts(&whole) < ts(&after));
        assert_eq!(ts(&whole).parse::<jiff::Timestamp>().unwrap(), whole);

        let s = Store::open_in_memory().await.unwrap();
        for (id, at) in [("later", after), ("earlier", whole)] {
            let mut env = EventEnvelope::new(
                RunId::new("s1"),
                Source::Hook,
                Event::tool_started("Bash", serde_json::json!({})),
            );
            env.id = id.into();
            env.at = at;
            s.append_event(&env).await.unwrap();
        }
        let order: Vec<String> = sqlx::query_scalar("SELECT id FROM events ORDER BY at ASC")
            .fetch_all(&s.pool)
            .await
            .unwrap();
        assert_eq!(order, ["earlier", "later"]);
    }

    /// A stamp that will not parse drops the row rather than reading as now.
    #[tokio::test]
    async fn a_row_with_a_corrupt_stamp_is_dropped_rather_than_stamped_now() {
        let s = Store::open_in_memory().await.unwrap();
        s.append_message(&crate::core::Message::new(
            RunId::new("r1"),
            crate::core::Role::Agent,
            "fine",
        ))
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, run_id, at, role, text) VALUES
               ('m-bad', 'r1', 'last tuesday', 'agent', 'when?'),
               ('m-role', 'r1', '2026-09-14T10:00:00.000000000Z', 'oracle', 'who?')",
        )
        .execute(&s.pool)
        .await
        .unwrap();
        let back = s.messages_for_run(&RunId::new("r1"), 10).await.unwrap();
        assert_eq!(
            back.len(),
            1,
            "the corrupt stamp and the unknown role are both dropped"
        );
        assert_eq!(back[0].text, "fine");

        sqlx::query(
            "INSERT INTO decisions (id, at, authority, action, subject, outcome)
             VALUES ('d-bad', 'never', 'person', 'a', 's', 'o')",
        )
        .execute(&s.pool)
        .await
        .unwrap();
        assert!(s.decisions(None, 10).await.unwrap().is_empty());
    }

    /// A run that ended is deleted once old enough; a live one never is.
    #[tokio::test]
    async fn prune_runs_deletes_ended_runs_and_keeps_live_ones() {
        let s = Store::open_in_memory().await.unwrap();
        let long_ago = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * 30);
        let mut done = Run::new(
            crate::core::SessionId::new("done"),
            PathBuf::from("/repo"),
            RunMode::Observed,
            "claude",
        );
        done.state = crate::core::RunState::Completed;
        done.last_event_at = long_ago;
        let mut busy = done.clone();
        busy.id = RunId::new("busy");
        busy.state = crate::core::RunState::Working;
        let mut fresh = done.clone();
        fresh.id = RunId::new("fresh");
        fresh.last_event_at = jiff::Timestamp::now();
        for r in [&done, &busy, &fresh] {
            s.save_run(r).await.unwrap();
        }

        // An old ended run of a change still open is its session: kept.
        let mut parked = done.clone();
        parked.id = RunId::new("parked");
        s.save_run(&parked).await.unwrap();
        let mut change = crate::core::Change::new_at(
            crate::core::ProjectId::new("p"),
            "waiting on a person".into(),
            "…".into(),
            long_ago,
        );
        change.runs.push(parked.id.clone());
        s.save_change(&change).await.unwrap();

        assert_eq!(s.prune_runs(7).await.unwrap(), 1);
        let left: Vec<String> = s
            .load_runs()
            .await
            .unwrap()
            .iter()
            .map(|r| r.id.to_string())
            .collect();
        assert_eq!(
            left,
            ["fresh", "busy", "parked"],
            "newest first; the old finished run is gone, the open change's is not"
        );
    }

    /// `runs` and `changes` carry only the columns a query reads beside the
    /// payload; this keeps the next convenient column out.
    #[tokio::test]
    async fn the_runs_and_changes_tables_hold_only_the_columns_a_query_reads() {
        let s = Store::open_in_memory().await.unwrap();
        for (table, expected) in [
            (
                "runs",
                vec!["id", "agent", "cwd", "last_event_at", "payload"],
            ),
            ("changes", vec!["id", "updated_at", "payload"]),
        ] {
            let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
                .bind(table)
                .fetch_all(&s.pool)
                .await
                .unwrap();
            assert_eq!(cols, expected, "{table}");
        }
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&s.pool)
            .await
            .unwrap();
        assert_eq!(
            version, SCHEMA_VERSION,
            "the pragma and the constant drifted"
        );
    }

    /// `about` answers for a change id as well as a run id, in order.
    #[tokio::test]
    async fn decisions_about_a_change_are_found_as_well_as_about_a_run() {
        let s = Store::open_in_memory().await.unwrap();
        let at = jiff::Timestamp::now();
        for (n, id) in ["w1", "r1", "w1"].iter().enumerate() {
            let mut d = crate::core::Decision::new(
                crate::core::Authority::Devplane,
                &format!("step:{n}"),
                "s",
                "done",
            );
            d.at = at;
            d = match *id {
                "w1" => d.for_change(&crate::core::ChangeId::new("w1")),
                _ => d.for_run(&RunId::new("r1")),
            };
            s.append_decision(&d).await.unwrap();
        }
        let about_change: Vec<String> = s
            .decisions(Some("w1"), 10)
            .await
            .unwrap()
            .iter()
            .map(|d| d.action.clone())
            .collect();
        assert_eq!(
            about_change,
            ["step:2", "step:0"],
            "newest first, by insertion when tied"
        );
        assert_eq!(s.decisions(Some("r1"), 10).await.unwrap().len(), 1);
        assert!(s.decisions(Some("nobody"), 10).await.unwrap().is_empty());
    }
}

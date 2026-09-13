//! The observation store: SQLite, WAL, rebuildable.
//!
//! Two rules shape everything here:
//!
//! * **Observations only.** Events, and the run and project rows derived from
//!   them. Anything Vibeplane causes belongs in the runtime journal instead, so
//!   there is never a question about which store owns a fact.
//! * **Rebuildable.** Losing this database costs history, not correctness: the
//!   providers are still the source of truth for what is running.

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use vibeplane_domain::event::EventEnvelope;
use vibeplane_domain::ids::{ProjectId, RunId};
use vibeplane_domain::project::Project;
use vibeplane_domain::run::Run;

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
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .context("sqlite connect options")?
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

    /// Applies the schema. There is no migration machinery on purpose: the
    /// project is unreleased, the store is rebuildable, and a hard cut is
    /// cheaper than carrying a migration for a shape nobody depends on.
    async fn migrate(&self) -> Result<()> {
        // `raw_sql` runs the whole file, comments and all, in one round trip.
        // Hand-splitting on `;` is how a schema loses the statement that a
        // comment happens to sit above.
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await
            .context("applying schema")?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Appends an event and indexes anything searchable in it.
    pub async fn append_event(&self, env: &EventEnvelope) -> Result<()> {
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
        .execute(&self.pool)
        .await?;

        if let Some(text) = searchable_text(&env.event) {
            sqlx::query("INSERT INTO events_fts (text, run_id, event_id) VALUES (?, ?, ?)")
                .bind(text)
                .bind(env.run_id.as_str())
                .bind(&env.id)
                .execute(&self.pool)
                .await?;
        }
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

    /// The whole log, oldest first, for a replay at startup.
    pub async fn all_events(&self, limit: i64) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            "SELECT id, at, run_id, project_id, source, payload FROM events
             ORDER BY id ASC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_event).collect()
    }

    /// Full-text search across tool commands, questions and summaries.
    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<(RunId, String)>> {
        let rows = sqlx::query(
            "SELECT run_id, text FROM events_fts WHERE events_fts MATCH ? ORDER BY rank LIMIT ?",
        )
        .bind(query)
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
        let rows = sqlx::query("SELECT payload FROM runs ORDER BY last_event_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .filter_map(|r| serde_json::from_str(r.get::<String, _>("payload").as_str()).ok())
            .collect())
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
            "INSERT INTO channel_health (channel, last_seen_at, count, p99_micros, last_error)
             VALUES (?, ?, 1, ?, ?)
             ON CONFLICT(channel) DO UPDATE SET
               last_seen_at = excluded.last_seen_at,
               count = channel_health.count + 1,
               p99_micros = MAX(channel_health.p99_micros, excluded.p99_micros),
               last_error = COALESCE(excluded.last_error, channel_health.last_error)",
        )
        .bind(channel)
        .bind(jiff::Timestamp::now().to_string())
        .bind(micros as i64)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn channel_health(&self) -> Result<Vec<ChannelHealth>> {
        let rows = sqlx::query(
            "SELECT channel, last_seen_at, count, p99_micros, last_error FROM channel_health",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| ChannelHealth {
                channel: r.get("channel"),
                last_seen_at: r.get("last_seen_at"),
                count: r.get::<i64, _>("count") as u64,
                worst_micros: r.get::<i64, _>("p99_micros") as u64,
                last_error: r.get("last_error"),
            })
            .collect())
    }

    /// Deletes events older than `days`. Runs are kept: a row on the board
    /// costs nothing, and losing it would make a resumable session invisible.
    pub async fn prune_events(&self, days: i64) -> Result<u64> {
        let cutoff =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * days)).to_string();
        let r = sqlx::query("DELETE FROM events WHERE at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected())
    }
}

/// Liveness of one observation channel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelHealth {
    pub channel: String,
    pub last_seen_at: String,
    pub count: u64,
    /// The worst handler latency seen. Named honestly: it is a maximum, not a
    /// percentile, because keeping a real p99 would need a histogram per
    /// channel and the number is only ever read by a human looking for a stall.
    pub worst_micros: u64,
    pub last_error: Option<String>,
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
            "otel" => vibeplane_domain::event::Source::Otel,
            "agents_json" => vibeplane_domain::event::Source::AgentsJson,
            "statusline" => vibeplane_domain::event::Source::StatusLine,
            "daemon" => vibeplane_domain::event::Source::Daemon,
            _ => vibeplane_domain::event::Source::Hook,
        },
        event: serde_json::from_str(row.get::<String, _>("payload").as_str())?,
    })
}

/// What of an event is worth searching for. Deliberately narrow: commands,
/// questions and errors are what a human looks for months later.
fn searchable_text(e: &vibeplane_domain::event::Event) -> Option<String> {
    use vibeplane_domain::event::Event as E;
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
    async fn the_schema_creates_every_table() {
        // Proves the schema applied, not that a string contains a word: a
        // half-applied schema only fails on the first query otherwise.
        let s = Store::open_in_memory().await.unwrap();
        let names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
                .fetch_all(s.pool())
                .await
                .unwrap();
        for table in ["projects", "runs", "events", "events_fts", "channel_health"] {
            assert!(
                names.iter().any(|n| n == table),
                "schema did not create `{table}`; got {names:?}"
            );
        }
    }
    use std::path::PathBuf;
    use vibeplane_domain::SessionId;
    use vibeplane_domain::event::{Event, Source};
    use vibeplane_domain::run::RunMode;

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
    async fn channel_health_accumulates() {
        let s = Store::open_in_memory().await.unwrap();
        s.record_channel("hook", 900, None).await.unwrap();
        s.record_channel("hook", 1500, None).await.unwrap();
        let h = s.channel_health().await.unwrap();
        assert_eq!(h[0].count, 2);
        assert_eq!(h[0].worst_micros, 1500);
    }
}

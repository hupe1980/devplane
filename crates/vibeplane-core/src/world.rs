//! The authoritative in-memory state, and the only place runs are created.
//!
//! The UI never reconstructs state from events: it subscribes to this. That is
//! the difference between a dashboard that survives a high-volume session and
//! one that melts, and it is why the reducer is pure — this struct is the only
//! thing that mutates.

use crate::reduce;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use vibeplane_domain::attention::{AttentionConfig, AttentionItem, items_for_run, rank};
use vibeplane_domain::event::{Event, EventEnvelope};
use vibeplane_domain::ids::{ProjectId, RunId, SessionId};
use vibeplane_domain::project::{Project, find_repo_root, main_checkout_for};
use vibeplane_domain::run::{Run, RunMode, RunState};

/// Everything the daemon knows right now.
#[derive(Debug, Default)]
pub struct World {
    runs: BTreeMap<RunId, Run>,
    projects: BTreeMap<ProjectId, Project>,
    pub attention: AttentionConfig,
}

/// What changed, so a subscriber can be told without diffing the whole world.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    RunUpserted(RunId),
    ProjectDiscovered(ProjectId),
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn runs(&self) -> impl Iterator<Item = &Run> {
        self.runs.values()
    }

    pub fn run(&self, id: &RunId) -> Option<&Run> {
        self.runs.get(id)
    }

    pub fn projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.values()
    }

    pub fn project(&self, id: &ProjectId) -> Option<&Project> {
        self.projects.get(id)
    }

    /// Live runs, newest activity first — the board's default order.
    pub fn board(&self) -> Vec<&Run> {
        let mut v: Vec<&Run> = self.runs.values().collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.last_event_at));
        v
    }

    /// The inbox, derived fresh every time. Cheap because it is a map over
    /// runs, and correct because there is no cached copy to go stale.
    pub fn inbox(&self) -> Vec<AttentionItem> {
        rank(
            self.runs
                .values()
                .flat_map(|r| items_for_run(r, &self.attention))
                .collect(),
        )
    }

    /// Registers a project explicitly. Explicit registration is what grants
    /// trust later; discovery never does.
    pub fn upsert_project(&mut self, project: Project) -> ProjectId {
        let id = project.id.clone();
        self.projects
            .entry(id.clone())
            .and_modify(|p| {
                p.name = project.name.clone();
                p.trusted = project.trusted || p.trusted;
                if project.repo_url.is_some() {
                    p.repo_url = project.repo_url.clone();
                }
                if !project.auto_discovered {
                    p.auto_discovered = false;
                }
            })
            .or_insert(project);
        id
    }

    /// Finds or discovers the project a path belongs to.
    ///
    /// A worktree Claude created under `.claude/worktrees/` resolves to the
    /// repository that owns it, so five parallel worktrees are five runs on one
    /// project rather than five projects nobody recognises.
    pub fn resolve_project(&mut self, path: &Path) -> Option<(ProjectId, Option<Change>)> {
        let root = main_checkout_for(path)
            .or_else(|| find_repo_root(path))
            .unwrap_or_else(|| path.to_path_buf());

        let id = ProjectId::from_path(&root);
        if self.projects.contains_key(&id) {
            return Some((id, None));
        }
        // A directory we have never seen becomes a project on sight, so that a
        // session in an unregistered repository still lands somewhere sensible.
        let mut p = Project::from_root(root);
        p.auto_discovered = true;
        let id = self.upsert_project(p);
        Some((id.clone(), Some(Change::ProjectDiscovered(id))))
    }

    /// Applies an event, creating the run if this is the first time we have
    /// seen the session.
    ///
    /// Returns the envelope with its project filled in, so the caller persists
    /// exactly what was applied.
    pub fn apply(&mut self, mut env: EventEnvelope, hint: RunHint) -> (EventEnvelope, Vec<Change>) {
        let mut changes = Vec::new();

        let cwd = hint.cwd.clone().unwrap_or_else(|| {
            self.runs
                .get(&env.run_id)
                .map(|r| r.cwd.clone())
                .unwrap_or_else(|| PathBuf::from("."))
        });

        if !self.runs.contains_key(&env.run_id) {
            let session = SessionId::new(env.run_id.as_str());
            let mut run = Run::new(session, cwd.clone(), hint.mode, &hint.agent);
            run.started_at = env.at;
            run.last_event_at = env.at;
            run.last_activity_at = env.at;
            self.runs.insert(env.run_id.clone(), run);
        }

        // Resolve the project from the most specific path we have.
        if let Some((pid, change)) = self.resolve_project(&cwd) {
            if let Some(c) = change {
                changes.push(c);
            }
            env.project_id = Some(pid.clone());
            if let Some(r) = self.runs.get_mut(&env.run_id) {
                r.project_id = Some(pid);
            }
        }

        if let Some(run) = self.runs.get_mut(&env.run_id) {
            if run.mode == RunMode::Observed && hint.mode != RunMode::Observed {
                // A session first seen through a hook and later found in the
                // background roster is a background run: the stronger claim
                // wins, because it tells us who supervises the process.
                run.mode = hint.mode;
            }
            reduce::apply(run, &env);
        }
        changes.push(Change::RunUpserted(env.run_id.clone()));
        (env, changes)
    }

    /// Marks a project as one an agent may be started in.
    pub fn trust(&mut self, id: &ProjectId) -> bool {
        match self.projects.get_mut(id) {
            Some(p) => {
                p.trusted = true;
                p.auto_discovered = false;
                true
            }
            None => false,
        }
    }

    /// Hides or un-hides a run's inbox items.
    pub fn snooze(&mut self, run: &RunId, until: Option<jiff::Timestamp>) -> bool {
        match self.runs.get_mut(run) {
            Some(r) => {
                r.snoozed_until = until;
                true
            }
            None => false,
        }
    }

    /// Loads previously persisted runs into memory.
    ///
    /// Restoring is not believing: a run recorded as working is a claim about a
    /// process that may have died while the daemon was down. The caller
    /// reconciles against the machine before any of this reaches the board.
    pub fn restore_runs(&mut self, runs: impl IntoIterator<Item = Run>) {
        for run in runs {
            if let Some(pid) = run.project_id.clone()
                && !self.projects.contains_key(&pid)
            {
                // The project row may be gone while its runs remain; keep the
                // reference resolvable rather than dropping the run.
                let mut p = Project::from_root(run.cwd.clone());
                p.id = pid;
                p.auto_discovered = true;
                self.upsert_project(p);
            }
            self.runs.insert(run.id.clone(), run);
        }
    }

    /// Records the surface a session is running on, and the repository it
    /// reported.
    ///
    /// OpenTelemetry is the only channel that names the entrypoint — `cli`,
    /// `claude-vscode`, `sdk-ts` — which is what lets the board say *where* a
    /// session is, not just that it exists.
    pub fn set_entrypoint(&mut self, run: &RunId, entrypoint: &str, repo_url: Option<&str>) {
        let Some(r) = self.runs.get_mut(run) else {
            return;
        };
        if r.entrypoint.as_deref() != Some(entrypoint) {
            r.entrypoint = Some(entrypoint.to_string());
        }
        if let Some(url) = repo_url
            && let Some(pid) = r.project_id.clone()
            && let Some(p) = self.projects.get_mut(&pid)
            && p.repo_url.is_none()
        {
            p.repo_url = Some(url.to_string());
        }
    }

    /// Re-derives state from a replay of stored events. Used at startup, and by
    /// the tests that prove the reducer and the store agree.
    pub fn replay(&mut self, events: impl IntoIterator<Item = EventEnvelope>) {
        for env in events {
            let cwd = match &env.event {
                Event::SessionStarted { cwd, .. } | Event::CwdChanged { cwd } => Some(cwd.clone()),
                _ => None,
            };
            self.apply(
                env,
                RunHint {
                    cwd,
                    mode: RunMode::Observed,
                    agent: "claude".into(),
                },
            );
        }
    }

    /// Marks live runs whose process is gone as lost, and returns the events
    /// that recorded it so the caller can persist them.
    ///
    /// Never assume a run still exists because the database says so: that is
    /// the difference between a board that reflects the machine and one that
    /// reflects a memory of it.
    pub fn reconcile(&mut self, alive: &dyn Fn(&Run) -> bool) -> Vec<EventEnvelope> {
        let mut out = Vec::new();
        let ids: Vec<RunId> = self
            .runs
            .values()
            .filter(|r| r.state.is_live())
            .map(|r| r.id.clone())
            .collect();
        for id in ids {
            let gone = self.runs.get(&id).map(|r| !alive(r)).unwrap_or(false);
            if gone {
                let env = EventEnvelope::new(
                    id.clone(),
                    vibeplane_domain::event::Source::Daemon,
                    Event::Lost {
                        reason: "process not found at startup".into(),
                    },
                );
                if let Some(run) = self.runs.get_mut(&id) {
                    reduce::apply(run, &env);
                }
                out.push(env);
            }
        }
        out
    }

    /// Drops terminal runs older than `max_age_seconds` from memory. The events
    /// stay on disk; this only keeps the board from growing forever.
    pub fn prune(&mut self, max_age_seconds: i64) -> usize {
        let before = self.runs.len();
        self.runs.retain(|_, r| {
            r.state.is_live()
                || (jiff::Timestamp::now() - r.last_event_at).get_seconds() < max_age_seconds
        });
        before - self.runs.len()
    }

    /// Counts for the board header.
    pub fn summary(&self) -> BoardSummary {
        let mut s = BoardSummary {
            projects: self.projects.len(),
            runs: self.runs.len(),
            ..Default::default()
        };
        for r in self.runs.values() {
            match r.state {
                RunState::Working | RunState::Starting => s.working += 1,
                RunState::Waiting(_) => s.needs_you += 1,
                RunState::Idle => s.idle += 1,
                RunState::Failed | RunState::Lost => s.failed += 1,
                _ => {}
            }
            if r.state.is_live() {
                s.live += 1;
            }
            s.cost_usd += r.totals.cost_usd;
        }
        s
    }
}

/// What the caller knows about a run that the event itself does not carry.
#[derive(Debug, Clone)]
pub struct RunHint {
    pub cwd: Option<PathBuf>,
    pub mode: RunMode,
    pub agent: String,
}

impl Default for RunHint {
    fn default() -> Self {
        Self {
            cwd: None,
            mode: RunMode::Observed,
            agent: "claude".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BoardSummary {
    pub projects: usize,
    pub runs: usize,
    pub live: usize,
    pub working: usize,
    pub needs_you: usize,
    pub idle: usize,
    pub failed: usize,
    pub cost_usd: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vibeplane_domain::event::{Source, WaitingFor};

    fn env(run: &str, e: Event) -> EventEnvelope {
        EventEnvelope::new(RunId::new(run), Source::Hook, e)
    }

    #[test]
    fn a_session_creates_a_run_and_a_project() {
        let mut w = World::new();
        let (env, changes) = w.apply(
            env(
                "s1",
                Event::SessionStarted {
                    cwd: PathBuf::from("/tmp/repo"),
                    source: Some("startup".into()),
                    model: Some("claude-opus-5".into()),
                    entrypoint: Some("cli".into()),
                },
            ),
            RunHint {
                cwd: Some(PathBuf::from("/tmp/repo")),
                ..Default::default()
            },
        );
        assert!(env.project_id.is_some());
        assert_eq!(w.runs().count(), 1);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, Change::ProjectDiscovered(_)))
        );
        assert_eq!(w.summary().working, 1);
    }

    #[test]
    fn a_worktree_belongs_to_the_repository_that_owns_it() {
        // Five parallel worktrees must be five runs on one project, not five
        // projects nobody registered.
        let mut w = World::new();
        for (i, path) in [
            "/tmp/repo",
            "/tmp/repo/.claude/worktrees/feature-a",
            "/tmp/repo/.claude/worktrees/feature-b",
        ]
        .iter()
        .enumerate()
        {
            w.apply(
                env(
                    &format!("s{i}"),
                    Event::SessionStarted {
                        cwd: PathBuf::from(path),
                        source: None,
                        model: None,
                        entrypoint: None,
                    },
                ),
                RunHint {
                    cwd: Some(PathBuf::from(path)),
                    ..Default::default()
                },
            );
        }
        assert_eq!(w.runs().count(), 3);
        assert_eq!(w.projects().count(), 1, "one repository, one project");
    }

    #[test]
    fn the_inbox_is_derived_and_ranked() {
        let mut w = World::new();
        w.apply(
            env(
                "s1",
                Event::QuestionAsked {
                    question: "Keep the legacy route?".into(),
                    options: vec!["yes".into(), "no".into()],
                },
            ),
            RunHint::default(),
        );
        w.apply(
            env(
                "s2",
                Event::ApiRequest {
                    usage: vibeplane_domain::event::ApiUsage {
                        model: Some("claude-opus-5".into()),
                        input_tokens: 190_000,
                        ..Default::default()
                    },
                },
            ),
            RunHint::default(),
        );
        let inbox = w.inbox();
        assert_eq!(inbox.len(), 2);
        // The question outranks the context warning: level first, then age.
        assert_eq!(inbox[0].kind, vibeplane_domain::AttentionKind::Question);
    }

    #[test]
    fn replaying_the_log_reproduces_the_state() {
        let events = vec![
            env(
                "s1",
                Event::SessionStarted {
                    cwd: PathBuf::from("/tmp/repo"),
                    source: None,
                    model: Some("claude-opus-5".into()),
                    entrypoint: None,
                },
            ),
            env(
                "s1",
                Event::ToolStarted {
                    tool: "Bash".into(),
                    input: serde_json::json!({"command": "cargo test"}),
                },
            ),
            env(
                "s1",
                Event::Blocked {
                    waiting_for: WaitingFor::Permission,
                    message: Some("Bash".into()),
                    request_id: None,
                },
            ),
        ];
        let mut a = World::new();
        for e in events.clone() {
            a.apply(e, RunHint::default());
        }
        let mut b = World::new();
        b.replay(events);
        assert_eq!(
            a.run(&RunId::new("s1")).unwrap().state,
            b.run(&RunId::new("s1")).unwrap().state
        );
        assert_eq!(a.inbox().len(), b.inbox().len());
    }

    #[test]
    fn reconciliation_marks_dead_runs_lost() {
        let mut w = World::new();
        w.apply(
            env(
                "s1",
                Event::ToolStarted {
                    tool: "Bash".into(),
                    input: serde_json::json!({}),
                },
            ),
            RunHint::default(),
        );
        let lost = w.reconcile(&|_| false);
        assert_eq!(lost.len(), 1);
        assert_eq!(w.run(&RunId::new("s1")).unwrap().state, RunState::Lost);
        assert_eq!(w.inbox()[0].kind, vibeplane_domain::AttentionKind::Lost);
    }
}

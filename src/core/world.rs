//! The authoritative in-memory state, and the only place runs are created.
//! Surfaces read this rather than reconstructing state from events; the
//! reducer is pure and this struct is the only thing that mutates.

use crate::core::attention::{AttentionConfig, AttentionItem, Derived, run_items_at};
use crate::core::event::{Event, EventEnvelope};
use crate::core::ids::{ProjectId, RunId, SessionId};
use crate::core::project::Project;
use crate::core::reduce;
use crate::core::run::{Run, RunMode, RunState};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What is wrong with the machine itself rather than any run on it — facts no
/// run-derived item can express. Default is a healthy machine.
pub struct Health<'a> {
    /// Why the installed gate did not refuse a call its own rule denies, when
    /// the host last asked it. `None` when it answered.
    pub gate_down: Option<&'a str>,
    /// Repository roots whose `devplane.toml` will not parse, with the
    /// parser's reason.
    pub broken_configs: &'a [(PathBuf, String)],
    /// Events and decisions the store would not take, and the last reason.
    /// `(0, 0, _)` raises nothing.
    pub unwritten: (u64, u64, Option<&'a str>),
    /// Agents a previous host started that still run unattached, as
    /// `(pid, command, worktree)`, read from the process table at startup.
    pub leaked_agents: &'a [(u32, String, Option<String>)],
    /// When these facts were first known — the host's start — so machine rows
    /// age from then rather than always reading as new.
    pub since: jiff::Timestamp,
    /// The instant the whole inbox is derived as of, so two runs cannot
    /// straddle a stall threshold and nothing here reads the clock.
    pub now: jiff::Timestamp,
    /// The `check` commands each project declares now, read by the caller;
    /// a project missing here is judged without them.
    pub checks: &'a [(ProjectId, Vec<String>)],
}

impl Default for Health<'_> {
    fn default() -> Self {
        Self {
            gate_down: None,
            broken_configs: &[],
            unwritten: (0, 0, None),
            leaked_agents: &[],
            since: jiff::Timestamp::UNIX_EPOCH,
            now: jiff::Timestamp::UNIX_EPOCH,
            checks: &[],
        }
    }
}

/// Everything `devplane modes` and `/api/modes` say: which projects decide
/// without you, and what can answer in your name. One type for writer and
/// reader, so a renamed field is a compile error.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Modes {
    pub projects: Vec<ProjectModes>,
    pub sessions: usize,
    pub unsupervised: usize,
    pub unknown: usize,
    /// Sessions that have reported no mode at all. The CLI guards its
    /// reassuring line on this, so it must never silently read as zero.
    pub unreported: usize,
    /// What can answer a question in your name on this machine, if anything.
    pub question_clock: Option<crate::core::clock::ClockLine>,
    /// Live sessions whose environment was read; one started before
    /// `devplane connect` has no reading.
    pub clock_read: usize,
    pub clock_unread: usize,
}

/// One project's live sessions and what is supervising them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectModes {
    pub project: Option<String>,
    pub name: Option<String>,
    /// Sessions whose mode means no person is asked about an ordinary call.
    pub unsupervised: usize,
    /// Sessions whose mode this build does not recognise.
    pub unknown: usize,
    /// Sessions that have not reported a mode — a different, unalarming fact.
    pub unreported: usize,
    pub sessions: Vec<SessionMode>,
}

/// One session's permission mode, as its vendor reported it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionMode {
    pub run: String,
    pub name: Option<String>,
    /// The vendor's own spelling, or `None` when nothing has reported one yet.
    pub mode: Option<String>,
    pub label: Option<String>,
    pub asks_a_person: Option<bool>,
    pub seen: Option<String>,
    /// The mode an ACP agent declared for itself, in its own spelling. Not a
    /// fallback for `mode`: nothing maps it, so no `asks_a_person` derives
    /// from it.
    pub agent_mode: Option<String>,
    pub agent_mode_seen: Option<String>,
    /// A clock this session's environment put on its questions
    /// (`CLAUDE_AFK_TIMEOUT_MS` overrides the settings files).
    pub question_clock: Option<crate::core::clock::ClockLine>,
    /// Whether the environment was read at all. `false` is not *nothing is
    /// set*, and must not render as *your questions wait for you*.
    pub clock_read: bool,
}

/// Everything the host knows right now. `Clone` so surfaces are composed from
/// a snapshot rather than under the lock, which would stall ingest.
#[derive(Debug, Default, Clone)]
pub struct World {
    runs: BTreeMap<RunId, Run>,
    projects: BTreeMap<ProjectId, Project>,
    pub attention: AttentionConfig,
    /// Where a directory's project root is. Answering reads the disk, so the
    /// edge supplies it ([`World::new`] outside this module); the default is
    /// the directory itself.
    roots: Roots,
    /// Each directory's root, asked once: the answer is file I/O, and an event
    /// arrives on every tool call.
    root_of: BTreeMap<PathBuf, PathBuf>,
}

/// The function that names a directory's project root, supplied from outside
/// the pure half. `None` means the directory is its own root.
#[derive(Clone, Copy)]
pub struct Roots(pub fn(&Path) -> Option<PathBuf>);

impl Default for Roots {
    fn default() -> Self {
        Self(|_| None)
    }
}

impl std::fmt::Debug for Roots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Roots")
    }
}

/// What changed, so a subscriber can be told without diffing the whole world.
#[derive(Debug, Clone, PartialEq)]
pub enum Changed {
    RunUpserted(RunId),
    ProjectDiscovered(ProjectId),
}

impl World {
    /// A world whose projects are resolved by `roots`.
    pub fn with_roots(roots: Roots) -> Self {
        Self {
            roots,
            ..Self::default()
        }
    }

    pub fn runs(&self) -> impl Iterator<Item = &Run> {
        self.runs.values()
    }

    pub fn run(&self, id: &RunId) -> Option<&Run> {
        self.runs.get(id)
    }

    /// Turns what a person typed — including the short label the board
    /// prints — into a run. In order: the exact id, a unique id prefix, then a
    /// unique session name, whole or with its `<project>-` prefix stripped.
    /// Ambiguity names the candidates rather than picking one.
    pub fn resolve_run(&self, needle: &str, now: jiff::Timestamp) -> Result<RunId, Ambiguous> {
        if needle.is_empty() {
            return Err(Ambiguous::NotFound);
        }
        if self.runs.contains_key(&RunId::new(needle)) {
            return Ok(RunId::new(needle));
        }
        let matches = |r: &Run| {
            r.id.as_str().starts_with(needle)
                || r.name.as_deref().is_some_and(|n| {
                    n == needle
                        || n.rsplit_once('-')
                            .is_some_and(|(_, suffix)| suffix == needle)
                })
        };
        let mut found: Vec<&Run> = self.runs.values().filter(|r| matches(r)).collect();
        match found.len() {
            0 => Err(Ambiguous::NotFound),
            1 => Ok(found.remove(0).id.clone()),
            _ => {
                // The active ones first: a dormant tab from Tuesday sharing a
                // prefix should not make today's session unaddressable.
                let live: Vec<&Run> = found
                    .iter()
                    .copied()
                    .filter(|r| r.is_active_at(now))
                    .collect();
                if live.len() == 1 {
                    return Ok(live[0].id.clone());
                }
                Err(Ambiguous::Several(
                    found.iter().map(|r| r.id.as_str().to_string()).collect(),
                ))
            }
        }
    }

    pub fn projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.values()
    }

    pub fn project(&self, id: &ProjectId) -> Option<&Project> {
        self.projects.get(id)
    }

    /// For facts from outside this module, such as the git remote.
    pub fn project_mut(&mut self, id: &ProjectId) -> Option<&mut Project> {
        self.projects.get_mut(id)
    }

    /// The board, by most recent real activity (not when the host noticed it).
    pub fn board(&self) -> Vec<&Run> {
        let mut v: Vec<&Run> = self.runs.values().collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.last_activity_at));
        v
    }

    /// The runs worth looking at: in play, or asking for something. Dormant
    /// editor tabs are left out.
    pub fn working_set(&self, now: jiff::Timestamp) -> Vec<&Run> {
        self.board()
            .into_iter()
            .filter(|r| r.is_active_at(now))
            .collect()
    }

    /// The inbox, derived fresh every time, with the machine-wide stall
    /// threshold. The host calls [`World::inbox_with_health`] instead.
    pub fn inbox(&self, now: jiff::Timestamp) -> Vec<AttentionItem> {
        self.inbox_with_health(
            &[],
            &std::collections::BTreeSet::new(),
            &|_| None,
            &Health {
                now,
                ..Health::default()
            },
            Vec::new(),
            Vec::new(),
        )
        .items
    }

    /// The inbox, plus items about the machine itself. [`Health`], `forge` and
    /// `from_disk` are threaded in because this module may not touch the
    /// outside world; ranking here keeps one list and one order.
    pub fn inbox_with_health(
        &self,
        changes: &[crate::core::Change],
        drivable: &std::collections::BTreeSet<RunId>,
        stall_for: &dyn Fn(&Path) -> Option<i64>,
        health: &Health<'_>,
        forge: Vec<AttentionItem>,
        from_disk: Vec<AttentionItem>,
    ) -> Derived {
        // The same runs the board shows, so the surfaces agree; `is_active`
        // keeps every blocked run. One instant for the whole pass, so two runs
        // cannot straddle a stall threshold.
        let now = health.now;
        let from_runs: Derived = self
            .runs
            .values()
            .filter(|r| r.is_active_at(now))
            .map(|r| {
                let stall = stall_for(r.working_dir()).unwrap_or(self.attention.stall_seconds);
                run_items_at(r, &self.attention, stall, now)
            })
            .collect();
        let from_change: Derived = changes
            .iter()
            .map(|w| {
                let can_drive = w.current_run().is_some_and(|r| drivable.contains(r));
                // Resumable: the agent named a session Devplane no longer holds.
                let can_resume = !can_drive
                    && w.current_run()
                        .and_then(|r| self.runs.get(r))
                        .is_some_and(|r| r.mode == RunMode::Driven && r.agent_session.is_some());
                let repo = self.projects.get(&w.project_id).and_then(|p| p.repo_slug());
                let checks = health
                    .checks
                    .iter()
                    .find(|(p, _)| *p == w.project_id)
                    .map(|(_, c)| c.as_slice());
                crate::core::attention::change_items_in(
                    w,
                    can_drive,
                    can_resume,
                    repo.as_deref(),
                    checks,
                    now,
                )
            })
            .collect();
        let since = health.since;
        let gate = health
            .gate_down
            .map(|why| crate::core::attention::gate_down_item_at(why, since));
        let configs = health
            .broken_configs
            .iter()
            .map(|(root, why)| crate::core::attention::config_broken_item_at(root, why, since));
        let leaked = health.leaked_agents.iter().map(|(pid, command, worktree)| {
            crate::core::attention::agent_leaked_item_at(*pid, command, worktree.as_deref(), since)
        });
        let (lost_events, lost_decisions, last_loss) = health.unwritten;
        let record = (lost_events > 0 || lost_decisions > 0).then(|| {
            crate::core::attention::record_incomplete_item_at(
                lost_events,
                lost_decisions,
                last_loss.unwrap_or("The store gave no reason."),
                since,
            )
        });
        let mut all = Derived {
            items: from_runs.items,
            snoozed: from_runs.snoozed + from_change.snoozed,
        };
        all.items.extend(from_change.items);
        all.items.extend(gate);
        all.items.extend(configs);
        all.items.extend(record);
        all.items.extend(leaked);
        all.items.extend(forge);
        all.items.extend(from_disk);
        all.rank()
    }

    /// Which projects are deciding without you, and since when we knew. Live
    /// sessions only. A session that has not reported a mode (`PreToolUse`
    /// does not carry it) is a row, not a gap, so a partial answer never looks
    /// complete.
    pub fn permission_modes(&self) -> Vec<ProjectModes> {
        let mut by_project: BTreeMap<Option<ProjectId>, Vec<&Run>> = BTreeMap::new();
        for r in self.runs.values().filter(|r| r.state.is_live()) {
            by_project.entry(r.project_id.clone()).or_default().push(r);
        }
        by_project
            .into_iter()
            .map(|(pid, runs)| {
                let name = pid
                    .as_ref()
                    .and_then(|id| self.projects.get(id))
                    .map(|p| p.name.clone())
                    .or_else(|| pid.as_ref().map(|p| p.to_string()));
                let mut modes: Vec<SessionMode> = runs
                    .iter()
                    .map(|r| SessionMode {
                        run: r.id.to_string(),
                        name: r.name.clone(),
                        mode: r.permission_mode.as_ref().map(|m| m.as_str().to_string()),
                        label: r.permission_mode.as_ref().map(|m| m.label().to_string()),
                        // `None` is *we do not know*, not *nobody is asked*.
                        asks_a_person: r.permission_mode.as_ref().and_then(|m| m.asks_a_person()),
                        // "Seen", never "since": nothing announces a mode
                        // change, so this is when Devplane first heard it.
                        seen: r.permission_mode_seen.map(|t| t.to_string()),
                        agent_mode: r.agent_mode.clone(),
                        agent_mode_seen: r.agent_mode_seen.map(|t| t.to_string()),
                        question_clock: r.question_clock.as_ref().map(Into::into),
                        clock_read: r.question_clock_read.is_some(),
                    })
                    .collect();
                // First a session closing its questions immediately (a
                // question no longer waits for you), then a clock the person
                // did not choose, then least supervised, unreadable, and
                // quiet. A `never` clock ranks by mode like no clock.
                modes.sort_by_key(|m| {
                    let rank = match m.question_clock.as_ref() {
                        Some(c) if c.immediate => -2,
                        Some(c) if c.answers_for_you && !c.chosen_by_the_person => -1,
                        _ => match (m.mode.is_some(), m.asks_a_person) {
                            (true, Some(false)) => 0,
                            (true, None) => 1,
                            (true, Some(true)) => 2,
                            (false, _) => 3,
                        },
                    };
                    (rank, m.run.clone())
                });
                ProjectModes {
                    project: pid.map(|p| p.to_string()),
                    name,
                    unsupervised: modes
                        .iter()
                        .filter(|m| m.asks_a_person == Some(false))
                        .count(),
                    // Said something unreadable, versus said nothing.
                    unknown: modes
                        .iter()
                        .filter(|m| m.mode.is_some() && m.asks_a_person.is_none())
                        .count(),
                    unreported: modes.iter().filter(|m| m.mode.is_none()).count(),
                    sessions: modes,
                }
            })
            .collect()
    }

    /// Registers a project. Only explicit registration can grant trust;
    /// discovery never does.
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

    /// Finds or discovers the project a path belongs to. A worktree under
    /// `.claude/worktrees/` resolves to the repository that owns it.
    pub fn resolve_project(&mut self, path: &Path) -> Option<(ProjectId, Option<Changed>)> {
        // A relative path (an event with no directory falls back to `.`)
        // belongs to no project.
        if !path.is_absolute() {
            return None;
        }
        // As the filesystem names it, so one directory is never two projects;
        // asked once per directory, not once per event.
        let root = match self.root_of.get(path) {
            Some(root) => root.clone(),
            None => {
                let root = (self.roots.0)(path).unwrap_or_else(|| path.to_path_buf());
                self.root_of.insert(path.to_path_buf(), root.clone());
                root
            }
        };

        let id = ProjectId::from_path(&root);
        if self.projects.contains_key(&id) {
            return Some((id, None));
        }
        // An unseen directory becomes an (untrusted) project on sight.
        let mut p = Project::from_root(root);
        p.auto_discovered = true;
        let id = self.upsert_project(p);
        Some((id.clone(), Some(Changed::ProjectDiscovered(id))))
    }

    /// Applies an event, creating the run on first sight. Returns the envelope
    /// with its project filled in, so the caller persists exactly what applied.
    pub fn apply(
        &mut self,
        mut env: EventEnvelope,
        hint: RunHint,
    ) -> (EventEnvelope, Vec<Changed>) {
        let mut changes = Vec::new();

        let cwd = hint.cwd.clone().unwrap_or_else(|| {
            self.runs
                .get(&env.run_id)
                .map(|r| r.cwd.clone())
                .unwrap_or_else(|| PathBuf::from("."))
        });

        if !self.runs.contains_key(&env.run_id) {
            let session = SessionId::new(env.run_id.as_str());
            let run = Run::new_at(session, cwd.clone(), hint.mode, &hint.agent, env.at);
            self.runs.insert(env.run_id.clone(), run);
        }

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
            if hint.agent_command.is_some() && run.agent_command.is_none() {
                run.agent_command = hint.agent_command.clone();
            }
            if run.mode == RunMode::Observed && hint.mode != RunMode::Observed {
                // The stronger claim wins: it says who supervises the process.
                run.mode = hint.mode;
            }
            reduce::apply(run, &env);
        }
        changes.push(Changed::RunUpserted(env.run_id.clone()));
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

    /// Takes trust back when a grant's row never reached the store, so trust
    /// cannot quietly expire at the next restart.
    pub fn untrust(&mut self, id: &ProjectId) -> bool {
        match self.projects.get_mut(id) {
            Some(p) => {
                p.trusted = false;
                true
            }
            None => false,
        }
    }

    /// Hides the run's *current* inbox item kinds until `until`, or shows
    /// everything when `None`. A kind that appears later is shown
    /// ([`Snoozed`](crate::core::attention::Snoozed)).
    pub fn snooze(
        &mut self,
        run: &RunId,
        until: Option<jiff::Timestamp>,
        now: jiff::Timestamp,
        stall_seconds: i64,
    ) -> bool {
        let Some(existing) = self.runs.get(run) else {
            return false;
        };
        let kinds: Vec<_> = run_items_at(existing, &self.attention, stall_seconds, now)
            .items
            .into_iter()
            .map(|i| i.kind)
            .collect();
        let r = self.runs.get_mut(run).expect("checked above");
        match until {
            Some(t) => r.snoozed.hide(kinds, t),
            None => r.snoozed.clear(),
        }
        true
    }

    /// Loads persisted runs. The caller must [`reconcile`](Self::reconcile)
    /// them against the machine before they reach the board.
    pub fn restore_runs(&mut self, runs: impl IntoIterator<Item = Run>) {
        for run in runs {
            if let Some(pid) = run.project_id.clone()
                && !self.projects.contains_key(&pid)
            {
                // Keep the reference resolvable if the project row is gone.
                let mut p = Project::from_root(run.cwd.clone());
                p.id = pid;
                p.auto_discovered = true;
                self.upsert_project(p);
            }
            self.runs.insert(run.id.clone(), run);
        }
    }

    /// Records the entrypoint a session runs on (`cli`, `claude-vscode`, …;
    /// only OpenTelemetry names it) and the repository it reported.
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

    /// Records the context window a driven agent reported for itself.
    pub fn set_context_window(&mut self, run: &RunId, window: u64) {
        if let Some(r) = self.runs.get_mut(run)
            && window > 0
        {
            r.totals.context_window = Some(window);
        }
    }

    /// [`apply`](Self::apply) for a stored event, at most once per run: an
    /// event whose `seq` the run already holds is skipped and returns `None`.
    pub fn apply_stored(
        &mut self,
        seq: i64,
        env: EventEnvelope,
        hint: RunHint,
    ) -> Option<(EventEnvelope, Vec<Changed>)> {
        if self
            .runs
            .get(&env.run_id)
            .is_some_and(|r| r.last_seq >= seq)
        {
            return None;
        }
        let (applied, changes) = self.apply(env, hint);
        if let Some(r) = self.runs.get_mut(&applied.run_id) {
            r.last_seq = r.last_seq.max(seq);
        }
        Some((applied, changes))
    }

    /// [`replay`](Self::replay) for stored events with their `seq`.
    pub fn replay_stored(&mut self, events: impl IntoIterator<Item = (i64, EventEnvelope)>) {
        for (seq, env) in events {
            let hint = replay_hint(&env);
            self.apply_stored(seq, env, hint);
        }
    }

    /// Re-derives state from a replay of events.
    pub fn replay(&mut self, events: impl IntoIterator<Item = EventEnvelope>) {
        for env in events {
            let hint = replay_hint(&env);
            self.apply(env, hint);
        }
    }

    /// Records that live runs whose process is gone have ended, and returns
    /// the events so the caller can persist them.
    pub fn reconcile(
        &mut self,
        alive: &dyn Fn(&Run) -> bool,
        now: jiff::Timestamp,
    ) -> Vec<EventEnvelope> {
        let mut out = Vec::new();
        let ids: Vec<RunId> = self
            .runs
            .values()
            .filter(|r| r.state.is_live())
            .map(|r| r.id.clone())
            .collect();
        for id in ids {
            let Some(run) = self.runs.get(&id) else {
                continue;
            };
            if alive(run) {
                continue;
            }
            // One fact — the process is gone; the reducer decides from what the
            // run was doing whether that is `Lost` or merely stopped.
            {
                let env = EventEnvelope::at(
                    id.clone(),
                    crate::core::event::Source::Host,
                    Event::Lost {
                        reason: "process not found at startup".into(),
                    },
                    now,
                );
                if let Some(run) = self.runs.get_mut(&id) {
                    reduce::apply(run, &env);
                }
                out.push(env);
            }
        }
        out
    }

    /// Drops terminal runs older than `max_age_seconds` as of `now` from
    /// memory; the events stay on disk.
    pub fn prune(&mut self, now: jiff::Timestamp, max_age_seconds: i64) -> usize {
        let before = self.runs.len();
        self.runs.retain(|_, r| {
            r.state.is_live() || (now - r.last_event_at).get_seconds() < max_age_seconds
        });
        before - self.runs.len()
    }

    /// The counts for the board header. They partition the runs:
    /// `working + needs_you + idle + failed + dormant == runs`.
    pub fn summary(&self, now: jiff::Timestamp) -> BoardSummary {
        let mut s = BoardSummary {
            projects: self.projects.len(),
            runs: self.runs.len(),
            ..Default::default()
        };
        for r in self.runs.values() {
            // Summed over every run, dormant included.
            s.cost_usd += r.totals.cost_usd;
            if !r.is_active_at(now) {
                s.dormant += 1;
                continue;
            }
            match r.state {
                RunState::Working | RunState::Starting => s.working += 1,
                RunState::Waiting(_) => s.needs_you += 1,
                RunState::Idle => s.idle += 1,
                RunState::Failed | RunState::Lost => s.failed += 1,
                // Interrupted is not `needs_you`: its inbox item asks, and
                // counting it here too would charge attention twice.
                RunState::Completed | RunState::Stopped | RunState::Interrupted => s.idle += 1,
            }
            if r.state.is_live() {
                s.live += 1;
            }
        }
        s
    }
}

/// Why a name did not identify one run.
#[derive(Debug, Clone, PartialEq)]
pub enum Ambiguous {
    NotFound,
    /// It matched several, which are named so the user can pick one.
    Several(Vec<String>),
}

impl std::fmt::Display for Ambiguous {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ambiguous::NotFound => f.write_str("no such run"),
            Ambiguous::Several(ids) => write!(
                f,
                "that matches {} sessions — {}. Use more of the id.",
                ids.len(),
                ids.join(", ")
            ),
        }
    }
}

/// What the caller knows about a run that the event itself does not carry.
#[derive(Debug, Clone)]
pub struct RunHint {
    pub cwd: Option<PathBuf>,
    pub mode: RunMode,
    pub agent: String,
    /// How the agent was launched, where the caller knows. Only a driven run
    /// has one, and it is what lets that run be resumed later.
    pub agent_command: Option<String>,
}

impl Default for RunHint {
    fn default() -> Self {
        Self {
            cwd: None,
            mode: RunMode::Observed,
            agent: "claude".into(),
            agent_command: None,
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
    /// Sessions that exist but have never reported: editor tabs left open.
    pub dormant: usize,
    pub cost_usd: f64,
    /// Open issues across every registered project, from the last forge poll.
    /// Filled by the host, which holds that state; zero here.
    #[serde(default)]
    pub open_issues: usize,
    /// Open pull requests, the same way.
    #[serde(default)]
    pub open_prs: usize,
    /// Of those, the ones waiting on the person (see `ForgeCounts`).
    #[serde(default)]
    pub forge_needs_you: usize,
    /// Projects whose last forge poll failed: their last good counts are
    /// left out of the three above rather than passed off as current.
    #[serde(default)]
    pub forge_stale: usize,
    /// Questions and permissions still waiting on a person whose session is no
    /// longer running; filled by the host. Separate from `needs_you`, which
    /// partitions sessions, so an orphaned ask never reads as *0 need you*.
    #[serde(default)]
    pub asks_waiting: usize,
}

/// How a replayed event's run is hinted: its own directory where it names
/// one, watched rather than driven, the agent its source names.
fn replay_hint(env: &EventEnvelope) -> RunHint {
    let cwd = match &env.event {
        Event::SessionStarted { cwd, .. } | Event::CwdChanged { cwd } => Some(cwd.clone()),
        _ => None,
    };
    RunHint {
        cwd,
        mode: RunMode::Observed,
        agent: env.source.agent().unwrap_or("claude").to_string(),
        agent_command: None,
    }
}

#[cfg(test)]
mod clock_ranking_tests {
    use crate::core::clock::{After, ClockLine, QuestionClock, Source};

    fn line(after: After) -> ClockLine {
        (&QuestionClock {
            after,
            source: Source::Environment,
            where_set: crate::core::clock::ENV_KEY.into(),
        })
            .into()
    }

    #[test]
    fn a_session_that_answers_instantly_sorts_above_everything() {
        let immediate = line(After::Immediately);
        let idle = line(After::Idle("60s".into()));
        assert!(immediate.immediate);
        assert!(!idle.immediate, "a timer is not the same as no wait at all");

        // The sentences are different facts and are worded as such.
        assert_ne!(immediate.says, idle.says);
        assert!(!immediate.says.contains("0s"), "{}", immediate.says);
    }

    /// The line carries the sentence, so surfaces cannot word it differently.
    #[test]
    fn the_rendered_line_carries_the_sentence_rather_than_the_ingredients() {
        let l = line(After::Idle("5m".into()));
        assert_eq!(l.after, "5m");
        assert_eq!(l.where_set, crate::core::clock::ENV_KEY);
        assert!(
            !l.chosen_by_the_person,
            "an env var is not the person's own"
        );
        assert!(l.says.contains("5m"), "{}", l.says);
        assert!(l.says.contains("overrides"), "{}", l.says);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::WaitingFor;

    /// An envelope carrying the channel the event actually comes from.
    fn env(run: &str, e: Event) -> EventEnvelope {
        let source = e.test_source();
        EventEnvelope::new(RunId::new(run), source, e)
    }

    /// A run on the board, with the name the provider gave its session.
    fn seed(w: &mut World, id: &str, name: Option<&str>) {
        w.apply(
            env(
                id,
                Event::SessionStarted {
                    cwd: PathBuf::from("/tmp/repo"),
                    source: None,
                    model: None,
                    entrypoint: None,
                    question_clock: None,
                    clock_read: false,
                },
            ),
            RunHint {
                cwd: Some(PathBuf::from("/tmp/repo")),
                ..Default::default()
            },
        );
        if let Some(n) = name
            && let Some(r) = w.runs.get_mut(&RunId::new(id))
        {
            r.name = Some(n.to_string());
        }
    }

    #[test]
    fn the_id_the_board_prints_is_an_id_the_commands_accept() {
        let mut w = World::new();
        seed(&mut w, "7c4f9a20-1111-2222-3333-444455556666", None);
        seed(
            &mut w,
            "0a1b2c3d-9999-8888-7777-666655554444",
            Some("repo-a1"),
        );

        assert_eq!(
            w.resolve_run(
                "7c4f9a20-1111-2222-3333-444455556666",
                jiff::Timestamp::now()
            )
            .unwrap()
            .as_str(),
            "7c4f9a20-1111-2222-3333-444455556666"
        );
        // A prefix, the way git takes a short sha.
        assert_eq!(
            w.resolve_run("7c4f", jiff::Timestamp::now())
                .unwrap()
                .as_str(),
            "7c4f9a20-1111-2222-3333-444455556666"
        );
        assert_eq!(
            w.resolve_run("7c", jiff::Timestamp::now())
                .unwrap()
                .as_str()
                .len(),
            36
        );
        // The label the board actually prints for a named session.
        assert_eq!(
            w.resolve_run("a1", jiff::Timestamp::now())
                .unwrap()
                .as_str(),
            "0a1b2c3d-9999-8888-7777-666655554444"
        );
        assert_eq!(
            w.resolve_run("repo-a1", jiff::Timestamp::now())
                .unwrap()
                .as_str(),
            "0a1b2c3d-9999-8888-7777-666655554444"
        );
    }

    #[test]
    fn an_ambiguous_id_names_the_candidates_rather_than_guessing() {
        let mut w = World::new();
        seed(&mut w, "abc111", None);
        seed(&mut w, "abc222", None);
        // Both are live, so neither wins on activity.
        match w.resolve_run("abc", jiff::Timestamp::now()) {
            Err(Ambiguous::Several(ids)) => {
                assert_eq!(ids.len(), 2);
                assert!(ids.iter().any(|i| i == "abc111"));
            }
            other => panic!("expected an ambiguity, got {other:?}"),
        }
        assert_eq!(
            w.resolve_run("zzz", jiff::Timestamp::now()),
            Err(Ambiguous::NotFound)
        );
        assert_eq!(
            w.resolve_run("", jiff::Timestamp::now()),
            Err(Ambiguous::NotFound)
        );
    }

    #[test]
    fn a_dormant_tab_does_not_make_todays_session_unaddressable() {
        let mut w = World::new();
        seed(&mut w, "abc111", None);
        seed(&mut w, "abc222", None);
        // The second never reported: a tab left open, not a session in use.
        if let Some(r) = w.runs.get_mut(&RunId::new("abc222")) {
            r.reporting = false;
            r.state = RunState::Idle;
        }
        assert!(
            !w.run(&RunId::new("abc222"))
                .unwrap()
                .is_active_at(jiff::Timestamp::now())
        );
        assert_eq!(
            w.resolve_run("abc", jiff::Timestamp::now())
                .unwrap()
                .as_str(),
            "abc111"
        );
    }

    /// Projection is at-least-once, and `turns` feeds the budget.
    #[test]
    fn replaying_a_stored_batch_twice_changes_nothing() {
        let batch = vec![
            (
                1,
                env(
                    "s1",
                    Event::SessionStarted {
                        cwd: PathBuf::from("/tmp/repo"),
                        source: None,
                        model: None,
                        entrypoint: None,
                        question_clock: None,
                        clock_read: false,
                    },
                ),
            ),
            (2, env("s1", Event::TurnEnded)),
            (3, env("s1", Event::TurnEnded)),
        ];
        let mut w = World::new();
        w.replay_stored(batch.clone());
        let once = w.run(&RunId::new("s1")).cloned().unwrap();
        assert_eq!(once.totals.turns, 2);
        assert_eq!(once.last_seq, 3);
        w.replay_stored(batch.clone());
        assert_eq!(w.run(&RunId::new("s1")), Some(&once));

        // And across a restart: the run as saved, the batch again.
        let mut w = World::new();
        w.restore_runs([once.clone()]);
        w.replay_stored(batch);
        assert_eq!(w.run(&RunId::new("s1")).unwrap().totals.turns, 2);
    }

    #[test]
    fn an_event_with_no_directory_registers_no_project() {
        let mut w = World::new();
        let (env, _) = w.apply(env("s9", Event::TurnEnded), RunHint::default());
        assert!(env.project_id.is_none());
        assert_eq!(w.projects().count(), 0);
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
                    question_clock: None,
                    clock_read: false,
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
                .any(|c| matches!(c, Changed::ProjectDiscovered(_)))
        );
        assert_eq!(w.summary(jiff::Timestamp::now()).working, 1);
    }

    #[test]
    fn a_worktree_belongs_to_the_repository_that_owns_it() {
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
                        question_clock: None,
                        clock_read: false,
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
                    ask: None,
                    request_id: None,
                    form: None,
                },
            ),
            RunHint::default(),
        );
        w.apply(
            env(
                "s2",
                Event::ApiRequest {
                    usage: crate::core::event::ApiUsage {
                        model: Some("claude-opus-5".into()),
                        input_tokens: 190_000,
                        ..Default::default()
                    },
                },
            ),
            RunHint::default(),
        );
        let inbox = w.inbox(jiff::Timestamp::now());
        assert_eq!(inbox.len(), 2);
        // The question outranks the context warning: level first, then age.
        assert_eq!(inbox[0].kind, crate::core::AttentionKind::Question);
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
                    question_clock: None,
                    clock_read: false,
                },
            ),
            env(
                "s1",
                Event::tool_started("Bash", serde_json::json!({"command": "cargo test"})),
            ),
            env(
                "s1",
                Event::Blocked {
                    waiting_for: WaitingFor::Permission,
                    message: Some("Bash".into()),
                    request_id: None,
                    ask: None,
                    options: vec![],
                    call: None,
                    context: None,
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
        assert_eq!(
            a.inbox(jiff::Timestamp::now()).len(),
            b.inbox(jiff::Timestamp::now()).len()
        );
    }

    fn roster_row(status: Option<&str>, started_ms: i64) -> Event {
        Event::RosterSeen {
            jobs: None,
            kind: "interactive".into(),
            state: None,
            status: status.map(Into::into),
            waiting_for: None,
            pid: Some(1),
            name: Some("repo-a1".into()),
            entrypoint: Some("claude-vscode".into()),
            started_at_ms: Some(started_ms),
        }
    }

    #[test]
    fn a_tab_left_open_for_days_is_not_the_working_set() {
        let mut w = World::new();
        let three_days_ago =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(72)).as_millisecond();

        w.apply(
            env("dormant", roster_row(None, three_days_ago)),
            RunHint::default(),
        );
        w.apply(
            env("busy", roster_row(Some("busy"), three_days_ago)),
            RunHint::default(),
        );

        assert_eq!(w.runs().count(), 2, "both are still on the board");
        let working: Vec<_> = w
            .working_set(jiff::Timestamp::now())
            .iter()
            .map(|r| r.id.clone())
            .collect();
        assert_eq!(working, vec![RunId::new("busy")]);
        assert_eq!(w.summary(jiff::Timestamp::now()).dormant, 1);
    }

    #[test]
    fn the_age_shown_is_the_sessions_own_not_the_moment_we_noticed_it() {
        let mut w = World::new();
        let two_days_ago =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(48)).as_millisecond();
        w.apply(
            env("old", roster_row(None, two_days_ago)),
            RunHint::default(),
        );

        let run = w.run(&RunId::new("old")).unwrap();
        assert!(
            run.idle_seconds_at(jiff::Timestamp::now()) > 47 * 3600,
            "expected roughly two days, got {}s",
            run.idle_seconds_at(jiff::Timestamp::now())
        );
    }

    #[test]
    fn a_session_that_says_anything_joins_the_working_set() {
        let mut w = World::new();
        let long_ago =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(72)).as_millisecond();
        w.apply(env("s1", roster_row(None, long_ago)), RunHint::default());
        assert!(w.working_set(jiff::Timestamp::now()).is_empty());

        w.apply(
            env("s1", Event::tool_started("Bash", serde_json::json!({}))),
            RunHint::default(),
        );
        assert_eq!(
            w.working_set(jiff::Timestamp::now()).len(),
            1,
            "a hook makes it real"
        );
        assert_eq!(w.summary(jiff::Timestamp::now()).dormant, 0);
    }

    #[test]
    fn a_dormant_session_that_needs_a_human_is_never_hidden() {
        let mut w = World::new();
        let long_ago =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(72)).as_millisecond();
        w.apply(env("s1", roster_row(None, long_ago)), RunHint::default());
        w.apply(
            env(
                "s1",
                Event::QuestionAsked {
                    question: "which one?".into(),
                    options: vec![],
                    ask: None,
                    request_id: None,
                    form: None,
                },
            ),
            RunHint::default(),
        );
        assert_eq!(w.working_set(jiff::Timestamp::now()).len(), 1);
        assert_eq!(w.inbox(jiff::Timestamp::now()).len(), 1);
    }

    #[test]
    fn reconciliation_marks_dead_runs_lost() {
        let mut w = World::new();
        w.apply(
            env("s1", Event::tool_started("Bash", serde_json::json!({}))),
            RunHint::default(),
        );
        let lost = w.reconcile(&|_| false, jiff::Timestamp::now());
        assert_eq!(lost.len(), 1);
        assert_eq!(w.run(&RunId::new("s1")).unwrap().state, RunState::Lost);
        assert_eq!(
            w.inbox(jiff::Timestamp::now())[0].kind,
            crate::core::AttentionKind::Lost
        );
    }

    /// A closed editor tab is not a loss.
    #[test]
    fn a_tab_that_closed_has_ended_and_work_that_vanished_is_lost() {
        let mut w = World::new();
        seed(&mut w, "idle-one", None);
        seed(&mut w, "busy-one", None);
        w.runs.get_mut(&RunId::new("idle-one")).unwrap().state = RunState::Idle;
        w.apply(
            env(
                "busy-one",
                Event::tool_started("Bash", serde_json::json!({"command": "cargo test"})),
            ),
            RunHint::default(),
        );
        assert_eq!(
            w.run(&RunId::new("busy-one")).unwrap().state,
            RunState::Working
        );

        // Neither process exists any more.
        let events = w.reconcile(&|_| false, jiff::Timestamp::now());
        assert_eq!(events.len(), 2);

        let idle = w.run(&RunId::new("idle-one")).unwrap();
        assert_eq!(idle.state, RunState::Stopped, "a tab closing is not a loss");
        let busy = w.run(&RunId::new("busy-one")).unwrap();
        assert_eq!(busy.state, RunState::Lost, "work stopped mid-flight is");
        assert_eq!(
            busy.summary.as_deref(),
            Some("Bash: cargo test"),
            "what it was doing is the only useful thing left; the diagnosis used to overwrite it"
        );

        // And only the loss reaches the inbox.
        let kinds: Vec<_> = w
            .inbox(jiff::Timestamp::now())
            .into_iter()
            .map(|i| i.kind)
            .collect();
        assert_eq!(kinds, [crate::core::AttentionKind::Lost]);
    }

    /// A repository whose rules will not parse has inert deny rules and must
    /// say so, even on an otherwise quiet machine.
    #[test]
    fn a_configuration_that_will_not_parse_is_a_critical_item_on_a_quiet_machine() {
        let w = World::new();
        let broken = vec![(
            PathBuf::from("/repos/payments-api"),
            "TOML parse error at line 12".to_string(),
        )];
        let inbox = w
            .inbox_with_health(
                &[],
                &Default::default(),
                &|_| None,
                &Health {
                    broken_configs: &broken,
                    now: jiff::Timestamp::now(),
                    ..Default::default()
                },
                Vec::new(),
                Vec::new(),
            )
            .items;
        assert_eq!(inbox.len(), 1, "nothing else is wrong and this still shows");
        let item = &inbox[0];
        assert_eq!(item.kind, crate::core::AttentionKind::ConfigBroken);
        assert_eq!(item.level, crate::core::attention::Level::Critical);
        assert!(item.title.contains("payments-api"), "{}", item.title);
        // The parser's own words, and the command that prints the line.
        let detail = item.detail.as_deref().unwrap_or_default();
        assert!(detail.contains("line 12"), "{detail}");
        assert!(
            detail.contains("devplane check /repos/payments-api"),
            "{detail}"
        );
        // Nothing to click: committed rules are not rewritten from the inbox.
        assert!(item.actions.is_empty());

        // Two repositories, two items, with ids stable across polls.
        let both = vec![
            broken[0].clone(),
            (
                PathBuf::from("/repos/web"),
                "unknown field `gate`".to_string(),
            ),
        ];
        let inbox = w
            .inbox_with_health(
                &[],
                &Default::default(),
                &|_| None,
                &Health {
                    broken_configs: &both,
                    now: jiff::Timestamp::now(),
                    ..Default::default()
                },
                Vec::new(),
                Vec::new(),
            )
            .items;
        assert_eq!(inbox.len(), 2);
        let again = w
            .inbox_with_health(
                &[],
                &Default::default(),
                &|_| None,
                &Health {
                    broken_configs: &both,
                    now: jiff::Timestamp::now(),
                    ..Default::default()
                },
                Vec::new(),
                Vec::new(),
            )
            .items;
        let ids: Vec<_> = inbox.iter().map(|i| i.id.clone()).collect();
        let ids_again: Vec<_> = again.iter().map(|i| i.id.clone()).collect();
        assert_eq!(ids, ids_again, "the id must not move between polls");
    }

    /// A failed store write is dropped on purpose (rather than stalling a
    /// blocked hook), so the hole in the record must surface as a row.
    #[test]
    fn a_record_with_a_hole_in_it_says_so_and_a_whole_one_stays_quiet() {
        let w = World::new();
        let quiet = w
            .inbox_with_health(
                &[],
                &Default::default(),
                &|_| None,
                &Health {
                    now: jiff::Timestamp::now(),
                    ..Health::default()
                },
                Vec::new(),
                Vec::new(),
            )
            .items;
        assert!(
            quiet.is_empty(),
            "a healthy machine raises nothing: {quiet:?}"
        );

        let inbox = w
            .inbox_with_health(
                &[],
                &Default::default(),
                &|_| None,
                &Health {
                    now: jiff::Timestamp::now(),
                    unwritten: (3, 1, Some("database or disk is full")),
                    ..Default::default()
                },
                Vec::new(),
                Vec::new(),
            )
            .items;
        assert_eq!(inbox.len(), 1);
        let item = &inbox[0];
        assert_eq!(item.kind, crate::core::AttentionKind::RecordIncomplete);
        // Critical: every other screen reads the now-incomplete record.
        assert_eq!(item.level, crate::core::attention::Level::Critical);
        // Both counts, said separately — a lost decision is not a lost event.
        assert!(
            item.title.contains('3') && item.title.contains('1'),
            "{}",
            item.title
        );
        // The store's own words, not a paraphrase of them.
        let detail = item.detail.as_deref().unwrap_or_default();
        assert!(detail.contains("database or disk is full"), "{detail}");
        // Nothing to click: the fix is disk space or a file permission.
        assert!(item.actions.is_empty());

        // Stable: a disk that stays full is one open row, not one per failure.
        let again = w
            .inbox_with_health(
                &[],
                &Default::default(),
                &|_| None,
                &Health {
                    now: jiff::Timestamp::now(),
                    unwritten: (9_001, 12, Some("database or disk is full")),
                    ..Default::default()
                },
                Vec::new(),
                Vec::new(),
            )
            .items;
        assert_eq!(again[0].id, item.id, "one open row, whatever the count");
        assert!(again[0].title.contains("9001"), "{}", again[0].title);

        // Singular for one.
        let one = w
            .inbox_with_health(
                &[],
                &Default::default(),
                &|_| None,
                &Health {
                    now: jiff::Timestamp::now(),
                    unwritten: (1, 0, None),
                    ..Default::default()
                },
                Vec::new(),
                Vec::new(),
            )
            .items;
        assert!(one[0].title.starts_with("1 event "), "{}", one[0].title);
        assert!(!one[0].title.contains("1 events"), "{}", one[0].title);
    }

    /// Three states, not two: asked, unreadable, and silent — only one of the
    /// last two is alarming.
    #[test]
    fn a_silent_session_is_not_a_session_running_something_exotic() {
        let mut w = World::new();
        for id in ["quiet", "auto", "strange", "manual"] {
            seed(&mut w, id, None);
        }
        let say = |w: &mut World, id: &str, mode: &str| {
            w.apply(
                env(id, Event::PermissionModeSeen { mode: mode.into() }),
                RunHint::default(),
            );
        };
        say(&mut w, "auto", "auto");
        say(&mut w, "strange", "hypervigilant");
        say(&mut w, "manual", "default");
        // `quiet` says nothing at all.

        let all = w.permission_modes();
        let unsupervised: usize = all.iter().map(|p| p.unsupervised).sum();
        let unknown: usize = all.iter().map(|p| p.unknown).sum();
        let unreported: usize = all.iter().map(|p| p.unreported).sum();
        assert_eq!(unsupervised, 1, "only `auto` decides without a person");
        assert_eq!(unknown, 1, "only `strange` reported something unreadable");
        assert_eq!(unreported, 1, "only `quiet` has said nothing");
        assert_eq!(
            unknown + unreported,
            2,
            "the two must not be the same number"
        );

        // Least supervised first.
        let sessions = &all[0].sessions;
        assert_eq!(
            sessions[0].asks_a_person,
            Some(false),
            "the unsupervised session is not first: {sessions:?}"
        );
        assert!(
            sessions.last().unwrap().mode.is_none(),
            "the silent session is not last: {sessions:?}"
        );
        // "Seen", never "since" — and only for one that actually reported.
        assert!(sessions[0].seen.is_some());
        assert!(sessions.last().unwrap().seen.is_none());
    }

    /// A run that has ended is not an answer to a present-tense question.
    #[test]
    fn only_live_sessions_are_asked_what_mode_they_are_in() {
        let mut w = World::new();
        seed(&mut w, "gone", None);
        w.apply(
            env(
                "gone",
                Event::PermissionModeSeen {
                    mode: "auto".into(),
                },
            ),
            RunHint::default(),
        );
        assert_eq!(
            w.permission_modes()
                .iter()
                .map(|p| p.unsupervised)
                .sum::<usize>(),
            1
        );
        w.apply(
            env("gone", Event::SessionEnded { reason: None }),
            RunHint::default(),
        );
        assert_eq!(
            w.permission_modes()
                .iter()
                .map(|p| p.sessions.len())
                .sum::<usize>(),
            0,
            "a session that ended in auto is history, not a row"
        );
    }

    /// The numbers above the board add up to the board.
    #[test]
    fn the_summary_partitions_every_session() {
        let mut w = World::new();
        seed(&mut w, "a", None);
        seed(&mut w, "b", None);
        seed(&mut w, "c", None);
        for id in ["a", "b", "c"] {
            let r = w.runs.get_mut(&RunId::new(id)).unwrap();
            r.reporting = true;
            r.state = RunState::Idle;
        }
        // Quiet since yesterday: real, and not today's business.
        w.runs.get_mut(&RunId::new("a")).unwrap().last_activity_at =
            jiff::Timestamp::now() - jiff::SignedDuration::from_hours(30);
        // Failed just now. The third is plain idle and was heard from a moment ago.
        w.runs.get_mut(&RunId::new("b")).unwrap().state = RunState::Failed;

        let s = w.summary(jiff::Timestamp::now());
        assert_eq!(s.runs, 3);
        assert_eq!(
            s.working + s.needs_you + s.idle + s.failed + s.dormant,
            s.runs,
            "every session is in exactly one column"
        );
        assert_eq!(
            s.dormant, 1,
            "the one nothing has been heard from since yesterday"
        );
        assert_eq!(s.failed, 1);
        assert_eq!(s.idle, 1);
    }

    /// The board and the inbox read the same list.
    ///
    /// A `stalled` or `lost` row from three days ago is history; a session
    /// *asking* for something never ages out, however long it has been asking.
    #[test]
    fn what_ages_off_the_board_ages_out_of_the_inbox_unless_it_is_asking() {
        let mut w = World::new();
        seed(&mut w, "old-stall", None);
        seed(&mut w, "old-ask", None);
        {
            let r = w.runs.get_mut(&RunId::new("old-stall")).unwrap();
            r.reporting = true;
            r.state = RunState::Working;
            r.last_activity_at = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(40);
        }
        {
            let r = w.runs.get_mut(&RunId::new("old-ask")).unwrap();
            r.reporting = true;
            r.state = RunState::Waiting(WaitingFor::Permission);
            r.last_activity_at = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(40);
        }

        // Working never ages out either — it is doing something.
        let ids: Vec<_> = w
            .working_set(jiff::Timestamp::now())
            .iter()
            .map(|r| r.id.to_string())
            .collect();
        assert!(ids.contains(&"old-stall".to_string()));
        assert!(ids.contains(&"old-ask".to_string()));

        // But a *terminal* row from forty hours ago is off both surfaces.
        w.runs.get_mut(&RunId::new("old-stall")).unwrap().state = RunState::Lost;
        assert!(
            !w.working_set(jiff::Timestamp::now())
                .iter()
                .any(|r| r.id.as_str() == "old-stall")
        );
        let kinds: Vec<_> = w
            .inbox(jiff::Timestamp::now())
            .into_iter()
            .map(|i| i.kind)
            .collect();
        assert!(!kinds.contains(&crate::core::AttentionKind::Lost));
        assert!(
            kinds.contains(&crate::core::AttentionKind::Permission),
            "a session that is asking is never history"
        );
    }

    /// Retention measures against the instant it is handed; a live run never
    /// ages out.
    #[test]
    fn a_prune_ages_terminal_runs_as_of_the_instant_it_is_given() {
        let mut w = World::new();
        seed(&mut w, "done", None);
        seed(&mut w, "busy", None);
        w.runs.get_mut(&RunId::new("done")).unwrap().state = RunState::Completed;
        w.runs.get_mut(&RunId::new("busy")).unwrap().state = RunState::Working;

        let now = jiff::Timestamp::now();
        assert_eq!(w.prune(now, 86_400), 0, "nothing is a day old yet");
        let two_days_on = now + jiff::SignedDuration::from_hours(48);
        assert_eq!(w.prune(two_days_on, 86_400), 1);
        assert!(w.run(&RunId::new("done")).is_none());
        assert!(
            w.run(&RunId::new("busy")).is_some(),
            "live runs never age out"
        );
    }
}

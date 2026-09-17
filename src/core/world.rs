//! The authoritative in-memory state, and the only place runs are created.
//!
//! The UI never reconstructs state from events: it subscribes to this. That is
//! the difference between a dashboard that survives a high-volume session and
//! one that melts, and it is why the reducer is pure — this struct is the only
//! thing that mutates.

use crate::core::attention::{AttentionConfig, AttentionItem, items_for_run, rank};
use crate::core::event::{Event, EventEnvelope};
use crate::core::ids::{ProjectId, RunId, SessionId};
use crate::core::project::{Project, governing_root};
use crate::core::reduce;
use crate::core::run::{Run, RunMode, RunState};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What is wrong with the machine itself, rather than with anything running on
/// it.
///
/// Both facts have the same shape and the same reason to exist: the board looks
/// completely normal while a prohibition somebody wrote is not being enforced,
/// and no run-derived item can say so. Default is a healthy machine.
#[derive(Default)]
pub struct Health<'a> {
    /// Why the installed gate did not refuse a call its own rule denies, when
    /// the daemon last asked it. `None` when it answered.
    pub gate_down: Option<&'a str>,
    /// Repository roots whose `vibeplane.toml` will not parse, with the
    /// parser's reason.
    pub broken_configs: &'a [(PathBuf, String)],
}

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

    /// Turns what a person typed into a run.
    ///
    /// The board prints a short label — eight characters of the id, or the
    /// session's name with the project stripped off it — because a column of
    /// full uuids tells you nothing about which row is which. Every command
    /// then took the *whole* id, so the only identifier a user ever sees was
    /// the one identifier they could not use. Resolving a prefix is what git
    /// and docker do for exactly this reason.
    ///
    /// In order: the id exactly, then a unique id prefix, then a unique session
    /// name — whole, or with a `<project>-` prefix stripped, which is the form
    /// the board shows. Ambiguity is an error that names the candidates; a
    /// silent pick would act on the wrong session.
    pub fn resolve_run(&self, needle: &str) -> Result<RunId, Ambiguous> {
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
                let live: Vec<&Run> = found.iter().copied().filter(|r| r.is_active()).collect();
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

    /// For the few facts that come from outside this module — the git remote,
    /// which only a subprocess can answer and which a launch link needs.
    pub fn project_mut(&mut self, id: &ProjectId) -> Option<&mut Project> {
        self.projects.get_mut(id)
    }

    /// The board, most recently active first.
    ///
    /// Ordered by real activity rather than by when the daemon noticed a
    /// session: a tab left open on Tuesday should not sit above the run that
    /// is asking you something now.
    pub fn board(&self) -> Vec<&Run> {
        let mut v: Vec<&Run> = self.runs.values().collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.last_activity_at));
        v
    }

    /// The runs worth looking at: in play, or asking for something.
    ///
    /// A machine that has been running agents for a week accumulates editor
    /// tabs whose processes are still alive. Listing them beside the work in
    /// progress is technically complete and practically useless — the board's
    /// job is to answer "what needs me", and twenty dormant rows answer nothing.
    pub fn working_set(&self) -> Vec<&Run> {
        self.board().into_iter().filter(|r| r.is_active()).collect()
    }

    /// The inbox, derived fresh every time. Cheap because it is a map over
    /// runs, and correct because there is no cached copy to go stale.
    ///
    /// Uses the machine-wide stall threshold for every run. The daemon calls
    /// [`World::inbox_with`] with the project resolver instead.
    pub fn inbox(&self) -> Vec<AttentionItem> {
        self.inbox_with(&[], &std::collections::BTreeSet::new(), &|_| None)
    }

    /// The inbox, including anything the given Work items are asking for.
    ///
    /// Work produces entries no run can — a pull request going red hours after
    /// the agent stopped — so they are merged rather than kept in a second list
    /// the human has to remember to look at.
    ///
    /// `drivable` is the set of runs Vibeplane still holds a session for. It is
    /// passed in because this module cannot see them, and because after a
    /// restart the rows come back looking alive while the processes are gone —
    /// so an offer to prompt an agent must never be decided from run state.
    ///
    /// `stall_for` gives the project's own quiet threshold for a directory. A
    /// callback, because the answer comes from a file this module may not read,
    /// and because the sweeper that emits `Stalled` must resolve it the same
    /// way: the two disagreeing puts a stall in the inbox that the event log
    /// says never happened.
    pub fn inbox_with(
        &self,
        works: &[crate::core::Work],
        drivable: &std::collections::BTreeSet<RunId>,
        stall_for: &dyn Fn(&Path) -> Option<i64>,
    ) -> Vec<AttentionItem> {
        self.inbox_with_health(works, drivable, stall_for, &Health::default(), Vec::new())
    }

    /// The inbox, plus the items that are about the machine rather than about
    /// anything on it.
    ///
    /// [`Health`] is threaded in rather than derived, because neither of its
    /// facts can be read off a run: nothing happening is exactly what a working
    /// gate and a broken one both look like from the event log, and a
    /// configuration file that will not parse leaves no event at all.
    pub fn inbox_with_health(
        &self,
        works: &[crate::core::Work],
        drivable: &std::collections::BTreeSet<RunId>,
        stall_for: &dyn Fn(&Path) -> Option<i64>,
        health: &Health<'_>,
        forge: Vec<AttentionItem>,
    ) -> Vec<AttentionItem> {
        // The same runs the board shows, for the reason the notifier reads
        // this list rather than deriving its own: two surfaces disagreeing
        // about what needs a person is worse than either being wrong. A
        // session asking for something never ages out — `is_active` keeps
        // every blocked run — so what this drops is the noise: a stall, a
        // context gauge or a lost tab from three days ago.
        let from_runs = self.runs.values().filter(|r| r.is_active()).flat_map(|r| {
            let stall = stall_for(r.working_dir()).unwrap_or(self.attention.stall_seconds);
            items_for_run(r, &self.attention, stall)
        });
        let from_work = works.iter().flat_map(|w| {
            let can_drive = w.current_run().is_some_and(|r| drivable.contains(r));
            // Resumable is a different question from drivable, and only this
            // module can answer it: the agent named a session, and Vibeplane no
            // longer holds one. Both halves matter — offering to resume a run
            // that is already running would start a second agent on one
            // worktree.
            let can_resume = !can_drive
                && w.current_run()
                    .and_then(|r| self.runs.get(r))
                    .is_some_and(|r| r.mode == RunMode::Driven && r.agent_session.is_some());
            // The slug, so an item that names work somebody is about to do
            // carries a link that opens an agent where the work is.
            let repo = self.projects.get(&w.project_id).and_then(|p| p.repo_slug());
            crate::core::attention::items_for_work_in(w, can_drive, can_resume, repo.as_deref())
        });
        let gate = health.gate_down.map(crate::core::attention::gate_down_item);
        let configs = health
            .broken_configs
            .iter()
            .map(|(root, why)| crate::core::attention::config_broken_item(root, why));
        // `forge` is derived by the caller from state this module cannot see
        // — the polled GitHub facts live on the other side of the purity line
        // — and ranked here so there is one list and one order.
        rank(
            from_runs
                .chain(from_work)
                .chain(gate)
                .chain(configs)
                .chain(forge)
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
        let root = governing_root(path).unwrap_or_else(|| path.to_path_buf());

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
            if hint.agent_command.is_some() && run.agent_command.is_none() {
                run.agent_command = hint.agent_command.clone();
            }
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

    /// Takes trust back, for the one caller that needs it: a grant whose row
    /// never reached the store.
    ///
    /// In-memory trust that outlives a failed write is trust that disappears
    /// at the next restart, and a security gesture that quietly expires is
    /// worse than one that fails loudly.
    pub fn untrust(&mut self, id: &ProjectId) -> bool {
        match self.projects.get_mut(id) {
            Some(p) => {
                p.trusted = false;
                true
            }
            None => false,
        }
    }

    /// Hides the run's *current* inbox items until `until`, or shows
    /// everything again when `until` is `None`.
    ///
    /// The kinds are derived here rather than taken from the caller, because
    /// "snooze this" means the things being asked right now. A kind that turns
    /// up afterwards was never dismissed and is shown
    /// ([`Snoozed`](crate::core::attention::Snoozed)).
    pub fn snooze(&mut self, run: &RunId, until: Option<jiff::Timestamp>) -> bool {
        let Some(existing) = self.runs.get(run) else {
            return false;
        };
        let kinds: Vec<_> = items_for_run(existing, &self.attention, self.attention.stall_seconds)
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

    /// Records the plan a driven agent reported for its turn.
    pub fn set_plan(&mut self, run: &RunId, plan: Vec<crate::core::PlanStep>) {
        if let Some(r) = self.runs.get_mut(run) {
            r.plan = plan;
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
                    agent_command: None,
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
            let Some(run) = self.runs.get(&id) else {
                continue;
            };
            if alive(run) {
                continue;
            }
            // One fact — the process is not there — and the reducer decides
            // what it means from what the run was doing. Both readings used to
            // be `Lost`, which is Critical, so every closed editor tab became a
            // critical inbox item: twelve of the development machine's thirteen
            // entries were sessions nobody had touched for two days.
            {
                let env = EventEnvelope::new(
                    id.clone(),
                    crate::core::event::Source::Daemon,
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
    /// The numbers above the board, and they **partition what is on it**.
    ///
    /// They did not. The state counts ran over every run and `dormant` was an
    /// orthogonal cut across the same set, so a line reading
    /// `38 sessions · 1 working · 0 need you · 25 idle` plus `10 dormant`
    /// double-counted and still left twelve sessions unmentioned. Now the
    /// breakdown covers exactly the runs the board shows and `dormant` is
    /// everything else, so `working + needs_you + idle + failed + dormant`
    /// is `runs` — which is what a reader assumes the moment they see a list
    /// of numbers with a total in front of it.
    pub fn summary(&self) -> BoardSummary {
        let mut s = BoardSummary {
            projects: self.projects.len(),
            runs: self.runs.len(),
            ..Default::default()
        };
        for r in self.runs.values() {
            // Spend is spend, whoever is looking: summed over everything.
            s.cost_usd += r.totals.cost_usd;
            if !r.is_active() {
                s.dormant += 1;
                continue;
            }
            match r.state {
                RunState::Working | RunState::Starting => s.working += 1,
                RunState::Waiting(_) => s.needs_you += 1,
                RunState::Idle => s.idle += 1,
                RunState::Failed | RunState::Lost => s.failed += 1,
                // A completed or stopped run that is still in play — it ended
                // ten minutes ago — is on the board and belongs somewhere.
                _ => s.idle += 1,
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
    /// Filled by the daemon, which holds that state; zero here.
    #[serde(default)]
    pub open_issues: usize,
    /// Open pull requests, the same way.
    #[serde(default)]
    pub open_prs: usize,
    /// Of those, the ones waiting on the person (see `ForgeCounts`).
    #[serde(default)]
    pub forge_needs_you: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Source, WaitingFor};

    fn env(run: &str, e: Event) -> EventEnvelope {
        EventEnvelope::new(RunId::new(run), Source::Hook, e)
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
        // The board shows eight characters of the id, or the session name with
        // the project stripped off — and every command took the whole thing, so
        // the one identifier a user ever sees was the one nothing accepted.
        let mut w = World::new();
        seed(&mut w, "7c4f9a20-1111-2222-3333-444455556666", None);
        seed(
            &mut w,
            "0a1b2c3d-9999-8888-7777-666655554444",
            Some("repo-a1"),
        );

        // Exactly, as before.
        assert_eq!(
            w.resolve_run("7c4f9a20-1111-2222-3333-444455556666")
                .unwrap()
                .as_str(),
            "7c4f9a20-1111-2222-3333-444455556666"
        );
        // A prefix, the way git takes a short sha.
        assert_eq!(
            w.resolve_run("7c4f").unwrap().as_str(),
            "7c4f9a20-1111-2222-3333-444455556666"
        );
        assert_eq!(w.resolve_run("7c").unwrap().as_str().len(), 36);
        // The label the board actually prints for a named session.
        assert_eq!(
            w.resolve_run("a1").unwrap().as_str(),
            "0a1b2c3d-9999-8888-7777-666655554444"
        );
        assert_eq!(
            w.resolve_run("repo-a1").unwrap().as_str(),
            "0a1b2c3d-9999-8888-7777-666655554444"
        );
    }

    #[test]
    fn an_ambiguous_id_names_the_candidates_rather_than_guessing() {
        // Picking one would act on the wrong session, and the actions on offer
        // include stopping an agent.
        let mut w = World::new();
        seed(&mut w, "abc111", None);
        seed(&mut w, "abc222", None);
        // Both are live, so neither wins on activity.
        match w.resolve_run("abc") {
            Err(Ambiguous::Several(ids)) => {
                assert_eq!(ids.len(), 2);
                assert!(ids.iter().any(|i| i == "abc111"));
            }
            other => panic!("expected an ambiguity, got {other:?}"),
        }
        assert_eq!(w.resolve_run("zzz"), Err(Ambiguous::NotFound));
        assert_eq!(w.resolve_run(""), Err(Ambiguous::NotFound));
    }

    #[test]
    fn a_dormant_tab_does_not_make_todays_session_unaddressable() {
        // A machine that has been running agents all week has editor tabs whose
        // processes are still alive. One of them sharing a prefix must not cost
        // the working session its short id.
        let mut w = World::new();
        seed(&mut w, "abc111", None);
        seed(&mut w, "abc222", None);
        // The second never reported: a tab left open, not a session in use.
        if let Some(r) = w.runs.get_mut(&RunId::new("abc222")) {
            r.reporting = false;
            r.state = RunState::Idle;
        }
        assert!(!w.run(&RunId::new("abc222")).unwrap().is_active());
        assert_eq!(w.resolve_run("abc").unwrap().as_str(), "abc111");
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
                    usage: crate::core::event::ApiUsage {
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
                    options: vec![],
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

    fn roster_row(status: Option<&str>, started_ms: i64) -> Event {
        Event::RosterSeen {
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
        // A machine that has been running agents for a week accumulates editor
        // tabs whose processes are still alive. Twenty rows that all look
        // equally alive answer no question at all.
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
        let working: Vec<_> = w.working_set().iter().map(|r| r.id.clone()).collect();
        assert_eq!(working, vec![RunId::new("busy")]);
        assert_eq!(w.summary().dormant, 1);
    }

    #[test]
    fn the_age_shown_is_the_sessions_own_not_the_moment_we_noticed_it() {
        // Otherwise a three-day-old tab reads as new, every row shows the same
        // number, and the board sorts by nothing.
        let mut w = World::new();
        let two_days_ago =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(48)).as_millisecond();
        w.apply(
            env("old", roster_row(None, two_days_ago)),
            RunHint::default(),
        );

        let run = w.run(&RunId::new("old")).unwrap();
        assert!(
            run.idle_seconds() > 47 * 3600,
            "expected roughly two days, got {}s",
            run.idle_seconds()
        );
    }

    #[test]
    fn a_session_that_says_anything_joins_the_working_set() {
        let mut w = World::new();
        let long_ago =
            (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(72)).as_millisecond();
        w.apply(env("s1", roster_row(None, long_ago)), RunHint::default());
        assert!(w.working_set().is_empty());

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
        assert_eq!(w.working_set().len(), 1, "a hook makes it real");
        assert_eq!(w.summary().dormant, 0);
    }

    #[test]
    fn a_dormant_session_that_needs_a_human_is_never_hidden() {
        // Hiding is about noise, not about silencing a question.
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
                },
            ),
            RunHint::default(),
        );
        assert_eq!(w.working_set().len(), 1);
        assert_eq!(w.inbox().len(), 1);
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
        assert_eq!(w.inbox()[0].kind, crate::core::AttentionKind::Lost);
    }

    /// A closed editor tab is not a loss.
    ///
    /// Reconciliation marked **every** live run whose process was missing as
    /// `Lost`, which is Critical. On the development machine that made twelve
    /// of the inbox's thirteen items sessions nobody had touched for two days
    /// — the exact failure the ranking policy exists to prevent, produced by
    /// the ranking policy itself.
    #[test]
    fn a_tab_that_closed_has_ended_and_work_that_vanished_is_lost() {
        let mut w = World::new();
        seed(&mut w, "idle-one", None);
        seed(&mut w, "busy-one", None);
        w.runs.get_mut(&RunId::new("idle-one")).unwrap().state = RunState::Idle;
        w.apply(
            env(
                "busy-one",
                Event::ToolStarted {
                    tool: "Bash".into(),
                    input: serde_json::json!({"command": "cargo test"}),
                },
            ),
            RunHint::default(),
        );
        assert_eq!(
            w.run(&RunId::new("busy-one")).unwrap().state,
            RunState::Working
        );

        // Neither process exists any more.
        let events = w.reconcile(&|_| false);
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
        let kinds: Vec<_> = w.inbox().into_iter().map(|i| i.kind).collect();
        assert_eq!(kinds, [crate::core::AttentionKind::Lost]);
    }

    /// A repository whose rules will not parse says so, at the top.
    ///
    /// The failure this covers is silence: the file is broken, the daemon kept
    /// no previous rules to fall back on, every `never_auto` in that repository
    /// is inert, and every other surface in the product looks exactly as it
    /// does when the machine is healthy.
    #[test]
    fn a_configuration_that_will_not_parse_is_a_critical_item_on_a_quiet_machine() {
        let w = World::new();
        let broken = vec![(
            PathBuf::from("/repos/payments-api"),
            "TOML parse error at line 12".to_string(),
        )];
        let inbox = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health {
                gate_down: None,
                broken_configs: &broken,
            },
            Vec::new(),
        );
        assert_eq!(inbox.len(), 1, "nothing else is wrong and this still shows");
        let item = &inbox[0];
        assert_eq!(item.kind, crate::core::AttentionKind::ConfigBroken);
        assert_eq!(item.level, crate::core::attention::Level::Critical);
        assert!(item.title.contains("payments-api"), "{}", item.title);
        // The parser's own words, and the command that prints the line.
        let detail = item.detail.as_deref().unwrap_or_default();
        assert!(detail.contains("line 12"), "{detail}");
        assert!(
            detail.contains("vibeplane check /repos/payments-api"),
            "{detail}"
        );
        // Nothing to click: rewriting somebody's committed rules from an inbox
        // row is not something this product does.
        assert!(item.actions.is_empty());

        // Two broken repositories are two items, and the id is stable per
        // repository so the attention log does not grow one row per poll.
        let both = vec![
            broken[0].clone(),
            (
                PathBuf::from("/repos/web"),
                "unknown field `gate`".to_string(),
            ),
        ];
        let inbox = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health {
                gate_down: None,
                broken_configs: &both,
            },
            Vec::new(),
        );
        assert_eq!(inbox.len(), 2);
        let again = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health {
                gate_down: None,
                broken_configs: &both,
            },
            Vec::new(),
        );
        let ids: Vec<_> = inbox.iter().map(|i| i.id.clone()).collect();
        let ids_again: Vec<_> = again.iter().map(|i| i.id.clone()).collect();
        assert_eq!(ids, ids_again, "the id must not move between polls");
    }

    /// The numbers above the board add up to the board.
    ///
    /// They did not: the state counts ran over every run while `dormant` was an
    /// orthogonal cut of the same set, so `38 sessions · 1 working · 0 need you
    /// · 25 idle` left twelve failed sessions unmentioned and double-counted
    /// the ten it did mention.
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

        let s = w.summary();
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
        let ids: Vec<_> = w.working_set().iter().map(|r| r.id.to_string()).collect();
        assert!(ids.contains(&"old-stall".to_string()));
        assert!(ids.contains(&"old-ask".to_string()));

        // But a *terminal* row from forty hours ago is off both surfaces.
        w.runs.get_mut(&RunId::new("old-stall")).unwrap().state = RunState::Lost;
        assert!(!w.working_set().iter().any(|r| r.id.as_str() == "old-stall"));
        let kinds: Vec<_> = w.inbox().into_iter().map(|i| i.kind).collect();
        assert!(!kinds.contains(&crate::core::AttentionKind::Lost));
        assert!(
            kinds.contains(&crate::core::AttentionKind::Permission),
            "a session that is asking is never history"
        );
    }
}

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
/// All three facts have the same shape and the same reason to exist: the board
/// looks completely normal while a prohibition somebody wrote is not being
/// enforced, or while the record every other row is derived from has a hole in
/// it — and no run-derived item can say so. Default is a healthy machine.
#[derive(Default)]
pub struct Health<'a> {
    /// Why the installed gate did not refuse a call its own rule denies, when
    /// the daemon last asked it. `None` when it answered.
    pub gate_down: Option<&'a str>,
    /// Repository roots whose `devplane.toml` will not parse, with the
    /// parser's reason.
    pub broken_configs: &'a [(PathBuf, String)],
    /// Events and decisions the store would not take, and the last reason.
    ///
    /// `(0, 0, _)` is the ordinary case and raises nothing. Counted rather
    /// than flagged because the row has to distinguish one lost write from
    /// thousands.
    pub unwritten: (u64, u64, Option<&'a str>),
    /// Agents a previous daemon started that are still running with nothing
    /// attached to them, as `(pid, command, worktree)`.
    ///
    /// Threaded in for the same reason as the other two: it cannot be derived
    /// from the event log. The log says a run was driven; whether the process
    /// behind it outlived the daemon is a fact about the machine, read from the
    /// process table once at startup (`observe::procs`).
    pub leaked_agents: &'a [(u32, String, Option<String>)],
}

/// Everything `devplane modes` and `/api/modes` say: which projects decide
/// without you, and what can answer in your name.
///
/// **One type, because the reader deserialises the writer's shape.** This was a
/// `json!` on one side and thirty-one `.get("key")` lookups on the other, and
/// the two disagreed once already: a field renamed `file` → `where_set` left a
/// reader untouched, `unwrap_or("")` turned the break into a blank line, and
/// `devplane modes` went on telling people a clock was answering their
/// questions without saying where to change it. Every test passed. A rename is
/// a compile error now.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Modes {
    pub projects: Vec<ProjectModes>,
    pub sessions: usize,
    pub unsupervised: usize,
    pub unknown: usize,
    /// Sessions that have reported no mode at all.
    ///
    /// **The CLI read this key for the life of the feature and nothing served
    /// it.** `.get("unreported")` against a payload with no such field is
    /// `unwrap_or(0)` — so the sentence explaining that a busy session can
    /// genuinely not have spoken never printed, and the guard on the reassuring
    /// line, `unreported < total`, was `0 < total`: always true. On a machine
    /// where **nothing** had reported, `devplane modes` printed *"Every session
    /// that has reported asks you"* in green, which is the reassuring answer
    /// over no evidence — the one direction of error this product calls
    /// expensive: guessing that nobody is needed, when somebody is, is the
    /// mistake this product cannot afford.
    pub unreported: usize,
    /// What can answer a question in your name on this machine, if anything.
    pub question_clock: Option<crate::core::clock::ClockLine>,
    /// How many live sessions this surface can actually speak for. A session
    /// that started before `devplane connect` has no environment reading, and
    /// the coverage of a surface is a fact about it rather than something a
    /// reader should assume.
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
    /// Sessions whose mode this build does not recognise, counted separately
    /// because *unknown* is not *fine*.
    pub unknown: usize,
    /// Sessions that have not reported a mode at all — a different fact from
    /// having reported one nobody here recognises, and the two read the same
    /// for exactly one pass before somebody noticed sixteen idle sessions
    /// being described as running something exotic.
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
    /// The mode an **ACP agent** declared for itself, in its own spelling.
    ///
    /// A separate field from `mode` rather than a fallback for it, because the
    /// two answer different questions. `mode` is one of a vendor's documented
    /// modes, read from a named settings file, and `asks_a_person` is derivable
    /// from it. This is a string the agent chose; nothing maps it onto anybody's
    /// vocabulary, and **no `asks_a_person` can be derived from it at all**.
    pub agent_mode: Option<String>,
    pub agent_mode_seen: Option<String>,
    /// A clock this session's own **environment** put on its questions.
    ///
    /// `CLAUDE_AFK_TIMEOUT_MS` overrides the settings files and turns
    /// auto-continue on even where they say `never`, so this is the one fact
    /// that can make the machine-wide line wrong for a single session.
    pub question_clock: Option<crate::core::clock::ClockLine>,
    /// Whether the environment was read for this session at all.
    ///
    /// **`false` is not *nothing is set*.** A session that started before
    /// `devplane connect`, or on a channel that carries no environment, has no
    /// reading — and rendering that as *your questions wait for you* is the
    /// reassuring wrong answer this whole feature exists to stop.
    pub clock_read: bool,
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
    /// [`World::inbox_with_health`] with the project resolver instead.
    pub fn inbox(&self) -> Vec<AttentionItem> {
        self.inbox_with_health(
            &[],
            &std::collections::BTreeSet::new(),
            &|_| None,
            &Health::default(),
            Vec::new(),
        )
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
            // module can answer it: the agent named a session, and Devplane no
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
        let leaked = health.leaked_agents.iter().map(|(pid, command, worktree)| {
            crate::core::attention::agent_leaked_item(*pid, command, worktree.as_deref())
        });
        let (lost_events, lost_decisions, last_loss) = health.unwritten;
        let record = (lost_events > 0 || lost_decisions > 0).then(|| {
            crate::core::attention::record_incomplete_item(
                lost_events,
                lost_decisions,
                last_loss.unwrap_or("The store gave no reason."),
            )
        });
        // `forge` is derived by the caller from state this module cannot see
        // — the polled GitHub facts live on the other side of the purity line
        // — and ranked here so there is one list and one order.
        rank(
            from_runs
                .chain(from_work)
                .chain(gate)
                .chain(configs)
                .chain(record)
                .chain(leaked)
                .chain(forge)
                .collect(),
        )
    }

    /// **Which projects are deciding without you, and since when we knew.**
    ///
    /// The first slice of the seat, and the only one the measurement of
    /// 2026-09-19 left standing: forty-nine consecutive tool calls across four
    /// sessions put nothing in front of a person, so *what was decided without
    /// you* is everything and is a transcript. *Which of my six repositories
    /// is in `auto`* is one short list, and nothing on this machine could
    /// answer it.
    ///
    /// **Live sessions only.** The question is present tense. A run that ended
    /// yesterday in `auto` is history, and putting it here would pad the list
    /// with rows nobody can act on.
    ///
    /// **A session that has not reported a mode is a row, not a gap.** Eleven
    /// hook events carry the mode and `PreToolUse` is not one of them, so a
    /// session that has done nothing but run tools since the daemon started
    /// genuinely has not said. Omitting those would make a partial answer look
    /// complete, which is the failure this whole surface exists to prevent.
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
                        // Three-valued on purpose: `None` is *we do not know*,
                        // which is a different row from *nobody is asked*.
                        asks_a_person: r.permission_mode.as_ref().and_then(|m| m.asks_a_person()),
                        // "Seen", never "since". Nothing announces a mode
                        // change, so this is when Devplane first heard it at
                        // this value — at best the person's next prompt after
                        // they switched.
                        seen: r.permission_mode_seen.map(|t| t.to_string()),
                        agent_mode: r.agent_mode.clone(),
                        agent_mode_seen: r.agent_mode_seen.map(|t| t.to_string()),
                        question_clock: r.question_clock.as_ref().map(Into::into),
                        clock_read: r.question_clock_read.is_some(),
                    })
                    .collect();
                // Least supervised first: the row worth reading is the one
                // nobody is watching, and a list sorted by name buries it.
                // Least supervised first, then the ones that said something
                // unreadable, then the quiet ones. A session nobody is
                // watching is the row worth reading and a list sorted by name
                // buries it.
                modes.sort_by_key(|m| {
                    // **A session closing its questions immediately sorts above
                    // everything**, including an unsupervised one. An
                    // unsupervised session is a mode somebody chose; this is a
                    // session in which *a question waits for you* — the
                    // product's own promise — is untrue right now.
                    //
                    // **Then a clock the person did not choose**, which is the
                    // row this surface exists for and which used to sort with
                    // the quiet ones because only `immediate` was read.
                    //
                    // **`never` is not either of those**, and until 2026-09-21
                    // it could not be told apart from a timer at all: every
                    // string in the settings file became a duration, so the
                    // setting's own default read as a clock answering in
                    // somebody's name.
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
                    // `mode.is_some()` is the whole distinction: a session
                    // that said something this build cannot read, versus one
                    // that has not said anything.
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
                // **Interrupted is counted here deliberately, and not as
                // `needs_you`.** Work cut off by a daemon bounce does want a
                // person, but the thing that asks for one is the inbox item
                // (`AttentionKind::Interrupted`), and a run counted in both
                // places is one interruption charging the attention budget
                // twice. The header line summarises; the inbox asks.
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
    /// Filled by the daemon, which holds that state; zero here.
    #[serde(default)]
    pub open_issues: usize,
    /// Open pull requests, the same way.
    #[serde(default)]
    pub open_prs: usize,
    /// Of those, the ones waiting on the person (see `ForgeCounts`).
    #[serde(default)]
    pub forge_needs_you: usize,
    /// Questions and permissions still waiting on a person **whose session is
    /// no longer running**. Filled by the daemon, which holds that state.
    ///
    /// **A separate number rather than part of `needs_you`, and the separation
    /// is the point twice over.** `needs_you` is a column in a breakdown of
    /// *sessions* — every session is in exactly one of them, and a test says so
    /// — and an ask that outlived its run is not a session at all. But leaving
    /// it out of the line entirely is how a machine with an unanswered question
    /// from yesterday came to print *"0 need you"*, which is the exact failure
    /// the durable ask exists to prevent, one layer up in the summary.
    #[serde(default)]
    pub asks_waiting: usize,
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

    /// **A session closing its questions immediately outranks an unsupervised
    /// one.** An unsupervised session is a mode somebody chose; this is a
    /// session in which *a question waits for you* is untrue right now, and a
    /// list that buries it under an alphabetical neighbour has hidden the one
    /// row it exists to show.
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

    /// The rendered line carries the words, so three surfaces cannot word one
    /// fact differently.
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

    /// An envelope carrying the channel the event actually comes from. A
    /// roster sample is not a hook, and labelling it as one tells the reducer
    /// a channel is connected that is not.
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
            env("s1", Event::tool_started("Bash", serde_json::json!({}))),
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
                    ask: None,
                    request_id: None,
                    form: None,
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
            env("s1", Event::tool_started("Bash", serde_json::json!({}))),
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
                Event::tool_started("Bash", serde_json::json!({"command": "cargo test"})),
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
                broken_configs: &broken,
                ..Default::default()
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
            detail.contains("devplane check /repos/payments-api"),
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
                broken_configs: &both,
                ..Default::default()
            },
            Vec::new(),
        );
        assert_eq!(inbox.len(), 2);
        let again = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health {
                broken_configs: &both,
                ..Default::default()
            },
            Vec::new(),
        );
        let ids: Vec<_> = inbox.iter().map(|i| i.id.clone()).collect();
        let ids_again: Vec<_> = again.iter().map(|i| i.id.clone()).collect();
        assert_eq!(ids, ids_again, "the id must not move between polls");
    }

    /// **A hole in the record gets a row, and a whole record gets none.**
    ///
    /// The store write that fails is logged and dropped on purpose — losing
    /// history beats stalling the hook a session is blocked on — and that log
    /// line goes to a daemon's stderr, which is nobody's screen. So for as
    /// long as this row did not exist, every count, every audit answer and
    /// every "who decided this" on the board was derived from a log that could
    /// be missing anything at all, and looked exactly as it does when it is
    /// complete.
    #[test]
    fn a_record_with_a_hole_in_it_says_so_and_a_whole_one_stays_quiet() {
        let w = World::new();
        let quiet = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health::default(),
            Vec::new(),
        );
        assert!(
            quiet.is_empty(),
            "a healthy machine raises nothing: {quiet:?}"
        );

        let inbox = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health {
                unwritten: (3, 1, Some("database or disk is full")),
                ..Default::default()
            },
            Vec::new(),
        );
        assert_eq!(inbox.len(), 1);
        let item = &inbox[0];
        assert_eq!(item.kind, crate::core::AttentionKind::RecordIncomplete);
        // Critical: this is the one failure the product cannot absorb, because
        // every other screen is a reading of the thing that is now incomplete.
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

        // Stable, so a disk that stays full is one open row rather than one
        // per write that failed — which would be the loudest possible way to
        // make this unreadable.
        let again = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health {
                unwritten: (9_001, 12, Some("database or disk is full")),
                ..Default::default()
            },
            Vec::new(),
        );
        assert_eq!(again[0].id, item.id, "one open row, whatever the count");
        assert!(again[0].title.contains("9001"), "{}", again[0].title);

        // One of each, singular. `1 events` is how a person learns the row was
        // generated rather than written.
        let one = w.inbox_with_health(
            &[],
            &Default::default(),
            &|_| None,
            &Health {
                unwritten: (1, 0, None),
                ..Default::default()
            },
            Vec::new(),
        );
        assert!(one[0].title.starts_with("1 event "), "{}", one[0].title);
        assert!(!one[0].title.contains("1 events"), "{}", one[0].title);
    }

    /// **Three states, not two: asked, unreadable, and silent.**
    ///
    /// Shipped briefly with the last two collapsed, and on a real machine that
    /// rendered as *"16 report a mode this build does not know"* about sixteen
    /// idle editors that had simply not spoken yet — a security incident made
    /// of nothing. A session that said something unreadable and a session that
    /// said nothing are different facts and only one of them is alarming.
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

        // Least supervised first: the row worth reading must not be buried by
        // an alphabetical sort.
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

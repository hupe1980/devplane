//! The daemon: receivers, API and the authoritative state.
//!
//! Everything lives in one process and one binary. The receivers must be
//! running whether or not a window is open — a hook that arrives while the UI
//! is closed is exactly the observation that matters — so the daemon is the
//! product and every other surface is a client of it.

use crate::core::event::{Event, EventEnvelope, Source};
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::core::{Policy, PolicyCache, World};
use crate::store::Store;
use anyhow::{Context, Result};
use axum::Router;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};

/// Shared daemon state.
pub struct AppState {
    pub world: Mutex<World>,
    /// Live ACP sessions, by run. Only driven runs appear here; an observed
    /// session belongs to whoever started it.
    pub sessions: Mutex<std::collections::HashMap<RunId, crate::acp::Session>>,
    /// Work items, by id.
    pub works: Mutex<std::collections::HashMap<crate::core::WorkId, crate::core::Work>>,
    /// Which work a run belongs to, so a finished turn knows what to verify.
    pub work_of_run: Mutex<std::collections::HashMap<RunId, crate::core::WorkId>>,
    pub store: Store,
    /// Projects whose git remote has already been asked for, so it is asked
    /// **once per project per daemon**, whatever the answer.
    ///
    /// Without this, a repository with no remote — a scratch checkout, a
    /// worktree of something never pushed — spawns `git remote get-url` on
    /// every single event, for ever. That is a subprocess per hook on the hot
    /// path, and it starves exactly the process-global reactor the agent
    /// sessions run on.
    pub remote_asked: Mutex<std::collections::HashSet<crate::core::ProjectId>>,
    /// Policies, resolved per repository. A rule belongs to the project it
    /// protects, so there is no single answer to "what may an agent do".
    pub policy: PolicyCache,
    /// Why the installed gate last failed to answer, or `None` when it did.
    ///
    /// **Nothing else can report this.** A gate that has stopped deciding looks
    /// exactly like a quiet machine from the event log: no hook arrives either
    /// way. So the daemon runs the installed command on a slow timer and keeps
    /// the answer here, and the inbox raises it — because `vibeplane doctor`
    /// only ever helps the person who thinks to run it.
    pub gate_down: Mutex<Option<String>>,
    /// What GitHub says about every registered project, from the last poll.
    ///
    /// In memory and rebuilt on restart, like the roster: it is an observation
    /// of somebody else's system of record, and a copy that outlives the
    /// daemon is a copy that can disagree with it.
    pub forge: Mutex<ForgeState>,
    /// Agents this machine can drive: the built-in list with the user's own
    /// `agents.toml` layered over it. Read once at start, because a launch
    /// command is not something that changes while a daemon runs.
    pub agents: Vec<crate::acp::AgentSpec>,
    pub token: String,
    /// Broadcasts everything live: state changes, and what driven agents say.
    /// The two are separate frames so a subscriber never has to inspect a
    /// payload to find out which it was handed.
    pub tx: broadcast::Sender<crate::core::Frame>,
    /// Raised by `POST /api/shutdown`, so `vibeplane stop` can ask rather than
    /// signal. A request carries the bearer token and reaches the same graceful
    /// path as ctrl-c; a signal needs a pid, and a pid read from a file is a
    /// pid the operating system may have given to somebody else.
    pub stopping: tokio::sync::Notify,
    pub started_at: jiff::Timestamp,
}

pub type Shared = Arc<AppState>;

/// The polled forge, for every project at once.
#[derive(Debug, Default)]
pub struct ForgeState {
    /// Whose `gh` this is. Asked once; "assigned to you" is relative to it.
    pub viewer: Option<String>,
    /// Why the last poll could not run at all — `gh` missing, not logged in,
    /// no network. Per project errors live on the project's entry.
    pub error: Option<String>,
    /// Projects `gh` said will never have a forge — no remote, no GitHub
    /// host — with why, and when that was decided.
    ///
    /// **A ruling expires** ([`should_skip`]). "Never" is a substring match on
    /// another program's English error ([`is_permanent`]), so a wrong guess
    /// must not cost a project its forge until the daemon restarts — and
    /// `git remote add origin …` makes a project a GitHub project anyway. The
    /// reason is kept so `doctor` can show it rather than guess.
    ///
    /// [`should_skip`]: ForgeState::should_skip
    /// [`is_permanent`]: crate::github::is_permanent
    pub skip: std::collections::BTreeMap<crate::core::ProjectId, (String, jiff::Timestamp)>,
    pub projects: std::collections::BTreeMap<crate::core::ProjectId, crate::core::ProjectForge>,
    /// Per project, which forge kinds a person dismissed and until when.
    pub snoozed:
        std::collections::BTreeMap<crate::core::ProjectId, crate::core::attention::Snoozed>,
    pub last_poll_at: Option<jiff::Timestamp>,
}

impl ForgeState {
    /// The inbox items the forge produces, across every project.
    ///
    /// `works` is what Vibeplane opened itself: those pull requests already
    /// raise items through their Work, and are skipped here rather than
    /// reported twice.
    pub fn items(&self, works: &[crate::core::Work]) -> Vec<crate::core::AttentionItem> {
        let none = crate::core::attention::Snoozed::default();
        self.projects
            .values()
            .flat_map(|f| {
                let own: std::collections::BTreeSet<u64> = works
                    .iter()
                    .filter(|w| w.project_id == f.project_id)
                    .filter_map(|w| w.pull_request.as_ref().map(|p| p.number))
                    .collect();
                let snoozed = self.snoozed.get(&f.project_id).unwrap_or(&none);
                crate::core::forge::items_for_forge(f, snoozed, &own)
            })
            .collect()
    }

    /// How long a "this will never have a forge" ruling stands before it is
    /// asked again.
    pub const RECHECK_AFTER: jiff::SignedDuration = jiff::SignedDuration::from_hours(1);

    /// Whether this project's forge should be left alone on this pass.
    ///
    /// The poller calls it and so does its test: a test that re-implements a
    /// predicate can agree with itself while the code does something else.
    pub fn should_skip(&self, id: &crate::core::ProjectId, now: jiff::Timestamp) -> bool {
        self.skip
            .get(id)
            .is_some_and(|(_, at)| now.duration_since(*at) < Self::RECHECK_AFTER)
    }

    /// The per-project counts the board's headings carry.
    pub fn counts(
        &self,
    ) -> std::collections::BTreeMap<crate::core::ProjectId, crate::core::ForgeCounts> {
        self.projects
            .iter()
            .map(|(id, f)| (id.clone(), f.counts()))
            .collect()
    }
}

impl AppState {
    /// `home` is where the machine-wide `policy.toml` was read from: a rule
    /// spelled with a single leading slash anchors at the file it was written
    /// in, so the policy cannot resolve one without knowing that.
    pub async fn new(db: PathBuf, token: String, policy: Policy, home: PathBuf) -> Result<Shared> {
        let store = Store::open(&db)
            .await
            .with_context(|| format!("opening {}", db.display()))?;
        let (tx, _) = broadcast::channel(1024);

        // What an earlier build's gate probe left behind is not state to come
        // up with. Idempotent, and zero on every store that never held any.
        match store
            .forget_probe(crate::observe::hook::PROBE_SESSION)
            .await
        {
            Ok(n) if n > 0 => {
                tracing::info!(rows = n, "forgot a gate probe an older build recorded")
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "could not forget probe rows"),
        }

        let mut world = World::new();
        // Come up with what we knew, then correct it against the machine.
        // Loading is not believing: a run recorded as working is a claim about
        // a process, and the process is the authority.
        for p in store.load_projects().await? {
            world.upsert_project(p);
        }
        let runs = store.load_runs().await?;
        let restored = runs.len();
        world.restore_runs(runs);

        // Work outlives the daemon that started it, so it comes back with the
        // runs. The sessions do not: those processes are gone, and a run in a
        // worktree is resumed deliberately rather than silently.
        let mut works = std::collections::HashMap::new();
        let mut work_of_run = std::collections::HashMap::new();
        for w in store.load_works().await? {
            for r in &w.runs {
                work_of_run.insert(r.clone(), w.id.clone());
            }
            works.insert(w.id.clone(), w);
        }
        let agents = crate::acp::available(&home);
        tracing::info!(
            restored,
            work = works.len(),
            agents = agents.len(),
            "restored from the store"
        );

        Ok(Arc::new(AppState {
            remote_asked: Mutex::new(Default::default()),
            world: Mutex::new(world),
            sessions: Mutex::new(std::collections::HashMap::new()),
            works: Mutex::new(works),
            work_of_run: Mutex::new(work_of_run),
            store,
            agents,
            gate_down: Mutex::new(None),
            forge: Mutex::new(ForgeState::default()),
            policy: PolicyCache::new(policy, home, dirs::home_dir()),
            token,
            tx,
            stopping: tokio::sync::Notify::new(),
            started_at: jiff::Timestamp::now(),
        }))
    }

    /// Applies an event: reduce, persist, broadcast.
    ///
    /// Persistence follows the reduction rather than preceding it, so the board
    /// is never behind what the store says. A store write that fails is logged
    /// and dropped: losing history is survivable, stalling the receiver is not.
    pub async fn ingest(
        &self,
        run: RunId,
        source: Source,
        event: Event,
        cwd: Option<PathBuf>,
        mode: RunMode,
    ) {
        self.ingest_as(
            run,
            source,
            event,
            crate::core::RunHint {
                cwd,
                mode,
                ..Default::default()
            },
        )
        .await
    }

    /// The same, saying what the caller knows about the run itself.
    ///
    /// Takes the whole [`RunHint`](crate::core::RunHint) rather than a widening
    /// list of loose arguments: every one of them is a fact about the run, the
    /// hint is the type that already collects them, and the alternative is a
    /// call whose meaning depends on remembering the order of eight things.
    ///
    /// Every observed session is Claude Code, because hooks and the roster are
    /// Claude Code's; a driven one is whatever was dispatched. This used to
    /// hard-code `claude` for all of them, so a Codex or OpenCode run Vibeplane
    /// had started itself said "claude" on the board and in `vibeplane show` —
    /// wrong about the one thing the row exists to identify.
    pub async fn ingest_as(
        &self,
        run: RunId,
        source: Source,
        event: Event,
        hint: crate::core::RunHint,
    ) {
        let env = EventEnvelope::new(run, source, event);
        let (env, _changes) = {
            let mut w = self.world.lock().await;
            w.apply(env, hint)
        };

        if let Err(e) = self.store.append_event(&env).await {
            tracing::warn!(error = %e, "could not persist event");
        }

        // The project row goes in before the run that references it. The other
        // order loses the run to a foreign-key violation, which shows up only
        // after a restart — as a board that forgot everything.
        if let Some(pid) = &env.project_id {
            let project = {
                let w = self.world.lock().await;
                w.project(pid).cloned()
            };
            if let Some(mut p) = project {
                // Ask git once per project, whatever the answer. The remote is
                // what a launch link names, and the only other source for it is
                // telemetry that somebody who has not run `connect` never
                // sends — but a repository with no remote must not be asked
                // twice, let alone once per event.
                let first_time =
                    p.repo_url.is_none() && self.remote_asked.lock().await.insert(pid.clone());
                if first_time && let Some(url) = crate::git::remote_url(&p.root).await {
                    p.repo_url = Some(url.clone());
                    let mut w = self.world.lock().await;
                    if let Some(live) = w.project_mut(pid) {
                        live.repo_url = Some(url);
                    }
                }
                if let Err(e) = self.store.save_project(&p).await {
                    tracing::warn!(error = %e, "could not persist project");
                }
            }
        }

        let run_snapshot = {
            let w = self.world.lock().await;
            w.run(&env.run_id).cloned()
        };
        if let Some(run) = run_snapshot
            && let Err(e) = self.store.save_run(&run).await
        {
            tracing::warn!(error = %e, "could not persist run");
        }

        let _ = self.tx.send(crate::core::Frame::Event(env));
    }

    /// Records a fragment of what a driven agent said, and streams it.
    ///
    /// Persistence failing is logged and dropped, exactly as for an event: a
    /// transcript is history, and losing a line of it must never stall the pump
    /// that is reading the agent.
    pub async fn ingest_message(&self, message: crate::core::Message) {
        if let Err(e) = self.store.append_message(&message).await {
            tracing::warn!(error = %e, "could not persist a transcript fragment");
        }
        let _ = self.tx.send(crate::core::Frame::Message(message));
    }

    /// Wakes subscribers when something changed that no event describes — a
    /// work phase, a snooze, a gate verdict.
    ///
    /// A real event with its own name, not a `StatusSample` of all-`None`
    /// pretending to be one: `vibeplane watch` prints what comes off this
    /// stream, and a reader should not have to know that an empty status sample
    /// secretly means "look again".
    pub fn notify_changed(&self) {
        let _ = self.tx.send(crate::core::Frame::Event(EventEnvelope::new(
            RunId::new("-"),
            Source::Daemon,
            Event::Refresh,
        )));
    }

    /// Appends a decision to the log and wakes the subscribers.
    ///
    /// Deliberately infallible from the caller's point of view: nothing that
    /// Vibeplane does should fail because the audit trail could not be written.
    /// It is logged loudly instead, because an audit trail that is quietly not
    /// being written is worse than none at all.
    pub async fn record(&self, decision: crate::core::Decision) {
        tracing::info!(decision = %decision.line());
        if let Err(e) = self.store.append_decision(&decision).await {
            tracing::error!(error = %e, "the decision log was not written");
        }
    }

    /// The runs Vibeplane still holds a session for.
    ///
    /// What "can this be driven" actually means. A run row restored from the
    /// store after a restart looks alive and has no process behind it, so the
    /// board must never answer this from run state.
    /// The inbox, exactly as a person sees it.
    ///
    /// One derivation for the API, the notifier and the attention log, because
    /// three callers computing it separately is how the notifier once announced
    /// items the inbox did not show. Locks are taken in a fixed order — works,
    /// sessions, gate, forge, then the world — and none is held across another
    /// `await` that could take one of them back.
    pub async fn current_inbox(&self) -> Vec<crate::core::AttentionItem> {
        let works: Vec<_> = self.works.lock().await.values().cloned().collect();
        let drivable = self.drivable_runs().await;
        let gate_down = self.gate_down.lock().await.clone();
        let forge = self.forge.lock().await.items(&works);
        let w = self.world.lock().await;
        w.inbox_with_gate(
            &works,
            &drivable,
            &|dir| self.policy.stall_seconds(dir),
            gate_down.as_deref(),
            forge,
        )
    }

    pub async fn drivable_runs(&self) -> std::collections::BTreeSet<RunId> {
        self.sessions
            .lock()
            .await
            .iter()
            .filter(|(_, s)| s.is_live())
            .map(|(r, _)| r.clone())
            .collect()
    }

    /// Records the plan a driven agent reported, and wakes the board.
    pub async fn set_plan(&self, run: &RunId, plan: Vec<crate::core::PlanStep>) {
        {
            let mut w = self.world.lock().await;
            w.set_plan(run, plan);
        }
        self.notify_changed();
    }

    /// Records the context window an agent reported for itself.
    ///
    /// Better than any table of model names: the protocol says how big the
    /// window is, so the gauge has a real denominator instead of a guess keyed
    /// on a string that changes with every release.
    pub async fn set_context_window(&self, run: &RunId, window: u64) {
        let mut w = self.world.lock().await;
        w.set_context_window(run, window);
    }

    /// Stops every agent Vibeplane started, and waits for their processes.
    ///
    /// Without this the daemon exits, its tasks are never dropped, the ACP
    /// connections are never torn down, and every agent it started is
    /// re-parented to init and keeps running — a model with a subscription
    /// attached, spending, with nothing left on the machine that knows it is
    /// there. The failure is invisible until you go looking with `ps`, which is
    /// exactly why it has to be handled here rather than left to Drop.
    pub async fn shutdown(&self) {
        let sessions: Vec<_> = {
            let mut map = self.sessions.lock().await;
            map.drain().collect()
        };
        if sessions.is_empty() {
            return;
        }
        tracing::info!(agents = sessions.len(), "stopping agents");
        for (_, session) in &sessions {
            session.stop();
        }
        // Each connection tears its agent's process group down as it closes.
        // Bounded, because a wedged agent must not hold the daemon open.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while sessions.iter().any(|(_, s)| s.is_live()) && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
}

/// Binds and serves until the process is asked to stop.
pub async fn serve(state: Shared, port: u16) -> Result<()> {
    let app: Router = crate::api::router(state.clone());
    let listener = bind(port).await?;
    let bound = listener.local_addr()?;

    crate::config::write_daemon_info(&crate::config::DaemonInfo {
        pid: std::process::id(),
        port: bound.port(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at: jiff::Timestamp::now().to_string(),
    })?;

    tracing::info!(%bound, "vibeplane daemon listening");

    let notifications = std::env::var("VIBEPLANE_NOTIFY").as_deref() != Ok("0");
    let poller = tokio::spawn(crate::poller::run(state.clone()));
    let sweeper = tokio::spawn(crate::poller::stall_sweeper(state.clone(), notifications));
    let retention = tokio::spawn(crate::poller::retention(state.clone(), 30, 7));
    let prs = tokio::spawn(crate::poller::pull_requests(state.clone()));
    // Nothing else can notice a gate that has stopped deciding: a broken one
    // and a quiet machine look identical from the event log.
    let gate = tokio::spawn(crate::poller::gate_watch(state.clone()));
    // GitHub, for every registered project: the other half of what is waiting
    // on the person, and the half no session reports.
    let forge = tokio::spawn(crate::poller::forge_watch(state.clone()));

    let asked = state.clone();
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown_signal() => {}
                _ = asked.stopping.notified() => tracing::info!("asked to stop"),
            }
        })
        .await;

    poller.abort();
    sweeper.abort();
    retention.abort();
    gate.abort();
    prs.abort();
    forge.abort();
    state.shutdown().await;
    crate::config::clear_daemon_info().ok();
    result.context("serving")?;
    Ok(())
}

/// Binds the loopback listener.
///
/// Loopback only. There is no configuration to expose this on another
/// interface, because a control plane that can approve tool calls has no
/// business listening on a network.
///
/// When the default port is taken, the daemon takes another one rather than
/// refusing to start. By the time this is reached the caller has already
/// established that no daemon of *this* `VIBEPLANE_HOME` is running, so
/// whatever holds the port belongs to somebody else — which is exactly the
/// case `VIBEPLANE_HOME=/tmp/vp vibeplane ls` is for. Clients read the port out
/// of `daemon.json`, so they follow.
///
/// A port asked for explicitly is never silently swapped: somebody who names a
/// port has a reason, and hooks already written into `settings.json` point at
/// the one they named.
async fn bind(port: u16) -> Result<tokio::net::TcpListener> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => Ok(l),
        Err(e)
            if e.kind() == std::io::ErrorKind::AddrInUse && port == crate::config::DEFAULT_PORT =>
        {
            tracing::warn!(
                port,
                "the usual port is taken by something else; taking another one"
            );
            tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
                .await
                .context("binding a loopback port")
        }
        Err(e) => Err(e).with_context(|| {
            format!("binding {addr} — something else is using it, and you asked for that port")
        }),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}

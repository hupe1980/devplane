//! The host: the API, the periodic watchers and the authoritative in-memory
//! state, in one process. Hooks write to the store without it; the host tails
//! the store and every other surface is a client of it.

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

/// Writes the store refused, counted so the inbox can say what was lost.
///
/// A counter rather than a `Result` because the callers are a blocked hook or
/// an agent's output stream: a full disk should cost a gap in the record, not
/// a stopped machine. Atomics, because both call sites already hold locks.
#[derive(Debug, Default)]
pub struct Unwritten {
    events: std::sync::atomic::AtomicU64,
    decisions: std::sync::atomic::AtomicU64,
    /// The last reason the store gave; the count carries the rest.
    last: std::sync::Mutex<Option<String>>,
}

impl Unwritten {
    fn note(&self, why: &str) {
        if let Ok(mut l) = self.last.lock() {
            *l = Some(why.to_string());
        }
    }

    /// An event the store would not take.
    pub fn lost_event(&self, why: &str) {
        self.events
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.note(why);
    }

    /// A decision the store would not take.
    pub fn lost_decision(&self, why: &str) {
        self.decisions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.note(why);
    }

    /// Events lost, decisions lost, and the last reason.
    pub fn counts(&self) -> (u64, u64, Option<String>) {
        (
            self.events.load(std::sync::atomic::Ordering::Relaxed),
            self.decisions.load(std::sync::atomic::Ordering::Relaxed),
            self.last.lock().ok().and_then(|l| l.clone()),
        )
    }
}

/// Shared host state.
pub struct AppState {
    /// Set before any session is torn down on a graceful stop, so the pump can
    /// tell *the agent's session ended* from *the host stopped it*. The latter
    /// leaves a waiting question still answerable (the vendor's session is on
    /// disk and can be resumed); both look like the same closed connection.
    pub tearing_down: std::sync::atomic::AtomicBool,
    pub world: Mutex<World>,
    /// Live ACP sessions, by run. Only driven runs appear here; an observed
    /// session belongs to whoever started it.
    pub sessions: Mutex<std::collections::HashMap<RunId, crate::acp::Session>>,
    /// Change items, by id.
    pub changes: Mutex<std::collections::HashMap<crate::core::ChangeId, crate::core::Change>>,
    /// Which change a run belongs to, so a finished turn knows what to verify.
    pub change_of_run: Mutex<std::collections::HashMap<RunId, crate::core::ChangeId>>,
    pub store: Store,
    /// Projects whose git remote has been asked for, once per project per host
    /// whatever the answer, so a repository with no remote does not spawn
    /// `git remote get-url` on every event.
    pub remote_asked: Mutex<std::collections::HashSet<crate::core::ProjectId>>,
    /// Policies, resolved per repository.
    pub policy: PolicyCache,
    /// Why the installed gate last failed to answer, or `None` when it did.
    ///
    /// A gate that stopped deciding looks like a quiet machine in the event
    /// log, so the host probes it on a timer and the inbox raises this.
    pub gate_down: Mutex<Option<String>>,
    /// How the OpenCode subscription is going, when one was asked for.
    ///
    /// The feed does not replay, so health is held explicitly rather than
    /// inferred from an absence of events.
    pub opencode: Mutex<crate::observe::opencode::Health>,
    /// What the store would not take, and why it last said no.
    ///
    /// Failed writes are logged and dropped rather than stalling the receiver;
    /// this makes the loss visible. In memory, because it is a fact about this
    /// host's run and the store is the thing refusing writes.
    pub unwritten: Unwritten,
    /// Agents a previous host started that are still running unattached:
    /// `(pid, command, worktree)`. Read once at startup; a live host tears down
    /// each agent's process group itself, so the set cannot grow.
    pub leaked_agents: Mutex<Vec<(u32, String, Option<String>)>>,
    /// What GitHub says about every registered project, from the last poll.
    /// In memory only: it is a copy of somebody else's system of record.
    pub forge: Mutex<ForgeState>,
    /// Agents this machine can drive: built-ins with the user's `agents.toml`
    /// layered over them. Read once at start.
    pub agents: Vec<crate::acp::AgentSpec>,
    pub token: String,
    /// Broadcasts state changes and driven agents' output, as separate frames.
    pub tx: broadcast::Sender<crate::core::Frame>,
    /// Flipped by `POST /api/quit`, so `devplane quit` asks with the token
    /// rather than signalling a pid that may have been reused. A `watch`, not a
    /// `Notify`, so every open `/api/stream` ends when it flips, not only
    /// those already waiting.
    pub stopping: tokio::sync::watch::Sender<bool>,
    pub started_at: jiff::Timestamp,
    /// Set once by the app; absent in the CLI host. See [`Opener`].
    pub opener: std::sync::OnceLock<Opener>,
}

pub type Shared = Arc<AppState>;

/// Where the change surface asks for a worktree to be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenIn {
    Editor,
    Terminal,
}

/// How a path is opened on this machine. Installed by the app; the CLI host
/// has none, since a browser page cannot start an editor.
pub type Opener = Box<dyn Fn(OpenIn, &std::path::Path) -> Result<(), String> + Send + Sync>;

/// The polled forge, for every project at once.
#[derive(Debug, Default)]
pub struct ForgeState {
    /// Whose `gh` this is. Asked once; "assigned to you" is relative to it.
    pub viewer: Option<String>,
    /// Why the last poll could not run at all — `gh` missing, not logged in,
    /// no network. Per project errors live on the project's entry.
    pub error: Option<String>,
    /// Projects `gh` said will never have a forge, with why and when.
    ///
    /// A ruling expires ([`should_skip`]): it rests on matching `gh`'s English
    /// error text ([`is_permanent`]), and adding a remote changes the answer.
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
    /// Pull requests Devplane opened (`changes`) already raise items through
    /// their Change and are skipped here.
    pub fn items(&self, changes: &[crate::core::Change]) -> crate::core::attention::Derived {
        let none = crate::core::attention::Snoozed::default();
        let mut out = crate::core::attention::Derived::default();
        for f in self.projects.values() {
            let own: std::collections::BTreeSet<u64> = changes
                .iter()
                .filter(|w| w.project_id == f.project_id)
                .filter_map(|w| w.pull_request.as_ref().map(|p| p.number))
                .collect();
            let snoozed = self.snoozed.get(&f.project_id).unwrap_or(&none);
            let d = crate::core::forge::items_for_forge(f, snoozed, &own);
            out.items.extend(d.items);
            out.snoozed += d.snoozed;
        }
        out
    }

    /// How long a "this will never have a forge" ruling stands before it is
    /// asked again.
    pub const RECHECK_AFTER: jiff::SignedDuration = jiff::SignedDuration::from_hours(1);

    /// Whether this project's forge should be left alone on this pass.
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
    /// `home` is where the machine-wide `policy.toml` was read from; a rule
    /// with a leading slash anchors there.
    pub async fn new(db: PathBuf, token: String, policy: Policy, home: PathBuf) -> Result<Shared> {
        let store = Store::open(&db)
            .await
            .with_context(|| format!("opening {}", db.display()))?;
        let (tx, _) = broadcast::channel(1024);

        // Drop any gate-probe rows an older build recorded. Idempotent.
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
        // Restore what was recorded, then reconcile it against the machine: a
        // run recorded as working is a claim about a process.
        for p in store.load_projects().await? {
            world.upsert_project(p);
        }
        let runs = store.load_runs().await?;
        let restored = runs.len();
        world.restore_runs(runs);

        // Changes come back with their runs; the sessions do not, and a run in
        // a worktree is resumed explicitly.
        let mut changes = std::collections::HashMap::new();
        let mut change_of_run = std::collections::HashMap::new();
        for w in store.load_changes().await? {
            for r in &w.runs {
                change_of_run.insert(r.clone(), w.id.clone());
            }
            changes.insert(w.id.clone(), w);
        }
        let agents = crate::acp::available(&home);
        tracing::info!(
            restored,
            changes = changes.len(),
            agents = agents.len(),
            "restored from the store"
        );

        Ok(Arc::new(AppState {
            tearing_down: std::sync::atomic::AtomicBool::new(false),
            remote_asked: Mutex::new(Default::default()),
            world: Mutex::new(world),
            sessions: Mutex::new(std::collections::HashMap::new()),
            changes: Mutex::new(changes),
            change_of_run: Mutex::new(change_of_run),
            store,
            agents,
            gate_down: Mutex::new(None),
            opencode: Mutex::new(crate::observe::opencode::Health::Live { last_event: None }),
            unwritten: Unwritten::default(),
            leaked_agents: Mutex::new(Vec::new()),
            forge: Mutex::new(ForgeState::default()),
            policy: PolicyCache::new(policy, home, dirs::home_dir()),
            token,
            tx,
            stopping: tokio::sync::watch::Sender::new(false),
            started_at: jiff::Timestamp::now(),
            opener: std::sync::OnceLock::new(),
        }))
    }

    /// Applies an event: reduce, persist, broadcast.
    ///
    /// Persisted after the reduction, so the board is never behind the store.
    /// A failed write is logged and counted, never allowed to stall the receiver.
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

    /// The same, with everything the caller knows about the run in a
    /// [`RunHint`](crate::core::RunHint) — including the vendor, so a driven
    /// Codex or OpenCode run is not labelled Claude Code.
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
            self.unwritten.lost_event(&e.to_string());
        }

        // The project row before the run that references it.
        if let Some(pid) = &env.project_id {
            self.learn_project(pid).await;
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

        let _ = self.tx.send(crate::core::Frame::Event(Box::new(env)));
    }

    /// Asks git for a project's remote (once per project, whatever the answer)
    /// and saves the project row. Shared by ingestion and the store tail.
    pub async fn learn_project(&self, pid: &crate::core::ProjectId) {
        let project = {
            let w = self.world.lock().await;
            w.project(pid).cloned()
        };
        let Some(mut p) = project else {
            return;
        };
        let first_time = p.repo_url.is_none() && self.remote_asked.lock().await.insert(pid.clone());
        if first_time && let Some(url) = crate::git::remote_url(&p.root).await {
            p.repo_url = Some(url.clone());
            let mut w = self.world.lock().await;
            if let Some(live) = w.project_mut(pid) {
                live.repo_url = Some(url);
            }
        }
        if let Err(e) = self.store.note_project(&p).await {
            tracing::warn!(error = %e, "could not persist project");
        }
    }

    /// Records a fragment of what a driven agent said, and streams it. A failed
    /// write is logged and dropped, never stalling the pump.
    pub async fn ingest_message(&self, message: crate::core::Message) {
        if let Err(e) = self.store.append_message(&message).await {
            tracing::warn!(error = %e, "could not persist a transcript fragment");
        }
        let _ = self.tx.send(crate::core::Frame::Message(message));
    }

    /// Wakes subscribers when something changed that no event describes — a
    /// change phase, a snooze, a gate verdict — with an explicit `Refresh`.
    pub fn notify_changed(&self) {
        let _ = self
            .tx
            .send(crate::core::Frame::Event(Box::new(EventEnvelope::new(
                RunId::new("-"),
                Source::Host,
                Event::Refresh,
            ))));
    }

    /// Appends a decision to the log and wakes the subscribers.
    ///
    /// Infallible to the caller: nothing should fail because the audit trail
    /// could not be written. A failed write is counted in [`Unwritten`] and
    /// raises an inbox row.
    pub async fn record(&self, decision: crate::core::Decision) {
        tracing::info!(decision = %decision.line());
        if let Err(e) = self.store.append_decision(&decision).await {
            tracing::error!(error = %e, "the decision log was not written");
            self.unwritten.lost_decision(&e.to_string());
        }
    }

    /// The facts every read surface is composed from, taken in one pass.
    ///
    /// Locks are taken in a fixed order — changes, sessions, gate, forge, then
    /// the world — and never held across an `await` that could take one back.
    /// The world is cloned so building a surface does not stall ingestion.
    pub async fn snapshot(&self) -> crate::view::Snapshot {
        let changes: Vec<_> = self.changes.lock().await.values().cloned().collect();
        let live: std::collections::BTreeSet<RunId> = self.drivable_runs().await;
        let gate = Some(self.gate_down.lock().await.clone());
        let forge = {
            let f = self.forge.lock().await;
            Some(crate::view::Forge {
                counts: f.counts(),
                items: f.items(&changes),
            })
        };
        let leaked_agents = self.leaked_agents.lock().await.clone();
        let (lost_events, lost_decisions, last_loss) = self.unwritten.counts();
        let world = self.world.lock().await.clone();
        let open_asks = self.store.open_asks().await.unwrap_or_default();
        crate::view::Snapshot {
            world,
            changes,
            open_asks,
            live,
            forge,
            gate,
            broken_configs: self.policy.broken(),
            unwritten: (lost_events, lost_decisions, last_loss),
            leaked_agents,
            agents: self.agents.clone(),
            from_host: true,
            now: jiff::Timestamp::now(),
            started_at: Some(self.started_at),
        }
    }

    /// The inbox as a person sees it: one derivation for the API, the notifier
    /// and the attention log, so they never disagree.
    pub async fn current_inbox(&self) -> Vec<crate::core::AttentionItem> {
        let snap = self.snapshot().await;
        crate::view::current_inbox(&snap, &self.store, &self.policy)
            .await
            .items
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

    /// Records the context window an agent reported for itself — a real
    /// denominator for the gauge, not a guess keyed on model names.
    pub async fn set_context_window(&self, run: &RunId, window: u64) {
        let mut w = self.world.lock().await;
        w.set_context_window(run, window);
    }

    /// Stops every agent Devplane started and waits for their processes, so
    /// none is re-parented to init and left running unseen.
    pub async fn shutdown(&self) {
        // Set first, so no `Ended` that follows reads as the agent finishing.
        self.tearing_down
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // Read, not drained: each pump removes its run as the last thing it
        // does, so an empty map means every final state has been written.
        let sessions: Vec<_> = {
            let map = self.sessions.lock().await;
            map.iter().map(|(id, s)| (id.clone(), s.clone())).collect()
        };
        if sessions.is_empty() {
            return;
        }
        tracing::info!(agents = sessions.len(), "stopping agents");
        for (_, session) in &sessions {
            session.stop();
        }
        // Each connection tears its agent's process group down as it closes.
        // Bounded, so a wedged agent cannot hold the host open. Both
        // conditions matter: the agent gone, and every pump done persisting.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let agents_gone = !sessions.iter().any(|(_, s)| s.is_live());
            let pumps_drained = self.sessions.lock().await.is_empty();
            if (agents_gone && pumps_drained) || std::time::Instant::now() >= deadline {
                if !pumps_drained {
                    tracing::warn!(
                        "shut down before every run's final state was written; \
                         a run may come back looking live and be reconciled"
                    );
                }
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }
}

/// Binds and serves until the process is asked to stop.
pub async fn serve(state: Shared, port: u16) -> Result<()> {
    let app: Router = crate::api::router(state.clone());
    let listener = bind(port).await?;
    let bound = listener.local_addr()?;

    crate::config::write_host(&crate::config::HostRecord {
        pid: std::process::id(),
        port: bound.port(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        started_at: jiff::Timestamp::now().to_string(),
        // Resolved rather than taken from `argv[0]`, which the caller chooses.
        exe: std::env::current_exe()
            .ok()
            .map(|p| p.display().to_string()),
    })?;

    tracing::info!(%bound, "devplane host listening");

    let notifications = std::env::var("DEVPLANE_NOTIFY").as_deref() != Ok("0");
    // Retention after the caller has filed the spool, so pruning by age never
    // drops rows written while no host was running.
    crate::poller::retention(&state, 30, 7).await;
    let tail = tokio::spawn(crate::poller::tail(state.clone()));
    let poller = tokio::spawn(crate::poller::run(state.clone()));
    let sweeper = tokio::spawn(crate::poller::stall_sweeper(state.clone(), notifications));
    let expiry = tokio::spawn(crate::poller::expiry_sweeper(state.clone()));
    let prs = tokio::spawn(crate::poller::pull_requests(state.clone()));
    // Opt-in; returns immediately unless the person runs `opencode serve`.
    let opencode = tokio::spawn(crate::observe::opencode::watch(state.clone()));
    // A broken gate and a quiet machine look identical from the event log.
    let gate = tokio::spawn(crate::poller::gate_watch(state.clone()));
    // GitHub: what is waiting on the person that no session reports.
    let forge = tokio::spawn(crate::poller::forge_watch(state.clone()));
    // Each open change's tree, so a hand edit makes *verified* false.
    let trees = tokio::spawn(crate::poller::tree_watch(state.clone()));

    let asked = state.clone();
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let mut rx = asked.stopping.subscribe();
            tokio::select! {
                _ = shutdown_signal() => {
                    // Every open stream ends on this flag, however the stop
                    // arrived, or shutdown waits on open browser tabs.
                    let _ = asked.stopping.send(true);
                }
                _ = rx.wait_for(|stop| *stop) => tracing::info!("asked to stop"),
            }
        })
        .await;

    tail.abort();
    poller.abort();
    sweeper.abort();
    expiry.abort();
    gate.abort();
    prs.abort();
    opencode.abort();
    forge.abort();
    trees.abort();
    state.shutdown().await;
    crate::config::clear_host_if_ours(std::process::id(), bound.port()).ok();
    result.context("serving")?;
    Ok(())
}

/// Binds a loopback-only listener; there is no option to expose it.
///
/// If the default port is taken, another is chosen: the caller has already
/// established no host of this `DEVPLANE_HOME` is running, and clients read
/// the port from `host.json`. An explicitly requested port is never swapped.
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

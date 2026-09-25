//! The loop that makes a run mean something.
//!
//! Starting a change makes an isolated checkout, prepares it, puts an agent in
//! it and, when the agent says it is finished, runs the project's own gates.
//! Failures go back to the same session, bounded; then a person is asked.
//! Nothing here writes a state: state is computed from the record and the tree.

use crate::core::ProjectConfig;
use crate::core::change::{Change, Completion, PullRequestRef, Stopped, Waiting};
use crate::core::ids::{ChangeId, ProjectId, RunId};
use crate::host::Shared;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// Saves a change row, surfacing a failure. The row is the whole of the
/// crash-resumption promise, so never `.ok()` this.
pub(crate) async fn persist(state: &Shared, change: &Change) {
    if let Err(e) = state.store.save_change(change).await {
        tracing::error!(
            change = %change.id.as_str(),
            error = %e,
            "could not save this change: the board is ahead of the store, and a restart will lose it"
        );
        // Surfaced for `devplane doctor`; a host log line is read by nobody.
        state
            .store
            .record_channel("store", 0, Some(&e.to_string()))
            .await
            .ok();
    }
}

/// Everything needed to start a change.
pub struct StartRequest {
    pub project_root: PathBuf,
    pub title: String,
    pub prompt: String,
    pub agent: Option<String>,
    /// Work in an isolated checkout. Off means the agent edits the repository
    /// you are looking at.
    pub worktree: bool,
    /// The specification this change answers (file or folder), already
    /// validated as a path inside the project.
    pub spec: Option<String>,
    /// Which tasks of the specification to send: a token, or `file:line`.
    /// Resolved before anything is created, so an unknown selector refuses.
    pub tasks: Vec<String>,
    /// The report a person started this from. Its prompt is the report,
    /// quoted and never resolved as a template, so a `{placeholder}` in it is
    /// text and cannot refuse the start.
    pub from_report: Option<crate::core::ReportId>,
}

/// The tasks a set of selectors names in a specification folder.
pub fn select_for(
    root: &Path,
    config: &ProjectConfig,
    spec: &str,
    selectors: &[String],
) -> Result<Vec<crate::core::spec::SentTask>, String> {
    let folder = crate::core::spec::ChangeFolder::at(root, spec)?;
    // An unrecognised notation removes the edges, never the tasks: a task is
    // still selectable by `file:line`.
    let trace = config.spec.trace(&folder);
    crate::core::spec::select_tasks(&trace, spec, selectors)
}

/// What a start asks of every project, before any is touched.
pub struct Ask<'a> {
    pub prompt: &'a str,
    pub agent: Option<&'a str>,
    pub worktree: bool,
    pub spec: Option<&'a str>,
    pub tasks: &'a [String],
}

/// Whether each named project can take this change, with every refusal
/// before anything is written: never one failure after three successes. A
/// name that matches nothing stops the whole start. A project is named by
/// registered name, id or path; an unregistered path is refused as untrusted.
pub async fn preflight(
    state: &Shared,
    asked: &[String],
    ask: &Ask<'_>,
) -> Vec<crate::core::preflight::Finding> {
    use crate::core::preflight::{Facts, Finding, PreflightReason, notes_for, refusal_line};
    let registered: Vec<(String, String, PathBuf)> = {
        let w = state.world.lock().await;
        w.projects()
            .map(|p| (p.id.to_string(), p.name.clone(), p.root.clone()))
            .collect()
    };
    let mut out = Vec::new();
    for a in asked {
        let path = Path::new(a).canonicalize().ok().filter(|p| p.is_dir());
        let hit = registered
            .iter()
            .find(|(id, name, root)| id == a || name == a || path.as_deref() == Some(root));
        let (name, root) = match (hit, path) {
            (Some((_, name, root)), _) => (name.clone(), root.clone()),
            (None, Some(root)) => (
                root.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| a.clone()),
                root,
            ),
            (None, None) => {
                let why = PreflightReason::UnknownProject;
                // A missing path is a different mistake from an unknown name.
                let detail = (a.contains('/') || a.starts_with('.'))
                    .then_some("is not a directory, and no registered project has that name");
                out.push(Finding {
                    asked: a.clone(),
                    name: a.clone(),
                    root: None,
                    refusal: Some(why),
                    says: Some(refusal_line(a, why, a, detail)),
                    notes: Vec::new(),
                });
                continue;
            }
        };
        let shown = root.display().to_string();

        // Keep the first sentence that explains a refusal: the parse error
        // says more than *will not parse*.
        let mut detail: Option<String> = None;
        let trusted = crate::driven::require_trust(state, &root).await.is_ok();
        let config = match ProjectConfig::load(&root) {
            Ok(c) => match refuse_unusable(&root, &c) {
                Ok(()) => Some(c),
                Err(e) => {
                    detail.get_or_insert(format!("{e:#}"));
                    None
                }
            },
            Err(e) => {
                detail.get_or_insert(e.to_string());
                None
            }
        };
        let agent_id = ask
            .agent
            .map(str::to_string)
            .or_else(|| {
                config
                    .as_ref()
                    .and_then(|c| c.project.default_agent.clone())
            })
            .unwrap_or_else(|| "claude".into());
        let agent_available = crate::acp::resolve(&agent_id, &state.agents).is_some();
        // Unreadable is not clean: defaulting to clean would reassure on the
        // least known case. Tracked files only, as the worktree refuses on.
        let dirty = match crate::git::uncommitted(&root).await {
            Ok(dirty) => dirty,
            Err(e) => Some(format!("git could not read its status: {e:#}")),
        };
        let worktree_clean = dirty.is_none();
        let sent = match (ask.spec, ask.tasks.is_empty(), &config) {
            (_, true, _) | (_, _, None) => Ok(Vec::new()),
            (None, false, _) => Err(
                "tasks need a specification: they name lines in the folder the spec points at"
                    .to_string(),
            ),
            (Some(spec), false, Some(c)) => {
                select_for(&root, c, spec, ask.tasks).map_err(|e| e.to_string())
            }
        };
        // A worktree is made from the base, so a spec missing there is one the
        // agent will never see.
        let on_base = match (ask.worktree, ask.spec, &config) {
            (true, Some(spec), Some(c)) => {
                let base = match &c.project.base_branch {
                    Some(b) => b.clone(),
                    None => crate::git::base_branch(&root).await,
                };
                crate::git::refuse_spec_missing_from_base(&root, &base, spec)
                    .await
                    .map_err(|e| format!("{e:#}"))
            }
            _ => Ok(()),
        };
        let sent = match (sent, on_base) {
            (Ok(_), Err(e)) => Err(e),
            (sent, _) => sent,
        };
        let can_send = match (ask.spec.map(|s| Change::with_spec(&root, s)), sent) {
            (Some(Err(e)), _) | (_, Err(e)) => {
                detail.get_or_insert(e);
                false
            }
            (_, Ok(sent)) => {
                match crate::driven::resolve_prompt(state, &root, ask.prompt, &sent).await {
                    Ok(_) => true,
                    Err(why) => {
                        detail.get_or_insert(
                            why.iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("; "),
                        );
                        false
                    }
                }
            }
        };
        let facts = Facts {
            trusted,
            config_loads: config.is_some(),
            agent_available,
            worktree_clean,
            can_send,
        };
        let refusal = crate::core::preflight::refusal(&facts);
        let detail = match refusal {
            Some(PreflightReason::ConfigWillNotLoad) | Some(PreflightReason::CannotSend) => detail,
            Some(PreflightReason::NoSuchAgent) => Some(format!("cannot run agent `{agent_id}`")),
            Some(PreflightReason::DirtyWorktree) => dirty,
            _ => None,
        };
        let notes = match &config {
            Some(c) => notes_for(
                c.workspace.setup.as_deref(),
                &lockfiles_at(&root),
                &c.workspace.share,
                ask.worktree,
            ),
            None => Vec::new(),
        };
        out.push(Finding {
            asked: a.clone(),
            says: refusal.map(|r| refusal_line(&name, r, &shown, detail.as_deref())),
            name,
            root: Some(shown),
            refusal,
            notes,
        });
    }
    out
}

/// Which recognised lockfiles sit at a project's root (names only).
fn lockfiles_at(root: &Path) -> Vec<String> {
    crate::core::preflight::LOCKFILES
        .iter()
        .filter(|f| root.join(f).is_file())
        .map(|f| f.to_string())
        .collect()
}

/// Everything needed to adopt a branch somebody made by hand.
pub struct AdoptRequest {
    pub project_root: PathBuf,
    pub branch: String,
    /// The specification this change answers, already validated as a path
    /// inside the project.
    pub spec: Option<String>,
    /// Defaults to the branch's first commit subject.
    pub title: Option<String>,
}

/// Stamps the specification a change answers onto a gate's verdict.
///
/// The spec is read from the worktree, where the gate ran, so a branch that
/// edited it is recorded against the text it was checked against. The
/// unanswered-question markers come from `config`, loaded from the person's
/// checkout, so a branch cannot redefine them away. Every gate path calls
/// this, so the certificate never lacks the stamp on only one of them.
pub(crate) async fn stamp_spec(
    state: &crate::host::Shared,
    id: &ChangeId,
    report: &mut crate::core::change::GateReport,
    dir: &std::path::Path,
    config: &ProjectConfig,
) {
    let spec = state
        .changes
        .lock()
        .await
        .get(id)
        .and_then(|c| c.spec.clone());
    if let Some(path) = spec {
        report.spec = Some(crate::core::change::SpecStamp::of(
            &path,
            dir,
            &config.spec.open_questions,
        ));
    }
}

/// The checkout whose `devplane.toml` governs a change: the project's
/// registered root, never the worktree.
///
/// The worktree's `devplane.toml` is on the branch the agent writes, so
/// reading it there would let the code under test choose its own gates,
/// `[github]` and `[budget]`. Gates run in the worktree but are declared by
/// the person's checkout. The fallback maps a `.claude/worktrees/<name>` path
/// back to the checkout it sits under.
pub(crate) async fn governing_root(state: &Shared, id: &ChangeId) -> Option<PathBuf> {
    let (project_id, worktree) = {
        let changes = state.changes.lock().await;
        let c = changes.get(id)?;
        (c.project_id.clone(), c.worktree.clone())
    };
    let registered = {
        let world = state.world.lock().await;
        world.project(&project_id).map(|p| p.root.clone())
    };
    registered.or_else(|| {
        worktree
            .as_deref()
            .and_then(crate::core::project::main_checkout_for)
    })
}

/// The governing root and the configuration read from it.
pub(crate) async fn governing_config(
    state: &Shared,
    id: &ChangeId,
) -> Result<(PathBuf, ProjectConfig)> {
    let root = governing_root(state, id)
        .await
        .context("that change belongs to no registered project")?;
    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok((root, config))
}

/// Refuses a `devplane.toml` that cannot do what it says, before anything is
/// made or spent.
fn refuse_unusable(root: &std::path::Path, config: &ProjectConfig) -> Result<()> {
    let fatal: Vec<String> = config
        .validate()
        .into_iter()
        .filter(|p| p.fatal)
        .map(|p| p.to_string())
        .collect();
    if !fatal.is_empty() {
        bail!(
            "{}/devplane.toml cannot do what it says:\n  {}",
            root.display(),
            fatal.join("\n  ")
        );
    }
    Ok(())
}

/// The files a fresh checkout carries, as `.worktreeinclude` names them.
///
/// Candidates are only what git reports as ignored, so a tracked file is never
/// chosen. No file, no match, or a git failure copies nothing: a guess here
/// hands an agent files nobody chose.
async fn included_files(root: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(root.join(".worktreeinclude")) else {
        return Vec::new();
    };
    let patterns = crate::core::worktreeinclude::Patterns::parse(&text);
    if patterns.is_empty() {
        return Vec::new();
    }
    match crate::git::ignored_files(root).await {
        Ok(files) => files.into_iter().filter(|f| patterns.matches(f)).collect(),
        Err(e) => {
            tracing::warn!(error = %e, "could not list the ignored files, so none were copied");
            Vec::new()
        }
    }
}

/// Creates the change, its checkout, and the run that does it.
pub async fn start(state: &Shared, req: StartRequest) -> Result<ChangeId> {
    let root = req
        .project_root
        .canonicalize()
        .with_context(|| format!("{} does not exist", req.project_root.display()))?;

    // Checked before the worktree is made so a refusal leaves no branch
    // behind; `dispatch_for_change` checks again.
    crate::driven::require_trust(state, &root).await?;
    let project_id = ProjectId::from_path(&root);

    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Before a worktree or any spend: a `back_to` naming nothing, an undeclared
    // gate, a review loop with no check behind it.
    refuse_unusable(&root, &config)?;

    // Tasks resolve before the worktree, so a refusal has nothing to clean up.
    let sent = if req.tasks.is_empty() {
        Vec::new()
    } else {
        let spec = req.spec.as_deref().context(
            "tasks need a specification: `tasks` names lines in the folder `spec` points at, \
             and no `spec` was given",
        )?;
        select_for(&root, &config, spec, &req.tasks).map_err(|e| anyhow::anyhow!("{e}"))?
    };

    // Resolved before the change exists, so the preflight, the agent and the
    // record see the same prompt and an unfillable placeholder refuses here.
    // Tasks go in via `{tasks}` or are appended as a block.
    let prompt = match req.from_report {
        Some(_) => crate::core::context::with_stop_and_say(&req.prompt),
        None => crate::driven::resolve_prompt(state, &root, &req.prompt, &sent)
            .await
            .map_err(crate::driven::cannot_send)?,
    };
    let mut change = Change::new(project_id.clone(), req.title.clone(), prompt.clone());
    change.spec = req.spec.clone();
    // The plan at start, read from the repository root (the worktree does not
    // exist yet), so drift is a comparison rather than a suspicion.
    change.spec_at_start = req.spec.as_deref().and_then(|spec| {
        crate::core::spec::Spec::read(&root, spec, &config.spec.open_questions).fingerprint()
    });
    change.from_report = req.from_report.clone();

    // Named the way Claude Code names its own worktrees, so its cleanup sweep
    // understands both.
    let base = match &config.project.base_branch {
        Some(b) => b.clone(),
        None => crate::git::base_branch(&root).await,
    };
    if req.worktree
        && let Some(spec) = &req.spec
    {
        crate::git::refuse_spec_missing_from_base(&root, &base, spec).await?;
    }
    let dir = if req.worktree {
        let branch = change.branch_name();
        let dir = crate::git::create_worktree(&root, &change.slug(), &branch, &base)
            .await
            .context("creating the worktree")?;
        change.worktree = Some(dir.clone());
        change.branch = Some(branch);
        change.tree_now = crate::git::commit_stamp(&dir).await;

        // `.worktreeinclude` is the vendor's file, so the behaviour survives
        // uninstalling Devplane.
        let copied = crate::git::copy_included(&root, &dir, &included_files(&root).await);
        if !copied.is_empty() {
            tracing::info!(files = ?copied, "copied gitignored files into the worktree");
        }
        if let Some(setup) = &config.workspace.setup {
            // Install dependencies up front, visibly: the change is on the
            // board with the command before it starts, so a long install is
            // not mistaken for a hang.
            change.waiting = Some(Waiting::Setup {
                command: setup.clone(),
                since: jiff::Timestamp::now(),
            });
            change.updated_at = jiff::Timestamp::now();
            persist(state, &change).await;
            state
                .changes
                .lock()
                .await
                .insert(change.id.clone(), change.clone());
            state.notify_changed();
            let report = crate::gates::run(
                "setup",
                std::slice::from_ref(setup),
                &dir,
                config.gates.timeout,
                1,
                &crate::core::caches::env_for(&root, &config.workspace.share),
            )
            .await;
            if !report.passed() {
                tracing::warn!(summary = %report.summary(), "workspace setup failed");
            }
            change.setup = Some(report);
            change.waiting = None;
            change.updated_at = jiff::Timestamp::now();
            persist(state, &change).await;
        }
        dir
    } else {
        root.clone()
    };

    // On the board before the agent exists, so `driven` can read the
    // project's declared cache from this row.
    state
        .changes
        .lock()
        .await
        .insert(change.id.clone(), change.clone());

    let agent_id = req
        .agent
        .or_else(|| config.project.default_agent.clone())
        .unwrap_or_else(|| "claude".into());
    let spec = crate::acp::resolve(&agent_id, &state.agents)
        .with_context(|| format!("unknown agent `{agent_id}`"))?;

    let change_id = change.id.clone();

    // Started silent, registered, and only then prompted. Dispatching with
    // the prompt attached lets a fast agent end its turn before the run is
    // mapped to this change, so `on_turn_ended` finds nothing and the gates
    // never run.
    let run_id = crate::driven::dispatch_for_change(state, &spec, dir, &change_id, sent).await?;
    change.runs.push(run_id.clone());
    change.updated_at = jiff::Timestamp::now();

    persist(state, &change).await;
    state.changes.lock().await.insert(change_id.clone(), change);
    state
        .change_of_run
        .lock()
        .await
        .insert(run_id.clone(), change_id.clone());

    // The turn starts only once both maps answer for this run.
    crate::driven::prompt(state, &run_id, prompt).await?;

    Ok(change_id)
}

/// Takes a branch somebody made by hand and makes it a change.
///
/// The title defaults to the first commit's subject; the worktree is the
/// checkout that has the branch or a fresh one under `.claude/worktrees/`; no
/// run is attached. Gates, decisions and the certificate then accrue as usual.
pub async fn adopt(state: &Shared, req: AdoptRequest) -> Result<ChangeId> {
    let root = req
        .project_root
        .canonicalize()
        .with_context(|| format!("{} does not exist", req.project_root.display()))?;
    crate::driven::require_trust(state, &root).await?;
    let project_id = ProjectId::from_path(&root);
    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    refuse_unusable(&root, &config)?;

    let branch = req.branch.trim().to_string();
    if branch.is_empty() {
        bail!("say which branch to adopt");
    }
    if !crate::git::branch_exists(&root, &branch).await {
        bail!("there is no branch `{branch}` in {}", root.display());
    }
    let base = match &config.project.base_branch {
        Some(b) => b.clone(),
        None => crate::git::base_branch(&root).await,
    };
    if branch == base {
        bail!("`{branch}` is the base branch; a change is what diverges from it");
    }
    {
        let changes = state.changes.lock().await;
        if let Some(existing) = changes.values().find(|c| {
            c.project_id == project_id
                && c.branch.as_deref() == Some(branch.as_str())
                && c.archived_at.is_none()
        }) {
            bail!(
                "`{branch}` is already change {} — {}",
                existing.id.as_str(),
                existing.title
            );
        }
    }

    let title = match req.title.filter(|t| !t.trim().is_empty()) {
        Some(t) => t,
        None => crate::git::first_subject(&root, &base, &branch)
            .await
            .unwrap_or_else(|| branch.clone()),
    };
    let mut change = Change::new(project_id, title, String::new());
    change.branch = Some(branch.clone());

    // The checkout that has the branch, or one made where `start` makes them.
    let dir = match crate::git::worktree_of_branch(&root, &branch).await {
        Some(dir) => dir,
        None => {
            let name = branch.replace('/', "-");
            crate::git::adopt_worktree(&root, &name, &branch)
                .await
                .context("creating the worktree")?
        }
    };
    // A spec folder the branch wrote is not in the person's checkout yet, so
    // it is looked for and fingerprinted on the branch.
    if let Some(spec) = req.spec.as_deref() {
        let spec = Change::with_spec(&dir, spec).map_err(|e| anyhow::anyhow!("{e}"))?;
        change.spec_at_start =
            crate::core::spec::Spec::read(&dir, &spec, &config.spec.open_questions).fingerprint();
        change.spec = Some(spec);
    }
    change.tree_now = crate::git::commit_stamp(&dir).await;
    change.worktree = Some(dir);
    change.updated_at = jiff::Timestamp::now();

    let id = change.id.clone();
    persist(state, &change).await;
    state.changes.lock().await.insert(id.clone(), change);
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:adopt",
                branch.clone(),
                "done",
            )
            .because("adopted a branch made outside Devplane; its history is git's")
            .for_change(&id),
        )
        .await;
    state.notify_changed();
    Ok(id)
}

/// Called when a run bound to a change finishes a turn: either the project's
/// checks agree it is done, or the change goes back with the reasons.
pub async fn on_turn_ended(state: &Shared, run: &RunId) {
    let change_id = { state.change_of_run.lock().await.get(run).cloned() };
    let Some(change_id) = change_id else { return };

    // Stamp *ticked* and detect spec drift before the gates run.
    observe_spec(state, run).await;

    // Claimed under the lock so two turn ends for the same stop (hook and
    // protocol) cannot both start the gates.
    let (dir, attempt, rounds) = {
        let mut changes = state.changes.lock().await;
        let Some(c) = changes.get_mut(&change_id) else {
            return;
        };
        if !may_run_gates(c) {
            return;
        }
        c.waiting = Some(Waiting::Gates);
        c.updated_at = jiff::Timestamp::now();
        (
            c.worktree.clone(),
            c.gates.len() as u32 + 1,
            c.feedback_rounds,
        )
    };
    // In place, the gate runs in the person's checkout; otherwise the change
    // could never be verified.
    let dir = match dir {
        Some(d) => d,
        None => match governing_root(state, &change_id).await {
            Some(root) => root,
            None => {
                set_waiting(state, &change_id, None).await;
                return;
            }
        },
    };
    refresh_tree(state, &change_id).await;

    // The definition of done is the person's checkout's; see `governing_root`.
    let (root, config) = match governing_config(state, &change_id).await {
        Ok(pair) => pair,
        Err(e) => {
            // An unreadable definition of done is not a pass.
            tracing::warn!(error = %e, "cannot read the project config");
            stop(
                state,
                &change_id,
                Stopped::Broken {
                    detail: format!("this project's devplane.toml cannot be read: {e}"),
                },
            )
            .await;
            return;
        }
    };
    if !config.has_gates() {
        // No definition of done: nothing to verify, so a person decides. The
        // bill is still shown.
        tally_all(state, &change_id).await;
        set_waiting(state, &change_id, Some(Waiting::Person)).await;
        retire_runs(state, &change_id).await;
        return;
    }

    // The ceiling is checked before every check and round, not only at finish.
    if charge(state, &change_id, &config).await {
        return;
    }

    // A checkout removed by hand is a chain that cannot continue, reported as
    // never run rather than failed.
    if !crate::git::worktree_present(&dir) {
        let report = crate::gates::absent_worktree("check", &config.gates.check, &dir, attempt);
        record_gate(state, &change_id, &report).await;
        {
            let mut changes = state.changes.lock().await;
            if let Some(c) = changes.get_mut(&change_id) {
                c.gates.push(report);
                c.updated_at = jiff::Timestamp::now();
            }
        }
        stop(
            state,
            &change_id,
            Stopped::Broken {
                detail: format!("the worktree {} is gone", dir.display()),
            },
        )
        .await;
        return;
    }

    state.notify_changed();
    // Nothing commits: the gate runs on the working tree, uncommitted work
    // included, and records that tree's digest for `verified`.
    let mut report = crate::gates::run(
        "check",
        &config.gates.check,
        &dir,
        config.gates.timeout,
        attempt,
        &crate::core::caches::env_for(&root, &config.workspace.share),
    )
    .await;
    stamp_spec(state, &change_id, &mut report, &dir, &config).await;
    let passed = report.passed();
    let feedback = report.feedback();
    let summary = report.summary();

    record_gate(state, &change_id, &report).await;
    push_gate(state, &change_id, report).await;

    if passed {
        tracing::info!(change = %change_id, "gates passed");
        // Nothing opens a pull request here; that is `change offer`.
        set_waiting(state, &change_id, Some(Waiting::Person)).await;
        retire_runs(state, &change_id).await;
        return;
    }

    use crate::core::config::OnFail;
    match config.gates.on_fail {
        OnFail::Ignore => set_waiting(state, &change_id, Some(Waiting::Person)).await,
        OnFail::Escalate => {
            tracing::info!(change = %change_id, %summary, "gates failed; asking a human");
            stop(
                state,
                &change_id,
                Stopped::GateFailed {
                    gate: "check".into(),
                },
            )
            .await;
        }
        OnFail::Feedback if rounds < config.gates.max_feedback_rounds => {
            // Back to the same session, which still has the context.
            let round = {
                let mut changes = state.changes.lock().await;
                match changes.get_mut(&change_id) {
                    Some(c) => {
                        c.feedback_rounds += 1;
                        c.feedback_rounds
                    }
                    None => return,
                }
            };
            set_waiting(state, &change_id, Some(Waiting::Feedback { round })).await;
            if let Err(e) = crate::driven::prompt(state, run, feedback).await {
                tracing::warn!(error = %e, "could not hand the failures back");
                stop(
                    state,
                    &change_id,
                    Stopped::Broken {
                        detail: format!("the failures could not be handed back: {e}"),
                    },
                )
                .await;
            }
        }
        OnFail::Feedback => {
            // Budget spent: an agent and a gate can disagree forever.
            tracing::info!(change = %change_id, %summary, "feedback budget spent");
            stop(
                state,
                &change_id,
                Stopped::GateFailed {
                    gate: "check".into(),
                },
            )
            .await;
        }
    }
}

/// Reads the specification at a run's close and records what it said.
///
/// Fingerprint and date come from the person's checkout (where `spec_at_start`
/// was taken and mid-run edits land); ticked keys come from the agent's
/// worktree. Git is not consulted: uncommitted edits are the case.
pub(crate) async fn observe_spec(state: &Shared, run: &RunId) {
    let change_id = { state.change_of_run.lock().await.get(run).cloned() };
    let Some(change_id) = change_id else { return };
    let (spec, dir) = {
        let changes = state.changes.lock().await;
        match changes.get(&change_id) {
            Some(c) => (c.spec.clone(), c.worktree.clone()),
            None => return,
        }
    };
    let Some(spec) = spec else { return };
    let Ok((root, config)) = governing_config(state, &change_id).await else {
        return;
    };
    let fingerprint =
        crate::core::spec::Spec::read(&root, &spec, &config.spec.open_questions).fingerprint();
    let changed_at = crate::core::spec::Spec::changed_at(&root, &spec);
    let worked_in = dir.unwrap_or_else(|| root.clone());
    let ticked = crate::core::spec::ChangeFolder::at(&worked_in, &spec)
        .map(|folder| crate::core::spec::ticked_keys(&config.spec.trace(&folder)))
        .unwrap_or_default();
    state
        .ingest(
            run.clone(),
            crate::core::Source::Host,
            crate::core::Event::SpecObserved {
                fingerprint,
                ticked,
                changed_at,
            },
            None,
            crate::core::run::RunMode::Driven,
        )
        .await;
}

/// The drift this change has for one run, with what the run saw.
async fn drift_of(
    state: &Shared,
    id: &ChangeId,
    run: &RunId,
) -> Option<(crate::core::change::Drift, String, String)> {
    // One lock at a time, in the usual order.
    let c = { state.changes.lock().await.get(id).cloned()? };
    let runs: Vec<crate::core::run::Run> = {
        let world = state.world.lock().await;
        c.runs
            .iter()
            .filter_map(|r| world.run(r).cloned())
            .collect()
    };
    let borrowed: Vec<&crate::core::run::Run> = runs.iter().collect();
    let drift = c.drifts(&borrowed).into_iter().find(|d| d.run == *run)?;
    let saw = runs
        .iter()
        .find(|r| r.id == *run)?
        .observed
        .as_ref()?
        .fingerprint
        .clone()?;
    Some((drift, saw, c.spec.clone()?))
}

/// Whether this change has a drift on any run — what a snooze must cover.
pub(crate) async fn has_drift(state: &Shared, id: &ChangeId) -> bool {
    let Some(c) = ({ state.changes.lock().await.get(id).cloned() }) else {
        return false;
    };
    let runs: Vec<crate::core::run::Run> = {
        let world = state.world.lock().await;
        c.runs
            .iter()
            .filter_map(|r| world.run(r).cloned())
            .collect()
    };
    let borrowed: Vec<&crate::core::run::Run> = runs.iter().collect();
    !c.drifts(&borrowed).is_empty()
}

/// Accepts that the specification moved under `run`: the change's start
/// moves forward to what the run saw, and the record says the person did it.
/// `None` when this change has no such drift.
pub async fn accept_drift(state: &Shared, id: &ChangeId, run: &RunId) -> Result<Option<String>> {
    let Some((_, saw, _)) = drift_of(state, id, run).await else {
        return Ok(None);
    };
    let saved = {
        let mut changes = state.changes.lock().await;
        let c = changes.get_mut(id).context("no such change")?;
        c.accept_drift(saw.clone());
        c.updated_at = jiff::Timestamp::now();
        c.clone()
    };
    persist(state, &saved).await;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:drift.accepted",
                run.to_string(),
                &saw,
            )
            .because("the specification moved under this run, and the person accepted what it saw")
            .for_change(id),
        )
        .await;
    state.notify_changed();
    Ok(Some(saw))
}

/// Tells `run` the specification moved: a prompt naming the files goes into
/// the same conversation (resumed if needed), and the change's start moves to
/// what it was told. `false` when there is no such drift.
pub async fn tell_drift(state: &Shared, id: &ChangeId, run: &RunId) -> Result<bool> {
    let Some((_, saw, spec)) = drift_of(state, id, run).await else {
        return Ok(false);
    };
    let started = {
        let world = state.world.lock().await;
        world.run(run).map(|r| r.started_at)
    }
    .context("no such run")?;
    let root = governing_root(state, id)
        .await
        .context("that change belongs to no registered project")?;
    let files = crate::core::spec::Spec::changed_since(&root, &spec, started);
    let text = drift_prompt(&root, &spec, &files);
    if !crate::driven::is_live(state, run).await {
        crate::driven::resume(state, run).await?;
    }
    crate::driven::prompt(state, run, text).await?;
    let saved = {
        let mut changes = state.changes.lock().await;
        let c = changes.get_mut(id).context("no such change")?;
        c.accept_drift(saw.clone());
        c.updated_at = jiff::Timestamp::now();
        c.clone()
    };
    persist(state, &saved).await;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:drift.told",
                run.to_string(),
                &saw,
            )
            .because(format!(
                "the specification moved under this run, and the person told it: {}",
                files.join(", ")
            ))
            .for_change(id),
        )
        .await;
    state.notify_changed();
    Ok(true)
}

/// How much of the changed specification a drift prompt carries.
const DRIFT_TEXT_BYTES: usize = 32 * 1024;

/// What the agent is told when the specification moved under it.
///
/// The text itself, not a pointer: drift is measured in the person's checkout,
/// and the agent's worktree has the old file or none. Each changed document
/// goes in whole, bounded, and what did not fit is named.
pub(crate) fn drift_prompt(root: &Path, spec: &str, files: &[String]) -> String {
    let mut out = format!(
        "The specification `{spec}` changed in the person's checkout while you were working. \
         Your checkout does not have the change, so the new text is below; work to it.\n"
    );
    if files.is_empty() {
        out.push_str(
            "\nWhich documents changed could not be dated; ask the person to commit the \
             specification, or read it from their checkout.\n",
        );
        return out;
    }
    let mut used = 0usize;
    let mut left_out = Vec::new();
    for f in files {
        let Ok(path) = crate::core::spec::confine(root, f) else {
            left_out.push(f.clone());
            continue;
        };
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        if used + body.len() > DRIFT_TEXT_BYTES {
            left_out.push(f.clone());
            continue;
        }
        used += body.len();
        let fence = "`".repeat(
            body.as_bytes()
                .split(|b| *b != b'`')
                .map(<[u8]>::len)
                .max()
                .unwrap_or(0)
                .max(2)
                + 1,
        );
        out.push_str(&format!("\n`{f}` now reads:\n\n{fence}\n{body}\n{fence}\n"));
    }
    if !left_out.is_empty() {
        out.push_str(&format!(
            "\nAlso changed, not included here for size: {}.\n",
            left_out.join(", ")
        ));
    }
    out
}

/// Whether a turn ending should send this change to the gates: not while they
/// run, and not once settled, so a stray turn cannot restart a stopped loop.
pub(crate) fn may_run_gates(c: &Change) -> bool {
    !matches!(c.waiting, Some(Waiting::Gates)) && !c.is_settled()
}

/// Appends a gate report and takes its tree as the current tree.
pub(crate) async fn push_gate(state: &Shared, id: &ChangeId, report: crate::core::GateReport) {
    let mut changes = state.changes.lock().await;
    if let Some(c) = changes.get_mut(id) {
        if report.commit.is_some() {
            c.tree_now = report.commit.clone();
        }
        c.gates.push(report);
        c.updated_at = jiff::Timestamp::now();
    }
}

/// Reads the tree of a change's worktree and records it, so `verified`
/// follows the tree.
///
/// Persisted only when it changed, and `updated_at` is left alone: the slow
/// tick calls this for every open change and is not an event on it.
pub(crate) async fn refresh_tree(state: &Shared, id: &ChangeId) {
    let (open, worktree) = {
        let changes = state.changes.lock().await;
        match changes.get(id) {
            Some(c) => (c.archived_at.is_none(), c.worktree.clone()),
            None => (false, None),
        }
    };
    if !open {
        return;
    }
    // In place, the tree is the person's checkout.
    let dir = match worktree {
        Some(d) => Some(d),
        None => governing_root(state, id).await,
    };
    let Some(dir) = dir else { return };
    let stamp = crate::git::commit_stamp(&dir).await;
    let saved = {
        let mut changes = state.changes.lock().await;
        match changes.get_mut(id) {
            Some(c) if c.tree_now != stamp => {
                c.tree_now = stamp;
                Some(c.clone())
            }
            _ => None,
        }
    };
    if let Some(c) = saved {
        persist(state, &c).await;
        state.notify_changed();
    }
}

/// Every open change, for the tick, in place or in a worktree.
pub(crate) async fn with_worktrees(state: &Shared) -> Vec<ChangeId> {
    state
        .changes
        .lock()
        .await
        .values()
        .filter(|c| c.archived_at.is_none())
        .map(|c| c.id.clone())
        .collect()
}

/// Writes a gate verdict to the decision log, with the gate's own summary as
/// the reason.
pub(crate) async fn record_gate(state: &Shared, id: &ChangeId, report: &crate::core::GateReport) {
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Devplane,
                "gate:run",
                report
                    .commands
                    .iter()
                    .map(|c| c.command.as_str())
                    .collect::<Vec<_>>()
                    .join(" && "),
                if report.passed() { "pass" } else { "fail" },
            )
            .because(report.summary())
            .for_change(id),
        )
        .await;
}

/// What offering a change came to.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "offer", rename_all = "snake_case")]
pub enum Offer {
    /// The branch was pushed and the pull request opened, as the person's
    /// configuration allows.
    Opened { pull_request: PullRequestRef },
    /// The exact commands to paste, because the configuration does not allow
    /// Devplane to push.
    Commands { push: String, create: String },
}

/// Offers a change: pushes the branch and opens the pull request, or hands
/// back the two commands that would.
///
/// Always a person's action, and it pushes only when the person's checkout
/// sets `[github] pull_request = true`.
pub async fn offer(state: &Shared, id: &ChangeId) -> Result<Offer> {
    let change = {
        let changes = state.changes.lock().await;
        changes.get(id).cloned().context("no such change")?
    };
    if change.archived_at.is_some() {
        bail!("that change is archived; its worktree is gone");
    }
    if let Some(pr) = &change.pull_request {
        bail!("that change is already offered: {}", pr.url);
    }
    let (dir, branch) = match (&change.worktree, &change.branch) {
        (Some(d), Some(b)) => (d.clone(), b.clone()),
        _ => bail!("that change has no branch to offer"),
    };
    let (root, config) = governing_config(state, id).await?;
    let base = match &config.project.base_branch {
        Some(b) => b.clone(),
        None => crate::git::base_branch(&root).await,
    };
    // Refused, naming which: a dirty worktree would offer a branch without the
    // work the gates saw; a branch with nothing past its base offers nothing.
    let st = crate::git::status(&dir)
        .await
        .context("reading the worktree's status")?;
    if !st.is_clean() {
        bail!(
            "the worktree has {} uncommitted or untracked file(s); commit them on `{branch}` \
             before offering, or the pull request will not carry the work",
            st.changed_files + st.untracked_files
        );
    }
    let ahead = crate::git::commits_ahead(&dir, &base).await?;
    if ahead == 0 {
        bail!("`{branch}` has no commits past `{base}`; there is nothing to offer");
    }
    let body = pr_body(state, id).await;

    if !config.github.pull_request {
        // Offered either way, and a report this change answers is answered.
        crate::reports::fixed_by(state, id).await;
        return Ok(Offer::Commands {
            push: format!(
                "git -C {} push --set-upstream origin {}",
                shell_word(&dir.display().to_string()),
                shell_word(&branch)
            ),
            // `gh` reads the repository from the directory it runs in;
            // `--repo` takes `OWNER/REPO`, never a path.
            create: format!(
                "cd {} && gh pr create --base {} --head {} --title {} --body \"$(devplane change export {})\"",
                shell_word(&dir.display().to_string()),
                shell_word(&base),
                shell_word(&branch),
                shell_word(&change.title),
                id.as_str()
            ),
        });
    }
    if !crate::github::is_available(&dir).await {
        bail!("no GitHub remote here, or `gh` is not logged in — nothing to offer to");
    }

    // The first thing Devplane does that other people can see, hence
    // `[github].pull_request` is off by default.
    if let Err(e) = push_branch(&dir, &branch).await {
        state
            .record(
                crate::core::Decision::new(
                    crate::core::Authority::Person,
                    "git:push",
                    branch.clone(),
                    "fail",
                )
                .because(e.to_string())
                .for_change(id),
            )
            .await;
        return Err(e).context("pushing the branch");
    }
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "git:push",
                branch.clone(),
                "done",
            )
            .because("offered the change")
            .for_change(id),
        )
        .await;

    let pr = crate::github::create_pr(
        &dir,
        &branch,
        &base,
        &change.title,
        &body,
        config.github.draft,
    )
    .await
    .context("opening the pull request")?;
    tracing::info!(number = pr.number, "opened a pull request");
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "gh:pr.create",
                pr.url.clone(),
                "done",
            )
            .because(match config.github.draft {
                true => "offered as a draft",
                false => "offered",
            })
            .for_change(id),
        )
        .await;
    let pull_request = PullRequestRef {
        number: pr.number,
        url: pr.url.clone(),
        status: pr.status().as_str().to_string(),
        failing_checks: Vec::new(),
    };
    let saved = {
        let mut changes = state.changes.lock().await;
        match changes.get_mut(id) {
            Some(c) => {
                c.pull_request = Some(pull_request.clone());
                c.waiting = None;
                c.updated_at = jiff::Timestamp::now();
                Some(c.clone())
            }
            None => None,
        }
    };
    if let Some(c) = saved {
        persist(state, &c).await;
        state.notify_changed();
    }
    crate::reports::fixed_by(state, id).await;
    Ok(Offer::Opened { pull_request })
}

/// A word a shell reads back as itself.
fn shell_word(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.' | ':'))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Records what a change has spent — money, turns, elapsed time — on the row.
///
/// Money can be missing (ACP makes cost optional; GenAI telemetry has none);
/// turns and time are counted by Devplane, so they bind every agent. Called
/// with or without a ceiling, so projects with no gates still show the bill.
pub(crate) async fn tally_all(
    state: &Shared,
    id: &ChangeId,
) -> (f64, u32, std::time::Duration, String) {
    let (runs, title, started) = {
        let changes = state.changes.lock().await;
        match changes.get(id) {
            Some(c) => (c.runs.clone(), c.title.clone(), c.created_at),
            None => return (0.0, 0, std::time::Duration::ZERO, String::new()),
        }
    };
    let (spent, turns): (f64, u32) = {
        let world = state.world.lock().await;
        runs.iter()
            .filter_map(|r| world.run(r))
            .fold((0.0, 0), |(c, t), r| {
                (c + r.totals.cost_usd, t + r.totals.turns as u32)
            })
    };
    // `duration_since` directly: a failure here must not turn `max_runtime`
    // into a budget that never fires.
    let elapsed = jiff::Timestamp::now()
        .duration_since(started)
        .unsigned_abs();
    // Kept on the row: runs age off the board, the bill does not.
    if let Some(c) = state.changes.lock().await.get_mut(id) {
        c.cost_usd = spent;
    }
    (spent, turns, elapsed, title)
}

async fn push_branch(dir: &std::path::Path, branch: &str) -> Result<()> {
    let out = tokio::process::Command::new("git")
        .args(["push", "--set-upstream", "origin", branch])
        .current_dir(dir)
        .kill_on_drop(true)
        .output()
        .await?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

/// The pull-request body, including the gate report as the evidence.
async fn pr_body(state: &Shared, id: &ChangeId) -> String {
    let changes = state.changes.lock().await;
    let Some(c) = changes.get(id) else {
        return String::new();
    };
    let mut body = format!(
        "{}\n\n",
        crate::core::context::without_stop_and_say(&c.prompt)
    );
    if let Some(g) = c.check_report() {
        body.push_str("## Verification\n\n");
        // Name the commit these commands ran against, so a reviewer can check
        // what *verified* refers to.
        if let Some(sha) = g.commit.as_ref().and_then(|s| s.commit.as_deref()) {
            let dirty = if g.commit.as_ref().is_some_and(|s| !s.clean) {
                ", with uncommitted changes present"
            } else {
                ""
            };
            let tree = match g.commit.as_ref().and_then(|s| s.tree.as_deref()) {
                Some(t) => format!(" (working-tree digest `{t}`, ignored files excluded)"),
                None => " (no working-tree digest: the tree moved while it ran)".into(),
            };
            body.push_str(&format!("Against commit `{sha}`{dirty}{tree}.\n\n"));
        } else {
            body.push_str(
                "The commit these ran against could not be established, so they describe no \
                 particular tree.\n\n",
            );
        }
        for cmd in &g.commands {
            body.push_str(&format!(
                "- `{}` — {}\n",
                cmd.command,
                if cmd.passed() { "passed" } else { "failed" }
            ));
        }
        if c.feedback_rounds > 0 {
            body.push_str(&format!(
                "\nThe checks failed {} time(s) first; the failures were handed back and fixed.\n",
                c.feedback_rounds
            ));
        }
    } else {
        body.push_str("## Verification\n\nNo gate has run against this change.\n");
    }
    body.push_str("\n---\n_Offered through Devplane. The checks above are what ran; nothing here is a verdict._\n");
    body
}

/// What an archive may do beyond removing a clean worktree.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArchiveHow {
    /// Delete the branch too; refused while it has commits its base lacks,
    /// unless its pull request merged or `force`.
    pub delete_branch: bool,
    /// Remove a worktree with uncommitted or untracked files in it.
    pub discard_uncommitted: bool,
    /// With `delete_branch`: delete it even with unmerged commits.
    pub force: bool,
}

/// Adds up what this change has spent, and stops it past the project's
/// ceiling. Returns `true` when stopped.
///
/// Money only binds when the agent reports cost, which ACP makes optional;
/// [`crate::core::config::Budget`] says so.
pub(crate) async fn charge(state: &Shared, id: &ChangeId, config: &ProjectConfig) -> bool {
    let (spent, turns, elapsed, title) = tally_all(state, id).await;

    // Three bounds, most confidently observed first: turns and time are
    // Devplane's own count, money is whatever the agent reported.
    let ceiling = config.budget.ceiling_usd();
    let over: Option<String> = if config.budget.max_turns.is_some_and(|m| turns > m) {
        Some(format!(
            "{turns} turns, past the {} this project allows",
            config.budget.max_turns.unwrap_or(0)
        ))
    } else if config.budget.max_runtime.is_some_and(|m| elapsed > m) {
        Some(format!(
            "running for {}, past the limit this project sets",
            crate::render::duration(elapsed)
        ))
    } else {
        ceiling
            .filter(|c| spent > *c)
            .map(|c| format!("${spent:.2} spent, past a ceiling of ${c:.2}"))
    };
    {
        let mut changes = state.changes.lock().await;
        let Some(c) = changes.get_mut(id) else {
            return false;
        };
        // Cleared when a raised ceiling now covers the spend, so `change
        // retry` stops refusing once the person raised it.
        c.stopped = over
            .clone()
            .map(|bound| crate::core::change::Stopped::OverBudget {
                spent_usd: spent,
                bound,
            });
    }
    let Some(bound) = over else { return false };

    tracing::info!(change = %id, spent, turns, %bound, "a bound was reached");
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Devplane,
                "change:stop",
                title,
                "done",
            )
            .because(&bound)
            .for_change(id),
        )
        .await;
    // Written now so the stop reason is on disk across a restart.
    set_waiting(state, id, None).await;
    retire_runs(state, id).await;
    true
}

/// Stops every agent this change still has running, once nothing more will be
/// said; each driven run is a process group that otherwise lives on.
///
/// Not called for a stopped change: `retry` hands the failures back to the
/// session that wrote the code.
pub(crate) async fn retire_runs(state: &Shared, id: &ChangeId) {
    let runs = {
        let changes = state.changes.lock().await;
        changes.get(id).map(|c| c.runs.clone()).unwrap_or_default()
    };
    for run in &runs {
        let _ = crate::driven::retire(state, run).await;
    }
    // Awaited, bounded: the next step may spawn an agent, and overlapping a
    // dying one races and miscounts `max_parallel_runs`. The bound keeps a
    // wedged agent from holding up the change.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let still_going = {
            let sessions = state.sessions.lock().await;
            runs.iter()
                .any(|r| sessions.get(r).is_some_and(|s| s.is_live()))
        };
        if !still_going {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    tracing::warn!(change = %id, "an agent did not stop when asked");
}

/// Records which other in-flight changes in this repository edit the same
/// files, and returns the list.
///
/// Worktrees stop agents overwriting each other but not two branches that
/// cannot both land. A measurement, not a gate: nothing is refused, a person
/// is told before the merge. One `git diff` per worktree when a change comes
/// to rest, never on the poller's timer.
pub(crate) async fn note_overlaps(state: &Shared, id: &ChangeId) -> Vec<crate::core::Overlap> {
    let (project_id, mine) = {
        let changes = state.changes.lock().await;
        let Some(c) = changes.get(id) else {
            return Vec::new();
        };
        let Some(dir) = c.worktree.clone() else {
            return Vec::new();
        };
        (c.project_id.clone(), dir)
    };
    // A worktree does not carry the name of the branch it diverged from.
    let base = {
        let world = state.world.lock().await;
        world
            .project(&project_id)
            .map(|p| p.root.clone())
            .and_then(|root| crate::core::ProjectConfig::load(&root).ok())
            .and_then(|c| c.project.base_branch)
            .unwrap_or_else(|| "HEAD".into())
    };

    // Same repository, still open, not this one.
    let others: Vec<(ChangeId, String, std::path::PathBuf)> = {
        let changes = state.changes.lock().await;
        changes
            .values()
            .filter(|c| {
                &c.id != id && c.project_id == project_id && !c.is_settled() && c.worktree.is_some()
            })
            .map(|c| {
                (
                    c.id.clone(),
                    c.title.clone(),
                    c.worktree.clone().unwrap_or_default(),
                )
            })
            .collect()
    };
    if others.is_empty() {
        return Vec::new();
    }

    let ours: std::collections::BTreeSet<String> = crate::git::touched_files(&mine, &base)
        .await
        .into_iter()
        .collect();
    if ours.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (cid, title, dir) in others {
        let theirs = crate::git::touched_files(&dir, &base).await;
        let shared: Vec<String> = theirs.into_iter().filter(|f| ours.contains(f)).collect();
        if !shared.is_empty() {
            out.push(crate::core::Overlap {
                change_id: cid,
                title,
                files: shared.into_iter().take(20).collect(),
            });
        }
    }
    out
}

/// Records what a change is waiting on, and writes the row.
///
/// The one setter here, and it cannot express *finished* or *verified*: the
/// first is `finish`, which requires the basis, and the second is computed.
pub(crate) async fn set_waiting(state: &Shared, id: &ChangeId, waiting: Option<Waiting>) {
    // Overlaps are checked when the code stops changing, before anybody looks.
    let at_rest = matches!(waiting, Some(Waiting::Person));
    let overlaps = if at_rest {
        note_overlaps(state, id).await
    } else {
        Vec::new()
    };
    let saved = {
        let mut changes = state.changes.lock().await;
        match changes.get_mut(id) {
            Some(c) => {
                c.waiting = waiting;
                if at_rest {
                    c.overlaps = overlaps;
                }
                c.updated_at = jiff::Timestamp::now();
                Some(c.clone())
            }
            None => None,
        }
    };
    if let Some(c) = saved {
        persist(state, &c).await;
        state.notify_changed();
    }
}

/// Runs the gates on demand. A read: it never moves a change a person has
/// finished.
pub async fn verify(state: &Shared, id: &ChangeId) -> Result<crate::core::GateReport> {
    let (dir, attempt, held) = {
        let changes = state.changes.lock().await;
        let c = changes.get(id).context("no such change")?;
        if c.archived_at.is_some() {
            bail!("that change is archived; its worktree is gone");
        }
        (
            c.worktree.clone(),
            c.gates.len() as u32 + 1,
            c.completion.is_some(),
        )
    };
    // From the person's checkout; see `governing_root`.
    let (root, config) = governing_config(state, id).await?;
    let dir = dir.unwrap_or_else(|| root.clone());
    if !config.has_gates() {
        bail!("{} defines no gates", root.display());
    }
    // A checkout removed by hand yields a report saying nothing ran, recorded
    // like any other verdict.
    let gone = !crate::git::worktree_present(&dir);
    let mut report = if gone {
        crate::gates::absent_worktree("check", &config.gates.check, &dir, attempt)
    } else {
        crate::gates::run(
            "check",
            &config.gates.check,
            &dir,
            config.gates.timeout,
            attempt,
            &crate::core::caches::env_for(&root, &config.workspace.share),
        )
        .await
    };
    if !gone {
        stamp_spec(state, id, &mut report, &dir, &config).await;
    }
    let passed = report.passed();
    record_gate(state, id, &report).await;
    push_gate(state, id, report.clone()).await;
    if gone {
        // The stamp on the report is empty; the tree is now unknown.
        if let Some(c) = state.changes.lock().await.get_mut(id) {
            c.tree_now = None;
        }
    }

    // Only a change sitting on a verdict follows the new one; a finished one
    // keeps its place, but the report still reaches disk.
    if held {
        let saved = state.changes.lock().await.get(id).cloned();
        if let Some(c) = saved {
            persist(state, &c).await;
        }
        state.notify_changed();
        return Ok(report);
    }
    // A re-run replaces the stop reason: green clears it, red is a fresh gate
    // failure, a missing checkout is a chain that cannot continue.
    let waiting = {
        let mut changes = state.changes.lock().await;
        let c = changes.get_mut(id).context("no such change")?;
        c.stopped = if gone {
            Some(Stopped::Broken {
                detail: format!("the worktree {} is gone", dir.display()),
            })
        } else {
            (!passed).then(|| Stopped::GateFailed {
                gate: "check".into(),
            })
        };
        (!gone && passed).then_some(Waiting::Person)
    };
    set_waiting(state, id, waiting).await;
    Ok(report)
}

/// Picks a change back up after the host running it stopped.
///
/// Reconnects to the same agent-side conversation rather than paying to
/// rediscover it. Refuses when there is nothing to continue (no run, no agent
/// session, or an agent already working) instead of starting a fresh one.
pub async fn resume(state: &Shared, id: &ChangeId) -> Result<()> {
    let (mid_flight, run) = {
        let changes = state.changes.lock().await;
        let c = changes.get(id).context("no such change")?;
        (
            !c.is_settled() && !c.needs_a_person(),
            c.current_run().cloned(),
        )
    };
    if !mid_flight {
        bail!("that change is not mid-flight; there is nothing to pick up");
    }
    let run = run.context("that change never had an agent — start it again instead")?;
    crate::driven::resume(state, &run).await?;

    // The next event is the agent's turn ending, which sends it to the gates.
    set_waiting(state, id, None).await;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:resume",
                run.to_string(),
                "resumed",
            )
            .because("picked the change up after a restart")
            .for_change(id),
        )
        .await;
    Ok(())
}

/// Hands the failures back to the agent once more, past the project's bound.
///
/// The bound stops the machine arguing with a test suite, not a person
/// deciding one more go is worth it. Recorded, and counted, so later automatic
/// rounds still respect the budget. Needs the original session; refuses
/// rather than starting a fresh agent.
pub async fn retry(state: &Shared, id: &ChangeId) -> Result<()> {
    let mut change = {
        let changes = state.changes.lock().await;
        changes.get(id).cloned().context("no such change")?
    };
    if change.stopped.is_none() {
        bail!("that change is not waiting on a failed check");
    }

    // An over-budget stop is spent once the ceiling in the file covers the
    // spend, as `charge` does; normalised before `retryable` is asked so
    // there is one rule.
    if let Some(Stopped::OverBudget { spent_usd, bound }) = &change.stopped {
        let ceiling = match governing_root(state, id).await {
            Some(root) => ProjectConfig::load(&root)
                .ok()
                .and_then(|c| c.budget.ceiling_usd()),
            None => None,
        };
        // No readable ceiling reads as still over: clearing it would approve
        // the spend by default.
        if ceiling.is_none_or(|c| *spent_usd > c) {
            bail!(
                "that change stopped: {bound}. The bound `[budget]` sets. Raise it in \
                 devplane.toml if it is worth more."
            );
        }
        let spent = change.last_gate().map(|g| Stopped::GateFailed {
            gate: g.gate.clone(),
        });
        change.stopped = spent.clone();
        if let Some(c) = state.changes.lock().await.get_mut(id) {
            c.stopped = spent;
        }
    }

    let run = change
        .current_run()
        .cloned()
        .context("that change has no run")?;
    let live = crate::driven::is_live(state, &run).await;

    // `retryable` is the one answer the inbox and API also use; below only
    // explains a refusal.
    if !change.retryable(live) {
        match &change.stopped {
            Some(Stopped::Broken { detail }) => bail!(
                "that change stopped because it could not continue ({detail}), which is not \
                 something the agent can fix. `devplane check` reads the file the same way \
                 this did."
            ),
            None => {
                bail!("that change stopped without recording why, so there is nothing to hand back")
            }
            Some(Stopped::GateFailed { .. }) if change.last_gate().is_none() => {
                bail!("that change stopped before any check ran, so there is nothing to hand back")
            }
            _ if !live => bail!(
                "the agent that wrote this code is gone, and a fresh one would have to \
                 learn it all again. Pick it up yourself with `devplane attach {run}`, \
                 or start a new change."
            ),
            Some(other) => bail!("that change cannot be handed back: {}", other.headline()),
        }
    }

    // The feedback depends on why it stopped: reviewer findings, or a failing
    // gate's report.
    let stopped = change
        .stopped
        .clone()
        .context("that change stopped without recording why, so there is nothing to hand back")?;
    let feedback = match stopped.feedback() {
        Some(text) => text,
        // `retryable` has already required a gate report to exist.
        None => change
            .last_gate()
            .map(crate::core::change::GateReport::feedback)
            .context(
                "that change stopped before any check ran, so there is nothing to hand back",
            )?,
    };
    let title = change.title.clone();

    let rounds = {
        let mut changes = state.changes.lock().await;
        let c = changes.get_mut(id).context("no such change")?;
        c.feedback_rounds += 1;
        // Spent with the round it bought, or the board shows it still stopped.
        c.stopped = None;
        c.feedback_rounds
    };
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:retry",
                title,
                "done",
            )
            .because(format!(
                "{} — handed back again, round {rounds}, past the project's bound",
                stopped.headline()
            ))
            .for_change(id),
        )
        .await;

    set_waiting(state, id, Some(Waiting::Feedback { round: rounds })).await;
    crate::driven::prompt(state, &run, feedback).await
}

/// Stops a change, recording why in the same breath, so the inbox never has
/// to reconstruct the reason from whatever gate report is lying around.
pub(crate) async fn stop(state: &Shared, id: &ChangeId, why: Stopped) {
    {
        let mut changes = state.changes.lock().await;
        if let Some(c) = changes.get_mut(id) {
            c.stopped = Some(why.clone());
        }
    }
    set_waiting(state, id, None).await;
    tracing::info!(change = %id, reason = %why.headline(), "change stopped");
}

/// Finishes a change: a person accepts it, and the record says on what basis.
/// Nothing is removed; the worktree and branch stay until `archive`.
pub async fn finish(state: &Shared, id: &ChangeId) -> Result<()> {
    let change = {
        let changes = state.changes.lock().await;
        changes.get(id).cloned().context("no such change")?
    };
    if change.archived_at.is_some() {
        bail!("that change is archived");
    }
    if change.completion.is_some() {
        bail!("that change is already finished");
    }
    // The basis is decided here, where the decision is, not reconstructed
    // later. Whether gates are declared is the person's checkout's answer
    // (see `governing_root`), and a file that does not load is not *no gates
    // declared*.
    let root = governing_root(state, id).await;
    let declares_gates = match &root {
        Some(root) => ProjectConfig::load(root)
            .map_err(|e| {
                anyhow::anyhow!(
                    "{}/devplane.toml does not load ({e}), so whether this project declares \
                     gates cannot be told; fix it before finishing",
                    root.display()
                )
            })?
            .has_gates(),
        None => false,
    };
    // The tree as it stands, so a gate that went green before the tree moved
    // is not a pass. `None` reads as not current: unknown is not unchanged.
    let now_stamp = match change.worktree.as_ref().or(root.as_ref()) {
        Some(dir) => crate::git::commit_stamp(dir).await,
        None => None,
    };
    let basis = Completion::of(&change, declares_gates, now_stamp.as_ref());

    retire_runs(state, id).await;
    mark_done(state, id, basis, now_stamp).await;
    crate::reports::fixed_by(state, id).await;
    Ok(())
}

/// The only writer of `completion` in this program. It takes the basis rather
/// than deriving one, so no finished change is silent about what it rests on.
async fn mark_done(
    state: &Shared,
    id: &ChangeId,
    basis: Completion,
    tree_now: Option<crate::core::change::CommitStamp>,
) {
    let saved = {
        let mut changes = state.changes.lock().await;
        match changes.get_mut(id) {
            Some(c) => {
                c.completion = Some(basis.clone());
                c.tree_now = tree_now;
                c.waiting = None;
                c.updated_at = jiff::Timestamp::now();
                Some(c.clone())
            }
            None => None,
        }
    };
    if let Some(c) = saved {
        persist(state, &c).await;
        state
            .record(
                crate::core::Decision::new(
                    crate::core::Authority::Person,
                    "change:done",
                    c.title.clone(),
                    "done",
                )
                .because(basis.headline())
                .for_change(&c.id),
            )
            .await;
        state.notify_changed();
        tracing::info!(change = %id, basis = %basis.headline(), "change finished");
    }
}

/// What archiving removed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Archived {
    pub worktree_removed: bool,
    pub branch_deleted: bool,
}

/// Archives a change: the worktree goes, the record stays.
///
/// The worktree removal refuses uncommitted or unmerged work unless forced;
/// the branch is deleted only when asked and only when fully merged.
pub async fn archive(state: &Shared, id: &ChangeId, how: ArchiveHow) -> Result<Archived> {
    let change = {
        let changes = state.changes.lock().await;
        changes.get(id).cloned().context("no such change")?
    };
    if let Some(at) = change.archived_at {
        bail!("that change was archived at {at}");
    }
    if how.force && !how.delete_branch {
        bail!("--force deletes a branch with unmerged commits, and needs --delete-branch");
    }
    let root = governing_root(state, id).await;

    // The branch is kept unless asked for. Unmerged commits refuse only a
    // branch deletion, and a PR the forge says merged counts as merged (a
    // squash leaves commits "unmerged" by ancestry). Checked before anything
    // is removed.
    let mut branch_deleted = false;
    if how.delete_branch {
        let branch = change
            .branch
            .clone()
            .context("that change has no branch to delete")?;
        let owner = root
            .clone()
            .context("that change belongs to no registered project")?;
        let merged_pr = change
            .pull_request
            .as_ref()
            .is_some_and(|p| p.status == "merged");
        let base = match ProjectConfig::load(&owner)
            .ok()
            .and_then(|c| c.project.base_branch)
        {
            Some(b) => b,
            None => crate::git::base_branch(&owner).await,
        };
        if !merged_pr && !how.force {
            match crate::git::unmerged_commits(&owner, &branch, &base).await {
                Some(commits) if commits.is_empty() => {}
                Some(commits) => bail!(
                    "`{branch}` has {} commit(s) `{base}` does not: {}. Merge it first, archive \
                     without --delete-branch to keep it, or pass --force to drop them",
                    commits.len(),
                    commits
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None => bail!(
                    "could not tell whether `{branch}` is merged into `{base}`, so it was not \
                     deleted"
                ),
            }
        }
        branch_deleted = true;
    }

    let mut worktree_removed = false;
    if let Some(dir) = &change.worktree {
        // `git worktree remove` runs in the owning repository.
        let owner = root
            .clone()
            .context("that change belongs to no registered project")?;
        crate::git::remove_worktree(&owner, dir, how.discard_uncommitted).await?;
        worktree_removed = true;
    }
    // Only now: a refused removal leaves the runs as they were.
    retire_runs(state, id).await;
    if branch_deleted {
        let owner = root
            .clone()
            .context("that change belongs to no registered project")?;
        if let Some(branch) = &change.branch {
            crate::git::delete_branch(&owner, branch).await?;
        }
    }

    let at = jiff::Timestamp::now();
    let saved = {
        let mut changes = state.changes.lock().await;
        match changes.get_mut(id) {
            Some(c) => {
                c.archived_at = Some(at);
                c.waiting = None;
                c.tree_now = None;
                c.updated_at = at;
                Some(c.clone())
            }
            None => None,
        }
    };
    if let Some(c) = saved {
        persist(state, &c).await;
        state
            .record(
                crate::core::Decision::new(
                    crate::core::Authority::Person,
                    "change:archive",
                    c.title.clone(),
                    "done",
                )
                .because(match (worktree_removed, branch_deleted) {
                    (true, true) => "worktree removed and branch deleted; the record is kept",
                    (true, false) => "worktree removed; the branch and the record are kept",
                    (false, _) => "no worktree to remove; the record is kept",
                })
                .for_change(&c.id),
            )
            .await;
        state.notify_changed();
    }
    Ok(Archived {
        worktree_removed,
        branch_deleted,
    })
}

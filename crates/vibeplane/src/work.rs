//! The loop that makes a run mean something.
//!
//! Starting work is four steps that nobody should have to do by hand: make an
//! isolated checkout, prepare it, put an agent in it, and — when the agent says
//! it is finished — check whether the project agrees.
//!
//! That last step is the one the product exists for. "The agent says it is
//! done" is a claim; `cargo test` is evidence. When the evidence disagrees, the
//! failures go back to the same session, bounded, and after that a human is
//! asked rather than the loop running forever.

use crate::daemon::Shared;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use vibeplane_core::ProjectConfig;
use vibeplane_domain::ids::{ProjectId, RunId, WorkId};
use vibeplane_domain::work::{Phase, Work, WorkKind};

/// Everything needed to start a piece of work.
pub struct StartRequest {
    pub project_root: PathBuf,
    pub kind: WorkKind,
    pub title: String,
    pub prompt: String,
    pub agent: Option<String>,
    /// Work in an isolated checkout. Off means the agent edits the repository
    /// you are looking at, which is occasionally what you want and never the
    /// default.
    pub worktree: bool,
}

/// Creates the work, its checkout, and the run that does it.
pub async fn start(state: &Shared, req: StartRequest) -> Result<WorkId> {
    let root = req
        .project_root
        .canonicalize()
        .with_context(|| format!("{} does not exist", req.project_root.display()))?;

    // The trust gate. A headless agent runs the repository's own hooks and MCP
    // servers with no dialog of its own, so the decision to let that happen has
    // to be one somebody made deliberately, once, for this directory.
    let project_id = ProjectId::from_path(&root);
    let trusted = {
        let w = state.world.lock().await;
        w.project(&project_id).map(|p| p.trusted).unwrap_or(false)
    };
    if !trusted {
        bail!(
            "{} is not trusted yet. Starting an agent there runs that repository's own \
             hooks and MCP servers without asking. Run `vibeplane trust {}` if you meant to.",
            root.display(),
            root.display()
        );
    }

    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut work = Work::new(
        project_id.clone(),
        req.kind,
        req.title.clone(),
        req.prompt.clone(),
    );

    // The isolated checkout, named the way Claude Code names its own so the two
    // are indistinguishable on disk and its cleanup sweep understands both.
    let dir = if req.worktree {
        let base = match &config.project.base_branch {
            Some(b) => b.clone(),
            None => vibeplane_git::base_branch(&root).await,
        };
        let branch = work.branch_name();
        let dir = vibeplane_git::create_worktree(&root, &work.slug(), &branch, &base)
            .await
            .context("creating the worktree")?;
        work.worktree = Some(dir.clone());
        work.branch = Some(branch);

        let copied = vibeplane_git::copy_includes(&root, &dir, &config.workspace.include);
        if !copied.is_empty() {
            tracing::info!(files = ?copied, "copied gitignored files into the worktree");
        }
        if let Some(setup) = &config.workspace.setup {
            // A fresh checkout has no dependencies installed. Making the agent
            // discover that costs a turn and a lot of tokens.
            let report = vibeplane_gates::run(
                "setup",
                std::slice::from_ref(setup),
                &dir,
                config.gates.timeout,
                1,
            )
            .await;
            if !report.passed() {
                tracing::warn!(summary = %report.summary(), "workspace setup failed");
            }
            work.gates.push(report);
        }
        dir
    } else {
        root.clone()
    };

    let agent_id = req
        .agent
        .or_else(|| config.project.default_agent.clone())
        .unwrap_or_else(|| "claude".into());
    let spec = vibeplane_acp::resolve(&agent_id, &[])
        .with_context(|| format!("unknown agent `{agent_id}`"))?;

    work.phase = Phase::Implement;
    let work_id = work.id.clone();
    let run_id = crate::driven::dispatch(state, &spec, dir, Some(req.prompt)).await?;
    work.runs.push(run_id.clone());
    work.updated_at = jiff::Timestamp::now();

    state.store.save_work(&work).await.ok();
    state.works.lock().await.insert(work_id.clone(), work);
    state
        .work_of_run
        .lock()
        .await
        .insert(run_id, work_id.clone());

    Ok(work_id)
}

/// Called when a run bound to a piece of work finishes a turn.
///
/// This is the hinge: the agent has stopped, so either the project's checks
/// agree it is done, or the work goes back with the reasons.
pub async fn on_turn_ended(state: &Shared, run: &RunId) {
    let work_id = { state.work_of_run.lock().await.get(run).cloned() };
    let Some(work_id) = work_id else { return };

    let (dir, attempt, rounds) = {
        let works = state.works.lock().await;
        let Some(w) = works.get(&work_id) else { return };
        if w.phase != Phase::Implement {
            return;
        }
        (
            w.worktree.clone(),
            w.gates.len() as u32 + 1,
            w.feedback_rounds,
        )
    };
    let Some(dir) = dir else { return };

    // Inside a worktree, `rev-parse --show-toplevel` is the worktree itself, so
    // the configuration that governs this work is the one committed on its
    // branch. That is the right answer: a project's definition of done should
    // not change because somebody has an unsaved edit in another window.
    let root = vibeplane_git::repo_root(&dir).await.unwrap_or(dir.clone());
    let config = match ProjectConfig::load(&root) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "cannot read project config; skipping gates");
            return;
        }
    };
    if !config.has_gates() {
        // No Definition of Done, so there is nothing to verify and nothing to
        // claim. The work waits for a person rather than pretending.
        set_phase(state, &work_id, Phase::Review).await;
        return;
    }

    set_phase(state, &work_id, Phase::Verify).await;
    let report = vibeplane_gates::run(
        "check",
        &config.gates.check,
        &dir,
        config.gates.timeout,
        attempt,
    )
    .await;
    let passed = report.passed();
    let feedback = report.feedback();
    let summary = report.summary();

    {
        let mut works = state.works.lock().await;
        if let Some(w) = works.get_mut(&work_id) {
            w.gates.push(report);
            w.updated_at = jiff::Timestamp::now();
        }
    }

    if passed {
        tracing::info!(work = %work_id, "gates passed");
        set_phase(state, &work_id, Phase::Review).await;
        return;
    }

    use vibeplane_core::config::OnFail;
    match config.gates.on_fail {
        OnFail::Ignore => set_phase(state, &work_id, Phase::Review).await,
        OnFail::Escalate => {
            tracing::info!(work = %work_id, %summary, "gates failed; asking a human");
            set_phase(state, &work_id, Phase::Failed).await;
        }
        OnFail::Feedback if rounds < config.gates.max_feedback_rounds => {
            // Back to the same session, which still has the context that wrote
            // the code. A fresh session would have to learn it all again.
            {
                let mut works = state.works.lock().await;
                if let Some(w) = works.get_mut(&work_id) {
                    w.feedback_rounds += 1;
                }
            }
            set_phase(state, &work_id, Phase::Implement).await;
            if let Err(e) = crate::driven::prompt(state, run, feedback).await {
                tracing::warn!(error = %e, "could not hand the failures back");
                set_phase(state, &work_id, Phase::Failed).await;
            }
        }
        OnFail::Feedback => {
            // The budget is spent. An agent and a gate can disagree forever,
            // and every round costs money.
            tracing::info!(work = %work_id, %summary, "feedback budget spent");
            set_phase(state, &work_id, Phase::Failed).await;
        }
    }
}

async fn set_phase(state: &Shared, id: &WorkId, phase: Phase) {
    let saved = {
        let mut works = state.works.lock().await;
        match works.get_mut(id) {
            Some(w) => {
                w.phase = phase;
                w.updated_at = jiff::Timestamp::now();
                Some(w.clone())
            }
            None => None,
        }
    };
    if let Some(w) = saved {
        state.store.save_work(&w).await.ok();
        state.notify_changed();
    }
}

/// Runs the gates on demand, whatever phase the work is in.
pub async fn verify(state: &Shared, id: &WorkId) -> Result<vibeplane_domain::GateReport> {
    let (dir, attempt) = {
        let works = state.works.lock().await;
        let w = works.get(id).context("no such work")?;
        (
            w.worktree.clone().context("that work has no checkout")?,
            w.gates.len() as u32 + 1,
        )
    };
    let root = vibeplane_git::repo_root(&dir).await.unwrap_or(dir.clone());
    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    if !config.has_gates() {
        bail!("{} defines no gates", root.display());
    }
    let report = vibeplane_gates::run(
        "check",
        &config.gates.check,
        &dir,
        config.gates.timeout,
        attempt,
    )
    .await;
    let passed = report.passed();
    {
        let mut works = state.works.lock().await;
        if let Some(w) = works.get_mut(id) {
            w.gates.push(report.clone());
        }
    }
    set_phase(
        state,
        id,
        if passed { Phase::Review } else { Phase::Failed },
    )
    .await;
    Ok(report)
}

/// Finishes a piece of work, optionally removing its checkout.
pub async fn finish(state: &Shared, id: &WorkId, remove_worktree: bool, force: bool) -> Result<()> {
    let work = {
        let works = state.works.lock().await;
        works.get(id).cloned().context("no such work")?
    };
    if let Some(run) = work.current_run() {
        let _ = crate::driven::stop(state, run).await;
    }
    if remove_worktree && let Some(dir) = &work.worktree {
        let root = vibeplane_git::repo_root(dir).await.unwrap_or(dir.clone());
        vibeplane_git::remove_worktree(&root, dir, force).await?;
    }
    set_phase(state, id, Phase::Done).await;
    Ok(())
}

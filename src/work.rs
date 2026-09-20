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

use crate::core::ProjectConfig;
use crate::core::ids::{ProjectId, RunId, WorkId};
use crate::core::work::{Completion, Phase, Stopped, Work, WorkKind};
use crate::daemon::Shared;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// Saves a work row, and says so loudly when it cannot.
///
/// The pipeline cursor lives on this row — which step it reached, how many
/// times each has been entered, what the last reviewer found — so the write is
/// the whole of the crash-resumption promise: a daemon that dies between review
/// and verify resumes at verify *because the row was written*. A discarded
/// error leaves the board ahead of the store, silently, until a restart pays
/// for a step twice. Never `.ok()`.
pub(crate) async fn persist(state: &Shared, work: &Work) {
    if let Err(e) = state.store.save_work(work).await {
        tracing::error!(
            work = %work.id.as_str(),
            phase = ?work.phase,
            error = %e,
            "could not save this work: the board is ahead of the store, and a restart will lose it"
        );
        // Surfaced rather than only logged, because a log line in a daemon is
        // not somewhere anybody looks. `devplane doctor` reads this.
        state
            .store
            .record_channel("store", 0, Some(&e.to_string()))
            .await
            .ok();
    }
}

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
    /// The specification this work answers, already validated as a path inside
    /// the project. A file, or the folder every spec-driven framework produces.
    pub spec: Option<String>,
    /// The fan-out this work belongs to, when it belongs to one.
    ///
    /// **The only thing a batch adds to this path.** Every accepted target goes
    /// through exactly this function, so a call inside a fan-out meets the same
    /// trust check, the same worktree creation, the same gates and the same
    /// permission machinery a single dispatch meets. A batch that had its own
    /// start path would be a second place for a call to be decided, and the
    /// claim that a fan-out weakens nothing would be a hope rather than a
    /// consequence.
    pub batch_id: Option<crate::core::BatchId>,
}

/// Stamps the specification a work answers onto a gate's verdict.
///
/// Read from the **worktree**, not the main checkout: the gate ran there, and
/// if the branch edited the specification then that is the text the code was
/// checked against. Recording the other copy would name a file the verdict
/// never saw.
///
/// One function for every gate in the product — the work loop, `work verify`
/// and a pipeline step — because a certificate that carries the stamp on two
/// of the three paths is worse than one that never does: it reads as evidence
/// of absence.
pub(crate) async fn stamp_spec(
    state: &crate::daemon::Shared,
    id: &WorkId,
    report: &mut crate::core::work::GateReport,
    dir: &std::path::Path,
) {
    let spec = state
        .works
        .lock()
        .await
        .get(id)
        .and_then(|w| w.spec.clone());
    if let Some(path) = spec {
        // The words that mark an unanswered question are the repository's, and
        // the repository here is the worktree the gate ran in — same file, same
        // reasoning as the stamp itself.
        let markers = crate::core::ProjectConfig::load(dir)
            .map(|c| c.spec.open_questions)
            .unwrap_or_default();
        report.spec = Some(crate::core::work::SpecStamp::of(&path, dir, &markers));
    }
}

/// Creates the work, its checkout, and the run that does it.
pub async fn start(state: &Shared, req: StartRequest) -> Result<WorkId> {
    let root = req
        .project_root
        .canonicalize()
        .with_context(|| format!("{} does not exist", req.project_root.display()))?;

    // Checked before the worktree is made, not only when the agent starts:
    // refusing after creating a branch would leave litter behind. `dispatch`
    // checks again, because that is where every path converges.
    crate::driven::require_trust(state, &root).await?;
    let project_id = ProjectId::from_path(&root);

    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Before a worktree, before an agent, before any money is spent: a
    // `back_to` that names nothing, a gate nobody declared, a review loop with
    // no check behind it — each of which would otherwise surface as a chain
    // that failed several minutes and one model call later.
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

    let mut work = Work::new(
        project_id.clone(),
        req.kind,
        req.title.clone(),
        req.prompt.clone(),
    );
    work.spec = req.spec.clone();
    work.batch_id = req.batch_id.clone();

    // The isolated checkout, named the way Claude Code names its own so the two
    // are indistinguishable on disk and its cleanup sweep understands both.
    let dir = if req.worktree {
        let base = match &config.project.base_branch {
            Some(b) => b.clone(),
            None => crate::git::base_branch(&root).await,
        };
        let branch = work.branch_name();
        let dir = crate::git::create_worktree(&root, &work.slug(), &branch, &base)
            .await
            .context("creating the worktree")?;
        work.worktree = Some(dir.clone());
        work.branch = Some(branch);

        let copied = crate::git::copy_includes(&root, &dir, &config.workspace.include);
        if !copied.is_empty() {
            tracing::info!(files = ?copied, "copied gitignored files into the worktree");
        }
        if let Some(setup) = &config.workspace.setup {
            // A fresh checkout has no dependencies installed. Making the agent
            // discover that costs a turn and a lot of tokens.
            let report = crate::gates::run(
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
            work.setup = Some(report);
        }
        dir
    } else {
        root.clone()
    };

    let agent_id = req
        .agent
        .or_else(|| config.project.default_agent.clone())
        .unwrap_or_else(|| "claude".into());
    let spec = crate::acp::resolve(&agent_id, &state.agents)
        .with_context(|| format!("unknown agent `{agent_id}`"))?;

    work.phase = Phase::Implement;
    let work_id = work.id.clone();

    // A project that declared a pipeline for this kind of work gets the chain
    // it wrote down; everything else gets one agent and the project's gates.
    // The kind chooses the pipeline, which is why `--kind` is the only thing a
    // person has to decide.
    if config.pipeline_for(req.kind.as_str()).is_some() {
        work.updated_at = jiff::Timestamp::now();
        persist(state, &work).await;
        state.works.lock().await.insert(work_id.clone(), work);
        crate::pipeline::begin(state, &work_id, &config, req.kind.as_str()).await?;
        return Ok(work_id);
    }

    // **Started silent, registered, and only then prompted.** Dispatching with
    // the prompt attached sends it before this work exists in either map — and
    // an agent that answers quickly ends its turn before the mapping lands, so
    // `on_turn_ended` looks the run up, finds nothing, and returns. The work
    // then sits in `Implement` for ever: the gates never run, and a claim of
    // done is never checked, which is the one failure this whole module exists
    // to prevent. Widening the window to 1.5s reproduces it every time.
    let run_id = crate::driven::dispatch_for_work(state, &spec, dir, &work_id).await?;
    work.runs.push(run_id.clone());
    work.updated_at = jiff::Timestamp::now();

    persist(state, &work).await;
    state.works.lock().await.insert(work_id.clone(), work);
    state
        .work_of_run
        .lock()
        .await
        .insert(run_id.clone(), work_id.clone());

    // The turn starts here, with both maps already answering for this run.
    crate::driven::prompt(state, &run_id, req.prompt).await?;

    Ok(work_id)
}

/// Called when a run bound to a piece of work finishes a turn.
///
/// This is the hinge: the agent has stopped, so either the project's checks
/// agree it is done, or the work goes back with the reasons.
pub async fn on_turn_ended(state: &Shared, run: &RunId) {
    let work_id = { state.work_of_run.lock().await.get(run).cloned() };
    let Some(work_id) = work_id else { return };

    // A declared chain decides for itself what comes after a step; the fixed
    // implement → verify → review below is what work without one gets.
    let piped = {
        let works = state.works.lock().await;
        works.get(&work_id).is_some_and(|w| w.pipeline.is_some())
    };
    if piped {
        crate::pipeline::on_turn_ended(state, &work_id).await;
        return;
    }

    // The transition out of `Implement` is claimed under the lock, so two turn
    // ends arriving together — a hook and the protocol both reporting the same
    // stop — cannot both start the gates and charge the project twice for them.
    let (dir, attempt, rounds) = {
        let mut works = state.works.lock().await;
        let Some(w) = works.get_mut(&work_id) else {
            return;
        };
        if w.phase != Phase::Implement {
            return;
        }
        w.phase = Phase::Verify;
        w.updated_at = jiff::Timestamp::now();
        (
            w.worktree.clone(),
            w.gates.len() as u32 + 1,
            w.feedback_rounds,
        )
    };
    let Some(dir) = dir else {
        set_phase(state, &work_id, Step::Implement).await;
        return;
    };

    // Inside a worktree, `rev-parse --show-toplevel` is the worktree itself, so
    // the configuration that governs this work is the one committed on its
    // branch. That is the right answer: a project's definition of done should
    // not change because somebody has an unsaved edit in another window.
    let root = crate::git::repo_root(&dir).await.unwrap_or(dir.clone());
    let config = match ProjectConfig::load(&root) {
        Ok(c) => c,
        Err(e) => {
            // A definition of done that cannot be read is not a pass. Saying so
            // is the point; carrying on would claim a check that never ran.
            tracing::warn!(error = %e, "cannot read the project config");
            stop(
                state,
                &work_id,
                Stopped::Broken {
                    detail: format!("this project's devplane.toml cannot be read: {e}"),
                },
            )
            .await;
            return;
        }
    };
    if !config.has_gates() {
        // No Definition of Done, so there is nothing to verify and nothing to
        // claim — and no pull request either, because its body would have to
        // say the checks passed. The work waits for a person instead.
        //
        // The bill is still real, and still worth showing.
        tally(state, &work_id).await;
        set_phase(state, &work_id, Step::Review).await;
        retire_runs(state, &work_id).await;
        return;
    }

    // Before the next check and before any further round: a ceiling that is
    // only looked at when the work finishes is a ceiling that never stopped
    // anything.
    if charge(state, &work_id, &config).await {
        return;
    }

    state.notify_changed();
    let mut report = crate::gates::run(
        "check",
        &config.gates.check,
        &dir,
        config.gates.timeout,
        attempt,
    )
    .await;
    stamp_spec(state, &work_id, &mut report, &dir).await;
    let passed = report.passed();
    let feedback = report.feedback();
    let summary = report.summary();

    record_gate(state, &work_id, &report).await;
    {
        let mut works = state.works.lock().await;
        if let Some(w) = works.get_mut(&work_id) {
            w.gates.push(report);
            w.updated_at = jiff::Timestamp::now();
        }
    }

    if passed {
        tracing::info!(work = %work_id, "gates passed");
        // Only now. A pull request opened before the checks pass is a
        // notification to other people that something is ready when it is not.
        open_pull_request(state, &work_id, &config).await;
        set_phase(state, &work_id, Step::Review).await;
        retire_runs(state, &work_id).await;
        return;
    }

    use crate::core::config::OnFail;
    match config.gates.on_fail {
        OnFail::Ignore => set_phase(state, &work_id, Step::Review).await,
        OnFail::Escalate => {
            tracing::info!(work = %work_id, %summary, "gates failed; asking a human");
            stop(
                state,
                &work_id,
                Stopped::GateFailed {
                    gate: "check".into(),
                },
            )
            .await;
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
            set_phase(state, &work_id, Step::Implement).await;
            if let Err(e) = crate::driven::prompt(state, run, feedback).await {
                tracing::warn!(error = %e, "could not hand the failures back");
                stop(
                    state,
                    &work_id,
                    Stopped::Broken {
                        detail: format!("the failures could not be handed back: {e}"),
                    },
                )
                .await;
            }
        }
        OnFail::Feedback => {
            // The budget is spent. An agent and a gate can disagree forever,
            // and every round costs money.
            tracing::info!(work = %work_id, %summary, "feedback budget spent");
            stop(
                state,
                &work_id,
                Stopped::GateFailed {
                    gate: "check".into(),
                },
            )
            .await;
        }
    }
}

/// Writes a gate verdict to the decision log.
///
/// The reason is the gate's own summary, so "why was this allowed through" is
/// answerable with the commands that ran rather than with the word "passed".
pub(crate) async fn record_gate(state: &Shared, id: &WorkId, report: &crate::core::GateReport) {
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Daemon,
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
            .for_work(id),
        )
        .await;
}

/// Opens a pull request for finished work, if the project asked for one.
///
/// Failures here are reported and dropped rather than failing the work: the
/// code is written and the checks passed, and a missing remote or an unlogged-in
/// `gh` is a setup problem, not a reason to throw that away.
pub(crate) async fn open_pull_request(state: &Shared, id: &WorkId, config: &ProjectConfig) {
    if !config.github.pull_request {
        return;
    }
    let (dir, branch, title, existing) = {
        let works = state.works.lock().await;
        let Some(w) = works.get(id) else { return };
        match (&w.worktree, &w.branch) {
            (Some(d), Some(b)) => (
                d.clone(),
                b.clone(),
                w.title.clone(),
                w.pull_request.is_some(),
            ),
            _ => return,
        }
    };
    if existing {
        return;
    }
    if !crate::github::is_available(&dir).await {
        tracing::info!("no GitHub remote here; skipping the pull request");
        return;
    }

    // The branch has to exist on the remote before a pull request can point at
    // it. This is the first thing Devplane does that other people can see,
    // which is why `[github].pull_request` is off until asked for.
    if let Err(e) = push_branch(&dir, &branch).await {
        tracing::warn!(error = %e, "could not push the branch");
        state
            .record(
                crate::core::Decision::new(
                    crate::core::Authority::Daemon,
                    "git:push",
                    branch.clone(),
                    "fail",
                )
                .because(e.to_string())
                .for_work(id),
            )
            .await;
        return;
    }
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Daemon,
                "git:push",
                branch.clone(),
                "done",
            )
            .because("the project's gates passed")
            .for_work(id),
        )
        .await;

    let base = match &config.project.base_branch {
        Some(b) => b.clone(),
        None => crate::git::base_branch(&dir).await,
    };
    let body = pr_body(state, id).await;
    match crate::github::create_pr(&dir, &branch, &base, &title, &body, config.github.draft).await {
        Ok(pr) => {
            tracing::info!(number = pr.number, "opened a pull request");
            state
                .record(
                    crate::core::Decision::new(
                        crate::core::Authority::Daemon,
                        "gh:pr.create",
                        pr.url.clone(),
                        "done",
                    )
                    .because(match config.github.draft {
                        true => "gates passed; opened as a draft",
                        false => "gates passed",
                    })
                    .for_work(id),
                )
                .await;
            let mut works = state.works.lock().await;
            if let Some(w) = works.get_mut(id) {
                w.pull_request = Some(crate::core::work::PullRequestRef {
                    number: pr.number,
                    url: pr.url.clone(),
                    status: pr.status().as_str().to_string(),
                    failing_checks: Vec::new(),
                });
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not open a pull request"),
    }
}

/// Adds up what this work has spent, and records it.
///
/// Separate from [`charge`] because the figure is worth having whether or not a
/// ceiling exists: work in a project with no gates never reached `charge` at
/// all, so `devplane work show` printed "not reported by this agent" for a run
/// that had reported perfectly well.
pub(crate) async fn tally(state: &Shared, id: &WorkId) -> (f64, WorkKind, String) {
    let (spent, _, _, kind, title) = tally_all(state, id).await;
    (spent, kind, title)
}

/// What a piece of work has spent, on all three axes.
///
/// Money is the one a project usually writes down and the only one that can be
/// missing: the protocol makes the agent's cost field optional, and the GenAI
/// telemetry conventions have no notion of money at all. Turns and elapsed time
/// are counted from what Devplane saw itself, so they bind every agent on
/// every provider — which is the whole reason they exist.
pub(crate) async fn tally_all(
    state: &Shared,
    id: &WorkId,
) -> (f64, u32, std::time::Duration, WorkKind, String) {
    let (runs, kind, title, started) = {
        let works = state.works.lock().await;
        match works.get(id) {
            Some(w) => (w.runs.clone(), w.kind, w.title.clone(), w.created_at),
            None => {
                return (
                    0.0,
                    0,
                    std::time::Duration::ZERO,
                    WorkKind::Quick,
                    String::new(),
                );
            }
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
    // `duration_since`, not a `Span` round-tripped through its own `Display`.
    // The old spelling formatted the difference as ISO-8601 and re-parsed it,
    // and the failure arm was `Duration::ZERO` — so anything that made the
    // round trip fail turned `max_runtime` into a budget that never fires,
    // silently, which is the one way a guard must not be wrong.
    let elapsed = jiff::Timestamp::now()
        .duration_since(started)
        .unsigned_abs();
    // Kept on the row: runs age off the board after a week and the bill does
    // not stop being true.
    if let Some(w) = state.works.lock().await.get_mut(id) {
        w.cost_usd = spent;
    }
    (spent, turns, elapsed, kind, title)
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

/// What the pull request says about itself.
///
/// The gate report goes in because it is the evidence: a reviewer can see which
/// checks ran and that they passed, rather than taking the description's word.
async fn pr_body(state: &Shared, id: &WorkId) -> String {
    let works = state.works.lock().await;
    let Some(w) = works.get(id) else {
        return String::new();
    };
    let mut body = format!("{}\n\n", w.prompt);
    if let Some(g) = w.last_gate() {
        body.push_str("## Verification\n\n");
        for c in &g.commands {
            body.push_str(&format!(
                "- `{}` — {}\n",
                c.command,
                if c.passed() { "passed" } else { "failed" }
            ));
        }
        if w.feedback_rounds > 0 {
            body.push_str(&format!(
                "\nThe checks failed {} time(s) first; the failures were handed back and fixed.\n",
                w.feedback_rounds
            ));
        }
    }
    body.push_str("\n---\n_Opened by Devplane after the project's own checks passed._\n");
    body
}

/// Adds up what this work has spent, and stops it if the project set a ceiling
/// it has passed.
///
/// Returns `true` when the work was stopped, so the caller goes no further.
///
/// The count-based bounds stop an agent and a test suite arguing for ever; this
/// is the other way an afternoon gets expensive — one turn going a long way on
/// its own. It only fires when the agent reports what it spent, which ACP makes
/// optional, and [`crate::core::config::Budget`] says so rather than implying a
/// guard that may not be there.
pub(crate) async fn charge(state: &Shared, id: &WorkId, config: &ProjectConfig) -> bool {
    let (spent, turns, elapsed, kind, title) = tally_all(state, id).await;

    // Three bounds, checked in the order of how confident each is that it saw
    // what it is bounding. Turns and elapsed time are counted from events
    // Devplane recorded itself; money is what an agent chose to report, and
    // both the protocol and the GenAI telemetry conventions make that optional.
    let ceiling = config.budget.for_kind(kind.as_str());
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
        let mut works = state.works.lock().await;
        let Some(w) = works.get_mut(id) else {
            return false;
        };
        // Cleared when a raised ceiling now covers what was spent. Without
        // this the flag was permanent, and so was the refusal behind it: the
        // inbox item said "raise the ceiling in devplane.toml if it is worth
        // more", the person did, and `work retry` went on refusing for a
        // ceiling that no longer existed. A remedy a product names has to work.
        w.stopped = over
            .clone()
            .map(|bound| crate::core::work::Stopped::OverBudget {
                spent_usd: spent,
                bound,
            });
    }
    let Some(bound) = over else { return false };

    tracing::info!(work = %id, spent, turns, %bound, "a bound was reached");
    state
        .record(
            crate::core::Decision::new(crate::core::Authority::Daemon, "work:stop", title, "done")
                .because(&bound)
                .for_work(id),
        )
        .await;
    // The reason was written on the row a few lines above, before the phase
    // moved: `stopped` and `Phase::Failed` are set together everywhere, because
    // a failed row without one is a row the inbox has to guess about.
    set_phase(state, id, Step::Failed).await;
    retire_runs(state, id).await;
    true
}

/// Stops every agent this work still has running.
///
/// Called when the work reaches a state nothing more will be said in: a step
/// the chain has moved past, a finished chain, a released human step. Each
/// driven run is a process group that lives until somebody ends it, so without
/// this a four-step pipeline holds four agents in memory for as long as the
/// daemon runs.
///
/// Deliberately *not* called for work that failed: `retry` hands the failures
/// back to the session that wrote the code, and a fresh agent would have to
/// learn all of it again.
pub(crate) async fn retire_runs(state: &Shared, id: &WorkId) {
    let runs = {
        let works = state.works.lock().await;
        works.get(id).map(|w| w.runs.clone()).unwrap_or_default()
    };
    for run in &runs {
        let _ = crate::driven::stop(state, run).await;
    }
    // Waited for, not fired and forgotten.
    //
    // Tearing a connection down kills the agent's process group, and the next
    // step spawns a new one. Letting those overlap means starting an agent
    // while the last one is still dying — which is both a race nobody needs and
    // a `max_parallel_runs` count that is briefly wrong. Bounded, because a
    // wedged agent must not hold up the work that is leaving it behind.
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
    tracing::warn!(work = %id, "an agent did not stop when asked");
}

/// Records which other in-flight work in this repository is editing the same
/// files, and returns the list.
///
/// An isolated checkout stops two agents overwriting each other while they
/// run, and does nothing about the failure they actually have: two branches
/// each locally correct that cannot both land. `max_parallel_runs` counts
/// agents, which is the wrong axis — two in different parts of a repository
/// never collide, two in one file always do.
///
/// A measurement rather than a gate: nothing is declared in advance, nothing is
/// refused, and a person is told before the merge instead of by it.
///
/// One `git diff` per worktree, at a phase transition — never on the poller's
/// timer, where a repository with twenty worktrees would pay it every tick.
pub(crate) async fn note_overlaps(state: &Shared, id: &WorkId) -> Vec<crate::core::Overlap> {
    let (project_id, mine) = {
        let works = state.works.lock().await;
        let Some(w) = works.get(id) else {
            return Vec::new();
        };
        let Some(dir) = w.worktree.clone() else {
            return Vec::new();
        };
        (w.project_id.clone(), dir)
    };
    // The project's base branch, from the one place that knows it. A worktree
    // does not carry the name of what it diverged from.
    let base = {
        let world = state.world.lock().await;
        world
            .project(&project_id)
            .map(|p| p.root.clone())
            .and_then(|root| crate::core::ProjectConfig::load(&root).ok())
            .and_then(|c| c.project.base_branch)
            .unwrap_or_else(|| "HEAD".into())
    };

    // The others worth asking about: same repository, still in flight, and not
    // this one. Work that has finished cannot collide with anything.
    let others: Vec<(WorkId, String, std::path::PathBuf)> = {
        let works = state.works.lock().await;
        works
            .values()
            .filter(|w| {
                &w.id != id
                    && w.project_id == project_id
                    && !w.phase.is_finished()
                    && w.worktree.is_some()
            })
            .map(|w| {
                (
                    w.id.clone(),
                    w.title.clone(),
                    w.worktree.clone().unwrap_or_default(),
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
    for (wid, title, dir) in others {
        let theirs = crate::git::touched_files(&dir, &base).await;
        let shared: Vec<String> = theirs.into_iter().filter(|f| ours.contains(f)).collect();
        if !shared.is_empty() {
            out.push(crate::core::Overlap {
                work_id: wid,
                title,
                files: shared.into_iter().take(20).collect(),
            });
        }
    }
    out
}

/// A phase this module may move work into without recording anything.
///
/// **`Done` is deliberately not in here.** It is the one transition that makes
/// this product's headline claim, so it cannot be reachable by passing a value
/// to a general-purpose setter — there is no way to *express* it through
/// `set_phase`, and `finish` is the only door, which takes the basis as an
/// argument it cannot be called without. A completion with nothing attached is
/// therefore not something a caller can forget to supply; it is something they
/// cannot write down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Implement,
    Verify,
    Review,
    Human,
    Failed,
}

impl From<Step> for Phase {
    fn from(s: Step) -> Phase {
        match s {
            Step::Implement => Phase::Implement,
            Step::Verify => Phase::Verify,
            Step::Review => Phase::Review,
            Step::Human => Phase::Human,
            Step::Failed => Phase::Failed,
        }
    }
}

pub(crate) async fn set_phase(state: &Shared, id: &WorkId, step: Step) {
    let phase: Phase = step.into();
    // Asked once, where it is cheap and where the answer is worth the most: the
    // moment the code stops changing and before anybody is asked to look at it.
    let overlaps = if matches!(phase, Phase::Review | Phase::Human) {
        note_overlaps(state, id).await
    } else {
        Vec::new()
    };
    let saved = {
        let mut works = state.works.lock().await;
        match works.get_mut(id) {
            Some(w) => {
                w.phase = phase;
                if matches!(phase, Phase::Review | Phase::Human) {
                    w.overlaps = overlaps;
                }
                w.updated_at = jiff::Timestamp::now();
                Some(w.clone())
            }
            None => None,
        }
    };
    if let Some(w) = saved {
        persist(state, &w).await;
        state.notify_changed();
    }
}

/// Runs the gates on demand, whatever phase the work is in.
///
/// Re-running checks is a read: it says what the project thinks of the code
/// right now, and never moves a phase a person asked for. Overwriting one would
/// release a pipeline parked at a human step, or park it for ever.
pub async fn verify(state: &Shared, id: &WorkId) -> Result<crate::core::GateReport> {
    let (dir, attempt, phase) = {
        let works = state.works.lock().await;
        let w = works.get(id).context("no such work")?;
        (
            w.worktree.clone().context("that work has no checkout")?,
            w.gates.len() as u32 + 1,
            w.phase,
        )
    };
    let root = crate::git::repo_root(&dir).await.unwrap_or(dir.clone());
    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    if !config.has_gates() {
        bail!("{} defines no gates", root.display());
    }
    let mut report = crate::gates::run(
        "check",
        &config.gates.check,
        &dir,
        config.gates.timeout,
        attempt,
    )
    .await;
    stamp_spec(state, id, &mut report, &dir).await;
    let passed = report.passed();
    record_gate(state, id, &report).await;
    {
        let mut works = state.works.lock().await;
        if let Some(w) = works.get_mut(id) {
            w.gates.push(report.clone());
        }
    }
    // Only work that is simply sitting on a verdict follows the new one. A
    // pipeline waiting for a person, or one already finished, keeps its place.
    if matches!(phase, Phase::Review | Phase::Failed | Phase::Verify) {
        // A re-run replaces whatever the row last said about why it stopped:
        // green clears it, red is a fresh gate failure. Leaving the old reason
        // behind would show a reviewer's findings beside a check that has since
        // been run again.
        {
            let mut works = state.works.lock().await;
            if let Some(w) = works.get_mut(id) {
                w.stopped = (!passed).then(|| Stopped::GateFailed {
                    gate: "check".into(),
                });
            }
        }
        // `set_phase` writes the whole row, gate report included.
        set_phase(state, id, if passed { Step::Review } else { Step::Failed }).await;
    } else {
        // The phase is somebody's decision and is left alone — but the report
        // still has to reach disk. It did not, so running `work verify` on a
        // pipeline parked at a human step produced a verdict that was on the
        // board until the next restart and then was not.
        let saved = state.works.lock().await.get(id).cloned();
        if let Some(w) = saved {
            persist(state, &w).await;
        }
        state.notify_changed();
    }
    Ok(report)
}

/// Picks a piece of work back up after the daemon that was running it stopped.
///
/// The row, the branch and the worktree all survive a restart; the agent
/// process does not. Resuming reconnects to the *same* agent-side conversation,
/// so the work continues from where it stopped rather than paying a second time
/// to rediscover what the first session already knew.
///
/// Refuses where there is nothing to continue — no run, no agent session, or an
/// agent already working — rather than quietly starting a fresh one under the
/// same Work. A restart wearing a resume's name is the failure this is for.
pub async fn resume(state: &Shared, id: &WorkId) -> Result<()> {
    let (phase, run) = {
        let works = state.works.lock().await;
        let w = works.get(id).context("no such work")?;
        (w.phase, w.current_run().cloned())
    };
    if !matches!(phase, Phase::Implement | Phase::Verify) {
        bail!("that work is not mid-flight; there is nothing to pick up");
    }
    let run = run.context("that work never had an agent — start it again instead")?;
    crate::driven::resume(state, &run).await?;

    // Back to `Implement`: the agent is in the worktree with the conversation
    // it had, and the next thing that happens is its turn ending, which is what
    // sends the work to the gates.
    set_phase(state, id, Step::Implement).await;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "work:resume",
                run.to_string(),
                "resumed",
            )
            .because("picked the work up after a restart")
            .for_work(id),
        )
        .await;
    Ok(())
}

/// Hands the failures back to the agent once more, past the project's bound.
///
/// The bound exists to stop the *machine* arguing with a test suite for ever,
/// and every round of that costs real money. It was never meant to stop a
/// person deciding that one more go is worth it — so this is a deliberate
/// gesture, it is recorded as one, and it counts, so the next automatic round
/// still respects the budget from where the human left it.
///
/// It needs the session that wrote the code to still be there. A fresh agent
/// would have to learn the whole context again, which is exactly what the
/// feedback loop exists to avoid, so this refuses rather than quietly starting
/// one and says what to do instead.
pub async fn retry(state: &Shared, id: &WorkId) -> Result<()> {
    let (phase, run, report, title, stopped, worktree, kind) = {
        let works = state.works.lock().await;
        let w = works.get(id).context("no such work")?;
        (
            w.phase,
            w.current_run().cloned(),
            w.last_gate().cloned(),
            w.title.clone(),
            w.stopped.clone(),
            w.worktree.clone(),
            w.kind,
        )
    };
    if phase != Phase::Failed {
        bail!("that work is not waiting on a failed check");
    }
    let stopped = stopped
        .context("that work stopped without recording why, so there is nothing to hand back")?;

    // What goes back to the agent depends on *why* it stopped, which is read
    // rather than guessed. Handing a reviewer's findings back as though they
    // were a gate report — or a passing gate's summary back as though it were
    // a failure — is how this used to read, and it hands an agent nothing it
    // can act on while telling a person it has.
    let feedback = match &stopped {
        Stopped::Broken { detail } => bail!(
            "that work stopped because the chain could not continue ({detail}), which is \
             a problem with the pipeline rather than the code. `devplane check` reads \
             the file the same way this did."
        ),
        Stopped::OverBudget { spent_usd, bound } => {
            // Re-read rather than trusted. The reason was written when the
            // ceiling was passed, and the message it produces tells the person
            // to raise that ceiling — so the next thing this has to do is
            // notice that they did. Refusing against a number the file no
            // longer contains is a remedy the product names and then refuses
            // to honour.
            let still_over = match &worktree {
                Some(dir) => {
                    let root = crate::git::repo_root(dir)
                        .await
                        .unwrap_or_else(|| dir.clone());
                    ProjectConfig::load(&root)
                        .ok()
                        .and_then(|c| c.budget.for_kind(kind.as_str()))
                        .is_some_and(|ceiling| *spent_usd > ceiling)
                }
                None => true,
            };
            if still_over {
                bail!(
                    "that work stopped: {bound}. The bound `[budget]` sets \
                     for its kind. Raise it in devplane.toml if it is worth more."
                );
            }
            // The ceiling was raised. What the agent needs handed back is
            // whatever the last check said, if anything did.
            report
                .as_ref()
                .map(crate::core::work::GateReport::feedback)
                .context(
                    "that work stopped before any check ran, so there is nothing to hand back",
                )?
        }
        // Taken from the variant rather than through the `Option` its summary
        // returns. The `expect` that was here could not fire today, and that
        // is exactly the kind of invariant that stops holding when somebody
        // adds a variant — and this runs inside a request handler, where the
        // cost of being wrong is a panicking daemon rather than an error.
        Stopped::ReviewExhausted { findings, .. } => format!(
            "A reviewer found these and they are not yet addressed. Fix the cause \
             rather than the symptom, and do not disable a check to make it pass.\n\n{findings}"
        ),
        Stopped::GateFailed { .. } => report
            .as_ref()
            .map(crate::core::work::GateReport::feedback)
            .context("that work stopped before any check ran, so there is nothing to hand back")?,
    };

    let run = run.context("that work has no run")?;
    if !crate::driven::is_live(state, &run).await {
        bail!(
            "the agent that wrote this code is gone, and a fresh one would have to \
             learn it all again. Pick it up yourself with `devplane attach {run}`, \
             or start new work."
        );
    }

    let rounds = {
        let mut works = state.works.lock().await;
        let w = works.get_mut(id).context("no such work")?;
        w.feedback_rounds += 1;
        // The reason is spent with the round it bought: leaving it set would
        // leave the board showing a stopped item that is running again.
        w.stopped = None;
        w.feedback_rounds
    };
    state
        .record(
            crate::core::Decision::new(crate::core::Authority::Person, "work:retry", title, "done")
                .because(format!(
                    "{} — handed back again, round {rounds}, past the project's bound",
                    stopped.headline()
                ))
                .for_work(id),
        )
        .await;

    set_phase(state, id, Step::Implement).await;
    crate::driven::prompt(state, &run, feedback).await
}

/// Stops a piece of work, recording **why** in the same breath.
///
/// The pairing is the point. `set_phase(Failed)` on its own leaves the inbox to
/// work out what happened from whatever is lying around, and what was lying
/// around was the last gate report — so a chain stopped by a reviewer was
/// announced as a gate failure naming a gate that had passed. A reason that is
/// written where the decision is made cannot disagree with it.
pub(crate) async fn stop(state: &Shared, id: &WorkId, why: Stopped) {
    {
        let mut works = state.works.lock().await;
        if let Some(w) = works.get_mut(id) {
            w.stopped = Some(why.clone());
        }
    }
    set_phase(state, id, Step::Failed).await;
    tracing::info!(work = %id, reason = %why.headline(), "work stopped");
}

/// Finishes a piece of work, optionally removing its checkout.
pub async fn finish(state: &Shared, id: &WorkId, remove_worktree: bool, force: bool) -> Result<()> {
    let work = {
        let works = state.works.lock().await;
        works.get(id).cloned().context("no such work")?
    };
    // **The basis is decided here, where the decision is.** Deriving it later
    // from the gate history is the mistake `stop` exists to prevent: a reason
    // reconstructed afterwards disagrees with the moment it describes.
    let declares_gates = match &work.worktree {
        Some(dir) => {
            let root = crate::git::repo_root(dir).await.unwrap_or(dir.clone());
            ProjectConfig::load(&root)
                .map(|c| c.has_gates())
                .unwrap_or(false)
        }
        None => false,
    };
    let basis = Completion::of(&work, declares_gates);

    retire_runs(state, id).await;
    if remove_worktree && let Some(dir) = &work.worktree {
        let root = crate::git::repo_root(dir).await.unwrap_or(dir.clone());
        crate::git::remove_worktree(&root, dir, force).await?;
    }
    mark_done(state, id, basis).await;
    Ok(())
}

/// **The only writer of `Phase::Done` in this program.**
///
/// It takes the basis rather than deriving one, so there is no path to a
/// finished piece of work whose record is silent about what it rests on. The
/// general phase setter cannot express `Done` at all — see `Step`.
async fn mark_done(state: &Shared, id: &WorkId, basis: Completion) {
    let saved = {
        let mut works = state.works.lock().await;
        match works.get_mut(id) {
            Some(w) => {
                w.completion = Some(basis.clone());
                w.phase = Phase::Done;
                w.updated_at = jiff::Timestamp::now();
                Some(w.clone())
            }
            None => None,
        }
    };
    if let Some(w) = saved {
        persist(state, &w).await;
        state
            .record(
                crate::core::Decision::new(
                    crate::core::Authority::Person,
                    "work:done",
                    w.title.clone(),
                    "done",
                )
                .because(basis.headline())
                .for_work(&w.id),
            )
            .await;
        tracing::info!(work = %id, basis = %basis.headline(), "work finished");
    }
}

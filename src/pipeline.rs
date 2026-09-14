//! Declared chains of agent runs.
//!
//! "Start Claude to implement; when it is done, start an agent to check it" is
//! only worth having if the chain is written down in the repository rather than
//! improvised per session. A pipeline is that: roles bound to agents, gates
//! between them, a bounded loop back when a reviewer finds something, and human
//! steps where the project says a person decides.
//!
//! Two rules shape everything here. **The cursor lives on the Work**, not in
//! the process running it, so a daemon that dies between review and verify
//! resumes at verify instead of paying for implement twice. And **a reviewer
//! writes its findings to a file**, not into prose: a file is either there or
//! it is not, a person can read it, and it is the same evidence the next agent
//! is handed. Parsing a model's prose for whether it was happy is a guess
//! dressed as a protocol.

use crate::core::config::{Expect, ProjectConfig, RoleStep, Step};
use crate::core::ids::WorkId;
use crate::core::work::{Phase, Pipeline};
use crate::daemon::Shared;
use anyhow::{Context, Result, bail};
use std::path::Path;

/// The roles of a declared pipeline, in order, for copying onto a Work.
pub fn roles(p: &crate::core::config::Pipeline) -> Vec<String> {
    p.steps.iter().map(|s| s.name().to_string()).collect()
}

/// The step a cursor points at, found by role rather than by index.
///
/// By role, because `vibeplane.toml` can be edited while a chain is running and
/// renumbering a pipeline mid-flight would send the work to a step it never
/// agreed to. The roles copied onto the Work are what it is running; if one has
/// since been deleted, that is an error rather than a guess.
fn step_at<'a>(config: &'a crate::core::config::Pipeline, role: &str) -> Option<&'a Step> {
    config.steps.iter().find(|s| s.name() == role)
}

/// Finds the text behind a step's `prompt`, in the order a project means it.
///
/// 1. `.vibeplane/prompts/<name>.md` — the portable form, committed on the
///    branch like the gates. First because a project that wrote one for this
///    chain meant it for this chain, whichever agent runs the step.
/// 2. `.claude/skills/<name>/SKILL.md`, project then personal — Claude Code's
///    own prompt-template format, which a project very likely already has.
///    Only the **body** is taken, and the body is portable: markdown is
///    markdown, and an agent that is not Claude reads it fine.
/// 3. The literal text, so a one-line pipeline needs no files at all.
///
/// The honest limit of step 2, stated rather than discovered: a skill's
/// frontmatter — `allowed-tools`, `context: fork`, `model`, `effort` — are
/// directives the Claude harness applies when *it* loads the skill by name.
/// Inlining the body takes the instructions and none of that.
fn template(dir: &Path, name: &str) -> Option<String> {
    let portable = dir.join(".vibeplane/prompts").join(format!("{name}.md"));
    if let Ok(text) = std::fs::read_to_string(&portable) {
        return Some(text);
    }
    let skill = |root: &Path| root.join(".claude/skills").join(name).join("SKILL.md");
    let personal = dirs::home_dir().map(|h| skill(&h));
    for path in [Some(skill(dir)), personal].into_iter().flatten() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let (_, body) = crate::core::text::split_frontmatter(&text);
            return Some(body.to_string());
        }
    }
    None
}

/// Builds the text a step's agent is given.
fn compose(dir: &Path, step: &RoleStep, title: &str, task: &str, findings: Option<&str>) -> String {
    let base = template(dir, &step.prompt).unwrap_or_else(|| step.prompt.clone());

    let mut text = base
        .replace("{title}", title)
        .replace("{task}", task)
        .replace("{role}", &step.role)
        .replace("{findings}", findings.unwrap_or(""));

    // A template that never mentions the findings would silently drop them, and
    // the loop would run its full budget re-reviewing the same code.
    if let Some(f) = findings
        && !base.contains("{findings}")
    {
        text.push_str("\n\nA reviewer found these and they are not yet addressed:\n\n");
        text.push_str(f);
    }

    if let Some(fd) = &step.findings {
        // The whole mechanism depends on the file being written, so the
        // instruction is not left to the project's template to remember.
        text.push_str(&format!(
            "\n\nWhen you are done, write what you found to `{}`, one finding per \
             line. If you found nothing that must change, do not create that file \
             — an empty file and a missing one both mean the same thing here, and \
             inventing a finding to fill it sends real work backwards.\n",
            fd.file
        ));
    }
    text
}

/// Starts the run for the step the cursor points at.
///
/// Boxed, and explicitly `Send`, because the call graph is a cycle: dispatching
/// a run spawns a pump, a pump that sees a turn end advances the pipeline, and
/// advancing it dispatches the next run. Rust cannot infer `Send` around a
/// cycle, so one edge has to state it.
fn start_step<'a>(
    state: &'a Shared,
    id: &'a WorkId,
    config: &'a ProjectConfig,
    declared: &'a crate::core::config::Pipeline,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(start_step_inner(state, id, config, declared))
}

async fn start_step_inner(
    state: &Shared,
    id: &WorkId,
    config: &ProjectConfig,
    declared: &crate::core::config::Pipeline,
) -> Result<()> {
    // The step being left is finished with. Its agent is a process group that
    // lives until somebody ends it, and this is only ever called when the
    // cursor has moved — a gate handing failures back stays on its own step and
    // prompts the same session instead.
    crate::work::retire_runs(state, id).await;

    let (dir, title, task, role, findings) = {
        let works = state.works.lock().await;
        let w = works.get(id).context("no such work")?;
        let p = w.pipeline.as_ref().context("that work has no pipeline")?;
        (
            w.worktree
                .clone()
                .unwrap_or_else(|| w.project_id.as_str().into()),
            w.title.clone(),
            w.prompt.clone(),
            p.role()
                .context("the pipeline has run off its end")?
                .to_string(),
            p.findings.clone(),
        )
    };

    let Some(Step::Role(step)) = step_at(declared, &role) else {
        bail!("`{role}` is no longer a step of this pipeline");
    };

    // `any` means the project's default rather than a vendor: role separation
    // is the point, naming one twice is not.
    let agent_id = match step.agent.as_deref() {
        Some("any") | None => config
            .project
            .default_agent
            .clone()
            .unwrap_or_else(|| "claude".into()),
        Some(a) => a.to_string(),
    };
    let spec = crate::acp::resolve(&agent_id, &state.agents)
        .with_context(|| format!("unknown agent `{agent_id}`"))?;

    let text = compose(&dir, step, &title, &task, findings.as_deref());
    // Silent first, prompted last — see `work::start` for what goes wrong
    // otherwise. A pipeline step is the case that hurts most: a dropped turn
    // end stops the whole chain, and the cursor still says it is running.
    let run_id = crate::driven::dispatch_for_work(state, &spec, dir, id).await?;

    let saved = {
        let mut works = state.works.lock().await;
        let w = works.get_mut(id).context("no such work")?;
        w.runs.push(run_id.clone());
        w.phase = Phase::Implement;
        w.feedback_rounds = 0;
        w.updated_at = jiff::Timestamp::now();
        if let Some(p) = w.pipeline.as_mut() {
            p.enter()
                .context("the pipeline cursor is past the end of its own steps")?;
            p.findings = None;
        }
        w.clone()
    };
    // The cursor advance above is the whole of the crash-resumption promise:
    // a daemon that dies between review and verify resumes at verify because
    // this row was written. Losing it silently pays for a step twice.
    crate::work::persist(state, &saved).await;
    state
        .work_of_run
        .lock()
        .await
        .insert(run_id.clone(), id.clone());
    crate::driven::prompt(state, &run_id, text).await?;
    state.notify_changed();
    tracing::info!(work = %id, %role, agent = %agent_id, "pipeline step started");
    Ok(())
}

/// Reads and clears a reviewer's findings.
///
/// Cleared because the next round must start from a blank sheet: a file left
/// behind would send the work back a second time for something already fixed.
fn take_findings(dir: &Path, file: &str) -> Option<String> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).ok()?;
    // Not `.ok()`: the bounded loop rests on this removal. A file left behind is
    // read again next round, so one objection would send the work back until
    // the bound is spent.
    if let Err(e) = std::fs::remove_file(&path) {
        tracing::error!(
            path = %path.display(),
            error = %e,
            "could not clear the findings file; the next round would read it again"
        );
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Bounded: a reviewer that pastes the whole diff would otherwise fill the
    // next agent's context with what it already has.
    Some(trimmed.chars().take(8000).collect())
}

/// Called when a step's agent stops. Decides what happens next.
pub async fn on_turn_ended(state: &Shared, id: &WorkId) {
    if let Err(e) = advance(state, id).await {
        tracing::warn!(work = %id, error = %e, "the pipeline could not continue");
        // The error text goes on the work, not only into a log line nobody
        // reads: "`review` is no longer a step of `feature`" tells somebody
        // exactly what to fix.
        crate::work::stop(
            state,
            id,
            crate::core::work::Stopped::Broken {
                detail: e.to_string(),
            },
        )
        .await;
    }
}

async fn advance(state: &Shared, id: &WorkId) -> Result<()> {
    let (dir, role, name, attempt, rounds) = {
        let works = state.works.lock().await;
        let w = works.get(id).context("no such work")?;
        if w.phase != Phase::Implement {
            // Not a step in flight: a stray turn from a run that has already
            // been moved on, or work a human is holding.
            return Ok(());
        }
        let p = w.pipeline.as_ref().context("no pipeline")?;
        (
            w.worktree.clone().context("no checkout")?,
            p.role().context("off the end")?.to_string(),
            p.name.clone(),
            w.gates.len() as u32 + 1,
            w.feedback_rounds,
        )
    };

    let root = crate::git::repo_root(&dir).await.unwrap_or(dir.clone());
    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    let declared = config
        .pipeline_for(&name)
        .with_context(|| format!("`{name}` is no longer declared in vibeplane.toml"))?
        .clone();
    let Some(Step::Role(step)) = step_at(&declared, &role).cloned() else {
        bail!("`{role}` is no longer a step of `{name}`");
    };

    // A chain is where a budget matters most: four steps, each with turns of
    // its own, is the shape that gets expensive without anybody noticing.
    if crate::work::charge(state, id, &config).await {
        return Ok(());
    }

    // 1. The step's own gate, if it declared one.
    if let Some(gate) = &step.gate {
        let (commands, expect, timeout) = config
            .gate_named(gate)
            .with_context(|| format!("`{gate}` is not a declared gate"))?;
        if commands.is_empty() {
            bail!("`{gate}` has no commands, and an empty gate is not a pass");
        }
        crate::work::set_phase(state, id, Phase::Verify).await;
        let report = crate::gates::run_expecting(
            gate,
            &commands,
            &dir,
            timeout,
            attempt,
            expect == Expect::Fail,
        )
        .await;
        let met = report.passed();
        let feedback = report.feedback();
        let summary = report.summary();
        crate::work::record_gate(state, id, &report).await;
        {
            let mut works = state.works.lock().await;
            if let Some(w) = works.get_mut(id) {
                w.gates.push(report);
                w.updated_at = jiff::Timestamp::now();
            }
        }

        if !met {
            if rounds >= config.gates.max_feedback_rounds {
                tracing::info!(work = %id, %summary, "feedback budget spent");
                crate::work::stop(
                    state,
                    id,
                    crate::core::work::Stopped::GateFailed { gate: gate.clone() },
                )
                .await;
                return Ok(());
            }
            // Back to the same session: it still has the context that wrote the
            // code, and a fresh one would pay to learn it again.
            let run = {
                let mut works = state.works.lock().await;
                let w = works.get_mut(id).context("no such work")?;
                w.feedback_rounds += 1;
                w.current_run().cloned().context("no run to tell")?
            };
            crate::work::set_phase(state, id, Phase::Implement).await;
            crate::driven::prompt(state, &run, feedback).await?;
            return Ok(());
        }
    }

    // 2. Findings, if this step is a reviewing one.
    if let Some(fd) = &step.findings
        && let Some(text) = take_findings(&dir, &fd.file)
    {
        let target = {
            let works = state.works.lock().await;
            let p = works
                .get(id)
                .and_then(|w| w.pipeline.as_ref())
                .context("no pipeline")?;
            p.index_of(&fd.back_to)
                .with_context(|| format!("`{}` is not a step of this pipeline", fd.back_to))?
        };
        let spent = {
            let mut works = state.works.lock().await;
            let w = works.get_mut(id).context("no such work")?;
            let p = w.pipeline.as_mut().context("no pipeline")?;
            // One entry is the first, honest pass; `max` counts the returns.
            let spent = p.entries.get(target).copied().unwrap_or(0) > fd.max;
            if !spent {
                p.step = target;
                p.findings = Some(text.clone());
            }
            spent
        };
        if spent {
            tracing::info!(work = %id, "the review loop is spent; asking a human");
            // Carrying what the reviewer found: `take_findings` has already
            // removed the file, so this text is the only copy left and the
            // only evidence of why the work stopped.
            crate::work::stop(
                state,
                id,
                crate::core::work::Stopped::ReviewExhausted {
                    step: role.clone(),
                    back_to: fd.back_to.clone(),
                    findings: text,
                },
            )
            .await;
            return Ok(());
        }
        tracing::info!(work = %id, back_to = %fd.back_to, "review found something");
        return start_step(state, id, &config, &declared).await;
    }

    // 3. Nothing sent it back, so move on.
    let next = {
        let mut works = state.works.lock().await;
        let w = works.get_mut(id).context("no such work")?;
        let p = w.pipeline.as_mut().context("no pipeline")?;
        p.step += 1;
        p.role().map(str::to_string)
    };

    let Some(next) = next else {
        // The chain is finished. The pull request comes now and not a step
        // earlier: one opened before the last check passes tells other people
        // something is ready when it is not.
        crate::work::open_pull_request(state, id, &config).await;
        crate::work::set_phase(state, id, Phase::Review).await;
        crate::work::retire_runs(state, id).await;
        tracing::info!(work = %id, "pipeline finished");
        return Ok(());
    };

    if matches!(step_at(&declared, &next), Some(Step::Human(_))) {
        crate::work::set_phase(state, id, Phase::Human).await;
        // A chain can wait at a human step for days. Holding an agent open for
        // all of them is a model idling in memory for nothing.
        crate::work::retire_runs(state, id).await;
        tracing::info!(work = %id, step = %next, "pipeline is waiting for a person");
        return Ok(());
    }
    start_step(state, id, &config, &declared).await
}

/// Releases a pipeline held at a human step.
pub async fn approve(state: &Shared, id: &WorkId) -> Result<String> {
    let (dir, phase) = {
        let works = state.works.lock().await;
        let w = works.get(id).context("no such work")?;
        (w.worktree.clone(), w.phase)
    };
    if phase != Phase::Human {
        bail!("that work is not waiting for a person");
    }
    let dir = dir.context("that work has no checkout")?;
    let root = crate::git::repo_root(&dir).await.unwrap_or(dir.clone());
    let config = ProjectConfig::load(&root).map_err(|e| anyhow::anyhow!("{e}"))?;

    let (name, released) = {
        let mut works = state.works.lock().await;
        let w = works.get_mut(id).context("no such work")?;
        let p = w.pipeline.as_mut().context("no pipeline")?;
        let released = p.role().unwrap_or("").to_string();
        p.step += 1;
        (p.name.clone(), released)
    };
    let declared = config
        .pipeline_for(&name)
        .with_context(|| format!("`{name}` is no longer declared in vibeplane.toml"))?
        .clone();

    state
        .record(
            crate::core::Decision::new(
                crate::core::Actor::Human,
                "work:advance",
                released.clone(),
                "done",
            )
            .because(format!("released the `{name}` pipeline"))
            .for_work(id),
        )
        .await;

    let next = {
        let works = state.works.lock().await;
        works
            .get(id)
            .and_then(|w| w.pipeline.as_ref())
            .and_then(|p| p.role().map(str::to_string))
    };

    match next {
        None => {
            crate::work::open_pull_request(state, id, &config).await;
            crate::work::set_phase(state, id, Phase::Review).await;
            crate::work::retire_runs(state, id).await;
        }
        Some(n) if matches!(step_at(&declared, &n), Some(Step::Human(_))) => {
            crate::work::set_phase(state, id, Phase::Human).await;
        }
        Some(_) => start_step(state, id, &config, &declared).await?,
    }
    Ok(released)
}

/// Starts the first step of a pipeline for freshly created work.
pub async fn begin(state: &Shared, id: &WorkId, config: &ProjectConfig, name: &str) -> Result<()> {
    let declared = config
        .pipeline_for(name)
        .with_context(|| format!("`{name}` declares no steps"))?
        .clone();
    let role_list = roles(&declared);

    // A pipeline that opens with a human step would otherwise start an agent
    // and only then ask; the person decides first.
    let first = role_list
        .first()
        .cloned()
        .with_context(|| format!("`{name}` is empty"))?;
    {
        let mut works = state.works.lock().await;
        let w = works.get_mut(id).context("no such work")?;
        w.pipeline = Some(Pipeline::new(name.to_string(), role_list));
    }
    if matches!(step_at(&declared, &first), Some(Step::Human(_))) {
        crate::work::set_phase(state, id, Phase::Human).await;
        return Ok(());
    }
    start_step(state, id, config, &declared).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Findings;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vp-pipe-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn step(prompt: &str, findings: Option<Findings>) -> RoleStep {
        RoleStep {
            role: "review".into(),
            agent: None,
            prompt: prompt.into(),
            gate: None,
            findings,
        }
    }

    fn sends_back() -> Findings {
        Findings {
            back_to: "implement".into(),
            max: 1,
            file: ".vibeplane/findings.md".into(),
        }
    }

    #[test]
    fn a_prompt_is_a_template_when_there_is_one_and_the_text_itself_otherwise() {
        // A one-line pipeline should not require creating a directory of files
        // first, and a project that wants real prompts should not have to
        // inline them into TOML.
        let d = dir("prompt");
        let bare = compose(
            &d,
            &step("review this change", None),
            "T",
            "do a thing",
            None,
        );
        assert!(bare.starts_with("review this change"));

        std::fs::create_dir_all(d.join(".vibeplane/prompts")).unwrap();
        std::fs::write(
            d.join(".vibeplane/prompts/review.md"),
            "Review {title}. The ask was: {task}",
        )
        .unwrap();
        let templated = compose(&d, &step("review", None), "rate limiting", "add it", None);
        assert_eq!(templated, "Review rate limiting. The ask was: add it");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_claude_skill_is_a_prompt_template_and_the_portable_file_still_wins() {
        // A project that already keeps its prompts as Claude Code Skills
        // should not have to keep a second copy. The body is portable —
        // markdown any agent reads — so this works for a step running on Codex
        // too. What stays Claude's is the frontmatter, which is dropped.
        let d = dir("skill");
        std::fs::create_dir_all(d.join(".claude/skills/review")).unwrap();
        std::fs::write(
            d.join(".claude/skills/review/SKILL.md"),
            "---\nname: review\ndescription: review a diff\nallowed-tools: Bash(git *)\n---\n\nReview {title} closely.",
        )
        .unwrap();

        let from_skill = compose(&d, &step("review", None), "rate limiting", "add it", None);
        assert_eq!(from_skill.trim(), "Review rate limiting closely.");
        assert!(
            !from_skill.contains("allowed-tools"),
            "frontmatter is a directive, not something to tell the model: {from_skill}"
        );

        // And where a project wrote the portable form for this chain, that is
        // what it meant — whichever agent runs the step.
        std::fs::create_dir_all(d.join(".vibeplane/prompts")).unwrap();
        std::fs::write(d.join(".vibeplane/prompts/review.md"), "Portable: {title}").unwrap();
        let overridden = compose(&d, &step("review", None), "rate limiting", "add it", None);
        assert_eq!(overridden, "Portable: rate limiting");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn findings_reach_a_template_that_never_asked_for_them() {
        // A project template that omits `{findings}` would otherwise drop them
        // silently, and the loop would spend its whole budget re-reviewing the
        // same code.
        let d = dir("drop");
        std::fs::create_dir_all(d.join(".vibeplane/prompts")).unwrap();
        std::fs::write(d.join(".vibeplane/prompts/fix.md"), "Fix {title}.").unwrap();
        let text = compose(
            &d,
            &step("fix", None),
            "the login bug",
            "fix it",
            Some("the error path is untested"),
        );
        assert!(text.contains("Fix the login bug."));
        assert!(text.contains("the error path is untested"));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_reviewing_step_is_always_told_where_to_write() {
        // The whole mechanism depends on that file, so the instruction cannot
        // be left to the project's template to remember.
        let d = dir("instruct");
        let text = compose(
            &d,
            &step("review it", Some(sends_back())),
            "T",
            "task",
            None,
        );
        assert!(text.contains(".vibeplane/findings.md"));
        assert!(
            text.contains("do not create that file"),
            "and told that finding nothing is an acceptable answer"
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn findings_are_read_once_and_then_gone() {
        // Left behind, the same file would send the work back a second time for
        // something that was already fixed.
        let d = dir("take");
        std::fs::create_dir_all(d.join(".vibeplane")).unwrap();
        let path = d.join(".vibeplane/findings.md");
        std::fs::write(&path, "  the error path is untested\n").unwrap();

        assert_eq!(
            take_findings(&d, ".vibeplane/findings.md").as_deref(),
            Some("the error path is untested")
        );
        assert!(!path.exists());
        assert_eq!(take_findings(&d, ".vibeplane/findings.md"), None);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn an_empty_findings_file_means_nothing_was_found() {
        // A reviewer that creates the file and writes nothing has said it is
        // happy, not that it has an unspeakable complaint.
        let d = dir("blank");
        std::fs::create_dir_all(d.join(".vibeplane")).unwrap();
        std::fs::write(d.join(".vibeplane/findings.md"), "\n  \n").unwrap();
        assert_eq!(take_findings(&d, ".vibeplane/findings.md"), None);
        std::fs::remove_dir_all(&d).ok();
    }
}

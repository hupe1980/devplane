//! Runs Vibeplane owns, over the Agent Client Protocol.
//!
//! An observed session is something Vibeplane watches; a driven one is
//! something it can answer. The difference on the board is one word, which is
//! the point — the same run, the same inbox, the same project grouping,
//! whether the session was started here or in somebody's terminal.
//!
//! The policy engine is shared deliberately. A rule that auto-allows
//! `Bash(pnpm test *)` applies to a Claude session in VS Code through the hook
//! and to a driven Codex session through this, with one audit trail, because
//! "what may an agent do here" is a property of the project rather than of how
//! the agent happens to be launched.

use crate::daemon::Shared;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use vibeplane_acp::{AcpEvent, AgentSpec, Session};
use vibeplane_core::Verdict;
use vibeplane_domain::event::{ApiUsage, Event, Source, WaitingFor};
use vibeplane_domain::ids::RunId;
use vibeplane_domain::run::RunMode;

/// Starts an agent and registers it as a run.
///
/// Every path that starts an agent comes through here, which is where the trust
/// gate belongs. Putting it only in `work::start` left `dispatch` as a way
/// around it — a check that one caller can skip is not a check.
pub async fn dispatch(
    state: &Shared,
    spec: &AgentSpec,
    cwd: PathBuf,
    prompt: Option<String>,
) -> Result<RunId> {
    if !cwd.is_dir() {
        anyhow::bail!("{} is not a directory", cwd.display());
    }
    let cwd = cwd
        .canonicalize()
        .with_context(|| format!("resolving {}", cwd.display()))?;
    require_trust(state, &cwd).await?;
    let (session, mut events) = vibeplane_acp::spawn(spec, cwd.clone())
        .await
        .with_context(|| format!("starting {}", spec.id))?;

    // The run id is minted here rather than taken from the agent: the board
    // needs a row the moment the process starts, and the agent's own session id
    // only arrives after its handshake.
    let run_id = RunId::new(format!("acp-{}", uuid::Uuid::now_v7().simple()));

    state
        .ingest(
            run_id.clone(),
            Source::Daemon,
            Event::SessionStarted {
                cwd: cwd.clone(),
                source: Some("dispatch".into()),
                model: None,
                entrypoint: Some(format!("acp:{}", spec.id)),
            },
            Some(cwd.clone()),
            RunMode::Driven,
        )
        .await;

    state
        .sessions
        .lock()
        .await
        .insert(run_id.clone(), session.clone());

    // One pump per run, translating the protocol into the board's vocabulary.
    let pump_state = state.clone();
    let pump_run = run_id.clone();
    let pump_cwd = cwd.clone();
    let pump_session = session.clone();
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            handle(&pump_state, &pump_run, &pump_cwd, &pump_session, event).await;
        }
        pump_state.sessions.lock().await.remove(&pump_run);
        pump_state
            .ingest(
                pump_run,
                Source::Daemon,
                Event::SessionEnded { reason: None },
                Some(pump_cwd),
                RunMode::Driven,
            )
            .await;
    });

    if let Some(text) = prompt {
        session.prompt(text).await?;
    }
    Ok(run_id)
}

/// Refuses unless somebody has said this directory is theirs.
///
/// A headless agent runs the repository's own hooks and MCP servers with no
/// dialog of its own, so the decision has to be deliberate and once. A worktree
/// inherits the trust of the repository that owns it: trusting a project and
/// then being asked again for each of its checkouts would teach people to say
/// yes without reading.
pub async fn require_trust(state: &Shared, cwd: &Path) -> Result<()> {
    let root = vibeplane_domain::project::main_checkout_for(cwd)
        .or_else(|| vibeplane_domain::project::find_repo_root(cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let id = vibeplane_domain::ProjectId::from_path(&root);

    let trusted = {
        let w = state.world.lock().await;
        w.project(&id).map(|p| p.trusted).unwrap_or(false)
    };
    if trusted {
        return Ok(());
    }
    anyhow::bail!(
        "{} is not trusted yet. Starting an agent there runs that repository's own hooks \
         and MCP servers without asking. Run `vibeplane trust {}` if you meant to.",
        root.display(),
        root.display()
    )
}

/// Sends a prompt to a driven run.
pub async fn prompt(state: &Shared, run: &RunId, text: String) -> Result<()> {
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Vibeplane drives")?;
    state
        .ingest(
            run.clone(),
            Source::Daemon,
            Event::PromptSubmitted { chars: text.len() },
            None,
            RunMode::Driven,
        )
        .await;
    session.prompt(text).await
}

/// Answers an outstanding permission request.
pub async fn decide(
    state: &Shared,
    run: &RunId,
    request_id: &str,
    option: Option<String>,
) -> Result<()> {
    let session = state
        .sessions
        .lock()
        .await
        .get(run)
        .cloned()
        .context("that run is not one Vibeplane drives")?;
    let decision = if option.is_some() { "allow" } else { "deny" };
    session.decide(request_id, option).await?;

    // Recording the decision is what takes the item out of the inbox. Waiting
    // for the agent's next move instead would leave an answered question on
    // screen until it happened to do something — and if the answer was "no",
    // that might be never.
    state
        .ingest(
            run.clone(),
            Source::Daemon,
            Event::PermissionDecided {
                tool: String::new(),
                decision: decision.into(),
                by: "human".into(),
            },
            None,
            RunMode::Driven,
        )
        .await;
    Ok(())
}

/// Stops a driven run.
pub async fn stop(state: &Shared, run: &RunId) -> Result<()> {
    let session = state.sessions.lock().await.remove(run);
    match session {
        Some(s) => {
            s.stop().await;
            Ok(())
        }
        None => anyhow::bail!("that run is not one Vibeplane drives"),
    }
}

/// Translates one protocol event, applying policy to permission requests.
async fn handle(state: &Shared, run: &RunId, cwd: &Path, session: &Session, event: AcpEvent) {
    let ingest = |e: Event| {
        let state = state.clone();
        let run = run.clone();
        let cwd = cwd.to_path_buf();
        async move {
            state
                .ingest(run, Source::Daemon, e, Some(cwd), RunMode::Driven)
                .await;
        }
    };

    match event {
        AcpEvent::Ready { agent_name, .. } => {
            ingest(Event::SessionStarted {
                cwd: cwd.to_path_buf(),
                source: Some("acp".into()),
                model: agent_name,
                entrypoint: None,
            })
            .await;
        }

        // Streamed text is the answer, not an action. Recording every chunk
        // would bury the log in prose the board never shows; the turn's end is
        // what changes state.
        AcpEvent::Text(_) | AcpEvent::Thought(_) => {}

        AcpEvent::Tool { title, status, .. } => match status.as_deref() {
            Some("completed") => {
                ingest(Event::ToolFinished {
                    tool: title,
                    ok: true,
                    duration_ms: None,
                })
                .await
            }
            Some("failed") => {
                ingest(Event::ToolFinished {
                    tool: title,
                    ok: false,
                    duration_ms: None,
                })
                .await
            }
            _ => {
                ingest(Event::ToolStarted {
                    tool: title,
                    input: serde_json::Value::Null,
                })
                .await
            }
        },

        AcpEvent::PermissionRequested {
            request_id,
            title,
            options,
        } => {
            // The same rules that answer a hook answer this. The protocol names
            // no tool, so the title is what the rule matches against — a rule
            // written for a shell command still reads as one here.
            let verdict = {
                let p = state.policy.lock().await;
                p.evaluate("Bash", &serde_json::json!({ "command": title }))
            };
            let pick = |kind: &str| options.iter().find(|o| o.kind.contains(kind)).cloned();

            match verdict {
                Verdict::Allow { rule } => {
                    if let Some(opt) = pick("allow") {
                        let _ = session.decide(&request_id, Some(opt.id)).await;
                        ingest(Event::PermissionDecided {
                            tool: title,
                            decision: "allow".into(),
                            by: format!("policy:{rule}"),
                        })
                        .await;
                        return;
                    }
                }
                Verdict::Deny { rule } => {
                    let _ = session
                        .decide(&request_id, pick("reject").map(|o| o.id))
                        .await;
                    ingest(Event::PermissionDecided {
                        tool: title,
                        decision: "deny".into(),
                        by: format!("policy:{rule}"),
                    })
                    .await;
                    return;
                }
                Verdict::Undecided => {}
            }

            // Nobody's rule covers it, so a human decides. Unlike an observed
            // session, this one can actually be answered from the inbox.
            ingest(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: Some(title),
                request_id: Some(request_id),
            })
            .await;
        }

        AcpEvent::Usage {
            cost_usd,
            context_tokens,
            context_window,
        } => {
            ingest(Event::ApiRequest {
                usage: ApiUsage {
                    model: None,
                    // The protocol reports a cumulative total; the reducer adds
                    // up per-request figures, so only the delta belongs here.
                    // Until the pump tracks the previous value, the cost is
                    // carried on the status sample instead.
                    cost_usd: 0.0,
                    input_tokens: context_tokens.unwrap_or(0),
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cache_creation_tokens: 0,
                },
            })
            .await;
            let _ = (cost_usd, context_window);
        }

        AcpEvent::TurnEnded { stop_reason } => {
            match stop_reason.as_str() {
                "refusal" => {
                    ingest(Event::TurnFailed {
                        message: "the agent refused to continue".into(),
                    })
                    .await
                }
                _ => ingest(Event::TurnEnded).await,
            }
            // The agent has stopped. If this run is doing a piece of work, that
            // is the moment its claim gets checked.
            crate::work::on_turn_ended(state, run).await;
        }

        AcpEvent::Ended { error } => match error {
            Some(e) => ingest(Event::TurnFailed { message: e }).await,
            None => ingest(Event::SessionEnded { reason: None }).await,
        },
    }
}

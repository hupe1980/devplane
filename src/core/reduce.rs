//! The reducer: `(Run, Event) -> Run`.
//!
//! Run state is a pure reduction over the event log, which is what makes the
//! board rebuildable after a restart and the state machine testable without a
//! provider. Nothing here does I/O or reads the clock beyond the timestamp the
//! envelope already carries.

use crate::core::event::{Event, EventEnvelope, WaitingFor};
use crate::core::run::{BlockedOn, Run, RunState, ToolCall};

/// How many recent tool calls a run keeps for its preview. Bounded because an
/// unbounded vector on a long session is a slow memory leak with a UI in front
/// of it.
const RECENT_TOOLS: usize = 20;

/// Applies one event to a run.
///
/// The ordering rule that matters: an event that proves the session is *doing*
/// something always beats a `Waiting` state, because a stale block is the one
/// error the inbox must never make — it puts a question in front of the human
/// that nobody is asking any more.
pub fn apply(run: &mut Run, env: &EventEnvelope) {
    run.last_event_at = env.at;
    if env.event.is_activity() {
        run.last_activity_at = env.at;
        // Anything the session says about itself makes it real rather than
        // merely present.
        run.reporting = true;
        // Anything the session does ends the quiet period, so the next silence
        // is reported as its own stall rather than suppressed by the last one.
        run.stall_noticed = false;
    }

    match &env.event {
        Event::SessionStarted {
            cwd,
            model,
            entrypoint,
            ..
        } => {
            run.cwd = cwd.clone();
            if model.is_some() {
                run.model = model.clone();
            }
            if entrypoint.is_some() {
                run.entrypoint = entrypoint.clone();
            }
            if run.totals.context_window.is_none() {
                run.totals.context_window = run
                    .model
                    .as_deref()
                    .and_then(crate::core::run::context_window_for_model);
            }
            run.state = RunState::Working;
            run.blocked_on = None;
        }

        Event::AgentSessionOpened { agent_session } => {
            // The agent is up and has named the conversation, so the run is
            // working rather than merely spawned — and it is now resumable,
            // which is the fact this event exists to make durable.
            run.agent_session = Some(agent_session.clone());
            run.state = RunState::Working;
        }

        Event::PromptSubmitted { .. } => {
            run.state = RunState::Working;
            run.blocked_on = None;
            // The prompt text is redacted and always will be, so the honest
            // line is that the agent has started and not yet done anything
            // visible. Leaving the last turn's tool call there is worse: the
            // board would show something finished as though it were running.
            run.summary = Some("thinking".into());
        }

        Event::ToolStarted { tool, input } => {
            run.state = RunState::Working;
            run.blocked_on = None;
            run.totals.tool_calls += 1;
            run.recent_tools.push(ToolCall {
                tool: tool.clone(),
                at: env.at,
                ok: None,
            });
            if run.recent_tools.len() > RECENT_TOOLS {
                run.recent_tools.remove(0);
            }
            run.summary = Some(summarise_tool(tool, input));
        }

        Event::ToolFinished { tool, ok, .. } => {
            run.state = RunState::Working;
            if let Some(last) = run
                .recent_tools
                .iter_mut()
                .rev()
                .find(|c| c.tool == *tool && c.ok.is_none())
            {
                last.ok = Some(*ok);
            }
            if !ok {
                run.totals.errors += 1;
            }
        }

        Event::PermissionDecided { tool, decision, by } => {
            // A decision unblocks whatever prompted it. This is what takes an
            // answered request out of the inbox. Waiting for the agent's next
            // move would leave it on screen until the agent did something —
            // and after a refusal, that may be never.
            if matches!(run.state, RunState::Waiting(WaitingFor::Permission)) {
                run.state = RunState::Working;
                run.blocked_on = None;
            }
            // A refusal is counted, because a refused agent does not stop. It
            // tries something else, and then something else again, and the
            // only outward sign of a rule that is too tight is a run that
            // costs more and finishes worse. `deny` is Vibeplane's own verdict
            // and Claude Code's auto-mode denial; `reject_once`/`reject_always`
            // are the protocol's spellings when a person refuses.
            if decision.starts_with("deny") || decision.starts_with("reject") {
                run.refusals += 1;
                run.last_refusal = Some(crate::core::run::Refusal {
                    tool: tool.clone(),
                    by: by.clone(),
                    at: env.at,
                });
            }
        }

        // The other half of `PermissionDecided`, and the reason an answered
        // question stops being asked: Claude Code fires `ElicitationResult`
        // when a person answers in their own terminal, so the item goes
        // wherever the decision was made.
        Event::QuestionAnswered { .. } => {
            if matches!(run.state, RunState::Waiting(WaitingFor::Question)) {
                run.state = RunState::Working;
                run.blocked_on = None;
            }
        }

        // Not a property of the run; recorded so `doctor` can say the settings
        // Vibeplane depends on were edited, and when.
        Event::ConfigChanged { .. } => {}

        // An observed session's own task list. A driven run gets its plan over
        // the protocol; this is the only equivalent for one we just watch.
        Event::TaskChanged { id, subject, done } => {
            let status = if *done { "completed" } else { "in_progress" };
            match run.plan.iter_mut().find(|s| s.content == *subject) {
                Some(step) => step.status = status.to_string(),
                None => run.plan.push(crate::core::run::PlanStep {
                    content: subject.clone(),
                    status: status.to_string(),
                }),
            }
            let _ = id;
        }

        Event::Blocked {
            waiting_for,
            message,
            request_id,
            options,
        } => {
            // `idle_prompt` means the turn ended and nobody has typed since. It
            // is not a question, so it must not outrank one: a run already
            // blocked on a permission stays blocked on it.
            if matches!(waiting_for, WaitingFor::Idle) {
                if !run.state.needs_human() {
                    run.state = RunState::Idle;
                    run.blocked_on = None;
                }
            } else {
                run.state = RunState::Waiting(waiting_for.clone());
                run.blocked_on = Some(BlockedOn {
                    waiting_for: waiting_for.clone(),
                    message: message.clone(),
                    request_id: request_id.clone(),
                    tool: last_pending_tool(run),
                    input: None,
                    options: options.clone(),
                    since: env.at,
                });
            }
        }

        Event::QuestionAsked { question, options } => {
            run.state = RunState::Waiting(WaitingFor::Question);
            run.blocked_on = Some(BlockedOn {
                waiting_for: WaitingFor::Question,
                message: Some(question.clone()),
                request_id: None,
                tool: None,
                input: None,
                options: options.clone(),
                since: env.at,
            });
            run.summary = Some(question.clone());
        }

        Event::TurnEnded => {
            run.totals.turns += 1;
            // Stop fires at the end of every turn, including the turn that
            // ends by asking a question. Keeping the block is the difference
            // between an inbox that works and one that empties itself.
            if !run.state.needs_human() {
                run.state = RunState::Idle;
                run.blocked_on = None;
            }
        }

        Event::TurnFailed { message } => {
            // A failed turn is a turn: it cost the same money and the same
            // wall-clock, and a bound that only counted the successful ones
            // would be loosest exactly where an agent is going in circles.
            run.totals.turns += 1;
            run.state = RunState::Failed;
            run.blocked_on = None;
            run.summary = Some(message.clone());
            run.totals.errors += 1;
        }

        Event::ApiRequest { usage } => {
            if usage.model.is_some() && run.model.is_none() {
                run.model = usage.model.clone();
            }
            run.totals.apply(usage);
        }

        Event::ApiError { error, .. } => {
            run.totals.errors += 1;
            run.summary = Some(error.clone());
        }

        Event::SubagentStarted { agent_id, kind } => {
            run.subagents.insert(agent_id.clone(), kind.clone());
        }
        Event::SubagentStopped { agent_id } => {
            run.subagents.remove(agent_id);
        }

        Event::CwdChanged { cwd } => run.cwd = cwd.clone(),

        Event::WorktreeEntered { path, branch } => {
            run.worktree = Some(path.clone());
            if branch.is_some() {
                run.branch = branch.clone();
            }
        }

        Event::Compacted => {
            // The window was just emptied; the old level would keep a false
            // `context_high` in the inbox until the next request.
            run.totals.last_context_tokens = 0;
            run.totals.reported_context_percent = None;
        }

        Event::ModelChanged { model } => {
            run.model = Some(model.clone());
            // Recomputed rather than left: a switch from a 200k model to a 1M
            // one changes what "83 % full" means, and the old denominator is
            // the wrong one from the moment the switch lands.
            run.totals.context_window = crate::core::run::context_window_for_model(model);
        }

        Event::SessionEnded { reason } => {
            // A session that failed and then ended is still a failure. The end
            // of a session says nothing about whether its work succeeded, so it
            // must not promote a failed run to completed and quietly drop it
            // out of the inbox.
            run.state = match (reason.as_deref(), &run.state) {
                (_, RunState::Failed) => RunState::Failed,
                (Some("error") | Some("failed"), _) => RunState::Failed,
                (Some("clear") | Some("logout"), _) => RunState::Stopped,
                _ => RunState::Completed,
            };
            run.blocked_on = None;
            run.subagents.clear();
        }

        Event::RosterSeen {
            kind,
            state,
            status,
            waiting_for,
            pid,
            name,
            entrypoint,
            started_at_ms,
        } => {
            // The session's own start time, not the moment we noticed it.
            if let Some(ms) = started_at_ms
                && let Some(t) = jiff::Timestamp::from_millisecond(*ms).ok()
            {
                run.started_at = t;
                if !run.reporting {
                    // Nothing has been heard from this session, so the last
                    // thing we know about it is that it started.
                    run.last_activity_at = t;
                }
            }
            if status.is_some() {
                // A status is the session reporting; that is what separates a
                // working session from a tab somebody left open on Tuesday.
                run.reporting = true;
            }
            if pid.is_some() {
                run.pid = *pid;
            }
            if name.is_some() {
                run.name = name.clone();
            }
            if entrypoint.is_some() && run.entrypoint.is_none() {
                run.entrypoint = entrypoint.clone();
            }

            if let Some(state) = state {
                // A background row: the provider's daemon owns this process, so
                // it knows better than we do what it is doing.
                run.state = match state.as_str() {
                    "working" => RunState::Working,
                    "blocked" => RunState::Waiting(match waiting_for.as_deref() {
                        Some(w) if w.contains("permission") => WaitingFor::Permission,
                        Some(w) if w.contains("input") || w.contains("question") => {
                            WaitingFor::Question
                        }
                        Some(other) => WaitingFor::Other(other.to_string()),
                        None => WaitingFor::Question,
                    }),
                    "done" => RunState::Idle,
                    "failed" => RunState::Failed,
                    "stopped" => RunState::Stopped,
                    _ => run.state.clone(),
                };
                if run.state.needs_human() && run.blocked_on.is_none() {
                    run.blocked_on = Some(BlockedOn {
                        waiting_for: match &run.state {
                            RunState::Waiting(w) => w.clone(),
                            _ => WaitingFor::Question,
                        },
                        message: waiting_for.clone(),
                        request_id: None,
                        tool: None,
                        input: None,
                        options: Vec::new(),
                        since: env.at,
                    });
                }
            } else if matches!(run.state, RunState::Starting) {
                // An interactive row for a session we have never heard from.
                // The roster is all we know, so it decides — but only until the
                // first hook arrives, which is always fresher than a poll.
                run.state = match status.as_deref() {
                    Some("busy") => RunState::Working,
                    Some(_) => RunState::Idle,
                    // No status at all: the session exists and that is the
                    // whole claim. Calling it working would be an invention.
                    None => RunState::Idle,
                };
            }
            let _ = kind;
        }

        Event::StatusSample {
            context_used_percent,
            rate_limit_five_hour,
            rate_limit_seven_day,
            session_name,
        } => {
            if context_used_percent.is_some() {
                run.totals.reported_context_percent = *context_used_percent;
            }
            // The window that is closest to its limit is the one that will stop
            // the work, so that is the one to carry. Both were being read off
            // the wire and thrown away here.
            let worst = [
                ("five_hour", rate_limit_five_hour),
                ("seven_day", rate_limit_seven_day),
            ]
            .into_iter()
            .filter_map(|(name, v)| v.map(|p| (name, p)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((window, percent)) = worst {
                run.totals.rate_limit_percent = Some(percent);
                run.totals.rate_limit_window = Some(window.to_string());
            }
            if session_name.is_some() && run.name.is_none() {
                run.name = session_name.clone();
            }
        }

        Event::Stalled { .. } => {
            run.stall_noticed = true;
        }

        Event::Lost { reason } => {
            run.state = RunState::Lost;
            run.summary = Some(reason.clone());
            run.blocked_on = None;
        }

        // Not about a run at all: it tells subscribers to re-read, and applying
        // it to a run would be inventing a change that did not happen.
        Event::Refresh => {}
    }
}

/// The tool a pending permission most likely refers to: the newest tool call
/// that has not reported a result. A permission prompt arrives between the
/// `PreToolUse` hook and the tool running, so the pending call is the subject.
fn last_pending_tool(run: &Run) -> Option<String> {
    run.recent_tools
        .iter()
        .rev()
        .find(|c| c.ok.is_none())
        .map(|c| c.tool.clone())
}

/// A one-line description of what a tool call is doing, for the board.
fn summarise_tool(tool: &str, input: &serde_json::Value) -> String {
    match crate::core::policy::rule_content(tool, input) {
        Some(d) => format!("{tool}: {}", crate::core::text::clip(&d, 80)),
        None => tool.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_turn_is_counted_once_however_many_usage_updates_it_streams() {
        // The bound `max_turns` rests on, and the reason it is not counted from
        // `api_requests`: the protocol streams *running totals*, so a driven
        // run reports usage several times inside one turn. Counting those would
        // make a 60-turn ceiling fire in a handful of real turns — on exactly
        // the runs a pipeline budget exists to bound.
        let mut r = run();
        for _ in 0..5 {
            apply(
                &mut r,
                &ev(Event::ApiRequest {
                    usage: ApiUsage {
                        cost_usd: 0.01,
                        ..Default::default()
                    },
                }),
            );
        }
        assert_eq!(r.totals.turns, 0, "usage is not a turn");
        apply(&mut r, &ev(Event::TurnEnded));
        assert_eq!(r.totals.turns, 1);

        // And a failed turn counts: it cost the same, and a bound that skipped
        // them would be loosest where an agent is going in circles.
        apply(
            &mut r,
            &ev(Event::TurnFailed {
                message: "boom".into(),
            }),
        );
        assert_eq!(r.totals.turns, 2);
    }

    use super::*;
    use crate::core::event::{ApiUsage, Source};
    use crate::core::ids::SessionId;
    use crate::core::run::RunMode;
    use serde_json::json;
    use std::path::PathBuf;

    fn run() -> Run {
        Run::new(
            SessionId::new("s1"),
            PathBuf::from("/repo"),
            RunMode::Observed,
            "claude",
        )
    }

    fn ev(e: Event) -> EventEnvelope {
        EventEnvelope::new(crate::core::ids::RunId::new("s1"), Source::Hook, e)
    }

    #[test]
    fn a_new_prompt_replaces_the_last_turns_summary() {
        // Otherwise the board shows the previous turn's last tool call as
        // though the agent were running it now.
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::ToolStarted {
                tool: "Bash".into(),
                input: json!({"command": "cargo test"}),
            }),
        );
        apply(&mut r, &ev(Event::PromptSubmitted { chars: 12 }));
        assert_eq!(r.summary.as_deref(), Some("thinking"));
    }

    #[test]
    fn tool_use_makes_a_run_working() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::ToolStarted {
                tool: "Bash".into(),
                input: json!({"command": "cargo test"}),
            }),
        );
        assert_eq!(r.state, RunState::Working);
        assert_eq!(r.summary.as_deref(), Some("Bash: cargo test"));
    }

    #[test]
    fn a_question_survives_the_turn_ending() {
        // Stop fires at the end of the turn that asked the question. If it
        // cleared the block, the item would vanish from the inbox before the
        // human ever saw it — the single worst bug this product can have.
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::QuestionAsked {
                question: "Keep the legacy route?".into(),
                options: vec!["Keep".into(), "Remove".into()],
            }),
        );
        apply(&mut r, &ev(Event::TurnEnded));
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Question));
        assert_eq!(r.blocked_on.as_ref().unwrap().options.len(), 2);
    }

    #[test]
    fn idle_notification_does_not_outrank_a_permission() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: Some("Bash wants to run".into()),
                request_id: None,
                options: vec![],
            }),
        );
        apply(
            &mut r,
            &ev(Event::Blocked {
                waiting_for: WaitingFor::Idle,
                message: None,
                request_id: None,
                options: vec![],
            }),
        );
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Permission));
    }

    #[test]
    fn a_new_prompt_clears_a_block() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: None,
                request_id: None,
                options: vec![],
            }),
        );
        apply(&mut r, &ev(Event::PromptSubmitted { chars: 12 }));
        assert_eq!(r.state, RunState::Working);
        assert!(r.blocked_on.is_none());
    }

    #[test]
    fn cost_accumulates_but_context_is_a_level() {
        let mut r = run();
        for input in [10_000u64, 25_000] {
            apply(
                &mut r,
                &ev(Event::ApiRequest {
                    usage: ApiUsage {
                        model: Some("claude-opus-5".into()),
                        cost_usd: 0.5,
                        input_tokens: input,
                        output_tokens: 100,
                        ..Default::default()
                    },
                }),
            );
        }
        assert_eq!(r.totals.cost_usd, 1.0);
        assert_eq!(r.totals.api_requests, 2);
        // The window holds what the last request sent, not the sum of both.
        assert_eq!(r.totals.last_context_tokens, 25_000);
        assert_eq!(r.totals.context_window, Some(200_000));
        assert!((r.totals.context_percent().unwrap() - 12.5).abs() < 0.01);
    }

    #[test]
    fn compaction_resets_the_gauge() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::ApiRequest {
                usage: ApiUsage {
                    model: Some("claude-opus-5".into()),
                    input_tokens: 190_000,
                    ..Default::default()
                },
            }),
        );
        assert!(r.totals.context_percent().unwrap() > 90.0);
        apply(&mut r, &ev(Event::Compacted));
        assert_eq!(r.totals.context_percent(), None);
    }

    #[test]
    fn a_background_row_is_authoritative() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::RosterSeen {
                kind: "background".into(),
                state: Some("blocked".into()),
                entrypoint: None,
                status: Some("waiting".into()),
                waiting_for: Some("permission prompt".into()),
                pid: Some(42),
                name: Some("flaky-test-fix".into()),
                started_at_ms: None,
            }),
        );
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Permission));
        assert_eq!(r.pid, Some(42));
        assert_eq!(r.name.as_deref(), Some("flaky-test-fix"));
    }

    fn roster(kind: &str, status: Option<&str>) -> Event {
        Event::RosterSeen {
            kind: kind.into(),
            state: None,
            status: status.map(Into::into),
            waiting_for: None,
            pid: Some(7),
            name: Some("repo-a1".into()),
            entrypoint: Some("claude-vscode".into()),
            started_at_ms: None,
        }
    }

    #[test]
    fn an_interactive_row_populates_the_board_before_any_hook() {
        // This is what makes `vibeplane ls` useful the moment it is installed:
        // the roster lists every live session, not only background ones.
        let mut r = run();
        apply(&mut r, &ev(roster("interactive", Some("busy"))));
        assert_eq!(r.state, RunState::Working);
        assert_eq!(r.entrypoint.as_deref(), Some("claude-vscode"));
        assert_eq!(r.name.as_deref(), Some("repo-a1"));
    }

    #[test]
    fn a_roster_poll_never_overwrites_what_a_hook_reported() {
        // The roster is a 2-second poll; a hook is the session speaking. A poll
        // that moved a blocked run back to "working" would empty the inbox.
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::QuestionAsked {
                question: "which one?".into(),
                options: vec![],
            }),
        );
        apply(&mut r, &ev(roster("interactive", Some("busy"))));
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Question));
        assert_eq!(r.pid, Some(7), "identity is still learned from the roster");
    }

    #[test]
    fn a_row_with_no_status_is_idle_not_working() {
        let mut r = run();
        apply(&mut r, &ev(roster("interactive", None)));
        assert_eq!(r.state, RunState::Idle);
    }

    #[test]
    fn answering_a_permission_clears_it_from_the_inbox() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: Some("rm -rf node_modules".into()),
                request_id: Some("req-1".into()),
                options: vec![],
            }),
        );
        assert!(r.state.needs_human());
        apply(
            &mut r,
            &ev(Event::PermissionDecided {
                tool: String::new(),
                decision: "deny".into(),
                by: "human".into(),
            }),
        );
        assert_eq!(r.state, RunState::Working);
        assert!(r.blocked_on.is_none(), "an answered request is answered");
    }

    #[test]
    fn a_stall_is_recorded_once_per_quiet_period() {
        let mut r = run();
        apply(&mut r, &ev(Event::Stalled { idle_seconds: 900 }));
        assert!(r.stall_noticed);
        // Anything the session does starts a new period.
        apply(
            &mut r,
            &ev(Event::ToolStarted {
                tool: "Bash".into(),
                input: json!({}),
            }),
        );
        assert!(!r.stall_noticed);
    }

    #[test]
    fn ending_a_session_does_not_erase_a_failure() {
        // StopFailure then SessionEnd is the ordinary shape of a failed run;
        // promoting it to "completed" would drop it out of the inbox.
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::TurnFailed {
                message: "API error".into(),
            }),
        );
        apply(
            &mut r,
            &ev(Event::SessionEnded {
                reason: Some("other".into()),
            }),
        );
        assert_eq!(r.state, RunState::Failed);
    }

    #[test]
    fn session_end_is_terminal() {
        let mut r = run();
        apply(&mut r, &ev(Event::SessionEnded { reason: None }));
        assert_eq!(r.state, RunState::Completed);
        assert!(!r.state.is_live());
    }

    #[test]
    fn a_refusal_is_counted_and_an_answer_is_not() {
        // The count is what makes a too-tight rule visible: the agent does not
        // stop when it is refused, so nothing else on any surface tells a
        // policy that is working apart from one that is quietly wrecking a run.
        let mut r = run();
        let refuse = |by: &str, decision: &str| Event::PermissionDecided {
            tool: "Bash".into(),
            decision: decision.into(),
            by: by.into(),
        };

        apply(&mut r, &ev(refuse("policy:Bash(git push *)", "deny")));
        assert_eq!(r.refusals, 1);
        assert_eq!(
            r.last_refusal.as_ref().map(|x| x.by.as_str()),
            Some("policy:Bash(git push *)")
        );

        // Claude Code's own auto mode, over the `PermissionDenied` hook.
        apply(&mut r, &ev(refuse("claude", "deny")));
        // And the protocol's spellings when a person says no.
        apply(&mut r, &ev(refuse("human", "reject_once")));
        apply(&mut r, &ev(refuse("human", "reject_always")));
        assert_eq!(r.refusals, 4, "every way of saying no counts as one");

        // An allow is not a refusal, and does not reset the count either: the
        // question is how much of this run is being spent on calls that never
        // happen, not whether the last one got through.
        apply(&mut r, &ev(refuse("policy:Read", "allow")));
        assert_eq!(r.refusals, 4);
    }
}

//! The reducer: `(Run, Event) -> Run`. A pure reduction over the event log, so
//! the board rebuilds after a restart and the state machine tests without a
//! provider. No I/O, and no clock beyond the envelope's timestamp.

use crate::core::event::{Event, EventEnvelope, WaitingFor};
use crate::core::run::{BlockedOn, Run, RunMode, RunState, ToolCall};

/// Recent tool calls a run keeps for its preview; bounded for long sessions.
const RECENT_TOOLS: usize = 20;

/// How a run's summary marks a call that was refused rather than run.
pub const REFUSED: &str = "refused: ";

/// Applies one event to a run. An event proving the session is *doing*
/// something beats a `Waiting` state: a stale block would show a question
/// nobody is asking any more.
pub fn apply(run: &mut Run, env: &EventEnvelope) {
    run.last_event_at = env.at;
    // Recorded first: which channel carries the session decides whose word counts.
    if matches!(env.source, crate::core::Source::Hook) {
        run.last_hook_at = Some(env.at);
    }
    if env.event.is_activity() {
        run.last_activity_at = env.at;
        run.reporting = true;
        // Later silence is then a fact, not an absence of wiring.
        run.activity_seen = true;
        // The next silence is its own stall, not suppressed by the last one.
        run.stall_noticed = false;
    }

    match &env.event {
        Event::SessionStarted {
            cwd,
            model,
            entrypoint,
            question_clock,
            clock_read,
            ..
        } => {
            run.cwd = cwd.clone();
            // Only where it was read, so a channel without the environment
            // cannot erase it.
            if *clock_read {
                run.question_clock = question_clock.clone();
                run.question_clock_read = Some(env.at);
            }
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
            // The agent named the conversation: working, and now resumable.
            run.agent_session = Some(agent_session.clone());
            run.state = RunState::Working;
        }

        Event::AgentProcessSpawned { pid } => {
            // Not a liveness signal (a driven run is live while its connection
            // is); kept so a killed host leaves enough to find its orphaned agent.
            run.pid = Some(*pid);
        }

        Event::PromptSubmitted { .. } => {
            run.state = RunState::Working;
            run.blocked_on = None;
            // The prompt is redacted; the last turn's tool call would read as
            // still running.
            run.summary = Some("thinking".into());
        }

        Event::ToolStarted {
            tool,
            input,
            agent_id,
            call_id,
            ..
        } => {
            // A second start for an in-flight call is the same call reported again.
            if call_id.is_some()
                && run
                    .recent_tools
                    .iter()
                    .any(|c| c.ok.is_none() && c.call_id == *call_id)
            {
                return;
            }
            // The agent moving on is the only evidence nobody answered, so it
            // records the question as abandoned. Only the main thread can move
            // on: a subagent's call arrives under the parent's session id while
            // the parent's question is legitimately still open, so it is counted
            // but never ends a question or clears a block.
            if agent_id.is_none() {
                abandon_open_question(
                    run,
                    env.at,
                    Some(summarise_tool(tool, input)),
                    crate::core::run::EndedBy::MovedOn,
                );
                run.state = RunState::Working;
                run.blocked_on = None;
            } else if !run.state.needs_human() {
                run.state = RunState::Working;
            }
            run.totals.tool_calls += 1;
            run.note_written(tool, input);
            run.recent_tools.push(ToolCall {
                tool: tool.clone(),
                at: env.at,
                ok: None,
                input: Some(input.clone()),
                call_id: call_id.clone(),
            });
            if run.recent_tools.len() > RECENT_TOOLS {
                run.recent_tools.remove(0);
            }
            run.summary = Some(summarise_tool(tool, input));
        }

        Event::ToolFinished {
            tool, ok, call_id, ..
        } => {
            run.state = RunState::Working;
            // By id where the source has one; by name otherwise (every hook).
            if let Some(last) = run.recent_tools.iter_mut().rev().find(|c| {
                c.ok.is_none()
                    && match call_id {
                        Some(id) => c.call_id.as_deref() == Some(id.as_str()),
                        None => c.tool == *tool,
                    }
            }) {
                last.ok = Some(*ok);
                // Input is kept only in flight, so a blocked run can say what
                // it is blocked on.
                last.input = None;
            }
            if !ok {
                run.totals.errors += 1;
            }
        }

        Event::PermissionModeSeen { mode } => {
            let seen = crate::core::run::PermissionMode::parse(mode);
            // Only a change moves the clock: this arrives on every prompt, and
            // the fact worth carrying is how long the mode has been true.
            if run.permission_mode.as_ref() != Some(&seen) {
                run.permission_mode = Some(seen);
                run.permission_mode_seen = Some(env.at);
            }
        }

        Event::AgentModeSeen { mode } => {
            // Only a change moves the clock, as for `PermissionModeSeen`.
            if run.agent_mode.as_deref() != Some(mode.as_str()) {
                run.agent_mode = Some(mode.clone());
                run.agent_mode_seen = Some(env.at);
            }
        }

        Event::PermissionDecided {
            tool,
            decision,
            by,
            reason,
            ..
        } => {
            // A decision takes the answered request out of the inbox now;
            // after a refusal the agent's next move may never come.
            if matches!(run.state, RunState::Waiting(WaitingFor::Permission)) {
                run.state = RunState::Working;
                run.blocked_on = None;
            }
            // Refusals are counted: a refused agent tries something else, so
            // this is the only sign of a rule that is too tight. `deny` is
            // Devplane's verdict or Claude Code's auto-mode denial; `reject_*`
            // is a person refusing over the protocol.
            if decision.starts_with("deny") || decision.starts_with("reject") {
                run.refusals += 1;
                run.last_refusal = Some(crate::core::run::Refusal {
                    tool: tool.clone(),
                    by: by.clone(),
                    reason: reason.clone(),
                    at: env.at,
                });
                // The call the summary names never ran.
                if run.recent_tools.last().is_some_and(|c| c.tool == *tool)
                    && let Some(said) = run.summary.as_mut()
                    && !said.starts_with(REFUSED)
                {
                    *said = format!("{REFUSED}{said}");
                }
            }
        }

        // Claude Code's `ElicitationResult`: answered in the terminal, so the
        // item leaves wherever the decision was made.
        Event::QuestionAnswered { .. } => {
            if matches!(run.state, RunState::Waiting(WaitingFor::Question)) {
                run.state = RunState::Working;
                run.blocked_on = None;
            }
        }

        // Not a property of the run; recorded for `doctor`.
        Event::ConfigChanged { .. } => {}

        // The protocol sends the whole list each time, so it replaces.
        Event::PlanUpdated { steps } => {
            run.plan = steps.clone();
        }

        // An observed session's task list, its only equivalent of a plan.
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
            ask,
            options,
            call,
            ..
        } => {
            // `idle_prompt` is not a question and must not outrank one.
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
                    ask: ask.clone().map(crate::core::AskId::new),
                    // The event's call first, else the run's in-flight call;
                    // the rule offered on a permission item needs the input.
                    tool: call
                        .as_ref()
                        .map(|c| c.tool.clone())
                        .or_else(|| last_pending_tool(run).map(|c| c.tool.clone())),
                    input: call
                        .as_ref()
                        .map(|c| c.input.clone())
                        .or_else(|| last_pending_tool(run).and_then(|c| c.input.clone())),
                    options: options.clone(),
                    form: None,
                    since: env.at,
                });
            }
        }

        Event::QuestionAsked {
            question,
            options,
            request_id,
            ask,
            form,
        } => {
            run.state = RunState::Waiting(WaitingFor::Question);
            run.blocked_on = Some(BlockedOn {
                waiting_for: WaitingFor::Question,
                message: Some(question.clone()),
                // Present only for a driven run; an observed session's dialog
                // belongs to the provider and cannot be answered from here.
                request_id: request_id.clone(),
                ask: ask.clone().map(crate::core::AskId::new),
                tool: None,
                input: None,
                options: options.clone(),
                form: form.clone(),
                since: env.at,
            });
            run.summary = Some(question.clone());
        }

        // A question that ended unanswered, stated by the vendor (OpenCode's
        // `question.rejected`). It must produce the same record as the
        // abandonment derived for Claude Code. The vendor schema carries no
        // cause, so none is invented.
        Event::QuestionEnded { .. } => {
            abandon_open_question(run, env.at, None, crate::core::run::EndedBy::VendorRejected);
            if matches!(run.state, RunState::Waiting(WaitingFor::Question)) {
                run.state = RunState::Working;
                run.blocked_on = None;
            }
        }

        // A listing fills gaps and may not contradict an observation; see
        // `Run::enrich_from_listing`.
        Event::SessionListed {
            agent_session,
            title,
        } => {
            run.enrich_from_listing(&crate::core::run::ListedSession {
                agent_session: agent_session.clone(),
                title: title.clone(),
            });
        }

        // The session's own job count off its `Stop` payload; same rule as the
        // roster's count, so the two channels cannot disagree.
        Event::JobsSeen { running } => apply_jobs(run, *running, env.at),

        Event::TurnEnded => {
            run.totals.turns += 1;
            // Bookkeeping may not revive an ended run: a late `Stop` can land
            // after `SessionEnd` on another channel. A genuinely resumed run
            // says so with work (a tool call, a question, a roster sighting).
            if !run.state.is_live() {
                return;
            }
            // A turn may end by asking a question; keep that block.
            if !run.state.needs_human() {
                run.state = RunState::Idle;
                run.blocked_on = None;
            }
        }

        Event::TurnFailed { message } => {
            // A failed turn still counts toward the turn bound.
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
            // The window was emptied; the old level would be a false `context_high`.
            run.totals.last_context_tokens = 0;
            run.totals.reported_context_percent = None;
        }

        Event::ModelChanged { model } => {
            run.model = Some(model.clone());
            // A new model may have a different window size.
            run.totals.context_window = crate::core::run::context_window_for_model(model);
        }

        Event::SessionEnded { reason } => {
            // One arm, so a session ending while asking both sets state and
            // records the abandonment. None of the vendor's documented reasons
            // means the work finished; each is a person ending the session.
            let next = match (reason.as_deref(), &run.state, run.mode) {
                (_, RunState::Failed, _) => RunState::Failed,
                (Some("error") | Some("failed"), _, _) => RunState::Failed,
                // The host tearing down (`driven.rs`) is not the work finishing.
                (Some("interrupted"), _, _) => RunState::Interrupted,
                // `stopped` is a person ending a driven run.
                (
                    Some("clear" | "resume" | "logout" | "prompt_input_exit" | "other" | "stopped"),
                    _,
                    _,
                ) => RunState::Stopped,
                // No known reason: a driven run's agent exiting on its own is
                // finishing; an unknown vendor reason is no grounds for `Completed`.
                (_, _, RunMode::Driven) => RunState::Completed,
                _ => RunState::Stopped,
            };

            // An open question ends unanswered, unless Devplane interrupted the
            // run: then it stays answerable through its `asks` row.
            if !matches!(next, RunState::Interrupted) {
                abandon_open_question(run, env.at, None, crate::core::run::EndedBy::SessionEnded);
            }

            run.state = next;
            // An interrupted run keeps its block: answerable after a restart.
            if !matches!(run.state, RunState::Interrupted) {
                run.blocked_on = None;
            }
            run.subagents.clear();
        }

        Event::RosterSeen {
            jobs,
            kind,
            state,
            status,
            waiting_for,
            pid,
            name,
            entrypoint,
            started_at_ms,
        } => {
            // The session's own start time, not when we noticed it.
            if let Some(ms) = started_at_ms
                && let Some(t) = jiff::Timestamp::from_millisecond(*ms).ok()
            {
                run.started_at = t;
                if !run.reporting {
                    // Nothing heard since it started.
                    run.last_activity_at = t;
                }
            }
            if status.is_some() {
                // A status separates a reporting session from an abandoned tab.
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
                // A background row: the provider owns this process and knows its state.
                run.state = match state.as_str() {
                    "working" => RunState::Working,
                    "blocked" => {
                        RunState::Waiting(WaitingFor::parse_roster(waiting_for.as_deref()))
                    }
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
                        ask: None,
                        tool: None,
                        input: None,
                        options: Vec::new(),
                        form: None,
                        since: env.at,
                    });
                }
            } else if run.last_hook_at.is_none() {
                // No hook has ever spoken about this session, so the roster is
                // the only channel and decides on every sample. (Testing
                // `Starting` instead would run once: the first sample clears it.)
                let reason = waiting_for.clone();
                match status.as_deref() {
                    Some("busy") => {
                        run.state = RunState::Working;
                        run.blocked_on = None;
                    }
                    Some("waiting") => {
                        let what = WaitingFor::parse_roster(reason.as_deref());
                        // Only additive: a hook's block is answerable, the
                        // roster's is not.
                        if !run.state.needs_human() {
                            run.state = RunState::Waiting(what.clone());
                            run.blocked_on = Some(BlockedOn {
                                waiting_for: what,
                                message: reason,
                                // Nothing to answer by: the person goes to the
                                // vendor's window.
                                request_id: None,
                                ask: None,
                                tool: None,
                                input: None,
                                options: Vec::new(),
                                form: None,
                                since: env.at,
                            });
                        }
                    }
                    Some("idle") => {
                        // `idle` is about token generation and is also true of a
                        // session waiting on its own jobs; the job count below
                        // is the finer signal.
                        if !run.state.waits_on_a_job() {
                            run.state = RunState::Idle;
                            run.blocked_on = None;
                        }
                    }
                    // Unknown or missing: the session exists, nothing more.
                    _ => {
                        if matches!(run.state, RunState::Starting) {
                            run.state = RunState::Idle;
                        }
                    }
                }
            } else if matches!(run.state, RunState::Starting) {
                // A hook has spoken but no turn yet; `Starting` never ages out,
                // so the roster settles it.
                run.state = match status.as_deref() {
                    Some("busy") => RunState::Working,
                    _ => RunState::Idle,
                };
            }

            // The process table tells a person's turn apart from a session
            // waiting on its own commands; see `apply_jobs`.
            if let Some(count) = jobs {
                apply_jobs(run, *count, env.at);
            }
            let _ = kind;
        }

        Event::StatusSample(s) => {
            if s.context_used_percent.is_some() {
                run.totals.reported_context_percent = s.context_used_percent;
            }
            // The stated window beats the one inferred from the model.
            if s.context_window_size.is_some() {
                run.totals.context_window = s.context_window_size;
            }
            // The window closest to its limit is the one that will stop the work.
            if let Some(w) = s
                .rate_limits
                .iter()
                .max_by(|a, b| a.used_percent.total_cmp(&b.used_percent))
            {
                run.totals.rate_limit_percent = Some(w.used_percent);
                run.totals.rate_limit_window = Some(w.name.clone());
                run.totals.rate_limit_resets_at = w.resets_at;
            }
            if s.session_name.is_some() && run.name.is_none() {
                run.name = s.session_name.clone();
            }
            // A sample never clears a fact; payloads omit blocks in some states.
            if s.model.is_some() {
                run.model = s.model.clone();
            }
            if s.claude_version.is_some() {
                run.claude_version = s.claude_version.clone();
            }
            // A session total (assigned), where telemetry sends per-request deltas.
            if let Some(c) = s.cost_usd {
                run.totals.cost_usd = c;
                run.totals.cost_reported = true;
            }
            if let Some(n) = s.lines_added {
                run.totals.lines_added = n;
            }
            if let Some(n) = s.lines_removed {
                run.totals.lines_removed = n;
            }
        }

        Event::Stalled { .. } => {
            run.stall_noticed = true;
        }

        // The tasks the agent was sent, whole; a later list replaces.
        Event::TasksSent { tasks } => {
            run.sent = Some(tasks.clone());
        }

        // What the spec said at this close; a resumed run's later close replaces it.
        Event::SpecObserved {
            fingerprint,
            ticked,
            changed_at,
        } => {
            run.observed = Some(crate::core::run::Observed {
                fingerprint: fingerprint.clone(),
                ticked: ticked.clone(),
                changed_at: *changed_at,
                at: env.at,
            });
        }

        Event::Lost { reason: _ } => {
            // The process is gone. Working or asking: lost mid-flight
            // (Critical). Otherwise it simply ended. Decided here so replay
            // agrees; `Lost` is not activity, and the summary keeps what it
            // was doing.
            run.state = match run.state {
                RunState::Working | RunState::Waiting(_) => RunState::Lost,
                _ => RunState::Stopped,
            };
            run.blocked_on = None;
        }

        // Tells subscribers to re-read; not about a run.
        Event::Refresh => {}
    }
}

/// Applies a checked count of the session's own running commands. Providers
/// report "idle" while a session waits on its own test suite, which is not a
/// person's turn. Decides only between idle and waiting on a job; a permission
/// or question block is untouched. Fed by the process table and `Stop`'s
/// `background_tasks`, so one rule keeps them in agreement.
fn apply_jobs(run: &mut Run, count: u32, at: jiff::Timestamp) {
    if run.state.needs_human() || !run.state.is_live() {
        return;
    }
    match count {
        0 => {
            // A checked zero clears only a block this rule set.
            if run.state.waits_on_a_job() {
                run.state = RunState::Idle;
                run.blocked_on = None;
            }
        }
        _ => {
            // `Working` is the provider's stronger claim; leave it.
            if !matches!(run.state, RunState::Working) {
                run.state = RunState::Waiting(WaitingFor::Job);
                run.blocked_on = Some(BlockedOn {
                    waiting_for: WaitingFor::Job,
                    message: Some(match count {
                        1 => "running a command it started".to_string(),
                        n => format!("running {n} commands it started"),
                    }),
                    request_id: None,
                    ask: None,
                    tool: None,
                    input: None,
                    options: Vec::new(),
                    form: None,
                    since: at,
                });
            }
        }
    }
}

/// The newest unfinished tool call: a permission prompt arrives between
/// `PreToolUse` and the tool running, so this is its subject.
fn last_pending_tool(run: &Run) -> Option<&ToolCall> {
    run.recent_tools.iter().rev().find(|c| c.ok.is_none())
}

/// How much of a tool call's content is kept in state. Not a display width:
/// it bounds storage (one heredoc); surfaces shorten for the screen.
pub const TOOL_CONTENT_KEPT: usize = 2_000;

/// What a tool call is doing, kept whole: a command is evidence a reviewer
/// must be able to paste. Bounded only by [`TOOL_CONTENT_KEPT`].
fn summarise_tool(tool: &str, input: &serde_json::Value) -> String {
    match crate::core::policy::rule_content(tool, input) {
        Some(d) => format!("{tool}: {}", crate::core::text::clip(&d, TOOL_CONTENT_KEPT)),
        None => tool.to_string(),
    }
}

/// Records that a question left the inbox unanswered. Only for questions not
/// answerable from here: a driven ask has its own durable `asks` row. Blind
/// spot: the vendor's auto-continue timer submits, so it looks exactly like an
/// answer; `devplane modes` covers that half.
fn abandon_open_question(
    run: &mut crate::core::Run,
    at: jiff::Timestamp,
    moved_on_to: Option<String>,
    ended_by: crate::core::run::EndedBy,
) {
    if !matches!(run.state, RunState::Waiting(WaitingFor::Question)) {
        return;
    }
    let Some(b) = run.blocked_on.as_ref() else {
        return;
    };
    // Answerable means driven, and a driven question is the `asks` row's.
    if b.request_id.is_some() || b.ask.is_some() {
        return;
    }
    let Some(question) = b.message.clone() else {
        return;
    };
    run.abandoned_questions
        .push(crate::core::run::AbandonedQuestion {
            question,
            options: b.options.clone(),
            asked_at: b.since,
            abandoned_at: at,
            moved_on_to,
            ended_by,
        });
    // Bounded; the dropped count is kept so no surface implies it shows all.
    while run.abandoned_questions.len() > ABANDONED_KEPT {
        run.abandoned_questions.remove(0);
        run.abandoned_dropped = run.abandoned_dropped.saturating_add(1);
    }
}

/// Abandoned questions one run keeps: a day's work, not a looping transcript.
const ABANDONED_KEPT: usize = 20;

/// Facts computed when read rather than materialised by a sweeper. Each takes
/// `now`, so it can be asked about any instant. Stalls count downtime (a fact
/// about the session); an ask's deadline does not (a fact about the person's
/// chance to answer, see [`clock_starts`](facts::clock_starts)).
pub mod facts {
    use crate::core::{Run, RunState};
    use jiff::Timestamp;

    /// Whether a run has been quiet longer than its project allows. Downtime
    /// counts; `stall_noticed` is deliberately not consulted.
    #[must_use]
    pub fn stalled(run: &Run, now: Timestamp, limit_seconds: i64) -> bool {
        run.activity_seen
            && matches!(run.state, RunState::Working)
            && (now - run.last_activity_at).get_seconds() > limit_seconds
    }

    /// Seconds a run has been quiet, as of `now`; one sweep measures every run
    /// against one instant.
    #[must_use]
    pub fn idle_for(run: &Run, now: Timestamp) -> i64 {
        (now - run.last_activity_at).get_seconds()
    }

    /// Whether a run's cost was never reported. Telemetry is pushed and never
    /// replayed, so a session that ran while nothing listened has no cost on
    /// record; unknown and zero must render differently.
    #[must_use]
    pub fn cost_is_unknown(run: &Run) -> bool {
        // `cost_reported`, not `api_requests`: the status line reports a
        // total without a request count.
        run.activity_seen && !run.totals.cost_reported
    }

    /// Whether a gate result still describes the tree in front of you:
    /// `verified` means the gate exited zero after the last change to the tree.
    /// Compares working-tree digests (untracked files included, see
    /// `git::tree_digest`); a clean tree is not required, since agents do not
    /// commit. A missing stamp or digest is false: unknown is not unchanged, and
    /// a gate whose tree moved while it ran is recorded without a digest.
    #[must_use]
    pub fn still_current(
        ran_at: Option<&crate::core::change::CommitStamp>,
        now: Option<&crate::core::change::CommitStamp>,
    ) -> bool {
        let (Some(then), Some(current)) = (ran_at, now) else {
            return false;
        };
        matches!((&then.tree, &current.tree), (Some(a), Some(b)) if a == b)
    }

    /// When an ask's deadline clock starts: when it was asked or when a person
    /// could next reach it, whichever is later. Deliberately excludes downtime.
    #[must_use]
    pub fn clock_starts(asked_at: Timestamp, reachable_since: Timestamp) -> Timestamp {
        asked_at.max(reachable_since)
    }

    /// One agent a quit would end, named the way a person would recognise it.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct Stopped {
        pub run: crate::core::RunId,
        pub agent: String,
        /// The worktree if it has one, else the checkout.
        pub at: String,
    }

    /// What quitting ends and what it leaves alone. A quit kills the agents
    /// this host started (its children); sessions in a person's own terminal
    /// are untouched. Unanswered questions stay open across the quit.
    #[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct Quitting {
        pub stops: Vec<Stopped>,
        /// Live sessions in somebody's own terminal.
        pub leaves: usize,
        /// Live runs delegated to a provider's own daemon (`claude --bg`).
        pub delegated: usize,
        pub unanswered: usize,
    }

    impl Quitting {
        /// The sentence a person consents to before a quit, shared by the
        /// terminal and the window.
        #[must_use]
        pub fn says(&self) -> String {
            let mut out = String::new();
            if self.stops.is_empty() {
                out.push_str(
                    "Quitting stops no agent: none of the live sessions is one Devplane started.\n",
                );
            } else {
                out.push_str(&format!(
                    "Quitting stops {} agent{} Devplane started:\n",
                    self.stops.len(),
                    if self.stops.len() == 1 { "" } else { "s" }
                ));
                for s in &self.stops {
                    out.push_str(&format!("  {}  {}  {}\n", s.run.as_str(), s.agent, s.at));
                }
            }
            // Said even when zero: "none" differs from "did not look".
            out.push_str(&format!(
                "It leaves {} session{} running in their own terminals untouched",
                self.leaves,
                if self.leaves == 1 { "" } else { "s" }
            ));
            if self.delegated > 0 {
                out.push_str(&format!(
                    ", and {} delegated to a provider's own daemon",
                    self.delegated
                ));
            }
            out.push_str(".\n");
            if self.unanswered > 0 {
                out.push_str(&format!(
                    "{} question{} waiting for you stay{} open: the agent's session is on disk, \
                     so they will be here when you start again.\n",
                    self.unanswered,
                    if self.unanswered == 1 { "" } else { "s" },
                    if self.unanswered == 1 { "s" } else { "" }
                ));
            }
            out
        }
    }

    /// What a quit would do, from the runs and the durable asks. `unanswered`
    /// counts open asks (permissions and questions), not blocked runs: the ask
    /// is the row that survives the quit.
    #[must_use]
    pub fn quitting<'a>(
        runs: impl Iterator<Item = &'a Run>,
        asks: &[crate::core::ask::Ask],
    ) -> Quitting {
        let mut q = Quitting {
            unanswered: asks.iter().filter(|a| a.is_open()).count(),
            ..Quitting::default()
        };
        for run in runs.filter(|r| r.state.is_live()) {
            match run.mode {
                crate::core::RunMode::Driven => q.stops.push(Stopped {
                    run: run.id.clone(),
                    agent: run.agent.clone(),
                    at: run
                        .worktree
                        .as_ref()
                        .unwrap_or(&run.cwd)
                        .display()
                        .to_string(),
                }),
                crate::core::RunMode::Observed => q.leaves += 1,
                crate::core::RunMode::Background => q.delegated += 1,
            }
        }
        // Stable order, so an unchanged world reads the same.
        q.stops.sort_by(|a, b| a.run.as_str().cmp(b.run.as_str()));
        q
    }
}

#[cfg(test)]
mod quitting_says {
    use super::facts::{Quitting, Stopped};

    #[test]
    fn the_sentence_names_runs_by_id_and_counts_the_rest() {
        let q = Quitting {
            stops: vec![
                Stopped {
                    run: crate::core::RunId::new("acp-1"),
                    agent: "claude".into(),
                    at: "/w/a".into(),
                },
                Stopped {
                    run: crate::core::RunId::new("acp-2"),
                    agent: "codex".into(),
                    at: "/w/b".into(),
                },
            ],
            leaves: 0,
            delegated: 1,
            unanswered: 3,
        };
        let s = q.says();
        assert!(
            s.starts_with("Quitting stops 2 agents Devplane started:\n"),
            "{s}"
        );
        assert!(
            s.contains("  acp-1  claude  /w/a\n") && s.contains("  acp-2  codex  /w/b\n"),
            "{s}"
        );
        assert!(
            s.contains("It leaves 0 sessions running"),
            "zero is said: {s}"
        );
        assert!(s.contains("1 delegated"), "{s}");
        assert!(s.contains("3 questions waiting for you stay open"), "{s}");

        let none = Quitting::default().says();
        assert!(none.starts_with("Quitting stops no agent"), "{none}");
        assert!(
            !none.contains("question"),
            "no questions, no sentence about them: {none}"
        );
    }
}

#[cfg(test)]
mod clipping {
    use super::*;

    fn bash(command: &str) -> serde_json::Value {
        serde_json::json!({ "command": command })
    }

    /// The reducer keeps the command; a surface shortens it.
    #[test]
    fn a_tool_call_keeps_the_whole_command_and_not_a_screens_worth() {
        // Long enough that every plausible display width has cut it.
        let command = format!("cargo test {} --quiet", "-".repeat(300));
        let summary = summarise_tool("Bash", &bash(&command));
        assert!(
            summary.contains(&command),
            "the command was shortened before any surface saw it: {summary}"
        );
        assert!(
            !summary.contains('…'),
            "the reducer put an ellipsis in state, so the rest of the command exists nowhere"
        );
        assert!(
            summary.chars().count() > 200,
            "a summary of {} characters is a display width in the wrong layer",
            summary.chars().count()
        );
    }

    /// Still bounded, generously: no single call grows a run's record without limit.
    #[test]
    fn a_heredoc_cannot_grow_a_runs_record_without_limit() {
        let huge = format!("bash -c 'cat <<EOF\n{}\nEOF'", "x".repeat(50_000));
        let summary = summarise_tool("Bash", &bash(&huge));
        assert!(
            summary.chars().count() <= TOOL_CONTENT_KEPT + 16,
            "{} characters went into state",
            summary.chars().count()
        );
        assert!(
            summary.ends_with('…'),
            "a bounded summary says it was bounded"
        );
    }

    #[test]
    fn a_tool_with_no_readable_content_is_still_named() {
        assert_eq!(summarise_tool("Task", &serde_json::json!({})), "Task");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_status_sample_carries_every_fact_only_it_knows() {
        use crate::core::event::{RateWindow, StatusSample};
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::StatusSample(StatusSample {
                context_used_percent: Some(42.0),
                context_window_size: Some(1_000_000),
                rate_limits: vec![
                    RateWindow {
                        name: "five_hour".into(),
                        used_percent: 88.0,
                        resets_at: Some(1_790_000_000),
                    },
                    RateWindow {
                        name: "seven_day".into(),
                        used_percent: 12.0,
                        resets_at: Some(1_790_500_000),
                    },
                ],
                session_name: Some("auth".into()),
                model: Some("claude-opus-5".into()),
                claude_version: Some("2.1.272".into()),
                cost_usd: Some(2.5),
                lines_added: Some(156),
                lines_removed: Some(23),
            })),
        );
        assert_eq!(r.totals.reported_context_percent, Some(42.0));
        assert_eq!(r.totals.context_window, Some(1_000_000));
        // The window closest to its limit.
        assert_eq!(r.totals.rate_limit_window.as_deref(), Some("five_hour"));
        assert_eq!(r.totals.rate_limit_percent, Some(88.0));
        assert_eq!(r.totals.rate_limit_resets_at, Some(1_790_000_000));
        assert_eq!(r.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(r.claude_version.as_deref(), Some("2.1.272"));
        assert_eq!(r.totals.cost_usd, 2.5);
        assert_eq!(r.totals.lines_added, 156);
        assert_eq!(r.totals.lines_removed, 23);
    }

    #[test]
    fn a_sample_that_omits_a_fact_does_not_forget_it() {
        // The payload drops blocks in some states; a board must not flicker.
        use crate::core::event::StatusSample;
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::StatusSample(StatusSample {
                model: Some("claude-opus-5".into()),
                claude_version: Some("2.1.272".into()),
                cost_usd: Some(2.5),
                context_window_size: Some(200_000),
                ..Default::default()
            })),
        );
        apply(&mut r, &ev(Event::StatusSample(StatusSample::default())));
        assert_eq!(r.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(r.claude_version.as_deref(), Some("2.1.272"));
        assert_eq!(r.totals.cost_usd, 2.5);
        assert_eq!(r.totals.context_window, Some(200_000));
    }

    #[test]
    fn the_status_line_cost_replaces_rather_than_accumulates() {
        // The status line sends a session total; adding it would double-count.
        use crate::core::event::StatusSample;
        let mut r = run();
        for c in [1.0, 2.0, 3.0] {
            apply(
                &mut r,
                &ev(Event::StatusSample(StatusSample {
                    cost_usd: Some(c),
                    ..Default::default()
                })),
            );
        }
        assert_eq!(r.totals.cost_usd, 3.0);
    }

    #[test]
    fn a_turn_is_counted_once_however_many_usage_updates_it_streams() {
        // `max_turns` rests on this: the protocol streams running totals
        // several times per turn, so usage updates are not turns.
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

        // A failed turn counts too.
        apply(
            &mut r,
            &ev(Event::TurnFailed {
                message: "boom".into(),
            }),
        );
        assert_eq!(r.totals.turns, 2);
    }

    use super::*;
    use crate::core::event::ApiUsage;
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

    /// An envelope with the event's real source channel: labelling a roster
    /// sample `Hook` would make the roster stand down.
    fn ev(e: Event) -> EventEnvelope {
        let source = e.test_source();
        EventEnvelope::new(crate::core::ids::RunId::new("s1"), source, e)
    }

    /// Hook events carry no tasks, so `sent` stays `None`, not an empty list.
    #[test]
    fn a_watched_run_reduced_from_hook_events_was_sent_nothing() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::SessionStarted {
                cwd: PathBuf::from("/repo"),
                source: None,
                model: None,
                entrypoint: None,
                question_clock: None,
                clock_read: false,
            }),
        );
        apply(
            &mut r,
            &ev(Event::tool_started("Read", serde_json::json!({}))),
        );
        apply(&mut r, &ev(Event::TurnEnded));
        assert_eq!(r.sent, None, "a watched run carries no task edges");
        assert_eq!(r.observed, None);
        assert!(r.sent_says().contains("watched"), "{}", r.sent_says());
    }

    /// The sent list replaces, and the last close is the one kept.
    #[test]
    fn a_driven_run_keeps_what_it_was_sent_and_its_last_close() {
        let mut r = Run::new(
            SessionId::new("s1"),
            PathBuf::from("/repo"),
            RunMode::Driven,
            "echo",
        );
        assert_eq!(r.sent_says(), "no tasks were sent to this run");
        let task = crate::core::spec::SentTask {
            path: "specs/1/tasks.md".into(),
            text: "T001 do it (FR-001)".into(),
            cites: vec!["FR-001".into()],
            line: 5,
            occurrence: 1,
        };
        apply(
            &mut r,
            &ev(Event::TasksSent {
                tasks: vec![task.clone()],
            }),
        );
        assert_eq!(r.sent.as_deref(), Some(&[task][..]));
        assert_eq!(r.sent_says(), "sent 1 task");

        let first = ev(Event::SpecObserved {
            fingerprint: Some("aaa".into()),
            ticked: vec![],
            changed_at: None,
        });
        apply(&mut r, &first);
        let second = ev(Event::SpecObserved {
            fingerprint: Some("bbb".into()),
            ticked: vec!["T001 do it (FR-001)".into()],
            changed_at: None,
        });
        apply(&mut r, &second);
        let seen = r.observed.as_ref().expect("observed");
        assert_eq!(seen.fingerprint.as_deref(), Some("bbb"));
        assert_eq!(seen.ticked, ["T001 do it (FR-001)"]);
        assert_eq!(
            seen.at, second.at,
            "the close is when the observation was taken"
        );
        // The host's own acts are not the run doing something.
        assert!(
            !Event::SpecObserved {
                fingerprint: None,
                ticked: vec![],
                changed_at: None
            }
            .is_activity()
        );
        assert!(!Event::TasksSent { tasks: vec![] }.is_activity());
    }

    fn asked(q: &str) -> Event {
        Event::QuestionAsked {
            question: q.into(),
            options: vec!["Keep".into(), "Remove".into()],
            ask: None,
            request_id: None,
            form: None,
        }
    }

    /// The next tool call after a watched question records that nobody
    /// answered, so "answered" and "gave up" differ in state.
    #[test]
    fn a_question_the_agent_moved_past_is_recorded_as_abandoned() {
        let mut r = run();
        apply(&mut r, &ev(asked("Keep the legacy /v1/login route?")));
        assert!(r.abandoned_questions.is_empty(), "still waiting");

        apply(
            &mut r,
            &ev(Event::tool_started(
                "Bash",
                serde_json::json!({"command": "cargo test"}),
            )),
        );
        assert_eq!(r.abandoned_questions.len(), 1, "nobody answered it");
        let q = &r.abandoned_questions[0];
        assert_eq!(q.question, "Keep the legacy /v1/login route?");
        assert_eq!(q.options.len(), 2, "what it was choosing between is kept");
        assert!(
            q.moved_on_to
                .as_deref()
                .is_some_and(|w| w.contains("cargo")),
            "what it did instead: {:?}",
            q.moved_on_to
        );
        assert!(!r.state.needs_human(), "the run moved on");
        assert_eq!(
            q.ended_by,
            crate::core::run::EndedBy::MovedOn,
            "and the record says this was derived from the agent moving on"
        );
    }

    /// What a run wrote comes from its editing calls, once per file, in first
    /// order; a read names nothing.
    #[test]
    fn an_edit_names_its_file_once_and_a_read_names_nothing() {
        let mut r = run();
        let call = |tool: &str, input: serde_json::Value| ev(Event::tool_started(tool, input));
        apply(
            &mut r,
            &call("Edit", serde_json::json!({"file_path": "src/a.rs"})),
        );
        apply(
            &mut r,
            &call("Read", serde_json::json!({"file_path": "src/b.rs"})),
        );
        apply(
            &mut r,
            &call("Edit", serde_json::json!({"file_path": "./src/a.rs"})),
        );
        apply(
            &mut r,
            &call("Write", serde_json::json!({"file_path": "/repo/src/c.rs"})),
        );
        apply(
            &mut r,
            &call(
                "NotebookEdit",
                serde_json::json!({"notebook_path": "nb.ipynb"}),
            ),
        );
        apply(
            &mut r,
            &call("Bash", serde_json::json!({"command": "echo > d.rs"})),
        );
        assert_eq!(
            r.wrote,
            ["src/a.rs", "src/c.rs", "nb.ipynb"],
            "an absolute path inside the cwd becomes relative; a read and a shell call add nothing"
        );
    }

    /// A subagent's tool call (parent's session id, `agent_id` set) is not the
    /// main thread moving on.
    #[test]
    fn a_subagents_tool_call_does_not_abandon_the_main_threads_question() {
        let mut r = run();
        apply(&mut r, &ev(asked("Keep the legacy /v1/login route?")));

        apply(
            &mut r,
            &ev(Event::ToolStarted {
                tool: "Grep".into(),
                input: serde_json::json!({"pattern": "login"}),
                server_source: None,
                agent_id: Some("agent-abc123".into()),
                call_id: None,
            }),
        );
        assert!(
            r.abandoned_questions.is_empty(),
            "a subagent's call was read as the parent moving past its question"
        );
        assert!(
            r.state.needs_human(),
            "the question is still open and the run still says so"
        );
        assert_eq!(r.totals.tool_calls, 1, "the call itself is still counted");

        // The main thread moving on still abandons it.
        apply(
            &mut r,
            &ev(Event::tool_started(
                "Bash",
                serde_json::json!({"command": "cargo test"}),
            )),
        );
        assert_eq!(r.abandoned_questions.len(), 1);
        assert_eq!(
            r.abandoned_questions[0].ended_by,
            crate::core::run::EndedBy::MovedOn
        );
    }

    /// Each way a question can end is recorded as that way: two are derived,
    /// one is the vendor's statement, and they are not equally strong claims.
    #[test]
    fn an_abandonment_records_whether_it_was_derived_or_asserted() {
        use crate::core::run::EndedBy;

        let mut ended = run();
        apply(&mut ended, &ev(asked("Keep it?")));
        apply(
            &mut ended,
            &ev(Event::SessionEnded {
                reason: Some("clear".into()),
            }),
        );
        assert_eq!(ended.abandoned_questions[0].ended_by, EndedBy::SessionEnded);

        let mut rejected = run();
        apply(&mut rejected, &ev(asked("Keep it?")));
        apply(
            &mut rejected,
            &ev(Event::QuestionEnded {
                request_id: "que_1".into(),
            }),
        );
        assert_eq!(
            rejected.abandoned_questions[0].ended_by,
            EndedBy::VendorRejected,
            "the vendor said so, and the record says the vendor said so"
        );

        // Serialised as the vendor-neutral word.
        let json = serde_json::to_value(&rejected.abandoned_questions[0]).unwrap();
        assert_eq!(json["ended_by"], "vendor_rejected");
    }

    /// `Stop`'s `background_tasks` go through the same rule as the process
    /// table: waiting on a job, and a checked zero clears only that block.
    #[test]
    fn a_stop_with_background_tasks_is_a_session_waiting_on_a_job() {
        let mut r = run();
        apply(&mut r, &ev(Event::TurnEnded));
        apply(&mut r, &ev(Event::JobsSeen { running: 1 }));
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Job));
        assert!(
            r.blocked_on
                .as_ref()
                .and_then(|b| b.message.as_deref())
                .is_some_and(|m| m.contains("running a command")),
            "{:?}",
            r.blocked_on
        );

        apply(&mut r, &ev(Event::TurnEnded));
        apply(&mut r, &ev(Event::JobsSeen { running: 0 }));
        assert_eq!(r.state, RunState::Idle, "a checked zero: the job finished");
        assert!(r.blocked_on.is_none());

        // A person being owed something outranks a job.
        let mut q = run();
        apply(&mut q, &ev(asked("Keep it?")));
        apply(&mut q, &ev(Event::JobsSeen { running: 2 }));
        assert_eq!(q.state, RunState::Waiting(WaitingFor::Question));
    }

    /// A session ending with a question open both records the abandonment and
    /// leaves `Waiting(Question)`.
    #[test]
    fn a_session_that_ends_while_asking_ends_and_records_it_once() {
        let mut r = run();
        apply(&mut r, &ev(asked("Keep the legacy route?")));
        apply(
            &mut r,
            &ev(Event::SessionEnded {
                reason: Some("clear".into()),
            }),
        );

        assert_eq!(r.state, RunState::Stopped, "the session ended");
        assert!(
            !r.state.needs_human(),
            "a dead session may not go on saying a person is owed something"
        );
        assert!(
            r.blocked_on.is_none(),
            "the block did not survive the session"
        );
        assert_eq!(
            r.abandoned_questions.len(),
            1,
            "and it is on the record exactly once"
        );
        assert_eq!(r.abandoned_questions[0].question, "Keep the legacy route?");
    }

    /// No documented `SessionEnd` reason means the work finished; each is a
    /// person ending the session.
    #[test]
    fn no_documented_reason_for_a_session_ending_claims_the_work_finished() {
        for reason in ["clear", "resume", "logout", "prompt_input_exit", "other"] {
            let mut r = run();
            apply(
                &mut r,
                &ev(Event::tool_started("Bash", json!({"command": "ls"}))),
            );
            apply(
                &mut r,
                &ev(Event::SessionEnded {
                    reason: Some(reason.into()),
                }),
            );
            assert_eq!(
                r.state,
                RunState::Stopped,
                "`{reason}` is a person ending a session, never a claim about the work"
            );
        }
    }

    /// An unknown reason on an observed session claims least: not completion.
    #[test]
    fn an_unknown_reason_does_not_become_completed_for_a_watched_session() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::SessionEnded {
                reason: Some("teleported".into()),
            }),
        );
        assert_eq!(r.state, RunState::Stopped);

        let mut none = run();
        apply(&mut none, &ev(Event::SessionEnded { reason: None }));
        assert_eq!(none.state, RunState::Stopped, "and neither does no reason");
    }

    /// For a driven run, the connection closing without a reason does mean the
    /// work ended; `driven.rs` sends `interrupted` when Devplane ended it.
    #[test]
    fn a_driven_run_whose_agent_finished_is_completed() {
        let mut r = Run::new(
            SessionId::new("d1"),
            PathBuf::from("/repo"),
            RunMode::Driven,
            "claude",
        );
        apply(&mut r, &ev(Event::SessionEnded { reason: None }));
        assert_eq!(r.state, RunState::Completed);
    }

    /// A late `Stop` after `SessionEnd` may not revive an ended run.
    #[test]
    fn a_turn_ending_late_does_not_revive_a_run_that_is_over() {
        for over in [
            RunState::Completed,
            RunState::Stopped,
            RunState::Failed,
            RunState::Interrupted,
            RunState::Lost,
        ] {
            let mut r = run();
            r.state = over.clone();
            apply(&mut r, &ev(Event::TurnEnded));
            assert_eq!(
                r.state, over,
                "a late turn ending moved a run that was over"
            );
            assert!(!r.state.is_live(), "and put it back in the working set");
            assert_eq!(r.totals.turns, 1, "the turn still happened and still cost");
        }
    }

    /// A genuinely resumed run (`/resume`, same id) says so with work, and
    /// that does revive it.
    #[test]
    fn work_arriving_after_a_session_ended_revives_the_run() {
        let mut r = run();
        r.state = RunState::Stopped;
        apply(
            &mut r,
            &ev(Event::tool_started("Bash", json!({"command": "ls"}))),
        );
        assert_eq!(r.state, RunState::Working, "the session came back");
    }

    /// A `session/list` result only fills gaps; an observation outranks a
    /// catalogue, and `SessionInfo` carries no state.
    #[test]
    fn a_session_listing_fills_gaps_and_never_moves_a_run() {
        use crate::core::run::ListedSession;

        let listed = |title: &str| Event::SessionListed {
            agent_session: "agent-1".into(),
            title: Some(title.into()),
        };

        // The listing leaves every state where it was.
        for state in [
            RunState::Starting,
            RunState::Working,
            RunState::Waiting(WaitingFor::Question),
            RunState::Idle,
            RunState::Completed,
            RunState::Failed,
            RunState::Stopped,
            RunState::Interrupted,
            RunState::Lost,
        ] {
            let mut r = run();
            r.agent_session = Some("agent-1".into());
            r.state = state.clone();
            apply(&mut r, &ev(listed("a name")));
            assert_eq!(r.state, state, "a catalogue moved an observed run");
        }

        // A gap is filled…
        let mut r = run();
        r.agent_session = Some("agent-1".into());
        apply(&mut r, &ev(listed("fix the flaky login test")));
        assert_eq!(r.name.as_deref(), Some("fix the flaky login test"));

        // …but an observed name is never overwritten.
        apply(&mut r, &ev(listed("something else")));
        assert_eq!(r.name.as_deref(), Some("fix the flaky login test"));

        // A listing about a different session says nothing about this one.
        let mut other = run();
        other.agent_session = Some("agent-2".into());
        apply(&mut other, &ev(listed("not yours")));
        assert_eq!(other.name, None);

        // A whitespace-only name is refused.
        let mut blank = run();
        blank.agent_session = Some("agent-1".into());
        assert!(!blank.enrich_from_listing(&ListedSession {
            agent_session: "agent-1".into(),
            title: Some("   ".into()),
        }));
        assert_eq!(blank.name, None);
    }

    /// A completed question (`PostToolUse`, i.e. `ToolFinished`) is answered,
    /// not abandoned.
    #[test]
    fn a_question_that_finished_is_not_abandoned() {
        let mut r = run();
        apply(&mut r, &ev(asked("Which framework?")));
        apply(
            &mut r,
            &ev(Event::ToolFinished {
                tool: "AskUserQuestion".into(),
                ok: true,
                duration_ms: None,
                call_id: None,
            }),
        );
        assert!(r.abandoned_questions.is_empty(), "it completed");
    }

    /// Time alone never abandons a question; it takes a later event.
    #[test]
    fn an_outstanding_question_is_never_abandoned_by_time_alone() {
        let mut r = run();
        apply(&mut r, &ev(asked("Which framework?")));
        let mut later = ev(Event::StatusSample(crate::core::event::StatusSample {
            context_used_percent: Some(10.0),
            ..Default::default()
        }));
        later.at = jiff::Timestamp::now() + jiff::SignedDuration::from_hours(48);
        apply(&mut r, &later);
        assert!(r.abandoned_questions.is_empty(), "no clock ends a question");
        assert!(r.state.needs_human(), "it is still waiting");
    }

    /// A driven question belongs to its `asks` row and is not recorded twice.
    #[test]
    fn a_driven_question_is_not_recorded_here() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::QuestionAsked {
                question: "Which framework?".into(),
                options: vec![],
                ask: Some("a-1".into()),
                request_id: Some("req-1".into()),
                form: None,
            }),
        );
        apply(
            &mut r,
            &ev(Event::tool_started(
                "Bash",
                serde_json::json!({"command": "ls"}),
            )),
        );
        assert!(
            r.abandoned_questions.is_empty(),
            "the durable ask owns this one"
        );
    }

    /// Bounded: the oldest go and the dropped count is kept.
    #[test]
    fn the_record_is_bounded_and_says_how_much_it_dropped() {
        let mut r = run();
        for i in 0..ABANDONED_KEPT + 3 {
            apply(&mut r, &ev(asked(&format!("question {i}"))));
            apply(
                &mut r,
                &ev(Event::tool_started(
                    "Bash",
                    serde_json::json!({"command": "ls"}),
                )),
            );
        }
        assert_eq!(r.abandoned_questions.len(), ABANDONED_KEPT);
        assert_eq!(r.abandoned_dropped, 3);
        assert_eq!(
            r.abandoned_questions.last().map(|q| q.question.as_str()),
            Some(format!("question {}", ABANDONED_KEPT + 2).as_str()),
            "the newest is kept"
        );
    }

    /// Only a change of mode moves the "since" clock.
    #[test]
    fn a_repeated_mode_does_not_reset_how_long_it_has_been_true() {
        use crate::core::run::PermissionMode;
        let mut r = run();
        assert_eq!(r.permission_mode, None, "unknown until a carrier arrives");

        apply(
            &mut r,
            &ev(Event::PermissionModeSeen {
                mode: "auto".into(),
            }),
        );
        let first = r.permission_mode_seen.expect("stamped on first sight");
        assert_eq!(r.permission_mode, Some(PermissionMode::Auto));

        // The same mode again, later: nothing moves.
        let mut again = ev(Event::PermissionModeSeen {
            mode: "auto".into(),
        });
        again.at = first + jiff::SignedDuration::from_secs(600);
        apply(&mut r, &again);
        assert_eq!(
            r.permission_mode_seen,
            Some(first),
            "an unchanged mode reset the clock, so the age is always zero"
        );

        // A different mode does.
        let mut switched = ev(Event::PermissionModeSeen {
            mode: "default".into(),
        });
        switched.at = first + jiff::SignedDuration::from_secs(1200);
        apply(&mut r, &switched);
        assert_eq!(r.permission_mode, Some(PermissionMode::Default));
        assert_eq!(r.permission_mode_seen, Some(switched.at));
    }

    /// A mode this build does not know is carried, not flattened.
    #[test]
    fn an_unknown_mode_reaches_the_run_intact() {
        use crate::core::run::PermissionMode;
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::PermissionModeSeen {
                mode: "somethingNew".into(),
            }),
        );
        assert_eq!(
            r.permission_mode,
            Some(PermissionMode::Unrecognised("somethingNew".into()))
        );
        assert_eq!(
            r.permission_mode.as_ref().unwrap().asks_a_person(),
            None,
            "and it still refuses to claim anybody is supervising it"
        );
    }

    #[test]
    fn a_new_prompt_replaces_the_last_turns_summary() {
        // Not the previous turn's last tool call.
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::tool_started(
                "Bash",
                json!({"command": "cargo test"}),
            )),
        );
        apply(&mut r, &ev(Event::PromptSubmitted { chars: 12 }));
        assert_eq!(r.summary.as_deref(), Some("thinking"));
    }

    #[test]
    fn tool_use_makes_a_run_working() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::tool_started(
                "Bash",
                json!({"command": "cargo test"}),
            )),
        );
        assert_eq!(r.state, RunState::Working);
        assert_eq!(r.summary.as_deref(), Some("Bash: cargo test"));
    }

    #[test]
    fn a_question_survives_the_turn_ending() {
        // Stop ends the turn that asked; clearing the block would empty the inbox.
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::QuestionAsked {
                question: "Keep the legacy route?".into(),
                options: vec!["Keep".into(), "Remove".into()],
                ask: None,
                request_id: None,
                form: None,
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
                ask: None,
                options: vec![],
                call: None,
                context: None,
            }),
        );
        apply(
            &mut r,
            &ev(Event::Blocked {
                waiting_for: WaitingFor::Idle,
                message: None,
                request_id: None,
                ask: None,
                options: vec![],
                call: None,
                context: None,
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
                ask: None,
                options: vec![],
                call: None,
                context: None,
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
                jobs: None,
            }),
        );
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Permission));
        assert_eq!(r.pid, Some(42));
        assert_eq!(r.name.as_deref(), Some("flaky-test-fix"));
    }

    fn roster(kind: &str, status: Option<&str>) -> Event {
        roster_with_jobs(kind, status, None)
    }

    fn roster_with_jobs(kind: &str, status: Option<&str>, jobs: Option<u32>) -> Event {
        Event::RosterSeen {
            kind: kind.into(),
            state: None,
            status: status.map(Into::into),
            waiting_for: None,
            pid: Some(7),
            name: Some("repo-a1".into()),
            entrypoint: Some("claude-vscode".into()),
            started_at_ms: None,
            jobs,
        }
    }

    /// For a session no hook speaks for, the roster decides on every sample,
    /// not just the first.
    #[test]
    fn a_session_with_no_hooks_is_not_frozen_at_its_first_sighting() {
        let mut r = run();
        apply(&mut r, &ev(roster("interactive", Some("busy"))));
        assert_eq!(r.state, RunState::Working);

        apply(&mut r, &ev(roster("interactive", Some("idle"))));
        assert_eq!(
            r.state,
            RunState::Idle,
            "the turn ended and the board says so"
        );

        apply(&mut r, &ev(roster("interactive", Some("busy"))));
        assert_eq!(r.state, RunState::Working, "and it starts again");
    }

    /// Roster `waiting` (with `waitingFor`) puts an unconnected session's own
    /// dialog in the inbox.
    #[test]
    fn a_permission_the_roster_reports_reaches_the_inbox() {
        for (reported, expected) in [
            ("permission prompt", WaitingFor::Permission),
            ("input needed", WaitingFor::Question),
            (
                "sandbox request",
                WaitingFor::Other("sandbox request".into()),
            ),
            ("worker request", WaitingFor::Other("worker request".into())),
            ("dialog open", WaitingFor::Other("dialog open".into())),
        ] {
            let mut r = run();
            apply(
                &mut r,
                &ev(Event::RosterSeen {
                    kind: "interactive".into(),
                    state: None,
                    status: Some("waiting".into()),
                    waiting_for: Some(reported.into()),
                    pid: Some(7),
                    name: None,
                    entrypoint: None,
                    started_at_ms: None,
                    jobs: None,
                }),
            );
            assert_eq!(r.state, RunState::Waiting(expected), "for {reported:?}");
            assert!(
                r.state.needs_human(),
                "{reported:?} is somebody being waited on"
            );
            assert_eq!(
                r.blocked_on.as_ref().unwrap().message.as_deref(),
                Some(reported),
                "and the vendor's own words are kept"
            );
            // Nothing to answer by: no button that cannot reach the session.
            assert!(r.blocked_on.as_ref().unwrap().request_id.is_none());

            // Cleared when the vendor says the wait ended.
            apply(&mut r, &ev(roster("interactive", Some("busy"))));
            assert_eq!(r.state, RunState::Working);
            assert!(r.blocked_on.is_none());
        }
    }

    /// The roster never downgrades an answerable block a hook recorded.
    #[test]
    fn the_roster_does_not_replace_a_block_a_hook_can_answer() {
        let mut r = run();
        let id = r.id.clone();
        apply(
            &mut r,
            &EventEnvelope::new(
                id,
                crate::core::Source::Hook,
                Event::QuestionAsked {
                    question: "Which database?".into(),
                    options: vec!["postgres".into(), "sqlite".into()],
                    request_id: Some("req-1".into()),
                    ask: Some("ask-1".into()),
                    form: None,
                },
            ),
        );
        apply(
            &mut r,
            &ev(Event::RosterSeen {
                kind: "interactive".into(),
                state: None,
                status: Some("waiting".into()),
                waiting_for: Some("input needed".into()),
                pid: Some(7),
                name: None,
                entrypoint: None,
                started_at_ms: None,
                jobs: None,
            }),
        );
        let b = r.blocked_on.as_ref().expect("still blocked");
        assert_eq!(
            b.request_id.as_deref(),
            Some("req-1"),
            "the answerable one survives"
        );
        assert_eq!(b.options.len(), 2, "and so do its options");

        // Once a hook has spoken, the roster stops deciding the state.
        apply(&mut r, &ev(roster("interactive", Some("busy"))));
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Question));
    }

    /// A roster `idle` session running its own background suite is waiting on
    /// a job, not on the person.
    #[test]
    fn a_session_waiting_on_its_own_test_suite_is_not_waiting_for_a_person() {
        let mut r = run();
        apply(
            &mut r,
            &ev(roster_with_jobs("interactive", Some("idle"), Some(1))),
        );

        assert_eq!(r.state, RunState::Waiting(WaitingFor::Job));
        assert!(!r.state.needs_human(), "nothing is owed by anybody");
        assert!(r.state.waits_on_a_job());
        assert!(
            r.blocked_on.as_ref().unwrap().message.as_deref()
                == Some("running a command it started"),
            "and the row says what it is waiting on"
        );

        // It stays on the board past the six-hour idle age-out.
        r.last_activity_at -= jiff::Span::new().hours(9);
        assert!(r.is_active(), "a running job never ages out");
    }

    /// A checked zero ends the block, so the state is never sticky.
    #[test]
    fn the_job_finishing_hands_the_session_back_to_the_person() {
        let mut r = run();
        apply(
            &mut r,
            &ev(roster_with_jobs("interactive", Some("idle"), Some(2))),
        );
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Job));
        assert_eq!(
            r.blocked_on.as_ref().unwrap().message.as_deref(),
            Some("running 2 commands it started")
        );

        apply(
            &mut r,
            &ev(roster_with_jobs("interactive", Some("idle"), Some(0))),
        );
        assert_eq!(r.state, RunState::Idle);
        assert!(r.blocked_on.is_none());
    }

    /// `None` is not zero: an unreadable process table changes nothing.
    #[test]
    fn a_poll_that_did_not_look_changes_nothing() {
        let mut r = run();
        apply(
            &mut r,
            &ev(roster_with_jobs("interactive", Some("idle"), Some(1))),
        );
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Job));
        apply(
            &mut r,
            &ev(roster_with_jobs("interactive", Some("idle"), None)),
        );
        assert_eq!(
            r.state,
            RunState::Waiting(WaitingFor::Job),
            "an unchecked poll is not evidence the job ended"
        );
    }

    /// A permission a person must clear outranks a running command.
    #[test]
    fn a_running_job_never_hides_a_question_somebody_is_owed() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::Blocked {
                waiting_for: WaitingFor::Permission,
                message: Some("Bash(rm -rf /)".into()),
                request_id: Some("r1".into()),
                ask: None,
                options: Vec::new(),
                call: None,
                context: None,
            }),
        );
        apply(
            &mut r,
            &ev(roster_with_jobs("interactive", Some("idle"), Some(3))),
        );
        assert_eq!(
            r.state,
            RunState::Waiting(WaitingFor::Permission),
            "a running command does not answer a permission"
        );
        assert!(r.state.needs_human());
    }

    #[test]
    fn an_interactive_row_populates_the_board_before_any_hook() {
        // The roster lists every live session, not only background ones.
        let mut r = run();
        apply(&mut r, &ev(roster("interactive", Some("busy"))));
        assert_eq!(r.state, RunState::Working);
        assert_eq!(r.entrypoint.as_deref(), Some("claude-vscode"));
        assert_eq!(r.name.as_deref(), Some("repo-a1"));
    }

    #[test]
    fn a_roster_poll_never_overwrites_what_a_hook_reported() {
        // A roster poll must not move a hook-blocked run back to working.
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::QuestionAsked {
                question: "which one?".into(),
                options: vec![],
                ask: None,
                request_id: None,
                form: None,
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
                ask: None,
                options: vec![],
                call: None,
                context: None,
            }),
        );
        assert!(r.state.needs_human());
        apply(
            &mut r,
            &ev(Event::PermissionDecided {
                tool: String::new(),
                decision: "deny".into(),
                by: "human".into(),
                reason: None,
                context: None,
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
        apply(&mut r, &ev(Event::tool_started("Bash", json!({}))));
        assert!(!r.stall_noticed);
    }

    #[test]
    fn ending_a_session_does_not_erase_a_failure() {
        // StopFailure then SessionEnd stays failed, in the inbox.
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

    /// Asserts the property (the run leaves the working set), not a state word.
    #[test]
    fn session_end_is_terminal() {
        let mut r = run();
        apply(&mut r, &ev(Event::SessionEnded { reason: None }));
        assert!(!r.state.is_live(), "the session is over");
        assert!(!r.state.needs_human(), "and nobody is owed anything by it");
    }

    #[test]
    fn a_refusal_is_counted_and_an_answer_is_not() {
        // The count is the only sign of a too-tight rule.
        let mut r = run();
        let refuse = |by: &str, decision: &str| Event::PermissionDecided {
            tool: "Bash".into(),
            decision: decision.into(),
            by: by.into(),
            reason: None,
            context: None,
        };

        apply(&mut r, &ev(refuse("policy:Bash(git push *)", "deny")));
        assert_eq!(r.refusals, 1);
        assert_eq!(
            r.last_refusal.as_ref().map(|x| x.by.as_str()),
            Some("policy:Bash(git push *)")
        );

        // Claude Code's own auto mode, over the `PermissionDenied` hook.
        apply(&mut r, &ev(refuse("claude", "deny")));
        // The protocol's spellings when a person says no.
        apply(&mut r, &ev(refuse("human", "reject_once")));
        apply(&mut r, &ev(refuse("human", "reject_always")));
        assert_eq!(r.refusals, 4, "every way of saying no counts as one");

        // An allow neither counts nor resets the count.
        apply(&mut r, &ev(refuse("policy:Read", "allow")));
        assert_eq!(r.refusals, 4);
    }

    /// A refused call's summary says it was refused, once.
    #[test]
    fn a_refused_call_is_marked_in_the_summary() {
        let mut r = run();
        apply(
            &mut r,
            &ev(Event::tool_started(
                "Read",
                json!({"file_path": "/repo/.env"}),
            )),
        );
        let said = r.summary.clone().unwrap();
        let deny = Event::PermissionDecided {
            tool: "Read".into(),
            decision: "deny".into(),
            by: "policy:Read(.env)".into(),
            reason: None,
            context: None,
        };
        apply(&mut r, &ev(deny.clone()));
        assert_eq!(
            r.summary.as_deref(),
            Some(format!("{REFUSED}{said}").as_str())
        );
        apply(&mut r, &ev(deny));
        assert_eq!(r.summary.unwrap().matches(REFUSED).count(), 1);
    }
}

#[cfg(test)]
mod published_endings {
    use super::*;
    use crate::core::event::{EventEnvelope, Source};
    use crate::core::ids::RunId;

    fn hint() -> crate::core::world::RunHint {
        crate::core::world::RunHint::default()
    }

    fn ask(run: &str) -> EventEnvelope {
        EventEnvelope::new(
            RunId::new(run),
            Source::Hook,
            Event::QuestionAsked {
                question: "Keep the legacy route?".into(),
                options: Vec::new(),
                request_id: None,
                ask: None,
                form: None,
            },
        )
    }

    /// A vendor-published ending (OpenCode) records the same thing as one
    /// derived (Claude Code: asked, then another tool call).
    #[test]
    fn a_published_ending_records_an_abandonment_like_a_derived_one() {
        let mut w = crate::core::World::default();
        w.apply(ask("ses_a"), hint());
        let r = w.run(&RunId::new("ses_a")).expect("the ask made a run");
        assert!(r.state.needs_human(), "the ask did not block the run");
        assert!(r.abandoned_questions.is_empty(), "still waiting");

        w.apply(
            EventEnvelope::new(
                RunId::new("ses_a"),
                Source::Hook,
                Event::QuestionEnded {
                    request_id: "que_7".into(),
                },
            ),
            hint(),
        );
        let r = w.run(&RunId::new("ses_a")).expect("the run is still there");
        assert_eq!(
            r.abandoned_questions.len(),
            1,
            "a question the vendor said was rejected left no record"
        );
        assert_eq!(
            r.abandoned_questions[0].question, "Keep the legacy route?",
            "the record lost the question"
        );
        assert!(
            !r.state.needs_human(),
            "the run is still waiting on a person"
        );
    }

    /// An answer is not an abandonment.
    #[test]
    fn an_answered_question_leaves_no_abandonment() {
        let mut w = crate::core::World::default();
        w.apply(ask("ses_b"), hint());
        w.apply(
            EventEnvelope::new(
                RunId::new("ses_b"),
                Source::Hook,
                Event::QuestionAnswered {
                    action: "accept".into(),
                },
            ),
            hint(),
        );
        let r = w.run(&RunId::new("ses_b")).expect("the run is there");
        assert!(
            r.abandoned_questions.is_empty(),
            "a question somebody answered was recorded as abandoned"
        );
        assert!(!r.state.needs_human());
    }

    /// An ending for a question this host never saw invents no question text.
    #[test]
    fn an_ending_with_no_question_behind_it_invents_no_text() {
        let mut w = crate::core::World::default();
        w.apply(
            EventEnvelope::new(
                RunId::new("ses_c"),
                Source::Hook,
                Event::QuestionEnded {
                    request_id: "que_9".into(),
                },
            ),
            hint(),
        );
        let r = w.run(&RunId::new("ses_c")).expect("the run is there");
        assert!(
            r.abandoned_questions.is_empty(),
            "a question nobody saw was given text"
        );
    }
}

#[cfg(test)]
mod fact_tests {
    use super::facts;
    use crate::core::{Run, RunState};
    use jiff::Timestamp;

    fn at(s: &str) -> Timestamp {
        s.parse().expect("a timestamp")
    }

    fn quiet_since(t: Timestamp) -> Run {
        let mut r = Run::new(
            crate::core::SessionId::from("s1"),
            std::path::PathBuf::from("/tmp/p"),
            crate::core::RunMode::Observed,
            "claude",
        );
        r.activity_seen = true;
        r.state = RunState::Working;
        r.last_activity_at = t;
        r
    }

    /// A read-time fact is right for every hour nothing ran.
    #[test]
    fn a_session_quiet_across_a_two_day_shutdown_is_stalled() {
        let run = quiet_since(at("2026-09-19T08:00:00Z"));
        let monday = at("2026-09-21T22:00:00Z"); // 62 hours later
        assert!(
            facts::stalled(&run, monday, 900),
            "62 hours of silence is stalled whether or not anybody was watching"
        );
    }

    #[test]
    fn a_run_that_never_started_is_not_stalled() {
        let mut run = quiet_since(at("2026-09-19T08:00:00Z"));
        run.activity_seen = false;
        assert!(!facts::stalled(&run, at("2026-09-21T22:00:00Z"), 900));
    }

    #[test]
    fn a_run_that_is_not_working_is_not_stalled() {
        let mut run = quiet_since(at("2026-09-19T08:00:00Z"));
        run.state = RunState::Idle;
        assert!(!facts::stalled(&run, at("2026-09-21T22:00:00Z"), 900));
    }

    /// A question's clock does not run while nobody could reach it.
    #[test]
    fn a_deadline_does_not_run_while_nobody_could_answer() {
        let asked = at("2026-09-19T08:00:00Z");
        let back = at("2026-09-21T22:00:00Z"); // the surfaces returned here
        assert_eq!(
            facts::clock_starts(asked, back),
            back,
            "the clock starts when a person could next reach it, not when it was asked"
        );
    }

    #[test]
    fn a_question_asked_while_reachable_keeps_its_own_clock() {
        let asked = at("2026-09-21T23:00:00Z");
        let back = at("2026-09-21T22:00:00Z");
        assert_eq!(
            facts::clock_starts(asked, back),
            asked,
            "an ask made while the surfaces were up is unaffected"
        );
    }

    /// Stall counts downtime; a question's clock does not.
    #[test]
    fn downtime_counts_against_a_session_and_not_against_a_person() {
        let gone = at("2026-09-19T08:00:00Z");
        let back = at("2026-09-21T22:00:00Z");
        assert!(facts::stalled(&quiet_since(gone), back, 900));
        assert_eq!(facts::clock_starts(gone, back), back);
    }
}

#[cfg(test)]
mod currency_tests {
    use super::facts::still_current;
    use crate::core::change::{CommitStamp, Reach};

    fn at(tree: Option<&str>, clean: bool) -> CommitStamp {
        CommitStamp {
            commit: Some("c0ffee".into()),
            tree: tree.map(str::to_string),
            branch: Some("main".into()),
            clean,
            changed_files: if clean { 0 } else { 1 },
            reach: Reach::LocalOnly,
            remote: None,
        }
    }

    #[test]
    fn the_same_working_tree_is_current() {
        assert!(still_current(
            Some(&at(Some("a3f9"), true)),
            Some(&at(Some("a3f9"), true))
        ));
    }

    /// Touching one file makes it false, with nobody clearing a flag.
    #[test]
    fn a_tree_that_moved_on_is_not_current() {
        assert!(!still_current(
            Some(&at(Some("a3f9"), true)),
            Some(&at(Some("b7c2"), true))
        ));
    }

    /// Uncommitted work stays verified while nothing changes.
    #[test]
    fn uncommitted_work_that_has_not_changed_is_current() {
        assert!(still_current(
            Some(&at(Some("a3f9"), false)),
            Some(&at(Some("a3f9"), false))
        ));
    }

    /// Unknown is not unchanged.
    #[test]
    fn an_unknown_digest_is_not_current() {
        assert!(!still_current(None, Some(&at(Some("a3f9"), true))));
        assert!(!still_current(Some(&at(Some("a3f9"), true)), None));
        assert!(!still_current(Some(&at(None, true)), Some(&at(None, true))));
        assert!(!still_current(
            Some(&at(None, true)),
            Some(&at(Some("a3f9"), true))
        ));
    }
}

#[cfg(test)]
mod downtime_tests {
    use super::facts::clock_starts;
    use jiff::Timestamp;

    fn at(s: &str) -> Timestamp {
        s.parse().expect("a timestamp")
    }

    /// Expiry can stay on the host's tick: the deadline runs from
    /// `asked_at.max(reachable_since)`, so an ask cannot become overdue while
    /// no host runs to write the ending.
    #[test]
    fn an_ask_cannot_become_overdue_while_nobody_could_reach_it() {
        let asked = at("2026-09-19T08:00:00Z");
        let five_minutes = 300;

        // Down all weekend, back Monday night.
        let back = at("2026-09-21T22:00:00Z");
        let clock = clock_starts(asked, back);
        assert_eq!(clock, back, "the clock starts when it became reachable");

        // During the downtime: not overdue.
        let saturday = at("2026-09-20T03:00:00Z");
        assert!(
            saturday < clock,
            "a moment before the clock started cannot be past a deadline"
        );

        // Five minutes after it came back: overdue, and a host is up.
        let later = back + jiff::SignedDuration::from_secs(five_minutes + 1);
        assert!(
            (later - clock).get_seconds() > five_minutes,
            "the deadline runs from the moment a person could act, and only then"
        );
    }
}

#[cfg(test)]
mod cost_tests {
    use super::facts::cost_is_unknown;
    use crate::core::{Run, RunMode, SessionId};

    fn run(activity: bool) -> Run {
        let mut r = Run::new(
            SessionId::from("s1"),
            std::path::PathBuf::from("/tmp/p"),
            RunMode::Observed,
            "claude",
        );
        r.activity_seen = activity;
        r
    }

    /// Work happened, nothing was listening.
    #[test]
    fn a_run_that_worked_with_no_telemetry_has_an_unknown_cost() {
        assert!(cost_is_unknown(&run(true)));
    }

    #[test]
    fn a_run_with_requests_reported_has_a_known_cost() {
        let mut r = run(true);
        for _ in 0..3 {
            r.totals.apply(&crate::core::event::ApiUsage {
                cost_usd: 0.1,
                ..Default::default()
            });
        }
        assert!(!cost_is_unknown(&r));
    }

    /// A status-line total without a request count is a known cost.
    #[test]
    fn a_cost_from_the_status_line_alone_is_known() {
        let mut r = run(true);
        let id = r.id.clone();
        super::apply(
            &mut r,
            &crate::core::event::EventEnvelope::new(
                id,
                crate::core::event::Source::StatusLine,
                crate::core::event::Event::StatusSample(crate::core::event::StatusSample {
                    cost_usd: Some(2.5),
                    ..Default::default()
                }),
            ),
        );
        assert_eq!(r.totals.api_requests, 0, "the shim reports no requests");
        assert_eq!(r.totals.cost_usd, 2.5);
        assert!(
            !cost_is_unknown(&r),
            "a stated total is not an unknown cost"
        );
    }

    /// A session that has done nothing has spent nothing, not an unknown amount.
    #[test]
    fn a_run_that_never_did_anything_is_not_unknown() {
        assert!(!cost_is_unknown(&run(false)));
    }
}

/// What a quit ends, and what it does not.
#[cfg(test)]
mod quitting_tests {
    use super::facts::{Quitting, quitting};
    use crate::core::{Run, RunMode, RunState, SessionId};

    /// The session id derives from `at`, so fixture ids are distinct and the
    /// ordering assertion means something.
    fn run(mode: RunMode, state: RunState, at: &str) -> Run {
        let mut r = Run::new(
            SessionId::from(at),
            std::path::PathBuf::from(at),
            mode,
            "claude",
        );
        r.state = state;
        r
    }

    fn ask(id: &str, kind: crate::core::ask::Kind) -> crate::core::ask::Ask {
        crate::core::ask::Ask::new(
            crate::core::AskId::new(id),
            crate::core::RunId::new("r1"),
            crate::core::ask::Asked {
                kind,
                request_id: id.into(),
                message: "may I?".into(),
                payload: serde_json::json!({}),
                at: "2026-09-24T09:00:00Z".parse().expect("a timestamp"),
                deadline: crate::core::ask::Deadline::Never,
            },
        )
    }

    /// What this host started versus what it merely watches.
    #[test]
    fn a_quit_stops_what_it_started_and_leaves_what_it_watches() {
        let runs = [
            run(RunMode::Driven, RunState::Working, "/code/api"),
            run(RunMode::Observed, RunState::Working, "/code/web"),
            run(RunMode::Observed, RunState::Working, "/code/cli"),
            run(RunMode::Background, RunState::Working, "/code/bg"),
        ];
        let q = quitting(runs.iter(), &[]);
        assert_eq!(q.stops.len(), 1, "only the driven run is ours to end");
        assert_eq!(q.stops[0].at, "/code/api");
        assert_eq!(q.leaves, 2);
        assert_eq!(q.delegated, 1);
    }

    /// An ended run is not something a quit stops.
    #[test]
    fn a_finished_run_is_not_something_a_quit_stops() {
        let runs = [
            run(RunMode::Driven, RunState::Completed, "/code/api"),
            run(RunMode::Observed, RunState::Interrupted, "/code/web"),
        ];
        assert_eq!(quitting(runs.iter(), &[]), Quitting::default());
    }

    /// Names the worktree, not the checkout.
    #[test]
    fn an_agent_in_a_worktree_is_named_by_where_it_is_working() {
        let mut r = run(RunMode::Driven, RunState::Working, "/code/api");
        r.worktree = Some(std::path::PathBuf::from("/code/api/.claude/worktrees/fix"));
        let q = quitting([r].iter(), &[]);
        assert_eq!(q.stops[0].at, "/code/api/.claude/worktrees/fix");
    }

    /// Counted from open asks of both kinds, not from blocked runs.
    #[test]
    fn questions_waiting_are_counted_from_the_asks_not_the_runs() {
        use crate::core::ask::Kind;
        let runs = [run(RunMode::Driven, RunState::Working, "/code/cli")];
        let mut answered = ask("a3", Kind::Question);
        answered
            .answer(
                serde_json::json!({}),
                "board",
                "2026-09-24T09:01:00Z".parse().expect("a timestamp"),
            )
            .expect("first answer");
        let asks = [
            ask("a1", Kind::Question),
            ask("a2", Kind::Permission),
            answered,
        ];
        let q = quitting(runs.iter(), &asks);
        assert_eq!(
            q.unanswered, 2,
            "one question and one permission still open"
        );
        assert_eq!(q.stops.len(), 1);
        assert_eq!(
            quitting(runs.iter(), &[]).unanswered,
            0,
            "a blocked run without an ask row is not a question that survives"
        );
    }

    /// Two readings of an unchanged world print the same order.
    #[test]
    fn the_agents_are_listed_in_a_stable_order() {
        let a = run(RunMode::Driven, RunState::Working, "/a");
        let b = run(RunMode::Driven, RunState::Working, "/b");
        let one = quitting([a.clone(), b.clone()].iter(), &[]);
        let other = quitting([b, a].iter(), &[]);
        assert_eq!(one, other);
    }
}

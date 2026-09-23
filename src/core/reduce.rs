//! The reducer: `(Run, Event) -> Run`.
//!
//! Run state is a pure reduction over the event log, which is what makes the
//! board rebuildable after a restart and the state machine testable without a
//! provider. Nothing here does I/O or reads the clock beyond the timestamp the
//! envelope already carries.

use crate::core::event::{Event, EventEnvelope, WaitingFor};
use crate::core::run::{BlockedOn, Run, RunMode, RunState, ToolCall};

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
    // Which channels are carrying this session decides whose word counts about
    // its state, so it is recorded before anything acts on it.
    if matches!(env.source, crate::core::Source::Hook) {
        run.last_hook_at = Some(env.at);
    }
    if env.event.is_activity() {
        run.last_activity_at = env.at;
        // Anything the session says about itself makes it real rather than
        // merely present.
        run.reporting = true;
        // And it means a channel is carrying this session's activity, so
        // silence from it later is a fact rather than an absence of wiring.
        run.activity_seen = true;
        // Anything the session does ends the quiet period, so the next silence
        // is reported as its own stall rather than suppressed by the last one.
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
            // **Only where it was read**, so a later event on a channel that
            // carries no environment cannot erase what the first one found.
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
            // The agent is up and has named the conversation, so the run is
            // working rather than merely spawned — and it is now resumable,
            // which is the fact this event exists to make durable.
            run.agent_session = Some(agent_session.clone());
            run.state = RunState::Working;
        }

        Event::AgentProcessSpawned { pid } => {
            // Not a liveness signal. A driven run is live while its connection
            // is, and `poller::still_running` deliberately never consults this
            // for one. It is here so that a daemon which was killed rather than
            // stopped leaves behind enough to find the agent it abandoned.
            run.pid = Some(*pid);
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

        Event::ToolStarted { tool, input, .. } => {
            // **The agent moving on is the only evidence there is that nobody
            // answered**, and clearing the block used to be all that happened.
            // A question that left the inbox because the person answered it and
            // one that left because the agent gave up produced byte-identical
            // state, which is this product's second obligation failing in the
            // one place it is derivable.
            abandon_open_question(run, env.at, Some(summarise_tool(tool, input)));
            run.state = RunState::Working;
            run.blocked_on = None;
            run.totals.tool_calls += 1;
            run.recent_tools.push(ToolCall {
                tool: tool.clone(),
                at: env.at,
                ok: None,
                input: Some(input.clone()),
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
                // The input is kept only while the call is in flight: it is
                // there so a *blocked* run can say what it is blocked on, and
                // `RECENT_TOOLS` of them would be a log nobody asked for.
                last.input = None;
            }
            if !ok {
                run.totals.errors += 1;
            }
        }

        Event::PermissionModeSeen { mode } => {
            let seen = crate::core::run::PermissionMode::parse(mode);
            // **Only a change moves the clock.** This arrives on every prompt
            // and every finished tool call, so stamping it each time would
            // make "seen in auto since" mean "a moment ago", for ever, which
            // is worse than not showing it: the one fact the row exists to
            // carry is *how long this has been true*.
            if run.permission_mode.as_ref() != Some(&seen) {
                run.permission_mode = Some(seen);
                run.permission_mode_seen = Some(env.at);
            }
        }

        Event::AgentModeSeen { mode } => {
            // Only a change moves the clock, for the same reason
            // `PermissionModeSeen` above does: the fact worth carrying is how
            // long this has been true.
            if run.agent_mode.as_deref() != Some(mode.as_str()) {
                run.agent_mode = Some(mode.clone());
                run.agent_mode_seen = Some(env.at);
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
            // costs more and finishes worse. `deny` is Devplane's own verdict
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
        // Devplane depends on were edited, and when.
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
            ask,
            options,
            call,
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
                    ask: ask.clone().map(crate::core::AskId::new),
                    // **The event first, the run's in-flight call second.**
                    // `BlockedOn::input` was `None` at every site, which made
                    // it a field the type documented and nothing ever set —
                    // and the one consumer, the rule offered on a permission
                    // item, silently did nothing for every watched session.
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
                // Present only for a driven run. An observed session's dialog
                // belongs to the provider, and a row offering to answer one
                // would be offering something no route can deliver.
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

        // The question stopped being answerable. Clearing the block is what
        // takes the row out of the inbox; leaving it would keep a button whose
        // request no longer exists.
        Event::QuestionEnded { .. } => {
            if matches!(run.state, RunState::Waiting(WaitingFor::Question)) {
                run.state = RunState::Working;
                run.blocked_on = None;
            }
        }

        // **A listing fills gaps and may not contradict an observation.** The
        // rule is `Run::enrich_from_listing`, which refuses anything that is
        // not a gap — a title where there is none, and nothing else. There is
        // no state to take: `SessionInfo` carries none.
        Event::SessionListed {
            agent_session,
            title,
        } => {
            run.enrich_from_listing(&crate::core::run::ListedSession {
                agent_session: agent_session.clone(),
                title: title.clone(),
            });
        }

        Event::TurnEnded => {
            // The turn happened and it cost what it cost, whatever state the
            // run is in now.
            run.totals.turns += 1;
            // **A turn ending is bookkeeping, and bookkeeping may not raise the
            // dead.** `Stop` fires at the end of every turn and arrives on a
            // different channel from `SessionEnd`, so a late one lands after a
            // session has ended — and this arm moved *every* state to `Idle`,
            // which `is_live()` counts as in-play. A **failed** session came
            // back to the board as *waiting for a prompt*: the failure gone
            // from the inbox, the row alive again, and nothing anywhere saying
            // so. It is the same class as writing `completed` over interrupted
            // work — a state this product distrusts, put over one it should
            // keep — reached from the other side.
            //
            // A run that has genuinely resumed says so with *work*: a tool
            // call, a question, a roster sighting. Those arms revive it on
            // purpose, and this one does not.
            if !run.state.is_live() {
                return;
            }
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
            // **One arm, because a `match` does not fall through.** There were
            // two: this one, and a guarded one above it that recorded an
            // abandoned question and set no state. The guarded arm won whenever
            // a session ended while asking — so the run stayed
            // `Waiting(Question)` for ever on a session that was gone, kept its
            // `blocked_on`, and was raised twice: once as a live question
            // nobody could answer, once as an abandonment. The one case this
            // product is named for was the one case the state machine dropped.
            //
            // **And the reasons are an enumerated set, so each is a case.** The
            // vendor documents `clear`, `resume`, `logout`, `prompt_input_exit`
            // and `other`, and **not one of them means the work finished** —
            // every one is how a *person* ended the session. The catch-all read
            // all of them as `Completed`, which is the flattering value and the
            // word this product exists to distrust, written over a session
            // somebody simply quit (`prompt_input_exit`) or moved elsewhere
            // (`resume`).
            let next = match (reason.as_deref(), &run.state, run.mode) {
                (_, RunState::Failed, _) => RunState::Failed,
                (Some("error") | Some("failed"), _, _) => RunState::Failed,
                // **The daemon stopping is not the work finishing.** Only
                // `driven.rs` sends this, and only while tearing down.
                (Some("interrupted"), _, _) => RunState::Interrupted,
                // Every documented reason is a person ending a session.
                (Some("clear" | "resume" | "logout" | "prompt_input_exit" | "other"), _, _) => {
                    RunState::Stopped
                }
                // **No reason, and the answer depends on who was driving.** A
                // run Devplane owns reaches here when the agent's own process
                // ends without a teardown, which is a driven run finishing. A
                // session somebody else started reaching here means the vendor
                // sent `SessionEnd` with a reason this build does not know —
                // and an unknown member of somebody else's enumeration is not
                // grounds to claim the work completed.
                (_, _, RunMode::Driven) => RunState::Completed,
                _ => RunState::Stopped,
            };

            // **A session that ends while a question is open ended it with
            // nobody having answered** — unless Devplane ended it, in which
            // case the question is still answerable and the `asks` row is what
            // carries it.
            if !matches!(next, RunState::Interrupted) {
                abandon_open_question(run, env.at, None);
            }

            run.state = next;
            // **A run that was interrupted keeps what it was blocked on.** That
            // is the difference between *this ended* and *this was cut off
            // while waiting for you*: the second is answerable after a restart
            // and the surface has to be able to say so.
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
                // **An interactive row for a session no hook has ever spoken
                // about, which on an unconnected machine is every session.** The
                // roster is the only channel there is, so it decides on every
                // sample rather than once.
                //
                // `last_hook_at` is the test, and the run's state is not: a
                // condition on `Starting` is cleared by the roster's own first
                // sample, so it would run once and shut.
                //
                // All three documented values have an arm. `waiting` means the
                // vendor is reporting a person blocked on its own dialog, and
                // for an unconnected session nothing else can say so.
                let reason = waiting_for.clone();
                match status.as_deref() {
                    Some("busy") => {
                        run.state = RunState::Working;
                        run.blocked_on = None;
                    }
                    Some("waiting") => {
                        let what = WaitingFor::parse_roster(reason.as_deref());
                        // Only ever additive: a block a hook recorded carries a
                        // request id, the options and a token to answer by, and
                        // the roster carries none of those. Replacing it would
                        // turn an answerable question into a row that can only
                        // be looked at.
                        if !run.state.needs_human() {
                            run.state = RunState::Waiting(what.clone());
                            run.blocked_on = Some(BlockedOn {
                                waiting_for: what,
                                message: reason,
                                // Nothing to answer by: this session is the
                                // vendor's, and the errand is to reach its own
                                // window. `attention` already renders such an
                                // item without Allow and Deny buttons.
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
                        // **`idle` is a statement about token generation, and
                        // it is true of a session waiting on its own test suite
                        // as well as of one waiting for you.** The job count
                        // below is the finer signal and ends itself on a
                        // checked zero; overwriting it here would take a
                        // running suite off the board twice a second.
                        if !run.state.waits_on_a_job() {
                            run.state = RunState::Idle;
                            run.blocked_on = None;
                        }
                    }
                    // A value this build does not know, and no value at all,
                    // are the same answer: the session exists and that is the
                    // whole claim. Neither is evidence for a change, and
                    // calling either one working would be an invention.
                    _ => {
                        if matches!(run.state, RunState::Starting) {
                            run.state = RunState::Idle;
                        }
                    }
                }
            } else if matches!(run.state, RunState::Starting) {
                // A hook has spoken about this session but has not yet said
                // what it is doing — the mode and the working directory arrive
                // before any turn does. `Starting` never ages out, so leaving
                // it there would keep a dormant tab in the working set for
                // ever; the roster is the only thing here with an opinion.
                run.state = match status.as_deref() {
                    Some("busy") => RunState::Working,
                    _ => RunState::Idle,
                };
            }

            // **What the roster calls `idle` is two different situations, and
            // only one of them is a person's turn.**
            //
            // Every provider reports a session as idle whenever it is not
            // generating — including while it waits on a background command it
            // started itself, which for agent work is most often a test suite.
            // The board said *waiting for a prompt* for both, so a forty-minute
            // suite read as *you are the blocker* for forty minutes. The
            // process table is the evidence the roster does not carry: the
            // session's own running commands, counted.
            //
            // It decides only between *idle* and *waiting on a job*. A session
            // blocked on a permission or a question is untouched, because a
            // person is genuinely owed something there and a running command
            // does not change that.
            if let Some(count) = jobs
                && !run.state.needs_human()
                && run.state.is_live()
            {
                match count {
                    0 => {
                        // A checked zero: the job finished. Only a block this
                        // same rule put there is cleared — anything else was
                        // put there by something that knew more.
                        if run.state.waits_on_a_job() {
                            run.state = RunState::Idle;
                            run.blocked_on = None;
                        }
                    }
                    _ => {
                        // `Working` is left alone: the provider saying the
                        // model is generating is a stronger claim than ours,
                        // and a command running during a turn is ordinary.
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
                                since: env.at,
                            });
                        }
                    }
                }
            }
            let _ = kind;
        }

        Event::StatusSample(s) => {
            if s.context_used_percent.is_some() {
                run.totals.reported_context_percent = s.context_used_percent;
            }
            // The window the percentage is *of*, stated rather than inferred
            // from the model. Only overwritten when the provider says so, so a
            // sample without it leaves a figure derived from telemetry alone.
            if s.context_window_size.is_some() {
                run.totals.context_window = s.context_window_size;
            }
            // The window closest to its limit is the one that will stop the
            // work, so that is the one to carry, with its reset time.
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
            // A sample never *clears* a fact: the payload omits blocks in some
            // states, and forgetting one because a message did not repeat it is
            // how a board flickers.
            if s.model.is_some() {
                run.model = s.model.clone();
            }
            if s.claude_version.is_some() {
                run.claude_version = s.claude_version.clone();
            }
            // Cost from the provider's own accounting. Assigned rather than
            // added: it is a session total, where the telemetry channel sends
            // per-request deltas. Whichever arrives last is the more recent
            // statement of the same quantity.
            if let Some(c) = s.cost_usd {
                run.totals.cost_usd = c;
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

        Event::Lost { reason: _ } => {
            // One observation — the process is not there — with two readings,
            // and which one applies is a property of what the run was doing.
            // A session that was working or being asked something stopped
            // mid-flight and nobody was told: that is a loss, and it is
            // Critical. A session that was idle or had only just announced
            // itself is an editor tab that closed: it is simply over.
            //
            // Deciding this here rather than at the call site is what keeps a
            // replay of the log honest, and `Lost` is deliberately not
            // activity — so a session that died on Tuesday does not get a
            // timestamp saying it did something just now.
            //
            // The summary is left alone for the same reason: what it was doing
            // is the only useful thing left, and overwriting it with the
            // diagnosis ("process not found at startup") threw the evidence
            // away.
            run.state = match run.state {
                RunState::Working | RunState::Waiting(_) => RunState::Lost,
                _ => RunState::Stopped,
            };
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
fn last_pending_tool(run: &Run) -> Option<&ToolCall> {
    run.recent_tools.iter().rev().find(|c| c.ok.is_none())
}

/// How much of a tool call's content is kept in state.
///
/// **Not a display width.** It bounds what the reducer stores, so one heredoc
/// cannot grow a run's record without limit. Shortening for a screen belongs to
/// the surface, which is the only place that knows how wide the screen is.
pub const TOOL_CONTENT_KEPT: usize = 2_000;

/// A description of what a tool call is doing, kept whole.
///
/// **This clipped to eighty characters here**, in the reducer — so the shortened
/// string was the only one that ever existed and no surface could recover the
/// rest. A command is evidence: the certificate carries `command`, `shown` and
/// `truncated` for the same reason, because *a reformatted command is one a
/// reviewer cannot paste*.
///
/// The only thing that can be long is a shell command with a heredoc in it,
/// which [`TOOL_CONTENT_KEPT`] bounds.
fn summarise_tool(tool: &str, input: &serde_json::Value) -> String {
    match crate::core::policy::rule_content(tool, input) {
        Some(d) => format!("{tool}: {}", crate::core::text::clip(&d, TOOL_CONTENT_KEPT)),
        None => tool.to_string(),
    }
}

/// Records that a question left the inbox without an answer.
///
/// **Only for a question nobody could have answered from here.** A driven ask
/// carries a `request_id` and a durable `asks` row: it is answerable, it is
/// ended deliberately with an authority, and recording it here as well would be
/// two rows for one fact, disagreeing the moment either changes.
///
/// `PostToolUse` for the question's own call arrives as `ToolFinished` and is
/// *not* an abandonment — the tool completed, which is what the vendor's
/// `PostToolUse` means. The reference is explicit that it runs *"after a tool
/// call succeeds"*.
///
/// **One thing it cannot see, stated rather than implied**: a question the
/// vendor's own auto-continue timer closed. That *submits*, so the tool
/// succeeds and `PostToolUse` fires — it looks exactly like an answer from
/// here, and the elapsed time cannot separate them because the timer restarts
/// on a keypress. `devplane modes` is the surface that covers that half.
fn abandon_open_question(
    run: &mut crate::core::Run,
    at: jiff::Timestamp,
    moved_on_to: Option<String>,
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
        });
    // A record, not a log. The oldest go and the count of what went is kept, so
    // no surface can imply it is showing all of them.
    while run.abandoned_questions.len() > ABANDONED_KEPT {
        run.abandoned_questions.remove(0);
        run.abandoned_dropped = run.abandoned_dropped.saturating_add(1);
    }
}

/// How many abandoned questions one run keeps.
///
/// Enough that a day of ordinary work fits, small enough that a session in a
/// loop cannot turn a run row into a transcript.
const ABANDONED_KEPT: usize = 20;

#[cfg(test)]
mod clipping {
    use super::*;

    fn bash(command: &str) -> serde_json::Value {
        serde_json::json!({ "command": command })
    }

    /// **The reducer keeps the command; a surface shortens it.**
    ///
    /// This clipped to eighty characters *in the reducer*, so the shortened form
    /// was the only form that ever existed. `run.summary` was eighty characters,
    /// the `stalled` inbox item copies `run.summary`, and a person looking at a
    /// row that ended in an ellipsis had no way to read the rest — not on the
    /// board, not in the terminal, not in `--json`. The characters were gone
    /// before any surface was reached.
    ///
    /// A command is evidence, and the work view already knew it: *a reformatted
    /// command is one a reviewer cannot paste.*
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

    /// **And it is still bounded**, because a heredoc is a command too and state
    /// that grows with one is state nobody can reason about. The bound is
    /// generous rather than a screen: the point is that no single call can grow
    /// a run's record without limit, not that eighty characters is enough of a
    /// command to keep.
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

    /// A tool whose content the policy cannot name is still named itself.
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
        // Stated by the provider, not inferred from which model is in play.
        assert_eq!(r.totals.context_window, Some(1_000_000));
        // The window closest to its limit is the one that will stop the work.
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
        // The payload drops blocks in some states — a model between turns, a
        // cost before the first response. Clearing a fact because one message
        // did not repeat it is how a board flickers, and a flickering board is
        // one people stop reading.
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
        // Telemetry sends per-request deltas and the status line sends a
        // session total. Adding the total to the running sum would double every
        // figure on a machine with both channels on — the kind of wrong that
        // looks plausible on a board and is never questioned.
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

    /// An envelope carrying the channel the event actually comes from.
    ///
    /// This labelled everything `Source::Hook`, including roster samples, which
    /// no hook has ever produced. The mislabelling was invisible while nothing
    /// read the source — and it hid the roster bugs completely, because every
    /// test that drove a roster event was telling the reducer a hook had
    /// spoken, which is the one condition that makes the roster stand down.
    fn ev(e: Event) -> EventEnvelope {
        let source = e.test_source();
        EventEnvelope::new(crate::core::ids::RunId::new("s1"), source, e)
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

    /// **The obligation, and the defect it was failing at.**
    ///
    /// A watched session's question puts the run in `Waiting(Question)`. The
    /// next tool call cleared the block and the state, because the agent is
    /// working again — so *the person answered it* and *the agent gave up on
    /// it* produced byte-identical runs, and the question left the inbox with
    /// nothing anywhere recording that nobody had answered.
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
    }

    /// **A session that ends while a question is open, which is the case the
    /// two arms disagreed about.**
    ///
    /// There were two `SessionEnded` arms: a guarded one that recorded the
    /// abandonment and set no state, and a general one that set the state. A
    /// `match` does not fall through, so the guarded arm won and the run stayed
    /// `Waiting(Question)` for ever on a session that had ended — raised once
    /// as a live question nobody could answer and once as an abandonment.
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

    /// **Every documented `SessionEnd` reason, and not one of them means the
    /// work finished.**
    ///
    /// The vendor enumerates `clear`, `resume`, `logout`, `prompt_input_exit`
    /// and `other`; every one is how a *person* ended a session. The reducer
    /// read four of them through a catch-all as `Completed` — the flattering
    /// value, and the word this product exists to distrust, written over a
    /// session somebody quit or moved elsewhere.
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

    /// A reason this build has never seen is not grounds to claim completion
    /// either — for a session somebody else started. An unknown member of
    /// another product's enumeration is unknown, and the safe reading of it is
    /// the one that claims least.
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

    /// **A run Devplane drives is the one case where the connection closing
    /// does mean the work ended.** The agent's own process reaching the end of
    /// its turn is what produces this, and `driven.rs` sends `interrupted`
    /// whenever the ending is Devplane's doing instead.
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

    /// **A turn ending is bookkeeping, and bookkeeping may not raise the
    /// dead.**
    ///
    /// `Stop` fires at the end of every turn and arrives on a different channel
    /// from `SessionEnd`, so a late one lands after a session has ended. This
    /// arm moved *every* state to `Idle` — which `is_live()` counts as in-play
    /// — so a **failed** session came back to the board as *waiting for a
    /// prompt*: the failure gone from the inbox, the row alive again, and
    /// nothing anywhere saying so.
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

    /// **And a run that genuinely resumes says so with work.** `/resume` ends a
    /// session and continues it under the same id, so the arms that see a tool
    /// call or a question revive it on purpose. Asserted beside the rule above,
    /// because the two together are the decision: evidence revives a run, and
    /// bookkeeping does not.
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

    /// **A `session/list` result cannot contradict an observed roster into a
    /// wrong state.**
    ///
    /// A listing is the agent's catalogue of its own history; a run is what
    /// Devplane has observed. An observation outranks a catalogue, so the
    /// enrichment may only fill gaps — and there is no state to take even if it
    /// wanted one, because `SessionInfo` carries none in v1 or in v2's draft.
    #[test]
    fn a_session_listing_fills_gaps_and_never_moves_a_run() {
        use crate::core::run::ListedSession;

        let listed = |title: &str| Event::SessionListed {
            agent_session: "agent-1".into(),
            title: Some(title.into()),
        };

        // Across every state, the listing leaves it exactly where it was.
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

        // …and a name that is already there is never overwritten, because the
        // one Devplane observed is the one the person has been reading.
        apply(&mut r, &ev(listed("something else")));
        assert_eq!(r.name.as_deref(), Some("fix the flaky login test"));

        // A listing about a different session says nothing about this one.
        let mut other = run();
        other.agent_session = Some("agent-2".into());
        apply(&mut other, &ev(listed("not yours")));
        assert_eq!(other.name, None);

        // Whitespace is refused rather than stored: a name of three spaces
        // renders as a name that is not there, which is worse than the id.
        let mut blank = run();
        blank.agent_session = Some("agent-1".into());
        assert!(!blank.enrich_from_listing(&ListedSession {
            agent_session: "agent-1".into(),
            title: Some("   ".into()),
        }));
        assert_eq!(blank.name, None);
    }

    /// A question that **completed** is not abandoned. `PostToolUse` arrives as
    /// `ToolFinished` and the vendor documents it as running *"after a tool call
    /// succeeds"* — so the tool ran, and whatever it returned is an answer.
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
            }),
        );
        assert!(r.abandoned_questions.is_empty(), "it completed");
    }

    /// A question that is merely **waiting** is not abandoned, and no amount of
    /// time makes it so. Nothing here ends a question on a duration; it
    /// takes a later event.
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

    /// **A driven question is the `asks` row's and is not recorded twice.** It
    /// is answerable, it ends deliberately with an authority on it, and a second
    /// record of the same fact is two rows that disagree the moment either
    /// changes.
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

    /// The list is a record, not a log: the oldest go and the count of what
    /// went is kept, so no surface can imply it is showing all of them.
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

    /// **Only a change moves the clock, because the clock is the whole point.**
    ///
    /// This event arrives on every prompt and every finished tool call, so
    /// stamping the timestamp each time would make *"in auto, seen 3 seconds
    /// ago"* true for ever — which reads as reassuring and is the opposite of
    /// the fact the row exists to carry. A person with six repositories wants
    /// to know one of them has been deciding without them since Tuesday.
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

        // The same mode again, later. Nothing changed, so nothing moves.
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

        // A different mode is a different fact and does move it.
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
        // Otherwise the board shows the previous turn's last tool call as
        // though the agent were running it now.
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
        // Stop fires at the end of the turn that asked the question. If it
        // cleared the block, the item would vanish from the inbox before the
        // human ever saw it — the single worst bug this product can have.
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

    /// **A session with no hooks is the only thing the roster speaks for, and
    /// it was consulted once and then ignored for the rest of the session.**
    ///
    /// The condition said *"until the first hook arrives"* and tested whether
    /// the state was still `Starting` — which the roster's own first sample
    /// clears. So the branch ran once and shut. On a machine where nothing is
    /// connected, that is every session on the board frozen at whatever it
    /// happened to be doing the first time Devplane looked.
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

    /// **The third status value, which had no arm and fell into idleness.**
    ///
    /// `status` is documented as `busy`, `waiting` or `idle`. `waiting` means
    /// the session is blocked on a person and `waitingFor` names the reason.
    /// Both were discarded: the state became `Idle`, `needs_human()` was false,
    /// and a permission dialog sitting on somebody's screen never reached the
    /// inbox at all — for an unconnected session, the only channel that could
    /// have said so.
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
            // Nothing to answer by, which is what stops the inbox offering a
            // button that cannot reach this session.
            assert!(r.blocked_on.as_ref().unwrap().request_id.is_none());

            // And it clears when the vendor says the wait ended.
            apply(&mut r, &ev(roster("interactive", Some("busy"))));
            assert_eq!(r.state, RunState::Working);
            assert!(r.blocked_on.is_none());
        }
    }

    /// **The roster never downgrades what a hook recorded.** A hook-set block
    /// carries a request id, the options and a token to answer by; the roster
    /// carries none of them. Overwriting would turn an answerable question into
    /// a row a person can only look at.
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

        // And once a hook has spoken, the roster stops deciding the state at
        // all — the hook channel is the fresher and richer one.
        apply(&mut r, &ev(roster("interactive", Some("busy"))));
        assert_eq!(r.state, RunState::Waiting(WaitingFor::Question));
    }

    /// **The bug a person found on their own board**, and the reason it is
    /// worth a state of its own.
    ///
    /// A session was running a test suite it had started in the background.
    /// The roster called it `idle`, because no tokens were being generated, and
    /// Devplane rendered that as *waiting for a prompt* — telling the person
    /// they were the blocker for as long as the suite ran. Agents wait on long
    /// suites constantly, so this was not an edge case; it was the board being
    /// wrong about its own central question most afternoons.
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

        // **And it stays on the board.** `is_active` ages an idle session out
        // after six hours; a suite that runs longer than that would take the
        // session off the board exactly when it mattered, and the person would
        // come back to a finished run they were never shown.
        r.last_activity_at -= jiff::Span::new().hours(9);
        assert!(r.is_active(), "a running job never ages out");
    }

    /// The other half, which is what makes the first half safe to act on: a
    /// checked zero ends the block. Without it the state would be sticky and a
    /// session really waiting for a prompt would read as busy for ever — the
    /// same bug pointed the other way.
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

    /// **`None` is not zero, and this is where that distinction earns its
    /// keep.** A poll that could not read the process table — or one taken
    /// before this field existed — must leave the board as it found it rather
    /// than reporting that every job on the machine just finished.
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

    /// A person waiting on a permission outranks a command running underneath
    /// it. The agent may well have left something in the background; what the
    /// board must say is the thing only a person can clear.
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
        // This is what makes `devplane ls` useful the moment it is installed:
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
        apply(&mut r, &ev(Event::tool_started("Bash", json!({}))));
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

    /// **What this test is named for is that the run leaves the working set**,
    /// and it asserted the *word* instead. It pinned `Completed` for a watched
    /// session with no reason — which is how the catch-all's flattering default
    /// survived every run of this suite. A test that asserts a value where it
    /// means a property is a test that defends whatever the code happens to do.
    #[test]
    fn session_end_is_terminal() {
        let mut r = run();
        apply(&mut r, &ev(Event::SessionEnded { reason: None }));
        assert!(!r.state.is_live(), "the session is over");
        assert!(!r.state.needs_human(), "and nobody is owed anything by it");
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

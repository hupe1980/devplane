//! Writing down what a short-lived process saw, with no host in the path.
//!
//! A hook decides in its own process, appends the observation (and any
//! decision) to the store, and exits. It never projects: the host is the one
//! writer of `runs`, folding these rows in ([`crate::poller::tail`]); with no
//! host, readers replay them ([`crate::local`]). A deferred permission adds a
//! wait: the hook writes the ask and polls the row until someone answers or
//! the hold runs out.

use crate::core::event::{Event, EventEnvelope, Source};
use crate::core::{DecidedEnvelope, RunId, World};
use crate::observe::hook::HookPayload;
use crate::store::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Appends the observation events a hook payload carries.
///
/// The project is resolved here so the row carries it, and a project seen for
/// the first time is registered on sight, as the host would.
pub async fn observe(store: &Store, payload: &HookPayload, source: Source) -> Result<()> {
    if payload.session_id == crate::observe::hook::PROBE_SESSION {
        return Ok(());
    }
    let events = crate::observe::hook::to_events(payload).events;
    append(
        store,
        RunId::new(payload.session_id.clone()),
        payload.cwd.as_deref(),
        events,
        source,
    )
    .await
}

/// Writes down a decision the gate has already enforced, and the observation
/// that came with it. The verdict is recorded, never recomputed, so the record
/// cannot disagree with the answer the agent received.
pub async fn decided(store: &Store, env: DecidedEnvelope) -> Result<()> {
    if env.session == crate::observe::hook::PROBE_SESSION {
        return Ok(());
    }
    let run = RunId::new(env.session.clone());
    let subject = env.subject.clone();
    // The call's directory and project go on the decision rows too, so the
    // ledger can be read per project.
    let cwd = env
        .payload
        .as_ref()
        .and_then(|p| p.get("cwd"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let project = match cwd.as_deref() {
        Some(dir) => project_of(store, dir).await?,
        None => None,
    };
    let late = if env.late {
        " (filed late: nothing was listening)"
    } else {
        ""
    };

    // An escalation (a person's rule constrains this tool and the matcher
    // could not read the call, or the rules file would not load) is recorded
    // under `rule` with outcome `unresolved`: it was caused by what the person
    // wrote, and Devplane decided nothing.
    if let Some(why) = env.why.as_deref()
        && env.rule.is_none()
    {
        let mut d = crate::core::Decision::new(
            crate::core::Authority::Rule,
            "agent:tool.use",
            subject.clone(),
            &env.verdict,
        )
        .because(format!("{why}{late}"))
        .by_tool(env.tool.clone())
        .from_server(env.server_source.clone())
        .for_run(&run)
        .in_project(project.clone());
        if let Some(at) = env.at {
            d = d.at(at);
        }
        store.append_decision(&d).await?;
    }
    if let Some(rule) = env.rule.as_deref() {
        let mut d = crate::core::Decision::new(
            crate::core::Authority::Rule,
            "agent:tool.use",
            subject,
            &env.verdict,
        )
        .because(format!("{rule}{late}"))
        .by_tool(env.tool.clone())
        .from_server(env.server_source.clone())
        .for_run(&run)
        .in_project(project.clone());
        if let Some(at) = env.at {
            d = d.at(at);
        }
        store.append_decision(&d).await?;
    }

    // The observation half, so the board shows the call whether or not a rule
    // spoke, and a question reaches the inbox the instant it is asked.
    let mut events = Vec::new();
    // The vendor's own verdict (classifier, matched rule) travels on the row:
    // which mechanism was about to decide is what this ledger is for.
    let mut context = None;
    if let Some(payload) = env.payload.clone()
        && let Ok(p) = serde_json::from_value::<HookPayload>(payload)
    {
        context = p.permission_context.clone();
        events.extend(crate::observe::hook::to_events(&p).events);
    }
    // An unruled permission is a blocked run, and the inbox learns it here.
    match env.verdict.as_str() {
        "allow" | "deny" => events.push(Event::PermissionDecided {
            tool: env.tool.clone(),
            decision: env.verdict.clone(),
            by: env
                .rule
                .as_deref()
                .map(|r| format!("policy:{r}"))
                .unwrap_or_else(|| "policy".into()),
            reason: env.why.clone(),
            context: context.clone(),
            call_id: None,
        }),
        _ if env.blocked => events.push(Event::Blocked {
            waiting_for: crate::core::event::WaitingFor::Permission,
            message: Some(env.subject.clone()),
            request_id: None,
            ask: None,
            options: Vec::new(),
            // The tool hook may never fire, so the gate carries the call.
            call: env
                .payload
                .as_ref()
                .and_then(|p| p.get("tool_input"))
                .map(|input| crate::core::event::ToolCallRef {
                    tool: env.tool.clone(),
                    input: input.clone(),
                }),
            context: context.clone(),
        }),
        _ => {}
    }
    append(store, run, cwd.as_deref(), events, env.source).await
}

/// The project a directory resolves to, noted if it is new.
async fn project_of(store: &Store, dir: &Path) -> Result<Option<crate::core::ProjectId>> {
    let mut world = World::new();
    for p in store.load_projects().await? {
        world.upsert_project(p);
    }
    let resolved = world.resolve_project(dir);
    if let Some((id, Some(_discovered))) = &resolved
        && let Some(p) = world.project(id)
    {
        store.note_project(p).await?;
    }
    Ok(resolved.map(|(id, _)| id))
}

/// Appends events for a run, each carrying the project its directory resolves
/// to; a directory never seen becomes a project on sight.
async fn append(
    store: &Store,
    run: RunId,
    cwd: Option<&Path>,
    events: Vec<Event>,
    source: Source,
) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let mut world = World::new();
    for p in store.load_projects().await? {
        world.upsert_project(p);
    }
    let resolved = cwd.and_then(|dir| world.resolve_project(dir));
    if let Some((id, Some(_discovered))) = &resolved
        && let Some(p) = world.project(id)
    {
        store.note_project(p).await?;
    }
    let project = resolved.map(|(id, _)| id);
    for event in events {
        let mut env = EventEnvelope::new(run.clone(), source, event);
        env.project_id = project.clone();
        store.append_event(&env).await?;
    }
    Ok(())
}

/// Appends a status-line sample.
pub async fn status(
    store: &Store,
    payload: &crate::observe::statusline::StatusPayload,
) -> Result<()> {
    let run = RunId::new(payload.session_id.clone());
    for event in crate::observe::statusline::to_events(payload) {
        let env = EventEnvelope::new(run.clone(), Source::StatusLine, event);
        store.append_event(&env).await?;
    }
    Ok(())
}

/// How long past its `held_until` an open hold may stay open before it is taken
/// to have lost its hook (the holder closes it within milliseconds).
const ORPHANED_AFTER: std::time::Duration = std::time::Duration::from_secs(10);

/// Ends every held permission whose hook is gone, as `nobody`.
///
/// A row still open [`ORPHANED_AFTER`] past its `held_until` has no process
/// left to close it (the hook was killed, the session interrupted, the machine
/// slept). It is ended the next time anything looks, so the inbox stops
/// offering it and a late answer is told so. Only holds past their deadline
/// are touched, so a live hook is never raced.
pub async fn end_orphaned_holds(store: &Store) {
    let Ok(open) = store.open_asks().await else {
        return;
    };
    let now = jiff::Timestamp::now();
    let grace = jiff::SignedDuration::from_secs(ORPHANED_AFTER.as_secs() as i64);
    for mut a in open {
        let held_until = (a.request_id.is_empty() && a.kind == crate::core::ask::Kind::Permission)
            .then(|| a.payload.get("held_until").and_then(|v| v.as_str()))
            .flatten()
            .and_then(|t| t.parse::<jiff::Timestamp>().ok());
        let Some(until) = held_until else { continue };
        if now < until + grace {
            continue;
        }
        a.end(
            crate::core::ask::Ended::Nobody {
                because: "the hook holding it is gone, so nobody was waiting for the answer".into(),
            },
            now,
        );
        let _ = store.close_ask(&a).await;
    }
}

/// Holds a watched session's permission for a person, against the store.
///
/// Writes the ask, notifies when no host is up to, and polls until an answer
/// lands or the hold runs out. Returns `None` for everything but an answer: a
/// lapse ends the row as `nobody` and the vendor's own dialog takes over.
pub async fn hold(
    store: &Store,
    session: &str,
    cwd: &Path,
    tool: &str,
    call: &str,
    wait: std::time::Duration,
    notify: bool,
) -> Option<String> {
    let run = RunId::new(session.to_string());
    let id = crate::core::AskId::new(crate::core::ids::new_event_id());
    let held_until = jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_millis(
            i64::try_from(wait.as_millis()).unwrap_or(i64::MAX),
        ))
        .unwrap_or(jiff::Timestamp::MAX);
    let asked = crate::core::ask::Asked {
        kind: crate::core::ask::Kind::Permission,
        // Empty: no protocol request stands behind a watched session's ask.
        request_id: String::new(),
        message: format!("{tool} · {call}"),
        payload: serde_json::json!({
            "tool": tool,
            "call": call,
            "held_until": held_until.to_string(),
            "options": [
                {"id": "allow", "label": "Allow"},
                {"id": "deny", "label": "Deny"},
            ],
        }),
        at: jiff::Timestamp::now(),
        // A hold is not a deadline: nothing is decided when it lapses.
        deadline: crate::core::ask::Deadline::Never,
    };
    let project = {
        let mut world = World::new();
        for p in store.load_projects().await.unwrap_or_default() {
            world.upsert_project(p);
        }
        world.resolve_project(cwd).map(|(id, _)| id)
    };
    let ask = crate::core::ask::Ask::new(id.clone(), run, asked).in_project(project);
    // Holds an earlier hook left behind are closed before this one is raised.
    end_orphaned_holds(store).await;
    if store.save_ask(&ask).await.is_err() {
        // A hold nobody can record is one nobody can answer: lapse.
        return None;
    }
    if notify {
        crate::notify::send(
            "A permission is waiting for you",
            &format!("{tool} · {}", crate::core::text::clip(call, 80)),
        );
    }

    // The wall clock, like the orphan sweep that reads `held_until`: a
    // monotonic clock stops while the machine sleeps, so after a sleep the
    // sweep would end a hold this loop still thought live.
    let deadline = held_until;
    let answer = loop {
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        match store.ask(id.as_str()).await {
            Ok(Some(a)) if a.answer.is_some() => break a.answer,
            _ => {}
        }
        // One read after the deadline too, so a last-moment answer is kept.
        if jiff::Timestamp::now() >= deadline {
            break match store.ask(id.as_str()).await {
                Ok(Some(a)) if a.answer.is_some() => a.answer,
                _ => None,
            };
        }
    };
    // A value nobody recognises is a lapse, never an allow.
    let behavior_of = |answer: Option<&serde_json::Value>| {
        answer.and_then(|a| {
            a.get("permission")
                .and_then(|v| v.as_str())
                .filter(|v| *v == "allow" || *v == "deny")
                .map(str::to_string)
        })
    };
    let mut behavior = behavior_of(answer.as_ref());
    if answer.is_none()
        && let Ok(Some(mut a)) = store.ask(id.as_str()).await
    {
        if a.is_open() {
            a.end(
                crate::core::ask::Ended::Nobody {
                    because: "the hold ran out and the agent's own dialog took over".into(),
                },
                jiff::Timestamp::now(),
            );
            // Compare-and-set: an answer landing since the last read wins.
            if !matches!(store.close_ask(&a).await, Ok(true))
                && let Ok(Some(now)) = store.ask(id.as_str()).await
            {
                behavior = behavior_of(now.answer.as_ref());
            }
        } else {
            behavior = behavior_of(a.answer.as_ref());
        }
    }
    // Read by the process that carries it back: only now has it been
    // delivered, and only now does the row say so.
    if behavior.is_some() {
        let _ = store
            .set_ask_delivery(id.as_str(), &crate::core::ask::Delivery::Live)
            .await;
    }
    behavior
}

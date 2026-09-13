//! The HTTP surface: receivers for the providers, and an API for the clients.
//!
//! One server, on loopback, for both. The browser shell needs HTTP anyway, and
//! a second transport for the CLI would mean two protocols to keep in step for
//! no gain. Everything under `/api` and `/vibeplane` requires the bearer token
//! from `~/.vibeplane/token`.

use crate::daemon::Shared;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event as SseEvent, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::time::Instant;
use vibeplane_core::Verdict;
use vibeplane_domain::event::{Event, Source};
use vibeplane_domain::ids::RunId;
use vibeplane_domain::run::RunMode;
use vibeplane_observe::hook::{HookPayload, PermissionResponse};

pub fn router(state: Shared) -> Router {
    Router::new()
        // Receivers. The paths carry the `/vibeplane/` marker that `connect`
        // uses to recognise its own entries when disconnecting.
        .route("/vibeplane/hook", post(hook))
        .route("/vibeplane/policy", post(policy))
        .route("/vibeplane/statusline", post(statusline))
        .route("/vibeplane/otel/v1/logs", post(otel_logs))
        .route("/vibeplane/otel/v1/metrics", post(otel_metrics))
        // Client API.
        .route("/api/board", get(board))
        .route("/api/inbox", get(inbox))
        .route("/api/runs/{id}", get(run_detail))
        .route("/api/runs/{id}/events", get(run_events))
        .route("/api/runs/{id}/snooze", post(snooze))
        .route("/api/runs/{id}/focus", post(focus_run))
        .route("/api/dispatch", post(dispatch))
        .route("/api/runs/{id}/prompt", post(prompt_run))
        .route("/api/runs/{id}/decide", post(decide_run))
        .route("/api/runs/{id}/stop", post(stop_run))
        .route("/api/agents", get(agents))
        .route("/api/search", get(search))
        .route("/api/diagnostics", get(diagnostics))
        .route("/api/stream", get(stream))
        .route("/healthz", get(|| async { "ok" }))
        .route("/", get(index))
        .with_state(state)
}

/// Checks the bearer token.
///
/// Loopback is not an access control: every process running as this user can
/// reach the port. The token, in a file only the user can read, is what
/// actually separates Vibeplane from everything else on the machine.
///
/// A `token` query parameter is accepted as well, because `EventSource` cannot
/// send a header and the browser shell needs the live stream. The page drops it
/// from the address bar as soon as it has it, so it does not end up in a
/// screenshot or a bookmark.
fn authorised(state: &Shared, headers: &HeaderMap, query: Option<&str>) -> bool {
    let from_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v));

    match from_header.or(query) {
        Some(t) => constant_time_eq(t.as_bytes(), state.token.as_bytes()),
        None => false,
    }
}

/// Compares without leaking the match position through timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

macro_rules! guard {
    ($state:expr, $headers:expr) => {
        if !authorised(&$state, &$headers, None) {
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorised"})))
                .into_response();
        }
    };
}

/// The board, embedded in the binary.
///
/// Deliberately unauthenticated: it is a static page that contains no data and
/// cannot fetch any without the token the user's browser holds. Gating it would
/// only mean the page could not render the message explaining that.
async fn index() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../ui/index.html"),
    )
}

/// Raises the editor window that owns a run, for the browser shell.
async fn focus_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let dir = {
        let w = state.world.lock().await;
        w.run(&RunId::new(id)).map(|r| r.working_dir().clone())
    };
    let Some(dir) = dir else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "no such run"}))).into_response();
    };
    match crate::focus::focus_path(&dir) {
        Ok(crate::focus::Focused::Editor { app, pid }) => {
            Json(json!({"focused": true, "app": app, "pid": pid})).into_response()
        }
        Ok(crate::focus::Focused::Nothing) => Json(json!({
            "focused": false,
            "reason": format!("no editor window has {} open", dir.display())
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Receivers
// ---------------------------------------------------------------------------

/// The lifecycle hook. Answers immediately and does the work after replying:
/// these hooks are configured `async`, but a receiver that blocks is still a
/// receiver that can make Claude feel slow if that ever changes.
async fn hook(State(state): State<Shared>, headers: HeaderMap, body: String) -> impl IntoResponse {
    guard!(state, headers);
    let started = Instant::now();

    let payload: HookPayload = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            state
                .store
                .record_channel(
                    "hook",
                    started.elapsed().as_micros() as u64,
                    Some(&e.to_string()),
                )
                .await
                .ok();
            // A malformed payload must not fail the hook: Claude Code would
            // show the user an error about a tool they did not misuse.
            return (StatusCode::OK, Json(json!({}))).into_response();
        }
    };

    let outcome = vibeplane_observe::hook::to_events(&payload);
    let run = RunId::new(payload.session_id.clone());
    for event in outcome.events {
        state
            .ingest(
                run.clone(),
                Source::Hook,
                event,
                payload.cwd.clone(),
                RunMode::Observed,
            )
            .await;
    }
    state
        .store
        .record_channel("hook", started.elapsed().as_micros() as u64, None)
        .await
        .ok();
    (StatusCode::OK, Json(json!({}))).into_response()
}

/// The permission gate.
///
/// This is the only synchronous hook, and it is the instant signal that a
/// session is blocked: the `permission_prompt` notification waits six seconds
/// and, in a terminal, defers again on every keystroke.
///
/// It answers in one of two ways and never waits for a human:
///
/// * a rule matches — allow or deny, the session continues, nothing reaches the
///   inbox;
/// * no rule matches — reply with no decision, so Claude Code prompts exactly
///   as it would have, and record that the run is blocked so the inbox knows.
async fn policy(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    guard!(state, headers);
    let started = Instant::now();

    let Ok(payload) = serde_json::from_str::<HookPayload>(&body) else {
        return (StatusCode::OK, Json(PermissionResponse::undecided())).into_response();
    };

    let tool = payload.tool_name.clone().unwrap_or_default();
    let input = payload
        .tool_input
        .clone()
        .unwrap_or(serde_json::Value::Null);
    let verdict = {
        let p = state.policy.lock().await;
        p.evaluate(&tool, &input)
    };

    let run = RunId::new(payload.session_id.clone());
    let (response, event) = match &verdict {
        Verdict::Allow { rule } => (
            PermissionResponse::allow(),
            Event::PermissionDecided {
                tool: tool.clone(),
                decision: "allow".into(),
                by: format!("policy:{rule}"),
            },
        ),
        Verdict::Deny { rule } => (
            PermissionResponse::deny(format!("denied by Vibeplane policy rule {rule}")),
            Event::PermissionDecided {
                tool: tool.clone(),
                decision: "deny".into(),
                by: format!("policy:{rule}"),
            },
        ),
        Verdict::Undecided => (
            PermissionResponse::undecided(),
            Event::Blocked {
                waiting_for: vibeplane_domain::event::WaitingFor::Permission,
                message: Some(describe(&tool, &input)),
                // An observed session's prompt belongs to Claude Code's own
                // dialog; Vibeplane can show it, not answer it.
                request_id: None,
            },
        ),
    };

    // Reply first, record second: the session is waiting on this response.
    let micros = started.elapsed().as_micros() as u64;
    let st = state.clone();
    let cwd = payload.cwd.clone();
    tokio::spawn(async move {
        st.ingest(run, Source::Hook, event, cwd, RunMode::Observed)
            .await;
        st.store.record_channel("policy", micros, None).await.ok();
    });

    (StatusCode::OK, Json(response)).into_response()
}

/// A one-line description of what is being asked for.
fn describe(tool: &str, input: &serde_json::Value) -> String {
    match vibeplane_core::policy::rule_content(tool, input) {
        Some(c) if c.len() > 120 => format!("{tool}: {}…", &c[..120]),
        Some(c) => format!("{tool}: {c}"),
        None => tool.to_string(),
    }
}

async fn statusline(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    guard!(state, headers);
    let Ok(payload) = serde_json::from_str::<vibeplane_observe::statusline::StatusPayload>(&body)
    else {
        return (StatusCode::OK, Json(json!({}))).into_response();
    };
    let run = RunId::new(payload.session_id.clone());
    for event in vibeplane_observe::statusline::to_events(&payload) {
        state
            .ingest(
                run.clone(),
                Source::StatusLine,
                event,
                None,
                RunMode::Observed,
            )
            .await;
    }
    (StatusCode::OK, Json(json!({}))).into_response()
}

/// OTLP/HTTP logs. Claude Code appends `/v1/logs` to the configured endpoint.
async fn otel_logs(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // The OTLP exporter cannot be given a bearer token per signal without
    // also sending it to every other collector the user configures, so the
    // telemetry endpoint is authorised by being on loopback alone. It accepts
    // only observations, never commands.
    let _ = headers;
    let started = Instant::now();
    let records = match vibeplane_observe::otel::parse_logs(&body) {
        Ok(r) => r,
        Err(e) => {
            state
                .store
                .record_channel(
                    "otel",
                    started.elapsed().as_micros() as u64,
                    Some(&e.to_string()),
                )
                .await
                .ok();
            return (StatusCode::OK, Json(json!({"partialSuccess": {}}))).into_response();
        }
    };
    for rec in records {
        let run = RunId::new(rec.session_id.clone());
        state
            .ingest(
                run.clone(),
                Source::Otel,
                rec.event,
                None,
                RunMode::Observed,
            )
            .await;
        if let Some(entry) = rec.entrypoint {
            let mut w = state.world.lock().await;
            w.set_entrypoint(&run, &entry, rec.repo_url.as_deref());
        }
    }
    state
        .store
        .record_channel("otel", started.elapsed().as_micros() as u64, None)
        .await
        .ok();
    (StatusCode::OK, Json(json!({"partialSuccess": {}}))).into_response()
}

async fn otel_metrics(State(state): State<Shared>, body: axum::body::Bytes) -> impl IntoResponse {
    let n = vibeplane_observe::otel::parse_metrics_sessions(&body).len();
    state
        .store
        .record_channel("otel_metrics", n as u64, None)
        .await
        .ok();
    (StatusCode::OK, Json(json!({"partialSuccess": {}})))
}

// ---------------------------------------------------------------------------
// Client API
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct BoardResponse {
    summary: vibeplane_core::BoardSummary,
    runs: Vec<RunView>,
    projects: Vec<vibeplane_domain::Project>,
}

/// What the board shows for one run. A view rather than the `Run` itself, so
/// that adding a field to the domain does not silently change the wire format.
#[derive(Serialize)]
pub struct RunView {
    pub id: String,
    pub project: Option<String>,
    pub project_name: Option<String>,
    pub agent: String,
    pub mode: String,
    pub state: String,
    pub waiting_for: Option<String>,
    pub cwd: String,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub entrypoint: Option<String>,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub cost_usd: f64,
    pub context_percent: Option<f64>,
    pub tool_calls: u64,
    pub subagents: usize,
    pub idle_seconds: i64,
    pub last_event_at: String,
}

impl RunView {
    fn of(run: &vibeplane_domain::Run, project_name: Option<String>) -> Self {
        Self {
            id: run.id.to_string(),
            project: run.project_id.as_ref().map(|p| p.to_string()),
            project_name,
            agent: run.agent.clone(),
            mode: run.mode.as_str().into(),
            state: run.state.as_str().into(),
            waiting_for: match &run.state {
                vibeplane_domain::RunState::Waiting(w) => Some(format!("{w:?}").to_lowercase()),
                _ => None,
            },
            cwd: run.cwd.display().to_string(),
            worktree: run.worktree.as_ref().map(|p| p.display().to_string()),
            branch: run.branch.clone(),
            model: run.model.clone(),
            entrypoint: run.entrypoint.clone(),
            name: run.name.clone(),
            summary: run.summary.clone(),
            cost_usd: run.totals.cost_usd,
            context_percent: run.totals.context_percent(),
            tool_calls: run.totals.tool_calls,
            subagents: run.subagents.len(),
            idle_seconds: run.idle_seconds(),
            last_event_at: run.last_event_at.to_string(),
        }
    }
}

async fn board(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let w = state.world.lock().await;
    let runs = w
        .board()
        .into_iter()
        .map(|r| {
            let name = r
                .project_id
                .as_ref()
                .and_then(|p| w.project(p))
                .map(|p| p.name.clone());
            RunView::of(r, name)
        })
        .collect();
    Json(BoardResponse {
        summary: w.summary(),
        runs,
        projects: w.projects().cloned().collect(),
    })
    .into_response()
}

async fn inbox(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let w = state.world.lock().await;
    Json(w.inbox()).into_response()
}

async fn run_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let w = state.world.lock().await;
    match w.run(&RunId::new(id)) {
        Some(r) => Json(r.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "no such run"}))).into_response(),
    }
}

#[derive(Deserialize)]
struct LimitQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 {
    200
}

async fn run_events(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    match state.store.events_for_run(&RunId::new(id), q.limit).await {
        Ok(evs) => Json(evs).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Starting an agent.
#[derive(Deserialize)]
struct DispatchBody {
    /// An agent id from `/api/agents`, or a command line.
    agent: String,
    /// Where it runs. The project must be trusted: a headless agent executes
    /// the repository's own hooks and MCP servers with no dialog of its own.
    cwd: String,
    #[serde(default)]
    prompt: Option<String>,
}

async fn dispatch(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<DispatchBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let Some(spec) = vibeplane_acp::resolve(&body.agent, &[]) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("unknown agent `{}`", body.agent)})),
        )
            .into_response();
    };
    match crate::driven::dispatch(&state, &spec, body.cwd.into(), body.prompt).await {
        Ok(run) => Json(json!({"run_id": run.to_string(), "agent": spec.id})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PromptBody {
    text: String,
}

async fn prompt_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<PromptBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::driven::prompt(&state, &RunId::new(id), body.text).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct DecideBody {
    request_id: String,
    /// The option to choose. Absent refuses, which is also what an unanswered
    /// request becomes when its deadline passes.
    #[serde(default)]
    option_id: Option<String>,
}

async fn decide_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<DecideBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::driven::decide(&state, &RunId::new(id), &body.request_id, body.option_id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn stop_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::driven::stop(&state, &RunId::new(id)).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn agents(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    Json(vibeplane_acp::builtin()).into_response()
}

#[derive(Deserialize)]
struct StreamQuery {
    #[serde(default)]
    token: Option<String>,
}

#[derive(Deserialize)]
struct SnoozeQuery {
    /// Minutes to stay quiet. Zero un-snoozes, which is the only way back.
    #[serde(default = "default_snooze")]
    minutes: i64,
}
fn default_snooze() -> i64 {
    60
}

/// Hides a run's inbox items for a while.
///
/// Snoozing is per run rather than per item: "not this one, not now" is the
/// thought a person actually has, and a run that is being ignored deliberately
/// should not keep producing new reasons to look at it.
async fn snooze(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<SnoozeQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let until = (q.minutes > 0)
        .then(|| jiff::Timestamp::now() + jiff::SignedDuration::from_mins(q.minutes));

    let run_id = RunId::new(id);
    let saved = {
        let mut w = state.world.lock().await;
        if !w.snooze(&run_id, until) {
            None
        } else {
            w.run(&run_id).cloned()
        }
    };

    let Some(run) = saved else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "no such run"}))).into_response();
    };
    // Persisted, so a snooze survives a restart. A quiet run that starts
    // shouting again because the daemon bounced is a broken promise.
    if let Err(e) = state.store.save_run(&run).await {
        tracing::warn!(error = %e, "could not persist snooze");
    }
    let _ = state.tx.send(vibeplane_domain::event::EventEnvelope::new(
        run_id,
        Source::Daemon,
        Event::StatusSample {
            context_used_percent: None,
            rate_limit_five_hour: None,
            rate_limit_seven_day: None,
            session_name: None,
        },
    ));
    Json(json!({"snoozed_until": until.map(|t| t.to_string())})).into_response()
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: i64,
}

async fn search(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    match state.store.search(&q.q, q.limit).await {
        Ok(hits) => Json(
            hits.into_iter()
                .map(|(r, t)| json!({"run_id": r.to_string(), "text": t}))
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn diagnostics(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let channels = state.store.channel_health().await.unwrap_or_default();
    let w = state.world.lock().await;
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
        "started_at": state.started_at.to_string(),
        "uptime_seconds": (jiff::Timestamp::now() - state.started_at).get_seconds(),
        "summary": w.summary(),
        "channels": channels,
        "stall_seconds": w.attention.stall_seconds,
    }))
    .into_response()
}

/// Live events, as server-sent events. The browser shell and `vibeplane watch`
/// use the same stream.
///
/// A subscriber that falls behind is skipped forward rather than disconnected:
/// a burst of tool calls must not knock the UI off the stream.
async fn stream(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<StreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, StatusCode> {
    // An empty stream would leak nothing, but it would also be indistinguishable
    // from a quiet machine — so a bad token says so.
    if !authorised(&state, &headers, q.token.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let rx = state.tx.subscribe();

    let stream = futures_util::stream::unfold(Some(rx), |rx| async move {
        let mut rx = rx?;
        loop {
            match rx.recv().await {
                Ok(env) => {
                    let data = serde_json::to_string(&env).unwrap_or_default();
                    return Some((Ok(SseEvent::default().data(data)), Some(rx)));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::debug!(skipped = n, "subscriber lagged");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

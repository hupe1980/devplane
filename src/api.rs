//! The HTTP surface: receivers for the providers, and an API for the clients.
//!
//! One server, on loopback, for both. The browser shell needs HTTP anyway, and
//! a second transport for the CLI would mean two protocols to keep in step for
//! no gain. Everything under `/api` and `/devplane` requires the bearer token
//! from `~/.devplane/token`.

use crate::core::event::Source;
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::daemon::Shared;
use crate::observe::hook::HookPayload;
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

pub fn router(state: Shared) -> Router {
    Router::new()
        // Receivers. The paths carry the `/devplane/` marker that `connect`
        // uses to recognise its own entries when disconnecting.
        .route("/devplane/hook", post(hook))
        // Where the `command` hook files what it has **already decided**.
        // This endpoint does not decide: the process that enforced a verdict is
        // the authority for what was enforced, and a second evaluation here
        // could disagree with it — writing down a rule other than the one that
        // fired is the same failure as naming no rule at all.
        .route("/devplane/decided", post(decided))
        .route("/devplane/statusline", post(statusline))
        .route("/devplane/otel/v1/logs", post(otel_logs))
        .route("/devplane/otel/v1/metrics", post(otel_metrics))
        // The second dialect. Claude Code exports log records; agents that
        // follow the GenAI semantic conventions — GitHub Copilot, Codex —
        // export traces, on the same transport and the same `http/json` wire
        // protocol. One receiver, two readers, one event model.
        .route("/devplane/otel/v1/traces", post(otel_traces))
        // GitHub Copilot's channels. Its deciding hook is a `command` one
        // — an HTTP `preToolUse` hook there fails open — so the gate arrives
        // through the shim rather than from the agent directly.
        .route("/devplane/copilot/hook", post(copilot_hook))
        // Telemetry arrives in batches and axum's default body limit is 2 MB,
        // which a busy session can exceed. A rejected batch is not an error
        // anybody sees: the exporter drops it, the receiver is never called,
        // and the only symptom is a board that is quietly missing cost. This
        // layer's whole promise is that what it shows is what happened, so the
        // limit is set deliberately rather than inherited — generous for a
        // batch, and still a bound, because the endpoint takes no credential.
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024 * 1024))
        // Client API.
        .route("/api/board", get(board))
        .route("/api/inbox", get(inbox))
        .route("/api/runs/{id}", get(run_detail))
        .route("/api/runs/{id}/events", get(run_events))
        .route("/api/runs/{id}/rewind-gap", get(run_rewind_gap))
        .route("/api/runs/{id}/messages", get(run_messages))
        .route("/api/runs/{id}/snooze", post(snooze))
        .route("/api/runs/{id}/focus", post(focus_run))
        .route("/api/dispatch", post(dispatch))
        .route("/api/runs/{id}/prompt", post(prompt_run))
        .route("/api/runs/{id}/decide", post(decide_run))
        .route("/api/runs/{id}/stop", post(stop_run))
        .route("/api/agents", get(agents))
        .route("/api/work", get(list_work).post(start_work))
        .route("/api/work/{id}/verify", post(verify_work))
        .route("/api/work/{id}/finish", post(finish_work))
        .route("/api/work/{id}/approve", post(approve_work))
        .route("/api/work/{id}/retry", post(retry_work))
        .route("/api/work/{id}/snooze", post(snooze_work))
        .route("/api/work/{id}/changes", get(work_changes))
        .route("/api/work/{id}/resume", post(resume_work))
        .route("/api/projects", get(projects))
        .route("/api/projects/trust", post(trust_project))
        .route("/api/projects/{id}/snooze", post(snooze_project))
        .route("/api/issues", post(list_issues))
        .route("/api/forge", get(forge))
        .route("/api/decisions", get(decisions))
        .route("/api/explain", get(explain))
        .route("/api/search", get(search))
        .route("/api/diagnostics", get(diagnostics))
        .route("/api/setup", get(setup))
        .route("/api/attention", get(attention))
        .route("/api/shutdown", post(shutdown))
        .route("/api/stream", get(stream))
        // Open, and it names the version: a client checks it before every
        // command and restarts a daemon older than itself, because a stale
        // daemon answers 404 to routes this client believes exist.
        .route(
            "/healthz",
            get(|| async { concat!("ok ", env!("CARGO_PKG_VERSION")) }),
        )
        .route("/", get(index))
        .with_state(state)
}

/// A status and a body, for a request that named something unresolvable.
///
/// Deliberately not a built `Response`: that type is large enough that carrying
/// one in every `Result::Err` is a clippy warning and a real cost on the happy
/// path. It becomes a response at the point of return.
type Refusal = (StatusCode, Json<serde_json::Value>);

/// Turns the id a person typed into the run they meant.
///
/// The board prints a short label, so that is what users copy. Requiring the
/// whole id everywhere made the only identifier anyone sees the one identifier
/// nothing accepts — see [`World::resolve_run`](crate::core::World::resolve_run).
/// Ambiguity is a 409 naming the candidates, never a guess.
async fn resolved_run(state: &Shared, id: String) -> Result<RunId, Refusal> {
    let world = state.world.lock().await;
    world.resolve_run(&id).map_err(|e| {
        let code = match e {
            crate::core::Ambiguous::NotFound => StatusCode::NOT_FOUND,
            crate::core::Ambiguous::Several(_) => StatusCode::CONFLICT,
        };
        (code, Json(json!({"error": e.to_string()})))
    })
}

/// The same, for work. Work ids are long enough that nobody types one in full,
/// and `devplane work ls` prints them clipped.
async fn resolved_work(state: &Shared, id: String) -> Result<crate::core::WorkId, Refusal> {
    let works = state.works.lock().await;
    if works.contains_key(&crate::core::WorkId::new(id.clone())) {
        return Ok(crate::core::WorkId::new(id));
    }
    let mut hits: Vec<&crate::core::WorkId> = works
        .keys()
        .filter(|w| w.as_str().starts_with(&id))
        .collect();
    match hits.len() {
        1 => Ok(hits.remove(0).clone()),
        0 => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no such work"})),
        )),
        n => Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": format!("that matches {n} pieces of work. Use more of the id.")
            })),
        )),
    }
}

macro_rules! work_id {
    ($state:expr, $id:expr) => {
        match resolved_work(&$state, $id).await {
            Ok(w) => w,
            Err(refusal) => return refusal.into_response(),
        }
    };
}

macro_rules! run_id {
    ($state:expr, $id:expr) => {
        match resolved_run(&$state, $id).await {
            Ok(r) => r,
            Err(refusal) => return refusal.into_response(),
        }
    };
}

/// Checks the bearer token.
///
/// Loopback is not an access control: every process running as this user can
/// reach the port. The token, in a file only the user can read, is what
/// actually separates Devplane from everything else on the machine.
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
/// The board.
///
/// Embedded, so the binary is the whole product and the page works on a laptop
/// with no network. `DEVPLANE_UI` points at the file on disk instead, which is
/// the difference between a one-second edit-reload loop and a rebuild plus a
/// daemon restart for every line of CSS. Development only: it reads a file the
/// user named, so it is opt-in by an environment variable rather than a
/// setting, and it falls back to the embedded copy rather than failing.
async fn index() -> impl IntoResponse {
    const EMBEDDED: &str = include_str!("../ui/index.html");
    let body = match std::env::var_os("DEVPLANE_UI") {
        Some(path) => std::fs::read_to_string(&path).unwrap_or_else(|e| {
            tracing::warn!(path = ?path, error = %e, "DEVPLANE_UI is set and unreadable; serving the embedded page");
            EMBEDDED.to_string()
        }),
        None => EMBEDDED.to_string(),
    };
    (
        [
            (axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8"),
            // The page is the binary's own; a stale one is a debugging session
            // spent on a bug that was fixed.
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
}

/// Raises the editor window that owns a run, for the browser shell.
async fn focus_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = run_id!(state, id);
    let dir = {
        let w = state.world.lock().await;
        w.run(&id).map(|r| r.working_dir().clone())
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

/// Records a decision the `command` hook has already enforced, and ingests the
/// observation that came with it.
///
/// **The verdict arrives; it is not recomputed.** The hook decides in its own
/// process so that a stopped daemon cannot silently disable the gate, which
/// leaves this endpoint one job: write down what happened. Re-deciding here
/// would put a rule in the log that did not fire, one process further out: the
/// daemon's cached rules can be seconds behind the file the hook just read, so
/// the log could name a rule that did not fire.
///
/// The same envelope is what `~/.devplane/pending-decisions.jsonl` holds, so
/// a decision taken with no daemon and one taken with a daemon are written down
/// by the same code.
async fn decided(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    guard!(state, headers);
    let started = Instant::now();
    let Ok(env) = serde_json::from_str::<crate::core::DecidedEnvelope>(&body) else {
        return (StatusCode::OK, Json(json!({}))).into_response();
    };
    record_decided(&state, env).await;
    state
        .store
        .record_channel("policy", started.elapsed().as_micros() as u64, None)
        .await
        .ok();
    (StatusCode::OK, Json(json!({}))).into_response()
}

/// One envelope, written down. Shared by the live path and the spool drain.
pub async fn record_decided(state: &Shared, env: crate::core::DecidedEnvelope) {
    // The gate is run against a probe call by `doctor` and by the daemon's own
    // timer, and the hook already declines to report those. Declined *here*
    // too, because an older hook binary — or a spool written by one — can
    // still deliver it, and it arrived once: a `devplane-probe-<pid>` project
    // with a working session on the board and two rows in the audit log.
    if env.session == crate::observe::hook::PROBE_SESSION {
        return;
    }
    let run = RunId::new(env.session.clone());
    let subject = env.subject.clone();

    if let Some(rule) = env.rule.as_deref() {
        let mut d = crate::core::Decision::new(
            crate::core::Actor::Policy,
            "agent:tool.use",
            subject,
            &env.verdict,
        )
        .because(if env.late {
            // Saying so matters: a reader comparing the log against a session
            // transcript would otherwise find the rows in the wrong order and
            // conclude the log was unreliable.
            format!("{rule} (filed late: no daemon was running)")
        } else {
            rule.to_string()
        })
        .by_tool(env.tool.clone())
        .for_run(&run);
        if let Some(at) = env.at {
            d = d.at(at);
        }
        state.record(d).await;
    }

    // The observation half, so the board shows the call whether or not a rule
    // had an opinion about it — and so a question reaches the inbox the instant
    // it is asked rather than six seconds later on a notification.
    let cwd = env
        .payload
        .as_ref()
        .and_then(|p| p.get("cwd"))
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from);
    let mut events = Vec::new();
    if let Some(payload) = env.payload.clone()
        && let Ok(p) = serde_json::from_value::<HookPayload>(payload)
    {
        events.extend(crate::observe::hook::to_events(&p).events);
    }
    // A permission nobody had a rule about is a run that is **blocked**, and
    // the inbox learns it from here. A decided one is not: it never reached a
    // person. The gate reports which it was; this only writes it down.
    match env.verdict.as_str() {
        "allow" | "deny" => events.push(crate::core::event::Event::PermissionDecided {
            tool: env.tool.clone(),
            decision: env.verdict.clone(),
            by: env
                .rule
                .as_deref()
                .map(|r| format!("policy:{r}"))
                .unwrap_or_else(|| "policy".into()),
        }),
        _ if env.blocked => events.push(crate::core::event::Event::Blocked {
            waiting_for: crate::core::event::WaitingFor::Permission,
            message: Some(env.subject.clone()),
            // An observed session's prompt belongs to Claude Code's own dialog;
            // Devplane can show it, not answer it.
            request_id: None,
            options: Vec::new(),
            // **The gate knows the call and the tool hook may never have
            // fired.** Claude Code resolves permission before invoking the
            // tool, so recovering this from an in-flight `ToolStarted` finds
            // nothing — which is what made the rule offered on a permission
            // item dead code for every watched session.
            call: env
                .payload
                .as_ref()
                .and_then(|p| p.get("tool_input"))
                .map(|input| crate::core::event::ToolCallRef {
                    tool: env.tool.clone(),
                    input: input.clone(),
                }),
        }),
        _ => {}
    }
    for event in events {
        state
            .ingest(
                run.clone(),
                Source::Hook,
                event,
                cwd.clone(),
                RunMode::Observed,
            )
            .await;
    }
}

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

    if payload.session_id == crate::observe::hook::PROBE_SESSION {
        // A probe of the gate, never a session. See `record_decided`.
        return (StatusCode::OK, Json(json!({}))).into_response();
    }
    let outcome = crate::observe::hook::to_events(&payload);
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

async fn statusline(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    guard!(state, headers);
    let Ok(payload) = serde_json::from_str::<crate::observe::statusline::StatusPayload>(&body)
    else {
        return (StatusCode::OK, Json(json!({}))).into_response();
    };
    let run = RunId::new(payload.session_id.clone());
    for event in crate::observe::statusline::to_events(&payload) {
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

/// One GitHub Copilot lifecycle payload. Informational only.
///
/// The event name comes from the query string because Copilot's native
/// payloads do not carry one — the registration knows which event it wrote,
/// and that is the only place the answer exists.
async fn copilot_hook(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<CopilotEvent>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if !authorised(&state, &headers, None) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "bad token"})),
        )
            .into_response();
    }
    let started = Instant::now();
    let mut payload: crate::observe::copilot::HookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            // Answered 200 and dropped, like every other receiver here. An
            // observer has no business making the work it observes fail.
            state
                .store
                .record_channel("copilot", 0, Some(&e.to_string()))
                .await
                .ok();
            return (StatusCode::OK, Json(json!({}))).into_response();
        }
    };
    payload.event = q.event.unwrap_or_default();
    let run = RunId::new(payload.session_id.clone());
    let cwd = payload.cwd.clone();
    for event in crate::observe::copilot::to_events(&payload) {
        state
            .ingest(
                run.clone(),
                Source::Hook,
                event,
                cwd.clone(),
                RunMode::Observed,
            )
            .await;
    }
    state
        .store
        .record_channel("copilot", started.elapsed().as_micros() as u64, None)
        .await
        .ok();
    (StatusCode::OK, Json(json!({}))).into_response()
}

#[derive(serde::Deserialize)]
struct CopilotEvent {
    event: Option<String>,
}

/// OTLP/HTTP logs. Claude Code appends `/v1/logs` to the configured endpoint.
///
/// The OTLP exporter cannot be given a bearer token per signal without also
/// sending it to every other collector the user configures, so the telemetry
/// endpoints are authorised by being on loopback alone. They accept only
/// observations, never commands.
async fn otel_logs(State(state): State<Shared>, body: axum::body::Bytes) -> impl IntoResponse {
    ingest_otel(state, &body, crate::observe::otel::parse_logs).await
}

/// OTLP/HTTP traces, written to the GenAI semantic conventions.
async fn otel_traces(State(state): State<Shared>, body: axum::body::Bytes) -> impl IntoResponse {
    ingest_otel(state, &body, crate::observe::otel::parse_traces).await
}

async fn ingest_otel(
    state: Shared,
    body: &[u8],
    parse: fn(&[u8]) -> Result<Vec<crate::observe::otel::OtelRecord>, serde_json::Error>,
) -> axum::response::Response {
    let started = Instant::now();
    let records = match parse(body) {
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
    let n = crate::observe::otel::parse_metrics_sessions(&body).len();
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
    summary: crate::core::BoardSummary,
    /// The working set by default; everything when `?all=true`.
    runs: Vec<RunView>,
    projects: Vec<crate::core::Project>,
    /// Per project id: open issues, open pull requests, and how many of them
    /// are waiting on the person. Absent for a project with no forge.
    forge: std::collections::BTreeMap<String, crate::core::ForgeCounts>,
    /// How stale the gate's measurement is, **when it is stale at all**.
    ///
    /// `None` when no session reports a version, and `None` when every one that
    /// does is at or below the release the matrix ran against. A field that is
    /// always present would put a number on the board that is usually zero, and
    /// a number nobody can act on is noise — which is this product's own rule
    /// about its own inbox, applied to itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    gate_behind: Option<GateGap>,
}

/// The gap between the release the gate was measured against and the newest one
/// actually running here.
#[derive(Serialize)]
struct GateGap {
    measured: &'static str,
    running: String,
    /// `None` across a minor or major boundary, where nothing here can count
    /// releases and a number would be a guess.
    #[serde(skip_serializing_if = "Option::is_none")]
    releases: Option<u64>,
}

#[derive(Deserialize)]
struct BoardQuery {
    /// Include sessions that exist but have never reported anything.
    #[serde(default)]
    all: bool,
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
    /// How much of the tightest subscription window is used, where the
    /// status-line shim is installed. The only channel that carries it.
    pub rate_limit_percent: Option<f64>,
    pub tool_calls: u64,
    pub subagents: usize,
    /// Whether this session has ever reported anything itself.
    pub reporting: bool,
    pub idle_seconds: i64,
    pub last_event_at: String,
    /// What the agent says it is going to do, where it reports a plan.
    pub plan: Vec<crate::core::PlanStep>,
    /// How far through it, for the one-line board row.
    pub plan_done: usize,
    pub plan_total: usize,
}

impl RunView {
    fn of(run: &crate::core::Run, project_name: Option<String>) -> Self {
        Self {
            id: run.id.to_string(),
            project: run.project_id.as_ref().map(|p| p.to_string()),
            project_name,
            agent: run.agent.clone(),
            mode: run.mode.as_str().into(),
            state: run.state.as_str().into(),
            waiting_for: match &run.state {
                crate::core::RunState::Waiting(w) => Some(format!("{w:?}").to_lowercase()),
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
            rate_limit_percent: run.totals.rate_limit_percent,
            tool_calls: run.totals.tool_calls,
            subagents: run.subagents.len(),
            reporting: run.reporting,
            idle_seconds: run.idle_seconds(),
            last_event_at: run.last_event_at.to_string(),
            plan_done: run.plan_progress().map(|(d, _)| d).unwrap_or(0),
            plan_total: run.plan.len(),
            plan: run.plan.clone(),
        }
    }
}

async fn board(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<BoardQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let forge = state.forge.lock().await.counts();
    let w = state.world.lock().await;
    let runs = if q.all { w.board() } else { w.working_set() };
    let runs = runs
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
    let mut summary = w.summary();
    for c in forge.values() {
        summary.open_issues += c.issues;
        summary.open_prs += c.pull_requests;
        summary.forge_needs_you += c.needs_you;
    }
    // The newest release any session here reports, when it is ahead of the one
    // the matrix ran against. Only the status-line shim reports a version, so
    // absence means *nothing is telling us*, never *there is no gap*.
    let gate_behind = w
        .runs()
        .filter_map(|r| r.claude_version.as_deref())
        .filter(|v| crate::core::policy::is_ahead_of_baseline(v))
        .max()
        .map(|running| GateGap {
            measured: crate::core::policy::VERIFIED_AGAINST,
            releases: crate::core::policy::releases_ahead(
                running,
                crate::core::policy::VERIFIED_AGAINST,
            ),
            running: running.to_string(),
        });
    Json(BoardResponse {
        summary,
        runs,
        gate_behind,
        projects: w.projects().cloned().collect(),
        forge: forge
            .into_iter()
            .map(|(id, c)| (id.to_string(), c))
            .collect(),
    })
    .into_response()
}

async fn inbox(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    Json(state.current_inbox().await).into_response()
}

/// One row of the cross-project issue or pull-request list.
#[derive(Serialize)]
struct ForgeRow<T: Serialize> {
    project: String,
    project_name: String,
    #[serde(flatten)]
    item: T,
}

/// Everything the forge says, for every registered project, from the last
/// poll: the open issues and the open pull requests in one answer.
///
/// One route rather than two, because both halves share an envelope — whose
/// `gh` this is, why the last poll failed, when it ran. Splitting them costs
/// the board a second round trip and lets one page show issues read at 10:42
/// beside pull requests read at 10:47 under a single `fetched_at`.
/// `devplane issues` and `devplane prs` each read the half they print.
///
/// Ordered by what needs the person, then by project, then newest first —
/// the order somebody with eight repositories actually wants.
async fn forge(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let names: std::collections::BTreeMap<_, _> = {
        let w = state.world.lock().await;
        w.projects()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect()
    };
    let f = state.forge.lock().await;
    let row = |pf: &crate::core::ProjectForge| {
        let name = names.get(&pf.project_id).cloned().unwrap_or_default();
        (pf.project_id.to_string(), name)
    };

    let mut issues: Vec<ForgeRow<crate::core::ForgeIssue>> = f
        .projects
        .values()
        .flat_map(|pf| {
            let (project, project_name) = row(pf);
            pf.issues.iter().cloned().map(move |item| ForgeRow {
                project: project.clone(),
                project_name: project_name.clone(),
                item,
            })
        })
        .collect();
    issues.sort_by(|a, b| {
        b.item
            .assigned_to_me
            .cmp(&a.item.assigned_to_me)
            .then_with(|| a.project_name.cmp(&b.project_name))
            .then_with(|| b.item.updated_at.cmp(&a.item.updated_at))
    });

    let mut prs: Vec<ForgeRow<crate::core::ForgePullRequest>> = f
        .projects
        .values()
        .flat_map(|pf| {
            let (project, project_name) = row(pf);
            pf.pull_requests.iter().cloned().map(move |item| ForgeRow {
                project: project.clone(),
                project_name: project_name.clone(),
                item,
            })
        })
        .collect();
    prs.sort_by(|a, b| {
        b.item
            .needs_me()
            .cmp(&a.item.needs_me())
            .then_with(|| a.project_name.cmp(&b.project_name))
            .then_with(|| b.item.updated_at.cmp(&a.item.updated_at))
    });

    // Which projects could not be read, and why. The counts on the board are
    // the last good ones for those, so the view has to be able to say which
    // numbers are old — otherwise stale reads exactly like fresh.
    let stale: Vec<_> = f
        .projects
        .values()
        .filter_map(|pf| {
            pf.error.as_ref().map(|e| {
                json!({
                    "project": pf.project_id.to_string(),
                    "project_name": names.get(&pf.project_id).cloned().unwrap_or_default(),
                    "error": e,
                    "last_good": pf.fetched_at.to_string(),
                })
            })
        })
        .collect();

    Json(json!({
        "viewer": f.viewer,
        "error": f.error,
        "fetched_at": f.last_poll_at.map(|t| t.to_string()),
        "issues": issues,
        "pull_requests": prs,
        "stale": stale,
    }))
    .into_response()
}

/// Hides a project's forge items for a while. Per kind, like every other
/// snooze here: dismissing a review request must not also swallow the issue
/// that gets assigned an hour later.
async fn snooze_project(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<SnoozeQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let until = (q.minutes > 0)
        .then(|| jiff::Timestamp::now() + jiff::SignedDuration::from_mins(q.minutes));
    let project = crate::core::ProjectId::new(id);
    let dismissed: Vec<crate::core::AttentionKind> = {
        let mut f = state.forge.lock().await;
        let Some(pf) = f.projects.get(&project).cloned() else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "no forge for that project"})),
            )
                .into_response();
        };
        let none = crate::core::attention::Snoozed::default();
        let kinds: Vec<_> = crate::core::forge::items_for_forge(
            &pf,
            f.snoozed.get(&project).unwrap_or(&none),
            &Default::default(),
        )
        .into_iter()
        .map(|i| i.kind)
        .collect();
        let entry = f.snoozed.entry(project.clone()).or_default();
        match until {
            Some(t) => entry.hide(kinds.clone(), t),
            None => entry.clear(),
        }
        kinds
    };
    for kind in &dismissed {
        let id = format!("gh:{}:{}", project.as_str(), kind.as_str());
        // The forge ids carry the number too; resolve every item of that kind
        // for the project, which is what the person dismissed.
        state
            .store
            .attention_resolve_prefix(&id, crate::core::attention::Resolution::Dismissed)
            .await
            .ok();
    }
    state.notify_changed();
    Json(json!({"snoozed_until": until.map(|t| t.to_string()), "kinds": dismissed})).into_response()
}

async fn run_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = run_id!(state, id);
    let w = state.world.lock().await;
    match w.run(&id) {
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
    let id = run_id!(state, id);
    match state.store.events_for_run(&id, q.limit).await {
        Ok(evs) => Json(evs).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// What the gate would decide about one call, and which rule decides it.
///
/// The same evaluation `devplane explain` does, with one difference that is
/// the whole reason this endpoint exists: **it leaves a row.** A read-only
/// interrogation is also a way to probe for a command the rules happen to
/// allow, and a gate that answers questions should be able to say it was asked.
///
/// The CLI's `explain` stays offline and unrecorded, and the asymmetry is
/// deliberate: a person at a terminal asking *what would this do* is not the
/// party the rules govern. An agent asking through MCP is.
#[derive(Deserialize)]
struct ExplainQuery {
    call: String,
    #[serde(default = "default_tool")]
    tool: String,
    #[serde(default = "default_dir")]
    dir: String,
    /// Who asked. Recorded, so the log distinguishes a probe from a person.
    #[serde(default)]
    asked_by: Option<String>,
}
fn default_tool() -> String {
    "Bash".into()
}
fn default_dir() -> String {
    ".".into()
}

async fn explain(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<ExplainQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let Ok(dir) = std::path::PathBuf::from(&q.dir).canonicalize() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{} does not exist", q.dir)})),
        )
            .into_response();
    };
    let Some(field) = crate::core::policy::rule_content_field(&q.tool) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("`{}` takes no plain specifier", q.tool)})),
        )
            .into_response();
    };
    let input = json!({ field: q.call });

    // The gate this machine enforces, machine-wide rules included — not a
    // project-only imitation that would answer `allow` for a call the machine
    // denies.
    let (cache, _) = crate::core::PolicyCache::from_disk();
    let verdict = cache.evaluate(&dir, &q.tool, &input);
    let restrictive = cache.restrictive(&dir, &q.tool, &input);

    let asked_by = q.asked_by.as_deref().unwrap_or("api");
    state
        .record(
            crate::core::Decision::new(
                crate::core::Actor::Policy,
                "policy:explain",
                format!("{}: {}", q.tool, q.call),
                verdict.as_str(),
            )
            .because(match verdict.rule() {
                Some(r) => format!("{r} (asked by {asked_by}, nothing ran)"),
                None => format!("no rule answers it (asked by {asked_by}, nothing ran)"),
            }),
        )
        .await;

    Json(json!({
        "dir": dir.display().to_string(),
        "tool": q.tool,
        "call": q.call,
        "verdict": verdict.as_str(),
        "rule": verdict.rule(),
        // What a `PreToolUse` hook would answer, which is not the same thing:
        // that hook may never grant, so a call this says `allow` for still
        // reaches the vendor's own permission system.
        "before_the_tool_runs": restrictive.as_str(),
        "nothing_ran": true,
    }))
    .into_response()
}

/// Files a shell call in this run named for writing — the class Claude Code's
/// own checkpointing documents that it does not cover.
///
/// A read over the decision log and nothing else. The wording in every surface
/// downstream of this is *named for writing* rather than *changed*, because a
/// `PreToolUse` record is a call the gate saw before the tool ran.
async fn run_rewind_gap(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = run_id!(state, id);
    // Every decision for the run: the gap is about the whole session, and a
    // limit here would silently answer about part of it.
    match state.store.decisions(Some(id.as_str()), i64::MAX).await {
        Ok(rows) => Json(json!({
            "run": id.to_string(),
            "files": crate::core::decision::files_a_shell_call_named_for_writing(&rows),
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// What a driven agent said, oldest first.
///
/// Empty for a session Devplane only watches, and that is not a gap to fill
/// later: hooks carry lifecycle and tool inputs, OpenTelemetry redacts prompts
/// and responses and Devplane never sets the flag that would change it, and
/// the transcript files are documented as internal. For those runs the honest
/// action is `focus` — the text is already on screen in the window that owns it.
async fn run_messages(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = run_id!(state, id);
    match state.store.messages_for_run(&id, q.limit).await {
        Ok(m) => Json(m).into_response(),
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
    let Some(spec) = crate::acp::resolve(&body.agent, &state.agents) else {
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
    let id = run_id!(state, id);
    match crate::driven::prompt(&state, &id, body.text).await {
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
    /// `allow` or `deny`, resolved against the options the agent offered.
    #[serde(default)]
    decision: Option<String>,
    /// An exact option id, for a caller that read one off the inbox item.
    /// Wins over `decision` when both are given.
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
    let want = crate::driven::Decision::parse(body.decision.as_deref(), body.option_id);
    let id = run_id!(state, id);
    match crate::driven::decide(&state, &id, &body.request_id, want).await {
        Ok(()) => {
            acted_on(
                &state,
                id.as_str(),
                &[
                    crate::core::AttentionKind::Permission,
                    crate::core::AttentionKind::Question,
                ],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(json!({"ok": true})).into_response()
        }
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
    let id = run_id!(state, id);
    match crate::driven::stop(&state, &id).await {
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
    Json(state.agents.clone()).into_response()
}

/// Starting a piece of work.
#[derive(Deserialize)]
struct StartWorkBody {
    /// The repository. Must be trusted first.
    cwd: String,
    /// What to do, in the user's words. Also the branch name.
    title: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default = "default_true")]
    worktree: bool,
    /// Start from a GitHub issue: its number becomes the title, its body the
    /// prompt, and the report is marked as untrusted for the agent.
    #[serde(default)]
    issue: Option<u64>,
    /// The specification this work answers, relative to the repository — a
    /// file, or the folder a spec tool wrote. Stamped, never interpreted.
    #[serde(default)]
    spec: Option<String>,
}
fn default_kind() -> String {
    "quick".into()
}
fn default_true() -> bool {
    true
}

async fn start_work(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<StartWorkBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let kind = match body.kind.as_str() {
        "chore" => crate::core::WorkKind::Chore,
        "bug" => crate::core::WorkKind::Bug,
        "feature" => crate::core::WorkKind::Feature,
        _ => crate::core::WorkKind::Quick,
    };
    // An issue supplies both, and says plainly that its body is a report from
    // someone else rather than an instruction.
    let mut title = body.title;
    let mut prompt = body.prompt.unwrap_or_else(|| title.clone());
    if let Some(number) = body.issue {
        let dir = std::path::PathBuf::from(&body.cwd);
        match crate::github::issues(&dir, None, 100).await {
            Ok(issues) => match issues.into_iter().find(|i| i.number == number) {
                Some(issue) => {
                    title = format!("#{} {}", issue.number, issue.title);
                    prompt = issue.prompt();
                }
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("issue #{number} is not open here")})),
                    )
                        .into_response();
                }
            },
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        }
    }
    // Resolved before the work exists, so a typo is an error the person can
    // still fix rather than a field that silently went missing.
    let root: std::path::PathBuf = body.cwd.into();
    let spec = match body.spec.as_deref() {
        Some(s) => match crate::core::Work::with_spec(&root, s) {
            Ok(p) => Some(p),
            Err(e) => {
                return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response();
            }
        },
        None => None,
    };
    let req = crate::work::StartRequest {
        project_root: root,
        kind,
        title,
        prompt,
        agent: body.agent,
        worktree: body.worktree,
        spec,
    };
    match crate::work::start(&state, req).await {
        Ok(id) => {
            let w = state.works.lock().await.get(&id).cloned();
            Json(json!({"work_id": id.to_string(), "work": w})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// A piece of work, with its last gate already judged.
///
/// The verdict is served rather than left to the caller. There is one
/// definition of a passing gate, it lives in the domain, and it is the one on
/// the wire — so no surface can read a reproduction gate, where failing *is*
/// passing, backwards.
#[derive(Serialize)]
struct WorkView<'a> {
    #[serde(flatten)]
    work: &'a crate::core::Work,
    gate: Option<GateView>,
    /// Whether `work retry` would work right now. Served rather than guessed,
    /// because the board cannot see whether the session that wrote the code is
    /// still there — and an offer it cannot keep is the one thing a control
    /// plane must never show.
    can_retry: bool,
    /// One line saying why the work stopped, already composed.
    ///
    /// The board used to title its "hand back again" button with the last
    /// gate's summary, which on work stopped by a *reviewer* is the summary of
    /// a gate that passed. Deriving a sentence in two places is how that
    /// happened; there is one definition and it is in the domain.
    ///
    /// Named apart from the flattened `stopped` object it summarises, because
    /// two fields of one name in a flattened struct is a duplicate key and
    /// whichever the reader takes is luck.
    stopped_summary: Option<String>,
    /// What the agent last said, for putting **beside** what the gate measured.
    ///
    /// Present only when the last gate **failed**, which is the one case the
    /// two are worth reading together: an end-of-task report references about
    /// one action in eleven and drifts toward its plan as the run leaves it, so
    /// alone it is worse than nothing and next to an exit code it is the whole
    /// point. Absent also when no transcript was kept, which is *nothing was
    /// recorded* rather than *the agent said nothing*.
    #[serde(skip_serializing_if = "Option::is_none")]
    claim: Option<String>,
    /// Why there is no claim, where there could have been one.
    ///
    /// **Two absences that are not the same fact.** A repository with
    /// `[transcripts] keep = false` recorded nothing, and a run that kept its
    /// transcript and ended without a closing message said nothing — and until
    /// this field existed both arrived as a missing `claim`, so any surface
    /// showing them had to render them identically. A reviewer reading *the
    /// agent said nothing* about a repository that simply never writes
    /// transcripts down has been told something false.
    ///
    /// `transcripts_off` or `nothing_said`, and present only where the claim was
    /// worth showing in the first place — a passing gate has no absence to
    /// explain.
    #[serde(skip_serializing_if = "Option::is_none")]
    claim_absent: Option<&'static str>,
}

#[derive(Serialize)]
struct GateView {
    name: String,
    passed: bool,
    /// One line a human can act on, from the same code the agent is told.
    summary: String,
    attempt: u32,
    /// Set when the gate was a reproduction: failing was the point.
    expect_fail: bool,
    /// The specification this verdict was reached against, as it was then.
    /// The half of a done certificate a reviewer can check without trusting
    /// this tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    spec: Option<crate::core::work::SpecStamp>,
    /// Every command the gate ran, with what it returned and what it printed.
    ///
    /// **Not new on the wire — new *here*.** The whole history is already
    /// served under the flattened `gates` array, so a consumer could reach into
    /// it and take the last element. This object exists so that nobody has to:
    /// `passed` lives here because the board once re-derived it from exit codes
    /// and read a reproduction gate exactly backwards, and the evidence for a
    /// verdict belongs beside the verdict for the same reason.
    ///
    /// The output is the same text that was handed back to the agent rather
    /// than a second description of it, and `output_tail` is bounded at capture
    /// time — which is what makes serving it affordable when a failing suite
    /// produces megabytes.
    commands: Vec<crate::core::work::CommandResult>,
}

impl<'a> WorkView<'a> {
    fn of(work: &'a crate::core::Work, can_retry: bool) -> Self {
        Self {
            gate: work.last_gate().map(|g| GateView {
                name: g.gate.clone(),
                passed: g.passed(),
                summary: g.summary(),
                attempt: g.attempt,
                expect_fail: g.expect_fail,
                spec: g.spec.clone(),
                commands: g.commands.clone(),
            }),
            can_retry,
            claim: None,
            claim_absent: None,
            stopped_summary: work
                .stopped
                .as_ref()
                .map(crate::core::work::Stopped::headline),
            work,
        }
    }
}

async fn list_work(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let live: std::collections::HashSet<crate::core::RunId> = state
        .sessions
        .lock()
        .await
        .iter()
        .filter(|(_, s)| s.is_live())
        .map(|(r, _)| r.clone())
        .collect();

    let works = state.works.lock().await;
    let mut all: Vec<_> = works.values().collect();
    all.sort_by_key(|w| std::cmp::Reverse(w.updated_at));
    let mut views: Vec<WorkView> = all
        .iter()
        .map(|w| {
            let can_retry = w.retryable(w.current_run().is_some_and(|r| live.contains(r)));
            WorkView::of(w, can_retry)
        })
        .collect();

    // The agent's claim, fetched **only** where a gate failed — which is the one
    // place it is worth reading, and which keeps this to zero queries on the
    // ordinary board. A report alone references about one action in eleven; a
    // report beside an exit code that contradicts it is the thing worth showing.
    // Where a project keeps transcripts at all. Read once per project rather
    // than once per work, and only for the works that could carry a claim.
    let roots: std::collections::BTreeMap<_, _> = {
        let world = state.world.lock().await;
        all.iter()
            .filter(|w| w.claim_is_worth_showing())
            .filter_map(|w| {
                world
                    .project(&w.project_id)
                    .map(|p| (w.project_id.clone(), p.root.clone()))
            })
            .collect()
    };
    for (view, w) in views.iter_mut().zip(all.iter()) {
        if !w.claim_is_worth_showing() {
            continue;
        }
        if let Some(run) = w.runs.last() {
            view.claim = state
                .store
                .last_agent_message(run)
                .await
                .ok()
                .flatten()
                .map(|t| crate::core::text::clip(t.trim(), 400));
        }
        if view.claim.is_none() {
            // Which absence this is. A repository that keeps no transcripts
            // recorded nothing; one that keeps them and has no closing message
            // has an agent that said nothing. The two read differently and a
            // surface that cannot tell them apart will say the wrong one.
            let keeps = roots
                .get(&w.project_id)
                .and_then(|root| crate::core::ProjectConfig::load(root).ok())
                .map(|c| c.transcripts.keep)
                .unwrap_or(true);
            view.claim_absent = Some(match keeps {
                true => "nothing_said",
                false => "transcripts_off",
            });
        }
    }
    Json(views).into_response()
}

async fn verify_work(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = work_id!(state, id);
    match crate::work::verify(&state, &id).await {
        Ok(report) => Json(json!({
            "passed": report.passed(),
            "summary": report.summary(),
            "report": report,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Marks the inbox items a person just answered, at the moment they answer.
///
/// This is what makes [`Resolution::Acted`](crate::core::attention::Resolution)
/// exact rather than inferred. The sweeper that follows closes whatever merely
/// vanished, and its `resolved_at IS NULL` guard means it can never overwrite
/// what is written here — so there is no timing window and no correlation
/// heuristic anywhere in the measurement.
async fn acted_on(
    state: &Shared,
    subject: &str,
    kinds: &[crate::core::AttentionKind],
    resolution: crate::core::attention::Resolution,
) {
    let ids: Vec<String> = kinds
        .iter()
        .map(|k| format!("{subject}:{}", k.as_str()))
        .collect();
    if let Err(e) = state.store.attention_resolve(&ids, resolution).await {
        tracing::warn!(error = %e, "could not record that an inbox item was answered");
    }
}

/// Releases a pipeline held at a declared human step.
async fn approve_work(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = work_id!(state, id);
    match crate::pipeline::approve(&state, &id).await {
        Ok(step) => {
            acted_on(
                &state,
                id.as_str(),
                &[crate::core::AttentionKind::HumanStep],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(json!({"ok": true, "released": step})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Hands a failed check back to the agent once more.
async fn retry_work(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = work_id!(state, id);
    match crate::work::retry(&state, &id).await {
        Ok(()) => {
            acted_on(
                &state,
                id.as_str(),
                // Both kinds retry closes. Naming only one would let the
                // sweeper record the other as `elsewhere` — "it stopped asking
                // on its own" — about an item a person had just answered,
                // which is precisely the correlation the attention log exists
                // to avoid.
                &[
                    crate::core::AttentionKind::GateFailed,
                    crate::core::AttentionKind::ReviewExhausted,
                ],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Picks work back up after a restart, against the same agent session.
async fn resume_work(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = work_id!(state, id);
    match crate::work::resume(&state, &id).await {
        Ok(()) => {
            acted_on(
                &state,
                id.as_str(),
                &[crate::core::AttentionKind::Interrupted],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Hides a piece of work's inbox items for a while.
///
/// Its own endpoint rather than the run's, because the items Work produces
/// outlive the sessions that produced them: a pull request that went red hours
/// after the agent stopped has no run left to snooze.
/// What a work's branch changed.
///
/// **A route of its own rather than a field on `/api/work`.** That list is
/// polled every few seconds by every open board, and a change set is expensive
/// to produce and large to send; putting it there would pay the cost for every
/// work on every poll to serve a view open for one of them. Same reasoning as
/// `/api/runs/{id}/messages`.
///
/// Computed on demand and never stored: git already holds it, and a second copy
/// is a second thing to keep true.
async fn work_changes(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = work_id!(state, id);
    let work = state.works.lock().await.get(&id).cloned();
    let Some(work) = work else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no such work"})),
        )
            .into_response();
    };
    // **The checkout being gone is not the same as nothing having changed**, and
    // a reviewer told the second when the first is true has been misled about
    // the thing they are approving.
    let Some(dir) = work.worktree.clone().filter(|d| d.is_dir()) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "this work has no checkout on disk"})),
        )
            .into_response();
    };
    // **Resolved the way `work start` resolved it**, config first and then
    // discovery — not defaulted to `HEAD`. `HEAD...HEAD` is an empty diff, so a
    // fallback that looks harmless would render *this branch changed nothing*
    // about a branch full of work, which is the one wrong answer this view
    // cannot give.
    let root = {
        let w = state.world.lock().await;
        w.project(&work.project_id).map(|p| p.root.clone())
    };
    let base = match &root {
        Some(root) => match crate::core::ProjectConfig::load(root)
            .ok()
            .and_then(|c| c.project.base_branch)
        {
            Some(b) => b,
            None => crate::git::base_branch(root).await,
        },
        None => crate::git::base_branch(&dir).await,
    };
    let set = crate::git::change_set(&dir, &base).await;
    let html = crate::core::diff::render(&set);
    Json(json!({ "changes": set, "html": html })).into_response()
}

async fn snooze_work(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<SnoozeQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let until = (q.minutes > 0)
        .then(|| jiff::Timestamp::now() + jiff::SignedDuration::from_mins(q.minutes));
    let id = work_id!(state, id);

    let saved = {
        let mut works = state.works.lock().await;
        match works.get_mut(&id) {
            Some(w) => {
                // The kinds it is asking about right now — a snooze covers
                // what was on screen, never what turns up later.
                let kinds: Vec<_> = crate::core::attention::items_for_work(w, false, false)
                    .into_iter()
                    .map(|i| i.kind)
                    .collect();
                match until {
                    Some(t) => w.snoozed.hide(kinds.clone(), t),
                    None => w.snoozed.clear(),
                }
                Some((w.clone(), kinds))
            }
            None => None,
        }
    };
    let Some((saved, dismissed)) = saved else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no such work"})),
        )
            .into_response();
    };
    acted_on(
        &state,
        saved.id.as_str(),
        &dismissed,
        crate::core::attention::Resolution::Dismissed,
    )
    .await;
    // A write the board's state depends on may not be answered with `.ok()`:
    // a snooze that silently failed to persist comes back shouting after the
    // next restart, which is a promise broken with nothing to explain it.
    if let Err(e) = state.store.save_work(&saved).await {
        tracing::warn!(error = %e, "could not persist a work snooze");
    }
    state.notify_changed();
    Json(json!({"ok": true, "until": until.map(|t| t.to_string())})).into_response()
}

#[derive(Deserialize)]
struct FinishQuery {
    #[serde(default)]
    remove_worktree: bool,
    #[serde(default)]
    force: bool,
}

async fn finish_work(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<FinishQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::work::finish(&state, &work_id!(state, id), q.remove_worktree, q.force).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Marks a project as one an agent may be started in.
///
/// Deliberately an explicit act. A headless agent runs the repository's own
/// hooks and MCP servers without asking, so somebody has to have decided that
/// this directory is theirs.
#[derive(Deserialize)]
struct TrustBody {
    /// The repository root. In the body rather than the path: a filesystem
    /// path is not one URL segment, and encoding it into one works until
    /// something between here and there normalises the escapes.
    path: String,
}

async fn trust_project(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<TrustBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let Ok(root) = std::path::PathBuf::from(&body.path).canonicalize() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{} does not exist", body.path)})),
        )
            .into_response();
    };
    let project = {
        let mut w = state.world.lock().await;
        let mut p = crate::core::Project::from_root(root.clone());
        p.trusted = true;
        let pid = w.upsert_project(p);
        w.trust(&pid);
        w.project(&pid).cloned()
    };
    match project {
        Some(p) => {
            // Checked, and rolled back when it fails. Trust is granted once,
            // by a person, for a directory — a headless agent runs that
            // repository's own hooks with no dialog of its own — so answering
            // `{"trusted": true}` for a row that never reached the store would
            // be untrue again after the next restart.
            if let Err(e) = state.store.save_project(&p).await {
                tracing::error!(project = %p.id.as_str(), error = %e, "could not record trust");
                state
                    .store
                    .record_channel("store", 0, Some(&e.to_string()))
                    .await
                    .ok();
                let mut w = state.world.lock().await;
                w.untrust(&p.id);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": format!(
                            "could not record that {} is trusted, so it is not: {e}",
                            p.root.display()
                        )
                    })),
                )
                    .into_response();
            }
            Json(json!({"trusted": true, "project": p})).into_response()
        }
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "could not register the project"})),
        )
            .into_response(),
    }
}

/// Issues a repository is offering as work.
#[derive(Deserialize)]
struct IssuesBody {
    cwd: String,
    /// Only issues carrying this label. Defaults to the project's
    /// `[github].ready_label`, so a repository decides what it is offering.
    #[serde(default)]
    label: Option<String>,
    #[serde(default = "default_issue_limit")]
    limit: u32,
}
fn default_issue_limit() -> u32 {
    30
}

async fn list_issues(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<IssuesBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let dir = std::path::PathBuf::from(&body.cwd);
    let label = match body.label {
        Some(l) => Some(l),
        None => crate::core::ProjectConfig::load(&dir)
            .ok()
            .and_then(|c| c.github.ready_label),
    };
    match crate::github::issues(&dir, label.as_deref(), body.limit).await {
        Ok(issues) => Json(issues).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// What a person can dispatch to, and what each project already offers.
///
/// The launcher's whole reason to exist is that it does not make you retype the
/// thing you do every Tuesday, so a project arrives with its prompts, the
/// pipelines its `devplane.toml` declares, and whether an agent may start in
/// it at all. Every project is listed, including ones with no session running —
/// the board shows what *is* happening, and this answers what *could*.
async fn projects(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let w = state.world.lock().await;
    let live: std::collections::BTreeMap<_, usize> =
        w.runs()
            .filter(|r| r.is_active())
            .fold(std::collections::BTreeMap::new(), |mut acc, r| {
                if let Some(p) = &r.project_id {
                    *acc.entry(p.clone()).or_default() += 1;
                }
                acc
            });

    let out: Vec<_> = w
        .projects()
        .map(|p| {
            // Read rather than cached: a project's configuration is a file the
            // person edits while this window is open, and a launcher offering
            // yesterday's pipelines is a launcher that lies.
            let cfg = crate::core::ProjectConfig::load(&p.root).ok();
            json!({
                "id": p.id.as_str(),
                "name": p.name,
                "root": p.root.display().to_string(),
                "trusted": p.trusted,
                "sessions": live.get(&p.id).copied().unwrap_or(0),
                "repo": p.repo_slug(),
                "default_agent": cfg.as_ref().and_then(|c| c.project.default_agent.clone()),
                "pipelines": cfg
                    .as_ref()
                    .map(|c| c.pipelines.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default(),
                "templates": crate::core::templates::list(&p.root),
            })
        })
        .collect();
    Json(out).into_response()
}

#[derive(Deserialize)]
struct AttentionQuery {
    /// How far back to look. A week by default: long enough that a kind which
    /// fires twice a day has something to say, short enough that a threshold
    /// changed last month is not still being judged on its old behaviour.
    #[serde(default = "default_attention_days")]
    days: i64,
}
fn default_attention_days() -> i64 {
    7
}

/// What the inbox asked for, and what became of it.
///
/// The product is a filter and this is the only thing that measures it. Read
/// it as three numbers per kind rather than one: `acted` is the item doing its
/// job, `dismissed` is the clearest evidence a kind is too loud, and
/// `elsewhere` is genuinely ambiguous — the question was answered in a
/// terminal, which means the item was right that a person was needed and wrong
/// about where they would be.
async fn attention(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<AttentionQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let since =
        jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * q.days.clamp(1, 365));
    match state.store.attention_stats(since).await {
        Ok(by_kind) => Json(json!({
            "since": since.to_string(),
            "days": q.days,
            "kinds": by_kind,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct StreamQuery {
    #[serde(default)]
    token: Option<String>,
    /// Narrow to one run. What a transcript view wants: a chatty agent in
    /// another project is not something this reader needs to be sent.
    #[serde(default)]
    run: Option<String>,
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

/// Hides the kinds a run is currently asking about, for a while.
///
/// "Not this one, not now" is the thought a person actually has, so a snooze
/// covers what is on screen — and nothing else. The single timestamp this
/// replaces also swallowed whatever turned up next, including the permission
/// request that is the one item the product exists to deliver
/// ([`Snoozed`](crate::core::attention::Snoozed)).
async fn snooze(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<SnoozeQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let until = (q.minutes > 0)
        .then(|| jiff::Timestamp::now() + jiff::SignedDuration::from_mins(q.minutes));

    let run_id = run_id!(state, id);
    // Named before the snooze hides them, because afterwards there is nothing
    // left to name. A dismissal is the clearest signal a kind is too loud, and
    // it is the one number that tells us so.
    let mut dismissed: Vec<crate::core::AttentionKind> = Vec::new();
    let saved = {
        let mut w = state.world.lock().await;
        if until.is_some()
            && let Some(r) = w.run(&run_id)
        {
            let cfg = w.attention;
            dismissed = crate::core::attention::items_for_run(r, &cfg, cfg.stall_seconds)
                .into_iter()
                .map(|i| i.kind)
                .collect();
        }
        if !w.snooze(&run_id, until) {
            None
        } else {
            w.run(&run_id).cloned()
        }
    };
    acted_on(
        &state,
        run_id.as_str(),
        &dismissed,
        crate::core::attention::Resolution::Dismissed,
    )
    .await;

    let Some(run) = saved else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "no such run"}))).into_response();
    };
    // Persisted, so a snooze survives a restart. A quiet run that starts
    // shouting again because the daemon bounced is a broken promise.
    if let Err(e) = state.store.save_run(&run).await {
        tracing::warn!(error = %e, "could not persist snooze");
    }
    state.notify_changed();
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

#[derive(Deserialize)]
struct AuditQuery {
    /// A run or work id to narrow to.
    #[serde(default)]
    about: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

/// What Devplane decided, newest first.
async fn decisions(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    match state.store.decisions(q.about.as_deref(), q.limit).await {
        Ok(d) => Json(d).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Asks the daemon to stop.
///
/// What `devplane stop` calls. A request rather than a signal to the pid in
/// `~/.devplane/daemon.json`, because a stale record names a pid the operating
/// system may since have given to somebody else. A request needs no such
/// guard: it carries the bearer token, it
/// reaches the same graceful path as ctrl-c, and it behaves identically on a
/// platform with no signals.
async fn shutdown(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    // Answer first: the reply has to leave before the server stops serving.
    let st = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        st.stopping.notify_waiters();
    });
    Json(json!({"stopping": true, "pid": std::process::id()})).into_response()
}

/// Everything that is configured, on this machine and in every registered
/// repository.
///
/// The product had no answer to *what is set up here* outside a terminal:
/// `devplane check` reads one repository at a time, `doctor` reads the
/// machine, and a person with eight projects had to visit eight of them to find
/// the one whose rules stopped loading. This is that answer in one request.
///
/// **It reads and never writes**, by design rather than by omission. The rules
/// are committed files reviewed like code, an agent here runs as the same user
/// and can read the bearer token, and a `POST` that edited `[policy]` would be
/// the widening path the gate exists to close. Every value arrives with the
/// file it came from; the editing happens where the review does.
///
/// Health is [`diagnostics`]'s job and stays there: a dead gate is already the
/// loudest thing in the inbox, and two endpoints answering "is it working"
/// would eventually disagree.
async fn setup(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);

    // The vendor's settings file, read fresh: it is the file `connect` writes
    // and a person edits by hand between one board refresh and the next.
    let settings_path = crate::observe::connect::settings_path().ok();
    let connect = settings_path.as_ref().map(|p| {
        let settings = crate::observe::connect::read_settings(p).unwrap_or_default();
        crate::observe::connect::inspect(&settings, p)
    });

    let home = crate::config::home().ok();
    let machine_policy = home.as_ref().map(|h| {
        let path = h.join("policy.toml");
        match crate::core::config::GlobalConfig::load(h) {
            Ok(g) => {
                let policy = g.policy();
                json!({
                    "path": path.display().to_string(),
                    "exists": path.exists(),
                    "deny": policy.deny_rules().iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                    "ask": policy.ask_rules().iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                    "allow": policy.allow_rules().iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                    "problems": g.validate(),
                })
            }
            // A machine-wide file that will not parse is the worst of the three
            // failures in this endpoint, because every repository inherits from
            // it. Named, not swallowed.
            Err(e) => json!({
                "path": path.display().to_string(),
                "exists": true,
                "error": e.to_string(),
            }),
        }
    });

    let provider = crate::core::provider::from_env();

    let projects: Vec<_> = {
        let w = state.world.lock().await;
        w.projects()
            .map(|p| (p.id.to_string(), p.name.clone(), p.root.clone(), p.trusted))
            .collect()
    };
    // Off the world lock: every entry reads a file, and holding the world
    // across eight repository reads stalls the board for everyone.
    let projects: Vec<_> = projects
        .into_iter()
        .map(|(id, name, root, trusted)| {
            let file = root.join(crate::core::config::CONFIG_FILE);
            let head = json!({
                "id": id,
                "name": name,
                "root": root.display().to_string(),
                "trusted": trusted,
                "config": {"path": file.display().to_string(), "exists": file.exists()},
            });
            let mut out = head;
            match crate::core::ProjectConfig::load(&root) {
                Ok(cfg) => {
                    out["describes"] = cfg.describe();
                    // What the repository is missing, said once and plainly,
                    // because a file full of correct values that verifies
                    // nothing is the common case rather than the broken one.
                    out["verified"] = json!(cfg.has_gates());
                }
                Err(e) => out["error"] = json!(e.to_string()),
            }
            out
        })
        .collect();

    Json(json!({
        "machine": {
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "started_at": state.started_at.to_string(),
            "uptime_seconds": (jiff::Timestamp::now() - state.started_at).get_seconds(),
            "home": home.as_ref().map(|h| h.display().to_string()),
            "database": home.as_ref().map(|h| h.join("devplane.db").display().to_string()),
        },
        "provider": {
            "name": provider.as_str(),
            "because": provider.because(),
            "vendor_supervision": provider.has_vendor_supervision(),
            "off": provider.missing(),
            "partial": provider.partial(),
        },
        "connect": connect,
        "gate": {
            "verified_against": crate::core::policy::VERIFIED_AGAINST,
            "rows_cleared_through": crate::core::policy::ROWS_CLEARED_THROUGH,
        },
        "machine_policy": machine_policy,
        "projects": projects,
        "agents": state.agents.clone(),
    }))
    .into_response()
}

async fn diagnostics(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let channels = state.store.channel_health().await.unwrap_or_default();
    // A `devplane.toml` that will not load keeps whatever rules were already
    // cached, which is the safe half of the answer. The unsafe half is that a
    // daemon restarted against a broken file has nothing cached, so that
    // repository's `never_auto` list is simply gone — and nothing said so.
    let broken: Vec<_> = state
        .policy
        .broken()
        .into_iter()
        .map(|(root, error)| json!({"project": root.display().to_string(), "error": error}))
        .collect();
    // The same argument one table down. The schema here changes without a
    // migration on purpose, so a row this build cannot decode is simply absent
    // from the board — which looks exactly like never having had it. A Work row
    // is the one that hurts: it carries the branch and the worktree, so losing
    // one orphans a checkout nobody is left to tell you about.
    let unreadable_rows: Vec<_> = state
        .store
        .unreadable()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(row, error)| json!({"row": row, "error": error}))
        .collect();
    // Before the lock: it shells out, and holding the world while waiting on
    // a subprocess would stall every other reader.
    let auto_mode = crate::observe::automode::effective().await;
    let forge = {
        let f = state.forge.lock().await;
        json!({
            "viewer": f.viewer,
            "error": f.error,
            "last_poll_at": f.last_poll_at.map(|t| t.to_string()),
            "projects": f.projects.len(),
            "skipped": f.skip.len(),
            // Which ones, and why. A count of things that were ruled out is a
            // number a person can do nothing with; the reason is the whole
            // value, because the usual one is "no GitHub remote" and the
            // second usual one is that `is_permanent` guessed wrong.
            "skipped_projects": f
                .skip
                .iter()
                .map(|(id, (why, at))| json!({
                    "project": id.to_string(),
                    "reason": why,
                    "at": at.to_string(),
                }))
                .collect::<Vec<_>>(),
        })
    };
    let w = state.world.lock().await;
    // Which observed sessions run a Claude Code newer than the release this
    // gate's behaviour was tested against — the gap a silent widening lives in.
    //
    // Only the status-line shim reports a version, so this is what is *known*
    // and never a claim about the whole machine. Hence the count beside it:
    // nought ahead of nought reporting is not agreement.
    let ahead: Vec<_> = w
        .runs()
        .filter_map(|r| r.claude_version.as_deref().map(|v| (r, v)))
        .filter(|(_, v)| crate::core::policy::is_ahead_of_baseline(v))
        .map(|(r, v)| json!({"run": r.id.to_string(), "version": v}))
        .collect();
    let reporting = w.runs().filter(|r| r.claude_version.is_some()).count();
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "gate": {
            "verified_against": crate::core::policy::VERIFIED_AGAINST,
            "sessions_reporting_a_version": reporting,
            "sessions_ahead_of_baseline": ahead,
        },
        "pid": std::process::id(),
        "started_at": state.started_at.to_string(),
        "uptime_seconds": (jiff::Timestamp::now() - state.started_at).get_seconds(),
        "summary": w.summary(),
        "channels": channels,
        "forge": forge,
        "stall_seconds": w.attention.stall_seconds,
        "unreadable_configs": broken,
        "unreadable_rows": unreadable_rows,
        // The *other* gate. Devplane's prohibitions reach auto mode, and in
        // that mode the thing actually deciding is a classifier configured
        // somewhere else. Read back, never written.
        "auto_mode": match auto_mode {
            Ok(cfg) => json!({
                "configured": !cfg.is_empty(),
                "environment": cfg.environment.len(),
                "allow": cfg.allow.len(),
                "soft_deny": cfg.soft_deny.len(),
                "hard_deny": cfg.hard_deny.len(),
            }),
            Err(why) => json!({"unavailable": why}),
        },
    }))
    .into_response()
}

/// Live events, as server-sent events. The browser shell and `devplane watch`
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
    let only = q.run.map(RunId::new);

    let stream = futures_util::stream::unfold(Some(rx), move |rx| {
        let only = only.clone();
        async move {
            let mut rx = rx?;
            loop {
                match rx.recv().await {
                    Ok(frame) => {
                        if let Some(want) = &only
                            && frame.run_id() != want
                        {
                            continue;
                        }
                        let data = serde_json::to_string(&frame).unwrap_or_default();
                        return Some((Ok(SseEvent::default().data(data)), Some(rx)));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(skipped = n, "subscriber lagged");
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

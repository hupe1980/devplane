//! The HTTP surface: OTLP receivers and the client API, on one loopback
//! server. Everything under `/api` and `/devplane` requires the bearer token
//! from `~/.devplane/token`.

use crate::core::event::Source;
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::host::Shared;
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
        // Receivers. The `/devplane/` marker lets `connect` recognise its own
        // entries when disconnecting.
        .route("/devplane/otel/v1/logs", post(otel_logs))
        .route("/devplane/otel/v1/metrics", post(otel_metrics))
        // Claude Code exports log records; GenAI-convention agents (Copilot,
        // Codex) export traces over the same `http/json` transport.
        .route("/devplane/otel/v1/traces", post(otel_traces))
        // A busy session's telemetry batch can exceed axum's 2 MB default, and
        // a rejected batch is silently dropped by the exporter — so the limit
        // is set explicitly: generous, but still a bound.
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024 * 1024))
        .route("/api/board", get(board))
        .route("/api/inbox", get(inbox))
        .route("/api/runs/{id}", get(run_detail))
        .route("/api/runs/{id}/rewind-gap", get(run_rewind_gap))
        .route("/api/runs/{id}/messages", get(run_messages))
        .route("/api/runs/{id}/snooze", post(snooze))
        .route("/api/runs/{id}/focus", post(focus_run))
        .route("/api/runs/{id}/prompt", post(prompt_run))
        // One answer route addressed by the ask, not the run: the row knows its
        // kind, the caller says what the person chose.
        .route("/api/asks", get(asks_index))
        .route("/api/asks/{id}/answer", post(answer_ask))
        .route("/api/runs/{id}/stop", get(stop_preview).post(stop_run))
        .route("/api/agents", get(agents))
        .route("/api/changes", get(list_change).post(start_change))
        .route("/api/changes/preflight", post(preflight_change))
        .route("/api/changes/{id}", get(change_detail))
        .route("/api/changes/{id}/drift/accept", post(accept_drift))
        .route("/api/changes/{id}/drift/tell", post(tell_drift))
        .route("/api/changes/{id}/verify", post(verify_change))
        .route("/api/changes/{id}/finish", post(finish_change))
        .route("/api/changes/{id}/retry", post(retry_change))
        .route("/api/changes/{id}/snooze", post(snooze_change))
        .route("/api/changes/{id}/review", get(change_review))
        .route("/api/changes/{id}/certificate", get(change_certificate))
        .route("/api/changes/{id}/open", post(open_change))
        .route("/api/changes/{id}/resume", post(resume_change))
        .route("/api/changes/adopt", post(adopt_change))
        .route("/api/changes/{id}/archive", post(archive_change))
        .route("/api/changes/{id}/offer", post(offer_change))
        .route("/api/reports", get(list_reports).post(file_report))
        .route("/api/reports/{id}", get(report_detail))
        .route("/api/reports/{id}/start", post(start_from_report))
        .route("/api/reports/{id}/resolve", post(resolve_report))
        // The one route that writes to a forge; only a person's act calls it.
        .route("/api/reports/{id}/open", post(open_report))
        .route("/api/projects", get(projects))
        .route("/api/specs", get(specs))
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
        .route("/api/modes", get(modes))
        .route("/api/quitting", get(quitting))
        .route("/api/quit", post(quit))
        .route("/api/stream", get(stream))
        // Open, and names the version: a client refuses a host of another
        // version, since a stale host 404s routes the client expects.
        .route(
            "/healthz",
            get(|| async { concat!("ok ", env!("CARGO_PKG_VERSION")) }),
        )
        .route("/api/rules", get(rules))
        .route("/", get(asset))
        .route("/{file}", get(asset))
        .with_state(state)
}

/// A status and a body, for a request that named something unresolvable.
/// Not a built `Response`, which is too large to carry in every `Err`.
type Refusal = (StatusCode, Json<serde_json::Value>);

/// Turns the id a person typed (often the board's short label) into the run
/// they meant. Ambiguity is a 409 naming the candidates, never a guess.
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

/// The same, for changes, whose ids are usually typed clipped.
async fn resolved_change(state: &Shared, id: String) -> Result<crate::core::ChangeId, Refusal> {
    let changes = state.changes.lock().await;
    if changes.contains_key(&crate::core::ChangeId::new(id.clone())) {
        return Ok(crate::core::ChangeId::new(id));
    }
    let mut hits: Vec<&crate::core::ChangeId> = changes
        .keys()
        .filter(|w| w.as_str().starts_with(&id))
        .collect();
    match hits.len() {
        1 => Ok(hits.remove(0).clone()),
        0 => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no such change"})),
        )),
        n => Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": format!("that matches {n} changes. Use more of the id.")
            })),
        )),
    }
}

macro_rules! change_id {
    ($state:expr, $id:expr) => {
        match resolved_change(&$state, $id).await {
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

/// Checks the bearer token. Loopback is not access control; the token, in a
/// user-only file, is. A `token` query parameter is accepted too, because
/// `EventSource` cannot send a header.
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

/// The built interface, embedded at compile time by `build.rs`. `None` when
/// no bundle existed at build time, so `cargo build` works without node; the
/// binary then says so on the page rather than falling back silently.
mod bundle {
    include!(concat!(env!("OUT_DIR"), "/ui_bundle.rs"));
}

pub use bundle::{Asset, BUNDLE};

/// What a built file is served as. Vite inlines small assets, so the list is
/// short; an unknown extension is served as bytes rather than guessed at.
fn content_type_of(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, e)| e) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// The interface: the embedded bundle, or the `dist/` directory named by
/// `DEVPLANE_UI` (development only; falls back to the embedded copy).
async fn asset(uri: axum::http::Uri) -> impl IntoResponse {
    // Vite emits flat names, so a path either matches an embedded name
    // exactly or does not exist — no directory walk, no `..` to defend.
    let want = match uri.path().trim_start_matches('/') {
        "" => "index.html",
        p => p,
    };

    if let Some(dir) = std::env::var_os("DEVPLANE_UI") {
        let path = std::path::Path::new(&dir).join(want);
        match std::fs::read(&path) {
            Ok(bytes) => {
                return (
                    [
                        (axum::http::header::CONTENT_TYPE, content_type_of(want)),
                        (axum::http::header::CACHE_CONTROL, "no-store"),
                    ],
                    bytes,
                )
                    .into_response();
            }
            Err(e) => tracing::warn!(
                path = ?path, error = %e,
                "DEVPLANE_UI is set and unreadable; serving the embedded interface"
            ),
        }
    }

    let Some(bundle) = BUNDLE else {
        // A binary built without the interface says so on the page, rather
        // than serving a blank page that sends the person debugging the host.
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            "<!doctype html><meta charset=utf-8><title>No interface</title>\
             <p style=\"font:16px system-ui;max-width:60ch;margin:4rem auto\">This binary was \
             built without its interface. The bundle is compiled in, so this is a build that \
             skipped <code>npm run build</code> in <code>ui/</code> — not a setting. The API is \
             unaffected and <code>devplane</code> on the command line works.",
        )
            .into_response();
    };

    let Some(found) = bundle.iter().find(|a| a.path == want) else {
        return (StatusCode::NOT_FOUND, "no such file").into_response();
    };
    (
        [
            (
                axum::http::header::CONTENT_TYPE,
                content_type_of(found.path),
            ),
            (axum::http::header::CACHE_CONTROL, "no-store"),
        ],
        found.bytes,
    )
        .into_response()
}

/// Which project is missing a rule relied on elsewhere.
async fn rules(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<RulesQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    match crate::view::rules(&snap, q.rule.as_deref(), q.ask) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(e)).into_response(),
    }
}

#[derive(Debug, serde::Deserialize, Default)]
struct RulesQuery {
    rule: Option<String>,
    /// Ask rather than deny, which changes the key the paste names.
    #[serde(default)]
    ask: bool,
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

/// OTLP/HTTP logs. Claude Code appends `/v1/logs` to the configured endpoint.
/// Authenticated like everything else: an open receiver would let any local
/// process or page post fabricated records into the ledger.
async fn otel_logs(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    guard!(state, headers);
    ingest_otel(state, &body, crate::observe::otel::parse_logs).await
}

/// OTLP/HTTP traces, written to the GenAI semantic conventions.
async fn otel_traces(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    guard!(state, headers);
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

async fn otel_metrics(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    guard!(state, headers);
    let n = crate::observe::otel::parse_metrics_sessions(&body).len();
    state
        .store
        .record_channel("otel_metrics", n as u64, None)
        .await
        .ok();
    (StatusCode::OK, Json(json!({"partialSuccess": {}}))).into_response()
}

// ---------------------------------------------------------------------------
// Client API
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct BoardQuery {
    /// Include sessions that exist but have never reported anything.
    #[serde(default)]
    all: bool,
}

/// The board, composed from a snapshot by the same function a host-less
/// command uses.
async fn board(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<BoardQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::board(&snap, q.all, &state.policy.broken())).into_response()
}

/// The inbox: narrowed, inhibited, folded, with the close — one computation
/// shared with the terminal.
async fn inbox(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<crate::view::InboxQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::inbox(&snap, &state.store, &state.policy, &q).await).into_response()
}

/// One row of the cross-project issue or pull-request list. `needs_you` is
/// the host's answer ([`crate::core::ForgeIssue::assigned_to_me`],
/// [`crate::core::ForgePullRequest::needs_me`]), never re-derived by a page.
#[derive(Serialize)]
struct ForgeRow<T: Serialize> {
    project: String,
    project_name: String,
    needs_you: bool,
    #[serde(flatten)]
    item: T,
}

/// Everything the forge says for every registered project, from the last
/// poll: issues and pull requests in one answer, so both share one envelope
/// and one `fetched_at`. Ordered by what needs the person, then project, then
/// newest first.
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
                needs_you: item.assigned_to_me,
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
                needs_you: item.needs_me(),
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

    // Projects that could not be read, and why: their counts are the last good
    // ones, and the view has to say which numbers are stale.
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

/// Hides a project's forge items for a while, per kind, so dismissing a
/// review request does not swallow a later assigned issue.
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
        .items
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
        // Resolve every item of that kind for the project.
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
        Some(r) => Json(crate::view::run_detail(r)).into_response(),
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

/// What this machine's rules say about one call, without running it.
/// Reached only by the MCP server (`src/mcp.rs`), which composes routes with
/// `format!` — removing this is not a compile error. `devplane explain`
/// computes the same verdict in process.
#[derive(serde::Deserialize)]
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

    // The gate this machine enforces, machine-wide rules included.
    let (cache, _) = crate::core::PolicyCache::from_disk();
    let verdict = cache.restrictive(&dir, &q.tool, &input);

    let asked_by = q.asked_by.as_deref().unwrap_or("api");
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Rule,
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
        // Set only for `unresolved`, where there is no rule to name.
        "why": verdict.why(),
        "nothing_ran": true,
    }))
    .into_response()
}

/// Files a shell call in this run named for writing — the class Claude Code's
/// checkpointing does not cover. "Named for writing" rather than "changed":
/// the gate saw the call before the tool ran.
async fn run_rewind_gap(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = run_id!(state, id);
    // Every decision: a limit would silently answer about part of the run.
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

/// What a driven agent said, oldest first. Empty for a watched session: its
/// channels carry no prompt or response text, so `focus` is the way to it.
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
    // Read before the prompt is recorded (which moves the run to working).
    let delivery = {
        let w = state.world.lock().await;
        w.run(&id)
            .map(|r| crate::core::run::Delivery::of(&r.state))
            .unwrap_or(crate::core::run::Delivery::Sent)
    };
    match crate::driven::prompt(&state, &id, body.text).await {
        Ok(()) => {
            Json(json!({"ok": true, "delivery": delivery, "says": delivery.says()})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// What a person chose, for either kind of ask. The route validates it
/// against the row. Unknown fields are refused, so a body in which nobody
/// said anything is a 422 rather than a decision.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnswerBody {
    /// `allow` or `deny`, for a permission.
    #[serde(default)]
    decision: Option<String>,
    /// An exact option the agent offered — an option id on a permission, an
    /// option value on a question. Wins over `decision`.
    #[serde(default)]
    option: Option<String>,
    /// The person's own words, where the agent offered a free-text box. Wins
    /// over `option` (the adapter's rule).
    #[serde(default)]
    custom: Option<String>,
    /// Which question, when the agent asked several at once.
    #[serde(default)]
    field: Option<String>,
    /// Which surface this came from.
    #[serde(default)]
    from: Option<String>,
    /// Every question's answer at once, for a form that asked several; a row
    /// is answered only once.
    #[serde(default)]
    answers: Vec<crate::driven::FieldAnswer>,
}

async fn asks_index(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let recent = state.store.asks(50).await.unwrap_or_default();
    let snap = state.snapshot().await;
    Json(crate::view::asks(&snap, &recent)).into_response()
}

/// Answers an ask, by its own token. There is no dismiss route:
/// an unanswered question going away is the agent proceeding on nothing.
async fn answer_ask(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<AnswerBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let Ok(Some(ask)) = state.store.ask(&id).await else {
        // The reason and the way out, as plain text.
        return (
            StatusCode::NOT_FOUND,
            format!(
                "no ask `{id}` is waiting.\n\n  devplane asks lists every question and what \
                 became of it."
            ),
        )
            .into_response();
    };

    let answer = match crate::driven::parse_answer(
        &ask,
        body.decision.as_deref(),
        body.option.clone(),
        body.custom.clone(),
        body.field.clone(),
        &body.answers,
    ) {
        Ok(a) => a,
        // Checked before the answer is stored, so a crash between the write
        // and the delivery replays as answered.
        Err(why) => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": why}))).into_response();
        }
    };

    let from = body.from.unwrap_or_else(|| "api".to_string());
    match crate::driven::answer_ask(&state, &id, answer, &from).await {
        Ok(a) => {
            acted_on(
                &state,
                a.run.as_str(),
                &[
                    crate::core::AttentionKind::Permission,
                    crate::core::AttentionKind::Question,
                ],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(crate::view::render_ask(&a)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// What stopping a run leaves behind, as the sentence a surface shows before
/// it stops, and the three things named.
async fn what_survives(state: &Shared, run: &RunId) -> serde_json::Value {
    let changes = state.changes.lock().await;
    match changes.values().find(|c| c.runs.contains(run)) {
        Some(c) => json!({
            "survives": {
                "change": c.id.to_string(),
                "worktree": c.worktree.as_ref().map(|w| w.display().to_string()),
                "branch": c.branch,
            },
            "says": c.stop_says(),
        }),
        None => json!({
            "survives": {"change": null, "worktree": null, "branch": null},
            "says": "stopping ends the agent's turn; the run's record stays",
        }),
    }
}

/// The sentence, without stopping anything — so a surface can say it first.
async fn stop_preview(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = run_id!(state, id);
    Json(what_survives(&state, &id).await).into_response()
}

async fn stop_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = run_id!(state, id);
    let mut survives = what_survives(&state, &id).await;
    match crate::driven::stop(&state, &id).await {
        Ok(()) => {
            if let Some(o) = survives.as_object_mut() {
                o.insert("ok".into(), serde_json::Value::Bool(true));
            }
            Json(survives).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn agents(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::agents(&snap, &state.store).await).into_response()
}

/// Starting a change — in one project, or the same prompt in several.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartChangeBody {
    /// The repository, for a change in one. Must be trusted first.
    #[serde(default)]
    cwd: Option<String>,
    /// Several projects at once: registered names, ids or paths, all given
    /// the same title and prompt. Every refusal is reported before any change
    /// is created.
    #[serde(default)]
    projects: Vec<String>,
    /// What to do, in the user's words. Also the branch name.
    #[serde(default)]
    title: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default = "default_true")]
    worktree: bool,
    /// Start from a GitHub issue: number becomes title, body the prompt,
    /// marked untrusted for the agent. One project only.
    #[serde(default)]
    issue: Option<u64>,
    /// The specification this change answers, relative to the repository — a
    /// file, or the folder a spec tool wrote. Stamped, never interpreted.
    #[serde(default)]
    spec: Option<String>,
    /// Which tasks of `spec` this change sends: a token, or `file:line`.
    /// Refused by name before anything is created when one matches nothing.
    #[serde(default)]
    tasks: Vec<String>,
}
fn default_true() -> bool {
    true
}

impl StartChangeBody {
    /// The projects asked for: `projects`, or the one `cwd` names.
    fn targets(&self) -> Vec<String> {
        let mut out = self.projects.clone();
        if let Some(cwd) = &self.cwd {
            out.push(cwd.clone());
        }
        out
    }

    /// The title and prompt, from the body or from the issue it names.
    async fn words(&self, targets: &[String]) -> Result<(String, String), String> {
        let title = self.title.clone();
        let prompt = self.prompt.clone().unwrap_or_else(|| title.clone());
        let Some(number) = self.issue else {
            return Ok((title, prompt));
        };
        let [dir] = targets else {
            return Err("an issue is one repository's: start from it in one project".into());
        };
        let issues = crate::github::issues(std::path::Path::new(dir), None, 100)
            .await
            .map_err(|e| e.to_string())?;
        match issues.into_iter().find(|i| i.number == number) {
            Some(issue) => Ok((format!("#{} {}", issue.number, issue.title), issue.prompt())),
            None => Err(format!("issue #{number} is not open here")),
        }
    }
}

/// Every project's preflight, rendered once for every caller.
fn preflight_json(findings: &[crate::core::preflight::Finding]) -> serde_json::Value {
    json!({
        "targets": findings.iter().map(|f| json!({
            "asked": f.asked,
            "name": f.name,
            "root": f.root,
            "refusal": f.refusal.map(|r| r.as_str()),
            "says": f.says,
            // What the first run in a fresh tree costs, as sentences.
            "notes": f.notes.iter().map(crate::core::preflight::Note::says).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "refused": findings.iter().filter(|f| !f.accepted()).count(),
    })
}

/// What starting this change would do in each project, writing nothing —
/// the same body and rule as `POST /api/changes`.
async fn preflight_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<StartChangeBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let targets = body.targets();
    let prompt = match body.words(&targets).await {
        Ok((_, prompt)) => prompt,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    };
    let ask = crate::change::Ask {
        prompt: &prompt,
        agent: body.agent.as_deref(),
        worktree: body.worktree,
        spec: body.spec.as_deref(),
        tasks: &body.tasks,
    };
    let findings = crate::change::preflight(&state, &targets, &ask).await;
    Json(preflight_json(&findings)).into_response()
}

/// Starts a change in each project named — or in none of them. Every
/// refusal is named before anything is created; each accepted project then
/// goes through the one start path.
async fn start_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<StartChangeBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let targets = body.targets();
    if targets.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "say where: `cwd`, or one or more `projects`"})),
        )
            .into_response();
    }
    let (title, prompt) = match body.words(&targets).await {
        Ok(words) => words,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    };
    let ask = crate::change::Ask {
        prompt: &prompt,
        agent: body.agent.as_deref(),
        worktree: body.worktree,
        spec: body.spec.as_deref(),
        tasks: &body.tasks,
    };
    let findings = crate::change::preflight(&state, &targets, &ask).await;
    let refused: Vec<&str> = findings.iter().filter_map(|f| f.says.as_deref()).collect();
    if !refused.is_empty() {
        let mut out = preflight_json(&findings);
        out["error"] = json!(match targets.len() {
            1 => refused.join("\n"),
            _ => format!("nothing was started:\n{}", refused.join("\n")),
        });
        return (StatusCode::BAD_REQUEST, Json(out)).into_response();
    }

    // A project that changes between preflight and start is reported and the
    // rest continue, rather than abandoning started work.
    let mut started = Vec::new();
    let mut failed = Vec::new();
    for f in &findings {
        let Some(root) = f.root.clone() else { continue };
        let root = std::path::PathBuf::from(root);
        // Normalised per project: a relative spec is each repository's own.
        let spec = body
            .spec
            .as_deref()
            .and_then(|s| crate::core::Change::with_spec(&root, s).ok());
        let req = crate::change::StartRequest {
            project_root: root,
            title: title.clone(),
            prompt: prompt.clone(),
            agent: body.agent.clone(),
            worktree: body.worktree,
            spec,
            tasks: body.tasks.clone(),
            from_report: None,
        };
        match crate::change::start(&state, req).await {
            Ok(id) => {
                let w = state.changes.lock().await.get(&id).cloned();
                started.push(json!({"project": f.name, "change_id": id.to_string(), "change": w}));
            }
            // The whole chain: the useful cause sits under the outer line.
            Err(e) => failed.push(format!("{}  {e:#}", f.name)),
        }
    }
    let mut out = json!({ "changes": started });
    if let [one] = started.as_slice() {
        out["change_id"] = one["change_id"].clone();
        out["change"] = one["change"].clone();
    }
    if !failed.is_empty() {
        out["error"] = json!(failed.join("\n"));
        if started.is_empty() {
            return (StatusCode::BAD_REQUEST, Json(out)).into_response();
        }
    }
    Json(out).into_response()
}

async fn list_change(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::change_list(&snap, &state.store).await).into_response()
}

async fn change_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let snap = state.snapshot().await;
    match crate::view::change_one(&snap, &state.store, &id).await {
        Some(v) => Json(v).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no such change"})),
        )
            .into_response(),
    }
}

/// Which run a drift decision is about.
#[derive(Deserialize)]
struct DriftBody {
    run: String,
}

/// The specification moved under a run and the person accepts it: the
/// change's start moves forward to what the run saw. Recorded as theirs.
async fn accept_drift(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<DriftBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let run = run_id!(state, body.run);
    match crate::change::accept_drift(&state, &id, &run).await {
        Ok(Some(fingerprint)) => {
            acted_on(
                &state,
                run.as_str(),
                &[crate::core::AttentionKind::SpecDrifted],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(json!({"accepted": true, "fingerprint": fingerprint})).into_response()
        }
        Ok(None) => (
            StatusCode::CONFLICT,
            Json(json!({"error": format!("no drift on this change for run {run}")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

/// The specification moved under a run and the person tells it: the run is
/// resumed with a prompt naming the files, and its work is measured against
/// what it was told from here on. Recorded as theirs.
async fn tell_drift(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<DriftBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let run = run_id!(state, body.run);
    match crate::change::tell_drift(&state, &id, &run).await {
        Ok(true) => {
            acted_on(
                &state,
                run.as_str(),
                &[crate::core::AttentionKind::SpecDrifted],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(json!({"told": true, "run": run.to_string()})).into_response()
        }
        Ok(false) => (
            StatusCode::CONFLICT,
            Json(json!({"error": format!("no drift on this change for run {run}")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

async fn verify_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::change::verify(&state, &id).await {
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

/// Marks the inbox items a person just answered, at the moment they answer,
/// so [`Resolution::Acted`](crate::core::attention::Resolution) is exact. The
/// sweeper's `resolved_at IS NULL` guard never overwrites it.
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

/// Hands a failed check back to the agent once more.
async fn retry_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::change::retry(&state, &id).await {
        Ok(()) => {
            acted_on(
                &state,
                id.as_str(),
                &[crate::core::AttentionKind::GateFailed],
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

/// Picks a change back up after a restart, against the same agent session.
async fn resume_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::change::resume(&state, &id).await {
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

#[derive(Deserialize)]
struct OpenQuery {
    #[serde(rename = "in")]
    r#in: crate::host::OpenIn,
}

/// Opens a change's worktree in the editor or a terminal where this process
/// can (the app); the CLI host answers with the path and a sentence instead.
async fn open_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<OpenQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let change = state.changes.lock().await.get(&id).cloned();
    let Some(change) = change else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no such change"})),
        )
            .into_response();
    };
    // An in-place change works in the project's own checkout.
    let path = match change.worktree.clone() {
        Some(p) => p,
        None => {
            let w = state.world.lock().await;
            match w.project(&change.project_id) {
                Some(p) => p.root.clone(),
                None => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "this change has no checkout on disk"})),
                    )
                        .into_response();
                }
            }
        }
    };
    let shown = path.display().to_string();
    match state.opener.get() {
        Some(open) => match open(q.r#in, &path) {
            Ok(()) => Json(json!({"opened": true, "path": shown})).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "opened": false,
                    "path": shown,
                    "says": format!("could not open it ({e}) — open it yourself: {shown}"),
                })),
            )
                .into_response(),
        },
        None => Json(json!({
            "opened": false,
            "path": shown,
            "says": format!("open it yourself: {shown}"),
        }))
        .into_response(),
    }
}

/// The change composed for deciding whether to merge it: in declared role
/// order, each file with its four facts, grouped by intent where the runs
/// allow it, and the gate standing once.
async fn change_review(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let snap = state.snapshot().await;
    match crate::view::review(&snap, &state.store, &id).await {
        Ok(v) => Json(v).into_response(),
        Err(why) => (StatusCode::NOT_FOUND, Json(json!({"error": why.says()}))).into_response(),
    }
}

async fn snooze_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<SnoozeQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let until = (q.minutes > 0)
        .then(|| jiff::Timestamp::now() + jiff::SignedDuration::from_mins(q.minutes));
    let id = change_id!(state, id);
    // A drift is derived from the runs, so it is asked before the row is held.
    let drifting = crate::change::has_drift(&state, &id).await;

    let saved = {
        let mut changes = state.changes.lock().await;
        match changes.get_mut(&id) {
            Some(w) => {
                // The kinds on screen now, never what turns up later.
                let mut kinds: Vec<_> = crate::core::attention::items_for_change(w, false, false)
                    .into_iter()
                    .map(|i| i.kind)
                    .collect();
                if drifting {
                    kinds.push(crate::core::AttentionKind::SpecDrifted);
                }
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
            Json(json!({"error": "no such change"})),
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
    // A write the board depends on may not be `.ok()`: a silently lost
    // snooze resurfaces after the next restart.
    if let Err(e) = state.store.save_change(&saved).await {
        tracing::warn!(error = %e, "could not persist a change snooze");
    }
    state.notify_changed();
    Json(json!({"ok": true, "until": until.map(|t| t.to_string())})).into_response()
}

/// Finishes a change: a person accepts it, and the basis is recorded. Nothing
/// is removed; `archive` is the removal.
async fn finish_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::change::finish(&state, &id).await {
        Ok(()) => {
            acted_on(
                &state,
                id.as_str(),
                &[crate::core::AttentionKind::ReadyToDecide],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            let c = state.changes.lock().await.get(&id).cloned();
            Json(json!({"ok": true, "state": c.map(|c| c.current_state())})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Adopts a branch somebody made by hand. Refuses a branch that does not
/// exist or is the base; reads the rest from git.
#[derive(Deserialize)]
struct AdoptBody {
    branch: String,
    /// The repository root.
    project: String,
    #[serde(default)]
    spec: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

async fn adopt_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<AdoptBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    // The specification is checked inside `adopt`, against the branch's
    // checkout.
    let req = crate::change::AdoptRequest {
        project_root: body.project.into(),
        branch: body.branch,
        spec: body.spec,
        title: body.title,
    };
    match crate::change::adopt(&state, req).await {
        Ok(id) => {
            let c = state.changes.lock().await.get(&id).cloned();
            Json(json!({"change_id": id.to_string(), "change": c})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ArchiveQuery {
    #[serde(default)]
    delete_branch: bool,
    /// Remove a worktree that has uncommitted work in it.
    #[serde(default)]
    discard_uncommitted: bool,
    /// With `delete_branch`: delete a branch with unmerged commits.
    #[serde(default)]
    force: bool,
}

/// Archives a change: the worktree goes, the record stays.
async fn archive_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<ArchiveQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let how = crate::change::ArchiveHow {
        delete_branch: q.delete_branch,
        discard_uncommitted: q.discard_uncommitted,
        force: q.force,
    };
    match crate::change::archive(&state, &id, how).await {
        Ok(done) => Json(json!({
            "ok": true,
            "worktree_removed": done.worktree_removed,
            "branch_deleted": done.branch_deleted,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Files a report. No provenance field is read: `run` is resolved against
/// this host's record, and a person's report says `as_person`. An unknown run
/// is a 400 naming it.
async fn file_report(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<crate::reports::FileRequest>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::reports::file_and_deliver(&state, body).await {
        Ok(filed) => (StatusCode::CREATED, Json(filed_json(filed))).into_response(),
        Err(refusal) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": refusal.says()})),
        )
            .into_response(),
    }
}

/// What filing answers, on the host and with no host alike.
pub fn filed_json(filed: crate::reports::Filed) -> serde_json::Value {
    let routed = match &filed.report.target {
        crate::core::report::Target::Project { .. } => "inbox",
        crate::core::report::Target::GitHub { .. } => "drafted",
        crate::core::report::Target::Source => "kept",
    };
    json!({
        "report": crate::view::ReportView::of(filed.report, jiff::Timestamp::now()),
        "routed": routed,
        "says": filed.says,
        "delivered": filed.delivered,
    })
}

async fn list_reports(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<crate::view::ReportQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    match crate::view::reports(&snap, &state.store, &q).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn report_detail(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    match crate::view::report(&snap, &state.store, &id).await {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no report `{id}`")})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize, Default)]
struct StartFromReportBody {
    #[serde(default)]
    agent: Option<String>,
}

/// Starts a change in the target from a report. The person's act, and the
/// row it answers is recorded as acted on.
async fn start_from_report(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<StartFromReportBody>>,
) -> impl IntoResponse {
    guard!(state, headers);
    let agent = body.and_then(|Json(b)| b.agent);
    match crate::reports::start(&state, &id, agent).await {
        Ok(change) => {
            report_acted_on(&state, &id).await;
            Json(json!({"change_id": change})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct ResolveBody {
    #[serde(rename = "as")]
    how: String,
    #[serde(default)]
    reason: Option<String>,
}

/// Answers a report by hand: rejected, deferred, fixed or — a draft —
/// discarded. The answer is recorded on the change that filed it.
async fn resolve_report(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ResolveBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let answer = match crate::reports::Answer::parse(&body.how, body.reason) {
        Ok(a) => a,
        Err(why) => return (StatusCode::BAD_REQUEST, Json(json!({"error": why}))).into_response(),
    };
    match crate::reports::resolve(&state, &id, answer).await {
        Ok(r) => {
            report_acted_on(&state, r.id.as_str()).await;
            Json(crate::view::ReportView::of(r, jiff::Timestamp::now())).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

/// Opens a GitHub draft with the person's own `gh`. `409` unless it is a draft.
async fn open_report(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    match state.store.report(&id).await {
        Ok(Some(r)) if r.state == crate::core::report::State::Drafted => {}
        Ok(Some(r)) => {
            return (
                StatusCode::CONFLICT,
                Json(
                    json!({"error": format!("report {} is {}, not a draft", r.id, r.state_says())}),
                ),
            )
                .into_response();
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("no report `{id}`")})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    }
    match crate::reports::open(&state, &id).await {
        Ok(r) => {
            report_acted_on(&state, r.id.as_str()).await;
            let url = match &r.state {
                crate::core::report::State::Opened { url } => url.clone(),
                _ => String::new(),
            };
            Json(json!({"url": url})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

/// A report's row was answered with one of its own actions.
async fn report_acted_on(state: &Shared, id: &str) {
    let id = match state.store.report(id).await {
        Ok(Some(r)) => r.id,
        _ => return,
    };
    acted_on(
        state,
        id.as_str(),
        &[
            crate::core::AttentionKind::ReportFiled,
            crate::core::AttentionKind::ReportWaiting,
        ],
        crate::core::attention::Resolution::Acted,
    )
    .await;
}

/// Offers a change: pushes and opens the pull request when the person's
/// configuration allows it, else hands back the commands that would.
async fn offer_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::change::offer(&state, &id).await {
        Ok(offer) => {
            acted_on(
                &state,
                id.as_str(),
                &[crate::core::AttentionKind::ReadyToDecide],
                crate::core::attention::Resolution::Acted,
            )
            .await;
            Json(serde_json::to_value(offer).unwrap_or_default()).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn change_certificate(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let snap = state.snapshot().await;
    match crate::view::change_certificate(&snap, &state.store, &id).await {
        Some(v) => Json(v).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no change `{id}`")})),
        )
            .into_response(),
    }
}

/// Marks a project as one an agent may be started in: an explicit act,
/// because a headless agent runs the repository's own hooks and MCP servers.
#[derive(Deserialize)]
struct TrustBody {
    /// The repository root, in the body because a filesystem path is not one
    /// URL segment.
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
            // Checked, and rolled back on failure, so `{"trusted": true}`
            // stays true after a restart.
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

async fn specs(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::specs(&snap)).into_response()
}

/// What a person can start a change in, and whether an agent may start
/// there. Every project is listed, running or not.
async fn projects(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::projects(&snap)).into_response()
}

#[derive(Deserialize)]
struct AttentionQuery {
    /// How far back to look. A week by default.
    #[serde(default = "default_attention_days")]
    days: i64,
}
fn default_attention_days() -> i64 {
    7
}

async fn modes(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::modes(&snap)).into_response()
}

async fn attention(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<AttentionQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::view::attention(&state.store, q.days).await {
        Ok(v) => Json(v).into_response(),
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
    /// Narrow to one run.
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

/// Hides the kinds a run is currently asking about, for a while — only what
/// is on screen, never what turns up next
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
    // Named before the snooze hides them: a dismissal is the clearest signal
    // that a kind is too loud.
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
    // Persisted, so a snooze survives a restart.
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
    match crate::view::search(&state.store, &q.q, q.limit).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct AuditQuery {
    /// A run or change id to narrow to.
    #[serde(default)]
    about: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    /// Only what was decided instead of you: a rule, a clock or nobody
    /// (`Authority::was_taken_for_you`). `person` and `devplane` rows are out.
    #[serde(default)]
    without_me: bool,
}

/// What Devplane decided, newest first.
async fn decisions(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::view::decisions(&state.store, q.about.as_deref(), q.limit, q.without_me).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// What quitting would end, and what it would leave running — read before
/// the stop, never folded into it.
async fn quitting(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    let q = crate::core::reduce::facts::quitting(snap.world.runs(), &snap.open_asks);
    // The window prints `says` verbatim, so both readers share one sentence.
    let mut v = serde_json::to_value(&q).unwrap_or_else(|_| json!({}));
    v["says"] = json!(q.says());
    Json(v).into_response()
}

/// Asks the host to stop (`devplane quit`). A request rather than a signal
/// to the recorded pid, which may since belong to another process; it reaches
/// the same graceful path as ctrl-c on every platform.
async fn quit(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    // Answer first: the reply has to leave before the server stops serving.
    let st = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let _ = st.stopping.send(true);
    });
    Json(json!({"stopping": true, "pid": std::process::id()})).into_response()
}

async fn setup(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::setup(&snap)).into_response()
}

async fn diagnostics(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let channels = state.store.channel_health().await.unwrap_or_default();
    let opencode = match std::env::var(crate::observe::opencode::ENV_SERVER) {
        Ok(url) => {
            let h = state.opencode.lock().await.clone();
            json!({"server": url, "says": h.says(), "live": matches!(
                h, crate::observe::opencode::Health::Live { .. }
            )})
        }
        // Not configured is not unhealthy, and the two must not render the same.
        Err(_) => serde_json::Value::Null,
    };
    // A `devplane.toml` that will not load keeps cached rules, but a host
    // restarted against it has none — so that repository's `never_auto` list
    // is gone, and this says so.
    let broken: Vec<_> = state
        .policy
        .broken()
        .into_iter()
        .map(|(root, error)| json!({"project": root.display().to_string(), "error": error}))
        .collect();
    // Rows this build cannot decode are absent from the board. A lost Change
    // row orphans a worktree, so they are counted here.
    let unreadable_rows: Vec<_> = state
        .store
        .unreadable()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(row, error)| json!({"row": row, "error": error}))
        .collect();
    // Before the lock: it shells out.
    let auto_mode = crate::observe::automode::effective().await;
    let forge = {
        let f = state.forge.lock().await;
        json!({
            "viewer": f.viewer,
            "error": f.error,
            "last_poll_at": f.last_poll_at.map(|t| t.to_string()),
            "projects": f.projects.len(),
            "skipped": f.skip.len(),
            // Which ones, and why: the reason (usually "no GitHub remote")
            // is what a person can act on.
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
    // How many live sessions report their version; "nothing reports a
    // version" is a real diagnostic.
    let reporting = w
        .runs()
        .filter(|r| r.state.is_live() && r.claude_version.is_some())
        .count();
    let (lost_events, lost_decisions, last_loss) = state.unwritten.counts();
    // Agents a previous host abandoned, read once at startup.
    let leaked: Vec<_> = state
        .leaked_agents
        .lock()
        .await
        .iter()
        .map(|(pid, command, worktree)| {
            json!({
                "pid": pid,
                "command": command,
                "worktree": worktree,
            })
        })
        .collect();
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "gate": {
            "syntax_modelled_on": crate::core::policy::SYNTAX_MODELLED_ON,
            "sessions_reporting_a_version": reporting,
        },
        // Live sessions whose environment Devplane could read — only the
        // `SessionStart` hook can, so this is the question-clock coverage.
        "modes": {
            "clock_read": w.runs().filter(|r| r.state.is_live() && r.question_clock_read.is_some()).count(),
            "clock_unread": w.runs().filter(|r| r.state.is_live() && r.question_clock_read.is_none()).count(),
        },
        "leaked_agents": leaked,
        "pid": std::process::id(),
        "started_at": state.started_at.to_string(),
        "uptime_seconds": (jiff::Timestamp::now() - state.started_at).get_seconds(),
        "summary": w.summary(),
        "channels": channels,
        // The OpenCode subscription. The feed does not replay, so a dead
        // subscription looks like a quiet machine. Absent where not asked for.
        "opencode": opencode,
        "forge": forge,
        "stall_seconds": w.attention.stall_seconds,
        "unreadable_configs": broken,
        "unreadable_rows": unreadable_rows,
        // How much of the store never arrived. A failed write is logged and
        // dropped (losing history beats stalling a blocked hook); this is
        // where that loss becomes visible.
        "unwritten": {
            "events": lost_events,
            "decisions": lost_decisions,
            "last_reason": last_loss,
        },
        // The other gate: in auto mode a classifier configured elsewhere
        // decides. Read back, never written.
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

/// Live events, as server-sent events, for the browser shell and `devplane
/// watch`. A lagging subscriber is skipped forward, not disconnected.
async fn stream(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<StreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, StatusCode> {
    // A bad token says so, rather than looking like a quiet machine.
    if !authorised(&state, &headers, q.token.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let rx = state.tx.subscribe();
    let stopping = state.stopping.subscribe();
    let only = q.run.map(RunId::new);

    // The stream ends when the host stops; otherwise one open tab would hold
    // a graceful shutdown for ever.
    let stream = futures_util::stream::unfold(Some((rx, stopping)), move |pair| {
        let only = only.clone();
        async move {
            let (mut rx, mut stopping) = pair?;
            loop {
                let next = tokio::select! {
                    got = rx.recv() => got,
                    _ = stopping.wait_for(|stop| *stop) => return None,
                };
                match next {
                    Ok(frame) => {
                        if let Some(want) = &only
                            && frame.run_id() != want
                        {
                            continue;
                        }
                        let data = serde_json::to_string(&frame).unwrap_or_default();
                        return Some((Ok(SseEvent::default().data(data)), Some((rx, stopping))));
                    }
                    // Frames were dropped, so a `resync` event tells the
                    // subscriber to read the state again.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(skipped = n, "subscriber lagged");
                        let data = serde_json::json!({ "skipped": n }).to_string();
                        return Some((
                            Ok(SseEvent::default().event("resync").data(data)),
                            Some((rx, stopping)),
                        ));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        }
    });

    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}

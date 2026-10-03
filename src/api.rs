//! The HTTP surface: OTLP receivers and the client API, on one loopback
//! server. Everything under `/api` and `/devplane` requires the bearer token
//! from `~/.devplane/token`.

use crate::core::event::Source;
use crate::core::ids::RunId;
use crate::core::run::RunMode;
use crate::firstrun as first;
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
        // Receivers. The `/devplane/` marker lets `agent connect` recognise its own
        // entries when disconnecting.
        .route("/devplane/otel/v1/logs", post(otel_logs))
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
        .route("/api/runs/{id}/messages", get(run_messages))
        .route("/api/runs/{id}/snooze", post(snooze))
        .route("/api/runs/{id}/prompt", post(prompt_run))
        // The stop tool's one write: an agent says its spec and tests conflict.
        .route("/api/runs/{id}/claim", post(claim_run))
        // One answer route addressed by the ask, not the run: the row knows its
        // kind, the caller says what the person chose.
        .route("/api/asks", get(asks_index))
        .route("/api/asks/{id}/answer", post(answer_ask))
        .route("/api/asks/{id}/undo", post(undo_answer))
        .route("/api/runs/{id}/stop", get(stop_preview).post(stop_run))
        .route("/api/agents", get(agents))
        .route("/api/changes", get(list_change).post(start_change))
        .route("/api/changes/preflight", post(preflight_change))
        .route("/api/changes/{id}", get(change_detail))
        .route("/api/changes/{id}/drift/accept", post(accept_drift))
        .route("/api/changes/{id}/drift/tell", post(tell_drift))
        .route("/api/changes/{id}/verify", post(verify_change))
        .route("/api/changes/{id}/base-checks", post(base_checks_change))
        .route("/api/changes/{id}/finish", post(finish_change))
        .route("/api/changes/{id}/retry", post(retry_change))
        .route("/api/changes/{id}/snooze", post(snooze_change))
        .route("/api/changes/{id}/review", get(change_review))
        .route("/api/changes/{id}/review/seen", post(mark_review_seen))
        .route(
            "/api/changes/{id}/comments",
            get(list_comments).post(create_comment),
        )
        .route("/api/changes/{id}/comments/send", post(send_comments))
        .route(
            "/api/changes/{id}/comments/{cid}",
            axum::routing::delete(delete_comment),
        )
        .route("/api/changes/{id}/certificate", get(change_certificate))
        .route("/api/changes/{id}/open", post(open_change))
        .route("/api/changes/{id}/resume", post(resume_change))
        .route("/api/changes/{id}/login", post(login_change))
        .route("/api/changes/{id}/claim/read", post(read_claim))
        .route("/api/changes/adopt", post(adopt_change))
        .route("/api/changes/draft", post(draft_change))
        .route("/api/changes/{id}/draft/accept", post(accept_draft))
        .route("/api/changes/{id}/draft/reject", post(reject_draft))
        .route("/api/changes/{id}/clarify", post(clarify_change))
        .route(
            "/api/projects/{id}/config/open-questions",
            post(accept_open_questions),
        )
        .route("/api/changes/{id}/archive", post(archive_change))
        .route("/api/changes/{id}/offer", post(offer_change))
        .route("/api/projects", get(projects))
        .route("/api/specs", get(specs))
        .route("/api/projects/trust", post(trust_project))
        .route("/api/projects/reload", post(reload_projects))
        .route("/api/projects/{id}/snooze", post(snooze_project))
        .route("/api/issues", post(list_issues))
        .route("/api/forge", get(forge))
        // Sign-in state per GitHub host; never the token.
        .route("/api/github", get(github_state))
        .route("/api/github/login", post(github_login))
        .route("/api/github/logout", post(github_logout))
        .route("/api/github/refresh", post(github_refresh))
        .route("/api/decisions", get(decisions))
        .route("/api/decisions/unattributed", get(unattributed))
        .route("/api/explain", get(explain))
        .route("/api/search", get(search))
        .route("/api/diagnostics", get(diagnostics))
        .route("/api/setup", get(setup))
        .route("/api/modes", get(modes))
        .route("/api/quitting", get(quitting))
        .route("/api/quit", post(quit))
        // Update checks: the window's, off until the person says (`crate::update`).
        .route("/api/app/update", get(crate::update::state))
        .route("/api/app/updates", post(crate::update::preference))
        .route("/api/app/update/install", post(crate::update::install))
        .route("/api/app/update/dismiss", post(crate::update::dismiss))
        .route("/api/stream", get(stream))
        // The first run from the window: the checklist, declaring a check,
        // scanning before trust, connecting an agent (`crate::firstrun`).
        .route("/api/first-run", get(first::first_run))
        .route("/api/projects/scan", get(first::scan))
        .route(
            "/api/projects/{id}/gates/candidates",
            get(first::gate_candidates),
        )
        .route("/api/projects/{id}/gates", post(first::declare_gates))
        .route("/api/connect/{agent}", post(first::connect_agent))
        .route("/api/disconnect/{agent}", post(first::disconnect_agent))
        // Open, and names the version: a client refuses a host of another
        // version, since a stale host 404s routes the client expects.
        .route("/healthz", get(healthz))
        .route("/", get(asset))
        .route("/{file}", get(asset))
        .with_state(state)
}

#[derive(Deserialize)]
struct HealthQuery {
    nonce: Option<String>,
}

/// `ok <version>`, and with a client's `nonce` the proof that this process
/// holds the token ([`crate::config::host_proof`]), which a client checks
/// before it sends the token to whatever answers on the port.
async fn healthz(State(state): State<Shared>, Query(q): Query<HealthQuery>) -> String {
    let ok = concat!("ok ", env!("CARGO_PKG_VERSION"));
    // Before the port is bound there is nothing to prove, so no proof.
    match (
        q.nonce.filter(|n| (16..=128).contains(&n.len())),
        state.port.get(),
    ) {
        (Some(n), Some(port)) => {
            format!(
                "{ok} {}",
                crate::config::host_proof(&state.token, *port, &n)
            )
        }
        _ => ok.to_string(),
    }
}

/// A status and a body, for a request that named something unresolvable.
/// Not a built `Response`, which is too large to carry in every `Err`.
type Refusal = (StatusCode, Json<serde_json::Value>);

/// Turns the id a person typed (often the board's short label) into the run
/// they meant. Ambiguity is a 409 naming the candidates, never a guess.
async fn resolved_run(state: &Shared, id: String) -> Result<RunId, Refusal> {
    let world = state.world.lock().await;
    world.resolve_run(&id, jiff::Timestamp::now()).map_err(|e| {
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
/// user-only file, is. Only ever from the header: a token in a URL ends up in
/// logs and process lists.
pub(crate) fn authorised(state: &Shared, headers: &HeaderMap) -> bool {
    let from_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or(v));

    match from_header {
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

/// The telemetry routes' check: the control token, or the telemetry-only one
/// vendor settings carry.
fn authorised_to_ingest(state: &Shared, headers: &HeaderMap) -> bool {
    authorised(state, headers)
        || headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.strip_prefix("Bearer ").unwrap_or(v))
            .is_some_and(|t| constant_time_eq(t.as_bytes(), state.ingest_token.as_bytes()))
}

/// Where an authorised answer came from, as the host can tell it: `window`
/// only when the request carries the window's proof for this very ask
/// ([`crate::config::ask_proof`]), which only the app's own webviews can make;
/// every other holder of the control token — the CLI, a script, a browser tab
/// on `devplane up`, which is handed no key — is `cli`. The `Origin` header
/// and the body are not read: any token holder can send either.
///
/// This narrows the hole and does not close it: an agent that can drive the
/// desktop (scripting, accessibility) can press the window's buttons, and a
/// process running as the person can read the key out of the app's memory.
///
/// No input yields [`AnswerPath::Notification`](crate::core::ask::AnswerPath):
/// that path exists only in the app's own process.
fn path_of(state: &Shared, headers: &HeaderMap, ask_id: &str) -> crate::core::ask::AnswerPath {
    if window_proved(state, headers, ASK_PROOF, |key| {
        crate::config::ask_proof(key, ask_id)
    }) {
        crate::core::ask::AnswerPath::Window
    } else {
        crate::core::ask::AnswerPath::Cli
    }
}

/// The header carrying the window's proof for an answer.
const ASK_PROOF: &str = "x-devplane-ask-proof";
/// The header carrying the window's proof for trusting a repository.
const TRUST_PROOF: &str = "x-devplane-trust-proof";

/// Whether `header` holds the proof `make` computes from the window key.
/// False when the host holds no key (the CLI host), whatever the request says.
fn window_proved(
    state: &Shared,
    headers: &HeaderMap,
    header: &str,
    make: impl FnOnce(&str) -> String,
) -> bool {
    let Some(key) = state.window_key.get() else {
        return false;
    };
    let Some(sent) = headers.get(header).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    constant_time_eq(
        sent.trim().to_ascii_lowercase().as_bytes(),
        make(key).as_bytes(),
    )
}

macro_rules! guard {
    ($state:expr, $headers:expr) => {
        if !authorised(&$state, &$headers) {
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

/// What the page may load and where it may send things: this host and
/// nothing else. The page renders agent-written text and holds the bearer
/// token, so a script that got in could reach no other origin with it, and no
/// other page may frame it. Inline styles are Svelte's own `style=`.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; \
     object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// The headers every interface file is served with.
fn asset_headers(path: &str) -> [(axum::http::HeaderName, &'static str); 5] {
    use axum::http::header;
    [
        (header::CONTENT_TYPE, content_type_of(path)),
        (header::CACHE_CONTROL, "no-store"),
        (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY),
        // The token is read from the address once and removed; no page this
        // one links to is ever told where it came from.
        (header::REFERRER_POLICY, "no-referrer"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ]
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
            Ok(bytes) => return (asset_headers(want), bytes).into_response(),
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
    (asset_headers(found.path), found.bytes).into_response()
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
    if !authorised_to_ingest(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorised"})),
        )
            .into_response();
    }
    ingest_otel(state, &body, crate::observe::otel::parse_logs).await
}

/// OTLP/HTTP traces, written to the GenAI semantic conventions.
async fn otel_traces(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if !authorised_to_ingest(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorised"})),
        )
            .into_response();
    }
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
        // A report of who decided is filled with what its attribution needs
        // before it is kept; the row it establishes, if any, is written after.
        let (event, row) = match rec.event {
            e @ crate::core::event::Event::VendorDecided { .. } => {
                let facts = {
                    let w = state.world.lock().await;
                    let r = w.run(&run);
                    crate::record::ReportedRun {
                        cwd: r.map(|r| r.cwd.clone()).filter(|c| c.is_absolute()),
                        project: r.and_then(|r| r.project_id.clone()),
                        entrypoint: rec
                            .entrypoint
                            .clone()
                            .or_else(|| r.and_then(|r| r.entrypoint.clone())),
                        driven: r.is_some_and(|r| r.mode == RunMode::Driven),
                    }
                };
                crate::record::vendor_report(&state.store, &run, &facts, e).await
            }
            e => (e, None),
        };
        state
            .ingest(run.clone(), Source::Otel, event, None, RunMode::Observed)
            .await;
        if let Some(d) = row {
            crate::record::append_vendor_row(&state.store, &d).await;
        }
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
    let held = state.hold.held();
    let mut view = crate::view::inbox(
        &snap,
        &state.store,
        &state.policy,
        &q,
        held.as_ref().map(|h| h.ask.as_str()),
    )
    .await;
    view.hold(held, state.hold.landed());
    Json(view).into_response()
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

    // Per project, which state its GitHub is in: read, stale, not signed in,
    // expired, rate limited, unreachable, or not a GitHub repository — so no
    // surface draws an empty list where a failure belongs.
    let projects: Vec<serde_json::Value> = names
        .iter()
        .map(|(id, name)| {
            let skipped = f.skip.get(id);
            let pf = f.projects.get(id);
            let host = pf
                .and_then(|p| p.host.clone())
                .or_else(|| f.repos.get(id).map(|r| r.host.clone()));
            let github = match (skipped, &host) {
                (Some((why, _)), _) => json!({"state": "not_github", "why": why}),
                (None, Some(h)) => serde_json::to_value(state.github.view(h)).unwrap_or_default(),
                // Not polled yet: nothing is known about its remote.
                (None, None) => json!({"state": "unknown"}),
            };
            json!({
                "project": id.to_string(),
                "project_name": name,
                "repo": pf.and_then(|p| p.repo.clone()).or_else(|| f.repos.get(id).map(|r| r.slug())),
                "host": host,
                "github": github,
                "stale": pf.and_then(|p| p.error.clone()),
                "read_at": pf.filter(|p| p.error.is_none() || !p.issues.is_empty() || !p.pull_requests.is_empty())
                    .map(|p| p.fetched_at.to_string()),
                "issues_more": pf.map(|p| p.issues_more()).unwrap_or(0),
                "pull_requests_more": pf.map(|p| p.pull_requests_more()).unwrap_or(0),
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
        // The configured host's sign-in, for the page as a whole.
        "github": state.github.view(state.github.default_host()),
        "projects": projects,
        "issues_more": f.projects.values().map(|p| p.issues_more()).sum::<usize>(),
        "pull_requests_more": f.projects.values().map(|p| p.pull_requests_more()).sum::<usize>(),
    }))
    .into_response()
}

/// Every GitHub host's sign-in: state, login, scopes, a pending code. Never
/// the token.
async fn github_state(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    Json(json!({
        "hosts": state.github.views(),
        // The configured host's; each host says its own as `device_flow`.
        "client_id": state.github.client_id(state.github.default_host()).is_some(),
    }))
    .into_response()
}

#[derive(Deserialize, Default)]
struct GitHubHostBody {
    #[serde(default)]
    host: Option<String>,
}

/// A refusal from the GitHub hub, as the status the contract names.
fn github_refusal(e: &crate::github::Error) -> axum::response::Response {
    use crate::github::Error as E;
    let code = match e {
        E::NoClientId => StatusCode::CONFLICT,
        E::NoCredentialStore(_) | E::Store(_) => StatusCode::SERVICE_UNAVAILABLE,
        E::Unreachable(_) | E::RateLimited { .. } => StatusCode::BAD_GATEWAY,
        _ => StatusCode::BAD_GATEWAY,
    };
    (code, Json(json!({"error": e.to_string()}))).into_response()
}

/// Starts the device flow for a host, or returns the one already waiting, so
/// the window and the CLI show one code. The poll runs here, in the host.
async fn github_login(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: Option<Json<GitHubHostBody>>,
) -> impl IntoResponse {
    guard!(state, headers);
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let host = state.github.host_or_default(body.host.as_deref());
    let already = matches!(
        state.github.view(&host).sign_in,
        crate::github::SignIn::Pending { .. }
    );
    match state.github.start(&host).await {
        Ok(pending) => {
            if !already {
                let hub = state.github.clone();
                let tell = state.clone();
                tokio::spawn(async move {
                    match hub.wait(pending).await {
                        Ok(v) => {
                            tracing::info!(login = %v.login, "signed in to GitHub");
                            tell.notify_changed();
                            // The surface that said *not signed in* fills now,
                            // not at the next tick.
                            crate::poller::forge_now(&tell).await;
                        }
                        Err(e) => tracing::info!(error = %e, "a GitHub sign-in ended"),
                    }
                    tell.notify_changed();
                });
            }
            state.notify_changed();
            Json(state.github.view(&host)).into_response()
        }
        Err(e) => github_refusal(&e),
    }
}

/// Deletes a host's token and every copy the host holds, and says where to
/// revoke the grant at GitHub.
async fn github_logout(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: Option<Json<GitHubHostBody>>,
) -> impl IntoResponse {
    guard!(state, headers);
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let host = state.github.host_or_default(body.host.as_deref());
    match state.github.logout(&host).await {
        Ok(from) => {
            {
                let mut f = state.forge.lock().await;
                let gone: Vec<_> = f
                    .projects
                    .iter()
                    .filter(|(_, p)| p.host.as_deref() == Some(host.as_str()))
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in gone {
                    f.projects.remove(&id);
                }
                if host == state.github.default_host() {
                    f.viewer = None;
                }
            }
            state.notify_changed();
            let mut v = serde_json::to_value(state.github.view(&host)).unwrap_or_default();
            // `null` when the host was not signed in and nothing was deleted.
            v["deleted_from"] = json!(from);
            v["revoke_at"] = json!(crate::github::revoke_url(&host));
            Json(v).into_response()
        }
        Err(e) => github_refusal(&e),
    }
}

/// Says a sign-in changed outside this host — `devplane auth login
/// --with-token` stores the token in its own process — so the copy this host
/// holds is dropped and the forge is read at once.
async fn github_refresh(
    State(state): State<Shared>,
    headers: HeaderMap,
    body: Option<Json<GitHubHostBody>>,
) -> impl IntoResponse {
    guard!(state, headers);
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let host = state.github.host_or_default(body.host.as_deref());
    state.github.forget_token(&host);
    let tell = state.clone();
    tokio::spawn(async move {
        crate::poller::forge_now(&tell).await;
        tell.notify_changed();
    });
    Json(state.github.view(&host)).into_response()
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
    let how = match q.how() {
        // A forge item has no session to report activity: only minutes.
        Ok(crate::core::attention::SnoozeFor::Activity) => {
            return bad_snooze(
                "a pull request or an issue has no session; snooze it for minutes".into(),
            );
        }
        Ok(h) => h,
        Err(why) => return bad_snooze(why),
    };
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
            jiff::Timestamp::now(),
        )
        .items
        .into_iter()
        .map(|i| i.kind)
        .collect();
        let entry = f.snoozed.entry(project.clone()).or_default();
        match crate::core::attention::snooze(
            entry,
            kinds,
            q.kind.as_deref(),
            how.clone(),
            Default::default(),
        ) {
            Ok(kinds) => kinds,
            Err(refused) => return never_snoozed(refused),
        }
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
    Json(json!({"snoozed_until": SnoozeQuery::says_until(&how), "kinds": dismissed}))
        .into_response()
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
/// `format!` — removing this is not a compile error. `devplane config explain`
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
    let (cache, _) = crate::policy_cache::PolicyCache::from_disk();
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

/// What the stop tool sends: the agent's reason and paths, plus what the host
/// gave its server — the change it was started for and the session's key.
/// There is no origin field: the run is the path, and the host resolves the
/// rest from the run it drives.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimBody {
    reason: String,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    change: Option<String>,
    #[serde(default)]
    key: String,
}

/// Records a stop claim, then stops the run once the answer is on its way.
/// Every refusal is a sentence, and nothing is written for any of them.
async fn claim_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ClaimBody>,
) -> impl IntoResponse {
    use crate::core::claim::ClaimRefusal as R;
    guard!(state, headers);
    // Never a prefix: a claim names its own run exactly.
    let run = RunId::new(id);
    let draft = crate::core::claim::ClaimDraft {
        reason: body.reason,
        paths: body.paths,
    };
    match crate::change::record_claim(&state, &run, draft, body.change, &body.key).await {
        Ok(claim) => {
            let stopper = state.clone();
            let stopped = run.clone();
            tokio::spawn(async move {
                // After the answer has reached the agent, so it reads that it
                // was recorded before its turn is cancelled.
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                crate::driven::stop_claimed(&stopper, &stopped).await;
            });
            Json(json!({
                "recorded": true,
                "claim": claim.id,
                "change": claim.origin.change(),
                "run": run,
                "says": format!(
                    "Recorded: {} — change {}, run {run}. Your person reads this before deciding \
                     anything about the change; it is your claim, not a failure, and it does not \
                     change whether the checks pass. Devplane is stopping this run now; do nothing \
                     further. Your person answers by resuming it.",
                    crate::core::claim::SAYS,
                    claim.origin.change(),
                ),
            }))
            .into_response()
        }
        Err(refusal) => {
            let status = match &refusal {
                R::NotDriven(_) => StatusCode::NOT_FOUND,
                R::BadKey => StatusCode::FORBIDDEN,
                R::EmptyReason | R::TooLarge { .. } | R::PathEscapes(_) | R::UnknownArgument(_) => {
                    StatusCode::UNPROCESSABLE_ENTITY
                }
                R::NoRun
                | R::NoHost
                | R::NoChange
                | R::WrongChange(_)
                | R::Stopping
                | R::AlreadyOpen { .. } => StatusCode::CONFLICT,
            };
            (
                status,
                Json(json!({"recorded": false, "refused": refusal.name(), "says": refusal.says()})),
            )
                .into_response()
        }
    }
}

/// A person marks a change's open stop claim read: it leaves what asks for
/// attention and stays on the change. No decision row.
async fn read_claim(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let open = state.changes.lock().await.get(&id).and_then(|c| {
        c.open_claim()
            .map(|o| (o.id.clone(), o.origin.run().clone()))
    });
    match open {
        Some((claim, run)) => {
            crate::change::close_claims(&state, &id, crate::core::claim::ClosedBy::Read).await;
            Json(json!({
                "closed": claim,
                "says": format!("marked read — it stays on the change under run {run}"),
            }))
            .into_response()
        }
        None => (
            StatusCode::CONFLICT,
            Json(json!({"error": "this change has no open claim", "says": "this change has no open claim"})),
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
    /// Ignored: where an answer came from is what the host can prove
    /// ([`path_of`]), never what a body claims.
    #[serde(default, rename = "from")]
    _from: Option<String>,
    /// Every question's answer at once, for a form that asked several; a row
    /// is answered only once.
    #[serde(default)]
    answers: Vec<crate::driven::FieldAnswer>,
    /// The window's answers are held for a few seconds so a slip can be taken
    /// back before it reaches the agent ([`crate::hold`]). Without it the
    /// answer is sent at once, as the command line's always is.
    #[serde(default)]
    hold: bool,
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
                "no ask `{id}` is waiting.\n\n  devplane inbox --all lists every question and \
                 what became of it."
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

    // Proved now, from this request, and kept with a held answer: the path
    // recorded at commit is the one this request proved.
    let from = path_of(&state, &headers, &id);
    if body.hold {
        // Refused now, not five seconds from now when nobody is looking.
        if !ask.is_open() {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error": "that question is no longer waiting — it was answered or withdrawn"})),
            )
                .into_response();
        }
        if let Err(e) = crate::driven::check_answer(&ask, &answer) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
        let (held, committed) = crate::hold::hold(&state, &ask, answer, from).await;
        // The held item leaves the count: the tray reads it again.
        state.notify_changed();
        return (
            StatusCode::ACCEPTED,
            Json(json!({"held": held, "committed": committed})),
        )
            .into_response();
    }

    match crate::driven::answer_ask(&state, &id, answer, from).await {
        Ok(a) => {
            acted_on_ask(&state, a.run.as_str()).await;
            Json(crate::view::render_ask(&a)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Takes back the answer the window is holding for this ask, before it
/// reaches the agent: nothing was recorded or delivered, and the ask is open
/// again. An answer the window held is taken back only with the window's proof
/// for this ask.
async fn undo_answer(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let window = path_of(&state, &headers, &id) == crate::core::ask::AnswerPath::Window;
    match state.hold.withdraw(&id, window) {
        Ok(()) => {
            // The item is back in the list and the count.
            state.notify_changed();
            Json(json!({
            "withdrawn": id,
            "says": "taken back — nothing reached the agent, and the question is waiting again",
            }))
            .into_response()
        }
        Err(refused) => (
            StatusCode::CONFLICT,
            Json(json!({"error": refused.says(), "says": refused.says()})),
        )
            .into_response(),
    }
}

/// Records that an answered ask's inbox item was acted on.
pub(crate) async fn acted_on_ask(state: &Shared, run: &str) {
    acted_on(
        state,
        run,
        &[
            crate::core::AttentionKind::Permission,
            crate::core::AttentionKind::Question,
        ],
        crate::core::attention::Resolution::Acted,
    )
    .await;
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
    async fn words(
        &self,
        hub: &crate::github::GitHub,
        targets: &[String],
    ) -> Result<(String, String), String> {
        let title = self.title.clone();
        let prompt = self.prompt.clone().unwrap_or_else(|| title.clone());
        let Some(number) = self.issue else {
            return Ok((title, prompt));
        };
        let [dir] = targets else {
            return Err("an issue is one repository's: start from it in one project".into());
        };
        // Asked by number: an issue older than any list is still one to start.
        match crate::github::issue(hub, std::path::Path::new(dir), number)
            .await
            .map_err(|e| e.to_string())?
        {
            Some((issue, true)) => {
                Ok((format!("#{} {}", issue.number, issue.title), issue.prompt()))
            }
            Some((_, false)) => Err(format!("issue #{number} is closed")),
            None => Err(format!("this repository has no issue #{number}")),
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
            "notes": f.notes.iter()
                .filter(|n| !matches!(n, crate::core::preflight::Note::NoCheck { .. }))
                .map(crate::core::preflight::Note::says)
                .collect::<Vec<_>>(),
            // No `check`, said apart from the costs: the window offers
            // *Declare a check* beside it.
            "no_check": f.notes.iter()
                .find(|n| matches!(n, crate::core::preflight::Note::NoCheck { .. }))
                .map(crate::core::preflight::Note::says),
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
    let prompt = match body.words(&state.github, &targets).await {
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
    let (title, prompt) = match body.words(&state.github, &targets).await {
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
    // One change is read now, not from the last tick: it states *verified*.
    crate::change::refresh_tree(&state, &id).await;
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
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    // `{"base_checks": true}`: after a passing `check`, the pair too, waited
    // for. The reply's `passed` is still `check`'s alone.
    let with_base = body
        .as_ref()
        .and_then(|b| b.get("base_checks"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    match crate::change::verify(&state, &id).await {
        Ok(report) => {
            let mut reply = json!({
                "passed": report.passed(),
                "summary": report.summary(),
                "report": report,
            });
            if with_base && report.passed() {
                let absent = crate::base_checks::ensure(&state, &id).await;
                reply["base_checks"] = base_checks_of(&state, &id).await;
                if let Ok(Some(a)) = absent {
                    reply["base_checks_refused"] = json!(a.says());
                }
            }
            Json(reply).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// A change's base checks as its view composes them.
async fn base_checks_of(state: &Shared, id: &crate::core::ChangeId) -> serde_json::Value {
    let change = state.changes.lock().await.get(id).cloned();
    let config = crate::change::governing_config(state, id)
        .await
        .ok()
        .map(|(_, c)| c);
    change
        .and_then(|c| crate::base_checks::view(&c, config.as_ref()))
        .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        .unwrap_or(serde_json::Value::Null)
}

/// Starts base checks for a change's current tree: 202 when started or
/// already running, 200 when a current result exists, 409 with the sentence
/// when none may run.
async fn base_checks_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::base_checks::start(&state, &id).await {
        Ok(crate::base_checks::Started::Running(_)) => {
            (StatusCode::ACCEPTED, Json(json!({"started": true}))).into_response()
        }
        Ok(crate::base_checks::Started::Current) => Json(json!({"current": true})).into_response(),
        Ok(crate::base_checks::Started::Refused(a)) => (
            StatusCode::CONFLICT,
            Json(json!({"refused": a, "says": a.says()})),
        )
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
    body: Option<Json<ResumeBody>>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let body = body.map(|Json(b)| b).unwrap_or_default();
    // Another agent than the one that last drove it: a new run in the same
    // worktree, told only the person's words. The same agent resumes below.
    if let Some(agent) = body.agent.as_deref() {
        let prompt = body.prompt.as_deref().unwrap_or_default();
        match crate::change::resume_with_agent(&state, &id, agent, prompt).await {
            Ok(Some(switched)) => {
                acted_on(
                    &state,
                    id.as_str(),
                    &[crate::core::AttentionKind::Interrupted],
                    crate::core::attention::Resolution::Acted,
                )
                .await;
                return Json(json!({"ok": true, "switched": switched})).into_response();
            }
            Ok(None) => {}
            Err(e) => {
                let status = match e.to_string().contains("still running") {
                    true => StatusCode::CONFLICT,
                    false => StatusCode::BAD_REQUEST,
                };
                return (status, Json(json!({"error": e.to_string()}))).into_response();
            }
        }
    }
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

/// What a resume may name: another agent to continue with, and the person's
/// words for it. Both absent resumes the change's own session.
#[derive(Deserialize, Default)]
struct ResumeBody {
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
}

#[derive(Deserialize)]
struct LoginBody {
    method: String,
    /// The person ran the agent's own terminal login; start it again.
    #[serde(default)]
    after_terminal: bool,
}

/// Starts a change's agent again with the sign-in method the person chose.
/// Never carries a credential.
async fn login_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<LoginBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::change::login(&state, &id, &body.method, body.after_terminal).await {
        Ok(run) => Json(json!({"ok": true, "run": run})).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(json!({"error": e.to_string()}))).into_response(),
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

/// Marks a unit of a change's review seen: a person read it. `digest` names
/// the hunk; without it every current unit at `path` is marked.
#[derive(Deserialize)]
struct SeenBody {
    path: String,
    #[serde(default)]
    digest: Option<String>,
}

async fn mark_review_seen(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SeenBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    let snap = state.snapshot().await;
    match crate::view::mark_seen(&snap, &state.store, &id, &body.path, body.digest.as_deref())
        .await
    {
        Ok(marked) => {
            state.notify_changed();
            Json(marked).into_response()
        }
        Err(crate::view::SeenRefusal::NoSuchChange) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no change `{id}`")})),
        )
            .into_response(),
        Err(crate::view::SeenRefusal::NoSuchUnit(digests)) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": match &body.digest {
                    Some(_) => format!("`{}` has no hunk with that content now — it changed", body.path),
                    None => format!("`{}` has nothing in this change to mark seen", body.path),
                },
                "digests": digests,
            })),
        )
            .into_response(),
    }
}

/// A change's review comments, oldest first, with how many are unsent.
async fn list_comments(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match state.store.comments(id.as_str()).await {
        Ok(all) => {
            let unsent = all.iter().filter(|c| c.sent_at.is_none()).count();
            Json(json!({ "comments": all, "unsent": unsent })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

/// Keeps a comment on a line the diff shows; sends nothing.
async fn create_comment(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<crate::comments::NewComment>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::comments::create(&state, &id, body).await {
        Ok(c) => (StatusCode::CREATED, Json(json!({ "comment": c }))).into_response(),
        Err(crate::comments::Refused::NoSuchChange) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no change `{id}`")})),
        )
            .into_response(),
        Err(crate::comments::Refused::NotThere(why)) => {
            (StatusCode::CONFLICT, Json(json!({ "error": why }))).into_response()
        }
    }
}

/// Deletes an unsent comment; a sent one stays.
async fn delete_comment(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path((id, cid)): Path<(String, String)>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::comments::delete(&state, &id, &cid).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(json!({"error": format!("{e:#}")})),
        )
            .into_response(),
    }
}

/// Sends every unsent comment as one prompt, to the change's live run, a
/// resumed session or a run started on it — or says why nothing was sent.
async fn send_comments(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::comments::send(&state, &id).await {
        Ok(Some(sent)) => Json(sent).into_response(),
        Ok(None) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "nothing to send", "says": "nothing to send"})),
        )
            .into_response(),
        Err(e) => {
            let says = format!("{e:#}");
            (
                StatusCode::CONFLICT,
                Json(json!({"error": says, "says": says})),
            )
                .into_response()
        }
    }
}

async fn snooze_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<SnoozeQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    let how = match q.how() {
        Ok(h) => h,
        Err(why) => return bad_snooze(why),
    };
    let id = change_id!(state, id);
    // A drift is derived from the runs, so it is asked before the row is held.
    let drifting = crate::change::has_drift(&state, &id).await;
    // The change's activity is its runs' too, so they are read first.
    let last_events: std::collections::BTreeMap<crate::core::RunId, jiff::Timestamp> = {
        let w = state.world.lock().await;
        w.runs().map(|r| (r.id.clone(), r.last_event_at)).collect()
    };

    let saved = {
        let mut changes = state.changes.lock().await;
        match changes.get_mut(&id) {
            Some(w) => {
                let mark = crate::core::attention::ActivityMark::of_change(w, |r| {
                    last_events.get(r).copied()
                });
                // The kinds on screen now, never what turns up later.
                let mut kinds: Vec<_> = crate::core::attention::items_for_change(
                    w,
                    false,
                    false,
                    &mark,
                    jiff::Timestamp::now(),
                )
                .into_iter()
                .map(|i| i.kind)
                .collect();
                if drifting {
                    kinds.push(crate::core::AttentionKind::SpecDrifted);
                }
                match crate::core::attention::snooze(
                    &mut w.snoozed,
                    kinds,
                    q.kind.as_deref(),
                    how.clone(),
                    mark,
                ) {
                    Ok(kinds) => Some((w.clone(), kinds)),
                    Err(refused) => return never_snoozed(refused),
                }
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
    Json(json!({"ok": true, "until": SnoozeQuery::says_until(&how), "kinds": dismissed}))
        .into_response()
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
            // The state the board will show, from the same declared checks.
            let checks = crate::change::governing_root(&state, &id)
                .await
                .and_then(|root| crate::core::ProjectConfig::load(&root).ok())
                .map(|cfg| cfg.gates.check);
            let declared = crate::core::change::Declared::of(checks.as_deref());
            let c = state.changes.lock().await.get(&id).cloned();
            Json(json!({"ok": true, "state": c.map(|c| c.state(declared, c.tree_now.as_ref()))}))
                .into_response()
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
        // Structured, so the window names each row with an action.
        Err(e) if e.downcast_ref::<crate::change::WeakenedUnseen>().is_some() => {
            let w = e
                .downcast_ref::<crate::change::WeakenedUnseen>()
                .expect("checked above");
            (
                StatusCode::CONFLICT,
                Json(json!({
                    "refused": "weakened_unseen",
                    "says": crate::change::WeakenedUnseen::SAYS,
                    "error": e.to_string(),
                    "rows": w.rows,
                    "seen_with": w.seen_with(),
                })),
            )
                .into_response()
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
    // Read now, not from the last tick: the certificate states *verified*.
    crate::change::refresh_tree(&state, &id).await;
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

/// Trusting from the window. The control token is not enough: it sits in a
/// file an agent's tools can read, and trust lets a headless agent run the
/// repository's own code. So the route also wants the window's proof for this
/// path ([`crate::config::trust_proof`]); a host with no window refuses every
/// request, and a person trusts with `devplane project trust` in a terminal of
/// their own, which writes `trusted.toml` and has the host re-read it
/// ([`reload_projects`]). This narrows the hole and does not close it: an
/// agent that can drive the desktop can press the window's button.
async fn trust_project(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<TrustBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    if !window_proved(&state, &headers, TRUST_PROOF, |key| {
        crate::config::trust_proof(key, &body.path)
    }) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": format!(
                    "trusting a repository is a person's decision, and this request cannot show \
                     it came from the Devplane window. In a terminal of your own:\n\n  \
                     devplane project trust {}",
                    body.path
                )
            })),
        )
            .into_response();
    }
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
            // stays true after a restart. The project row is history; the
            // trust is the person's file.
            let saved = match state.store.save_project(&p).await {
                Ok(()) => crate::trust::add(&state.home, [p.root.clone()]).map(|_| ()),
                Err(e) => Err(e),
            };
            if let Err(e) = saved {
                let e = format!("{e:#}");
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
            // Trust lets a headless agent run this repository's own hooks
            // and MCP servers, so it is a decision, and the record says who.
            state
                .record(crate::core::Decision::new(
                    crate::core::Authority::Person,
                    "project:trust",
                    p.root.display().to_string(),
                    "trusted",
                ))
                .await;
            Json(json!({"trusted": true, "project": p})).into_response()
        }
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "could not register the project"})),
        )
            .into_response(),
    }
}

/// Re-reads the projects the store records and the repositories
/// `trusted.toml` names, so trust a person wrote with `devplane project
/// trust` while this host runs takes effect at once rather than at the next
/// start. Grants nothing the file does not already say, and takes nothing
/// back: a project's trust only ever merges in.
async fn reload_projects(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let projects = match state.store.load_projects().await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("could not read the projects: {e}")})),
            )
                .into_response();
        }
    };
    let trusted = match crate::trust::Trusted::read(&state.home) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("could not read the trusted repositories: {e:#}")})),
            )
                .into_response();
        }
    };
    let mut w = state.world.lock().await;
    let mut ids = std::collections::BTreeSet::new();
    for p in projects.into_iter().chain(trusted.projects()) {
        ids.insert(w.upsert_project(p));
    }
    Json(json!({"projects": ids.len()})).into_response()
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
    match crate::github::issues(&state.github, &dir, label.as_deref(), body.limit).await {
        // In the board's shape, as `/api/forge` lists issues.
        Ok(issues) => {
            let me = crate::github::remote::of_dir(&dir)
                .await
                .ok()
                .and_then(|r| state.github.signed_in(&r.host))
                .map(|v| v.login);
            let rows: Vec<crate::core::ForgeIssue> =
                issues.iter().map(|i| i.to_forge(me.as_deref())).collect();
            Json(rows).into_response()
        }
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
    Json(crate::view::specs(&snap, &state.store).await).into_response()
}

/// What a person can start a change in, and whether an agent may start
/// there. Every project is listed, running or not.
async fn projects(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::projects(&snap)).into_response()
}

async fn modes(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    let snap = state.snapshot().await;
    Json(crate::view::modes(&snap)).into_response()
}

#[derive(Deserialize)]
struct StreamQuery {
    /// Narrow to one run.
    #[serde(default)]
    run: Option<String>,
}

#[derive(Deserialize)]
struct SnoozeQuery {
    /// Minutes to stay quiet. Zero un-snoozes, which is the only way back.
    #[serde(default = "default_snooze")]
    minutes: i64,
    /// `activity`: stay quiet until the subject next does something, rather
    /// than for `minutes`.
    #[serde(default)]
    until: Option<String>,
    /// One kind on the subject rather than every kind on screen.
    #[serde(default)]
    kind: Option<String>,
}

impl SnoozeQuery {
    /// What the request asks for, or why it cannot be read.
    fn how(&self) -> Result<crate::core::attention::SnoozeFor, String> {
        use crate::core::attention::SnoozeFor;
        match self.until.as_deref() {
            Some("activity") => Ok(SnoozeFor::Activity),
            Some(other) => Err(format!("`until` takes `activity`, not `{other}`")),
            None => Ok(match snooze_until(self.minutes) {
                Some(t) => SnoozeFor::At(t),
                None => SnoozeFor::Back,
            }),
        }
    }

    /// The `until` a response names: a moment, `activity`, or nothing.
    fn says_until(how: &crate::core::attention::SnoozeFor) -> Option<String> {
        use crate::core::attention::SnoozeFor;
        match how {
            SnoozeFor::At(t) => Some(t.to_string()),
            SnoozeFor::Activity => Some("activity".to_string()),
            SnoozeFor::Back => None,
        }
    }
}

/// A snooze request that cannot be read.
fn bad_snooze(why: String) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": why, "says": why})),
    )
        .into_response()
}

/// A snooze refused because a question or a permission is among the kinds:
/// those wait for an answer, and nothing was hidden.
fn never_snoozed(n: crate::core::attention::NeverSnoozed) -> axum::response::Response {
    let kinds: Vec<&str> = n.0.iter().map(|k| k.as_str()).collect();
    (
        StatusCode::CONFLICT,
        Json(json!({"error": n.says(), "says": n.says(), "kinds": kinds})),
    )
        .into_response()
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
    let how = match q.how() {
        Ok(h) => h,
        Err(why) => return bad_snooze(why),
    };

    let run_id = run_id!(state, id);
    let now = jiff::Timestamp::now();
    let saved = {
        let mut w = state.world.lock().await;
        // The run's own project threshold, the one its inbox row was
        // derived with, so the snooze hides exactly what was on screen.
        let stall = w
            .run(&run_id)
            .and_then(|r| state.policy.stall_seconds(r.working_dir()))
            .unwrap_or(w.attention.stall_seconds);
        match w.snooze(&run_id, q.kind.as_deref(), how.clone(), now, stall) {
            None => None,
            Some(Err(refused)) => return never_snoozed(refused),
            Some(Ok(dismissed)) => w.run(&run_id).cloned().map(|r| (r, dismissed)),
        }
    };
    let Some((run, dismissed)) = saved else {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "no such run"}))).into_response();
    };
    // Named once hidden: a dismissal is the clearest signal that a kind is
    // too loud.
    acted_on(
        &state,
        run_id.as_str(),
        &dismissed,
        crate::core::attention::Resolution::Dismissed,
    )
    .await;
    // Persisted, so a snooze survives a restart.
    if let Err(e) = state.store.save_run(&run).await {
        tracing::warn!(error = %e, "could not persist snooze");
    }
    state.notify_changed();
    Json(json!({"snoozed_until": SnoozeQuery::says_until(&how), "kinds": dismissed}))
        .into_response()
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

#[derive(Deserialize)]
struct UnattributedQuery {
    #[serde(default)]
    about: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

/// The calls a vendor reported deciding that no authority can be named for,
/// each with its reason: listed beside the decisions, never as one.
async fn unattributed(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<UnattributedQuery>,
) -> impl IntoResponse {
    guard!(state, headers);
    match crate::view::unattributed(&state.store, q.about.as_deref(), q.limit).await {
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
    let q = crate::core::reduce::facts::quitting(
        snap.world.runs(),
        &snap.open_asks,
        state.hold.summary(),
    );
    // The window prints `says` verbatim, so both readers share one sentence.
    let mut v = serde_json::to_value(&q).unwrap_or_else(|_| json!({}));
    v["says"] = json!(q.says());
    Json(v).into_response()
}

/// Asks the host to stop (`devplane down`). A request rather than a signal
/// to the recorded pid, which may since belong to another process; it reaches
/// the same graceful path as ctrl-c on every platform.
async fn quit(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    guard!(state, headers);
    // Dropped before the reply: a held answer must not be sent in the moment
    // between the request and the stop.
    state.hold.drop_on_stop();
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
    use crate::view::{
        AutoModeView, Diagnostics, DiagnosticsForge, DiagnosticsGate, DiagnosticsModes,
        LeakedAgent, OpenCodeHealth, SkippedProject, UnreadableConfig, UnreadableRow, Unwritten,
    };
    guard!(state, headers);
    let channels = state.store.channel_health().await.unwrap_or_default();
    // Not configured is not unhealthy, and the two must not render the same.
    let opencode = match std::env::var(crate::observe::opencode::ENV_SERVER) {
        Ok(server) => {
            let h = state.opencode.lock().await.clone();
            Some(OpenCodeHealth {
                server,
                says: h.says(),
                live: matches!(h, crate::observe::opencode::Health::Live { .. }),
            })
        }
        Err(_) => None,
    };
    // A `devplane.toml` that will not load keeps cached rules, but a host
    // restarted against it has none — so that repository's `deny` list is
    // gone, and this says so.
    let unreadable_configs = state
        .policy
        .broken()
        .into_iter()
        .map(|(root, error)| UnreadableConfig {
            project: root.display().to_string(),
            error,
        })
        .collect();
    // Rows this build cannot decode are absent from the board. A lost Change
    // row orphans a worktree, so they are counted here.
    let unreadable_rows = state
        .store
        .unreadable()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(row, error)| UnreadableRow { row, error })
        .collect();
    // Before the lock: it shells out.
    let auto_mode = match crate::observe::automode::effective().await {
        Ok(cfg) => AutoModeView::Read {
            configured: !cfg.is_empty(),
            environment: cfg.environment.len(),
            allow: cfg.allow.len(),
            soft_deny: cfg.soft_deny.len(),
            hard_deny: cfg.hard_deny.len(),
        },
        Err(unavailable) => AutoModeView::Unavailable { unavailable },
    };
    // Whether the inbox is worth reading: what each kind raised in the last
    // week and what became of it. Measures the tool, not the person.
    let attention = crate::view::attention(&state.store, 7).await.ok();
    let forge = {
        let f = state.forge.lock().await;
        DiagnosticsForge {
            viewer: f.viewer.clone(),
            error: f.error.clone(),
            github: state.github.views(),
            last_poll_at: f.last_poll_at.map(|t| t.to_string()),
            projects: f.projects.len(),
            skipped: f.skip.len(),
            // The reason (usually "no GitHub remote") is what a person can act on.
            skipped_projects: f
                .skip
                .iter()
                .map(|(id, (why, at))| SkippedProject {
                    project: id.to_string(),
                    reason: why.to_string(),
                    at: at.to_string(),
                })
                .collect(),
        }
    };
    let leaked_agents = state
        .leaked_agents
        .lock()
        .await
        .iter()
        .map(|(pid, command, worktree)| LeakedAgent {
            pid: *pid,
            command: command.clone(),
            worktree: worktree.clone(),
        })
        .collect();
    let (lost_events, lost_decisions, last_loss) = state.unwritten.counts();
    let decision_sources = crate::view::decision_sources(&state.store).await.ok();
    let w = state.world.lock().await;
    let live = || w.runs().filter(|r| r.state.is_live());
    Json(Diagnostics {
        version: env!("CARGO_PKG_VERSION").to_string(),
        decision_sources,
        gate: DiagnosticsGate {
            syntax_modelled_on: crate::core::policy::SYNTAX_MODELLED_ON.to_string(),
            // "Nothing reports a version" is a real diagnostic.
            sessions_reporting_a_version: live().filter(|r| r.claude_version.is_some()).count(),
        },
        modes: DiagnosticsModes {
            clock_read: live().filter(|r| r.question_clock_read.is_some()).count(),
            clock_unread: live().filter(|r| r.question_clock_read.is_none()).count(),
        },
        leaked_agents,
        attention,
        pid: std::process::id(),
        started_at: state.started_at.to_string(),
        uptime_seconds: (jiff::Timestamp::now() - state.started_at).get_seconds(),
        summary: w.summary(jiff::Timestamp::now()),
        channels,
        opencode,
        forge,
        stall_seconds: w.attention.stall_seconds,
        unreadable_configs,
        unreadable_rows,
        unwritten: Unwritten {
            events: lost_events,
            decisions: lost_decisions,
            last_reason: last_loss,
        },
        auto_mode,
        notifications: state.notifications.get().map(|line| line()),
    })
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
    if !authorised(&state, &headers) {
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
                        // A refresh is about no one run, so it reaches every
                        // subscriber: a change or gate update is news to a
                        // watcher of one run too.
                        let refresh = matches!(
                            &frame,
                            crate::core::Frame::Event(e) if matches!(e.event, crate::core::Event::Refresh)
                        );
                        if let Some(want) = &only
                            && frame.run_id() != want
                            && !refresh
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

/// When a snooze of `minutes` ends: none for zero or less, and at most thirty
/// days, so a huge number is a long snooze rather than an overflow panic.
fn snooze_until(minutes: i64) -> Option<jiff::Timestamp> {
    const MAX_MINUTES: i64 = 30 * 24 * 60;
    (minutes > 0).then(|| {
        let span = jiff::SignedDuration::from_mins(minutes.min(MAX_MINUTES));
        jiff::Timestamp::now()
            .checked_add(span)
            .unwrap_or(jiff::Timestamp::MAX)
    })
}

#[cfg(test)]
mod snooze_tests {
    #[test]
    fn a_huge_snooze_is_long_not_a_panic() {
        assert!(super::snooze_until(i64::MAX).is_some());
        assert!(super::snooze_until(0).is_none());
        assert!(super::snooze_until(-5).is_none());
    }
}

// ---------------------------------------------------------------------------
// Drafting a specification by the project's own methodology
// ---------------------------------------------------------------------------

/// A draft request: the change's name, the agent, the methodology where the
/// project has two, and the person's words, sent verbatim.
#[derive(Deserialize)]
struct DraftBody {
    /// A registered project's id or name, or its root.
    project: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    methodology: Option<String>,
    #[serde(default)]
    intent: String,
    /// Draft again into this change, whose last draft was rejected.
    #[serde(default)]
    change: Option<String>,
    /// Say what would be sent, and create nothing.
    #[serde(default)]
    preview: bool,
}

/// A refusal as every draft route says it: the cause, its sentence, and the
/// sentence again under `error`, which every client reads.
fn draft_refused(status: StatusCode, r: &crate::draft::Refused) -> axum::response::Response {
    (
        status,
        Json(json!({"refused": r.cause, "says": r.says, "error": r.says})),
    )
        .into_response()
}

/// The root a request names: a registered project's id, name or root, else
/// the path as given.
async fn project_root_of(state: &Shared, named: &str) -> std::path::PathBuf {
    let w = state.world.lock().await;
    let path = std::path::Path::new(named);
    let canonical = path.canonicalize().ok();
    w.projects()
        .find(|p| {
            p.id.as_str() == named
                || p.name == named
                || canonical.as_deref() == Some(p.root.as_path())
        })
        .map(|p| p.root.clone())
        .unwrap_or_else(|| path.to_path_buf())
}

/// Starts a draft, or with `preview` says what it would send. Every refusal
/// comes before anything is made.
async fn draft_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<DraftBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let methodology = match body.methodology.as_deref().filter(|m| !m.trim().is_empty()) {
        None => None,
        Some(m) => match crate::core::draft::Methodology::parse(m) {
            Some(m) => Some(m),
            None => {
                return draft_refused(
                    StatusCode::CONFLICT,
                    &crate::draft::Refused {
                        cause: "methodology",
                        says: format!(
                            "`{m}` is not a methodology drafting dispatches: speckit or openspec"
                        ),
                    },
                );
            }
        },
    };
    let change = match body.change.as_deref() {
        Some(c) => Some(change_id!(state, c.to_string())),
        None => None,
    };
    let req = crate::draft::DraftRequest {
        project: project_root_of(&state, &body.project).await,
        name: body.name,
        agent: body.agent,
        methodology,
        intent: body.intent,
        change,
    };
    let status_of = |r: &crate::draft::Refused| match r.cause {
        "empty_intent" | "no_name" => StatusCode::BAD_REQUEST,
        _ => StatusCode::CONFLICT,
    };
    if body.preview {
        return match crate::draft::preview(&req) {
            Ok(p) => Json(json!(p)).into_response(),
            Err(r) => {
                // The methodologies on offer, so a surface can ask for one.
                let offered = req
                    .project
                    .canonicalize()
                    .map(|root| crate::draft::offered(&root))
                    .unwrap_or_default();
                let mut out = json!({"refused": r.cause, "says": r.says, "error": r.says});
                out["offered"] = json!(offered);
                (status_of(&r), Json(out)).into_response()
            }
        };
    }
    match crate::draft::start(&state, req).await {
        Ok(started) => {
            let c = state.changes.lock().await.get(&started.change).cloned();
            Json(json!({
                "change": started.change,
                "change_id": started.change,
                "run": started.run,
                "sent": started.sent,
                "folder": started.folder,
                "says": started.says,
                "worktree": c.as_ref().and_then(|c| c.worktree.clone()),
                "branch": c.as_ref().and_then(|c| c.branch.clone()),
            }))
            .into_response()
        }
        Err(r) => draft_refused(status_of(&r), &r),
    }
}

/// The person accepts the drafted folder as the change's specification.
async fn accept_draft(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::draft::accept(&state, &id).await {
        Ok(spec) => Json(json!({
            "spec": spec,
            "says": format!("accepted — {spec} is the specification this change answers; recorded as yours"),
        }))
        .into_response(),
        Err(r) => draft_refused(StatusCode::CONFLICT, &r),
    }
}

/// The person rejects the draft: the worktree is put back.
async fn reject_draft(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::draft::reject(&state, &id).await {
        Ok(removed) => Json(json!({
            "removed": removed,
            "says": "rejected — the worktree is back where it was at dispatch; the record is kept",
        }))
        .into_response(),
        Err(r) => draft_refused(StatusCode::CONFLICT, &r),
    }
}

/// One marker line answered: the diff, and the write only on `confirm`.
#[derive(Deserialize)]
struct ClarifyBody {
    path: String,
    line: u32,
    text: String,
    answer: String,
    #[serde(default)]
    confirm: bool,
}

async fn clarify_change(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ClarifyBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    let id = change_id!(state, id);
    match crate::draft::clarify(
        &state,
        &id,
        &body.path,
        body.line,
        &body.text,
        &body.answer,
        body.confirm,
    )
    .await
    {
        Ok(a) => Json(json!({
            "diff": a.diff,
            "written": a.written,
            "says": match a.written {
                true => "written — your words replaced the marker, and the answer is recorded as yours",
                false => "this is the whole edit; nothing is written until you confirm",
            },
        }))
        .into_response(),
        Err((status, r)) => draft_refused(
            StatusCode::from_u16(status).unwrap_or(StatusCode::CONFLICT),
            &r,
        ),
    }
}

/// Accepting the offered `[spec] open_questions`: written exactly as offered.
#[derive(Deserialize)]
struct OpenQuestionsBody {
    #[serde(default)]
    accept: bool,
    /// The fingerprint of `devplane.toml` the offer was computed from.
    #[serde(default)]
    base: String,
}

async fn accept_open_questions(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<OpenQuestionsBody>,
) -> impl IntoResponse {
    guard!(state, headers);
    if !body.accept {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "nothing is written unless `accept` is true"})),
        )
            .into_response();
    }
    let root = {
        let w = state.world.lock().await;
        w.projects()
            .find(|p| p.id.as_str() == id)
            .map(|p| p.root.clone())
    };
    let Some(root) = root else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no such project"})),
        )
            .into_response();
    };
    match crate::draft::accept_open_questions(&state, &root, &body.base).await {
        Ok(diff) => Json(json!({
            "written": true,
            "diff": diff,
            "says": "written — devplane.toml now declares Spec Kit's marker; recorded as yours",
        }))
        .into_response(),
        Err((status, r)) => draft_refused(
            StatusCode::from_u16(status).unwrap_or(StatusCode::CONFLICT),
            &r,
        ),
    }
}

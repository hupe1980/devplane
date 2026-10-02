//! The first run, from the window (its routes are listed in [`crate::api`]): the checklist's facts, the gate
//! candidates a repository already has, declaring a `check` into
//! `devplane.toml`, scanning a repository before it is trusted, and connecting
//! an agent.
//!
//! Every write here is **preview, then confirm**: a body with `confirm: false`
//! answers the exact diff and touches nothing; `confirm: true` writes only if
//! the file still produces the diff the person saw, else `409 changed_since`
//! with the new one. Nothing offered from a repository is ever run.

use crate::core::candidates::{self, RepoFiles};
use crate::core::declare::{self, Declared, Refused};
use crate::core::firstrun::{self, AgentFact, ChangeFact, Facts, ProjectFact, Updates};
use crate::host::Shared;
use crate::observe::connect::{self, Agent, Planned};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path as FsPath, PathBuf};

fn refuse(code: StatusCode, body: Value) -> Response {
    (code, Json(body)).into_response()
}

fn unauthorised() -> Response {
    refuse(StatusCode::UNAUTHORIZED, json!({"error": "unauthorised"}))
}

/// A registered project by id, name or root.
async fn project(state: &Shared, id: &str) -> Option<crate::core::Project> {
    let w = state.world.lock().await;
    w.projects()
        .find(|p| p.id.as_str() == id || p.name == id || p.root.display().to_string() == id)
        .cloned()
}

// ── the checklist ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct FirstRunQuery {
    project: Option<String>,
}

pub(crate) async fn first_run(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<FirstRunQuery>,
) -> Response {
    if !crate::api::authorised(&state, &headers) {
        return unauthorised();
    }
    let facts = facts(&state, q.project.as_deref()).await;
    let project = facts.project.as_ref().map(|p| p.id.clone());
    let trusted = {
        let w = state.world.lock().await;
        w.projects()
            .filter(|p| p.trusted)
            .map(|p| json!({"id": p.id.as_str(), "name": p.name, "root": p.root.display().to_string()}))
            .collect::<Vec<_>>()
    };
    let latest = facts
        .project
        .as_ref()
        .and_then(|p| p.latest.as_ref())
        .map(|c| json!({"id": c.id, "title": c.title, "state": c.state, "created_at": c.created_at, "check_failed": c.check_failed}));
    Json(json!({
        "project": project,
        "trusted": trusted,
        "latest": latest,
        "agents": connect::connections(),
        "steps": firstrun::steps(&facts),
    }))
    .into_response()
}

/// What is true on this machine, for the steps.
async fn facts(state: &Shared, chosen: Option<&str>) -> Facts {
    let snap = state.snapshot().await;
    let names = snap.names();
    let sessions: Vec<(String, Option<String>)> = snap
        .world
        .runs()
        .filter(|r| r.is_active())
        .map(|r| {
            (
                r.agent.clone(),
                r.project_id.as_ref().and_then(|p| names.get(p).cloned()),
            )
        })
        .collect();
    let watching = crate::core::vendors::watching();
    let mut cannot_see: Vec<String> = Vec::new();
    if let Some(l) = snap.limits() {
        cannot_see.push(l.to_string());
    }
    if !watching.driven_only.is_empty() {
        cannot_see.push(format!(
            "{} {} not watched here: sessions {} runs are not visible unless Devplane starts them",
            watching.driven_only.join(", "),
            if watching.driven_only.len() == 1 {
                "is"
            } else {
                "are"
            },
            if watching.driven_only.len() == 1 {
                "it"
            } else {
                "they"
            },
        ));
    }
    let agents = connect::connections()
        .into_iter()
        .map(|a| AgentFact {
            name: a.name.to_string(),
            connected: a.connected,
            gate_off: !a.gate_off.is_empty(),
        })
        .collect();
    let trusted: Vec<&crate::core::Project> = snap.world.projects().filter(|p| p.trusted).collect();
    let github_unsigned = github_unsigned(state, &snap).await;
    let chosen = match chosen {
        Some(c) => trusted
            .iter()
            .find(|p| p.id.as_str() == c || p.name == c)
            .copied(),
        // The only trusted project; with several, the one most recently
        // worked in, else the first.
        None => trusted
            .iter()
            .max_by_key(|p| {
                snap.changes
                    .iter()
                    .filter(|c| c.project_id == p.id)
                    .map(|c| c.updated_at)
                    .max()
            })
            .copied(),
    };
    let project = match chosen {
        None => None,
        Some(p) => {
            let check = crate::core::ProjectConfig::load(&p.root)
                .map(|c| c.gates.check.clone())
                .map_err(|e| e.to_string());
            let dialects = crate::spec::detect(&p.root)
                .into_iter()
                .map(|d| format!("{} ({})", d.name, d.detected_by))
                .collect();
            let (latest, verified_once) = latest_change(state, &snap, &p.id).await;
            Some(ProjectFact {
                id: p.id.as_str().to_string(),
                name: p.name.clone(),
                check,
                dialects,
                latest,
                verified_once,
            })
        }
    };
    Facts {
        sessions,
        cannot_see,
        updates: Updates::NotThisBuild,
        agents,
        trusted: trusted.iter().map(|p| p.name.clone()).collect(),
        github_unsigned,
        project,
    }
}

/// The newest change in `project`, as the change list states it, and the
/// title of one whose `check` has passed, if any has.
async fn latest_change(
    state: &Shared,
    snap: &crate::view::Snapshot,
    project: &crate::core::ProjectId,
) -> (Option<ChangeFact>, Option<String>) {
    let once = snap
        .changes
        .iter()
        .filter(|c| &c.project_id == project)
        .find(|c| c.gates.iter().any(|g| g.gate == "check" && g.passed()))
        .map(|c| c.title.clone());
    (latest_view(state, snap, project).await, once)
}

async fn latest_view(
    state: &Shared,
    snap: &crate::view::Snapshot,
    project: &crate::core::ProjectId,
) -> Option<ChangeFact> {
    let views = crate::view::change_list(snap, &state.store).await;
    let v = views.iter().find(|v| &v.change.project_id == project)?;
    let v = serde_json::to_value(v).ok()?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let gate = v.get("gate").filter(|g| !g.is_null());
    let check_failed = gate
        .filter(|g| g.get("passed").and_then(Value::as_bool) == Some(false))
        .map(|g| {
            g.get("summary")
                .and_then(Value::as_str)
                .unwrap_or("check failed")
                .to_string()
        });
    let qualifier = v
        .pointer("/qualifier/says")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Some(ChangeFact {
        created_at: s("created_at"),
        id: s("id"),
        title: s("title"),
        state: s("state"),
        qualifier,
        check_failed,
    })
}

/// A GitHub host some registered project's remote is on, with nobody signed in.
async fn github_unsigned(state: &Shared, snap: &crate::view::Snapshot) -> Option<String> {
    let hosts: Vec<String> = snap
        .world
        .projects()
        .filter_map(|p| crate::core::project::parse_remote(p.repo_url.as_deref()?))
        .map(|r| r.host.to_ascii_lowercase())
        .collect();
    let views = state.github.views();
    hosts.into_iter().find(|h| {
        let on_github = h == "github.com" || views.iter().any(|v| &v.host == h);
        let signed_in = views
            .iter()
            .any(|v| &v.host == h && matches!(v.sign_in, crate::github::SignIn::SignedIn { .. }));
        on_github && !signed_in
    })
}

// ── declaring a check ──────────────────────────────────────────────────────

/// Reads only the files candidate detection looks at.
pub fn repo_files(root: &FsPath) -> RepoFiles {
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).ok();
    let justfile = ["justfile", "Justfile", ".justfile"]
        .iter()
        .find_map(|n| read(n).map(|t| (n.to_string(), t)));
    let mut workflows: Vec<(String, String)> = std::fs::read_dir(root.join(".github/workflows"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name.ends_with(".yml") || name.ends_with(".yaml"))
                .then(|| {
                    Some((
                        format!(".github/workflows/{name}"),
                        std::fs::read_to_string(e.path()).ok()?,
                    ))
                })
                .flatten()
        })
        .collect();
    workflows.sort();
    RepoFiles {
        cargo_toml: root.join("Cargo.toml").is_file(),
        package_json: read("package.json"),
        lockfiles: candidates::LOCKFILES
            .iter()
            .filter(|(f, _)| root.join(f).is_file())
            .map(|(f, _)| f.to_string())
            .collect(),
        justfile,
        workflows,
        openspec: root.join("openspec").is_dir(),
    }
}

/// Whether `devplane.toml` has uncommitted changes (or is untracked) in the checkout.
async fn uncommitted(root: &FsPath) -> bool {
    tokio::process::Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--",
            crate::core::config::CONFIG_FILE,
        ])
        .current_dir(root)
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| !o.stdout.is_empty())
}

/// A refusal, small enough to carry in an `Err`.
type Refusal = (StatusCode, Json<Value>);

async fn trusted_project(state: &Shared, id: &str) -> Result<crate::core::Project, Refusal> {
    match project(state, id).await {
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no registered project `{id}`")})),
        )),
        Some(p) if !p.trusted => Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": format!(
                "{} is not trusted, so nothing is read from or written to it; trust it first",
                p.name
            )})),
        )),
        Some(p) => Ok(p),
    }
}

pub(crate) async fn gate_candidates(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if !crate::api::authorised(&state, &headers) {
        return unauthorised();
    }
    let p = match trusted_project(&state, &id).await {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };
    let file = p.root.join(crate::core::config::CONFIG_FILE);
    let text = std::fs::read_to_string(&file).ok();
    let found = candidates::candidates(&repo_files(&p.root));
    let (current, parse) = match text.as_deref().map(declare::check_parses) {
        None => (json!({"check": [], "named": {}}), json!({"parses": true})),
        Some(Ok(cfg)) => (
            json!({
                "check": cfg.gates.check,
                "named": cfg.gates.named.iter().map(|(k, v)| (k.clone(), json!(v.run))).collect::<serde_json::Map<_, _>>(),
            }),
            json!({"parses": true}),
        ),
        Some(Err(Refused::Unparseable { line, error })) => (
            json!({"check": [], "named": {}}),
            json!({"parses": false, "line": line, "error": error}),
        ),
        Some(Err(other)) => (
            json!({"check": [], "named": {}}),
            json!({"parses": false, "error": other.says()}),
        ),
    };
    let mut config = json!({
        "path": file.display().to_string(),
        "exists": text.is_some(),
        "uncommitted": uncommitted(&p.root).await,
    });
    if let (Some(c), Some(pp)) = (config.as_object_mut(), parse.as_object()) {
        c.extend(pp.clone());
    }
    Json(json!({
        "project": {"id": p.id.as_str(), "name": p.name, "root": p.root.display().to_string()},
        "current": current,
        "candidates": found.candidates,
        "looked_for": found.looked_for,
        "config": config,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub(crate) struct DeclareBody {
    #[serde(flatten)]
    declared: Declared,
    #[serde(default)]
    confirm: bool,
    /// The diff the person saw, sent back with the confirm.
    #[serde(default)]
    diff: Option<String>,
}

pub(crate) async fn declare_gates(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<DeclareBody>,
) -> Response {
    if !crate::api::authorised(&state, &headers) {
        return unauthorised();
    }
    let p = match trusted_project(&state, &id).await {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };
    let file = p.root.join(crate::core::config::CONFIG_FILE);
    let before = match std::fs::read_to_string(&file) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": format!("could not read {}: {e}", file.display())}),
            );
        }
    };
    let after = match declare::splice(before.as_deref(), &body.declared) {
        Ok(a) => a,
        Err(r) => {
            let mut out = serde_json::to_value(&r).unwrap_or_default();
            out["error"] = json!(r.says());
            return refuse(StatusCode::UNPROCESSABLE_ENTITY, out);
        }
    };
    let diff = connect::diff(before.as_deref().unwrap_or(""), &after);
    let dirty = uncommitted(&p.root).await;
    if !body.confirm {
        return Json(json!({
            "path": file.display().to_string(),
            "diff": diff,
            "writes": !diff.is_empty(),
            "uncommitted": dirty,
            "new_file": before.is_none(),
        }))
        .into_response();
    }
    if body.diff.as_deref() != Some(diff.as_str()) {
        return refuse(
            StatusCode::CONFLICT,
            json!({
                "refused": "changed_since",
                "error": "devplane.toml changed since the diff was shown; this is the change now",
                "path": file.display().to_string(),
                "diff": diff,
                "writes": !diff.is_empty(),
                "uncommitted": dirty,
            }),
        );
    }
    if !diff.is_empty()
        && let Err(e) = connect::write_whole(&file, &after)
    {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}),
        );
    }
    let declared = body.declared.cleaned();
    state
        .record(crate::core::Decision::new(
            crate::core::Authority::Person,
            "project:declare-gates",
            p.root.display().to_string(),
            "declared",
        ))
        .await;
    Json(json!({
        "declared": {"check": declared.check, "named": declared.named},
        "path": file.display().to_string(),
        "written": !diff.is_empty(),
    }))
    .into_response()
}

// ── trust: what the repository would be allowed to run ─────────────────────

#[derive(Deserialize)]
pub(crate) struct ScanQuery {
    path: String,
}

pub(crate) async fn scan(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<ScanQuery>,
) -> Response {
    if !crate::api::authorised(&state, &headers) {
        return unauthorised();
    }
    let typed = expand_home(q.path.trim());
    let Ok(root) = typed.canonicalize() else {
        return refuse(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{} does not exist", typed.display())}),
        );
    };
    if !root.is_dir() {
        return refuse(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{} is not a directory", root.display())}),
        );
    }
    let is_repo = root.join(".git").exists();
    let found = crate::setup::scan(&root);
    let registered = project(&state, &root.display().to_string()).await;
    Json(json!({
        "root": root.display().to_string(),
        "name": root.file_name().map(|n| n.to_string_lossy().to_string()),
        "repository": is_repo,
        "trusted": registered.as_ref().is_some_and(|p| p.trusted),
        "findings": found.findings.iter().map(|f| json!({
            "source": f.source,
            "kind": f.kind.as_str(),
            "subject": f.subject,
            "detail": f.detail,
        })).collect::<Vec<_>>(),
        "skills": found.skills,
        "unreadable": found.unreadable,
    }))
    .into_response()
}

fn expand_home(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(p),
    }
}

// ── connecting an agent ────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub(crate) struct ConnectBody {
    #[serde(default)]
    confirm: bool,
    #[serde(default)]
    diff: Option<String>,
}

pub(crate) async fn connect_agent(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(agent): Path<String>,
    body: Option<Json<ConnectBody>>,
) -> Response {
    if !crate::api::authorised(&state, &headers) {
        return unauthorised();
    }
    let Some(agent) = Agent::parse(&agent) else {
        return refuse(
            StatusCode::NOT_FOUND,
            json!({"error": format!("no agent `{agent}`: claude, codex or copilot")}),
        );
    };
    // Every hook runs this path; one that will vanish is refused, writing nothing.
    let exe = match connect::stable_binary(None) {
        Ok(e) => e,
        Err(e) => {
            return refuse(
                StatusCode::CONFLICT,
                json!({"refused": "unstable_path", "error": format!("{e:#}"), "says": format!("{e:#}")}),
            );
        }
    };
    let port = state
        .port
        .get()
        .copied()
        .unwrap_or(crate::config::DEFAULT_PORT);
    let base_url = format!("http://127.0.0.1:{port}");
    let planned = connect::plan_connect(agent, &exe, &base_url, &state.ingest_token);
    finish(planned, body.map(|b| b.0).unwrap_or_default(), agent)
}

pub(crate) async fn disconnect_agent(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(agent): Path<String>,
    body: Option<Json<ConnectBody>>,
) -> Response {
    if !crate::api::authorised(&state, &headers) {
        return unauthorised();
    }
    let Some(agent) = Agent::parse(&agent) else {
        return refuse(
            StatusCode::NOT_FOUND,
            json!({"error": format!("no agent `{agent}`: claude, codex or copilot")}),
        );
    };
    finish(
        connect::plan_disconnect(agent),
        body.map(|b| b.0).unwrap_or_default(),
        agent,
    )
}

/// Answers a preview, or writes on a confirm of the same diff.
fn finish(planned: anyhow::Result<Planned>, body: ConnectBody, agent: Agent) -> Response {
    let planned = match planned {
        Ok(p) => p,
        Err(e) => {
            return refuse(
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({"error": format!("{e:#}")}),
            );
        }
    };
    let shown = json!({
        "agent": agent.id(),
        "name": agent.name(),
        "path": planned.path.display().to_string(),
        "diff": planned.diff,
        "writes": !planned.diff.is_empty(),
        "removes": planned.after.is_none(),
    });
    if !body.confirm {
        return Json(shown).into_response();
    }
    if body.diff.as_deref() != Some(planned.diff.as_str()) {
        let mut out = shown;
        out["refused"] = json!("changed_since");
        out["error"] = json!(format!(
            "{} changed since the diff was shown; this is the change now",
            planned.path.display()
        ));
        return refuse(StatusCode::CONFLICT, out);
    }
    match connect::apply(&planned) {
        Ok(true) => Json(json!({"written": !planned.diff.is_empty(), "path": planned.path.display().to_string()})).into_response(),
        Ok(false) => {
            let mut out = shown;
            out["refused"] = json!("changed_since");
            out["error"] = json!("the file changed while it was being written; nothing was written");
            refuse(StatusCode::CONFLICT, out)
        }
        Err(e) => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}),
        ),
    }
}

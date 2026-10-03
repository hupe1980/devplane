//! Drafting a specification with the project's own methodology, over ACP.
//!
//! A draft is a change started with intent. The prompt is the methodology's
//! own command — Spec Kit's `/speckit-specify`, spelled the way the chosen
//! agent invokes it, or OpenSpec's installed propose workflow — followed by
//! the person's words, and nothing Devplane composed. The agent runs as any
//! driven session: in the change's own worktree, every permission reaching the
//! person or a rule they wrote. Devplane writes nothing into the folder the
//! methodology produces; the person reviews it as a diff and accepts or
//! rejects it. The pure half is [`crate::core::draft`].

use crate::core::draft::{Draft, DraftOutcome, Methodology};
use crate::core::ids::{ChangeId, ProjectId, RunId};
use crate::core::{Change, ProjectConfig};
use crate::host::Shared;
use std::path::{Path, PathBuf};

/// One host-wide lock over choosing a Spec Kit folder and reserving it on
/// the change, so two drafts started at once never share a number.
static ASSIGNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What a person asks for when drafting.
#[derive(Debug, Clone)]
pub struct DraftRequest {
    /// The project's root.
    pub project: PathBuf,
    /// The change's name: its branch and its folder's name.
    pub name: String,
    pub agent: Option<String>,
    pub methodology: Option<Methodology>,
    /// The person's words, sent verbatim.
    pub intent: String,
    /// Draft again into this change, whose last draft was rejected.
    pub change: Option<ChangeId>,
}

/// Why nothing was started, by cause, with the sentence that says it.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    pub cause: &'static str,
    pub says: String,
}

impl Refused {
    fn new(cause: &'static str, says: impl Into<String>) -> Self {
        Self {
            cause,
            says: says.into(),
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.says)
    }
}

/// What a draft would send, before anything exists.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Preview {
    pub methodology: Methodology,
    pub agent: String,
    pub sent: String,
    /// The methodologies this project offers drafting for.
    pub offered: Vec<Methodology>,
    /// Where the methodology will write, as far as is known before the
    /// worktree exists.
    pub folder_says: String,
}

/// What a started draft is.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Started {
    pub change: ChangeId,
    pub run: RunId,
    pub sent: String,
    pub folder: String,
    pub says: Vec<String>,
}

/// The names of the layouts detected at `root`.
fn detected(root: &Path) -> Vec<String> {
    crate::spec::detect(root)
        .into_iter()
        .map(|d| d.name)
        .collect()
}

/// The methodologies drafting is offered for in a project.
pub fn offered(root: &Path) -> Vec<Methodology> {
    let found = detected(root);
    [Methodology::SpecKit, Methodology::OpenSpec]
        .into_iter()
        .filter(|m| found.iter().any(|d| d == m.dialect()))
        .collect()
}

/// `.specify/integration.json`, read as Spec Kit writes it.
fn integrations(dir: &Path) -> Result<crate::core::draft::Integrations, Refused> {
    let path = dir.join(".specify/integration.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        Refused::new(
            "integration_record",
            format!(
                "Spec Kit's integration record {} could not be read ({e}); `specify init` writes it",
                path.display()
            ),
        )
    })?;
    serde_json::from_str(&text).map_err(|e| {
        Refused::new(
            "integration_record",
            format!(
                "Spec Kit's integration record {} does not parse: {e}",
                path.display()
            ),
        )
    })
}

/// `.specify/init-options.json` `feature_numbering`, which Spec Kit's own
/// specify command reads; absent is sequential.
fn numbering(dir: &Path) -> crate::core::draft::Numbering {
    let value = std::fs::read_to_string(dir.join(".specify/init-options.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["feature_numbering"].as_str().map(str::to_string));
    crate::core::draft::Numbering::of(value.as_deref())
}

/// The invocation for `agent` under `methodology`, read in `dir`.
fn invocation(methodology: Methodology, agent: &str, dir: &Path) -> Result<String, Refused> {
    match methodology {
        Methodology::SpecKit => crate::core::draft::speckit_for(agent, &integrations(dir)?)
            .map_err(|e| Refused::new("not_installed", e)),
        Methodology::OpenSpec => crate::core::draft::openspec_invocation(agent, |f| {
            crate::spec::confine(dir, f)
                .ok()
                .and_then(|p| std::fs::symlink_metadata(p).ok())
                .is_some_and(|m| m.is_file())
        })
        .map_err(|e| Refused::new("not_installed", e)),
    }
}

/// The agent a request names, or the project's default.
fn agent_of(req: &DraftRequest, config: &ProjectConfig) -> String {
    req.agent
        .clone()
        .filter(|a| !a.trim().is_empty())
        .or_else(|| config.agent.default.clone())
        .unwrap_or_else(|| "claude".into())
}

/// Every check a draft makes before anything is created, in the project's
/// checkout: the words, the methodology, the agent's spelling.
pub fn preview(req: &DraftRequest) -> Result<Preview, Refused> {
    let root = req.project.canonicalize().map_err(|_| {
        Refused::new(
            "no_project",
            format!("{} does not exist", req.project.display()),
        )
    })?;
    let config = ProjectConfig::load(&root)
        .map_err(|e| Refused::new("config", format!("devplane.toml cannot be read: {e}")))?;
    let methodology = crate::core::draft::choose(&detected(&root), req.methodology)
        .map_err(|e| Refused::new("methodology", e))?;
    let agent = agent_of(req, &config);
    let invocation = invocation(methodology, &agent, &root)?;
    let sent = crate::core::draft::sent(&invocation, &req.intent)
        .map_err(|e| Refused::new("empty_intent", e))?;
    let folder_says = match methodology {
        Methodology::SpecKit => format!(
            "Spec Kit writes into specs/<next number>-{}, which Devplane numbers and names in the \
             change's worktree",
            crate::core::draft::folder_slug(&req.name)
        ),
        Methodology::OpenSpec => {
            "OpenSpec's propose workflow names its own change under openspec/changes/".into()
        }
    };
    Ok(Preview {
        methodology,
        agent,
        sent,
        offered: offered(&root),
        folder_says,
    })
}

/// Folder names under `dir`, files excluded.
fn folder_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// A command-line word as the protocol crate's parser reads it back.
fn quoted(s: &str) -> String {
    let plain = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./-:@+,=".contains(c));
    match plain {
        true => s.to_string(),
        false => format!("'{}'", s.replace('\'', r"'\''")),
    }
}

/// The agent's launch with Spec Kit's two variables set. The protocol crate
/// reads leading `NAME=value` words of a command as the process's
/// environment, so the agent gets them however it is launched.
fn with_speckit_env(
    spec: &crate::acp::AgentSpec,
    folder: &Path,
) -> Result<crate::acp::AgentSpec, Refused> {
    if spec.command.trim_start().starts_with('{') {
        return Err(Refused::new(
            "agent_command",
            format!(
                "agent `{}` is configured as a JSON launch, so Devplane cannot hand Spec Kit its \
                 feature folder",
                spec.id
            ),
        ));
    }
    let mut out = spec.clone();
    out.command = format!(
        "SPECIFY_FEATURE_DIRECTORY={} SPECIFY_FEATURE_NO_PERSIST=1 {}",
        quoted(&folder.to_string_lossy()),
        spec.command
    );
    Ok(out)
}

/// Starts a draft: a change, its worktree, the methodology's command sent to
/// the chosen agent over ACP. Every refusal comes before anything is made.
pub async fn start(state: &Shared, req: DraftRequest) -> Result<Started, Refused> {
    let root = req.project.canonicalize().map_err(|_| {
        Refused::new(
            "no_project",
            format!("{} does not exist", req.project.display()),
        )
    })?;
    crate::driven::require_trust(state, &root)
        .await
        .map_err(|e| Refused::new("untrusted", e.to_string()))?;
    let checked = preview(&DraftRequest {
        project: root.clone(),
        ..req.clone()
    })?;
    let agent_spec = crate::acp::resolve(&checked.agent, &state.agents).ok_or_else(|| {
        Refused::new(
            "unknown_agent",
            format!(
                "unknown agent `{}`; `devplane agent list` lists them",
                checked.agent
            ),
        )
    })?;
    let config = ProjectConfig::load(&root)
        .map_err(|e| Refused::new("config", format!("devplane.toml cannot be read: {e}")))?;
    let project_id = ProjectId::from_path(&root);

    // The change: a new one, or the one whose last draft was rejected.
    let (mut change, dir, fresh) = match &req.change {
        Some(id) => {
            let c = state
                .changes
                .lock()
                .await
                .get(id)
                .cloned()
                .ok_or_else(|| Refused::new("no_change", "there is no such change"))?;
            if c.spec.is_some() || c.draft.as_ref().is_some_and(Draft::stands) {
                return Err(Refused::new(
                    "has_spec",
                    "this change already answers a specification; reject its draft first — a \
                     change answers one specification",
                ));
            }
            let dir = c.worktree.clone().ok_or_else(|| {
                Refused::new("no_worktree", "this change has no worktree to draft in")
            })?;
            (c, dir, false)
        }
        None => {
            if req.name.trim().is_empty() {
                return Err(Refused::new(
                    "no_name",
                    "name the change: it becomes the branch and the folder",
                ));
            }
            let mut change = Change::new(
                project_id.clone(),
                req.name.trim().to_string(),
                checked.sent.clone(),
            );
            let base = match &config.project.base_branch {
                Some(b) => b.clone(),
                None => crate::git::base_branch(&root).await,
            };
            let dir =
                crate::git::create_worktree(&root, &change.slug(), &change.branch_name(), &base)
                    .await
                    .map_err(|e| {
                        Refused::new("worktree", format!("creating the worktree: {e:#}"))
                    })?;
            change.worktree = Some(dir.clone());
            change.branch = Some(change.branch_name());
            let copied =
                crate::git::copy_included(&root, &dir, &crate::change::included_files(&root).await);
            if !copied.is_empty() {
                tracing::info!(files = ?copied, "copied gitignored files into the worktree");
            }
            (change, dir, true)
        }
    };
    // The agent sees the worktree, so the workflow must be there too.
    let invocation_here = match invocation(checked.methodology, &checked.agent, &dir) {
        Ok(i) => i,
        Err(e) => {
            if fresh {
                discard_worktree(&root, &dir).await;
            }
            return Err(e);
        }
    };
    let sent = crate::core::draft::sent(&invocation_here, &req.intent)
        .map_err(|e| Refused::new("empty_intent", e))?;

    if fresh && let Some(setup) = &config.workspace.setup {
        let report = crate::gates::run_in_turn(
            config.gates.serial,
            "setup",
            std::slice::from_ref(setup),
            &dir,
            config.gates.timeout,
            1,
            &crate::gates::GateInput::env(crate::core::caches::env_for(
                &root,
                &config.workspace.share,
            )),
        )
        .await;
        if !report.passed() {
            tracing::warn!(summary = %report.summary(), "workspace setup failed");
        }
        change.setup = Some(report);
    }
    let base_commit = crate::git::commit_of(&dir, "HEAD").await.ok();
    change.tree_now = crate::git::commit_stamp(&dir).await;
    change.tree_as_of = Some(jiff::Timestamp::now());
    change.prompt = sent.clone();

    // The folder, chosen and reserved under one lock.
    let assigning = ASSIGNING.lock().await;
    let (folder, launch, found) = match checked.methodology {
        Methodology::SpecKit => {
            let mut taken = folder_names(&root.join("specs"));
            taken.extend(folder_names(&dir.join("specs")));
            for c in state.changes.lock().await.values() {
                if c.project_id != project_id || c.archived_at.is_some() {
                    continue;
                }
                if let Some(w) = &c.worktree {
                    taken.extend(folder_names(&w.join("specs")));
                }
                if let Some(d) = c.draft.as_ref().filter(|d| d.stands())
                    && let Some(name) = d.folder.rsplit('/').next()
                {
                    taken.push(name.to_string());
                }
            }
            let name = crate::core::draft::next_folder(
                numbering(&dir),
                &crate::core::draft::folder_slug(&change.title),
                &taken,
                jiff::Timestamp::now(),
            );
            let folder = format!("specs/{name}");
            let launch = match with_speckit_env(&agent_spec, &dir.join(&folder)) {
                Ok(l) => l,
                Err(e) => {
                    drop(assigning);
                    if fresh {
                        discard_worktree(&root, &dir).await;
                    }
                    return Err(e);
                }
            };
            (folder, launch, Vec::new())
        }
        Methodology::OpenSpec => (
            "openspec/changes".to_string(),
            agent_spec.clone(),
            folder_names(&dir.join("openspec/changes")),
        ),
    };
    let speckit_version = match checked.methodology {
        Methodology::SpecKit => integrations(&dir).ok().and_then(|r| r.version),
        Methodology::OpenSpec => None,
    };
    change.draft = Some(Draft {
        methodology: checked.methodology,
        agent: checked.agent.clone(),
        intent: req.intent.clone(),
        sent: sent.clone(),
        folder: folder.clone(),
        base_commit,
        speckit_version,
        found,
        run: None,
        outcome: DraftOutcome::Pending,
    });
    change.waiting = None;
    change.stopped = None;
    change.updated_at = jiff::Timestamp::now();
    let change_id = change.id.clone();
    crate::change::persist(state, &change).await;
    state
        .changes
        .lock()
        .await
        .insert(change_id.clone(), change.clone());
    drop(assigning);
    state.notify_changed();

    // Started silent, registered, and only then prompted — as every change
    // run is, so the turn's end finds its change.
    let run = match crate::driven::dispatch_for_change(
        state,
        &launch,
        dir.clone(),
        &change_id,
        Vec::new(),
    )
    .await
    {
        Ok(run) => run,
        Err(e) => {
            // Nothing was sent: the draft is not pending any more.
            let mut changes = state.changes.lock().await;
            if let Some(c) = changes.get_mut(&change_id) {
                c.draft = None;
                c.stopped = Some(crate::core::change::Stopped::Broken {
                    detail: format!("the agent did not start: {e:#}"),
                });
                let c = c.clone();
                drop(changes);
                crate::change::persist(state, &c).await;
            }
            return Err(Refused::new(
                "agent",
                format!("the agent did not start: {e:#}"),
            ));
        }
    };
    let saved = {
        let mut changes = state.changes.lock().await;
        let c = changes.get_mut(&change_id).expect("inserted above");
        c.runs.push(run.clone());
        if let Some(d) = c.draft.as_mut() {
            d.run = Some(run.clone());
        }
        c.updated_at = jiff::Timestamp::now();
        c.clone()
    };
    crate::change::persist(state, &saved).await;
    state
        .change_of_run
        .lock()
        .await
        .insert(run.clone(), change_id.clone());
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:draft",
                checked.methodology.as_str(),
                "sent",
            )
            .because(format!(
                "sent {}'s own command over ACP to {}",
                checked.methodology.dialect(),
                checked.agent
            ))
            .for_change(&change_id),
        )
        .await;
    crate::driven::prompt(state, &run, sent.clone())
        .await
        .map_err(|e| Refused::new("agent", format!("the command could not be sent: {e:#}")))?;
    let says = saved.draft.as_ref().map(Draft::says).unwrap_or_default();
    Ok(Started {
        change: change_id,
        run,
        sent,
        folder,
        says,
    })
}

/// Removes a worktree made for a draft that was refused before it started.
async fn discard_worktree(root: &Path, dir: &Path) {
    if let Err(e) = crate::git::remove_worktree(root, dir, true).await {
        tracing::warn!(error = %e, "could not remove the worktree of a refused draft");
    }
}

/// Called at the top of every turn end: while a draft is unaccepted, its
/// turn ends run no gate, no base checks and no drift, and hand nothing back.
/// Says whether it handled the turn.
pub async fn on_turn_ended(state: &Shared, id: &ChangeId) -> bool {
    let (draft, dir) = {
        let changes = state.changes.lock().await;
        match changes.get(id) {
            Some(c) => (c.draft.clone(), c.worktree.clone()),
            None => return false,
        }
    };
    let Some(mut draft) = draft.filter(Draft::asks_for_folder) else {
        return false;
    };
    let Some(dir) = dir else { return false };
    match draft.methodology {
        Methodology::SpecKit => {
            draft.outcome = match dir.join(&draft.folder).is_dir() {
                true => DraftOutcome::Produced,
                false => DraftOutcome::NoneProduced,
            };
        }
        Methodology::OpenSpec => {
            let now = folder_names(&dir.join("openspec/changes"));
            let known = draft
                .folder
                .strip_prefix("openspec/changes/")
                .map(str::to_string);
            let made = known
                .filter(|k| now.contains(k))
                .or_else(|| crate::core::draft::made_change(&draft.found, &now));
            match made {
                Some(name) => {
                    draft.folder = format!("openspec/changes/{name}");
                    draft.outcome = DraftOutcome::Produced;
                }
                None => {
                    draft.folder = "openspec/changes".into();
                    draft.outcome = DraftOutcome::NoneProduced;
                }
            }
        }
    }
    {
        let mut changes = state.changes.lock().await;
        if let Some(c) = changes.get_mut(id) {
            c.draft = Some(draft);
        }
    }
    crate::change::refresh_tree(state, id).await;
    crate::change::set_waiting(state, id, Some(crate::core::Waiting::Person)).await;
    true
}

/// Whether the change's draft run is mid-turn, when accepting or rejecting
/// would race the agent's writes.
async fn mid_turn(state: &Shared, draft: &Draft) -> bool {
    let Some(run) = &draft.run else { return false };
    let w = state.world.lock().await;
    w.run(run).is_some_and(|r| {
        crate::core::run::Delivery::of(&r.state) == crate::core::run::Delivery::Queued
    })
}

/// The change and its draft, when there is one a person can decide on.
async fn decidable(state: &Shared, id: &ChangeId) -> Result<(Change, Draft), Refused> {
    let c = state
        .changes
        .lock()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| Refused::new("no_change", "there is no such change"))?;
    let d = c
        .draft
        .clone()
        .filter(Draft::asks_for_folder)
        .ok_or_else(|| {
            Refused::new(
                "no_draft",
                "this change has no draft waiting for a decision",
            )
        })?;
    if d.outcome == DraftOutcome::Pending || mid_turn(state, &d).await {
        return Err(Refused::new(
            "live",
            "the agent is still drafting; wait for its turn to end, or stop it",
        ));
    }
    Ok((c, d))
}

/// Accepts a draft: its folder becomes the specification the change
/// answers. Devplane commits nothing.
pub async fn accept(state: &Shared, id: &ChangeId) -> Result<String, Refused> {
    let (c, d) = decidable(state, id).await?;
    if d.outcome != DraftOutcome::Produced {
        return Err(Refused::new(
            "none_produced",
            format!(
                "the command produced no specification at {}; there is nothing to accept",
                d.folder
            ),
        ));
    }
    let dir = c.worktree.clone().unwrap_or_default();
    let markers = crate::change::governing_config(state, id)
        .await
        .map(|(_, cfg)| cfg.spec.open_questions)
        .unwrap_or_default();
    let fingerprint = crate::spec::Spec::read(&dir, &d.folder, &markers).fingerprint();
    let saved = {
        let mut changes = state.changes.lock().await;
        let Some(c) = changes.get_mut(id) else {
            return Err(Refused::new("no_change", "there is no such change"));
        };
        c.spec = Some(d.folder.clone());
        c.spec_at_start = fingerprint;
        if let Some(draft) = c.draft.as_mut() {
            draft.outcome = DraftOutcome::Accepted;
        }
        c.updated_at = jiff::Timestamp::now();
        c.clone()
    };
    crate::change::persist(state, &saved).await;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:draft.accept",
                d.folder.clone(),
                "accepted",
            )
            .because("the drafted folder is the specification this change answers")
            .for_change(id),
        )
        .await;
    state.notify_changed();
    Ok(d.folder)
}

/// Rejects a draft: the worktree goes back to the commit it had at dispatch,
/// untracked files are removed, and the folder the methodology wrote is
/// removed by name even where git ignores it. The change, its run and every
/// decision stay on the record.
pub async fn reject(state: &Shared, id: &ChangeId) -> Result<Vec<String>, Refused> {
    let (c, d) = decidable(state, id).await?;
    let dir = c
        .worktree
        .clone()
        .ok_or_else(|| Refused::new("no_worktree", "this change has no worktree"))?;
    let commit = d.base_commit.clone().ok_or_else(|| {
        Refused::new(
            "no_base",
            "the commit at dispatch was not recorded, so nothing can be put back",
        )
    })?;
    let named: Vec<String> = match (d.methodology, d.outcome) {
        (Methodology::OpenSpec, DraftOutcome::NoneProduced) => Vec::new(),
        _ => vec![d.folder.clone()],
    };
    let removed = crate::git::restore_to(&dir, &commit, &named)
        .await
        .map_err(|e| {
            Refused::new(
                "restore",
                format!("the worktree could not be put back: {e:#}"),
            )
        })?;
    let saved = {
        let mut changes = state.changes.lock().await;
        let Some(c) = changes.get_mut(id) else {
            return Err(Refused::new("no_change", "there is no such change"));
        };
        if let Some(draft) = c.draft.as_mut() {
            draft.outcome = DraftOutcome::Rejected;
        }
        c.updated_at = jiff::Timestamp::now();
        c.clone()
    };
    crate::change::persist(state, &saved).await;
    crate::change::refresh_tree(state, id).await;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "change:draft.reject",
                d.folder.clone(),
                "rejected",
            )
            .because("the worktree was put back to the commit it had at dispatch")
            .for_change(id),
        )
        .await;
    state.notify_changed();
    Ok(removed)
}

/// The folder whose unresolved lines a change carries: its specification,
/// or the folder its unaccepted draft produced.
fn spec_folder(change: &Change) -> Option<String> {
    change.spec.clone().or_else(|| {
        change
            .draft
            .as_ref()
            .filter(|d| d.outcome == DraftOutcome::Produced)
            .map(|d| d.folder.clone())
    })
}

/// The checkout a change works in: its worktree, else the project.
fn checkout(change: &Change, root: &Path) -> PathBuf {
    change
        .worktree
        .clone()
        .filter(|w| w.exists())
        .unwrap_or_else(|| root.to_path_buf())
}

/// Every unresolved-question line in the change's specification, as
/// `(path, line, text)`, read from the checkout the change works in.
pub fn question_lines(
    change: &Change,
    root: &Path,
    markers: &[String],
) -> Vec<(String, u32, String)> {
    let Some(folder) = spec_folder(change) else {
        return Vec::new();
    };
    crate::spec::Spec::read(&checkout(change, root), &folder, markers)
        .question_lines_at()
        .map(|(p, l, t)| (p.to_string(), l, t.to_string()))
        .collect()
}

/// What answering a marker came to.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Answered {
    pub diff: String,
    pub written: bool,
}

/// Answers one bracketed marker: the diff, and the write only on `confirm`.
pub async fn clarify(
    state: &Shared,
    id: &ChangeId,
    path: &str,
    line: u32,
    text: &str,
    answer: &str,
    confirm: bool,
) -> Result<Answered, (u16, Refused)> {
    let c = state
        .changes
        .lock()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| (404, Refused::new("no_change", "there is no such change")))?;
    let (root, config) = crate::change::governing_config(state, id)
        .await
        .map_err(|e| (409, Refused::new("config", e.to_string())))?;
    let markers = config.spec.open_questions;
    let folder = spec_folder(&c).ok_or_else(|| {
        (
            409,
            Refused::new("no_spec", "this change has no specification to answer in"),
        )
    })?;
    let folder = folder.trim_end_matches('/');
    if path != folder && !path.starts_with(&format!("{folder}/")) {
        return Err((
            409,
            Refused::new(
                "outside",
                format!("`{path}` is not in this change's specification, {folder}"),
            ),
        ));
    }
    let dir = checkout(&c, &root);
    let file = crate::spec::confine(&dir, path).map_err(|e| (409, Refused::new("outside", e)))?;
    let before = std::fs::read_to_string(&file).map_err(|e| {
        (
            409,
            Refused::new("unreadable", format!("{path} could not be read: {e}")),
        )
    })?;
    let (after, old, new) = crate::core::draft::answer_in(&before, line, text, &markers, answer)
        .map_err(|e| {
            let status = match e {
                crate::core::draft::Clarify::Unbracketed => 422,
                crate::core::draft::Clarify::Empty => 400,
                crate::core::draft::Clarify::Moved => 409,
            };
            (status, Refused::new("clarify", e.says(id.as_str())))
        })?;
    let diff = crate::core::draft::line_diff(path, line, &old, &new);
    if !confirm {
        return Ok(Answered {
            diff,
            written: false,
        });
    }
    std::fs::write(&file, after).map_err(|e| {
        (
            409,
            Refused::new("unwritable", format!("{path} could not be written: {e}")),
        )
    })?;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "spec:clarify",
                format!("{path}:{line}"),
                "answered",
            )
            .because("the person's answer replaced the bracketed marker, word for word")
            .for_change(id),
        )
        .await;
    crate::change::refresh_tree(state, id).await;
    state.notify_changed();
    Ok(Answered {
        diff,
        written: true,
    })
}

/// The project's `devplane.toml` text and its fingerprint, as an offer is
/// computed from it; empty when there is none.
fn config_text(root: &Path) -> (String, String) {
    let text = std::fs::read_to_string(root.join("devplane.toml")).unwrap_or_default();
    let hash = crate::core::hash::hex(text.as_bytes());
    (text, hash)
}

/// Today, as the fence names it.
fn today() -> String {
    jiff::Zoned::now().strftime("%Y-%m-%d").to_string()
}

/// The offer a change carries after a draft when the project declares no
/// unresolved-question words.
fn offer_of(root: &Path, draft: &Draft) -> Option<serde_json::Value> {
    if !matches!(
        draft.outcome,
        DraftOutcome::Produced | DraftOutcome::Accepted
    ) {
        return None;
    }
    let declared = ProjectConfig::load(root)
        .map(|c| !c.spec.open_questions.is_empty())
        .unwrap_or(true);
    if declared {
        return None;
    }
    Some(match draft.methodology {
        Methodology::OpenSpec => {
            serde_json::json!({ "says": crate::core::draft::OPENSPEC_NO_MARKER })
        }
        Methodology::SpecKit => {
            let (text, base) = config_text(root);
            let (_, diff) = crate::core::draft::open_questions_offer(&text, &today())?;
            serde_json::json!({
                "says": crate::core::draft::SPECKIT_NO_MARKER,
                "diff": diff,
                "base": base,
            })
        }
    })
}

/// Adds the draft's sentences and the marker-setting offer to a change as
/// `GET /api/changes/{id}` serves it.
pub fn decorate(snap: &crate::view::Snapshot, change: &Change, out: &mut serde_json::Value) {
    let Some(draft) = &change.draft else { return };
    out["draft_says"] = serde_json::json!(draft.says());
    let root = snap
        .world
        .project(&change.project_id)
        .map(|p| p.root.clone());
    if let Some(offer) = root.as_deref().and_then(|r| offer_of(r, draft)) {
        out["open_questions_offer"] = offer;
    }
}

/// Writes the offered `[spec] open_questions` into the project's
/// `devplane.toml`: exactly the offered lines, refused when the file changed
/// since the offer was read.
pub async fn accept_open_questions(
    state: &Shared,
    root: &Path,
    base: &str,
) -> Result<String, (u16, Refused)> {
    let (text, now) = config_text(root);
    if now != base {
        return Err((
            409,
            Refused::new(
                "changed",
                "devplane.toml changed since the offer was read; read it again",
            ),
        ));
    }
    let (out, diff) =
        crate::core::draft::open_questions_offer(&text, &today()).ok_or_else(|| {
            (
                409,
                Refused::new(
                    "declared",
                    "devplane.toml already declares [spec] open_questions",
                ),
            )
        })?;
    std::fs::write(root.join("devplane.toml"), out).map_err(|e| {
        (
            409,
            Refused::new(
                "unwritable",
                format!("devplane.toml could not be written: {e}"),
            ),
        )
    })?;
    state
        .record(
            crate::core::Decision::new(
                crate::core::Authority::Person,
                "config:open_questions",
                "devplane.toml".to_string(),
                "written",
            )
            .because("the person accepted Spec Kit's own marker as this project's unresolved-question word"),
        )
        .await;
    state.notify_changed();
    Ok(diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quoted_path_reads_back_as_one_word() {
        assert_eq!(quoted("/tmp/a-b/specs/001-x"), "/tmp/a-b/specs/001-x");
        assert_eq!(quoted("/tmp/a b/it's"), r"'/tmp/a b/it'\''s'");
    }

    #[test]
    fn the_spec_kit_variables_ride_on_the_launch() {
        let spec = crate::acp::AgentSpec::new("claude", "Claude", "npx -y x@1");
        let out = with_speckit_env(&spec, Path::new("/w/specs/001-a")).unwrap();
        assert_eq!(
            out.command,
            "SPECIFY_FEATURE_DIRECTORY=/w/specs/001-a SPECIFY_FEATURE_NO_PERSIST=1 npx -y x@1"
        );
        let json = crate::acp::AgentSpec::new("claude", "Claude", "{\"command\":\"x\"}");
        assert!(with_speckit_env(&json, Path::new("/w")).is_err());
    }
}

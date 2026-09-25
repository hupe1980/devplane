//! What the surfaces show, composed once for every caller.
//!
//! Every read surface is a function over a [`Snapshot`], so the host and a
//! host-less command show the same thing. A host-less snapshot must not render
//! missing live facts as calm; [`Snapshot::from_host`] says which it is.

use crate::core::ask::Ask;
use crate::core::{AttentionItem, Change, ForgeCounts, ProjectId, Run, RunId, World};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// The facts a read surface is composed from.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub world: World,
    pub changes: Vec<Change>,
    pub open_asks: Vec<Ask>,
    /// Runs a host holds a live session for. Empty with no host means
    /// *unknown*, not *none*.
    pub live: BTreeSet<RunId>,
    /// GitHub's answer per project from the last poll, and the items it
    /// raises. `None` when nothing has polled.
    pub forge: Option<Forge>,
    /// What the gate probe last found: `Some(None)` answered, `Some(Some(why))`
    /// down, `None` not probed here.
    pub gate: Option<Option<String>>,
    pub broken_configs: Vec<(PathBuf, String)>,
    pub unwritten: (u64, u64, Option<String>),
    pub leaked_agents: Vec<(u32, String, Option<String>)>,
    pub agents: Vec<crate::acp::AgentSpec>,
    /// Whether a running host assembled this. The surfaces say so when not.
    pub from_host: bool,
    pub now: jiff::Timestamp,
    pub started_at: Option<jiff::Timestamp>,
}

/// The forge's half of a snapshot.
#[derive(Debug, Clone, Default)]
pub struct Forge {
    pub counts: BTreeMap<ProjectId, ForgeCounts>,
    /// What the forge asks of the person, with how many rows a snooze hides.
    pub items: crate::core::attention::Derived,
}

impl Snapshot {
    pub fn names(&self) -> BTreeMap<ProjectId, String> {
        self.world
            .projects()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect()
    }

    /// The sentence a surface prints when the snapshot was not a host's.
    pub fn limits(&self) -> Option<&'static str> {
        (!self.from_host).then_some(
            "read from the store — no host is running, so live sessions, GitHub and the \
             gate probe are not visible from here; `devplane serve` starts one",
        )
    }

    pub fn change(&self, id: &crate::core::ChangeId) -> Option<&Change> {
        self.changes.iter().find(|w| &w.id == id)
    }

    /// The change an id or unambiguous prefix names.
    pub fn resolve_change(&self, needle: &str) -> Result<crate::core::ChangeId, String> {
        if let Some(w) = self.changes.iter().find(|w| w.id.as_str() == needle) {
            return Ok(w.id.clone());
        }
        let hits: Vec<_> = self
            .changes
            .iter()
            .filter(|w| w.id.as_str().starts_with(needle))
            .collect();
        match hits.as_slice() {
            [one] => Ok(one.id.clone()),
            [] => Err(format!("no change `{needle}`")),
            many => Err(format!(
                "`{needle}` matches {} changes: {}",
                many.len(),
                many.iter()
                    .map(|w| w.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

// ── The board ───────────────────────────────────────────────────────────────

/// What the board shows: serialised by the host, and built directly by a
/// terminal with no host.
#[derive(Debug, Serialize, Deserialize)]
pub struct BoardResponse {
    /// The numbers the inbox raises at, so a page cannot colour by another.
    pub thresholds: Value,
    pub summary: crate::core::BoardSummary,
    /// Which projects this board covers; an unreadable forge or configuration
    /// is named, so an empty list is not mistaken for good news.
    pub coverage: Coverage,
    /// Which vendors this board can see at all.
    pub watching: crate::core::vendors::Watching,
    /// What can answer a question on this machine without the person.
    pub question_clock: Option<crate::core::clock::ClockLine>,
    /// The working set by default; everything when asked.
    pub runs: Vec<RunView>,
    /// The changes this machine knows, named only; detail is fetched per id.
    pub changes: Vec<ChangeBrief>,
    pub projects: Vec<crate::core::Project>,
    /// Per project id: open issues, open pull requests, and how many wait on
    /// the person. Absent for a project with no forge.
    #[serde(default)]
    pub forge: BTreeMap<String, ForgeCounts>,
    /// What this board could not see, as a sentence, or nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Coverage {
    pub projects: usize,
    /// The ones it could not read, named.
    pub unreadable: Vec<Unreadable>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Unreadable {
    pub name: String,
    pub why: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeBrief {
    pub id: String,
    pub title: String,
    /// The state's word, from the one table.
    pub state: crate::core::ChangeState,
    pub glyph: String,
    /// What it is waiting on, when it is waiting, and the phrase for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<crate::core::Waiting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_says: Option<String>,
    /// Whether it stopped, so the surface can offer to hand it back.
    #[serde(default)]
    pub stopped: bool,
    /// Working in the person's own checkout, with the sentence naming what
    /// that gives up.
    #[serde(default)]
    pub in_place: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_place_says: Option<String>,
    /// The project it belongs to, by id and by the name the board shows.
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub updated_at: String,
    /// The gates' word (`verified`, `stale`, `failed`, `not run`, `no gates
    /// declared`), or `configuration unreadable` when `devplane.toml` fails.
    pub standing: String,
}

/// What the board shows for one run; a view, so domain fields do not leak
/// into the wire format.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunView {
    pub id: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    pub agent: String,
    pub mode: String,
    /// The permission mode the vendor reported; `None` is *nothing said yet*.
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// Whether a person is in the loop for an ordinary call; `None` when the
    /// mode is unreported or unrecognised.
    #[serde(default)]
    pub asks_a_person: Option<bool>,
    pub state: String,
    #[serde(default)]
    pub waiting_for: Option<String>,
    pub cwd: String,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    pub cost_usd: f64,
    /// Change happened and no telemetry arrived: a gap, not a free session.
    #[serde(default)]
    pub cost_unknown: bool,
    #[serde(default)]
    pub context_percent: Option<f64>,
    /// Usage of the tightest subscription window, from the status-line shim.
    #[serde(default)]
    pub rate_limit_percent: Option<f64>,
    #[serde(default)]
    pub tool_calls: u64,
    #[serde(default)]
    pub subagents: usize,
    #[serde(default)]
    pub reporting: bool,
    #[serde(default)]
    pub idle_seconds: i64,
    #[serde(default)]
    pub last_event_at: String,
    #[serde(default)]
    pub plan_done: usize,
    #[serde(default)]
    pub plan_total: usize,
    #[serde(default)]
    pub plan: Vec<crate::core::PlanStep>,
    /// The tasks sent, from the dispatch record, never parsed. `None` is
    /// *nothing was sent*; `sent_says` says by whom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent: Option<Vec<crate::core::spec::SentTask>>,
    /// What the specification said when this run last closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<crate::core::run::Observed>,
    /// *sent 4 tasks*, *no tasks were sent to this run*, or the sentence for
    /// a session Devplane only watched.
    #[serde(default)]
    pub sent_says: String,
}

impl RunView {
    pub fn of(run: &Run, project_name: Option<String>, now: jiff::Timestamp) -> Self {
        Self {
            id: run.id.to_string(),
            project: run.project_id.as_ref().map(|p| p.to_string()),
            project_name,
            agent: run.agent.clone(),
            mode: run.mode.as_str().into(),
            permission_mode: run.permission_mode.as_ref().map(|m| m.label().to_string()),
            asks_a_person: run.permission_mode.as_ref().and_then(|m| m.asks_a_person()),
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
            cost_unknown: crate::core::reduce::facts::cost_is_unknown(run),
            context_percent: run.totals.context_percent(),
            rate_limit_percent: run.totals.rate_limit_percent,
            tool_calls: run.totals.tool_calls,
            subagents: run.subagents.len(),
            reporting: run.reporting,
            idle_seconds: crate::core::reduce::facts::idle_for(run, now),
            last_event_at: run.last_event_at.to_string(),
            plan_done: run.plan_progress().map(|(d, _)| d).unwrap_or(0),
            plan_total: run.plan.len(),
            plan: run.plan.clone(),
            sent: run.sent.clone(),
            observed: run.observed.clone(),
            sent_says: run.sent_says(),
        }
    }
}

/// One run as `GET /api/runs/{id}` serves it, plus the sentence on its tasks.
pub fn run_detail(run: &Run) -> Value {
    let mut v = serde_json::to_value(run).unwrap_or(Value::Null);
    if let Some(o) = v.as_object_mut() {
        o.insert("sent_says".into(), Value::String(run.sent_says()));
    }
    v
}

pub fn board(snap: &Snapshot, all: bool, broken: &[(PathBuf, String)]) -> BoardResponse {
    let w = &snap.world;
    let forge = snap.forge.as_ref().map(|f| &f.counts);
    let runs = if all { w.board() } else { w.working_set() };
    let runs = runs
        .into_iter()
        .map(|r| {
            let name = r
                .project_id
                .as_ref()
                .and_then(|p| w.project(p))
                .map(|p| p.name.clone());
            RunView::of(r, name, snap.now)
        })
        .collect();
    let mut summary = w.summary();
    if let Some(forge) = forge {
        for c in forge.values() {
            summary.open_issues += c.issues;
            summary.open_prs += c.pull_requests;
            summary.forge_needs_you += c.needs_you;
        }
    }
    // Counted exactly as the inbox counts them (open, no live session).
    summary.asks_waiting = snap
        .open_asks
        .iter()
        .filter(|a| !snap.live.contains(&a.run))
        .count();
    let unreadable: Vec<Unreadable> = w
        .projects()
        .filter_map(|p| {
            let forge_why = forge
                .and_then(|f| f.get(&p.id))
                .and_then(|c| c.stale.clone());
            let config_why = broken
                .iter()
                .find(|(root, _)| *root == p.root)
                .map(|(_, why)| format!("its configuration will not parse: {why}"));
            forge_why.or(config_why).map(|why| Unreadable {
                name: p.name.clone(),
                why,
            })
        })
        .collect();
    let mut changes: Vec<&Change> = snap.changes.iter().collect();
    changes.sort_by_key(|w| std::cmp::Reverse(w.updated_at));
    // Per project: `Some` declares-gates answer, `None` an unloadable file.
    let mut declares: BTreeMap<String, Option<bool>> = BTreeMap::new();
    for c in &changes {
        declares.entry(c.project_id.to_string()).or_insert_with(|| {
            match w.project(&c.project_id) {
                Some(p) => crate::core::ProjectConfig::load(&p.root)
                    .ok()
                    .map(|cfg| cfg.has_gates()),
                None => Some(false),
            }
        });
    }

    BoardResponse {
        // The inbox thresholds, so a gauge colours by the number that decides.
        thresholds: json!({
            "context_high_percent": w.attention.context_high_percent,
            "rate_limit_percent": w.attention.rate_limit_percent,
        }),
        summary,
        coverage: Coverage {
            projects: w.projects().count(),
            unreadable,
        },
        watching: crate::core::vendors::watching(),
        question_clock: crate::observe::connect::question_clock()
            .as_ref()
            .map(crate::core::clock::ClockLine::from),
        runs,
        changes: changes
            .iter()
            .map(|w| {
                let state = w.current_state();
                ChangeBrief {
                    id: w.id.to_string(),
                    title: w.title.clone(),
                    state,
                    glyph: state.glyph().to_string(),
                    waiting: w.waiting.clone(),
                    waiting_says: w.waiting.as_ref().map(crate::core::Waiting::says),
                    stopped: w.is_stopped(),
                    in_place: w.in_place(),
                    in_place_says: w.in_place_says().map(str::to_string),
                    project_id: w.project_id.to_string(),
                    project_name: snap.world.project(&w.project_id).map(|p| p.name.clone()),
                    branch: w.branch.clone(),
                    updated_at: w.updated_at.to_string(),
                    standing: match declares.get(w.project_id.as_str()).copied().flatten() {
                        Some(d) => w.verdict(d, w.tree_now.as_ref()).word().to_string(),
                        None => "configuration unreadable".to_string(),
                    },
                }
            })
            .collect(),
        projects: w.projects().cloned().collect(),
        forge: forge
            .map(|f| {
                f.iter()
                    .map(|(id, c)| (id.to_string(), c.clone()))
                    .collect()
            })
            .unwrap_or_default(),
        limits: snap.limits().map(str::to_string),
    }
}

// ── The inbox ───────────────────────────────────────────────────────────────

/// Everything that needs a person, ranked, before narrowing and folding.
/// `policy` gives quiet windows and rule offers; `store` holds the evidence.
pub async fn current_inbox(
    snap: &Snapshot,
    store: &crate::store::Store,
    policy: &crate::core::PolicyCache,
) -> crate::core::attention::Derived {
    let changes = &snap.changes;
    let drivable: BTreeSet<RunId> = snap.live.iter().cloned().collect();
    let broken = snap.broken_configs.clone();
    let (lost_events, lost_decisions, last_loss) =
        (snap.unwritten.0, snap.unwritten.1, snap.unwritten.2.clone());
    let forge = snap
        .forge
        .as_ref()
        .map(|f| f.items.clone())
        .unwrap_or_default();
    let forge_snoozed = forge.snoozed;
    let forge_items = forge.items;

    // Unanswered questions in committed spec files, one item per project that
    // declares markers. Derived here because `World` may not walk a folder.
    let mut from_disk = {
        let mut out = Vec::new();
        for p in snap.world.projects() {
            let markers = crate::core::ProjectConfig::load(&p.root)
                .map(|c| c.spec.open_questions)
                .unwrap_or_default();
            if markers.is_empty() {
                continue;
            }
            let plans: Vec<(String, crate::core::spec::Plan)> = changes
                .iter()
                .filter(|w| w.project_id == p.id && !w.is_settled())
                .filter_map(|w| {
                    let spec = w.spec.as_deref()?;
                    Some((
                        w.id.as_str().to_string(),
                        crate::core::spec::Plan::read(&p.root, spec, &markers),
                    ))
                })
                .collect();
            out.extend(crate::core::attention::plan_question_item_at(
                &p.name,
                &p.id,
                &plans,
                snap.started_at.unwrap_or(snap.now),
            ));
        }
        out
    };

    // Spec drift under a run, for every open change; its snooze covers it.
    let mut drifts_snoozed = 0;
    {
        let runs: Vec<&Run> = snap.world.runs().collect();
        for w in changes.iter().filter(|w| !w.is_settled()) {
            for d in w.drifts(&runs) {
                if w.snoozed
                    .hides(&crate::core::attention::AttentionKind::SpecDrifted)
                {
                    drifts_snoozed += 1;
                    continue;
                }
                from_disk.push(crate::core::attention::spec_drifted_item_at(
                    w, &d, snap.now,
                ));
            }
        }
    }

    let gate_down = snap.gate.clone().flatten();
    let mut derived = snap.world.inbox_with_health(
        changes,
        &drivable,
        &|dir| policy.stall_seconds(dir),
        &crate::core::world::Health {
            gate_down: gate_down.as_deref(),
            broken_configs: &broken,
            unwritten: (lost_events, lost_decisions, last_loss.as_deref()),
            leaked_agents: &snap.leaked_agents,
            // Known since the host started; with no host, since this read.
            since: snap.started_at.unwrap_or(snap.now),
        },
        forge_items,
        from_disk,
    );
    derived.snoozed += forge_snoozed + drifts_snoozed;
    let items = &mut derived.items;
    // Asks that outlived the process that asked them, only where the run is
    // not live (a live run already produced its own item).
    for ask in &snap.open_asks {
        if snap.live.contains(&ask.run) {
            continue;
        }
        items.push(crate::core::attention::stranded_ask_item(ask));
    }
    // Reports, for whoever decides: the target for an open one, the filer for
    // a GitHub draft. The target's `[questions] deadline` sets the wait.
    items.extend(report_items(snap, store).await);
    let mut derived = derived.rank();
    fill_offers(snap, store, policy, &mut derived.items).await;
    derived
}

/// Every report that needs a person, as inbox rows.
async fn report_items(
    snap: &Snapshot,
    store: &crate::store::Store,
) -> Vec<crate::core::AttentionItem> {
    let reports = match store.reports(REPORTS_READ).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "could not read the reports");
            return Vec::new();
        }
    };
    let windows: BTreeMap<ProjectId, Option<std::time::Duration>> = snap
        .world
        .projects()
        .map(|p| {
            let window = crate::core::ProjectConfig::load(&p.root)
                .ok()
                .and_then(|c| c.questions.deadline())
                .and_then(|d| match d {
                    crate::core::ask::Deadline::Never => None,
                    crate::core::ask::Deadline::After(s) => {
                        Some(std::time::Duration::from_secs(s.into()))
                    }
                });
            (p.id.clone(), window)
        })
        .collect();
    reports
        .iter()
        .filter_map(|r| {
            let window = match &r.target {
                crate::core::report::Target::Project { project, .. } => {
                    windows.get(project).copied().flatten()
                }
                _ => None,
            };
            crate::core::attention::report_item_at(r, window, snap.now)
        })
        .collect()
}

/// The most reports one read considers.
const REPORTS_READ: i64 = 500;

/// The rule to paste, on every permission item that can have one. A query,
/// so bounded: only when a permission item is open, scoped to its repository.
async fn fill_offers(
    snap: &Snapshot,
    store: &crate::store::Store,
    policy: &crate::core::PolicyCache,
    items: &mut [AttentionItem],
) {
    use crate::core::{AttentionKind, Verdict, policy as rules};
    /// How far back the evidence reaches, as `explain --replay` reads.
    const LOOK_BACK: i64 = 500;

    let wanted: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.kind == AttentionKind::Permission)
        .map(|(n, _)| n)
        .collect();
    if wanted.is_empty() {
        return;
    }
    // Some permission items come from a notification naming no tool; those
    // get the reason instead of a blank.
    let mut asked: Vec<(usize, PathBuf, String, Value)> = Vec::new();
    for n in wanted {
        let call = items[n]
            .run_id
            .as_ref()
            .and_then(|id| snap.world.run(id))
            .and_then(|run| {
                let b = run.blocked_on.as_ref()?;
                Some((run.cwd.clone(), b.tool.clone()?, b.input.clone()?))
            });
        match call {
            Some((cwd, tool, input)) => asked.push((n, cwd, tool, input)),
            None => items[n].no_offer = Some(crate::core::offer::NoOffer::UnknownCall.into()),
        }
    }

    for (n, cwd, tool, input) in asked {
        let root = crate::core::project::governing_root(&cwd);
        let scope = root.clone().unwrap_or_else(|| cwd.clone());
        // Family first (a string comparison), verdict second, so the poll
        // does not grow with the event log.
        let family = crate::core::command::rule_family(
            &tool,
            &rules::rule_content(&tool, &input).unwrap_or_default(),
        );
        let others: Vec<crate::core::offer::Interrupting> = store
            .observed_tool_calls(Some(&scope), LOOK_BACK)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.tool.eq_ignore_ascii_case(&tool))
            .filter_map(|c| {
                let content = rules::rule_content(&c.tool, &c.input)?;
                (crate::core::command::rule_family(&c.tool, &content) == family)
                    .then_some((c, content))
            })
            // Distinct: a command run twenty times is one grant.
            .fold(Vec::new(), |mut seen, pair| {
                if seen.len() < crate::core::offer::MAX_KIN
                    && !seen.iter().any(|(_, c): &(_, String)| *c == pair.1)
                {
                    seen.push(pair);
                }
                seen
            })
            .into_iter()
            .filter(|(c, _)| {
                matches!(
                    policy.restrictive(&c.cwd, &c.tool, &c.input),
                    Verdict::Undecided
                )
            })
            .map(|(c, content)| crate::core::offer::Interrupting {
                content,
                tool: c.tool,
            })
            .collect();
        match policy.offer_for(&cwd, &tool, &input, &others, root.as_deref()) {
            Ok(offer) => items[n].offer = Some(offer),
            Err(why) => items[n].no_offer = Some(why.into()),
        }
    }
}

/// One waiting row, with the name of the project it came from.
#[derive(Debug, Serialize, Deserialize)]
pub struct WaitingRow {
    #[serde(flatten)]
    pub item: AttentionItem,
    /// Raised since this person last read the inbox, decided here so CLI and
    /// board agree. `false` with no previous look.
    #[serde(default)]
    pub new_to_you: bool,
    #[serde(default)]
    pub project_name: Option<String>,
}

/// What an inbox read asks for.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct InboxQuery {
    /// Somebody is reading it, not polling; only a read advances the mark.
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub needs_you: bool,
}

/// The inbox as a person sees it: narrowed, inhibited, folded, with the close.
pub async fn inbox(
    snap: &Snapshot,
    store: &crate::store::Store,
    policy: &crate::core::PolicyCache,
    q: &InboxQuery,
) -> Value {
    let names = snap.names();
    // Read before the mark advances, so *new* means new when it was opened.
    let mark = store.last_look().await;

    // Narrow, inhibit, fold: pure, so `listed + narrowed + inhibited + folded
    // == raised` and every surface folds alike.
    let known: Vec<String> = names.values().cloned().collect();
    let narrowing = crate::core::attention::Narrowing {
        project: q.project.as_deref(),
        needs_you: q.needs_you,
    };
    let derived = current_inbox(snap, store, policy).await;
    let raised = derived.items.len();
    let snoozed = derived.snoozed;
    let (in_scope, narrowed) = crate::core::attention::narrow(
        derived.items,
        &narrowing,
        &|id| names.get(id).cloned(),
        &known,
    );
    let (kept, inhibited) = crate::core::attention::inhibit(in_scope);
    let (listed, folded) = crate::core::attention::fold(kept);
    crate::core::attention::accounted(
        raised,
        &[
            listed.len(),
            narrowed.as_ref().map_or(0, |n| n.count),
            inhibited.iter().map(|i| i.count).sum(),
            folded.iter().map(|f| f.count).sum(),
        ],
    );

    let rows: Vec<WaitingRow> = listed
        .into_iter()
        .map(|item| WaitingRow {
            project_name: item
                .project_id
                .as_ref()
                .and_then(|id| names.get(id).cloned()),
            new_to_you: crate::core::close::new_to_you(item.since, mark),
            item,
        })
        .collect();

    // The close is composed here so no surface words a day itself. Empty means
    // nothing raised at all, and it does not render over a narrowed list.
    let nothing_raised = rows.is_empty() && folded.is_empty() && inhibited.is_empty();
    let close = if narrowing.is_none() {
        close(snap, store, nothing_raised).await
    } else {
        Value::Null
    };

    // Recorded after the close, so the boundary reported is the one that held.
    if q.read {
        let _ = store.mark_look(jiff::Timestamp::now()).await;
        let hidden: Vec<crate::core::AttentionId> = folded
            .iter()
            .flat_map(|f| f.ids.iter().cloned())
            .chain(inhibited.iter().flat_map(|i| i.ids.iter().cloned()))
            .collect();
        let _ = store.mark_folded(&hidden).await;
    }
    json!({
        "items": rows,
        "folded": folded,
        "inhibited": inhibited,
        "narrowed": narrowed,
        // A snooze is a choice; counted so the list does not look like calm.
        "snoozed": snoozed,
        "close": close,
        "limits": snap.limits(),
    })
}

/// The boundary, and — for an empty inbox — what the day came to.
pub async fn close(snap: &Snapshot, store: &crate::store::Store, empty: bool) -> Value {
    let now = snap.now;
    let last = store.last_look().await;
    let since = crate::core::close::since_last_look(now, last);
    let looked_at = last.map(|t| t.to_string());
    if !empty {
        return json!({ "since_last_look": since, "looked_at": looked_at });
    }
    // Midnight local: the day the person's machine had.
    let start = jiff::Zoned::now()
        .start_of_day()
        .map(|z| z.timestamp())
        .unwrap_or(now - jiff::SignedDuration::from_hours(24));
    // Unreadable is not empty: a locked store must not print *0 questions
    // waited*.
    let day = match store.day(&start.to_string()).await {
        Ok(d) => d,
        Err(e) => {
            return json!({
                "since_last_look": since,
                "looked_at": looked_at,
                "unreadable": e.to_string(),
            });
        }
    };
    let (decisions, waited) = day;
    let tally = crate::core::close::Tally::of(&decisions).with_waited(waited);
    let expiring = snap
        .open_asks
        .iter()
        .filter(|a| matches!(a.deadline, crate::core::ask::Deadline::After(_)))
        .count();
    let soonest = snap
        .open_asks
        .iter()
        .filter_map(|a| match a.deadline {
            crate::core::ask::Deadline::After(secs) => Some((secs, a)),
            crate::core::ask::Deadline::Never => None,
        })
        .min_by_key(|(secs, _)| *secs)
        .map(|(secs, a)| {
            format!(
                "a question in {} has a deadline of {}",
                a.run,
                crate::core::close::human_secs(u64::from(secs))
            )
        });
    // What continues while the person is away. An ask with a deadline ends
    // without them, so no reassurance over it.
    let keeps_running = match (expiring, snap.from_host) {
        (0, true) => "Devplane keeps watching while it runs, and nothing repeats if it \
                      restarts, so closing this costs nothing."
            .to_string(),
        (0, false) => "Nothing is running. The gate still decides in its own process; \
                       what waits here will still be waiting when a host starts."
            .to_string(),
        (1, _) => "One question has a deadline and will end without you. Everything else \
                   keeps running and repeats nothing."
            .to_string(),
        (n, _) => format!(
            "{n} questions have deadlines and will end without you. Everything else keeps \
             running and repeats nothing."
        ),
    };
    json!({
        "since_last_look": since,
        "looked_at": looked_at,
        "clear": true,
        "quiet": tally.quiet(),
        "sentences": tally.sentences(),
        "next": soonest,
        "keeps_running": keeps_running,
    })
}

// ── Asks ────────────────────────────────────────────────────────────────────

/// One ask, with its outcome sentence composed in `core::ask`.
pub fn render_ask(a: &Ask) -> Value {
    json!({
        "id": a.id.as_str(),
        "kind": a.kind.as_str(),
        "run": a.run.as_str(),
        "project": a.project.as_ref().map(|p| p.as_str()),
        "message": a.message,
        "payload": a.payload,
        "asked_at": a.asked_at.to_string(),
        "open": a.is_open(),
        "outcome": a.outcome(),
        "deadline": a.deadline,
        "deadline_says": a.deadline.says(),
        "answered_at": a.answered_at.map(|t| t.to_string()),
        "answered_from": a.answered_from,
        "delivery": a.delivery,
        "delivered": a.delivery.as_ref().map(|d| d.reached_the_agent()),
        "ended": a.ended,
    })
}

/// Everything an agent has asked, and what became of each one.
pub fn asks(snap: &Snapshot, recent: &[Ask]) -> Value {
    let settled: Vec<_> = recent.iter().filter(|a| !a.is_open()).collect();
    // Questions nobody answered from watched-only sessions: not answerable
    // `asks` rows, but *the agent gave up* is one of the outcomes.
    let mut abandoned: Vec<Value> = snap
        .world
        .runs()
        .flat_map(|r| {
            r.abandoned_questions.iter().map(move |q| {
                json!({
                    "run": r.id.as_str(),
                    "project": r.project_id.as_ref().map(|p| p.as_str()),
                    "question": q.question,
                    "options": q.options,
                    "asked_at": q.asked_at.to_string(),
                    "abandoned_at": q.abandoned_at.to_string(),
                    "moved_on_to": q.moved_on_to,
                    "authority": "nobody",
                    "outcome": "nobody answered: the agent asked and moved on",
                })
            })
        })
        .collect();
    abandoned.sort_by(|a, b| b["abandoned_at"].as_str().cmp(&a["abandoned_at"].as_str()));
    // Which vendors are unseen, so an empty list never reads as none.
    let mut blind: Vec<String> = snap
        .world
        .runs()
        .filter(|r| r.state.is_live())
        .map(|r| r.agent.clone())
        .filter(|a| {
            crate::core::vendors::agent_can_show_an_abandoned_question(a)
                != crate::core::vendors::Reach::Read
        })
        .collect();
    blind.sort();
    blind.dedup();
    json!({
        "open": snap.open_asks.iter().map(render_ask).collect::<Vec<_>>(),
        "settled": settled.iter().map(|a| render_ask(a)).collect::<Vec<_>>(),
        "abandoned": abandoned,
        "blind_to": blind,
    })
}

// ── Modes ───────────────────────────────────────────────────────────────────

/// Which projects are deciding without you. Live sessions only; one with no
/// reported mode is a row, not a gap.
pub fn modes(snap: &Snapshot) -> crate::core::world::Modes {
    let projects = snap.world.permission_modes();
    let sessions: usize = projects.iter().map(|p| p.sessions.len()).sum();
    let clock = crate::observe::connect::question_clock();
    let (read, unread) =
        projects
            .iter()
            .flat_map(|p| p.sessions.iter())
            .fold((0usize, 0usize), |(r, u), m| match m.clock_read {
                true => (r + 1, u),
                false => (r, u + 1),
            });
    crate::core::world::Modes {
        unsupervised: projects.iter().map(|p| p.unsupervised).sum(),
        unknown: projects.iter().map(|p| p.unknown).sum(),
        unreported: projects.iter().map(|p| p.unreported).sum(),
        projects,
        sessions,
        question_clock: clock.as_ref().map(crate::core::clock::ClockLine::from),
        clock_read: read,
        clock_unread: unread,
    }
}

// ── Attention, search, decisions ────────────────────────────────────────────

/// What the inbox asked for and what became of it: measures the tool, not
/// the person.
pub async fn attention(store: &crate::store::Store, days: i64) -> anyhow::Result<Value> {
    let since = jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24 * days.clamp(1, 365));
    let agents = store
        .agents_behind_attention(since)
        .await
        .unwrap_or_default();
    let by_kind = store.attention_stats(since).await?;
    Ok(json!({
        "since": since.to_string(),
        "days": days,
        "agents": agents,
        "kinds": by_kind
            .iter()
            .map(|(kind, st)| {
                let mut v = serde_json::to_value(st).unwrap_or_default();
                if let Some(o) = v.as_object_mut() {
                    o.insert("acted_share".into(), json!(st.acted_share()));
                }
                (kind.clone(), v)
            })
            .collect::<BTreeMap<_, _>>(),
    }))
}

pub async fn search(store: &crate::store::Store, q: &str, limit: i64) -> anyhow::Result<Value> {
    let hits = store.search(q, limit).await?;
    Ok(hits
        .into_iter()
        .map(|(r, t)| json!({"run_id": r.to_string(), "text": t}))
        .collect())
}

/// What Devplane decided, newest first. `without_me` keeps only what was
/// decided instead of the person: a rule, a clock or nobody.
pub async fn decisions(
    store: &crate::store::Store,
    about: Option<&str>,
    limit: i64,
    without_me: bool,
) -> anyhow::Result<Vec<crate::core::Decision>> {
    let d = store.decisions(about, limit).await?;
    Ok(match without_me {
        true => d
            .into_iter()
            .filter(|r| r.authority.was_taken_for_you())
            .collect(),
        false => d,
    })
}

// ── Agents ──────────────────────────────────────────────────────────────────

/// The agents this machine can drive, with what each was measured to support.
/// No record means not probed, not unsupported.
pub async fn agents(snap: &Snapshot, store: &crate::store::Store) -> Vec<Value> {
    let measured = store.agent_capabilities().await.unwrap_or_default();
    let by_command: std::collections::HashMap<&str, &crate::core::AgentCapabilityRecord> =
        measured.iter().map(|c| (c.command.as_str(), c)).collect();
    snap.agents
        .iter()
        .map(|a| {
            let mut v = serde_json::to_value(a).unwrap_or_default();
            if let Some(c) = by_command.get(a.command.as_str())
                && let Some(obj) = v.as_object_mut()
            {
                obj.insert(
                    "measured".into(),
                    json!({
                        "agent_name": c.agent_name,
                        "resume": c.resume,
                        "load_session": c.load_session,
                        "list_sessions": c.list_sessions,
                        "declares_modes": c.declares_modes,
                        "needs_auth": c.needs_auth,
                        "at": c.measured_at.to_string(),
                    }),
                );
            }
            v
        })
        .collect()
}

// ── Reports ─────────────────────────────────────────────────────────────────

/// One report with every sentence already written. `quoted` is the only
/// rendering a person reads: attributed, with every line prefixed.
#[derive(Debug, Serialize)]
pub struct ReportView {
    #[serde(flatten)]
    pub report: crate::core::report::Report,
    pub quoted: String,
    pub routed_says: String,
    pub state_says: String,
    /// *filed 3h ago*.
    pub age_says: String,
    /// *api — claude, run acp-…, change c-…, 14:02*.
    pub provenance_says: String,
    pub target_says: String,
    /// What a person can do about it, in the inbox row's words.
    pub actions: Vec<crate::core::Action>,
}

impl ReportView {
    pub fn of(report: crate::core::report::Report, now: jiff::Timestamp) -> Self {
        use crate::core::Action;
        use crate::core::report::{State, Target};
        let actions = match (&report.state, &report.target) {
            (State::Open, Target::Project { .. }) => vec![
                Action::StartFromReport,
                Action::RejectReport,
                Action::DeferReport,
            ],
            (State::Deferred { .. }, Target::Project { .. }) => vec![Action::StartFromReport],
            (State::Drafted, _) => vec![Action::OpenDraft, Action::DiscardDraft],
            _ => Vec::new(),
        };
        let secs = (now.as_second() - report.provenance.at.as_second()).max(0);
        Self {
            quoted: report.quoted(),
            routed_says: report.routed_says(),
            state_says: report.state_says(),
            age_says: format!("filed {} ago", crate::render::ago(secs)),
            provenance_says: report.provenance.says(),
            target_says: report.target_says(),
            actions,
            report,
        }
    }
}

/// What a report read asks for: to, from, and whether answered ones count.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ReportQuery {
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub all: bool,
}

/// Reports, newest first, narrowed as asked; an unregistered project name
/// matches nothing.
pub async fn reports(
    snap: &Snapshot,
    store: &crate::store::Store,
    q: &ReportQuery,
) -> anyhow::Result<Vec<ReportView>> {
    let id_of = |named: &str| -> ProjectId {
        snap.world
            .projects()
            .find(|p| p.name == named || p.id.as_str() == named)
            .map(|p| p.id.clone())
            .unwrap_or_else(|| ProjectId::new(named))
    };
    // Indexed read where one end is named; the bounded whole otherwise.
    let found = match (q.to.as_deref(), q.from.as_deref()) {
        (Some(t), _) => store.reports_to(&id_of(t), false).await?,
        (None, Some(f)) => store.reports_from(&id_of(f), false).await?,
        (None, None) => store.reports(REPORTS_READ).await?,
    };
    Ok(found
        .into_iter()
        .filter(|r| {
            q.from
                .as_deref()
                .is_none_or(|f| r.provenance.project == id_of(f))
        })
        .filter(|r| q.all || !r.state.is_resolved())
        .map(|r| ReportView::of(r, snap.now))
        .collect())
}

/// One report by id, or the start of one.
pub async fn report(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &str,
) -> anyhow::Result<Option<ReportView>> {
    Ok(store.report(id).await?.map(|r| ReportView::of(r, snap.now)))
}

// ── Change ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ChangeView<'a> {
    #[serde(flatten)]
    pub change: &'a Change,
    /// Where it is, computed from the record and the tree last seen.
    pub state: crate::core::ChangeState,
    pub glyph: &'static str,
    /// What it is waiting on, as a phrase, when it is waiting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_says: Option<String>,
    /// Working in the person's own checkout, with the sentence naming what
    /// that gives up (parallel safety, a clean review base).
    pub in_place: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_place_says: Option<&'static str>,
    /// What the gates say now: verified, stale with both digests, failed, not
    /// run, or *no gates declared*.
    pub standing: crate::core::change::Standing,
    pub standing_says: String,
    pub gate: Option<GateView>,
    /// Whether `change retry` would work now; a surface cannot see whether
    /// the session is still there.
    pub can_retry: bool,
    /// Why the change stopped, composed in the domain.
    pub stopped_summary: Option<String>,
    /// What the agent last said, present only where the last gate failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<String>,
    /// Which absence this is: `transcripts_off` or `nothing_said`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_absent: Option<&'static str>,
    /// The specification this change answers, as it is on disk now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<crate::core::spec::Plan>,
    /// Done, and the plan it answers is not. A sentence, never a verdict.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub plan_contradicts_done: bool,
    /// The plan moved under this change. `None` when unknowable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_drifted: Option<bool>,
    /// Ticked and verified counts. `None` without a readable, recognised
    /// specification; `counts_says` then carries the sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<crate::core::spec::Counts>,
    /// *11 ticked · 9 verified*.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts_says: Option<String>,
    /// The same two counts per requirement token, in heading order.
    pub token_rows: Vec<TokenRowView>,
    /// Runs whose close saw a specification this change did not start with,
    /// each with its sentence and the two decisions a person can take.
    pub drifts: Vec<DriftView>,
    /// This change's runs with what each was sent, from the dispatch record.
    pub run_rows: Vec<RunRowView>,
    /// Size and where the weight sits (*12 files · 640+ 80− lines · 3 shared ·
    /// 1 security*), or why it was not read. Counts only, no time estimate.
    pub shape_says: String,
    /// Reports this change filed, and the one it started from, quoted.
    pub reports: Vec<ReportView>,
}

#[derive(Debug, Serialize)]
pub struct TokenRowView {
    #[serde(flatten)]
    pub row: crate::core::spec::TokenRow,
    pub says: String,
}

#[derive(Debug, Serialize)]
pub struct DriftView {
    #[serde(flatten)]
    pub drift: crate::core::change::Drift,
    pub says: String,
}

#[derive(Debug, Serialize)]
pub struct RunRowView {
    pub id: String,
    pub sent_says: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent: Option<Vec<crate::core::spec::SentTask>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GateView {
    pub name: String,
    /// Whether these commands passed against `ran_at`, not whether they would
    /// now; the claim of *done* checks currency where it is made.
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ran_at: Option<crate::core::change::CommitStamp>,
    pub summary: String,
    pub attempt: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<crate::core::change::SpecStamp>,
    /// Every command the gate ran, with exit and output as handed to the agent.
    pub commands: Vec<crate::core::change::CommandResult>,
}

impl<'a> ChangeView<'a> {
    pub fn of(change: &'a Change, can_retry: bool, declares_gates: bool) -> Self {
        let state = change.current_state();
        let standing =
            crate::core::change::Standing::of(change, declares_gates, change.tree_now.as_ref());
        Self {
            state,
            glyph: state.glyph(),
            waiting_says: change.waiting.as_ref().map(crate::core::Waiting::says),
            in_place: change.in_place(),
            in_place_says: change.in_place_says(),
            standing_says: standing.says(),
            standing,
            gate: change.last_gate().map(|g| GateView {
                name: g.gate.clone(),
                passed: g.passed(),
                ran_at: g.commit.clone(),
                summary: g.summary(),
                attempt: g.attempt,
                spec: g.spec.clone(),
                commands: g.commands.clone(),
            }),
            can_retry,
            claim: None,
            claim_absent: None,
            stopped_summary: change
                .stopped
                .as_ref()
                .map(crate::core::change::Stopped::headline),
            plan: None,
            plan_contradicts_done: false,
            plan_drifted: None,
            counts: None,
            counts_says: None,
            token_rows: Vec::new(),
            drifts: Vec::new(),
            run_rows: Vec::new(),
            shape_says: SHAPE_NOT_COMPUTED.to_string(),
            reports: Vec::new(),
            change,
        }
    }

    fn with_plan(mut self, plan: crate::core::spec::Plan) -> Self {
        self.plan_contradicts_done = self.change.completion.is_some() && plan.contradicts_done();
        self.plan_drifted = self.change.plan_drifted(plan.fingerprint.as_deref());
        self.plan = Some(plan);
        self
    }

    /// The edges: counts over the folder's trace and this change's runs,
    /// per-token rows, drifts, and what each run was sent. `trace` is `None`
    /// when unreadable; an unrecognised notation carries a sentence, not zeros.
    fn with_edges(
        mut self,
        trace: Option<crate::core::spec::Trace>,
        runs: &[&Run],
        declares_gates: bool,
    ) -> Self {
        let mine: Vec<&Run> = self
            .change
            .runs
            .iter()
            .filter_map(|id| runs.iter().copied().find(|r| r.id == *id))
            .collect();
        let stale = matches!(self.standing, crate::core::change::Standing::Stale { .. });
        let last_pass = self.change.last_pass_at();
        match trace {
            None => {
                self.counts_says = Some(
                    "the specification folder could not be read, so nothing is counted".into(),
                );
            }
            Some(t) if t.unrecognised => {
                self.counts_says = Some(crate::core::spec::UNRECOGNISED.into());
            }
            Some(t) => {
                let counts = crate::core::spec::counts(&t, &mine, declares_gates, last_pass);
                self.counts_says = Some(counts.says(stale));
                self.counts = Some(counts);
                self.token_rows =
                    crate::core::spec::token_rows(&t, &mine, declares_gates, last_pass)
                        .into_iter()
                        .map(|row| TokenRowView {
                            says: row.says(),
                            row,
                        })
                        .collect();
            }
        }
        self.drifts = self
            .change
            .drifts(&mine)
            .into_iter()
            .map(|drift| DriftView {
                says: drift.says(),
                drift,
            })
            .collect();
        self.run_rows = mine
            .iter()
            .map(|r| RunRowView {
                id: r.id.to_string(),
                sent_says: r.sent_says(),
                sent: r.sent.clone(),
            })
            .collect();
        self
    }
}

/// What a change's shape says when it was not read for this list.
const SHAPE_NOT_COMPUTED: &str = "shape not computed — `devplane change review` reads it";

/// How many open changes a list reads the shape of (most recently updated),
/// so a long list does not run a `git diff` per change.
const SHAPED: usize = 20;

/// A change's shape sentence, read from its checkout now.
async fn shape_says(snap: &Snapshot, w: &Change) -> String {
    let Some(dir) = w.worktree.as_deref().filter(|d| d.is_dir()) else {
        return "no checkout on disk, so no shape".to_string();
    };
    let root = snap.world.project(&w.project_id).map(|p| p.root.clone());
    let config = root
        .as_deref()
        .and_then(|r| crate::core::ProjectConfig::load(r).ok())
        .unwrap_or_default();
    let base = base_of(root.as_deref(), dir).await;
    let set = crate::git::change_set(dir, &base).await;
    let ordered = crate::core::review::order(&set, config.review.roles.as_ref());
    crate::core::review::shape(&set, &ordered).says()
}

/// Every change, newest first, with its plan and, where a gate failed, the
/// agent's claim beside the exit code.
pub async fn change_list<'a>(
    snap: &'a Snapshot,
    store: &crate::store::Store,
) -> Vec<ChangeView<'a>> {
    let mut views = change_views(snap, store).await;
    let mut shaped = 0;
    for v in views.iter_mut() {
        if shaped < SHAPED && v.change.archived_at.is_none() && v.change.completion.is_none() {
            v.shape_says = shape_says(snap, v.change).await;
            shaped += 1;
        }
    }
    views
}

async fn change_views<'a>(snap: &'a Snapshot, store: &crate::store::Store) -> Vec<ChangeView<'a>> {
    let mut all: Vec<&Change> = snap.changes.iter().collect();
    all.sort_by_key(|w| std::cmp::Reverse(w.updated_at));
    let roots: BTreeMap<ProjectId, PathBuf> = snap
        .world
        .projects()
        .map(|p| (p.id.clone(), p.root.clone()))
        .collect();

    let mut views: Vec<ChangeView> = all
        .iter()
        .map(|w| {
            let can_retry = w.retryable(w.current_run().is_some_and(|r| snap.live.contains(r)));
            // Gates are declared by the person's checkout.
            let config = roots
                .get(&w.project_id)
                .and_then(|root| crate::core::ProjectConfig::load(root).ok());
            let declares_gates = config.as_ref().is_some_and(|c| c.has_gates());
            let view = ChangeView::of(w, can_retry, declares_gates);
            match (w.spec.as_deref(), roots.get(&w.project_id), config) {
                (Some(spec), Some(root), Some(config)) => {
                    // Boxes are read where the agent worked, as the gate is.
                    let worked_in = w.worktree.as_deref().unwrap_or(root);
                    let trace = crate::core::spec::ChangeFolder::at(worked_in, spec)
                        .ok()
                        .map(|f| config.spec.trace(&f));
                    let runs: Vec<&Run> = snap.world.runs().collect();
                    view.with_plan(crate::core::spec::Plan::read(
                        root,
                        spec,
                        &config.spec.open_questions,
                    ))
                    .with_edges(trace, &runs, declares_gates)
                }
                _ => view,
            }
        })
        .collect();

    let filed = store.reports(REPORTS_READ).await.unwrap_or_default();
    for view in views.iter_mut() {
        let id = &view.change.id;
        view.reports = filed
            .iter()
            .filter(|r| {
                r.provenance.change.as_ref() == Some(id)
                    || view.change.from_report.as_ref() == Some(&r.id)
            })
            .cloned()
            .map(|r| ReportView::of(r, snap.now))
            .collect();
    }

    for (view, w) in views.iter_mut().zip(all.iter()) {
        if !w.claim_is_worth_showing() {
            continue;
        }
        if let Some(run) = w.runs.last() {
            view.claim = store
                .last_agent_message(run)
                .await
                .ok()
                .flatten()
                .map(|t| crate::core::text::clip(t.trim(), 400));
        }
        if view.claim.is_none() {
            // No transcripts kept reads differently from an agent that said nothing.
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
    views
}

/// One change, as the list renders it, or `None` for an id nobody has.
pub async fn change_one(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &crate::core::ChangeId,
) -> Option<Value> {
    let mut v = change_views(snap, store)
        .await
        .into_iter()
        .find(|v| v.change.id == *id)?;
    v.shape_says = shape_says(snap, v.change).await;
    Some(serde_json::to_value(&v).unwrap_or(Value::Null))
}

/// The done certificate for one change, fully composed.
pub async fn change_certificate(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &crate::core::ChangeId,
) -> Option<Value> {
    let change = snap.change(id)?;
    let claim = match change.claim_is_worth_showing() {
        false => None,
        true => match change.runs.last() {
            Some(run) => store
                .last_agent_message(run)
                .await
                .ok()
                .flatten()
                .map(|t| crate::core::text::clip(t.trim(), 400)),
            None => None,
        },
    };
    // What the change did to its own checks, from its diff; `None` without a
    // worktree, which the certificate states.
    let weakened = match change.worktree.as_deref().filter(|d| d.is_dir()) {
        Some(dir) => {
            let root = snap
                .world
                .project(&change.project_id)
                .map(|p| p.root.clone());
            let config = root
                .as_deref()
                .and_then(|r| crate::core::ProjectConfig::load(r).ok())
                .unwrap_or_default();
            let base = base_of(root.as_deref(), dir).await;
            let set = crate::git::change_set(dir, &base).await;
            Some(crate::core::review::weakened(
                &set,
                config.review.roles.as_ref(),
            ))
        }
        None => None,
    };
    let mut cert = crate::core::certificate::Certificate::of(change, claim.as_deref());
    cert.weakened = weakened;
    Some(json!({
        "finished": cert.is_finished(),
        "markdown": cert.markdown_bounded(),
        "statement": cert.json(),
        "page": cert.page(),
    }))
}

// ── Review ──────────────────────────────────────────────────────────────────

/// The base a change is read against: declared, else discovered, never
/// `HEAD` (`HEAD...HEAD` is an empty diff).
pub async fn base_of(root: Option<&std::path::Path>, dir: &std::path::Path) -> String {
    match root {
        Some(root) => match crate::core::ProjectConfig::load(root)
            .ok()
            .and_then(|c| c.project.base_branch)
        {
            Some(b) => b,
            None => crate::git::base_branch(root).await,
        },
        None => crate::git::base_branch(dir).await,
    }
}

/// Why a review could not be composed: unknown change, or checkout gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoReview {
    NoSuchChange,
    NoCheckout,
}

impl NoReview {
    pub fn says(self) -> &'static str {
        match self {
            NoReview::NoSuchChange => "no such change",
            NoReview::NoCheckout => {
                "this change has no checkout on disk, so there is nothing to read"
            }
        }
    }
}

/// One change, composed for deciding whether to merge it. Every sentence is
/// composed here once, so no surface softens *no mapping declared*.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewView {
    pub change: String,
    pub title: String,
    pub base: String,
    /// The gates against the tree as it stands, once for the change.
    pub standing: crate::core::change::Standing,
    pub standing_says: String,
    /// *unordered — `[review] roles` orders it*, when no roles are declared.
    pub unordered: Option<String>,
    /// *no test mapping declared — `[review] covers` adds one*, when none is;
    /// then every `coverage_says` is null.
    pub coverage_absent: Option<String>,
    pub shape: crate::core::review::Shape,
    pub shape_says: String,
    /// The files by role, in reading order.
    pub groups: Vec<ReviewGroup>,
    pub intent: IntentView,
    pub formatter_only_collapsed: u32,
    /// *3 formatter-only hunks collapsed*, when there are any.
    pub formatter_only_says: Option<String>,
    pub truncated: Option<crate::core::diff::Truncation>,
    pub truncated_says: Option<String>,
    /// A branch that changed nothing is a finding, not an empty state.
    pub empty_says: Option<String>,
    /// The run a request for a fix is sent to: the change's newest.
    pub latest_run: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewGroup {
    /// `None` is undeclared, every file when unordered, or the
    /// *checks weakened or changed* group.
    pub role: Option<crate::core::review::Role>,
    pub says: String,
    pub files: Vec<ReviewFile>,
    /// Only on the *checks weakened or changed* group, which comes first;
    /// files here are not listed again under their role.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weakened: Vec<crate::core::review::Weakened>,
}

/// One file with the facts derivable per file: role, test coverage,
/// decisions taken while written, and task.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewFile {
    pub path: String,
    pub status: crate::core::diff::Status,
    pub status_says: String,
    pub added: u32,
    pub removed: u32,
    pub role: Option<crate::core::review::Role>,
    pub role_says: String,
    pub coverage: crate::core::review::Coverage,
    pub coverage_says: Option<String>,
    pub decisions: Vec<DecisionBrief>,
    /// Index into `intent.groups`, when a dispatched run with tasks wrote it.
    pub task_group: Option<usize>,
    pub task_says: String,
    /// Why there are no hunks, when there are none: binary, or not shown.
    pub body_says: Option<String>,
    pub hunks: Vec<ReviewHunk>,
    /// What to run to see each fact on this row for yourself.
    pub marker_commands: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewHunk {
    pub header: String,
    pub lines: Vec<(crate::core::diff::Kind, String)>,
    /// Whitespace only: collapsed by default, counted, expandable.
    pub formatter_only: bool,
}

/// A decision, briefly: who decided, what, when, about what.
#[derive(Debug, Serialize, Deserialize)]
pub struct DecisionBrief {
    pub authority: String,
    pub action: String,
    pub outcome: String,
    pub at: String,
    pub subject: String,
    pub says: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntentView {
    /// The granularity: a file belongs to the tasks its writing run was sent.
    pub heading: String,
    pub groups: Vec<IntentGroupView>,
    /// Always its own heading, always last, never merged into a group.
    pub not_asked_for: Vec<NotAskedForView>,
    pub not_asked_for_heading: String,
    /// Why grouping cannot be drawn, when it cannot.
    pub unavailable: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntentGroupView {
    pub run: String,
    pub tasks: Vec<crate::core::spec::SentTask>,
    /// The tasks' texts and the run, as the group's title.
    pub title: String,
    /// *2 files · 14+ 3− lines*, or that the run wrote none of this change.
    pub says: String,
    pub files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotAskedForView {
    pub path: String,
    pub why: crate::core::review::NotAskedFor,
    pub says: String,
}

/// A path a call named, relative to where the run worked.
fn relative_to(run: &Run, raw: &str) -> String {
    let p = std::path::Path::new(raw);
    if p.is_absolute() {
        for root in [run.worktree.as_ref(), Some(&run.cwd)]
            .into_iter()
            .flatten()
        {
            if let Ok(rel) = p.strip_prefix(root) {
                return rel.to_string_lossy().into_owned();
            }
        }
    }
    raw.trim_start_matches("./").to_string()
}

/// The review of one change: one `git diff`, one standing read, then a fold
/// over `[review]`, the change's runs and files, and its decisions.
pub async fn review(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &crate::core::ChangeId,
) -> Result<ReviewView, NoReview> {
    use crate::core::review;
    let change = snap.change(id).ok_or(NoReview::NoSuchChange)?;
    let dir = change
        .worktree
        .clone()
        .filter(|d| d.is_dir())
        .ok_or(NoReview::NoCheckout)?;
    let root = snap
        .world
        .project(&change.project_id)
        .map(|p| p.root.clone());
    let config = root
        .as_deref()
        .and_then(|r| crate::core::ProjectConfig::load(r).ok())
        .unwrap_or_default();
    let base = base_of(root.as_deref(), &dir).await;
    let set = crate::git::change_set(&dir, &base).await;
    let now = crate::git::commit_stamp(&dir).await;
    let standing = crate::core::change::Standing::of(change, config.has_gates(), now.as_ref());

    // The runs, oldest first, each with every file it named for writing.
    let runs: Vec<&Run> = change
        .runs
        .iter()
        .filter_map(|r| snap.world.run(r))
        .collect();
    let mut decisions: Vec<crate::core::Decision> = store
        .decisions(Some(change.id.as_str()), i64::MAX)
        .await
        .unwrap_or_default();
    let mut wrote: Vec<(&Run, Vec<String>)> = Vec::new();
    for run in &runs {
        let rows = store
            .decisions(Some(run.id.as_str()), i64::MAX)
            .await
            .unwrap_or_default();
        let mut files = run.wrote.clone();
        for f in crate::core::decision::files_a_shell_call_named_for_writing(&rows) {
            let f = relative_to(run, &f);
            if !files.contains(&f) {
                files.push(f);
            }
        }
        for d in rows {
            if !decisions.iter().any(|x| x.id == d.id) {
                decisions.push(d);
            }
        }
        wrote.push((run, files));
    }
    decisions.sort_by_key(|d| d.at);

    let roles = config.review.roles.as_ref();
    let covers = config.review.covers.as_deref();
    let ordered = review::order(&set, roles);
    let shape = review::shape(&set, &ordered);
    let intent = review::by_intent(&set, &wrote);

    let group_of = |i: usize| intent.groups.iter().position(|g| g.files.contains(&i));
    let group_title = |g: &review::IntentGroup| {
        let texts: Vec<&str> = g.tasks.iter().map(|t| t.text.as_str()).collect();
        format!("{} — run {}", texts.join("; "), g.run)
    };
    // The gate command that runs a test file, else the test's own path.
    let gate_commands: Vec<&String> = config
        .gates
        .check
        .iter()
        .chain(config.gates.named.values().flat_map(|g| g.run.iter()))
        .collect();
    let runs_test = |test: &str| -> String {
        gate_commands
            .iter()
            .find(|c| c.contains(test))
            .map(|c| c.to_string())
            .unwrap_or_else(|| test.to_string())
    };

    let file_view = |i: usize| -> ReviewFile {
        let f = &set.files[i];
        let role = ordered.role(i);
        let role_says = match (role, ordered.unordered) {
            (Some(r), _) => r.says().to_string(),
            (None, false) => "undeclared — no role in `[review] roles` matches".to_string(),
            (None, true) => "no roles declared".to_string(),
        };
        let coverage = review::coverage(&f.path, covers);
        let mine: Vec<DecisionBrief> = review::decisions_for(&f.path, &decisions)
            .into_iter()
            .map(|d| DecisionBrief {
                authority: d.authority.as_str().to_string(),
                action: d.action.clone(),
                outcome: d.outcome.clone(),
                at: d.at.to_string(),
                subject: d.subject.clone(),
                says: format!("{} · {} · {}", d.authority.as_str(), d.action, d.outcome),
            })
            .collect();
        let task_group = group_of(i);
        let task_says = match (&intent.unavailable, task_group) {
            (Some(why), _) => why.clone(),
            (None, Some(g)) => group_title(&intent.groups[g]),
            (None, None) => intent
                .not_asked_for
                .iter()
                .find(|(j, _)| *j == i)
                .map(|(_, why)| format!("not asked for — {}", why.says()))
                .unwrap_or_default(),
        };
        let mut marker_commands = vec![format!(
            "git diff $(git merge-base {base} HEAD) -- {}",
            f.path
        )];
        if let review::Coverage::Covered { test } = &coverage {
            marker_commands.push(runs_test(test));
        }
        if !mine.is_empty() {
            marker_commands.push(format!("devplane audit {}", change.id));
        }
        let (body_says, hunks) = match &f.body {
            crate::core::diff::Body::Hunks(h) => (
                None,
                h.iter()
                    .map(|h| ReviewHunk {
                        header: h.header.clone(),
                        lines: h.lines.clone(),
                        formatter_only: review::formatter_only(&f.path, h),
                    })
                    .collect(),
            ),
            crate::core::diff::Body::Binary { bytes } => (
                Some(match bytes {
                    Some(n) => format!("binary, {n} bytes"),
                    None => "binary".to_string(),
                }),
                Vec::new(),
            ),
            crate::core::diff::Body::Skipped { why } => {
                (Some(format!("not shown — {why}")), Vec::new())
            }
        };
        ReviewFile {
            path: f.path.clone(),
            status_says: match &f.status {
                crate::core::diff::Status::Added => "added".into(),
                crate::core::diff::Status::Modified => "modified".into(),
                crate::core::diff::Status::Deleted => "deleted".into(),
                crate::core::diff::Status::Renamed { from } => format!("renamed from {from}"),
            },
            status: f.status.clone(),
            added: f.added,
            removed: f.removed,
            role,
            role_says,
            coverage_says: coverage.says(),
            coverage,
            decisions: mine,
            task_group,
            task_says,
            body_says,
            hunks,
            marker_commands,
        }
    };

    // Checks weakened or changed come first, before the rest is read as
    // though they meant what they did. Absent, not empty, when none.
    let weak = review::weakened(&set, roles);
    let mut first: Vec<usize> = Vec::new();
    for w in &weak {
        if let Some(i) = set.files.iter().position(|f| f.path == w.path)
            && !first.contains(&i)
        {
            first.push(i);
        }
    }
    let mut groups: Vec<ReviewGroup> = Vec::new();
    if !weak.is_empty() {
        groups.push(ReviewGroup {
            role: None,
            says: "checks weakened or changed".to_string(),
            files: first.iter().map(|i| file_view(*i)).collect(),
            weakened: weak,
        });
    }
    groups.extend(ordered.groups.iter().filter_map(|(role, files)| {
        let files: Vec<ReviewFile> = files
            .iter()
            .filter(|i| !first.contains(i))
            .map(|i| file_view(*i))
            .collect();
        (!files.is_empty()).then(|| ReviewGroup {
            role: *role,
            says: match (role, ordered.unordered) {
                (Some(r), _) => r.says().to_string(),
                (None, false) => "undeclared".to_string(),
                (None, true) => "every file, in path order".to_string(),
            },
            files,
            weakened: Vec::new(),
        })
    }));

    let lines_of = |files: &[usize]| {
        let (a, r) = files.iter().fold((0, 0), |(a, r), i| {
            (a + set.files[*i].added, r + set.files[*i].removed)
        });
        let n = files.len();
        format!(
            "{n} {} · {a}+ {r}− lines",
            if n == 1 { "file" } else { "files" }
        )
    };
    let intent_view = IntentView {
        heading: "grouped by the run that wrote each file".into(),
        groups: intent
            .groups
            .iter()
            .map(|g| IntentGroupView {
                run: g.run.to_string(),
                tasks: g.tasks.clone(),
                title: group_title(g),
                says: match g.files.is_empty() {
                    true => "this run wrote none of the files this change changed".into(),
                    false => lines_of(&g.files),
                },
                files: g.files.iter().map(|i| set.files[*i].path.clone()).collect(),
            })
            .collect(),
        not_asked_for: intent
            .not_asked_for
            .iter()
            .map(|(i, why)| NotAskedForView {
                path: set.files[*i].path.clone(),
                says: why.says(),
                why: why.clone(),
            })
            .collect(),
        not_asked_for_heading: "not asked for".into(),
        unavailable: intent.unavailable.clone(),
    };

    let collapsed = shape.formatter_only;
    Ok(ReviewView {
        change: change.id.to_string(),
        title: change.title.clone(),
        standing_says: standing.says(),
        standing,
        unordered: ordered
            .unordered
            .then(|| "unordered — `[review] roles` orders it".to_string()),
        coverage_absent: covers
            .is_none_or(|c| c.is_empty())
            .then(|| "no test mapping declared — `[review] covers` adds one".to_string()),
        shape_says: shape.says(),
        shape,
        groups,
        intent: intent_view,
        formatter_only_collapsed: collapsed,
        formatter_only_says: (collapsed > 0).then(|| match collapsed {
            1 => "1 formatter-only hunk collapsed".to_string(),
            n => format!("{n} formatter-only hunks collapsed"),
        }),
        truncated_says: set.truncated.as_ref().map(|t| {
            format!(
                "showing {} of {} files — for all of it: {}",
                t.files_shown, t.files_total, t.command
            )
        }),
        truncated: set.truncated.clone(),
        empty_says: set.is_empty().then(|| {
            format!(
                "this branch changed nothing against {}; any check that passed here passed over \
                 no change",
                set.base
            )
        }),
        latest_run: change.runs.last().map(|r| r.to_string()),
        base,
    })
}

// ── Specs, projects, setup, rules ───────────────────────────────────────────

/// The plan each project is working to: one row per in-flight change with its
/// specification as on disk now. A plan belongs to a change, not a project.
pub fn specs(snap: &Snapshot) -> Value {
    const MOST_PROJECTS: usize = 40;
    const MOST_PLANS: usize = 25;

    let roots: BTreeMap<ProjectId, (String, PathBuf)> = snap
        .world
        .projects()
        .map(|p| (p.id.clone(), (p.name.clone(), p.root.clone())))
        .collect();
    let markers: BTreeMap<ProjectId, Vec<String>> = roots
        .iter()
        .map(|(id, (_, root))| {
            let m = crate::core::ProjectConfig::load(root)
                .map(|c| c.spec.open_questions)
                .unwrap_or_default();
            (id.clone(), m)
        })
        .collect();

    // In flight, plus unapproved `Done`: a done verdict beside an incomplete
    // plan is what this surfaces.
    let mut in_flight: Vec<&Change> = snap
        .changes
        .iter()
        .filter(|w| !w.is_settled() || w.completion.is_some())
        .collect();
    in_flight.sort_by_key(|w| std::cmp::Reverse(w.updated_at));
    let mut worked: BTreeMap<(ProjectId, String), &Change> = Default::default();
    for w in in_flight {
        if let Some(spec) = w.spec.as_deref() {
            worked
                .entry((w.project_id.clone(), spec.to_string()))
                .or_insert(w);
        }
    }

    let mut by_project: BTreeMap<ProjectId, Vec<Value>> = Default::default();
    for (id, (_, root)) in &roots {
        let empty = Vec::new();
        let m = markers.get(id).unwrap_or(&empty);
        let mut paths: Vec<String> = crate::core::ProjectConfig::load(root)
            .map(|c| c.spec.plan_paths(root))
            .unwrap_or_default();
        for (pid, spec) in worked.keys() {
            if pid == id && !paths.contains(spec) {
                paths.push(spec.clone());
            }
        }
        paths.sort();
        paths.dedup();
        let cfg = crate::core::ProjectConfig::load(root).ok();
        for spec in paths.into_iter().take(MOST_PLANS) {
            let plan = crate::core::spec::Plan::read(root, &spec, m);
            let w = worked.get(&(id.clone(), spec.clone()));
            // The edges: a requirement token in a heading and a task line.
            // Orphans are named; with no tokens at all, the notation is
            // reported as unrecognised, not as missing coverage.
            let trace = cfg.as_ref().and_then(|c| {
                crate::core::spec::ChangeFolder::at(root, &spec)
                    .ok()
                    .map(|change| c.spec.trace(&change))
            });
            // The two counts, over the working change's own runs.
            let (counts, token_rows) = match (w, &trace) {
                (Some(w), Some(t)) if !t.unrecognised => {
                    let mine: Vec<&Run> =
                        w.runs.iter().filter_map(|id| snap.world.run(id)).collect();
                    let gates = cfg.as_ref().is_some_and(|c| c.has_gates());
                    let last_pass = w.last_pass_at();
                    (
                        Some(crate::core::spec::counts(t, &mine, gates, last_pass)),
                        crate::core::spec::token_rows(t, &mine, gates, last_pass)
                            .into_iter()
                            .map(|row| {
                                json!({
                                    "token": row.token,
                                    "tasks": row.tasks,
                                    "ticked": row.ticked,
                                    "verified": row.verified_tasks,
                                    "says": row.says(),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                }
                _ => (None, Vec::new()),
            };
            by_project.entry(id.clone()).or_default().push(json!({
                "counts_says": counts.as_ref().map(|c| c.says(false)),
                "counts": counts,
                "token_rows": token_rows,
                "change_id": w.map(|w| w.id.as_str()),
                "title": w.map(|w| w.title.clone()),
                "state": w.map(|w| w.current_state()),
                "contradicts_done": w.is_some_and(|w| {
                    w.completion.is_some() && plan.contradicts_done()
                }),
                "drifted": w.and_then(|w| w.plan_drifted(plan.fingerprint.as_deref())),
                "plan": plan,
                "trace": trace,
            }));
        }
    }
    let omitted = roots.len().saturating_sub(MOST_PROJECTS);
    let out: Vec<_> = roots
        .iter()
        .take(MOST_PROJECTS)
        .map(|(id, (name, root))| {
            json!({
                "project_id": id.as_str(),
                "project": name,
                "root": root.display().to_string(),
                // Absent and empty differ: no markers declared is not zero.
                "declares_markers": !markers.get(id).map(|m| m.is_empty()).unwrap_or(true),
                "declares_plans": crate::core::ProjectConfig::load(root)
                    .map(|c| c.spec.plans.is_some())
                    .unwrap_or(false),
                // The layouts found, or the sentence on how to declare another.
                "layouts": crate::core::ProjectConfig::load(root)
                    .map(|c| c.spec.layouts(root))
                    .unwrap_or_default(),
                "no_layout": crate::core::ProjectConfig::load(root)
                    .map(|c| c.spec.layouts(root).is_empty())
                    .unwrap_or(true)
                    .then_some(crate::core::spec::NO_LAYOUT),
                "plans": by_project.get(id).cloned().unwrap_or_default(),
            })
        })
        .collect();
    json!({ "projects": out, "omitted": omitted })
}

/// Every registered project with what its configuration offers, read fresh.
pub fn projects(snap: &Snapshot) -> Vec<Value> {
    let live: BTreeMap<_, usize> =
        snap.world
            .runs()
            .filter(|r| r.is_active())
            .fold(BTreeMap::new(), |mut acc, r| {
                if let Some(p) = &r.project_id {
                    *acc.entry(p.clone()).or_default() += 1;
                }
                acc
            });
    snap.world
        .projects()
        .map(|p| {
            let cfg = crate::core::ProjectConfig::load(&p.root).ok();
            json!({
                "id": p.id.as_str(),
                "name": p.name,
                "root": p.root.display().to_string(),
                "trusted": p.trusted,
                "sessions": live.get(&p.id).copied().unwrap_or(0),
                "repo": p.repo_slug(),
                "default_agent": cfg.as_ref().and_then(|c| c.project.default_agent.clone()),
            })
        })
        .collect()
}

/// Everything that is configured, on this machine and in every project.
pub fn setup(snap: &Snapshot) -> Value {
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
                    "problems": g.validate(),
                })
            }
            // Worst failure here: every repository inherits this file.
            Err(e) => json!({
                "path": path.display().to_string(),
                "exists": true,
                "error": e.to_string(),
            }),
        }
    });
    let provider = crate::core::provider::from_env();
    let projects: Vec<Value> = snap
        .world
        .projects()
        .map(|p| {
            let file = p.root.join(crate::core::config::CONFIG_FILE);
            let mut out = json!({
                "id": p.id.as_str(),
                "name": p.name,
                "root": p.root.display().to_string(),
                "trusted": p.trusted,
                "config": {"path": file.display().to_string(), "exists": file.exists()},
            });
            match crate::core::ProjectConfig::load(&p.root) {
                Ok(cfg) => {
                    out["describes"] = cfg.describe();
                    // *Declares gates*: `verified` means something else here.
                    out["declares_gates"] = json!(cfg.has_gates());
                }
                Err(e) => out["error"] = json!(e.to_string()),
            }
            out
        })
        .collect();
    json!({
        "machine": {
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "started_at": snap.started_at.map(|t| t.to_string()),
            "uptime_seconds": snap.started_at.map(|t| (snap.now - t).get_seconds()),
            "host": snap.from_host,
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
        "gate": { "syntax_modelled_on": crate::core::policy::SYNTAX_MODELLED_ON },
        "machine_policy": machine_policy,
        "projects": projects,
        "agents": snap.agents,
    })
}

/// A project's rule files, read from disk. An unparseable file is an error,
/// never an empty list: the vendor applies none of its settings.
pub fn read_rules(p: &crate::core::Project) -> crate::core::rules::ProjectRules {
    use crate::core::rules::RuleSet;

    let cfg = p.root.join(crate::core::config::CONFIG_FILE);
    let devplane = match crate::core::ProjectConfig::load(&p.root) {
        Ok(c) => RuleSet {
            file: cfg.display().to_string(),
            deny: c.policy.never_auto.clone(),
            ask: c.policy.always_ask.clone(),
            error: None,
        },
        Err(e) => RuleSet {
            file: cfg.display().to_string(),
            error: Some(e.to_string()),
            ..Default::default()
        },
    };
    // `settings.local.json` and the committed file form one set; the agent reads both.
    let mut agent = RuleSet {
        file: p.root.join(".claude/settings.json").display().to_string(),
        ..Default::default()
    };
    for rel in [".claude/settings.json", ".claude/settings.local.json"] {
        let path = p.root.join(rel);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(v) => {
                for (key, into) in [("deny", &mut agent.deny), ("ask", &mut agent.ask)] {
                    if let Some(list) = v
                        .get("permissions")
                        .and_then(|p| p.get(key))
                        .and_then(|a| a.as_array())
                    {
                        into.extend(list.iter().filter_map(|r| r.as_str().map(str::to_string)));
                    }
                }
            }
            Err(e) => {
                agent.error = Some(format!("{}: {e}", path.display()));
                agent.deny.clear();
                agent.ask.clear();
                break;
            }
        }
    }
    crate::core::rules::ProjectRules {
        project: p.name.clone(),
        devplane,
        agent,
    }
}

/// Which project is missing a rule relied on elsewhere, or where one rule
/// stands across every project.
pub fn rules(snap: &Snapshot, rule: Option<&str>, ask: bool) -> Result<Value, Value> {
    let read: Vec<crate::core::rules::ProjectRules> =
        snap.world.projects().map(read_rules).collect();
    let Some(raw) = rule.map(str::trim).filter(|r| !r.is_empty()) else {
        return Ok(json!({
            "asked": null,
            "disagreements": crate::core::rules::disagreements(&read),
            "projects": read.len(),
            "wrote_nothing": crate::core::rules::WROTE_NOTHING,
            "why_no_apply_to_all": crate::core::rules::WHY_NO_APPLY_TO_ALL,
        }));
    };
    let class = if ask {
        crate::core::policy::Class::Ask
    } else {
        crate::core::policy::Class::Deny
    };
    let Some(wanted) = crate::core::Rule::parse(raw, class).filter(|r| !r.is_malformed()) else {
        return Err(json!({
            "error": format!("`{raw}` is not a rule this syntax can express"),
            "hint": "a rule looks like `Bash(curl:*)`, `Read(./.env)` or `mcp__github(create_issue)`",
        }));
    };
    Ok(json!({
        "asked": wanted.as_str(),
        "rows": crate::core::rules::compare(&read, &wanted),
        "wrote_nothing": crate::core::rules::WROTE_NOTHING,
        "why_no_apply_to_all": crate::core::rules::WHY_NO_APPLY_TO_ALL,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::change::{CommandResult, CommitStamp, GateReport, Reach, Standing};

    fn stamp(tree: &str, clean: bool, changed: u32) -> CommitStamp {
        CommitStamp {
            commit: Some(format!("commit-of-{tree}")),
            tree: Some(tree.into()),
            branch: Some("feat/x".into()),
            clean,
            changed_files: changed,
            reach: Reach::LocalOnly,
            remote: None,
        }
    }

    fn passed_at(tree: &str) -> Change {
        let mut c = Change::new(
            ProjectId::new("/repo"),
            "rate-limit login".into(),
            "…".into(),
        );
        c.worktree = Some("/repo/.claude/worktrees/x".into());
        c.runs.push(RunId::new("r1"));
        c.gates.push(GateReport {
            gate: "check".into(),
            at: jiff::Timestamp::now(),
            duration_ms: 1,
            commands: vec![CommandResult::exited("cargo test", 0)],
            attempt: 1,
            spec: None,
            commit: Some(stamp(tree, true, 0)),
        });
        c
    }

    /// A stale pass reaches the surface as stale, naming both trees.
    #[test]
    fn a_pass_the_tree_moved_under_is_served_as_stale_with_both_digests() {
        let mut c = passed_at("aaaaaaa111");
        c.tree_now = Some(stamp("aaaaaaa111", true, 0));
        let v = ChangeView::of(&c, false, true);
        assert_eq!(v.state, crate::core::ChangeState::Verified);
        assert_eq!(v.glyph, "✓");
        assert_eq!(v.standing, Standing::Verified);

        // The branch moved on: a different tree.
        c.tree_now = Some(stamp("bbbbbbb222", true, 0));
        let v = ChangeView::of(&c, false, true);
        assert_eq!(v.state, crate::core::ChangeState::InFlight);
        assert!(matches!(v.standing, Standing::Stale { .. }));
        assert!(
            v.standing_says.starts_with("gates passed, stale"),
            "{}",
            v.standing_says
        );
        assert!(
            v.standing_says.contains("aaaaaaa") && v.standing_says.contains("bbbbbbb"),
            "both digests: {}",
            v.standing_says
        );

        // Uncommitted work alone is not a reason: same working-tree digest.
        c.tree_now = Some(stamp("aaaaaaa111", false, 1));
        let v = ChangeView::of(&c, false, true);
        assert_eq!(v.state, crate::core::ChangeState::Verified);

        // A touched file changes the working-tree digest.
        c.tree_now = Some(stamp("ccccccc333", false, 1));
        let v = ChangeView::of(&c, false, true);
        assert_ne!(v.state, crate::core::ChangeState::Verified);
        assert!(
            v.standing_says.contains("changed after"),
            "{}",
            v.standing_says
        );

        // Unknown is not unchanged.
        c.tree_now = None;
        let v = ChangeView::of(&c, false, true);
        assert_ne!(v.state, crate::core::ChangeState::Verified);
    }

    /// A project with no gates never verifies, and says so in words.
    #[test]
    fn no_gates_declared_is_a_sentence_and_never_verified() {
        let mut c = passed_at("aaaaaaa111");
        c.tree_now = Some(stamp("aaaaaaa111", true, 0));
        let v = ChangeView::of(&c, false, false);
        assert_eq!(v.standing, Standing::NoGatesDeclared);
        assert_eq!(v.standing.word(), "no gates declared");
        assert!(v.standing_says.starts_with("no gates declared"));
        // The state follows the existing gate report; the standing is the
        // checkout's answer.
        assert_eq!(v.state, crate::core::ChangeState::Verified);
    }
}

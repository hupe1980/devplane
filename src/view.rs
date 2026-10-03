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
             gate probe are not visible from here; `devplane up` starts one",
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
    /// When the tree `state` and `standing` read was read: the board shows
    /// them from the last read, not a read made now. `None` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_as_of: Option<String>,
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
    /// mode is unreported or unrecognised. For a driven run, from its mode
    /// statement.
    #[serde(default)]
    pub asks_a_person: Option<bool>,
    /// What a driven run says about its session's permission mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_statement: Option<ModeStatementView>,
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
    pub sent: Option<Vec<crate::spec::SentTask>>,
    /// What the specification said when this run last closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<crate::core::run::Observed>,
    /// *sent 4 tasks*, *no tasks were sent to this run*, or the sentence for
    /// a session Devplane only watched.
    #[serde(default)]
    pub sent_says: String,
}

/// A run's mode statement as surfaces carry it: the sentence, and the facts it
/// is made of. *Decides without you* is never folded into a count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeStatementView {
    pub sentence: String,
    /// The agent's own id for the mode.
    #[serde(default)]
    pub mode: Option<String>,
    /// The agent's own name for it.
    #[serde(default)]
    pub name: Option<String>,
    pub reading: crate::core::run::Reading,
    #[serde(default)]
    pub decides: Option<String>,
    pub why: crate::core::run::Why,
    #[serde(default)]
    pub detail: Option<String>,
    /// When it was said.
    #[serde(default)]
    pub at: Option<String>,
}

impl ModeStatementView {
    pub fn of(run: &Run) -> Option<Self> {
        let s = run.mode_statement.as_ref()?;
        Some(Self {
            sentence: s.sentence(),
            mode: s.mode.clone(),
            name: s.name.clone(),
            reading: s.reading,
            decides: s.decides.clone(),
            why: s.why,
            detail: s.detail.clone(),
            at: run.mode_statement_at.map(|t| t.to_string()),
        })
    }
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
            asks_a_person: match &run.mode_statement {
                Some(s) => s.asks_a_person(),
                None => run.permission_mode.as_ref().and_then(|m| m.asks_a_person()),
            },
            mode_statement: ModeStatementView::of(run),
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
    let runs = if all {
        w.board()
    } else {
        w.working_set(snap.now)
    };
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
    let mut summary = w.summary(snap.now);
    if let Some(forge) = forge {
        for c in forge.values() {
            // Last-good numbers are not current ones: said, not summed.
            if c.stale.is_some() {
                summary.forge_stale += 1;
                continue;
            }
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
    // Per project: `Some` the declared `check` commands (empty: none), `None`
    // an unloadable file.
    let mut declares: BTreeMap<String, Option<Vec<String>>> = BTreeMap::new();
    for c in &changes {
        declares.entry(c.project_id.to_string()).or_insert_with(|| {
            match w.project(&c.project_id) {
                Some(p) => crate::core::ProjectConfig::load(&p.root)
                    .ok()
                    .map(|cfg| cfg.gates.check.clone()),
                None => Some(Vec::new()),
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
                // One verdict decides the state and the standing both.
                let declared = crate::core::change::Declared::of(
                    declares
                        .get(w.project_id.as_str())
                        .and_then(|c| c.as_deref()),
                );
                let state = w.state(declared, w.tree_now.as_ref());
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
                    standing: w.verdict(declared, w.tree_now.as_ref()).word().to_string(),
                    tree_as_of: w.tree_as_of.map(|t| t.to_string()),
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
    policy: &crate::policy_cache::PolicyCache,
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
    let mut snoozed = forge.snoozed;
    let forge_items = forge.items;

    // Unresolved-question lines in each in-flight change's specification,
    // one item per line, read from the checkout the change works in. Derived
    // here because `World` may not walk a folder.
    let mut from_disk = {
        let mut out = Vec::new();
        let asked = crate::core::attention::AttentionKind::SpecQuestion;
        for p in snap.world.projects() {
            let markers = crate::core::ProjectConfig::load(&p.root)
                .map(|c| c.spec.open_questions)
                .unwrap_or_default();
            if markers.is_empty() {
                continue;
            }
            for w in changes
                .iter()
                .filter(|w| w.project_id == p.id && !w.is_settled())
            {
                let questions = crate::draft::question_lines(w, &p.root, &markers);
                if questions.is_empty() {
                    continue;
                }
                let mark = crate::core::attention::ActivityMark::of_change(w, |r| {
                    snap.world.run(r).map(|r| r.last_event_at)
                });
                let items = crate::core::attention::spec_question_items_at(
                    &p.name,
                    w,
                    &questions,
                    &markers,
                    snap.started_at.unwrap_or(snap.now),
                );
                for item in items {
                    if w.snoozed.hides(&asked, snap.now, &mark)
                        && let Some(until) = w.snoozed.until(&asked)
                    {
                        snoozed.push(crate::core::attention::SnoozedRow::new(
                            item.id.as_str().to_string(),
                            &asked,
                            item.title,
                            until,
                            &format!("/api/changes/{}", w.id.as_str()),
                            !w.current_run().is_some_and(|r| snap.live.contains(r)),
                        ));
                        continue;
                    }
                    out.push(item);
                }
            }
        }
        out
    };

    // Spec drift under a run, for every open change; its snooze covers it.
    {
        let runs: Vec<&Run> = snap.world.runs().collect();
        let drifted = crate::core::attention::AttentionKind::SpecDrifted;
        for w in changes.iter().filter(|w| !w.is_settled()) {
            let mark = crate::core::attention::ActivityMark::of_change(w, |r| {
                snap.world.run(r).map(|r| r.last_event_at)
            });
            for d in w.drifts(&runs) {
                let item = crate::core::attention::spec_drifted_item_at(w, &d, snap.now);
                if w.snoozed.hides(&drifted, snap.now, &mark)
                    && let Some(until) = w.snoozed.until(&drifted)
                {
                    snoozed.push(crate::core::attention::SnoozedRow::new(
                        item.id.as_str().to_string(),
                        &drifted,
                        item.title,
                        until,
                        &format!("/api/changes/{}", w.id.as_str()),
                        !w.current_run().is_some_and(|r| snap.live.contains(r)),
                    ));
                    continue;
                }
                from_disk.push(item);
            }
        }
    }

    let gate_down = snap.gate.clone().flatten();
    // The declared `check` commands, so the ready row agrees with the change
    // document on a pass by commands the project no longer declares.
    let checks: Vec<(ProjectId, Vec<String>)> = snap
        .world
        .projects()
        .filter_map(|p| {
            let c = crate::core::ProjectConfig::load(&p.root).ok()?;
            Some((p.id.clone(), c.gates.check))
        })
        .collect();
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
            now: snap.now,
            checks: &checks,
        },
        forge_items,
        from_disk,
    );
    derived.snoozed.extend(snoozed);
    let items = &mut derived.items;
    // Asks that outlived the process that asked them, only where the run is
    // not live (a live run already produced its own item).
    for ask in &snap.open_asks {
        if snap.live.contains(&ask.run) {
            continue;
        }
        items.push(crate::core::attention::stranded_ask_item(ask, snap.now));
    }
    let mut derived = derived.rank();
    fill_offers(snap, store, policy, &mut derived.items).await;
    derived
}

/// The rule to paste, on every permission item that can have one. A query,
/// so bounded: only when a permission item is open, scoped to its repository.
async fn fill_offers(
    snap: &Snapshot,
    store: &crate::store::Store,
    policy: &crate::policy_cache::PolicyCache,
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
        let root = crate::repo::governing_root(&cwd);
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
    /// On a *ready to decide* row: what its review leads with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<crate::core::review::ReadyFacts>,
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

/// The inbox's one number ([`crate::core::attention::count`]) as of `snap`,
/// with `held` the ask a held answer belongs to. What the tray and the dock
/// badge show; `/api/inbox` carries the same figure as `count`.
pub async fn inbox_count(
    snap: &Snapshot,
    store: &crate::store::Store,
    policy: &crate::policy_cache::PolicyCache,
    held: Option<&str>,
) -> usize {
    let derived = current_inbox(snap, store, policy).await;
    crate::core::attention::count(derived.items, held)
}

/// The inbox as a person sees it: narrowed, inhibited, folded, with the close.
/// `held` is the ask a held answer belongs to: its item is out of the list
/// and out of the count while it waits.
pub async fn inbox(
    snap: &Snapshot,
    store: &crate::store::Store,
    policy: &crate::policy_cache::PolicyCache,
    q: &InboxQuery,
    held: Option<&str>,
) -> InboxView {
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
    let mut derived = current_inbox(snap, store, policy).await;
    if let Some(h) = held {
        derived
            .items
            .retain(|i| i.ask.as_ref().is_none_or(|a| a.as_str() != h));
    }
    // The whole inbox's number, whatever this read narrows to.
    let count = crate::core::attention::count(derived.items.clone(), None);
    let raised = derived.items.len();
    let snoozed = derived.snoozed.len();
    let snoozed_rows = derived.snoozed;
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

    let mut rows: Vec<WaitingRow> = listed
        .into_iter()
        .map(|item| WaitingRow {
            project_name: item
                .project_id
                .as_ref()
                .and_then(|id| names.get(id).cloned()),
            new_to_you: crate::core::close::new_to_you(item.since, mark),
            facts: None,
            item,
        })
        .collect();
    for row in rows.iter_mut() {
        if row.item.kind == crate::core::AttentionKind::ReadyToDecide
            && let Some(id) = row.item.change_id.as_ref()
        {
            row.facts = ready_facts(snap, store, id).await;
        }
    }

    // The close is composed here so no surface words a day itself. Empty means
    // nothing raised at all, and it does not render over a narrowed list.
    let nothing_raised = rows.is_empty() && folded.is_empty() && inhibited.is_empty();
    let close = match narrowing.is_none() {
        true => Some(close(snap, store, nothing_raised).await),
        false => None,
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
    InboxView {
        items: rows,
        folded,
        inhibited,
        narrowed,
        count,
        snoozed,
        snoozed_rows,
        close,
        limits: snap.limits().map(str::to_string),
        held: None,
        landed: None,
    }
}

/// `/api/inbox`: served by the host, composed by a terminal with no host, and
/// read back by the terminal as this same type.
#[derive(Debug, Serialize, Deserialize)]
pub struct InboxView {
    pub items: Vec<WaitingRow>,
    #[serde(default)]
    pub folded: Vec<crate::core::attention::Summary>,
    #[serde(default)]
    pub inhibited: Vec<crate::core::attention::Inhibited>,
    #[serde(default)]
    pub narrowed: Option<crate::core::attention::Narrowed>,
    /// The inbox's one number, of the whole inbox even when this read is
    /// narrowed ([`crate::core::attention::count`]): what the badge, the
    /// activity bar and the status bar show.
    #[serde(default)]
    pub count: usize,
    /// A snooze is a choice; counted so the list does not look like calm.
    #[serde(default)]
    pub snoozed: usize,
    /// What the snooze keeps off the list, one row per item, each with its
    /// way back: `snoozed` is their number.
    #[serde(default)]
    pub snoozed_rows: Vec<crate::core::attention::SnoozedRow>,
    /// The boundary and the day's close; absent over a narrowed list.
    #[serde(default)]
    pub close: Option<Close>,
    /// Why the answer is partial, when it was read without a host.
    #[serde(default)]
    pub limits: Option<String>,
    /// The answer the window gave and has not sent yet: its item is out of
    /// `items` while held, and is shown here instead — never hidden uncounted.
    #[serde(default)]
    pub held: Option<crate::hold::HeldView>,
    /// What the last held answer came to once it was sent.
    #[serde(default)]
    pub landed: Option<crate::hold::Landed>,
}

impl InboxView {
    /// Lays the host's held answer over the inbox: the item it answers leaves
    /// the list while it waits, and the hold says what it is.
    pub fn hold(
        &mut self,
        held: Option<crate::hold::HeldView>,
        landed: Option<crate::hold::Landed>,
    ) {
        // The item it answers was left out of the list and the count when the
        // inbox was composed ([`inbox`]'s `held`).
        if let Some(h) = &held {
            self.items
                .retain(|r| r.item.ask.as_ref().is_none_or(|a| a.as_str() != h.ask));
        }
        self.held = held;
        self.landed = landed;
    }
}

/// The boundary since the last look and, for an empty inbox, what the day
/// came to. Every sentence is composed here; surfaces only lay them out.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Close {
    #[serde(default)]
    pub since_last_look: Option<String>,
    #[serde(default)]
    pub looked_at: Option<String>,
    /// The day could not be read; never shown as an empty day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreadable: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet: Option<bool>,
    #[serde(default)]
    pub sentences: Vec<String>,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keeps_running: Option<String>,
}

/// The review's leading facts for a change ready to decide, composed from the
/// review itself so the row and the review cannot disagree.
async fn ready_facts(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &crate::core::ChangeId,
) -> Option<crate::core::review::ReadyFacts> {
    let r = review_read(snap, store, id, true).await.ok()?;
    let files: Vec<&ReviewFile> = r.groups.iter().flat_map(|g| g.files.iter()).collect();
    let hunks = files.iter().map(|f| f.hunks.len() as u32).sum();
    let hunks_seen = files
        .iter()
        .flat_map(|f| f.hunks.iter())
        .filter(|h| h.seen)
        .count() as u32;
    let not_covered = r.coverage_absent.is_none().then(|| {
        files
            .iter()
            .filter(|f| matches!(f.coverage, crate::core::review::Coverage::NotCovered))
            .count() as u32
    });
    let not_asked_for = r.intent.unavailable.is_none().then(|| {
        r.intent
            .not_asked_for
            .iter()
            .filter_map(|n| files.iter().find(|f| f.path == n.path))
            .map(|f| f.hunks.len() as u32)
            .sum()
    });
    Some(crate::core::review::ReadyFacts {
        hunks_seen: Some(hunks_seen),
        ..crate::core::review::ReadyFacts::of(r.qualifier, hunks, not_covered, not_asked_for)
    })
}

/// The boundary, and — for an empty inbox — what the day came to.
pub async fn close(snap: &Snapshot, store: &crate::store::Store, empty: bool) -> Close {
    let now = snap.now;
    let last = store.last_look().await;
    let since = crate::core::close::since_last_look(now, last);
    let looked_at = last.map(|t| t.to_string());
    if !empty {
        return Close {
            since_last_look: since,
            looked_at,
            ..Close::default()
        };
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
            return Close {
                since_last_look: since,
                looked_at,
                unreadable: Some(e.to_string()),
                ..Close::default()
            };
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
    Close {
        since_last_look: since,
        looked_at,
        unreadable: None,
        clear: true,
        quiet: Some(tally.quiet()),
        sentences: tally.sentences(),
        next: soonest,
        keeps_running: Some(keeps_running),
    }
}

// ── Asks ────────────────────────────────────────────────────────────────────

/// One ask, with its outcome sentence composed in `core::ask`: what the
/// host answers and what the CLI reads back, one type for both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskView {
    pub id: String,
    pub kind: String,
    pub run: String,
    pub project: Option<String>,
    pub message: String,
    pub payload: Value,
    pub asked_at: String,
    pub open: bool,
    pub outcome: String,
    pub deadline: crate::core::ask::Deadline,
    pub deadline_says: String,
    pub answered_at: Option<String>,
    pub answered_from: Option<String>,
    pub delivery: Option<crate::core::ask::Delivery>,
    pub delivered: Option<bool>,
    pub ended: Option<crate::core::ask::Ended>,
}

/// A question nobody answered in a watched session: the agent asked and
/// moved on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbandonedAsk {
    pub run: String,
    pub project: Option<String>,
    pub question: String,
    pub options: Vec<crate::core::event::Choice>,
    pub asked_at: String,
    pub abandoned_at: String,
    pub moved_on_to: Option<String>,
    pub authority: String,
    pub outcome: String,
}

/// `/api/asks`: everything an agent has asked, and what became of each one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsksView {
    pub open: Vec<AskView>,
    pub settled: Vec<AskView>,
    pub abandoned: Vec<AbandonedAsk>,
    /// Which vendors are unseen, so an empty list never reads as none.
    pub blind_to: Vec<String>,
}

/// One ask, with its outcome sentence composed in `core::ask`.
pub fn render_ask(a: &Ask) -> AskView {
    AskView {
        id: a.id.as_str().to_string(),
        kind: a.kind.as_str().to_string(),
        run: a.run.as_str().to_string(),
        project: a.project.as_ref().map(|p| p.as_str().to_string()),
        message: a.message.clone(),
        payload: a.payload.clone(),
        asked_at: a.asked_at.to_string(),
        open: a.is_open(),
        outcome: a.outcome(),
        deadline: a.deadline,
        deadline_says: a.deadline.says(),
        answered_at: a.answered_at.map(|t| t.to_string()),
        answered_from: a.answered_from.clone(),
        delivery: a.delivery.clone(),
        delivered: a.delivery.as_ref().map(|d| d.reached_the_agent()),
        ended: a.ended.clone(),
    }
}

/// Everything an agent has asked, and what became of each one.
pub fn asks(snap: &Snapshot, recent: &[Ask]) -> AsksView {
    let settled: Vec<_> = recent.iter().filter(|a| !a.is_open()).collect();
    // Questions nobody answered from watched-only sessions: not answerable
    // `asks` rows, but *the agent gave up* is one of the outcomes.
    let mut abandoned: Vec<AbandonedAsk> = snap
        .world
        .runs()
        .flat_map(|r| {
            r.abandoned_questions.iter().map(move |q| AbandonedAsk {
                run: r.id.as_str().to_string(),
                project: r.project_id.as_ref().map(|p| p.as_str().to_string()),
                question: q.question.clone(),
                options: q.options.clone(),
                asked_at: q.asked_at.to_string(),
                abandoned_at: q.abandoned_at.to_string(),
                moved_on_to: q.moved_on_to.clone(),
                authority: "nobody".to_string(),
                outcome: "nobody answered: the agent asked and moved on".to_string(),
            })
        })
        .collect();
    abandoned.sort_by(|a, b| b.abandoned_at.cmp(&a.abandoned_at));
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
    AsksView {
        open: snap.open_asks.iter().map(render_ask).collect(),
        settled: settled.iter().map(|a| render_ask(a)).collect(),
        abandoned,
        blind_to: blind,
    }
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
) -> anyhow::Result<Vec<DecisionRow>> {
    let d = store.decisions(about, limit).await?;
    let d: Vec<crate::core::Decision> = match without_me {
        true => d
            .into_iter()
            .filter(|r| r.authority.was_taken_for_you())
            .collect(),
        false => d,
    };
    // Each run's reported edits are read once, only for runs whose decisions
    // name a call.
    let mut by_run: std::collections::HashMap<crate::core::RunId, (Vec<EditView>, bool)> =
        Default::default();
    for run in d
        .iter()
        .filter(|r| r.call_id.is_some())
        .filter_map(|r| r.run_id.clone())
    {
        if by_run.contains_key(&run) {
            continue;
        }
        let edits = store
            .edits_reported(&run)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| EditView::of(e.event))
            .collect();
        let kept = keeps_transcripts(store, &run).await;
        by_run.insert(run, (edits, kept));
    }
    Ok(d.into_iter()
        .map(|decision| {
            let (edits, kept) = match (&decision.call_id, &decision.run_id) {
                (Some(call), Some(run)) => {
                    let (all, kept) = by_run.get(run).cloned().unwrap_or((Vec::new(), true));
                    let mine: Vec<EditView> =
                        all.into_iter().filter(|e| &e.call_id == call).collect();
                    (Some(mine), kept)
                }
                _ => (None, true),
            };
            let no_edit = match &edits {
                Some(e) if e.is_empty() && !kept => {
                    Some("no diff was kept: this project keeps no transcripts".to_string())
                }
                Some(e) if e.is_empty() => Some("the agent sent no diff for this call".to_string()),
                _ => None,
            };
            DecisionRow {
                decision,
                edits,
                no_edit,
            }
        })
        .collect())
}

/// A decision as the ledger shows it: the row, and for a driven call the
/// edits its agent reported for that call — empty when it reported none or
/// none was kept, absent for anything that is not a driven call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRow {
    #[serde(flatten)]
    pub decision: crate::core::Decision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<EditView>>,
    /// Why a driven call's row shows no diff, in one of two sentences: the
    /// agent sent none, or the project keeps no transcripts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_edit: Option<String>,
}

/// Whether the project a run worked in keeps transcripts, read from the
/// directory its first event names; a run that cannot say reads as keeping
/// them (the default), so *no diff sent* is the claim made.
async fn keeps_transcripts(store: &crate::store::Store, run: &crate::core::RunId) -> bool {
    let first = store.events_for_run(run, 1).await.unwrap_or_default();
    let Some(crate::core::Event::SessionStarted { cwd, .. }) =
        first.into_iter().next().map(|e| e.event)
    else {
        return true;
    };
    let root = crate::repo::governing_root(&cwd).unwrap_or(cwd);
    crate::core::ProjectConfig::load(&root)
        .map(|c| c.agent.keep_transcripts)
        .unwrap_or(true)
}

/// One edit an agent reported for one call, as shown beside the decision
/// about the call: the agent's claim, labelled so, never evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditView {
    pub call_id: String,
    pub path: PathBuf,
    #[serde(default)]
    pub old_text: Option<String>,
    pub new_text: String,
    #[serde(default)]
    pub omitted_bytes: u64,
    pub says: String,
}

impl EditView {
    fn of(event: crate::core::Event) -> Option<Self> {
        match event {
            crate::core::Event::EditReported {
                call_id,
                path,
                old_text,
                new_text,
                omitted_bytes,
            } => Some(Self {
                call_id,
                path,
                old_text,
                new_text,
                omitted_bytes,
                says: crate::driven::EDIT_SAYS.into(),
            }),
            _ => None,
        }
    }
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

// ── Change ────────────────────────────────────────────────────────────────────

/// One change as every surface reads it: the host serialises it, and the
/// terminal reads it back as the same type. Borrowed where the host builds
/// it, owned where a client decodes it.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeView<'a> {
    #[serde(flatten)]
    pub change: std::borrow::Cow<'a, Change>,
    /// Where it is, computed from the record and the tree last seen.
    pub state: crate::core::ChangeState,
    pub glyph: String,
    /// What it is waiting on, as a phrase, when it is waiting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_says: Option<String>,
    /// Working in the person's own checkout, with the sentence naming what
    /// that gives up (parallel safety, a clean review base).
    pub in_place: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_place_says: Option<String>,
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
    pub claim_absent: Option<String>,
    /// The specification this change answers, as it is on disk now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<crate::spec::Plan>,
    /// Done, and the plan it answers is not. A sentence, never a verdict.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub plan_contradicts_done: bool,
    /// The plan moved under this change. `None` when unknowable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_drifted: Option<bool>,
    /// Ticked and verified counts. `None` without a readable, recognised
    /// specification; `counts_says` then carries the sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<crate::spec::Counts>,
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
    /// What the change's own diff weakened, by kind, beside its state.
    pub qualifier: crate::core::review::Qualifier,
    /// How many tests ran, and the base checks: beside `standing`, never in
    /// it. Absent until `check` has run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_checks: Option<crate::core::base_checks::View>,
    /// The open stop claim: what an agent said through the stop tool, quoted
    /// and attributed. Read after the weakened checks and before the gates,
    /// and never an input to `standing` or `state`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_claim: Option<ClaimView>,
    /// Every stop claim on the change, newest first, open or closed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claim_history: Vec<ClaimView>,
    /// The project keeps GSD's `.planning/` out of git, so this change's
    /// worktree lacks it: the sentence naming `.worktreeinclude`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planning_absent: Option<String>,
    /// What the current run says about its session's permission mode, never
    /// folded: *decides without you* is the sentence a reviewer most needs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_statement: Option<ModeStatementView>,
}

/// A stop claim as every surface shows it: the one label, the agent's words
/// as text, and who said it. Nothing here is named like a verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimView {
    pub id: String,
    /// Always *the agent says the spec and the tests conflict*.
    pub says: String,
    /// The agent's reason, control characters made visible. Text, never markup.
    pub reason: String,
    pub paths: Vec<String>,
    pub agent: String,
    pub run: String,
    pub at: jiff::Timestamp,
    /// *saas — echo, run acp-…, change c-…, 2026-09-27 10:02 UTC*.
    pub origin_says: String,
    /// The whole claim as quoted lines, for a terminal.
    pub quoted: String,
    /// How the run that claimed ended.
    pub ending: String,
    /// `open` or `closed`.
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_by: Option<crate::core::claim::ClosedBy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_says: Option<String>,
    /// Whether the claiming run's conversation can be resumed, so a person's
    /// own words can reach it.
    #[serde(default)]
    pub can_tell: bool,
}

impl ClaimView {
    pub fn of(c: &crate::core::claim::StopClaim) -> Self {
        use crate::core::claim::{ClaimState, visible};
        let (state, closed_by) = match &c.state {
            ClaimState::Open => ("open", None),
            ClaimState::Closed { by, .. } => ("closed", Some(*by)),
        };
        Self {
            id: c.id.to_string(),
            says: c.says().to_string(),
            reason: visible(&c.reason),
            paths: c.paths.iter().map(|p| visible(p)).collect(),
            agent: c.origin.agent().to_string(),
            run: c.origin.run().to_string(),
            at: c.origin.at(),
            origin_says: c.origin.says(),
            quoted: c.quoted(),
            ending: crate::core::claim::ENDING.to_string(),
            state: state.to_string(),
            closed_by,
            closed_says: closed_by.map(|b| b.says().to_string()),
            can_tell: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenRowView {
    #[serde(flatten)]
    pub row: crate::spec::TokenRow,
    pub says: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriftView {
    #[serde(flatten)]
    pub drift: crate::core::change::Drift,
    pub says: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunRowView {
    pub id: String,
    pub sent_says: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent: Option<Vec<crate::spec::SentTask>>,
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
    /// `checks` are the `check` commands the project declares now (`None`
    /// when its configuration could not be read): a pass by other commands is
    /// stale, in the state as in the standing, as on every other surface.
    pub fn of(change: &'a Change, can_retry: bool, checks: Option<&[String]>) -> Self {
        let now = change.tree_now.as_ref();
        let declared = crate::core::change::Declared::of(checks);
        let (state, standing) = (change.state(declared, now), change.verdict(declared, now));
        Self {
            state,
            glyph: state.glyph().to_string(),
            waiting_says: change.waiting.as_ref().map(crate::core::Waiting::says),
            in_place: change.in_place(),
            in_place_says: change.in_place_says().map(str::to_string),
            standing_says: standing.says(),
            standing,
            gate: change.check_report().map(|g| GateView {
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
            qualifier: crate::core::review::Qualifier::default(),
            base_checks: None,
            stop_claim: change.open_claim().map(ClaimView::of),
            claim_history: change.stop_claims.iter().rev().map(ClaimView::of).collect(),
            planning_absent: None,
            mode_statement: None,
            change: std::borrow::Cow::Borrowed(change),
        }
    }

    fn with_plan(mut self, plan: crate::spec::Plan) -> Self {
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
        trace: Option<crate::spec::Trace>,
        runs: &[&Run],
        declares_gates: bool,
    ) -> Self {
        let mine: Vec<&Run> = self
            .change
            .runs
            .iter()
            .filter_map(|id| runs.iter().copied().find(|r| r.id == *id))
            .collect();
        // A person's words reach a claiming run by resuming its conversation.
        let resumable = |run: &str| {
            mine.iter()
                .any(|r| r.id.as_str() == run && r.agent_session.is_some())
        };
        if let Some(c) = self.stop_claim.as_mut() {
            c.can_tell = resumable(&c.run);
        }
        let stale = matches!(
            self.standing,
            crate::core::change::Standing::Stale { .. }
                | crate::core::change::Standing::ChecksChanged { .. }
        );
        let last_pass = self.change.last_pass_at();
        // A layout with no task file (or a schema not read) counts nothing,
        // whatever box-like lines its files hold; its sentence says why.
        let unread = self.plan.as_ref().and_then(|p| p.no_progress.clone());
        match trace {
            _ if unread.is_some() => {
                self.counts_says = unread;
            }
            None => {
                self.counts_says = Some(
                    "the specification folder could not be read, so nothing is counted".into(),
                );
            }
            Some(t) if t.unrecognised => {
                self.counts_says = Some(crate::spec::UNRECOGNISED.into());
            }
            Some(t) => {
                let counts = crate::spec::counts(&t, &mine, declares_gates, last_pass);
                self.counts_says = Some(counts.says(stale));
                self.counts = Some(counts);
                self.token_rows = crate::spec::token_rows(&t, &mine, declares_gates, last_pass)
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

/// Diffs read for a list, keyed by checkout and base, each with the tree
/// digest the host last saw when it was read. Lists (the change list, the
/// inbox's ready rows, the plans) refetch on every refresh; a diff is read
/// again only once the digest moves. A claim — the offer's guard, the
/// certificate, the review itself — never reads from here.
type ShownSets = std::collections::HashMap<
    (PathBuf, String),
    (String, std::sync::Arc<crate::core::diff::ChangeSet>),
>;
static SHOWN_SETS: std::sync::LazyLock<std::sync::Mutex<ShownSets>> =
    std::sync::LazyLock::new(Default::default);
/// Past this many checkouts the cache starts again.
const MOST_SHOWN_SETS: usize = 256;

/// A change's diff for a list: from [`SHOWN_SETS`] while the change's tree
/// digest is the one it was read at, else read now (and kept, when the digest
/// is known — an unknown tree is never the same tree).
pub(crate) async fn shown_set(
    change: &Change,
    dir: &std::path::Path,
    base: &str,
) -> anyhow::Result<std::sync::Arc<crate::core::diff::ChangeSet>> {
    let tree = change.tree_now.as_ref().and_then(|t| t.tree.clone());
    let key = (dir.to_path_buf(), base.to_string());
    if let Some(tree) = &tree
        && let Ok(sets) = SHOWN_SETS.lock()
        && let Some((at, set)) = sets.get(&key)
        && at == tree
    {
        return Ok(set.clone());
    }
    let set = std::sync::Arc::new(crate::git::change_set(dir, base).await?);
    if let Some(tree) = tree
        && let Ok(mut sets) = SHOWN_SETS.lock()
    {
        if sets.len() >= MOST_SHOWN_SETS {
            sets.clear();
        }
        sets.insert(key, (tree, set.clone()));
    }
    Ok(set)
}

/// A change's diff against its base for a list (see [`shown_set`]), with the
/// configuration that orders it; `None` without a checkout on disk or when
/// the diff cannot be read (the board then shows no shape, never an empty one).
async fn read_set(
    snap: &Snapshot,
    w: &Change,
) -> Option<(
    std::sync::Arc<crate::core::diff::ChangeSet>,
    crate::core::ProjectConfig,
)> {
    let dir = w.worktree.as_deref().filter(|d| d.is_dir())?;
    let root = snap.world.project(&w.project_id).map(|p| p.root.clone());
    let config = root
        .as_deref()
        .and_then(|r| crate::core::ProjectConfig::load(r).ok())
        .unwrap_or_default();
    let base = base_of(root.as_deref(), dir).await;
    let set = shown_set(w, dir, &base)
        .await
        .inspect_err(|e| {
            tracing::warn!(dir = %dir.display(), error = %e, "the change set could not be read")
        })
        .ok()?;
    Some((set, config))
}

/// Fills a view's shape (when `shape`) and qualifier from one read of its diff.
async fn read_diff_facts(
    snap: &Snapshot,
    store: &crate::store::Store,
    v: &mut ChangeView<'_>,
    shape: bool,
) {
    let Some((set, config)) = read_set(snap, &v.change).await else {
        let on_disk = v.change.worktree.as_deref().is_some_and(|d| d.is_dir());
        if shape {
            v.shape_says = match on_disk {
                true => "the diff could not be read, so no shape",
                false => "no checkout on disk, so no shape",
            }
            .to_string();
        }
        if on_disk {
            v.qualifier = crate::core::review::Qualifier::unreadable();
        }
        return;
    };
    if shape {
        let ordered = crate::core::review::order(&set, config.review.roles.as_ref());
        v.shape_says = crate::core::review::shape(&set, &ordered).says();
    }
    let rows = weakened_for(&set, &config, &v.change);
    let seen = rows_read(store, v.change.id.as_str(), &rows, &set).await;
    v.qualifier = crate::core::review::Qualifier::of(&rows, &seen);
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
        // The qualifier is read for every change with a checkout: a green
        // without it would stand alone. The shape only for the newest few.
        let shape =
            shaped < SHAPED && v.change.archived_at.is_none() && v.change.completion.is_none();
        if shape {
            shaped += 1;
        }
        read_diff_facts(snap, store, v, shape).await;
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
            // A pass by commands the project no longer declares is not verified.
            let checks = config.as_ref().map(|c| c.gates.check.as_slice());
            let mut view = ChangeView::of(w, can_retry, checks);
            view.base_checks = crate::base_checks::view(w, config.as_ref());
            view.mode_statement = w
                .current_run()
                .and_then(|r| snap.world.run(r))
                .and_then(ModeStatementView::of);
            match (w.spec.as_deref(), roots.get(&w.project_id), config) {
                (Some(spec), Some(root), Some(config)) => {
                    // Boxes are read where the agent worked, as the gate is.
                    let worked_in = w.worktree.as_deref().unwrap_or(root);
                    let trace = crate::spec::ChangeFolder::at(worked_in, spec)
                        .ok()
                        .map(|f| config.spec.trace(&f));
                    let runs: Vec<&Run> = snap.world.runs().collect();
                    view.with_plan(crate::spec::Plan::read(
                        root,
                        spec,
                        &config.spec.open_questions,
                    ))
                    .with_edges(trace, &runs, config.has_gates())
                }
                _ => view,
            }
        })
        .collect();

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
                .map(|c| c.agent.keep_transcripts)
                .unwrap_or(true);
            view.claim_absent = Some(
                match keeps {
                    true => "nothing_said",
                    false => "transcripts_off",
                }
                .to_string(),
            );
        }
    }

    // A gitignored GSD planning folder is not carried into a worktree. Git is
    // asked only where the folder is missing from one.
    for (view, w) in views.iter_mut().zip(all.iter()) {
        let (Some(root), Some(tree)) = (
            roots.get(&w.project_id),
            w.worktree.as_deref().filter(|d| d.is_dir()),
        ) else {
            continue;
        };
        if crate::spec::planning_absent(root, tree, true).is_some()
            && crate::git::is_ignored(root, ".planning").await == Some(true)
        {
            view.planning_absent = crate::spec::planning_absent(root, tree, true);
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
    read_diff_facts(snap, store, &mut v, true).await;
    let mut out = serde_json::to_value(&v).unwrap_or(Value::Null);
    crate::draft::decorate(snap, &v.change, &mut out);
    Some(out)
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
    // worktree or with an unreadable diff, which the certificate states.
    let (weakened, unreadable, seen) = match read_rows_of(snap, change).await {
        None => (None, None, Default::default()),
        Some(Ok((rows, set))) => {
            // Every row read by a person before the offer, said as such; the
            // rows themselves are stated either way.
            let seen = rows_read(store, change.id.as_str(), &rows, &set).await;
            (Some(rows), None, seen)
        }
        Some(Err(e)) => (None, Some(format!("{e:#}")), Default::default()),
    };
    let all_seen = weakened.as_ref().is_some_and(|w| {
        !w.is_empty()
            && w.iter()
                .all(|r| seen.contains(&(r.path.clone(), r.matched.clone())))
    });
    let checks = snap
        .world
        .project(&change.project_id)
        .and_then(|p| crate::core::ProjectConfig::load(&p.root).ok())
        .map(|cfg| cfg.gates.check);
    let mut cert = crate::core::certificate::Certificate::of(
        change,
        crate::core::change::Declared::of(checks.as_deref()),
        claim.as_deref(),
    );
    cert.weakened = weakened;
    cert.weakened_unreadable = unreadable;
    cert.weakened_read = all_seen;
    // The same computation as the change's own view, so they cannot disagree.
    let config = snap
        .world
        .project(&change.project_id)
        .and_then(|p| crate::core::ProjectConfig::load(&p.root).ok());
    cert.base_checks = crate::base_checks::view(change, config.as_ref());
    // The statement's two edge reads: who decided this change's tool calls,
    // counted; and `devplane.toml` in the checked commit, when that commit
    // is the tree that was checked.
    cert.decided_by = store.authority_counts(&change.id, &change.runs).await.ok();
    if let Some(sha) = cert
        .evidence
        .and_then(|r| r.commit.as_ref())
        .filter(|c| c.clean)
        .and_then(|c| c.commit.clone())
    {
        let dir = change.worktree.clone().filter(|w| w.exists()).or_else(|| {
            snap.world
                .project(&change.project_id)
                .map(|p| p.root.clone())
        });
        if let Some(dir) = dir {
            cert.config_blob = crate::git::blob_at(&dir, &sha, "devplane.toml").await;
        }
    }
    Some(json!({
        "finished": cert.is_finished(),
        "markdown": cert.markdown_bounded(),
        "statement": cert.json(),
        "page": cert.page(),
    }))
}

// ── Review ──────────────────────────────────────────────────────────────────

/// Every command the declared gates run, for the files they name.
pub fn gate_commands(config: &crate::core::ProjectConfig) -> Vec<String> {
    config
        .gates
        .check
        .iter()
        .chain(config.gates.named.values().flat_map(|g| g.run.iter()))
        .cloned()
        .collect()
}

/// The checks a change set weakened, under a project's configuration: the one
/// computation behind the review's first group, the qualifier, the
/// certificate and the offer's guard.
pub fn weakened_by(
    set: &crate::core::diff::ChangeSet,
    config: &crate::core::ProjectConfig,
) -> Vec<crate::core::review::Weakened> {
    crate::core::review::weakened(set, config.review.roles.as_ref(), &gate_commands(config))
}

/// A change's weakened rows: its diff's, then the base tests that fail on it
/// from a **current** base-checks result (a stale one adds none). The one
/// list the qualifier, the review's first group, the inbox row, the
/// certificate and the offer's refusal all read.
pub fn weakened_for(
    set: &crate::core::diff::ChangeSet,
    config: &crate::core::ProjectConfig,
    change: &Change,
) -> Vec<crate::core::review::Weakened> {
    // Observed failures lead: they are what the diff's patterns could miss.
    let mut rows = crate::core::base_checks::current_rows(change, config);
    rows.extend(weakened_by(set, config));
    rows
}

/// The same, read from a checkout now against its base, with the change set
/// the rows were read from, for what reads the host's seen marks against it.
/// An unreadable diff is an error, never "nothing weakened"; so is an
/// unreadable `devplane.toml`, whose gates name files the defaults do not.
pub async fn read_rows_in(
    root: Option<&std::path::Path>,
    dir: &std::path::Path,
    change: Option<&Change>,
) -> anyhow::Result<(
    Vec<crate::core::review::Weakened>,
    crate::core::diff::ChangeSet,
)> {
    let config = match root {
        Some(r) => crate::core::ProjectConfig::load(r).map_err(|e| {
            anyhow::anyhow!("configuration unreadable, so what the gates run is unknown: {e}")
        })?,
        None => crate::core::ProjectConfig::default(),
    };
    let base = base_of(root, dir).await;
    let set = crate::git::change_set(dir, &base)
        .await
        .map_err(|e| anyhow::anyhow!("the change's diff could not be read: {e:#}"))?;
    let rows = match change {
        Some(c) => weakened_for(&set, &config, c),
        None => weakened_by(&set, &config),
    };
    Ok((rows, set))
}

/// A change's weakened rows with the change set they were read from, or
/// `None` without a checkout on disk.
pub async fn read_rows_of(
    snap: &Snapshot,
    change: &Change,
) -> Option<
    anyhow::Result<(
        Vec<crate::core::review::Weakened>,
        crate::core::diff::ChangeSet,
    )>,
> {
    let dir = change.worktree.as_deref().filter(|d| d.is_dir())?;
    let root = snap
        .world
        .project(&change.project_id)
        .map(|p| p.root.clone());
    Some(read_rows_in(root.as_deref(), dir, Some(change)).await)
}

/// The weakened rows a person has read, from the host's seen marks: a row is
/// read when every hunk it was found in is marked seen.
pub async fn rows_read(
    store: &crate::store::Store,
    change: &str,
    rows: &[crate::core::review::Weakened],
    set: &crate::core::diff::ChangeSet,
) -> std::collections::HashSet<(String, String)> {
    let seen = store.seen(change).await.unwrap_or_default();
    crate::core::review::rows_seen(rows, set, &seen)
}

/// A change's qualifier: its weakened rows by kind, and how many are unseen.
/// Read for a list, so the diff comes from [`shown_set`].
pub async fn qualifier_of(
    snap: &Snapshot,
    store: &crate::store::Store,
    change: &Change,
) -> crate::core::review::Qualifier {
    if !change.worktree.as_deref().is_some_and(|d| d.is_dir()) {
        return crate::core::review::Qualifier::default();
    }
    let Some((set, config)) = read_set(snap, change).await else {
        return crate::core::review::Qualifier::unreadable();
    };
    let rows = weakened_for(&set, &config, change);
    let seen = rows_read(store, change.id.as_str(), &rows, &set).await;
    crate::core::review::Qualifier::of(&rows, &seen)
}

/// Why marking a unit seen was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeenRefusal {
    NoSuchChange,
    /// Nothing at that path has that digest (or the path has nothing); carries
    /// the digests that are there.
    NoSuchUnit(Vec<String>),
}

/// What one mark covered: the units marked, and the weakened rows it
/// completed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Marked {
    pub seen: Vec<SeenUnit>,
    pub weakened_seen: Vec<crate::core::review::Weakened>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeenUnit {
    pub path: String,
    pub digest: String,
}

/// Marks one unit at `path` seen as the person's — the hunk whose digest is
/// `digest` — or, without one, every unit at `path` (its hunks, and the rows
/// read there). Never marks a unit that is not in the change now.
pub async fn mark_seen(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &crate::core::ChangeId,
    path: &str,
    digest: Option<&str>,
) -> Result<Marked, SeenRefusal> {
    let change = snap.change(id).ok_or(SeenRefusal::NoSuchChange)?;
    let (rows, set) = match read_rows_of(snap, change).await {
        Some(Ok(x)) => x,
        _ => return Err(SeenRefusal::NoSuchUnit(Vec::new())),
    };
    let there = crate::core::review::units_at(path, &set, &rows);
    let hit: Vec<String> = match digest {
        Some(d) => there.iter().filter(|x| *x == d).cloned().collect(),
        None => there.clone(),
    };
    if hit.is_empty() {
        return Err(SeenRefusal::NoSuchUnit(there));
    }
    let before = rows_read(store, id.as_str(), &rows, &set).await;
    for d in &hit {
        if let Err(e) = store.mark_seen(id.as_str(), path, d).await {
            tracing::warn!(error = %e, "a seen mark was not written");
        }
    }
    let after = rows_read(store, id.as_str(), &rows, &set).await;
    Ok(Marked {
        seen: hit
            .into_iter()
            .map(|digest| SeenUnit {
                path: path.to_string(),
                digest,
            })
            .collect(),
        weakened_seen: rows
            .into_iter()
            .filter(|w| {
                let k = (w.path.clone(), w.matched.clone());
                after.contains(&k) && !before.contains(&k)
            })
            .collect(),
    })
}

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

/// Why a review could not be composed: unknown change, checkout gone, or a diff git would not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoReview {
    NoSuchChange,
    NoCheckout,
    /// The checkout is there, and git would not diff it.
    Unreadable,
}

impl NoReview {
    pub fn says(self) -> &'static str {
        match self {
            NoReview::NoSuchChange => "no such change",
            NoReview::NoCheckout => {
                "this change has no checkout on disk, so there is nothing to read"
            }
            NoReview::Unreadable => "the change's diff could not be read",
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
    /// The first group's rows, counted by kind, with how many are unseen.
    pub qualifier: crate::core::review::Qualifier,
    /// Every comment on the change, oldest first, with where its line is now.
    #[serde(default)]
    pub comments: Vec<CommentView>,
    /// The files none of whose hunks a person marked seen, by path, in
    /// reading order — never a share of the change.
    #[serde(default)]
    pub not_opened: Vec<String>,
    #[serde(default)]
    pub not_opened_says: Option<String>,
}

/// A comment as the review shows it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentView {
    #[serde(flatten)]
    pub comment: crate::core::comments::ReviewComment,
    pub anchor: crate::core::comments::Anchor,
    /// For a sent comment once its run has finished a turn since: whether
    /// the hunk it was written against changed. `None` until then.
    pub hunk_changed: Option<bool>,
}

/// A weakened row as the review shows it: with whether a person marked it seen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeakenedRow {
    #[serde(flatten)]
    pub row: crate::core::review::Weakened,
    pub seen: bool,
    /// The gate that runs what this row weakened: the one whose command
    /// names its path, else the declared `check`. `None` when no gate is
    /// declared.
    #[serde(default)]
    pub gate_says: Option<String>,
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
    pub weakened: Vec<WeakenedRow>,
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
    /// A file with no hunks is one unit to mark: whether it is marked.
    pub unit_seen: Option<bool>,
    /// A hunk here was marked seen, and the agent has since rewritten it.
    pub changed_since_seen: bool,
    /// What to run to see each fact on this row for yourself.
    pub marker_commands: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewHunk {
    pub header: String,
    pub lines: Vec<(crate::core::diff::Kind, String)>,
    /// Whitespace only: collapsed by default, counted, expandable.
    pub formatter_only: bool,
    /// The hunk's content digest: what a seen mark and a comment are keyed by.
    pub digest: String,
    /// A person marked this hunk seen (in the host, for every surface).
    pub seen: bool,
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
    pub tasks: Vec<crate::spec::SentTask>,
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

/// A change's comments, each with where its line is now and — once the run
/// it was sent to has finished a turn since — whether its hunk changed.
async fn comment_views(
    snap: &Snapshot,
    store: &crate::store::Store,
    change: &Change,
    set: &crate::core::diff::ChangeSet,
) -> Vec<CommentView> {
    use crate::core::comments;
    use crate::core::run::RunState;
    let rows = store.comments(change.id.as_str()).await.unwrap_or_default();
    rows.into_iter()
        .map(|c| {
            let turned = match (&c.sent_at, &c.run_id) {
                (Some(at), Some(run)) => at.parse::<jiff::Timestamp>().ok().is_some_and(|at| {
                    snap.world
                        .run(&crate::core::RunId::new(run.clone()))
                        .is_some_and(|r| {
                            !matches!(r.state, RunState::Starting | RunState::Working)
                                && r.last_activity_at > at
                        })
                }),
                _ => false,
            };
            CommentView {
                anchor: comments::anchor(&c, set),
                hunk_changed: turned.then(|| !comments::hunk_still_there(&c, set)),
                comment: c,
            }
        })
        .collect()
}

/// The review of one change: one `git diff`, one standing read, then a fold
/// over `[review]`, the change's runs and files, and its decisions.
pub async fn review(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &crate::core::ChangeId,
) -> Result<ReviewView, NoReview> {
    review_read(snap, store, id, false).await
}

/// [`review`]; `for_list` reads the diff through [`shown_set`] and takes the
/// tree the host last saw, so an inbox refresh runs no git while the change's
/// tree digest stands still.
async fn review_read(
    snap: &Snapshot,
    store: &crate::store::Store,
    id: &crate::core::ChangeId,
    for_list: bool,
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
    let unreadable = |e: anyhow::Error| {
        tracing::warn!(dir = %dir.display(), error = %e, "the change set could not be read");
        NoReview::Unreadable
    };
    let (set, now) = match for_list {
        true => (
            shown_set(change, &dir, &base).await.map_err(unreadable)?,
            change.tree_now.clone(),
        ),
        false => (
            std::sync::Arc::new(
                crate::git::change_set(&dir, &base)
                    .await
                    .map_err(unreadable)?,
            ),
            crate::git::commit_stamp(&dir).await,
        ),
    };
    let standing = change.verdict(
        crate::core::change::Declared::Checks(&config.gates.check),
        now.as_ref(),
    );

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
    let marks = store.seen(change.id.as_str()).await.unwrap_or_default();
    let ordered = review::order(&set, roles);
    let shape = review::shape(&set, &ordered);
    let intent = review::by_intent(&set, &wrote);
    // A draft asks for one thing, its folder: the rest is *not asked for*.
    let intent = match change.draft.as_ref() {
        Some(d) if d.asks_for_folder() => match d.run.as_ref() {
            Some(run) => {
                crate::core::draft::asked_for_folder(intent, &set, run, &d.folder, &d.sent)
            }
            None => intent,
        },
        _ => intent,
    };

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
    // The gate that runs a weakened file: named where a command names it,
    // else the `check` gate, which runs the project's tests.
    let ran_by = |path: &str| -> Option<String> {
        let named = config
            .gates
            .check
            .iter()
            .map(|c| ("check", c))
            .chain(
                config
                    .gates
                    .named
                    .iter()
                    .flat_map(|(n, g)| g.run.iter().map(move |c| (n.as_str(), c))),
            )
            .find(|(_, c)| c.contains(path));
        match named {
            Some((gate, cmd)) => Some(format!("the `{gate}` gate runs it: `{cmd}`")),
            None => (!config.gates.check.is_empty()).then(|| {
                format!(
                    "the `check` gate ran over it: `{}`",
                    config.gates.check.join("` · `")
                )
            }),
        }
    };
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
                    .map(|h| {
                        let digest = review::hunk_digest(h);
                        ReviewHunk {
                            header: h.header.clone(),
                            lines: h.lines.clone(),
                            formatter_only: review::formatter_only(&f.path, h),
                            seen: marks.contains(&(f.path.clone(), digest.clone())),
                            digest,
                        }
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
        let units: Vec<String> = set
            .uncut()
            .files
            .iter()
            .filter(|u| u.path == f.path)
            .flat_map(review::file_units)
            .collect();
        let unit_seen = hunks.is_empty().then(|| {
            units
                .iter()
                .all(|d| marks.contains(&(f.path.clone(), d.clone())))
        });
        let changed_since_seen = marks
            .iter()
            .any(|(p, d)| *p == f.path && !units.contains(d));
        ReviewFile {
            unit_seen,
            changed_since_seen,
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
    let weak = weakened_for(&set, &config, change);
    let seen = review::rows_seen(&weak, &set, &marks);
    let qualifier = review::Qualifier::of(&weak, &seen);
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
            weakened: weak
                .into_iter()
                .map(|row| WeakenedRow {
                    seen: seen.contains(&(row.path.clone(), row.matched.clone())),
                    gate_says: ran_by(&row.path),
                    row,
                })
                .collect(),
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

    let reading: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.files.iter().map(|f| f.path.as_str()))
        .collect();
    let not_opened = review::not_opened(reading.iter().copied(), &set, &marks);
    let comments = match for_list {
        true => Vec::new(),
        false => comment_views(snap, store, change, &set).await,
    };
    let collapsed = shape.formatter_only;
    Ok(ReviewView {
        not_opened_says: review::not_opened_says(&not_opened),
        not_opened,
        comments,
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
        qualifier,
        base,
    })
}

// ── Specs, projects, setup, rules ───────────────────────────────────────────

/// The plan each project is working to: one row per in-flight change with its
/// specification as on disk now. A plan belongs to a change, not a project.
pub async fn specs(snap: &Snapshot, store: &crate::store::Store) -> Value {
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

    // What each working change's diff weakened, beside its state.
    let mut qualifiers: BTreeMap<String, crate::core::review::Qualifier> = Default::default();
    for w in worked.values() {
        qualifiers.insert(w.id.to_string(), qualifier_of(snap, store, w).await);
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
        // The newest folders and every one a change is working on, rather than
        // the oldest: folder names carry a number or a date that rises.
        let working: Vec<String> = worked
            .keys()
            .filter(|(pid, _)| pid == id)
            .map(|(_, spec)| spec.clone())
            .collect();
        let rest: Vec<String> = paths
            .iter()
            .filter(|p| !working.contains(p))
            .cloned()
            .collect();
        let room = MOST_PLANS.saturating_sub(working.len());
        let newest: Vec<String> = rest[rest.len().saturating_sub(room)..].to_vec();
        let mut paths: Vec<String> = working.into_iter().chain(newest).collect();
        paths.sort();
        let cfg = crate::core::ProjectConfig::load(root).ok();
        for spec in paths.into_iter().take(MOST_PLANS) {
            let plan = crate::spec::Plan::read(root, &spec, m);
            let w = worked.get(&(id.clone(), spec.clone()));
            // The edges: a requirement token in a heading and a task line.
            // Orphans are named; with no tokens at all, the notation is
            // reported as unrecognised, not as missing coverage.
            let trace = cfg.as_ref().and_then(|c| {
                crate::spec::ChangeFolder::at(root, &spec)
                    .ok()
                    .map(|change| c.spec.trace(&change))
            });
            // The two counts, over the working change's own runs.
            let (counts, token_rows) = match (w, &trace) {
                (Some(w), Some(t)) if !t.unrecognised && plan.no_progress.is_none() => {
                    let mine: Vec<&Run> =
                        w.runs.iter().filter_map(|id| snap.world.run(id)).collect();
                    let gates = cfg.as_ref().is_some_and(|c| c.has_gates());
                    let last_pass = w.last_pass_at();
                    (
                        Some(crate::spec::counts(t, &mine, gates, last_pass)),
                        crate::spec::token_rows(t, &mine, gates, last_pass)
                            .into_iter()
                            .map(|row| {
                                json!({
                                    "token": row.token,
                                    "tasks": row.tasks,
                                    "ticked": row.ticked,
                                    "seen_by_pass": row.seen_by_pass,
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
                "state": w.map(|w| {
                    let declared = crate::core::change::Declared::of(
                        cfg.as_ref().map(|c| c.gates.check.as_slice()),
                    );
                    w.state(declared, w.tree_now.as_ref())
                }),
                "qualifier": w.and_then(|w| qualifiers.get(w.id.as_str())),
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
                    .then_some(crate::spec::NO_LAYOUT),
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
                "default_agent": cfg.as_ref().and_then(|c| c.agent.default.clone()),
                // Whether a change here can become verified at all.
                "declares_check": cfg.as_ref().is_some_and(|c| c.has_gates()),
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
        "connect": connect,
        "gate": { "syntax_modelled_on": crate::core::policy::SYNTAX_MODELLED_ON },
        "machine_policy": machine_policy,
        "projects": projects,
        "agents": snap.agents,
    })
}

// ── Diagnostics ─────────────────────────────────────────────────────────────

/// `/api/diagnostics`: what only a running host knows about its own health,
/// read back by `devplane doctor` as this same type.
#[derive(Debug, Serialize, Deserialize)]
pub struct Diagnostics {
    pub version: String,
    pub gate: DiagnosticsGate,
    /// Live sessions whose environment Devplane could read — only the
    /// `SessionStart` hook can, so this is the question-clock coverage.
    pub modes: DiagnosticsModes,
    /// Agents a previous host started that are still running, unreachable.
    pub leaked_agents: Vec<LeakedAgent>,
    /// What each inbox kind raised in the last week and what became of it.
    pub attention: Option<Value>,
    pub pid: u32,
    pub started_at: String,
    pub uptime_seconds: i64,
    pub summary: crate::core::BoardSummary,
    pub channels: Vec<crate::store::ChannelHealth>,
    /// The OpenCode subscription; absent where it was not asked for.
    pub opencode: Option<OpenCodeHealth>,
    pub forge: DiagnosticsForge,
    pub stall_seconds: i64,
    /// Projects whose `devplane.toml` will not load, so their rules are gone.
    pub unreadable_configs: Vec<UnreadableConfig>,
    /// Stored rows this build cannot decode.
    pub unreadable_rows: Vec<UnreadableRow>,
    /// How much of the store never arrived.
    pub unwritten: Unwritten,
    /// Claude Code's own auto-mode classifier, read back, never written.
    pub auto_mode: AutoModeView,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosticsGate {
    pub syntax_modelled_on: String,
    pub sessions_reporting_a_version: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosticsModes {
    pub clock_read: usize,
    pub clock_unread: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LeakedAgent {
    pub pid: u32,
    pub command: String,
    pub worktree: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenCodeHealth {
    pub server: String,
    pub says: String,
    pub live: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosticsForge {
    pub viewer: Option<String>,
    pub error: Option<String>,
    /// Per GitHub host: signed in as whom, and when GitHub last answered.
    /// Never the token.
    pub github: Vec<crate::github::HostView>,
    pub last_poll_at: Option<String>,
    pub projects: usize,
    pub skipped: usize,
    /// Which projects were ruled out, and why.
    pub skipped_projects: Vec<SkippedProject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SkippedProject {
    pub project: String,
    pub reason: String,
    pub at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnreadableConfig {
    pub project: String,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnreadableRow {
    pub row: String,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Unwritten {
    pub events: u64,
    pub decisions: u64,
    pub last_reason: Option<String>,
}

/// The classifier's configuration counted, or why it could not be read.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutoModeView {
    Unavailable {
        unavailable: crate::observe::automode::Unavailable,
    },
    Read {
        configured: bool,
        environment: usize,
        allow: usize,
        soft_deny: usize,
        hard_deny: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::change::{CommandResult, CommitStamp, GateReport, Reach, Standing};

    fn stamp(tree: &str, clean: bool, changed: u32) -> CommitStamp {
        CommitStamp {
            commit: Some(format!("commit-of-{tree}")),
            tree: Some(tree.into()),
            tree_unknown: None,
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
            junit: Vec::new(),
            base_commit: None,
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

    /// The terminal reads a change as the very type the host wrote: what is
    /// serialised reads back, field for field.
    #[test]
    fn a_change_view_reads_back_as_the_type_the_host_serialised() {
        let mut c = passed_at("aaaaaaa111");
        c.tree_now = Some(stamp("aaaaaaa111", true, 0));
        let v = ChangeView::of(&c, false, Some(&["cargo test".to_string()]));
        let wire = serde_json::to_value(&v).unwrap();
        let back: ChangeView<'static> = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(back.change.as_ref(), &c);
        assert_eq!(back.state, v.state);
        assert_eq!(back.standing, v.standing);
        assert_eq!(serde_json::to_value(&back).unwrap(), wire);
    }

    /// A stale pass reaches the surface as stale, naming both trees.
    #[test]
    fn a_pass_the_tree_moved_under_is_served_as_stale_with_both_digests() {
        let mut c = passed_at("aaaaaaa111");
        c.tree_now = Some(stamp("aaaaaaa111", true, 0));
        let v = ChangeView::of(&c, false, Some(&["cargo test".to_string()]));
        assert_eq!(v.state, crate::core::ChangeState::Verified);
        assert_eq!(v.glyph, "✓");
        assert_eq!(v.standing, Standing::Verified);

        // The branch moved on: a different tree.
        c.tree_now = Some(stamp("bbbbbbb222", true, 0));
        let v = ChangeView::of(&c, false, Some(&["cargo test".to_string()]));
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
        let v = ChangeView::of(&c, false, Some(&["cargo test".to_string()]));
        assert_eq!(v.state, crate::core::ChangeState::Verified);

        // A touched file changes the working-tree digest.
        c.tree_now = Some(stamp("ccccccc333", false, 1));
        let v = ChangeView::of(&c, false, Some(&["cargo test".to_string()]));
        assert_ne!(v.state, crate::core::ChangeState::Verified);
        assert!(
            v.standing_says.contains("changed after"),
            "{}",
            v.standing_says
        );

        // Unknown is not unchanged.
        c.tree_now = None;
        let v = ChangeView::of(&c, false, Some(&["cargo test".to_string()]));
        assert_ne!(v.state, crate::core::ChangeState::Verified);
    }

    /// A project with no gates never verifies, and says so in words.
    #[test]
    fn no_gates_declared_is_a_sentence_and_never_verified() {
        let mut c = passed_at("aaaaaaa111");
        c.tree_now = Some(stamp("aaaaaaa111", true, 0));
        let v = ChangeView::of(&c, false, Some(&[]));
        assert_eq!(v.standing, Standing::NoGatesDeclared);
        assert_eq!(v.standing.word(), "no gates declared");
        assert!(v.standing_says.starts_with("no gates declared"));
        // The state is the same verdict: a deleted `check` does not keep the
        // green an earlier pass left.
        assert_eq!(v.state, crate::core::ChangeState::InFlight);

        // Nor does a configuration nobody could read.
        let v = ChangeView::of(&c, false, None);
        assert_eq!(v.standing, Standing::ConfigUnreadable);
        assert_ne!(v.state, crate::core::ChangeState::Verified);
    }

    /// A list reads a change's diff once per tree digest: a refresh with the
    /// digest unchanged runs no git, and a moved digest reads it again.
    #[tokio::test]
    async fn a_list_reads_a_diff_again_only_when_the_tree_moves() {
        let dir = std::env::temp_dir().join(format!(
            "devplane-shown-set-{}-{}",
            std::process::id(),
            jiff::Timestamp::now().as_nanosecond()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&dir)
                    .status()
                    .unwrap()
                    .success()
            );
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-qm", "init"]);
        std::fs::write(dir.join("a.txt"), "b\n").unwrap();

        let mut c = passed_at("t1");
        c.worktree = Some(dir.clone());
        c.tree_now = Some(stamp("t1", false, 1));
        let first = shown_set(&c, &dir, "main").await.unwrap();
        std::fs::write(dir.join("b.txt"), "new\n").unwrap();
        let again = shown_set(&c, &dir, "main").await.unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&first, &again),
            "read twice at one digest"
        );

        c.tree_now = Some(stamp("t2", false, 2));
        let moved = shown_set(&c, &dir, "main").await.unwrap();
        assert_eq!(moved.files.len(), 2, "{moved:?}");
        // An unknown tree is never the same tree.
        c.tree_now = None;
        let unknown = shown_set(&c, &dir, "main").await.unwrap();
        assert!(!std::sync::Arc::ptr_eq(&moved, &unknown));
        std::fs::remove_dir_all(&dir).ok();
    }
}

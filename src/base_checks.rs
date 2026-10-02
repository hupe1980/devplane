//! Base checks, the host's half: one throwaway detached checkout at the
//! change's merge base, inside the vendor's worktree folder, and the
//! project's own `check` run in it twice —
//!
//! 1. **the base's tests on this change**: the verified tree, materialised
//!    from git objects, with every file that defines the checks put back as
//!    the base has it;
//! 2. **this change's new tests on the base**: the base, fully cleaned, with
//!    the change's test files placed on it.
//!
//! Every decision (which paths, what the outcomes mean, every sentence) is
//! the pure half's, `core::base_checks`. Only commands the governing
//! `devplane.toml` declares run, nothing is written into the change's
//! checkout or the person's, and the throwaway checkout is removed when the
//! pair ends, or at the next host start if the host stopped first.
//!
//! Nothing runs after `check` by default. A pair runs when a person offers
//! or asks, or after every passing `check` when the project set
//! `[gates] base_checks = "auto"`, and never on the verdict's path.

use crate::core::ProjectConfig;
use crate::core::base_checks::{self as pure, Absent, BaseChecks, Key, OnBase, OnChange, Run};
use crate::core::change::GateReport;
use crate::core::config::BaseChecksWhen;
use crate::core::ids::ChangeId;
use crate::host::Shared;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Every throwaway checkout's name starts with this, under
/// `<root>/.claude/worktrees/`; [`sweep`] removes what a stopped host left.
pub const PREFIX: &str = ".devplane-base-";

type Flights = std::collections::HashMap<String, tokio::sync::watch::Receiver<bool>>;

/// Pairs in flight, by change: a second trigger waits for the first.
static RUNNING: std::sync::LazyLock<std::sync::Mutex<Flights>> =
    std::sync::LazyLock::new(Default::default);

/// One lane per repository: at most one pair runs per project at a time.
static LANES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::LazyLock::new(Default::default);

/// Why the last attempt for a change stopped before a result existed, by
/// change; cleared when a pair starts or a result is current.
static DID_NOT_RUN: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(Default::default);

/// Changes whose offer is waiting on their pair now.
static OFFERS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(Default::default);

/// Whether a pair for this change is running now.
pub fn is_running(id: &ChangeId) -> bool {
    RUNNING
        .lock()
        .map(|m| m.contains_key(id.as_str()))
        .unwrap_or(false)
}

/// Why the last attempt for this change stopped before any result, if it did.
pub fn did_not_run(id: &ChangeId) -> Option<String> {
    DID_NOT_RUN
        .lock()
        .ok()
        .and_then(|m| m.get(id.as_str()).cloned())
}

/// Says on the change that base checks did not run, and why.
fn note_failure(state: &Shared, id: &ChangeId, why: String) {
    tracing::warn!(change = %id, error = %why, "base checks did not run");
    if let Ok(mut m) = DID_NOT_RUN.lock() {
        m.insert(id.as_str().to_string(), why);
    }
    state.notify_changed();
}

fn clear_failure(id: &ChangeId) {
    if let Ok(mut m) = DID_NOT_RUN.lock() {
        m.remove(id.as_str());
    }
}

/// Whether this change's offer is waiting on its pair now.
pub fn offer_waits(id: &ChangeId) -> bool {
    OFFERS
        .lock()
        .map(|s| s.contains(id.as_str()))
        .unwrap_or(false)
}

/// A change's base checks as every surface shows them: the shared
/// composition, with what only the host knows — an offer waiting on the pair,
/// and why the last attempt stopped before a result.
pub fn view(
    change: &crate::core::Change,
    config: Option<&ProjectConfig>,
) -> Option<crate::core::base_checks::View> {
    crate::core::base_checks::View::of(change, config, is_running(&change.id))
        .map(|v| v.with_host(offer_waits(&change.id), did_not_run(&change.id)))
}

/// Takes an offer off [`OFFERS`] however it ends.
struct Offering {
    id: ChangeId,
    state: Shared,
}

impl Drop for Offering {
    fn drop(&mut self) {
        if let Ok(mut s) = OFFERS.lock() {
            s.remove(self.id.as_str());
        }
        self.state.notify_changed();
    }
}

/// Before an offer: the pair, once per tree, waited for, with the change
/// saying *running base checks first* meanwhile. Anything that stops them is
/// said on the change and never stops the offer.
pub async fn before_offer(state: &Shared, id: &ChangeId) {
    if let Ok(mut s) = OFFERS.lock() {
        s.insert(id.as_str().to_string());
    }
    let _offering = Offering {
        id: id.clone(),
        state: state.clone(),
    };
    state.notify_changed();
    if let Err(e) = ensure(state, id).await {
        note_failure(state, id, format!("{e:#}"));
    }
}

fn lane(root: &Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    LANES
        .lock()
        .map(|mut m| m.entry(root.to_path_buf()).or_default().clone())
        .unwrap_or_default()
}

/// What asking for base checks came to.
#[derive(Debug)]
pub enum Started {
    /// A result for the tree, base and definition now exists: nothing runs.
    Current,
    /// A pair is running for this change, started now or already.
    Running(tokio::sync::watch::Receiver<bool>),
    /// No pair may run, and why.
    Refused(Absent),
}

/// Ends a flight however the pair ends: done, failed, or cancelled.
struct Flight {
    id: ChangeId,
    tx: tokio::sync::watch::Sender<bool>,
    state: Shared,
}

impl Drop for Flight {
    fn drop(&mut self) {
        if let Ok(mut m) = RUNNING.lock() {
            m.remove(self.id.as_str());
        }
        let _ = self.tx.send(true);
        self.state.notify_changed();
    }
}

/// Starts the pair for a change's current tree, joins the one running, or
/// says why none may run. Returns at once; [`ensure`] waits.
pub async fn start(state: &Shared, id: &ChangeId) -> Result<Started> {
    if let Some(rx) = RUNNING
        .lock()
        .ok()
        .and_then(|m| m.get(id.as_str()).cloned())
    {
        return Ok(Started::Running(rx));
    }
    crate::change::refresh_tree(state, id).await;
    let (root, config) = crate::change::governing_config(state, id).await?;
    let change = {
        let changes = state.changes.lock().await;
        changes.get(id).cloned().context("no such change")?
    };
    let key = match pure::key_now(&change, &config) {
        Ok(k) => k,
        Err(a) => return Ok(Started::Refused(a)),
    };
    if change.base_checks.as_ref().is_some_and(|b| b.key == key) {
        clear_failure(id);
        return Ok(Started::Current);
    }
    let (tx, rx) = tokio::sync::watch::channel(false);
    {
        let mut m = RUNNING
            .lock()
            .map_err(|_| anyhow::anyhow!("the base-checks registry is poisoned"))?;
        if let Some(rx) = m.get(id.as_str()) {
            return Ok(Started::Running(rx.clone()));
        }
        m.insert(id.as_str().to_string(), rx.clone());
    }
    clear_failure(id);
    let flight = Flight {
        id: id.clone(),
        tx,
        state: state.clone(),
    };
    state.notify_changed();
    let dir = change.worktree.clone().unwrap_or_else(|| root.clone());
    let check = change.check_report().cloned();
    let state = state.clone();
    tokio::spawn(async move {
        let flight = flight;
        let lane = lane(&root);
        let _held = lane.lock().await;
        // In its own task, so a pair that panics is said on the change
        // rather than lost with the task.
        let pair =
            tokio::spawn(async move { run(&root, &dir, &key, &config, check.as_ref()).await });
        let result = match pair.await {
            Ok(r) => r,
            Err(e) => {
                note_failure(&state, &flight.id, format!("the run stopped: {e}"));
                return;
            }
        };
        let saved = {
            let mut changes = state.changes.lock().await;
            changes.get_mut(&flight.id).map(|c| {
                c.base_checks = Some(result);
                c.clone()
            })
        };
        if let Some(c) = saved {
            crate::change::persist(&state, &c).await;
        }
        drop(flight);
    });
    Ok(Started::Running(rx))
}

/// Runs the pair when needed and waits for it; `Some` says why none ran.
pub async fn ensure(state: &Shared, id: &ChangeId) -> Result<Option<Absent>> {
    match start(state, id).await? {
        Started::Current => Ok(None),
        Started::Refused(a) => Ok(Some(a)),
        Started::Running(mut rx) => {
            while !*rx.borrow_and_update() {
                if rx.changed().await.is_err() {
                    break;
                }
            }
            Ok(None)
        }
    }
}

/// After a `check`: in auto mode a passing one starts base checks in the
/// background. Never awaited on the verdict's path, and nothing from them
/// reaches the agent.
pub fn after_check(state: &Shared, id: &ChangeId, config: &ProjectConfig, passed: bool) {
    if !passed || config.gates.base_checks != BaseChecksWhen::Auto {
        return;
    }
    let (state, id) = (state.clone(), id.clone());
    tokio::spawn(async move {
        if let Err(e) = start(&state, &id).await {
            note_failure(&state, &id, format!("{e:#}"));
        }
    });
}

/// The throwaway checkout, removed however the pair ends.
struct Checkout {
    root: PathBuf,
    dir: PathBuf,
}

impl Drop for Checkout {
    fn drop(&mut self) {
        remove_now(&self.root, &self.dir);
    }
}

/// Removes one throwaway checkout and its registration. Synchronous, so a
/// drop on any path (a panic, a cancelled pair) still removes it.
fn remove_now(root: &Path, dir: &Path) {
    let git = |args: &[&str]| {
        let _ = std::process::Command::new("git")
            .args(["-c", "core.fsmonitor=false"])
            .args(args)
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(std::process::Stdio::null())
            .output();
    };
    if let Some(d) = dir.to_str() {
        git(&[
            "worktree",
            "remove",
            "--force",
            "--force",
            "--end-of-options",
            d,
        ]);
    }
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
    git(&["worktree", "prune"]);
}

/// Removes every throwaway checkout a stopped host left in this repository.
pub async fn sweep(root: &Path) {
    let parent = root.join(crate::git::WORKTREE_DIR);
    let Ok(entries) = std::fs::read_dir(&parent) else {
        return;
    };
    for e in entries.flatten() {
        if e.file_name().to_string_lossy().starts_with(PREFIX) {
            let (root, dir) = (root.to_path_buf(), e.path());
            let _ = tokio::task::spawn_blocking(move || remove_now(&root, &dir)).await;
        }
    }
}

/// [`sweep`] over every registered project, at host start.
pub async fn sweep_all(state: &Shared) {
    let roots: Vec<PathBuf> = {
        let world = state.world.lock().await;
        world.projects().map(|p| p.root.clone()).collect()
    };
    for root in roots {
        sweep(&root).await;
    }
}

/// Both runs for `key`, from the change's checkout at `change_dir`, in a
/// throwaway checkout of the repository at `root`. Host-free: no store, no
/// lane, no state — CI calls this directly. `check` is the change's latest
/// `check` report, whose JUnit names what did not run on the change.
pub async fn run(
    root: &Path,
    change_dir: &Path,
    key: &Key,
    config: &ProjectConfig,
    check: Option<&GateReport>,
) -> BaseChecks {
    let both = |why: String| {
        (
            OnChange {
                defining: pure::Defining::default(),
                run: Run::not_run(pure::RunOutcome::CouldNotRun { why: why.clone() }),
                named: None,
            },
            OnBase {
                placement: pure::Placement::default(),
                run: Run::not_run(pure::RunOutcome::CouldNotRun { why }),
                counted: pure::NewTests::None,
            },
        )
    };
    let (on_change, on_base) = match pair(root, change_dir, key, config, check).await {
        Ok(p) => p,
        Err(e) => both(format!("{e:#}")),
    };
    BaseChecks {
        key: key.clone(),
        at: jiff::Timestamp::now(),
        on_change,
        on_base,
    }
}

async fn pair(
    root: &Path,
    change_dir: &Path,
    key: &Key,
    config: &ProjectConfig,
    check: Option<&GateReport>,
) -> Result<(OnChange, OnBase)> {
    // The verified tree, written so the throwaway checkout can read it.
    let written = crate::git::digest_stored(change_dir)
        .await
        .context("the tree could not be materialised")?;
    anyhow::ensure!(written.tree == key.tree, "the tree moved since `check`");
    if let Some((path, _)) = written
        .entries
        .iter()
        .find(|(_, (mode, _))| mode == "160000")
    {
        anyhow::bail!("the tree holds a submodule ({path})");
    }
    let set = crate::git::change_set(change_dir, &key.base)
        .await
        .context("the change's diff could not be read")?;
    let roles = config.review.roles.as_ref();
    let commands: Vec<String> = crate::view::gate_commands(config)
        .into_iter()
        .chain(config.workspace.setup.clone())
        .collect();
    let defining = pure::defining(&set, roles, &commands);
    let mut placement = pure::placement(&set, roles);
    for p in &mut placement.placed {
        p.blob = written.entries.get(&p.path).map(|(_, oid)| oid.clone());
    }

    let name = format!(
        "{PREFIX}{}-{}",
        crate::core::text::clip(&key.base, 12),
        crate::core::text::clip(&key.tree, 8)
    );
    let at = root.join(crate::git::WORKTREE_DIR).join(&name);
    if at.exists() {
        remove_now(root, &at);
    }
    let dir = crate::git::create_detached_worktree(root, &name, &key.base)
        .await
        .context("the throwaway checkout could not be made")?;
    let guard = Checkout {
        root: root.to_path_buf(),
        dir: dir.clone(),
    };

    // The base's tests on this change.
    let first = match defining.is_empty() {
        true => Run::not_run(pure::RunOutcome::Skipped {
            why: "the change edits no file that defines the checks".into(),
        }),
        false => {
            crate::git::read_tree_reset(&dir, &key.tree).await?;
            let restored_from_base =
                crate::git::checkout_paths(&dir, &key.base, &defining.restored)
                    .await
                    .context("the base's files could not be put back");
            match restored_from_base {
                Err(e) => Run::not_run(pure::RunOutcome::CouldNotRun {
                    why: format!("{e:#}"),
                }),
                Ok(()) => {
                    for p in &defining.removed {
                        let _ = std::fs::remove_file(dir.join(p));
                    }
                    run_check(root, &dir, config).await
                }
            }
        }
    };
    let named = match (first.wrote_junit(), check) {
        (true, Some(c)) if c.junit.iter().any(|j| !j.read.cases().is_empty()) => {
            let theirs: Vec<_> = c
                .junit
                .iter()
                .flat_map(|j| j.read.cases())
                .cloned()
                .collect();
            let ours: Vec<_> = first.cases().into_iter().cloned().collect();
            Some(crate::core::testcount::junit::diff(&ours, &theirs))
        }
        _ => None,
    };
    let on_change = OnChange {
        defining,
        run: first,
        named,
    };

    // This change's new tests on the base, in the same checkout.
    let second = match pure::nothing_to_place(&placement) {
        Some(why) => Run::not_run(pure::RunOutcome::Skipped { why }),
        None => {
            crate::git::read_tree_reset(&dir, &key.base).await?;
            crate::git::clean_all(&dir).await?;
            let paths: Vec<String> = placement.placed.iter().map(|p| p.path.clone()).collect();
            crate::git::checkout_paths(&dir, &key.tree, &paths)
                .await
                .context("the change's test files could not be placed")?;
            run_check(root, &dir, config).await
        }
    };
    let change_wrote_junit = check.is_some_and(|c| {
        c.junit.iter().any(|j| {
            matches!(
                j.read,
                crate::core::testcount::junit::JunitRead::Read { .. }
            )
        })
    });
    let first_ran = !matches!(on_change.run.outcome, pure::RunOutcome::Skipped { .. });
    let counted = pure::new_tests(
        &second,
        first_ran.then_some(&on_change.run),
        &placement.placed,
        change_wrote_junit,
    );
    drop(guard);
    Ok((
        on_change,
        OnBase {
            placement,
            run: second,
            counted,
        },
    ))
}

/// `.worktreeinclude`, setup, then `check`, in the throwaway checkout, with
/// the shared caches and the declared JUnit paths. A setup that fails is a
/// run that could not be made.
async fn run_check(root: &Path, dir: &Path, config: &ProjectConfig) -> Run {
    let included = crate::git::ignored_files(root).await.unwrap_or_default();
    crate::git::copy_included(root, dir, &included);
    let env = crate::core::caches::env_for(root, &config.workspace.share);
    let mut setup = None;
    if let Some(cmd) = &config.workspace.setup {
        let report = crate::gates::run(
            "setup",
            std::slice::from_ref(cmd),
            dir,
            config.gates.timeout,
            1,
            &crate::gates::GateInput::env(env.clone()),
        )
        .await;
        let passed = report.passed();
        setup = report.commands.into_iter().next();
        if !passed {
            let mut run = Run::not_run(pure::RunOutcome::CouldNotRun {
                why: format!("the setup command `{cmd}` failed"),
            });
            run.setup = setup;
            return run;
        }
    }
    let report = crate::gates::run(
        crate::core::change::CHECK,
        &config.gates.check,
        dir,
        config.gates.timeout,
        1,
        &crate::gates::GateInput {
            env,
            junit: config.gates.junit.clone(),
        },
    )
    .await;
    Run {
        outcome: pure::RunOutcome::of(&report),
        commands: report.commands,
        junit: report.junit,
        setup,
    }
}

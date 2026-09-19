//! Resolving the policy that governs a directory.
//!
//! A permission rule belongs to the project it protects. `Bash(pnpm test *)`
//! is safe in the repository whose tests that runs and meaningless in the one
//! next to it, so a single global rule set is the wrong shape. Offering a
//! `[policy]` section and then not resolving it is worse than not offering one.
//!
//! The constraint that shapes everything here is latency. This is consulted on
//! the synchronous hook that Claude Code blocks on while it decides whether to
//! show a dialog, so a lookup has to cost microseconds. Reading a file per
//! check would not; caching without noticing edits would mean a rule someone
//! just wrote does nothing until the daemon restarts. So: cache, keyed by
//! repository, invalidated by the file's modification time, re-checked at most
//! once a second.

use crate::core::policy::Context;
// The repository whose rules govern a directory. One definition, shared with
// the board and the dispatcher: a worktree governed by one of them and not the
// others is a worktree where the rules a person wrote quietly do not apply.
use crate::core::project::governing_root as repo_root_of;
use crate::core::{Policy, ProjectConfig, Verdict};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

thread_local! {
    /// What this thread has resolved **during the current evaluation**, and to
    /// what.
    ///
    /// One entry was enough while every rule asked about the same file. It is
    /// not any more: a deny rule now also resolves its own leading literal
    /// segments, so that a rule naming a symlinked directory meets a command
    /// naming the real one. Those prefixes differ per rule, so a
    /// single-entry memo thrashes and the cost goes back to one syscall per
    /// rule — on the hook a session is blocked on.
    ///
    /// Cleared at the start of every evaluation rather than invalidated, which
    /// keeps the guarantee the single entry used to give for free: an answer
    /// is never reused across calls, so a symlink re-pointed between two tool
    /// calls is seen. Within one call it is a race no cache size fixes, and
    /// the answer to that is the sandbox.
    static LAST_REAL: RefCell<HashMap<PathBuf, Option<PathBuf>>> =
        RefCell::new(HashMap::new());
}

/// Where a path really points when that is somewhere else, memoised for the
/// length of one evaluation.
///
/// Both tests happen here rather than in the matcher, which would repeat them
/// per rule. `canonicalize` fails for a file that does not exist — the ordinary
/// case for the target of a write — and a path resolving to itself is `None`
/// too, since it has only one spelling either way.
fn realpath(p: &Path) -> Option<PathBuf> {
    LAST_REAL.with(|cell| {
        if let Some(answer) = cell.borrow().get(p) {
            return answer.clone();
        }
        #[cfg(test)]
        RESOLVED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let answer = std::fs::canonicalize(p).ok().filter(|r| r != p);
        let mut memo = cell.borrow_mut();
        // A rule set is attacker-supplied, so the memo is bounded
        // rather than trusted to stay small. Past the bound it stops growing;
        // the calls beyond it pay a syscall each and still get right answers.
        if memo.len() < 256 {
            memo.insert(p.to_path_buf(), answer.clone());
        }
        answer
    })
}

/// Forget what the last evaluation resolved.
///
/// Called at the start of each one, so an answer is never reused across tool
/// calls and a symlink re-pointed between two of them is seen.
fn forget_resolved() {
    LAST_REAL.with(|cell| cell.borrow_mut().clear());
}

/// How many times the filesystem has actually been asked, as opposed to the
/// memo answering.
///
/// A counter rather than a stopwatch. The property that matters is *one syscall
/// per evaluation, whatever the rule count*, and a wall-clock assertion for it
/// would fold in the cache lookup and the gitignore matching, be flaky on a
/// loaded machine, and fail for reasons that have nothing to do with symlinks —
/// which is exactly what the first version of the test did.
#[cfg(test)]
pub(crate) static RESOLVED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How often the file is re-checked for edits. Short enough that a rule takes
/// effect while you are still looking at the terminal.
const RECHECK: Duration = Duration::from_secs(1);

/// What a project's config says, once compiled. Both pieces come from the same
/// file, so they are read, cached and invalidated together.
#[derive(Debug, Clone, Default)]
struct Rules {
    policy: Policy,
    /// `[policy] stall_timeout`, in seconds. `None` means the project has not
    /// said, and the machine-wide setting decides.
    stall_seconds: Option<i64>,
}

#[derive(Debug)]
struct Entry {
    rules: Rules,
    /// The config file's modification time when it was read; `None` when there
    /// was no file, so one appearing is noticed too.
    mtime: Option<SystemTime>,
    checked: Instant,
    /// Set when the file would not load. Keeping the previous rules is the
    /// right behaviour — a typo must not read as permission — but it is only
    /// half an answer, because after a restart there are no previous rules to
    /// keep and the project's `never_auto` list is silently gone. So the fact
    /// is recorded and `devplane doctor` says it out loud.
    error: Option<String>,
}

/// Policies by repository, plus the global fallback.
#[derive(Debug)]
pub struct PolicyCache {
    /// Applies everywhere, from `~/.devplane/policy.toml`.
    global: Policy,
    /// Where that file lives. A rule spelled `Read(/secrets/**)` anchors at the
    /// file it was written in, so the machine-wide set and a project's set
    /// resolve the same pattern to different directories — which is the trap
    /// Claude Code's own documentation calls out, and it can only be got right
    /// by knowing where each set came from.
    global_root: PathBuf,
    /// The user's home, for `~/`-anchored rules. Read once: this is consulted
    /// on the hook a session is blocked on, and an environment lookup per check
    /// is a syscall per check.
    home: Option<PathBuf>,
    projects: Mutex<HashMap<PathBuf, Entry>>,
}

impl PolicyCache {
    /// `global_root` is the directory the machine-wide rules were read from —
    /// `~/.devplane` — and `home` the user's home directory.
    pub fn new(global: Policy, global_root: PathBuf, home: Option<PathBuf>) -> Self {
        Self {
            global,
            global_root,
            home,
            projects: Mutex::new(HashMap::new()),
        }
    }

    /// A cache with nothing but a project's own rules. **Tests only**, where
    /// there is no home directory to read and none should be invented.
    ///
    /// Not for a command a person runs: `devplane explain` used this and so
    /// answered without the machine-wide rules, which meant the surface built
    /// to say *what would the gate decide* could say `allow` for a call
    /// `~/.devplane/policy.toml` denies. [`Self::from_disk`] is what a command
    /// uses.
    pub fn for_projects_only() -> Self {
        Self::new(Policy::default(), PathBuf::from("/"), None)
    }

    /// The rules this machine actually enforces: the machine-wide file plus
    /// whatever each project adds.
    ///
    /// **One evaluator for every surface.** The daemon, `devplane explain` and
    /// the `command` hook all build the gate this way, so a verdict cannot
    /// depend on which of the three was asked. It used to: two of them read the
    /// machine-wide file and one did not.
    ///
    /// A machine-wide file that will not parse yields no machine-wide rules and
    /// says so through `error`. The projects' own rules still apply — refusing
    /// to answer at all would take every project's prohibitions down with one
    /// typo in a file they do not control.
    pub fn from_disk() -> (Self, Option<String>) {
        let Ok(home) = crate::config::home() else {
            return (Self::for_projects_only(), None);
        };
        let global_root = home.clone();
        let user_home = dirs::home_dir();
        match crate::core::GlobalConfig::load(&home) {
            Ok(g) => (Self::new(g.policy(), global_root, user_home), None),
            Err(e) => (
                Self::new(Policy::default(), global_root, user_home),
                Some(e.to_string()),
            ),
        }
    }

    /// Decides one tool call, for an agent working in `dir`.
    ///
    /// Deny wins across both rule sets and in either direction: a project
    /// cannot allow what the machine forbids, and the machine's allow does not
    /// override a project's deny. Anything else would make adding a rule
    /// somewhere able to quietly widen a prohibition written somewhere else.
    pub fn evaluate(&self, dir: &Path, tool: &str, input: &serde_json::Value) -> Verdict {
        forget_resolved();
        let project = self.for_dir(dir).map(|r| r.policy);
        let source = repo_root_of(dir).unwrap_or_else(|| dir.to_path_buf());
        let sets = self.sets(dir, &source, project.as_ref());

        match restrictive_over(&sets, tool, input) {
            Verdict::Undecided => {}
            decided => return decided,
        }
        // An allow rule covers the command, not the file it writes — and the
        // target may be spoken for by *either* rule set, so the question is
        // asked once across both rather than inside each. Asking each set on
        // its own would let a target the machine-wide file allows be refused
        // because the project's file never mentioned it.
        // `is_shell`, never a literal `"Bash"`: `Policy::evaluate` asks the same
        // question through that function, and the two spellings drifted apart
        // the moment `Monitor` was added — leaving the redirect check running
        // per rule set here and across both sets there, for the same call.
        if crate::core::policy::is_shell(tool)
            && crate::core::policy::uncovered_targets(input, dir, |side, file| {
                sets.iter().any(|(p, c)| p.allows_path(side, c, file))
                    || crate::core::policy::within(dir, file)
            })
            .is_some()
        {
            return Verdict::Undecided;
        }
        for (p, ctx) in &sets {
            // Nothing here can answer yes any more, so a hit is a prohibition
            // or a deferral, and both are worth returning early.
            match p.evaluate(ctx, tool, input) {
                Verdict::Undecided => {}
                decided => return decided,
            }
        }
        Verdict::Undecided
    }

    /// The prohibitions only, across both rule sets.
    ///
    /// What the `PreToolUse` hook answers with, and the only path by which a
    /// project's `never_auto` or `always_ask` rule reaches a session running in
    /// **auto mode** — where a classifier approves routine actions with no
    /// prompt, so `PermissionRequest` never fires and the rest of this file
    /// would never be consulted at all.
    pub fn restrictive(&self, dir: &Path, tool: &str, input: &serde_json::Value) -> Verdict {
        forget_resolved();
        let project = self.for_dir(dir).map(|r| r.policy);
        let source = repo_root_of(dir).unwrap_or_else(|| dir.to_path_buf());
        restrictive_over(&self.sets(dir, &source, project.as_ref()), tool, input)
    }

    /// The prohibitions from the machine-wide file, for a call naming no
    /// directory. Same reasoning as [`Self::evaluate_global_only`].
    pub fn restrictive_global_only(&self, tool: &str, input: &serde_json::Value) -> Verdict {
        forget_resolved();
        let ctx = Context::at(&self.global_root)
            .with_home(self.home.as_deref())
            .with_realpath(realpath);
        self.global.restrictive(&ctx, tool, input)
    }

    /// The rule sets that govern `dir`, each paired with the directory it was
    /// written in — because that is what a single leading slash anchors to.
    /// Both see the same working directory, which is what `Read(.env)` is
    /// relative to.
    fn sets<'a>(
        &'a self,
        dir: &'a Path,
        source: &'a Path,
        project: Option<&'a Policy>,
    ) -> Vec<(&'a Policy, Context<'a>)> {
        let home = self.home.as_deref();
        let mut out: Vec<(&Policy, Context<'_>)> = Vec::with_capacity(2);
        if let Some(p) = project {
            out.push((
                p,
                Context::at(dir)
                    .with_home(home)
                    .with_source(source)
                    .with_realpath(realpath),
            ));
        }
        out.push((
            &self.global,
            Context::at(dir)
                .with_home(home)
                .with_source(&self.global_root)
                .with_realpath(realpath),
        ));
        out
    }
}

/// Deny then ask, across every rule set, each stage completed before the next
/// begins — so a project's file cannot answer a call the machine-wide file
/// asked to be asked about, any more than it may answer one the machine forbids.
fn restrictive_over(
    sets: &[(&Policy, Context<'_>)],
    tool: &str,
    input: &serde_json::Value,
) -> Verdict {
    let verdicts: Vec<Verdict> = sets
        .iter()
        .map(|(p, ctx)| p.restrictive(ctx, tool, input))
        .collect();
    for v in &verdicts {
        if let Verdict::Deny { rule } = v {
            return Verdict::Deny { rule: rule.clone() };
        }
    }
    for v in &verdicts {
        if let Verdict::Ask { rule } = v {
            return Verdict::Ask { rule: rule.clone() };
        }
    }
    Verdict::Undecided
}

impl PolicyCache {
    /// Decides a call that names no directory.
    ///
    /// Evaluating a payload without a `cwd` against `"."` would use the
    /// *daemon's* working directory, letting whichever repository it was started
    /// in answer for a session somewhere else. There is no honest project answer
    /// without a directory, so the machine-wide rules decide alone.
    /// The rule to paste so this call is never asked about again.
    ///
    /// **The context is built the way a verdict's is**, and that is the point
    /// rather than a detail. `core::offer` refuses any rule that does not
    /// decide the call, and a replay anchored somewhere the real evaluation is
    /// not would be checking a different question — a rule spelled
    /// `Read(/secrets/**)` means one directory in a project file and another in
    /// the machine-wide one.
    ///
    /// So the anchor follows the **destination**: a call inside a registered
    /// project is answered by that project's `devplane.toml`, anchored at its
    /// root, and everything else by `~/.devplane/policy.toml`, anchored at the
    /// directory that file lives in.
    ///
    /// `others` are the calls that would **also** interrupt. Filtering them is
    /// the caller's, because it needs the event log and this side may not have
    /// one.
    pub fn offer_for(
        &self,
        dir: &Path,
        tool: &str,
        input: &serde_json::Value,
        others: &[crate::core::offer::Interrupting],
        project_root: Option<&Path>,
    ) -> Result<crate::core::offer::RuleOffer, crate::core::offer::NoOffer> {
        forget_resolved();
        // **Where a grant goes now that Devplane does not evaluate one.**
        //
        // This named `[policy] auto_allow` in Devplane's own file until
        // 2026-09-18. That key still parses, but nothing in it decides a call
        // any more: approving one would mean claiming the vendor would have
        // approved it too, and Devplane stopped making that claim. A suggestion
        // pointing at a key that no longer answers is worse than no suggestion
        // — somebody pastes it, nothing changes, and the next identical call
        // interrupts them again.
        //
        // So the rule goes where it is enforced: the agent's own settings.
        // Devplane composes the narrowest text that covers the call and hands
        // it over; it still writes nothing, for the reason it never did.
        let (source, dest) = match project_root {
            Some(root) => (
                root.to_path_buf(),
                crate::core::offer::Destination {
                    file: root.join(".claude/settings.json").display().to_string(),
                    section: "permissions.allow".into(),
                },
            ),
            None => (
                self.global_root.clone(),
                crate::core::offer::Destination {
                    file: "~/.claude/settings.json".to_string(),
                    section: "permissions.allow".into(),
                },
            ),
        };
        let ctx = Context::at(dir)
            .with_source(&source)
            .with_home(self.home.as_deref())
            .with_realpath(realpath);
        crate::core::offer::compose(tool, input, &ctx, others, &dest)
    }

    pub fn evaluate_global_only(&self, tool: &str, input: &serde_json::Value) -> Verdict {
        forget_resolved();
        let ctx = Context::at(&self.global_root)
            .with_home(self.home.as_deref())
            .with_realpath(realpath);
        self.global.evaluate(&ctx, tool, input)
    }

    /// How long a run working in `dir` may be quiet before it has stalled.
    ///
    /// A project that has not said returns `None`, and the caller keeps its own
    /// number — a repository should have to opt into a different threshold, not
    /// inherit one by accident.
    pub fn stall_seconds(&self, dir: &Path) -> Option<i64> {
        self.for_dir(dir)?.stall_seconds
    }

    /// The rules for a directory, loading or refreshing as needed.
    fn for_dir(&self, dir: &Path) -> Option<Rules> {
        let root = repo_root_of(dir)?;
        let path = root.join(crate::core::config::CONFIG_FILE);

        let mut cache = self.projects.lock().ok()?;
        if let Some(entry) = cache.get(&root)
            && entry.checked.elapsed() < RECHECK
        {
            return Some(entry.rules.clone());
        }

        let mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        if let Some(entry) = cache.get_mut(&root) {
            if entry.mtime == mtime {
                // Unchanged: bump the clock so the next check is free again.
                entry.checked = Instant::now();
                return Some(entry.rules.clone());
            }
            tracing::info!(project = %root.display(), "policy reloaded");
        }

        let (rules, error) = match ProjectConfig::load(&root) {
            Ok(c) => (
                Rules {
                    policy: c.policy(),
                    stall_seconds: c.policy.stall_timeout.map(|d| d.as_secs() as i64),
                },
                None,
            ),
            Err(e) => {
                // A malformed file must not silently become "no rules": that
                // would turn a typo in a deny rule into permission.
                tracing::warn!(project = %root.display(), error = %e, "keeping the previous policy");
                (
                    cache
                        .get(&root)
                        .map(|e| e.rules.clone())
                        .unwrap_or_default(),
                    Some(e.to_string()),
                )
            }
        };

        cache.insert(
            root.clone(),
            Entry {
                rules: rules.clone(),
                mtime,
                checked: Instant::now(),
                error,
            },
        );
        Some(rules)
    }

    /// Projects whose `devplane.toml` would not load, with the reason.
    ///
    /// Reported rather than only logged. The previous rules are kept, which is
    /// the safe half; the unsafe half is that a daemon restarted against a
    /// broken file has no previous rules, so the repository's `never_auto` list
    /// is gone and nothing on screen says so.
    pub fn broken(&self) -> Vec<(PathBuf, String)> {
        let Ok(cache) = self.projects.lock() else {
            return Vec::new();
        };
        cache
            .iter()
            .filter_map(|(root, e)| e.error.clone().map(|msg| (root.clone(), msg)))
            .collect()
    }

    /// Forgets everything, so the next check reads from disk.
    pub fn clear(&self) {
        if let Ok(mut c) = self.projects.lock() {
            c.clear();
        }
    }
}

/// The repository a directory belongs to, resolving a Claude Code worktree back
/// to the checkout that owns it so both are governed by one rule set.
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_project_sets_its_own_stall_timeout() {
        // Twelve minutes of silence is a hung agent in one repository and a
        // test suite running in another. The project decides.
        let dir = repo("stall", "[policy]\nstall_timeout = \"12m\"\n");
        let cache = PolicyCache::for_projects_only();
        assert_eq!(cache.stall_seconds(&dir), Some(720));
    }

    #[test]
    fn a_project_that_says_nothing_keeps_the_machines_timeout() {
        // `None`, not a default of its own: inheriting a threshold by accident
        // is how a run gets called stalled for doing its job.
        let dir = repo("stall-silent", "[policy]\nauto_allow = [\"Read\"]\n");
        let cache = PolicyCache::for_projects_only();
        assert_eq!(cache.stall_seconds(&dir), None);
    }

    #[test]
    fn a_worktree_stalls_on_its_repositorys_timeout() {
        // Work happens in `.claude/worktrees/<name>`, which has no config file
        // of its own and must not therefore lose the one that governs it.
        let dir = repo("stall-wt", "[policy]\nstall_timeout = \"90s\"\n");
        let wt = dir.join(".claude/worktrees/fix-login");
        std::fs::create_dir_all(&wt).unwrap();
        let cache = PolicyCache::for_projects_only();
        assert_eq!(cache.stall_seconds(&wt), Some(90));
    }

    fn repo(tag: &str, policy: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vp-pol-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        if !policy.is_empty() {
            std::fs::write(dir.join(crate::core::config::CONFIG_FILE), policy).unwrap();
        }
        dir
    }

    #[test]
    fn a_projects_own_rules_decide_its_agents() {
        // The thing that was broken: `[policy]` in devplane.toml did nothing.
        let dir = repo(
            "own",
            "[policy]\nauto_allow = [\"Bash(pnpm test *)\"]\nnever_auto = [\"Bash(rm -rf *)\"]\n",
        );
        let cache = PolicyCache::for_projects_only();
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "pnpm test -- --run"})),
            Verdict::Undecided
        ));
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "rm -rf node_modules"})),
            Verdict::Deny { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn one_projects_rules_do_not_govern_another() {
        let a = repo("a", "[policy]\nauto_allow = [\"Bash(pnpm test *)\"]\n");
        let b = repo("b", "");
        let cache = PolicyCache::for_projects_only();
        // The space before `*` is significant, as it is in Claude Code's own
        // rules: `pnpm test *` covers `pnpm test -- --run` and not `pnpm testx`.
        let cmd = json!({"command": "pnpm test -- --run"});
        assert!(matches!(
            cache.evaluate(&a, "Bash", &cmd),
            Verdict::Undecided
        ));
        assert_eq!(
            cache.evaluate(&b, "Bash", &cmd),
            Verdict::Undecided,
            "a rule belongs to the repository it protects"
        );
        std::fs::remove_dir_all(&a).ok();
        std::fs::remove_dir_all(&b).ok();
    }

    #[test]
    fn deny_wins_in_both_directions() {
        // A project cannot allow what the machine forbids, and the machine's
        // allow does not override a project's deny.
        let dir = repo("deny", "[policy]\nauto_allow = [\"Bash(git push *)\"]\n");
        let global = Policy::new(&["Bash(ls *)".into()], &["Bash(git push *)".into()]);
        let cache = PolicyCache::new(global, PathBuf::from("/"), None);
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "git push origin main"})),
            Verdict::Deny { .. }
        ));

        let dir2 = repo("deny2", "[policy]\nnever_auto = [\"Bash(ls *)\"]\n");
        let cache2 = PolicyCache::new(
            Policy::new(&["Bash(ls *)".into()], &[]),
            PathBuf::from("/"),
            None,
        );
        assert!(matches!(
            cache2.evaluate(&dir2, "Bash", &json!({"command": "ls -la"})),
            Verdict::Deny { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&dir2).ok();
    }

    #[test]
    fn a_worktree_is_governed_by_the_repository_that_owns_it() {
        // Otherwise every isolated checkout would silently lose the project's
        // rules at the moment an agent starts working in one.
        let root = repo("wt", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let wt = root.join(".claude/worktrees/feature-a");
        std::fs::create_dir_all(&wt).unwrap();
        let cache = PolicyCache::for_projects_only();
        assert!(matches!(
            cache.evaluate(&wt, "Bash", &json!({"command": "rm -rf /"})),
            Verdict::Deny { .. }
        ));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_agent_cannot_widen_its_own_rules_from_the_branch_it_is_working_on() {
        // The whole point of the split. An agent works in a worktree and may
        // edit every file in it, `devplane.toml` included — so the rules are
        // read from the checkout that *owns* the worktree, which is the copy on
        // the trunk that a person reviewed. If they were read from the worktree,
        // "delete the deny rule, then do the thing" would be a two-step escape
        // from any prohibition in the file.
        let root = repo("escalate", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let wt = root.join(".claude/worktrees/feature-a");
        std::fs::create_dir_all(&wt).unwrap();
        // The agent rewrites the rules on its branch to permit exactly what the
        // repository forbids.
        std::fs::write(
            wt.join(crate::core::config::CONFIG_FILE),
            "[policy]\nauto_allow = [\"Bash(rm -rf *)\"]\n",
        )
        .unwrap();

        let cache = PolicyCache::for_projects_only();
        assert!(
            matches!(
                cache.evaluate(&wt, "Bash", &json!({"command": "rm -rf /"})),
                Verdict::Deny { .. }
            ),
            "the owning checkout's rules decide, not the branch's"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_linked_worktree_is_governed_by_its_repository_too() {
        // `git worktree add ../feature` puts the checkout anywhere on the disk
        // and leaves a `.git` *file* behind. Walking up to the nearest `.git`
        // stopped there and called it a repository of its own — so the
        // project's `never_auto` list silently did not apply to any work
        // happening inside it.
        let root = repo("linked", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let wt = root
            .parent()
            .unwrap()
            .join(format!("vp-pol-linked-wt-{}", std::process::id()));
        std::fs::remove_dir_all(&wt).ok();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::create_dir_all(root.join(".git/worktrees/feature")).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}/.git/worktrees/feature\n", root.display()),
        )
        .unwrap();

        let cache = PolicyCache::for_projects_only();
        assert!(
            matches!(
                cache.evaluate(&wt, "Bash", &json!({"command": "rm -rf /"})),
                Verdict::Deny { .. }
            ),
            "a linked worktree inherits the repository's rules"
        );
        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&wt).ok();
    }

    #[test]
    fn an_ask_rule_outranks_an_allow_rule_in_either_file() {
        // Claude Code's third list. Without it, an `ask` rule pasted over from
        // `settings.json` had nowhere to go, and a broader `auto_allow` then
        // approved — silently — exactly the call somebody wrote a rule to be
        // asked about. "A matching ask rule prompts even when a more specific
        // allow rule also matches the same call."
        let dir = repo(
            "ask",
            "[policy]\nauto_allow = [\"Bash(git *)\"]\nalways_ask = [\"Bash(git push *)\"]\n",
        );
        let cache = PolicyCache::for_projects_only();
        // The narrow allow does not win.
        assert!(
            matches!(
                cache.evaluate(&dir, "Bash", &json!({"command": "git push --force"})),
                Verdict::Ask { .. }
            ),
            "an ask rule has to outrank the allow beside it"
        );
        // And the allow still does its job for everything else.
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "git status"})),
            Verdict::Undecided
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_projects_allow_cannot_answer_what_the_machine_asked_to_be_asked() {
        // The same rule as deny: adding a permissive rule in one file must not
        // quietly widen a decision written in another.
        let dir = repo(
            "askglobal",
            "[policy]\nauto_allow = [\"Bash(git push *)\"]\n",
        );
        let cache = PolicyCache::new(
            Policy::with_ask(&[], &[], &["Bash(git push *)".into()]),
            std::path::PathBuf::from("/"),
            None,
        );
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "git push --force"})),
            Verdict::Ask { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_edit_takes_effect_without_a_restart() {
        let dir = repo("edit", "[policy]\nauto_allow = []\n");
        let cache = PolicyCache::for_projects_only();
        assert_eq!(
            cache.evaluate(&dir, "Bash", &json!({"command": "ls"})),
            Verdict::Undecided
        );

        std::fs::write(
            dir.join(crate::core::config::CONFIG_FILE),
            "[policy]\nauto_allow = [\"Bash(ls *)\"]\n",
        )
        .unwrap();
        // The cache re-checks on a timer; clearing is what a settings change
        // does, and proves the reload path rather than the clock.
        cache.clear();
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "ls -la"})),
            Verdict::Undecided
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_broken_file_keeps_the_rules_it_had() {
        // A typo in a deny rule must never read as "no rules".
        let dir = repo("broken", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let cache = PolicyCache::for_projects_only();
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "rm -rf x"})),
            Verdict::Deny { .. }
        ));

        std::fs::write(
            dir.join(crate::core::config::CONFIG_FILE),
            "[policy]\nnever_autoo = [",
        )
        .unwrap();
        cache.clear();
        // Nothing was ever loaded for this root after clearing, so the safe
        // answer is the empty policy. Devplane has no way to say yes at all
        // now, so what this holds is that a cleared cache does not resurrect a
        // stale prohibition either.
        assert_eq!(
            cache.evaluate(&dir, "Bash", &json!({"command": "rm -rf x"})),
            Verdict::Undecided
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_directory_outside_any_repository_falls_back_to_the_global_rules() {
        let cache = PolicyCache::new(Policy::new(&["Read".into()], &[]), PathBuf::from("/"), None);
        assert!(matches!(
            cache.evaluate(Path::new("/"), "Read", &json!({})),
            Verdict::Undecided
        ));
    }

    #[test]
    fn repeated_checks_are_cheap() {
        // This runs on the hook Claude Code blocks on. Ten thousand lookups
        // must not become ten thousand file reads.
        let dir = repo("fast", "[policy]\nauto_allow = [\"Bash(ls *)\"]\n");
        let cache = PolicyCache::for_projects_only();
        let started = Instant::now();
        for _ in 0..10_000 {
            cache.evaluate(&dir, "Bash", &json!({"command": "ls -la"}));
        }
        let each = started.elapsed() / 10_000;
        assert!(
            each < Duration::from_micros(200),
            "a policy check took {each:?}; the budget is microseconds"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_path_check_asks_the_filesystem_once_per_distinct_path() {
        // The symlink rules mean a path rule asks the filesystem where things
        // really are, and this runs on the hook a session is blocked on.
        //
        // The property used to be *one syscall per evaluation, whatever the
        // rule count*, and it was true while the only path resolved was the
        // file. It is no longer, and saying so is the point of rewriting this
        // test rather than relaxing it: a deny rule also resolves its own
        // leading literal segments, so that a rule naming a symlinked
        // directory meets a command naming the real one. Ten rules with
        // ten different prefixes name eleven different paths, and eleven
        // different paths cost eleven questions. What the memo buys is that
        // **the same path is never asked about twice in one call**.
        use std::sync::atomic::Ordering;
        let distinct: String = (0..10)
            .map(|i| format!("  \"Read(never-matches-{i}/deep/**)\",\n"))
            .collect();
        let dir = repo(
            "realpath",
            &format!("[policy]\nnever_auto = [\n{distinct}]\n"),
        );
        let file = dir.join("f.txt");
        std::fs::write(&file, "x").ok();
        let cache = PolicyCache::for_projects_only();
        let call = json!({"file_path": file.to_str().unwrap()});

        // Warm the config cache. The memo is cleared by `evaluate` itself, so
        // the count starts cold whatever the warm-up left.
        cache.evaluate(&dir, "Read", &call);
        RESOLVED.store(0, Ordering::Relaxed);
        cache.evaluate(&dir, "Read", &call);
        assert_eq!(
            RESOLVED.load(Ordering::Relaxed),
            11,
            "one for the file, one for each distinct rule prefix"
        );
        std::fs::remove_dir_all(&dir).ok();

        // Ten rules that name the **same** prefix cost one question, not ten.
        // That is the memo doing its job, and it is what keeps a realistic
        // rule set — many rules under one protected directory — cheap.
        let same: String = (0..10)
            .map(|i| format!("  \"Read(secrets/deep/x{i}/**)\",\n"))
            .collect();
        let dir = repo(
            "realpath-same",
            &format!("[policy]\nnever_auto = [\n{same}]\n"),
        );
        let file = dir.join("f.txt");
        std::fs::write(&file, "x").ok();
        let call = json!({"file_path": file.to_str().unwrap()});
        cache.evaluate(&dir, "Read", &call);
        RESOLVED.store(0, Ordering::Relaxed);
        cache.evaluate(&dir, "Read", &call);
        assert!(
            RESOLVED.load(Ordering::Relaxed) <= 11,
            "ten rules under one prefix must not cost more than ten questions"
        );

        // And the cost stays in the budget this whole module exists for: the
        // hook a session waits on. A counter cannot say that, so the one
        // wall-clock assertion here is a ceiling, not a measurement.
        let started = std::time::Instant::now();
        for _ in 0..100 {
            cache.evaluate(&dir, "Read", &call);
        }
        assert!(
            started.elapsed().as_millis() < 250,
            "a hundred evaluations over ten path rules must stay far inside one hook timeout"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

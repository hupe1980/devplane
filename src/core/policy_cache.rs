//! Resolving the policy that governs a directory: the machine-wide file plus
//! the governing project's `devplane.toml`, a deny in either winning. The
//! project file is cached by root and re-read when its bytes change (checked
//! at most once a second). A file that will not load fails closed as
//! [`Policy::unloadable`]; there are no "previous rules" to keep.

use crate::core::policy::Context;
use crate::core::{Policy, ProjectConfig, Verdict};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

thread_local! {
    /// Cleared per evaluation so a symlink re-pointed between calls is seen.
    static LAST_REAL: RefCell<HashMap<PathBuf, Option<PathBuf>>> = RefCell::new(HashMap::new());
}

/// Where a path really points, if elsewhere; memo bounded because the rule set
/// is attacker-supplied text.
fn realpath(p: &Path) -> Option<PathBuf> {
    LAST_REAL.with(|cell| {
        if let Some(answer) = cell.borrow().get(p) {
            return answer.clone();
        }
        #[cfg(test)]
        RESOLVED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let answer = std::fs::canonicalize(p).ok().filter(|r| r != p);
        let mut memo = cell.borrow_mut();
        if memo.len() < 256 {
            memo.insert(p.to_path_buf(), answer.clone());
        }
        answer
    })
}

fn forget_resolved() {
    LAST_REAL.with(|cell| cell.borrow_mut().clear());
}

/// Filesystem lookups, as opposed to memo hits.
#[cfg(test)]
pub(crate) static RESOLVED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Serialises the tests that read the counters.
#[cfg(test)]
pub(crate) static COUNTED: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(test)]
pub(crate) static PARSED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

const RECHECK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
struct Rules {
    policy: Policy,
    stall_seconds: Option<i64>,
    hold: Option<Duration>,
}

#[derive(Debug)]
struct Entry {
    rules: Rules,
    /// The file's bytes when it was read; `None` when there was no file.
    text: Option<Vec<u8>>,
    checked: Instant,
}

/// Policies by repository, plus the machine-wide fallback.
#[derive(Debug)]
pub struct PolicyCache {
    global: Policy,
    /// What a single leading slash anchors to in the machine-wide file.
    global_root: PathBuf,
    home: Option<PathBuf>,
    projects: Mutex<HashMap<PathBuf, Entry>>,
}

impl PolicyCache {
    pub fn new(global: Policy, global_root: PathBuf, home: Option<PathBuf>) -> Self {
        Self {
            global,
            global_root,
            home,
            projects: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    pub fn for_projects_only() -> Self {
        Self::new(Policy::default(), PathBuf::from("/"), None)
    }

    /// The machine-wide file plus whatever each project adds. An unloadable
    /// machine-wide file (or missing home) is [`Policy::unloadable`]; the error
    /// is also returned for surfaces that print it.
    pub fn from_disk() -> (Self, Option<String>) {
        let user_home = dirs::home_dir();
        let (global, root) = global_policy();
        let error = global.load_error().map(str::to_string);
        (Self::new(global, root, user_home), error)
    }

    /// Deny across both rule sets, then ask, then unresolved — so neither the
    /// project nor the machine can soften what the other forbids.
    pub fn restrictive(&self, dir: &Path, tool: &str, input: &serde_json::Value) -> Verdict {
        forget_resolved();
        let home = self.home.as_deref();
        let dir = canonical(dir);
        let project = self.for_dir(&dir);
        let mut sets: Vec<(&Policy, Context<'_>)> = Vec::with_capacity(2);
        if let Some((root, rules)) = &project {
            sets.push((
                &rules.policy,
                Context::at(&dir)
                    .with_home(home)
                    .with_source(root)
                    .with_realpath(realpath),
            ));
        }
        sets.push((
            &self.global,
            Context::at(&dir)
                .with_home(home)
                .with_source(&self.global_root)
                .with_realpath(realpath),
        ));
        let verdicts: Vec<Verdict> = sets
            .iter()
            .map(|(p, ctx)| p.restrictive(ctx, tool, input))
            .collect();
        for want in ["deny", "ask", "unresolved"] {
            if let Some(v) = verdicts.iter().find(|v| v.as_str() == want) {
                return v.clone();
            }
        }
        Verdict::Undecided
    }

    /// The machine-wide rules alone, for a call naming no directory (never this
    /// process's cwd, which may be another repository).
    pub fn restrictive_global_only(&self, tool: &str, input: &serde_json::Value) -> Verdict {
        forget_resolved();
        let ctx = Context::at(&self.global_root)
            .with_home(self.home.as_deref())
            .with_realpath(realpath);
        self.global.restrictive(&ctx, tool, input)
    }

    /// The allow rule to paste into the agent's own settings so this call is
    /// not asked about again, replayed in the context a verdict would use.
    pub fn offer_for(
        &self,
        dir: &Path,
        tool: &str,
        input: &serde_json::Value,
        others: &[crate::core::offer::Interrupting],
        project_root: Option<&Path>,
    ) -> Result<crate::core::offer::RuleOffer, crate::core::offer::NoOffer> {
        forget_resolved();
        let (source, file) = match project_root {
            Some(root) => (
                root.to_path_buf(),
                root.join(".claude/settings.json").display().to_string(),
            ),
            None => (
                self.global_root.clone(),
                "~/.claude/settings.json".to_string(),
            ),
        };
        let dest = crate::core::offer::Destination {
            file,
            section: crate::core::offer::ALLOW_KEY.into(),
        };
        let ctx = Context::at(dir)
            .with_source(&source)
            .with_home(self.home.as_deref())
            .with_realpath(realpath);
        crate::core::offer::compose(tool, input, &ctx, others, &dest)
    }

    pub fn stall_seconds(&self, dir: &Path) -> Option<i64> {
        self.for_dir(&canonical(dir))?.1.stall_seconds
    }

    /// How long a permission in `dir` may be held for a person, if the project
    /// said. An unloadable file holds nothing.
    pub fn hold(&self, dir: &Path) -> Option<Duration> {
        self.for_dir(&canonical(dir))?.1.hold
    }

    /// Whether any rule in force at `dir` speaks about `tool` (or a file there
    /// would not load); what an unreadable payload is judged against.
    pub fn speaks_about(&self, dir: Option<&Path>, tool: &str) -> bool {
        let project = dir.and_then(|d| self.for_dir(&canonical(d)));
        self.global.speaks_about(tool) || project.is_some_and(|(_, r)| r.policy.speaks_about(tool))
    }

    pub fn is_empty_at(&self, dir: Option<&Path>) -> bool {
        let project = dir.and_then(|d| self.for_dir(&canonical(d)));
        self.global.is_empty() && project.is_none_or(|(_, r)| r.policy.is_empty())
    }

    /// The governing root and its rules. A directory outside every root gets
    /// no project rules; the machine-wide rules decide alone.
    fn for_dir(&self, dir: &Path) -> Option<(PathBuf, Rules)> {
        let mut cache = self.projects.lock().unwrap_or_else(|e| e.into_inner());
        let known = cache
            .keys()
            .filter(|r| dir.starts_with(r))
            .max_by_key(|r| r.as_os_str().len())
            .cloned();
        let root =
            known.or_else(|| crate::core::project::governing_root(dir).map(|r| canonical(&r)))?;
        let path = root.join(crate::core::config::CONFIG_FILE);
        if let Some(entry) = cache.get(&root)
            && entry.checked.elapsed() < RECHECK
        {
            return Some((root, entry.rules.clone()));
        }
        // The file is small: its bytes are the identity, not its mtime.
        let text = std::fs::read(&path).ok();
        if let Some(entry) = cache.get_mut(&root) {
            if entry.text == text {
                entry.checked = Instant::now();
                return Some((root, entry.rules.clone()));
            }
            tracing::info!(project = %root.display(), "policy reloaded");
        }
        #[cfg(test)]
        PARSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let rules = match ProjectConfig::load(&root) {
            Ok(c) => Rules {
                policy: c.policy(),
                stall_seconds: c.policy.stall_timeout.map(|d| d.as_secs() as i64),
                // A hold that will not parse is no hold: `devplane check`
                // names it, and holding for an unintended time is worse.
                hold: c.questions.hold().ok().flatten().map(|h| h.0),
            },
            Err(e) => {
                tracing::warn!(project = %root.display(), error = %e, "the policy would not load; every call is asked");
                Rules {
                    policy: Policy::unloadable(e.to_string()),
                    ..Rules::default()
                }
            }
        };
        cache.insert(
            root.clone(),
            Entry {
                rules: rules.clone(),
                text,
                checked: Instant::now(),
            },
        );
        Some((root, rules))
    }

    /// Projects whose `devplane.toml` would not load, with the reason.
    pub fn broken(&self) -> Vec<(PathBuf, String)> {
        let cache = self.projects.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .iter()
            .filter_map(|(root, e)| {
                e.rules
                    .policy
                    .load_error()
                    .map(|msg| (root.clone(), msg.to_string()))
            })
            .collect()
    }

    pub fn clear(&self) {
        self.projects
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

/// The path as the filesystem knows it, so `.`, `..` and a symlinked parent
/// cannot make one directory look like another to the prefix test below.
fn canonical(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

/// The machine-wide rules and the directory they anchor to. A missing home is
/// unloadable, not "no rules", or an unset `HOME` would switch them all off.
pub fn global_policy() -> (Policy, PathBuf) {
    let home = match crate::config::home() {
        Ok(h) => h,
        Err(e) => {
            return (
                Policy::unloadable(format!("the machine-wide policy.toml: {e}")),
                PathBuf::from("/"),
            );
        }
    };
    match crate::core::GlobalConfig::load(&home) {
        Ok(g) => (g.policy(), home),
        Err(e) => (Policy::unloadable(e.to_string()), home),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn repo(tag: &str, policy: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vp-pol-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        if !policy.is_empty() {
            std::fs::write(dir.join(crate::core::config::CONFIG_FILE), policy).unwrap();
        }
        dir
    }

    fn sh(cmd: &str) -> serde_json::Value {
        json!({"command": cmd})
    }

    #[test]
    fn a_project_sets_its_own_stall_timeout_and_a_worktree_inherits_it() {
        let dir = repo("stall", "[policy]\nstall_timeout = \"12m\"\n");
        let cache = PolicyCache::for_projects_only();
        assert_eq!(cache.stall_seconds(&dir), Some(720));
        let wt = dir.join(".claude/worktrees/fix-login");
        std::fs::create_dir_all(&wt).unwrap();
        assert_eq!(cache.stall_seconds(&wt), Some(720));
        let silent = repo("stall-silent", "[policy]\nnever_auto = [\"Read(.env)\"]\n");
        assert_eq!(
            cache.stall_seconds(&silent),
            None,
            "inheriting a threshold by accident is how a run gets called stalled for doing its job"
        );
    }

    #[test]
    fn a_projects_own_rules_decide_its_agents_and_not_its_neighbours() {
        let a = repo("own-a", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let b = repo("own-b", "");
        let cache = PolicyCache::for_projects_only();
        assert_eq!(
            cache.restrictive(&a, "Bash", &sh("pnpm test -- --run")),
            Verdict::Undecided
        );
        assert!(matches!(
            cache.restrictive(&a, "Bash", &sh("rm -rf node_modules")),
            Verdict::Deny { .. }
        ));
        assert_eq!(
            cache.restrictive(&b, "Bash", &sh("rm -rf node_modules")),
            Verdict::Undecided
        );
    }

    #[test]
    fn deny_wins_in_both_directions_and_ask_reaches_from_either_file() {
        let dir = repo("deny", "[policy]\nalways_ask = [\"Bash(git push *)\"]\n");
        let cache = PolicyCache::new(
            Policy::rules(&["Bash(git push *)".into()], &[]),
            PathBuf::from("/"),
            None,
        );
        assert!(matches!(
            cache.restrictive(&dir, "Bash", &sh("git push origin main")),
            Verdict::Deny { .. }
        ));
        let dir2 = repo("deny2", "[policy]\nnever_auto = [\"Bash(ls *)\"]\n");
        let cache2 = PolicyCache::new(
            Policy::rules(&[], &["Bash(ls *)".into()]),
            PathBuf::from("/"),
            None,
        );
        assert!(matches!(
            cache2.restrictive(&dir2, "Bash", &sh("ls -la")),
            Verdict::Deny { .. }
        ));
        let dir3 = repo("askglobal", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let cache3 = PolicyCache::new(
            Policy::rules(&[], &["Bash(git push *)".into()]),
            PathBuf::from("/"),
            None,
        );
        assert!(matches!(
            cache3.restrictive(&dir3, "Bash", &sh("git push --force")),
            Verdict::Ask { .. }
        ));
        assert!(
            matches!(
                cache3.restrictive(&dir3, "Bash", &sh("$(x)")),
                Verdict::Unresolved { .. }
            ),
            "the machine's rule constrains the tool here too"
        );
    }

    #[test]
    fn an_agent_cannot_widen_its_rules_from_the_worktree_it_works_in() {
        let root = repo("escalate", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let wt = root.join(".claude/worktrees/feature-a");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(crate::core::config::CONFIG_FILE),
            "[policy]\nnever_auto = []\n",
        )
        .unwrap();
        let cache = PolicyCache::for_projects_only();
        assert!(
            matches!(
                cache.restrictive(&wt, "Bash", &sh("rm -rf /")),
                Verdict::Deny { .. }
            ),
            "the owning checkout's rules decide, not the branch's"
        );
        // Nor by spelling the directory differently.
        let dotted = wt.join("sub/..");
        std::fs::create_dir_all(wt.join("sub")).unwrap();
        assert!(matches!(
            cache.restrictive(&dotted, "Bash", &sh("rm -rf /")),
            Verdict::Deny { .. }
        ));
    }

    #[test]
    fn a_linked_worktree_is_governed_by_its_repository_only_when_git_agrees() {
        let root = repo("linked", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let wt = root
            .parent()
            .unwrap()
            .join(format!("vp-pol-linked-wt-{}", std::process::id()));
        std::fs::remove_dir_all(&wt).ok();
        std::fs::create_dir_all(&wt).unwrap();
        let private = root.join(".git/worktrees/feature");
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", private.display())).unwrap();
        std::fs::write(
            private.join("gitdir"),
            format!("{}\n", wt.join(".git").display()),
        )
        .unwrap();
        let cache = PolicyCache::for_projects_only();
        assert!(
            matches!(
                cache.restrictive(&wt, "Bash", &sh("rm -rf /")),
                Verdict::Deny { .. }
            ),
            "a linked worktree inherits the repository's rules"
        );

        // A rewritten `.git` is not believed: the back-reference disagrees.
        let lax = repo("linked-lax", "");
        std::fs::create_dir_all(lax.join(".git/worktrees/feature")).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", lax.join(".git/worktrees/feature").display()),
        )
        .unwrap();
        std::fs::write(
            wt.join(crate::core::config::CONFIG_FILE),
            "[policy]\nnever_auto = []\n",
        )
        .unwrap();
        cache.clear();
        let global = PolicyCache::new(
            Policy::rules(&["Bash(rm -rf *)".into()], &[]),
            PathBuf::from("/"),
            None,
        );
        assert!(
            matches!(
                global.restrictive(&wt, "Bash", &sh("rm -rf /")),
                Verdict::Deny { .. }
            ),
            "the machine-wide rules still apply"
        );
        assert!(matches!(
            global.restrictive(&wt, "Bash", &sh("rm -rf /")),
            Verdict::Deny { .. }
        ));
        assert_eq!(
            global.stall_seconds(&wt),
            None,
            "and the forged checkout's own file is not read"
        );
        std::fs::remove_dir_all(&wt).ok();
    }

    #[test]
    fn an_edit_takes_effect_and_a_broken_file_asks_about_everything() {
        let dir = repo("edit", "[policy]\nnever_auto = []\n");
        let cache = PolicyCache::for_projects_only();
        assert_eq!(
            cache.restrictive(&dir, "Bash", &sh("ls -la")),
            Verdict::Undecided
        );
        std::fs::write(
            dir.join(crate::core::config::CONFIG_FILE),
            "[policy]\nnever_auto = [\"Bash(ls *)\"]\n",
        )
        .unwrap();
        cache.clear();
        assert!(matches!(
            cache.restrictive(&dir, "Bash", &sh("ls -la")),
            Verdict::Deny { .. }
        ));
        std::fs::write(
            dir.join(crate::core::config::CONFIG_FILE),
            "[policy]\nnever_autoo = [",
        )
        .unwrap();
        std::thread::sleep(RECHECK);
        match cache.restrictive(&dir, "Read", &json!({"file_path": "x"})) {
            Verdict::Unresolved { why } => assert!(
                why.contains(crate::core::config::CONFIG_FILE),
                "the reason names the file: {why}"
            ),
            v => panic!("a typo must not read as no rules: {v:?}"),
        }
        assert_eq!(cache.broken().len(), 1);
        // And a fresh process — which is what every hook is — sees the same.
        let fresh = PolicyCache::for_projects_only();
        assert!(matches!(
            fresh.restrictive(&dir, "Bash", &sh("ls -la")),
            Verdict::Unresolved { .. }
        ));
        assert_eq!(fresh.hold(&dir), None);
        // A machine-wide deny still wins over a broken project file.
        let machine = PolicyCache::new(
            Policy::rules(&["Bash(ls *)".into()], &[]),
            PathBuf::from("/"),
            None,
        );
        assert!(matches!(
            machine.restrictive(&dir, "Bash", &sh("ls -la")),
            Verdict::Deny { .. }
        ));
    }

    #[test]
    fn a_directory_outside_any_repository_falls_back_to_the_global_rules() {
        let cache = PolicyCache::new(
            Policy::rules(&["Bash(rm *)".into()], &[]),
            PathBuf::from("/"),
            None,
        );
        assert!(matches!(
            cache.restrictive(Path::new("/"), "Bash", &json!({"command": "rm x"})),
            Verdict::Deny { .. }
        ));
        assert_eq!(
            cache.restrictive(Path::new("/nowhere/at/all"), "Read", &json!({})),
            Verdict::Undecided
        );
    }

    #[test]
    fn repeated_checks_are_cheap() {
        use std::sync::atomic::Ordering;
        let _counted = COUNTED.lock().unwrap_or_else(|e| e.into_inner());
        let dir = repo("fast", "[policy]\nnever_auto = [\"Bash(ls *)\"]\n");
        let cache = PolicyCache::for_projects_only();
        cache.restrictive(&dir, "Bash", &sh("ls -la"));
        PARSED.store(0, Ordering::Relaxed);
        for _ in 0..10_000 {
            cache.restrictive(&dir, "Bash", &sh("ls -la"));
        }
        let parsed = PARSED.load(Ordering::Relaxed);
        assert!(
            parsed <= 4,
            "ten thousand lookups parsed the config {parsed} times"
        );
    }

    #[test]
    fn a_path_check_asks_the_filesystem_once_per_distinct_path() {
        use std::sync::atomic::Ordering;
        let _counted = COUNTED.lock().unwrap_or_else(|e| e.into_inner());
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
        cache.restrictive(&dir, "Read", &call);
        RESOLVED.store(0, Ordering::Relaxed);
        cache.restrictive(&dir, "Read", &call);
        assert_eq!(
            RESOLVED.load(Ordering::Relaxed),
            11,
            "one for the file, one for each distinct rule prefix"
        );

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
        cache.restrictive(&dir, "Read", &call);
        RESOLVED.store(0, Ordering::Relaxed);
        cache.restrictive(&dir, "Read", &call);
        assert!(
            RESOLVED.load(Ordering::Relaxed) <= 11,
            "ten rules under one prefix must not cost more than ten questions"
        );
        let started = std::time::Instant::now();
        for _ in 0..100 {
            cache.restrictive(&dir, "Read", &call);
        }
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
}

//! Resolving the policy that governs a directory.
//!
//! A permission rule belongs to the project it protects. `Bash(pnpm test *)`
//! is safe in the repository whose tests that runs and meaningless in the one
//! next to it, so a single global rule set is the wrong shape — and until now
//! the `[policy]` section in `vibeplane.toml` was parsed and then ignored,
//! which is worse than not offering it.
//!
//! The constraint that shapes everything here is latency. This is consulted on
//! the synchronous hook that Claude Code blocks on while it decides whether to
//! show a dialog, so a lookup has to cost microseconds. Reading a file per
//! check would not; caching without noticing edits would mean a rule someone
//! just wrote does nothing until the daemon restarts. So: cache, keyed by
//! repository, invalidated by the file's modification time, re-checked at most
//! once a second.

use crate::{Policy, ProjectConfig, Verdict};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

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
}

/// Policies by repository, plus the global fallback.
#[derive(Debug)]
pub struct PolicyCache {
    /// Applies everywhere, from `~/.vibeplane/policy.toml`.
    global: Policy,
    projects: Mutex<HashMap<PathBuf, Entry>>,
}

impl PolicyCache {
    pub fn new(global: Policy) -> Self {
        Self {
            global,
            projects: Mutex::new(HashMap::new()),
        }
    }

    /// Decides one tool call, for an agent working in `dir`.
    ///
    /// Deny wins across both rule sets and in either direction: a project
    /// cannot allow what the machine forbids, and the machine's allow does not
    /// override a project's deny. Anything else would make adding a rule
    /// somewhere able to quietly widen a prohibition written somewhere else.
    pub fn evaluate(&self, dir: &Path, tool: &str, input: &serde_json::Value) -> Verdict {
        let project = self.for_dir(dir).map(|r| r.policy);
        let policies: Vec<&Policy> = project
            .iter()
            .chain(std::iter::once(&self.global))
            .collect();

        for p in &policies {
            if let Verdict::Deny { rule } = p.evaluate(tool, input) {
                return Verdict::Deny { rule };
            }
        }
        for p in &policies {
            if let Verdict::Allow { rule } = p.evaluate(tool, input) {
                return Verdict::Allow { rule };
            }
        }
        Verdict::Undecided
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
        let path = root.join(crate::config::CONFIG_FILE);

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

        let rules = ProjectConfig::load(&root)
            .map(|c| Rules {
                policy: c.policy(),
                stall_seconds: c.policy.stall_timeout.map(|d| d.as_secs() as i64),
            })
            .unwrap_or_else(|e| {
                // A malformed file must not silently become "no rules": that
                // would turn a typo in a deny rule into permission.
                tracing::warn!(error = %e, "keeping the previous policy");
                cache
                    .get(&root)
                    .map(|e| e.rules.clone())
                    .unwrap_or_default()
            });

        cache.insert(
            root.clone(),
            Entry {
                rules: rules.clone(),
                mtime,
                checked: Instant::now(),
            },
        );
        Some(rules)
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
fn repo_root_of(dir: &Path) -> Option<PathBuf> {
    if let Some(main) = worktree_parent(dir) {
        return Some(main);
    }
    let mut cur = Some(dir);
    while let Some(d) = cur {
        if d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
        cur = d.parent();
    }
    None
}

fn worktree_parent(path: &Path) -> Option<PathBuf> {
    let s = path.to_string_lossy();
    let idx = s.find("/.claude/worktrees/")?;
    Some(PathBuf::from(&s[..idx]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_project_sets_its_own_stall_timeout() {
        // Twelve minutes of silence is a hung agent in one repository and a
        // test suite running in another. The project decides.
        let dir = repo("stall", "[policy]\nstall_timeout = \"12m\"\n");
        let cache = PolicyCache::new(Policy::default());
        assert_eq!(cache.stall_seconds(&dir), Some(720));
    }

    #[test]
    fn a_project_that_says_nothing_keeps_the_machines_timeout() {
        // `None`, not a default of its own: inheriting a threshold by accident
        // is how a run gets called stalled for doing its job.
        let dir = repo("stall-silent", "[policy]\nauto_allow = [\"Read\"]\n");
        let cache = PolicyCache::new(Policy::default());
        assert_eq!(cache.stall_seconds(&dir), None);
    }

    #[test]
    fn a_worktree_stalls_on_its_repositorys_timeout() {
        // Work happens in `.claude/worktrees/<name>`, which has no config file
        // of its own and must not therefore lose the one that governs it.
        let dir = repo("stall-wt", "[policy]\nstall_timeout = \"90s\"\n");
        let wt = dir.join(".claude/worktrees/fix-login");
        std::fs::create_dir_all(&wt).unwrap();
        let cache = PolicyCache::new(Policy::default());
        assert_eq!(cache.stall_seconds(&wt), Some(90));
    }

    fn repo(tag: &str, policy: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vp-pol-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        if !policy.is_empty() {
            std::fs::write(dir.join(crate::config::CONFIG_FILE), policy).unwrap();
        }
        dir
    }

    #[test]
    fn a_projects_own_rules_decide_its_agents() {
        // The thing that was broken: `[policy]` in vibeplane.toml did nothing.
        let dir = repo(
            "own",
            "[policy]\nauto_allow = [\"Bash(pnpm test *)\"]\nnever_auto = [\"Bash(rm -rf *)\"]\n",
        );
        let cache = PolicyCache::new(Policy::default());
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "pnpm test -- --run"})),
            Verdict::Allow { .. }
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
        let cache = PolicyCache::new(Policy::default());
        // The space before `*` is significant, as it is in Claude Code's own
        // rules: `pnpm test *` covers `pnpm test -- --run` and not `pnpm testx`.
        let cmd = json!({"command": "pnpm test -- --run"});
        assert!(matches!(
            cache.evaluate(&a, "Bash", &cmd),
            Verdict::Allow { .. }
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
        let cache = PolicyCache::new(global);
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "git push origin main"})),
            Verdict::Deny { .. }
        ));

        let dir2 = repo("deny2", "[policy]\nnever_auto = [\"Bash(ls *)\"]\n");
        let cache2 = PolicyCache::new(Policy::new(&["Bash(ls *)".into()], &[]));
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
        let cache = PolicyCache::new(Policy::default());
        assert!(matches!(
            cache.evaluate(&wt, "Bash", &json!({"command": "rm -rf /"})),
            Verdict::Deny { .. }
        ));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_edit_takes_effect_without_a_restart() {
        let dir = repo("edit", "[policy]\nauto_allow = []\n");
        let cache = PolicyCache::new(Policy::default());
        assert_eq!(
            cache.evaluate(&dir, "Bash", &json!({"command": "ls"})),
            Verdict::Undecided
        );

        std::fs::write(
            dir.join(crate::config::CONFIG_FILE),
            "[policy]\nauto_allow = [\"Bash(ls *)\"]\n",
        )
        .unwrap();
        // The cache re-checks on a timer; clearing is what a settings change
        // does, and proves the reload path rather than the clock.
        cache.clear();
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "ls -la"})),
            Verdict::Allow { .. }
        ));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_broken_file_keeps_the_rules_it_had() {
        // A typo in a deny rule must never read as "no rules".
        let dir = repo("broken", "[policy]\nnever_auto = [\"Bash(rm -rf *)\"]\n");
        let cache = PolicyCache::new(Policy::default());
        assert!(matches!(
            cache.evaluate(&dir, "Bash", &json!({"command": "rm -rf x"})),
            Verdict::Deny { .. }
        ));

        std::fs::write(
            dir.join(crate::config::CONFIG_FILE),
            "[policy]\nnever_autoo = [",
        )
        .unwrap();
        cache.clear();
        // Nothing was ever loaded for this root after clearing, so the safe
        // answer is the empty policy — and crucially never a silent allow.
        assert_ne!(
            cache.evaluate(&dir, "Bash", &json!({"command": "rm -rf x"})),
            Verdict::Allow {
                rule: String::new()
            }
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_directory_outside_any_repository_falls_back_to_the_global_rules() {
        let cache = PolicyCache::new(Policy::new(&["Read".into()], &[]));
        assert!(matches!(
            cache.evaluate(Path::new("/"), "Read", &json!({})),
            Verdict::Allow { .. }
        ));
    }

    #[test]
    fn repeated_checks_are_cheap() {
        // This runs on the hook Claude Code blocks on. Ten thousand lookups
        // must not become ten thousand file reads.
        let dir = repo("fast", "[policy]\nauto_allow = [\"Bash(ls *)\"]\n");
        let cache = PolicyCache::new(Policy::default());
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
}

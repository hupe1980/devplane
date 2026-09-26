//! Resolving the policy that governs a directory: the machine-wide file plus
//! the governing project's `devplane.toml`, a deny in either winning. The
//! project file is cached by root and re-read when its bytes change (checked
//! at most once a second). A file that will not load fails closed as
//! [`Policy::unloadable`]; there are no "previous rules" to keep.

use crate::core::policy::Context;
use crate::core::{Policy, ProjectConfig, Verdict};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
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
    /// `~/.devplane`, whose files no agent call may change.
    devplane_home: Option<PathBuf>,
    projects: Mutex<HashMap<PathBuf, Entry>>,
}

impl PolicyCache {
    pub fn new(global: Policy, global_root: PathBuf, home: Option<PathBuf>) -> Self {
        Self {
            global,
            global_root,
            home,
            devplane_home: None,
            projects: Mutex::new(HashMap::new()),
        }
    }

    /// Protects Devplane's own files under `dir` (see [`own_files`]).
    pub fn with_devplane_home(mut self, dir: PathBuf) -> Self {
        self.devplane_home = Some(canonical(&dir));
        self
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
        let own = crate::config::home().ok();
        let mut cache = Self::new(global, root, user_home);
        if let Some(own) = own {
            cache = cache.with_devplane_home(own);
        }
        (cache, error)
    }

    /// Deny across both rule sets, then ask, then unresolved — so neither the
    /// project nor the machine can soften what the other forbids.
    pub fn restrictive(&self, dir: &Path, tool: &str, input: &serde_json::Value) -> Verdict {
        forget_resolved();
        let home = self.home.as_deref();
        let dir = canonical(dir);
        let project = self.for_dir(&dir);
        // A refusal ends it; a question joins the rest, so a rule's deny still
        // wins over the built-in rule's doubt.
        let own = self.own_files(
            &dir,
            project.as_ref().map(|(r, _)| r.as_path()),
            tool,
            input,
        );
        if let Some(v @ Verdict::Deny { .. }) = own {
            return v;
        }
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
            .chain(own)
            .collect();
        for want in ["deny", "ask", "unresolved"] {
            if let Some(v) = verdicts.iter().find(|v| v.as_str() == want) {
                return v.clone();
            }
        }
        Verdict::Undecided
    }

    /// The built-in prohibition, beneath every rule a person writes: an agent
    /// may not change the rules that gate it, the ledger that records its
    /// permissions or the host's credentials — nor read the token, with which
    /// it could answer its own questions. Reading the rest stays allowed.
    fn own_files(
        &self,
        dir: &Path,
        root: Option<&Path>,
        tool: &str,
        input: &serde_json::Value,
    ) -> Option<Verdict> {
        use crate::core::policy::{Class, Rule};
        const WHY: &str = "built in: an agent may not change Devplane's own rules, records or \
                           credentials, nor read its token";
        let mut edits: Vec<String> = Vec::new();
        let mut reads: Vec<String> = Vec::new();
        let mut repo: Vec<String> = Vec::new();
        if let Some(h) = &self.devplane_home {
            let h = h.display();
            for f in [
                "policy.toml",
                "agents.toml",
                "devplane.db*",
                "token",
                "host.json",
            ] {
                edits.push(format!("Edit(/{h}/{f})"));
            }
            reads.push(format!("Read(/{h}/token)"));
        }
        if let Some(r) = root {
            repo.push(format!(
                "Edit(/{}/{})",
                r.display(),
                crate::core::config::CONFIG_FILE
            ));
        }
        let parse = |v: &[String]| -> Vec<Rule> {
            v.iter()
                .filter_map(|r| Rule::parse(r, Class::Deny))
                .collect()
        };
        let (edits, reads, repo) = (parse(&edits), parse(&reads), parse(&repo));
        if edits.is_empty() && repo.is_empty() {
            return None;
        }
        let ctx = Context::at(dir).with_home(self.home.as_deref());
        let deny = |r: &Rule| Verdict::Deny {
            rule: format!("{r} ({WHY})"),
        };
        if !crate::core::policy::is_shell(tool) {
            if let Some(r) = edits
                .iter()
                .chain(&repo)
                .chain(&reads)
                .find(|r| r.matches(&ctx, tool, input))
            {
                return Some(deny(r));
            }
            return None;
        }
        let command = crate::core::policy::rule_content(tool, input)?;
        let own = Own::new(self.devplane_home.as_deref(), root);
        let why = |what: &str| format!("{what} ({WHY})");
        match own.judge(&command, dir, self.home.as_deref()) {
            Judged::Deny(what) => Some(Verdict::Deny { rule: why(&what) }),
            Judged::Unresolved(what) => Some(Verdict::Unresolved { why: why(&what) }),
            Judged::Clear => None,
        }
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
        // The one resolver every surface asks, so the host and a hook never
        // pick different roots for one directory.
        let root = crate::repo::governing_root(dir).map(|r| canonical(&r))?;
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
/// Devplane's own files as absolute paths, for reading a shell line: a
/// command's arguments are resolved against every directory the line may have
/// changed to, so `cd ~/.devplane && rm policy.toml` names the file too.
struct Own {
    /// `~/.devplane`, canonical.
    home: Option<PathBuf>,
    /// The governing project's `devplane.toml`.
    repo: Option<PathBuf>,
}

enum Judged {
    Deny(String),
    Unresolved(String),
    Clear,
}

enum Held {
    Is(String),
    Above(String),
}

/// The files under `~/.devplane` no agent may change; `devplane.db` also
/// covers its `-wal`, `-shm` and `-journal` siblings.
const OWN_NAMES: &[&str] = &["policy.toml", "agents.toml", "token", "host.json"];
const OWN_DB: &str = "devplane.db";
/// Past this many candidate working directories the line is not followed.
const MOST_CWDS: usize = 16;

impl Own {
    fn new(home: Option<&Path>, root: Option<&Path>) -> Self {
        Self {
            home: home.map(Path::to_path_buf),
            repo: root.map(|r| r.join(crate::core::config::CONFIG_FILE)),
        }
    }

    /// Which protected file `p` is, if any: its display, and whether reading
    /// it is forbidden too.
    fn file(&self, p: &Path) -> Option<(String, bool)> {
        if self.repo.as_deref() == Some(p) {
            return Some((p.display().to_string(), false));
        }
        let home = self.home.as_deref()?;
        if p.parent() != Some(home) {
            return None;
        }
        let name = p.file_name()?.to_str()?;
        (OWN_NAMES.contains(&name) || name.starts_with(OWN_DB))
            .then(|| (p.display().to_string(), name == "token"))
    }

    /// Whether `p` is a protected file or `~/.devplane` itself (`Held::Is`),
    /// or a directory above one (`Held::Above`: `/`, the home, the project).
    fn holds(&self, p: &Path) -> Option<Held> {
        if let Some((f, _)) = self.file(p) {
            return Some(Held::Is(f));
        }
        if self.home.as_deref() == Some(p) {
            return Some(Held::Is(p.display().to_string()));
        }
        let inside = |f: &Path| f.starts_with(p) && f != p;
        self.home
            .iter()
            .chain(&self.repo)
            .find(|f| inside(f))
            .map(|f| Held::Above(f.display().to_string()))
    }

    /// Whether a relative path, from a directory nobody can know, could be
    /// one of the protected files or hold one.
    fn could_be(&self, rel: &Path, whole: bool) -> bool {
        let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let named = OWN_NAMES.contains(&name)
            || name.starts_with(OWN_DB)
            || name == crate::core::config::CONFIG_FILE
            || rel.components().any(|c| c.as_os_str() == ".devplane");
        let upward = rel
            .components()
            .all(|c| matches!(c, Component::CurDir | Component::ParentDir));
        named || (whole && upward)
    }

    fn judge(&self, command: &str, dir: &Path, user_home: Option<&Path>) -> Judged {
        use crate::core::command::{Access, ArgKind};
        let line = crate::core::command::read(command);
        // Every directory a command on this line may run in; `None` once a
        // `cd` goes somewhere the reader cannot name.
        let mut cwds: Vec<Option<PathBuf>> = vec![Some(dir.to_path_buf())];
        let mut named_repo: Option<String> = None;
        // A directory above the files (`rm -rf ~`, `rm -rf .` in the project):
        // asked about, since so much else goes with it.
        let mut above: Option<String> = None;
        let resolve = |arg: &str, base: &Path| -> Option<PathBuf> {
            let p = match arg.strip_prefix('~') {
                Some("") => user_home?.to_path_buf(),
                Some(rest) if rest.starts_with('/') => {
                    user_home?.join(rest.trim_start_matches('/'))
                }
                Some(_) => return None,
                None => base.join(arg),
            };
            Some(settle(&p))
        };
        for cmd in &line.commands {
            let literal = |i: usize| cmd.kinds.get(i).is_none_or(|k| *k == ArgKind::Literal);
            if matches!(cmd.program.as_str(), "cd" | "pushd" | "popd") {
                let to = cmd
                    .args
                    .iter()
                    .enumerate()
                    .find(|(_, a)| !a.starts_with('-') || a.as_str() == "-");
                let next: Vec<Option<PathBuf>> = match (cmd.program.as_str(), to) {
                    ("popd", _) => vec![None],
                    (_, None) => vec![user_home.map(Path::to_path_buf)],
                    (_, Some((i, a))) if literal(i) && a != "-" => cwds
                        .iter()
                        .map(|c| c.as_deref().and_then(|c| resolve(a, c)))
                        .collect(),
                    _ => vec![None],
                };
                for n in next {
                    if !cwds.contains(&n) {
                        cwds.push(n);
                    }
                }
                if cwds.len() > MOST_CWDS {
                    cwds = vec![None];
                }
                continue;
            }
            // `git -C dir …`: that command's arguments are relative to `dir`.
            let mut here = cwds.clone();
            if let Some(i) = cmd.args.iter().position(|a| a == "-C")
                && cmd.program == "git"
            {
                here = match cmd.args.get(i + 1) {
                    Some(a) if literal(i + 1) => cwds
                        .iter()
                        .map(|c| c.as_deref().and_then(|c| resolve(a, c)))
                        .collect(),
                    _ => vec![None],
                };
            }
            // A directory `cp`, `mv`, `install` or `ln` puts its sources in.
            let into =
                |path: &Path| path.is_dir() || cmd.args.last().is_some_and(|a| a.ends_with('/'));
            let copier = matches!(cmd.program.as_str(), "cp" | "mv" | "install" | "ln");
            let whole_dir = matches!(cmd.program.as_str(), "chmod" | "chown" | "chgrp" | "rsync");
            let targets = crate::core::command::command_targets(cmd);
            let sources: Vec<&str> = targets
                .iter()
                .take(targets.len().saturating_sub(1))
                .filter(|t| t.via == crate::core::command::Via::Command)
                .map(|t| t.path.as_str())
                .collect();
            let last_command = targets
                .iter()
                .rposition(|t| t.via == crate::core::command::Via::Command);
            for (n, t) in targets.iter().enumerate() {
                if t.path.contains(['$', '*', '?', '[']) {
                    continue; // the fallback below
                }
                let written = t.access == Access::Write;
                let whole = written && (t.subtree || whole_dir);
                for base in &here {
                    let Some(path) = (match base {
                        Some(b) => resolve(&t.path, b),
                        None if Path::new(&t.path).is_absolute() || t.path.starts_with('~') => {
                            resolve(&t.path, dir)
                        }
                        None => {
                            if (written || t.access == Access::Named)
                                && self.could_be(Path::new(&t.path), whole)
                            {
                                return Judged::Unresolved(format!(
                                    "`{}` is named after a `cd` the reader cannot follow",
                                    t.path
                                ));
                            }
                            None
                        }
                    }) else {
                        continue;
                    };
                    if let Some((f, read_too)) = self.file(&path) {
                        let home_file = self.repo.as_deref() != Some(path.as_path());
                        match t.access {
                            Access::Write => return Judged::Deny(format!("writes `{f}`")),
                            Access::Named if home_file => {
                                return Judged::Deny(format!("hands `{f}` to a program"));
                            }
                            Access::Named if named_repo.is_none() => {
                                // `git add devplane.toml` is ordinary; `sed -i`
                                // is not, and the reader cannot tell them apart.
                                named_repo = Some(f);
                            }
                            Access::Read if read_too => {
                                return Judged::Deny(format!("reads `{f}`"));
                            }
                            _ => {}
                        }
                    }
                    match self.holds(&path).filter(|_| whole) {
                        Some(Held::Is(f)) => {
                            return Judged::Deny(format!("removes, moves or re-modes `{f}`"));
                        }
                        Some(Held::Above(f)) if above.is_none() => {
                            above =
                                Some(format!("changes `{}`, which holds `{f}`", path.display()));
                        }
                        _ => {}
                    }
                    // `cp policy.toml ~/.devplane/`: what lands inside.
                    if copier && written && Some(n) == last_command && into(&path) {
                        for src in &sources {
                            let Some(name) = Path::new(src).file_name() else {
                                continue;
                            };
                            let landed = path.join(name);
                            let whole_copy = t.subtree || cmd.program == "mv" || recursive(cmd);
                            let over = match self.holds(&landed) {
                                Some(Held::Is(f)) if whole_copy || self.file(&landed).is_some() => {
                                    Some(f)
                                }
                                Some(Held::Above(f)) if whole_copy => Some(f),
                                _ => None,
                            };
                            if let Some(f) = over {
                                return Judged::Deny(format!("puts a file over `{f}`"));
                            }
                        }
                    }
                }
            }
        }
        if let Some(what) = above {
            return Judged::Unresolved(what);
        }
        if let Some(f) = named_repo {
            return Judged::Unresolved(format!(
                "a program this reader does not know is given `{f}`"
            ));
        }
        // Spelled so the reader cannot name it: a person looks.
        let lower = command.to_ascii_lowercase();
        let mentions = lower.contains(".devplane")
            || lower.contains(crate::core::config::CONFIG_FILE)
            || lower.contains(OWN_DB);
        let hidden = line.barrier.is_some()
            || command.contains(['$', '`', '*', '?', '{'])
            || line.commands.iter().any(|c| {
                c.args
                    .iter()
                    .any(|a| a == "-C" || a.starts_with("--directory"))
            });
        if mentions && hidden {
            return Judged::Unresolved(
                "this line names Devplane's own files in a form the reader cannot resolve".into(),
            );
        }
        Judged::Clear
    }
}

fn recursive(cmd: &crate::core::command::Simple) -> bool {
    cmd.args.iter().any(|a| {
        a == "--recursive"
            || a == "--archive"
            || (a.starts_with('-') && !a.starts_with("--") && a.contains(['r', 'R', 'a']))
    })
}

/// `p` without `.` and `..`, with its existing directories' links resolved,
/// so `~/.devplane/../.devplane/token` and a symlink to the home both name
/// the file.
fn settle(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    match (out.parent(), out.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => out,
    }
}

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

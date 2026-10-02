//! Git: worktrees, status, digests and diffs, all through the `git` command.
//!
//! Using the CLI keeps Devplane's worktrees identical to Claude Code's and the
//! user's (`.claude/worktrees/<name>`), so Claude's own cleanup handles both.
//! Every ref or path from a person or a repository goes after
//! `--end-of-options`: `base_branch` comes from a committed file, and a value
//! like `--output=/tmp/x` would otherwise be an option. Config validation is
//! the first lock; this is the second.

use crate::core::change::{CommitStamp, Reach};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where isolated checkouts live, relative to the repository root.
pub const WORKTREE_DIR: &str = ".claude/worktrees";

/// How many files a refusal names before it says *and n more*.
const NAMED: usize = 10;

/// What `git status` says, in the shape the board needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub branch: Option<String>,
    /// The commit `HEAD` points at; `None` when the repository has no commits
    /// yet (`# branch.oid (initial)`).
    pub commit: Option<String>,
    pub changed_files: usize,
    pub untracked_files: usize,
    pub ahead: u32,
    pub behind: u32,
    pub has_conflicts: bool,
}

/// One read of `git status`: counts for the board and paths for refusals, from
/// the same call so they cannot disagree.
struct Porcelain {
    status: Status,
    /// Tracked files staged, unstaged or in conflict; a rename contributes both names.
    tracked: Vec<String>,
    untracked: Vec<String>,
}

/// How long any one git command may take. Generous — a first `worktree add`
/// of a large repository checks out every file — but finite, so a hung
/// command cannot stop the poller that called it.
pub const GIT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(300);

/// Runs a git command in a directory and returns stdout.
///
/// `GIT_OPTIONAL_LOCKS=0` on every call: a read such as `git status` otherwise
/// refreshes and rewrites `.git/index`, and the poller reads every open
/// change's tree on a timer, so the person's or agent's own `git commit` would
/// fail at random on `index.lock`. Writes (`worktree add`, `branch -D`) take
/// the locks they need regardless; the variable only drops the optional ones.
///
/// A checkout's `.git/config` is writable by the agent in it, so nothing it
/// names runs here: `core.fsmonitor` is forced off (it is a command git would
/// start on every `status`), git never prompts for a credential, and a call
/// that has not answered in [`GIT_DEADLINE`] is killed.
async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let run = tokio::process::Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(GIT_DEADLINE, run)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "git {} did not answer in {}s",
                args.join(" "),
                GIT_DEADLINE.as_secs()
            )
        })?
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The `origin` remote's URL, when there is one; needed to name the repository
/// even when no telemetry has reported it.
pub async fn remote_url(root: &Path) -> Option<String> {
    let out = git(root, &["remote", "get-url", "origin"]).await.ok()?;
    let url = out.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// The remote a forge is read from: `origin`, else the first remote git lists.
///
/// `git remote get-url` applies `url.<base>.insteadOf` rewrites itself, so the
/// URL returned is the one git would fetch from.
pub async fn forge_remote_url(root: &Path) -> Option<String> {
    if let Some(url) = remote_url(root).await {
        return Some(url);
    }
    let names = git(root, &["remote"]).await.ok()?;
    let first = names.lines().map(str::trim).find(|l| !l.is_empty())?;
    let out = git(root, &["remote", "get-url", first]).await.ok()?;
    let url = out.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// The branch a repository considers its trunk.
///
/// `origin/HEAD` when the remote says so, else the current branch. Guessing
/// `main` would branch from the wrong place in repos still on `master`.
pub async fn base_branch(root: &Path) -> String {
    if let Ok(s) = git(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .await
        && let Some(name) = s.trim().strip_prefix("origin/")
    {
        return name.to_string();
    }
    git(root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "main".into())
}

/// Reads status via porcelain v2, the format with a stability promise.
pub async fn status(dir: &Path) -> Result<Status> {
    porcelain(dir).await.map(|p| p.status)
}

/// The tracked files with uncommitted changes, as a refusal names them; the
/// same rule `create_worktree` refuses on.
pub async fn uncommitted(dir: &Path) -> Result<Option<String>> {
    let p = porcelain(dir).await?;
    Ok((!p.tracked.is_empty()).then(|| {
        format!(
            "uncommitted changes to {} tracked file(s): {}",
            p.tracked.len(),
            named(&p.tracked)
        )
    }))
}

/// [`uncommitted`], not counting the files in `except`: a change governed by
/// the checkout's own `devplane.toml` does not need that file committed, since
/// gates are read from the checkout and never from the worktree.
pub async fn uncommitted_except(dir: &Path, except: &[&str]) -> Result<Option<String>> {
    let p = porcelain(dir).await?;
    let tracked: Vec<String> = p
        .tracked
        .into_iter()
        .filter(|f| !except.contains(&f.as_str()))
        .collect();
    Ok((!tracked.is_empty()).then(|| {
        format!(
            "uncommitted changes to {} tracked file(s): {}",
            tracked.len(),
            named(&tracked)
        )
    }))
}

/// `-z`, so a path is bytes up to a NUL rather than a C-quoted string; a
/// rename's original name follows as its own record.
async fn porcelain(dir: &Path) -> Result<Porcelain> {
    let text = git(
        dir,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=normal",
            "-z",
        ],
    )
    .await?;
    Ok(parse_porcelain(&text))
}

fn parse_porcelain(text: &str) -> Porcelain {
    let mut p = Porcelain {
        status: Status::default(),
        tracked: Vec::new(),
        untracked: Vec::new(),
    };
    let s = &mut p.status;
    // The path follows a fixed number of space-separated fields: eight for a
    // change, nine for a rename, ten for a conflict.
    let path_after =
        |rec: &str, fields: usize| rec.splitn(fields + 1, ' ').nth(fields).map(str::to_string);
    let mut records = text.split('\0');
    while let Some(rec) = records.next() {
        if let Some(rest) = rec.strip_prefix("# branch.oid ") {
            let oid = rest.trim();
            // `(initial)` means no commits yet, not a commit id.
            s.commit = (oid != "(initial)").then(|| oid.to_string());
        } else if let Some(rest) = rec.strip_prefix("# branch.head ") {
            let head = rest.trim();
            // `(detached)` is not a branch name.
            s.branch = (head != "(detached)").then(|| head.to_string());
        } else if let Some(rest) = rec.strip_prefix("# branch.ab ") {
            let mut parts = rest.split_whitespace();
            s.ahead = parts
                .next()
                .and_then(|p| p.trim_start_matches('+').parse().ok())
                .unwrap_or(0);
            s.behind = parts
                .next()
                .and_then(|p| p.trim_start_matches('-').parse().ok())
                .unwrap_or(0);
        } else if let Some(path) = rec.strip_prefix("? ") {
            s.untracked_files += 1;
            p.untracked.push(path.to_string());
        } else if rec.starts_with("u ") {
            s.has_conflicts = true;
            s.changed_files += 1;
            p.tracked.extend(path_after(rec, 10));
        } else if rec.starts_with("1 ") {
            s.changed_files += 1;
            p.tracked.extend(path_after(rec, 8));
        } else if rec.starts_with("2 ") {
            s.changed_files += 1;
            p.tracked.extend(path_after(rec, 9));
            // The previous name is the next record.
            if let Some(from) = records.next().filter(|f| !f.is_empty()) {
                p.tracked.push(from.to_string());
            }
        }
    }
    p
}

/// What the tree is right now: commit, branch, cleanliness, working-tree
/// digest, and whether anyone else could fetch the commit.
///
/// The commit and the file counts come from one `git status`. *Clean* does
/// not: `status` trusts the index's stat cache and flags (`assume-unchanged`,
/// `skip-worktree`), which the agent in the checkout can set, so clean is the
/// digest being the commit's own tree — nothing on disk that the commit does
/// not record.
pub async fn commit_stamp(dir: &Path) -> Option<CommitStamp> {
    stamp(dir).await.map(|(s, _)| s)
}

/// [`commit_stamp`], with the digest it was made from (`None` when it could
/// not be read, as the stamp says), so two stamps can name what differs.
pub async fn stamp(dir: &Path) -> Option<(CommitStamp, Option<Digest>)> {
    let status = status(dir).await.ok()?;
    let reach = match &status.commit {
        Some(sha) => reach_of(dir, sha).await,
        // No commit: nothing to be reachable, unlike an unpushed one.
        None => Reach::NoRemote,
    };
    let digest = digest(dir).await;
    let remote = remote_url(dir).await;
    let clean = match (&digest, &status.commit) {
        (Ok(d), Some(sha)) => {
            let spec = format!("{sha}^{{tree}}");
            git(dir, &["rev-parse", "--verify", "--end-of-options", &spec])
                .await
                .is_ok_and(|t| t.trim() == d.tree)
        }
        // No commit yet: clean is nothing on disk to record.
        (Ok(d), None) => d.entries.is_empty(),
        (Err(_), _) => false,
    };
    let changed_files = (status.changed_files + status.untracked_files) as u32;
    let (tree, tree_unknown, digest) = match digest {
        Ok(d) => (Some(d.tree.clone()), None, Some(d)),
        Err(e) => (None, Some(format!("{e:#}")), None),
    };
    Some((
        CommitStamp {
            commit: status.commit,
            tree,
            tree_unknown,
            branch: status.branch,
            clean,
            changed_files,
            reach,
            remote,
        },
        digest,
    ))
}

/// A working tree as the digest read it: the tree, and each path's mode and
/// object id, so two reads can say which paths differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    pub tree: String,
    /// Path to `(mode, object id)`.
    pub entries: std::collections::BTreeMap<String, (String, String)>,
}

impl Digest {
    /// The paths added, removed or changed from `self` to `other`, in order.
    pub fn moved(&self, other: &Digest) -> Vec<String> {
        let mut out: Vec<String> = self
            .entries
            .iter()
            .filter(|(p, e)| other.entries.get(*p) != Some(e))
            .map(|(p, _)| p.clone())
            .chain(
                other
                    .entries
                    .keys()
                    .filter(|p| !self.entries.contains_key(*p))
                    .cloned(),
            )
            .collect();
        out.sort();
        out
    }
}

/// The git tree of the working tree as it is: tracked files as they are on
/// disk, untracked files that are not ignored, ignored files left out.
///
/// Computed with a temporary index, so nothing the person or agent sees
/// changes. The digest must not be steerable from inside the checkout, and
/// the checkout's `.git/config`, `.git/info/`, its index and every file in it
/// are writable by the agent in it, so:
/// * **every file's bytes are hashed as they are on disk**, with
///   `hash-object --no-filters`: no clean or `process` filter, no end-of-line
///   or encoding conversion, no `ident` — an attribute cannot make an edited
///   file hash as the old one, and no filter command runs in this process;
/// * git's stat cache is never trusted, so no `assume-unchanged`,
///   `skip-worktree`, `core.checkStat` or restored mtime can hide an edit.
///   Devplane keeps its own ([`BLOBS`]), keyed by what user space cannot set
///   back: the inode's change time;
/// * untracked files are excluded by the `.gitignore` files **committed at
///   `HEAD`** alone — never one only on disk (an untracked `sub/.gitignore`
///   holding `*` would hide itself and everything beside it), never
///   `.git/info/exclude` or `core.excludesFile`. An ignore pattern therefore
///   hides a file only once it is committed, where the diff shows it.
///   Devplane's own `.claude/worktrees/` is the one exception: other changes'
///   checkouts are not this tree;
/// * an embedded repository (a submodule, or a checkout inside this one)
///   records only the commit it has checked out, so one with files that
///   commit does not record has **no digest** — the error says which — rather
///   than a digest blind to them.
///
/// Where a repository declares no filter and has no uncommitted ignore rules,
/// this is the tree `git add -A && git write-tree` would give. Nothing is
/// written to the repository's object store; the copy of the index lives in
/// the git directory and is always removed.
pub async fn tree_digest(dir: &Path) -> Result<String> {
    Ok(digest(dir).await?.tree)
}

/// [`tree_digest`], with every entry.
pub async fn digest(dir: &Path) -> Result<Digest> {
    digest_into(dir, Objects::Nowhere).await
}

/// The digest, with every blob and tree it names written into the
/// repository's own object store, so the tree can be committed as it is.
/// Writes objects only: no ref, index or file of the person's moves.
pub async fn digest_stored(dir: &Path) -> Result<Digest> {
    digest_into(dir, Objects::Repository).await
}

/// Where a digest writes the objects it hashes.
#[derive(Clone, Copy)]
enum Objects<'a> {
    /// Nowhere: only the ids are wanted.
    Nowhere,
    /// A scratch object directory (see [`Scratch`]), so a diff can read them
    /// back and the repository's store is never written.
    Scratch(&'a Scratch),
    /// The repository's own store, for a tree that is about to be committed.
    Repository,
}

impl Objects<'_> {
    fn writes(self) -> bool {
        !matches!(self, Objects::Nowhere)
    }
}

/// The digest, writing its blobs and tree where `objects` says.
async fn digest_into(dir: &Path, objects: Objects<'_>) -> Result<Digest> {
    let index = git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )
    .await?;
    let index = PathBuf::from(index.trim());
    let top = git(dir, &["rev-parse", "--show-toplevel"]).await?;
    let top = PathBuf::from(top.trim());
    let tmp = index.with_file_name(format!("devplane-index-{}", scratch_suffix()));
    let result = digest_with(&top, &index, &tmp, objects).await;
    for t in [
        tmp.clone(),
        tmp.with_extension("lock"),
        tmp.with_extension("exclude"),
    ] {
        let _ = std::fs::remove_file(t);
    }
    result
}

/// A name no other read in this process or another is using.
fn scratch_suffix() -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{n}", std::process::id())
}

/// An object directory of its own, in front of the repository's: what the
/// digest writes goes here, what it reads comes from either, and the directory
/// is removed when this is dropped — the repository's store is never written.
struct Scratch {
    dir: PathBuf,
    /// The repository's own object directory, as git's alternate.
    alternate: String,
}

impl Scratch {
    async fn new(dir: &Path) -> Result<Self> {
        let objects = git(
            dir,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "objects",
            ],
        )
        .await?;
        let objects = PathBuf::from(objects.trim());
        let scratch = objects.with_file_name(format!("devplane-objects-{}", scratch_suffix()));
        std::fs::create_dir_all(scratch.join("info"))
            .with_context(|| format!("creating {}", scratch.display()))?;
        Ok(Scratch {
            dir: scratch,
            alternate: objects
                .to_str()
                .context("git directory is not utf-8")?
                .to_string(),
        })
    }

    fn env(&self) -> Vec<(&str, &str)> {
        vec![
            (
                "GIT_OBJECT_DIRECTORY",
                self.dir.to_str().unwrap_or_default(),
            ),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", self.alternate.as_str()),
        ]
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// One entry of the tree being built: its mode, and where its object id
/// comes from.
enum Entry {
    /// A regular file, hashed from disk by path, with what its cache entry
    /// is keyed by and the blob the index has for it.
    File(&'static str, Option<Stat>, Option<String>),
    /// A symbolic link: its target, hashed as the blob git stores for it.
    Link(Vec<u8>),
    /// A submodule or embedded repository, at the commit it has checked out.
    Gitlink(String),
}

/// What a file's cached blob id is keyed by: a file whose bytes changed has a
/// new change time (`ctime`), which — unlike the modification time — no
/// system call lets a process set back. Path, inode, size and both times, to
/// the nanosecond.
type Stat = (PathBuf, u64, u64, u64, i64, i64, i64, i64);

/// Blob ids of files already hashed, by [`Stat`]: a read of an unchanged
/// tree re-hashes nothing. Only files are cached, and only ones whose times
/// are not within the last two seconds, so a write in the same clock tick as
/// the hash (same size, same times) cannot keep a stale id.
static BLOBS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<Stat, String>>> =
    std::sync::LazyLock::new(Default::default);
/// Past this many files the cache starts again.
const MOST_BLOBS: usize = 500_000;

#[cfg(unix)]
fn stat_of(path: &Path, meta: &std::fs::Metadata) -> Option<Stat> {
    use std::os::unix::fs::MetadataExt;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    // Racily clean: written too recently to tell from a second write.
    if meta.mtime() >= now - 2 || meta.ctime() >= now - 2 {
        return None;
    }
    Some((
        path.to_path_buf(),
        meta.dev(),
        meta.ino(),
        meta.size(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    ))
}

#[cfg(not(unix))]
fn stat_of(_: &Path, _: &std::fs::Metadata) -> Option<Stat> {
    None
}

async fn digest_with(top: &Path, index: &Path, tmp: &Path, objects: Objects<'_>) -> Result<Digest> {
    let tmp_str = tmp.to_str().context("git directory is not utf-8")?;
    let mut env: Vec<(&str, &str)> = vec![("GIT_INDEX_FILE", tmp_str)];
    if let Objects::Scratch(scratch) = objects {
        env.extend(scratch.env());
    }
    let env = env.as_slice();
    // What is tracked is what the index says, staged additions included; the
    // index is copied so the real one is never locked or refreshed.
    let seeded = index.exists() && std::fs::copy(index, tmp).is_ok();
    let has_head = git(top, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .await
        .is_ok();
    if !seeded {
        let args: &[&str] = if has_head {
            &["read-tree", "HEAD"]
        } else {
            &["read-tree", "--empty"]
        };
        git_with_index(top, env, args, None).await?;
    }
    let staged = git_with_index(top, env, &["ls-files", "--stage", "-z"], None).await?;
    let mut tracked: Vec<(String, String, String)> = Vec::new();
    for rec in staged.split('\0').filter(|r| !r.is_empty()) {
        let Some((meta, path)) = rec.split_once('\t') else {
            continue;
        };
        let mut f = meta.split(' ');
        let (Some(mode), Some(oid)) = (f.next(), f.next()) else {
            continue;
        };
        // A conflicted path is listed once per stage; the file on disk is one.
        if tracked.last().is_none_or(|(_, _, p)| p != path) {
            tracked.push((mode.to_string(), oid.to_string(), path.to_string()));
        }
    }
    // Untracked files, excluded by the committed `.gitignore` files alone,
    // rewritten into one list: no `--exclude-per-directory`, which would read
    // whatever `.gitignore` is on disk, and no `--exclude-standard`, which
    // would read `info/exclude` and the person's global excludes file. An
    // embedded repository is listed as `dir/`.
    let exclude = tmp.with_extension("exclude");
    let rules = match has_head {
        true => committed_ignores(top, env).await?,
        false => String::new(),
    };
    std::fs::write(&exclude, rules).context("writing the committed ignore rules")?;
    let exclude_from = format!(
        "--exclude-from={}",
        exclude.to_str().context("git directory is not utf-8")?
    );
    let untracked = git_with_index(
        top,
        env,
        &[
            "ls-files",
            "--others",
            "-z",
            &exclude_from,
            "--exclude=/.claude/worktrees/",
        ],
        None,
    )
    .await?;

    let mut entries: Vec<(String, Entry)> = Vec::new();
    let candidates = tracked
        .iter()
        .map(|(mode, oid, path)| (path.clone(), Some((mode.as_str(), oid.as_str()))))
        .chain(
            untracked
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(|p| (p.trim_end_matches('/').to_string(), None)),
        );
    for (path, was) in candidates {
        let on_disk = top.join(&path);
        let Ok(meta) = std::fs::symlink_metadata(&on_disk) else {
            continue; // deleted
        };
        let gitlink =
            was.is_some_and(|(mode, _)| mode == "160000") || (was.is_none() && meta.is_dir());
        if gitlink {
            let head = git(&on_disk, &["rev-parse", "--verify", "--quiet", "HEAD"]).await;
            match (head, was) {
                (Ok(h), _) => {
                    embedded_is_committed(&on_disk, &path, h.trim()).await?;
                    entries.push((path, Entry::Gitlink(h.trim().to_string())))
                }
                // Not checked out: what the index says it should be.
                (Err(_), Some((_, oid))) => entries.push((path, Entry::Gitlink(oid.to_string()))),
                (Err(_), None) => {}
            }
        } else if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&on_disk)?;
            entries.push((
                path,
                Entry::Link(target.as_os_str().as_encoded_bytes().to_vec()),
            ));
        } else if meta.is_file() {
            let mode = file_mode(&meta, was.map(|(m, _)| m));
            let indexed = was.map(|(_, oid)| oid.to_string());
            entries.push((path, Entry::File(mode, stat_of(&on_disk, &meta), indexed)));
        }
        // A tracked path now a directory holds only untracked files, which
        // the untracked listing names on their own.
    }

    // Where objects are written they must exist: a cached id stands in only
    // when it is the blob the index has, which the repository already holds.
    let cached = |stat: &Option<Stat>, indexed: &Option<String>| -> Option<String> {
        let oid = BLOBS.lock().ok()?.get(stat.as_ref()?).cloned()?;
        (!objects.writes() || indexed.as_ref() == Some(&oid)).then_some(oid)
    };
    let mut hashed: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (path, entry) in &entries {
        if let Entry::File(_, stat, indexed) = entry
            && let Some(oid) = cached(stat, indexed)
        {
            hashed.insert(path.clone(), oid);
        }
    }
    // Regular files by path in one process; a path git's line-based input
    // cannot carry, and every link target, by content.
    let by_path: Vec<&str> = entries
        .iter()
        .filter(|(p, e)| {
            matches!(e, Entry::File(..)) && !p.contains('\n') && !hashed.contains_key(p)
        })
        .map(|(p, _)| p.as_str())
        .collect();
    let write: &[&str] = if objects.writes() { &["-w"] } else { &[] };
    if !by_path.is_empty() {
        let input = by_path.join("\n") + "\n";
        let args: Vec<&str> = ["hash-object", "--no-filters", "--stdin-paths"]
            .into_iter()
            .chain(write.iter().copied())
            .collect();
        let out = git_with_index(top, env, &args, Some(input.as_bytes())).await?;
        let oids: Vec<&str> = out.lines().collect();
        if oids.len() != by_path.len() {
            bail!(
                "git hash-object answered {} of {} paths",
                oids.len(),
                by_path.len()
            );
        }
        for (p, o) in by_path.iter().zip(oids) {
            hashed.insert((*p).to_string(), o.to_string());
        }
    }
    let mut info: Vec<u8> = Vec::new();
    let mut listed = std::collections::BTreeMap::new();
    for (path, entry) in &entries {
        let (mode, oid) = match entry {
            Entry::Gitlink(oid) => ("160000", oid.clone()),
            Entry::File(mode, stat, _) => match hashed.get(path) {
                Some(oid) => {
                    if let Some(stat) = stat.clone()
                        && let Ok(mut blobs) = BLOBS.lock()
                    {
                        if blobs.len() >= MOST_BLOBS {
                            blobs.clear();
                        }
                        blobs.insert(stat, oid.clone());
                    }
                    (*mode, oid.clone())
                }
                None => (
                    *mode,
                    hash_bytes(top, env, write, &std::fs::read(top.join(path))?).await?,
                ),
            },
            Entry::Link(target) => ("120000", hash_bytes(top, env, write, target).await?),
        };
        info.extend_from_slice(format!("{mode} {oid}\t").as_bytes());
        info.extend_from_slice(path.as_bytes());
        info.push(0);
        listed.insert(path.clone(), (mode.to_string(), oid));
    }
    // A fresh index holding exactly those entries.
    std::fs::remove_file(tmp).ok();
    git_with_index(top, env, &["read-tree", "--empty"], None).await?;
    if !info.is_empty() {
        git_with_index(
            top,
            env,
            &["update-index", "-z", "--index-info"],
            Some(&info),
        )
        .await?;
    }
    let tree = git_with_index(top, env, &["write-tree", "--missing-ok"], None).await?;
    let tree = tree.trim().to_string();
    if tree.is_empty() {
        bail!("git write-tree printed nothing");
    }
    Ok(Digest {
        tree,
        entries: listed,
    })
}

/// Refuses an embedded repository with files its checked-out commit does not
/// record: the tree records only that commit, so a digest would be blind to
/// them. Read with the same digest, never `git status`, whose filters the
/// embedded repository's own configuration names.
///
/// Boxed, with its `Send` declared: it recurses through [`tree_digest`].
fn embedded_is_committed<'a>(
    dir: &'a Path,
    path: &'a str,
    head: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let inner = tree_digest(dir)
            .await
            .with_context(|| format!("the embedded repository `{path}` could not be read"))?;
        let spec = format!("{head}^{{tree}}");
        let committed = git(dir, &["rev-parse", "--verify", "--end-of-options", &spec]).await?;
        if committed.trim() != inner {
            bail!(
                "the embedded repository `{path}` holds files its commit does not record, which \
                 the digest cannot see; commit them there, or remove it"
            );
        }
        Ok(())
    })
}

/// Every `.gitignore` committed at `HEAD`, rewritten into one list of rules
/// relative to the top of the tree, shallowest first — git's own precedence,
/// where a deeper file overrides and the last match wins.
async fn committed_ignores(top: &Path, env: &[(&str, &str)]) -> Result<String> {
    let listing = git(top, &["ls-tree", "-r", "-z", "--full-tree", "HEAD"]).await?;
    let mut files: Vec<(String, String)> = listing
        .split('\0')
        .filter_map(|rec| {
            let (meta, path) = rec.split_once('\t')?;
            let mut f = meta.split(' ');
            let (_, kind, oid) = (f.next()?, f.next()?, f.next()?);
            let named = path == ".gitignore" || path.ends_with("/.gitignore");
            (kind == "blob" && named).then(|| (path.to_string(), oid.to_string()))
        })
        .collect();
    files.sort_by_key(|(p, _)| (p.matches('/').count(), p.clone()));
    let mut out = String::new();
    for (path, oid) in files {
        let text = git_with_index(top, env, &["cat-file", "blob", &oid], None).await?;
        let dir = path.strip_suffix(".gitignore").unwrap_or("");
        out.push_str(&rebase_ignores(dir.trim_end_matches('/'), &text));
    }
    Ok(out)
}

/// The rules of the `.gitignore` in `dir` (`""` for the top), rewritten to
/// mean the same from the top: a pattern with a slash is anchored to `dir`, one
/// without matches at any depth below it.
fn rebase_ignores(dir: &str, text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if dir.is_empty() {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let (neg, rule) = match line.strip_prefix('!') {
            Some(rest) => ("!", rest),
            None => ("", line),
        };
        let anchored = rule.trim_end().trim_end_matches('/').contains('/');
        let prefix: String = dir
            .chars()
            .enumerate()
            .flat_map(|(i, c)| {
                let escape =
                    matches!(c, '\\' | '*' | '?' | '[') || (i == 0 && matches!(c, '#' | '!'));
                escape.then_some('\\').into_iter().chain(std::iter::once(c))
            })
            .collect();
        match anchored {
            true => out.push_str(&format!(
                "{neg}{prefix}/{}\n",
                rule.strip_prefix('/').unwrap_or(rule)
            )),
            false => out.push_str(&format!("{neg}{prefix}/**/{rule}\n")),
        }
    }
    out
}

/// The mode git records for a regular file: executable by its bits where the
/// file system has them, else as the index had it.
fn file_mode(meta: &std::fs::Metadata, indexed: Option<&str>) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = indexed;
        if meta.permissions().mode() & 0o111 != 0 {
            "100755"
        } else {
            "100644"
        }
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        if indexed == Some("100755") {
            "100755"
        } else {
            "100644"
        }
    }
}

/// The blob id of `bytes`, unfiltered; `write` is `["-w"]` to store it.
async fn hash_bytes(
    top: &Path,
    env: &[(&str, &str)],
    write: &[&str],
    bytes: &[u8],
) -> Result<String> {
    let args: Vec<&str> = ["hash-object", "--no-filters", "--stdin"]
        .into_iter()
        .chain(write.iter().copied())
        .collect();
    let out = git_with_index(top, env, &args, Some(bytes)).await?;
    Ok(out.trim().to_string())
}

/// A git command against an index that is not the person's (`env` names it,
/// and a scratch object directory when there is one), optionally fed on
/// stdin. The file system monitor and untracked cache are off, so neither a
/// copied cache nor a daemon's answer stands in for looking at the files, and
/// every stat check the repository's config could relax is forced on.
async fn git_with_index(
    dir: &Path,
    env: &[(&str, &str)],
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<String> {
    let mut forced: Vec<&str> = vec![
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.untrackedCache=false",
        "-c",
        "core.trustctime=true",
        "-c",
        "core.checkStat=default",
        "-c",
        "core.ignoreStat=false",
    ];
    // Windows reports no executable bit, which is why git turns this off there.
    if cfg!(not(windows)) {
        forced.extend(["-c", "core.fileMode=true"]);
    }
    let full: Vec<&str> = forced.into_iter().chain(args.iter().copied()).collect();
    let out = git_stdin(dir, env, &full, input).await?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Runs git in `dir` with `env`, optionally fed on stdin, and returns what it
/// printed whatever its exit status. Never takes an optional lock (`add` must
/// not take the real index's lock or refresh it; status must not rewrite it).
async fn git_stdin(
    dir: &Path,
    env: &[(&str, &str)],
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<std::process::Output> {
    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .envs(env.iter().copied())
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        let bytes = bytes.to_vec();
        tokio::spawn(async move {
            let _ = stdin.write_all(&bytes).await;
        });
    }
    tokio::time::timeout(GIT_DEADLINE, child.wait_with_output())
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "git {} did not answer in {}s",
                args.join(" "),
                GIT_DEADLINE.as_secs()
            )
        })?
        .with_context(|| format!("running git {}", args.join(" ")))
}

/// Whether a commit exists anywhere but this machine.
///
/// A certificate pointing at a commit a reviewer cannot fetch is worse than
/// none. "No remote" is fine; "remote without the commit" is not.
async fn reach_of(dir: &Path, sha: &str) -> Reach {
    let Ok(remotes) = git(dir, &["remote"]).await else {
        return Reach::Unknown;
    };
    if remotes.trim().is_empty() {
        return Reach::NoRemote;
    }
    // Offline: local `refs/remotes` only. `--contains=` as one argument,
    // because the option's optional value would swallow `--end-of-options`.
    // The sha is git's own, from status.
    let contains = format!("--contains={sha}");
    match git(dir, &["branch", "--remotes", &contains]).await {
        Ok(out) if !out.trim().is_empty() => Reach::Remote,
        Ok(_) => Reach::LocalOnly,
        // An unknown object is "could not tell", not "not pushed".
        Err(_) => Reach::Unknown,
    }
}

/// A worktree that exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
}

pub async fn list(root: &Path) -> Result<Vec<Worktree>> {
    let text = git(root, &["worktree", "list", "--porcelain"]).await?;
    let mut out = Vec::new();
    let mut current: Option<PathBuf> = None;
    let mut branch = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(path) = current.take() {
                out.push(Worktree {
                    path,
                    branch: branch.take(),
                });
            }
            current = Some(PathBuf::from(p));
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.trim_start_matches("refs/heads/").to_string());
        }
    }
    if let Some(path) = current {
        out.push(Worktree { path, branch });
    }
    Ok(out)
}

/// Whether a change's checkout is still there.
///
/// git's registration outlives an `rm -rf`. A gate run in a missing directory
/// never ran, and the caller records it that way.
pub fn worktree_present(dir: &Path) -> bool {
    dir.is_dir() && dir.join(".git").exists()
}

/// Creates an isolated checkout branched from the base (not whatever is
/// checked out).
///
/// Refuses, before writing anything and naming the cause: an existing target,
/// uncommitted changes to tracked files, or a base that does not resolve.
/// Untracked files do not count, nor does `devplane.toml`, which governs from
/// the checkout.
pub async fn create_worktree(root: &Path, name: &str, branch: &str, base: &str) -> Result<PathBuf> {
    let dir = root.join(WORKTREE_DIR).join(name);
    refuse_occupied(&dir)?;

    // `devplane.toml` is read from the checkout, never from the worktree, so
    // a check declared and not yet committed still governs: it is not dirt.
    let dirty: Vec<String> = porcelain(root)
        .await?
        .tracked
        .into_iter()
        .filter(|f| f != crate::core::config::CONFIG_FILE)
        .collect();
    if !dirty.is_empty() {
        bail!(
            "{} has uncommitted changes to {} tracked file(s): {}. Commit or stash them before \
             isolating work",
            root.display(),
            dirty.len(),
            named(&dirty)
        );
    }

    let Some(base_ref) = resolve_base(root, base).await else {
        bail!(
            "the base branch `{base}` does not exist in {}, neither locally nor as `origin/{base}`",
            root.display()
        );
    };

    // Keep `?? .claude/` out of the person's `git status`, as the vendor
    // documents. Best effort.
    if let Err(e) = exclude_worktrees(root).await {
        tracing::warn!(error = %e, "could not add .claude/worktrees/ to .git/info/exclude");
    }

    let parent = dir
        .parent()
        .with_context(|| format!("{} has no parent directory", dir.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", dir.display()))?;
    git(
        root,
        &[
            "worktree",
            "add",
            "-b",
            branch,
            "--end-of-options",
            dir.to_str().context("worktree path is not utf-8")?,
            &base_ref,
        ],
    )
    .await?;
    Ok(dir)
}

/// Refuses an existing target, saying whether it is a file, directory or
/// symlink. `symlink_metadata` so a dangling symlink counts as occupied.
fn refuse_occupied(dir: &Path) -> Result<()> {
    if let Ok(meta) = dir.symlink_metadata() {
        let what = if meta.file_type().is_symlink() {
            "a symlink"
        } else if meta.is_dir() {
            "a directory"
        } else {
            "a file"
        };
        bail!("{} already exists and is {what}", dir.display());
    }
    Ok(())
}

/// Whether a local branch of this name exists.
pub async fn branch_exists(root: &Path, branch: &str) -> bool {
    let spec = format!("refs/heads/{branch}");
    git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )
    .await
    .is_ok()
}

/// The worktree that has this branch checked out, when one does.
pub async fn worktree_of_branch(root: &Path, branch: &str) -> Option<PathBuf> {
    list(root)
        .await
        .ok()?
        .into_iter()
        .find(|w| w.branch.as_deref() == Some(branch))
        .map(|w| w.path)
}

/// Checks an existing branch out into its own worktree; the same refusals as
/// [`create_worktree`] minus the base.
pub async fn adopt_worktree(root: &Path, name: &str, branch: &str) -> Result<PathBuf> {
    let dir = root.join(WORKTREE_DIR).join(name);
    refuse_occupied(&dir)?;
    if let Err(e) = exclude_worktrees(root).await {
        tracing::warn!(error = %e, "could not add .claude/worktrees/ to .git/info/exclude");
    }
    let parent = dir
        .parent()
        .with_context(|| format!("{} has no parent directory", dir.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", dir.display()))?;
    git(
        root,
        &[
            "worktree",
            "add",
            "--end-of-options",
            dir.to_str().context("worktree path is not utf-8")?,
            branch,
        ],
    )
    .await?;
    Ok(dir)
}

/// The subject of the first commit a branch made after leaving its base, for
/// a title nobody typed. `None` when the branch has no commits of its own.
pub async fn first_subject(root: &Path, base: &str, branch: &str) -> Option<String> {
    let base_ref = resolve_base(root, base).await?;
    let range = format!("{base_ref}..{branch}");
    git(
        root,
        &[
            "log",
            "--reverse",
            "--format=%s",
            "--end-of-options",
            &range,
        ],
    )
    .await
    .ok()?
    .lines()
    .next()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// The commits a branch has that `base` does not, one line each. Empty means
/// merged; `None` means "could not tell".
///
/// `base` is the project's configured base, never rediscovered here. A squash
/// merge leaves commits unmerged by this measure; the caller trusts the forge.
pub async fn unmerged_commits(root: &Path, branch: &str, base: &str) -> Option<Vec<String>> {
    let base = resolve_base(root, base).await?;
    let range = format!("{base}..{branch}");
    git(root, &["log", "--format=%h %s", "--end-of-options", &range])
        .await
        .ok()
        .map(|o| o.lines().map(str::to_string).collect())
}

/// Deletes a branch. The caller must first establish it is merged: git's `-d`
/// compares with the checked-out branch, not the base.
pub async fn delete_branch(root: &Path, branch: &str) -> Result<()> {
    git(root, &["branch", "-D", "--end-of-options", branch]).await?;
    Ok(())
}

/// Up to [`NAMED`] entries, then a count of the rest.
fn named(items: &[String]) -> String {
    let shown: Vec<&str> = items.iter().take(NAMED).map(String::as_str).collect();
    match items.len().saturating_sub(NAMED) {
        0 => shown.join(", "),
        more => format!("{}, and {more} more", shown.join(", ")),
    }
}

/// Whether `path` exists in the tree of `rev` — `git cat-file -e rev:path`.
pub async fn exists_at(root: &Path, rev: &str, path: &str) -> bool {
    let spec = format!("{rev}:{}", path.trim_end_matches('/'));
    git(root, &["cat-file", "-e", "--end-of-options", &spec])
        .await
        .is_ok()
}

/// Refuses a spec the worktree would not have: one not committed (or not
/// pushed) to the base the worktree is made from would never reach the agent.
pub async fn refuse_spec_missing_from_base(root: &Path, base: &str, spec: &str) -> Result<()> {
    let Some(base_ref) = resolve_base(root, base).await else {
        bail!(
            "the base branch `{base}` does not exist in {}, neither locally nor as `origin/{base}`",
            root.display()
        );
    };
    if !exists_at(root, &base_ref, spec).await {
        bail!(
            "`{spec}` is not in `{base_ref}`, which the change's worktree is made from, so the \
             agent would never see it. Commit it{} first, or start the change in place",
            if base_ref.starts_with("origin/") {
                " and push it"
            } else {
                ""
            }
        );
    }
    Ok(())
}

/// How many commits `HEAD` has that the base does not.
pub async fn commits_ahead(dir: &Path, base: &str) -> Result<u32> {
    let base_ref = resolve_base(dir, base)
        .await
        .with_context(|| format!("the base branch `{base}` does not exist here"))?;
    let range = format!("{base_ref}..HEAD");
    let n = git(dir, &["rev-list", "--count", "--end-of-options", &range]).await?;
    n.trim()
        .parse()
        .with_context(|| format!("git rev-list printed {n:?}"))
}

/// The tree `HEAD` records, or `None` with no commit.
pub async fn head_tree(dir: &Path) -> Option<String> {
    git(
        dir,
        &["rev-parse", "--verify", "--end-of-options", "HEAD^{tree}"],
    )
    .await
    .ok()
    .map(|t| t.trim().to_string())
}

/// Commits `tree` on top of the checkout's `HEAD` as one commit, moves the
/// branch `HEAD` names to it, and resets that checkout's index to it, so the
/// files on disk — which the tree was read from — are clean against it. The
/// objects must already be in the store ([`digest_stored`]). Only for a
/// change's own worktree: the person's checkout is never passed here.
pub async fn commit_tree_on_head(dir: &Path, tree: &str, message: &str) -> Result<String> {
    let head = git(
        dir,
        &["rev-parse", "--verify", "--end-of-options", "HEAD^{commit}"],
    )
    .await
    .context("the worktree has no commit to build on")?;
    let head = head.trim().to_string();
    let branch = git(dir, &["symbolic-ref", "--quiet", "HEAD"])
        .await
        .context("the worktree is not on a branch, so there is nothing to commit onto")?;
    let branch = branch.trim().to_string();
    let run = tokio::process::Command::new("git")
        .args([
            "-c",
            "core.fsmonitor=false",
            "commit-tree",
            tree,
            "-p",
            &head,
            "-F",
            "-",
        ])
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("starting git commit-tree")?;
    let mut run = run;
    if let Some(mut stdin) = run.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(message.as_bytes()).await?;
    }
    let out = tokio::time::timeout(GIT_DEADLINE, run.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("git commit-tree did not answer"))??;
    if !out.status.success() {
        bail!(
            "git commit-tree failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let commit = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // Compare-and-swap: a branch that moved since `head` was read is not
    // overwritten.
    git(
        dir,
        &[
            "update-ref",
            "-m",
            "devplane: offer the verified tree",
            &branch,
            &commit,
            &head,
        ],
    )
    .await
    .context("moving the branch to the new commit")?;
    // The index follows; the files on disk are the tree already.
    git(dir, &["read-tree", "--reset", &commit])
        .await
        .context("resetting the worktree's index to the new commit")?;
    Ok(commit)
}

/// Prefers `origin/<base>`, so new work starts from what was pushed. `None`
/// when neither resolves to a commit.
async fn resolve_base(root: &Path, base: &str) -> Option<String> {
    for candidate in [format!("origin/{base}"), base.to_string()] {
        let spec = format!("{candidate}^{{commit}}");
        if git(
            root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &spec,
            ],
        )
        .await
        .is_ok()
        {
            return Some(candidate);
        }
    }
    None
}

/// Appends `.claude/worktrees/` to `.git/info/exclude`, once.
///
/// Not `.gitignore`, which is tracked and would need committing. Resolved via
/// `--git-common-dir`, because a linked worktree or submodule keeps `info/` in
/// the repository it belongs to.
async fn exclude_worktrees(root: &Path) -> Result<bool> {
    const ENTRY: &str = ".claude/worktrees/";
    let common = git(root, &["rev-parse", "--git-common-dir"]).await?;
    let common = PathBuf::from(common.trim());
    let common = if common.is_absolute() {
        common
    } else {
        root.join(common)
    };
    let info = common.join("info");
    std::fs::create_dir_all(&info).with_context(|| format!("creating {}", info.display()))?;
    let file = info.join("exclude");
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    let covered = existing
        .lines()
        .map(str::trim)
        .any(|l| l == ENTRY || l == ENTRY.trim_end_matches('/') || l == ".claude/");
    if covered {
        return Ok(false);
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# Isolated checkouts made by Claude Code and Devplane.\n");
    text.push_str(ENTRY);
    text.push('\n');
    std::fs::write(&file, text).with_context(|| format!("writing {}", file.display()))?;
    Ok(true)
}

/// Removes a worktree, refusing to destroy uncommitted work.
///
/// Dirty and untracked files are named by path; `discard_uncommitted` removes
/// them anyway. The branch is never touched. A worktree whose directory is
/// already gone is pruned.
pub async fn remove_worktree(root: &Path, dir: &Path, discard_uncommitted: bool) -> Result<()> {
    // A present directory whose status cannot be read is not "clean": "could
    // not tell" must not read as "nothing there".
    let present = dir.exists();
    let dirty: Vec<String> = if present {
        match porcelain(dir).await {
            Ok(p) => p.tracked.into_iter().chain(p.untracked).collect(),
            Err(e) if discard_uncommitted => {
                tracing::warn!(
                    dir = %dir.display(),
                    %e,
                    "status unreadable; --discard-uncommitted was given"
                );
                Vec::new()
            }
            Err(e) => bail!(
                "cannot read the state of {}: {e}. Pass --discard-uncommitted to remove it anyway",
                dir.display()
            ),
        }
    } else {
        Vec::new()
    };

    if !dirty.is_empty() && !discard_uncommitted {
        bail!(
            "{} has uncommitted changes in {} file(s): {}. Pass --discard-uncommitted to \
             discard them",
            dir.display(),
            dirty.len(),
            named(&dirty)
        );
    }

    if !present {
        // `worktree remove` refuses a missing path; `prune` drops the stale
        // registration.
        git(root, &["worktree", "prune"]).await?;
        return Ok(());
    }
    let mut args = vec!["worktree", "remove"];
    if discard_uncommitted {
        args.push("--force");
    }
    args.push("--end-of-options");
    let path = dir.to_str().context("worktree path is not utf-8")?;
    args.push(path);
    git(root, &args).await?;
    Ok(())
}

/// Every file this checkout changed since it diverged from `base`, committed
/// or not, as a parsed diff plus the command that reproduces it.
///
/// Diffs against the merge base, so a `base` that moved on does not show
/// everybody else's commits. Uncommitted work is included: a reviewer
/// approves the checkout as it stands.
///
/// The other side is **the tree the digest reads** ([`tree_digest`]), written
/// with its blobs into a scratch object directory, never the working tree as
/// `git diff` sees it: `git diff` trusts the index's `assume-unchanged` and
/// `skip-worktree` flags and `.git/info/exclude`, all of which the agent can
/// set, so what is reviewed is exactly the bytes a pass was verified on.
///
/// Every failure is returned — a base that resolves neither as `origin/<base>`
/// nor locally, no merge base, a diff git refused — because an unreadable diff
/// is not an empty one: whatever makes a claim from it (the offer's guard, the
/// certificate's "altered none of its checks") must say it could not read it.
///
/// The base resolves the way [`create_worktree`] resolves it, `origin/` first,
/// so the diff is against what the checkout was made from. The diff is
/// plumbing (`diff-tree`), independent of the person's diff config (external
/// drivers, textconv, prefixes), which the parser would otherwise misread, and
/// of every attribute git lets a caller turn off — the committed and system
/// ones and `core.attributesFile`. `.git/info/attributes` cannot be turned
/// off, so a file git calls binary is read again with `--text` and kept as
/// text when it has no NUL byte; one still binary is named by the weakened
/// checks as not readable when it is a test or defines a check.
pub async fn change_set(dir: &Path, base: &str) -> Result<crate::core::diff::ChangeSet> {
    let (base_ref, from) = merge_base(dir, base).await?;
    change_set_from(dir, &from, base, &base_ref).await
}

/// Where this checkout left `base`: the spelling that resolved (`origin/`
/// first, as [`change_set`] resolves it) and the merge base's commit. An
/// unknown base names both spellings tried.
pub async fn merge_base(dir: &Path, base: &str) -> Result<(String, String)> {
    let base_ref = resolve_base(dir, base).await.with_context(|| {
        format!("the base branch `{base}` resolves neither as `origin/{base}` nor locally")
    })?;
    // One diff from the merge base to the working tree, so a file committed
    // then edited (or deleted) is listed once, as it stands.
    let from = git(dir, &["merge-base", "--end-of-options", &base_ref, "HEAD"])
        .await
        .with_context(|| format!("finding where this checkout left `{base_ref}`"))?;
    let from = from.trim().to_string();
    if from.is_empty() {
        bail!("`{base_ref}` and HEAD share no history");
    }
    Ok((base_ref, from))
}

/// The spelling of `base` that names a commit here, `origin/<base>` first;
/// `None` when neither does.
pub async fn resolve_commitish(dir: &Path, base: &str) -> Option<String> {
    resolve_base(dir, base).await
}

/// The branch `origin/HEAD` names, without `origin/`; `None` when the remote
/// names none. Never the current branch: a default that is the checkout itself
/// compares a change with nothing.
pub async fn default_branch(dir: &Path) -> Option<String> {
    let s = git(
        dir,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .await
    .ok()?;
    s.trim().strip_prefix("origin/").map(str::to_string)
}

/// The top of the working tree `dir` is in; `None` outside a repository.
pub async fn toplevel(dir: &Path) -> Option<PathBuf> {
    let s = git(dir, &["rev-parse", "--show-toplevel"]).await.ok()?;
    let p = PathBuf::from(s.trim());
    (!s.trim().is_empty()).then_some(p)
}

/// Whether this checkout is a shallow clone, whose history may not reach a
/// merge base.
pub async fn is_shallow(dir: &Path) -> bool {
    git(dir, &["rev-parse", "--is-shallow-repository"])
        .await
        .is_ok_and(|s| s.trim() == "true")
}

/// The commit `rev` resolves to, in full.
pub async fn commit_of(dir: &Path, rev: &str) -> Result<String> {
    let spec = format!("{rev}^{{commit}}");
    Ok(
        git(dir, &["rev-parse", "--verify", "--end-of-options", &spec])
            .await?
            .trim()
            .to_string(),
    )
}

/// A file's bytes as committed at `commit`; `None` when the commit has no such
/// path. Read from the object store, never the working tree.
pub async fn read_at(dir: &Path, commit: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let spec = format!("{commit}:{path}");
    // `cat-file -e` separates *absent* from *unreadable*.
    let exists = tokio::process::Command::new("git")
        .args([
            "-c",
            "core.fsmonitor=false",
            "cat-file",
            "-e",
            "--end-of-options",
            &spec,
        ])
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .context("running git cat-file")?;
    if !exists.success() {
        commit_of(dir, commit).await?;
        return Ok(None);
    }
    let out = tokio::process::Command::new("git")
        .args([
            "-c",
            "core.fsmonitor=false",
            "cat-file",
            "blob",
            "--end-of-options",
            &spec,
        ])
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .context("running git cat-file")?;
    if !out.status.success() {
        bail!(
            "git cat-file blob {spec} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(Some(out.stdout))
}

/// The blob id of `path` at `commit`, for naming a definition exactly.
pub async fn blob_at(dir: &Path, commit: &str, path: &str) -> Option<String> {
    let spec = format!("{commit}:{path}");
    git(dir, &["rev-parse", "--verify", "--end-of-options", &spec])
        .await
        .ok()
        .map(|s| s.trim().to_string())
}

/// [`change_set`] from a merge base already found: the diff from `from` to
/// the working tree, untracked files included. `label` is what the set names
/// as its base; `base_ref` the spelling a person re-runs the diff with.
pub async fn change_set_from(
    dir: &Path,
    from: &str,
    label: &str,
    base_ref: &str,
) -> Result<crate::core::diff::ChangeSet> {
    let base = label;
    let from = from.to_string();
    let scratch = Scratch::new(dir).await?;
    let tree = digest_into(dir, Objects::Scratch(&scratch)).await?.tree;
    // The empty tree (stdin is empty): attributes are read from it, so none
    // from the tree itself.
    let empty = git(dir, &["hash-object", "-t", "tree", "--stdin"]).await?;
    let empty = empty.trim();
    let mut env = scratch.env();
    env.extend([("GIT_ATTR_NOSYSTEM", "1"), ("GIT_ATTR_SOURCE", empty)]);
    let mut text = diff_tree(dir, &env, &from, &tree, &[]).await?;
    // A file git called binary is read again as text: `.git/info/attributes`,
    // which no option turns off, can call any file binary with `-diff`. Kept
    // as text only when it is text — no NUL byte, git's own test.
    let binary: Vec<String> = crate::core::diff::parse(base, &text, "")
        .uncut()
        .files
        .iter()
        .filter(|f| matches!(f.body, crate::core::diff::Body::Binary { .. }))
        .flat_map(|f| match &f.status {
            crate::core::diff::Status::Renamed { from } => vec![from.clone(), f.path.clone()],
            _ => vec![f.path.clone()],
        })
        .map(|p| format!(":(literal){p}"))
        .collect();
    if !binary.is_empty() {
        let again = diff_tree(dir, &env, &from, &tree, &binary).await?;
        let as_text: std::collections::HashMap<&str, &str> = file_chunks(&again)
            .into_iter()
            .filter(|c| !c.contains('\0'))
            .filter_map(|c| Some((c.lines().next()?, c)))
            .collect();
        text = file_chunks(&text)
            .into_iter()
            .map(|c| {
                c.lines()
                    .next()
                    .and_then(|header| as_text.get(header))
                    .copied()
                    .unwrap_or(c)
            })
            .collect();
    }
    // What a person runs: the index a reviewer has is not the digest's, so
    // status names the files `git diff` does not show.
    let command = format!("git diff $(git merge-base {base_ref} HEAD); git status --short");
    let mut set = crate::core::diff::parse(base, &text, &command);
    // `diff-tree` prints no size for a binary and the parser is pure, so stat
    // it here. A deleted binary stays just "binary".
    for f in &mut set.files {
        if let crate::core::diff::Body::Binary { bytes } = &mut f.body {
            *bytes = tokio::fs::metadata(dir.join(&f.path))
                .await
                .ok()
                .map(|m| m.len());
        }
    }
    drop(scratch);
    Ok(set)
}

/// A detached checkout of `commit` at `<root>/.claude/worktrees/<name>`,
/// inside the folder Devplane owns, for a run nobody works in.
pub async fn create_detached_worktree(root: &Path, name: &str, commit: &str) -> Result<PathBuf> {
    let dir = root.join(WORKTREE_DIR).join(name);
    refuse_occupied(&dir)?;
    if let Err(e) = exclude_worktrees(root).await {
        tracing::warn!(error = %e, "could not add .claude/worktrees/ to .git/info/exclude");
    }
    let parent = dir
        .parent()
        .with_context(|| format!("{} has no parent directory", dir.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", dir.display()))?;
    git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            "--end-of-options",
            dir.to_str().context("worktree path is not utf-8")?,
            commit,
        ],
    )
    .await?;
    Ok(dir)
}

/// Makes a checkout's index and files exactly `tree`, as git stores it:
/// symlinks, modes and deletions included.
pub async fn read_tree_reset(dir: &Path, tree: &str) -> Result<()> {
    git(dir, &["read-tree", "-u", "--reset", tree]).await?;
    Ok(())
}

/// Puts `paths` in a checkout as `treeish` has them.
pub async fn checkout_paths(dir: &Path, treeish: &str, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let specs: Vec<String> = paths.iter().map(|p| format!(":(literal){p}")).collect();
    let mut args: Vec<&str> = vec!["checkout", treeish, "--"];
    args.extend(specs.iter().map(String::as_str));
    git(dir, &args).await?;
    Ok(())
}

/// Removes everything git does not track, ignored files included.
pub async fn clean_all(dir: &Path) -> Result<()> {
    git(dir, &["clean", "-ffdx"]).await?;
    Ok(())
}

/// Whether git ignores `path` in `dir`; `None` when git could not say.
pub async fn is_ignored(dir: &Path, path: &str) -> Option<bool> {
    let out = git_stdin(
        dir,
        &[],
        &[
            "-c",
            "core.fsmonitor=false",
            "check-ignore",
            "-q",
            "--no-index",
            "--",
            path,
        ],
        None,
    )
    .await
    .ok()?;
    match out.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// `diff-tree` from `from` to `tree`, as a patch; with `paths`, only those,
/// read as text whatever git takes them for.
async fn diff_tree(
    dir: &Path,
    env: &[(&str, &str)],
    from: &str,
    tree: &str,
    paths: &[String],
) -> Result<String> {
    let attributes = format!("core.attributesFile={}", null_device());
    let mut args: Vec<&str> = vec![
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.quotePath=false",
        "-c",
        &attributes,
        "diff-tree",
        "-r",
        "-p",
        // A moved file reads as moved, not deleted and added.
        "-M",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ];
    if !paths.is_empty() {
        args.push("--text");
    }
    args.extend(["--end-of-options", from, tree]);
    if !paths.is_empty() {
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
    }
    let out = git_stdin(dir, env, &args, None).await?;
    if !out.status.success() {
        bail!(
            "git diff-tree failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// A patch split into one piece per file, each starting at its `diff --git`
/// line. Content lines carry a one-character prefix, so none starts a piece.
fn file_chunks(text: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = text
        .match_indices("diff --git ")
        .map(|(at, _)| at)
        .filter(|at| *at == 0 || text.as_bytes()[at - 1] == b'\n')
        .collect();
    if starts.first() != Some(&0) {
        starts.insert(0, 0);
    }
    starts
        .iter()
        .zip(
            starts
                .iter()
                .skip(1)
                .copied()
                .chain(std::iter::once(text.len())),
        )
        .map(|(a, b)| &text[*a..b])
        .filter(|c| !c.is_empty())
        .collect()
}

/// The file that reads as empty, for a setting that must name one.
fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// Files this branch touched, for detecting two changes editing the same file.
///
/// Three-dot against the merge base, so a moved `base` does not make every
/// change overlap every other.
pub async fn touched_files(dir: &Path, base: &str) -> Vec<String> {
    let mut out = Vec::new();
    let range = format!("{base}...HEAD");
    if let Ok(o) = git(dir, &["diff", "--name-only", "--end-of-options", &range]).await {
        out.extend(o.lines().map(str::to_string));
    }
    // Uncommitted work counts: two agents editing one file collide before
    // either commits.
    if let Ok(p) = porcelain(dir).await {
        out.extend(p.tracked);
        out.extend(p.untracked);
    }
    out.retain(|p| !p.is_empty());
    out.sort();
    out.dedup();
    out
}

/// The gitignored files `.worktreeinclude` names, repository-relative and
/// `/`-separated: the candidates a fresh checkout may carry. Empty when the
/// repository has no `.worktreeinclude`.
///
/// Git does the matching with its own gitignore engine rather than listing
/// every ignored file (a `target/` or `node_modules/` can hold hundreds of
/// thousands): `ls-files --others --ignored --exclude-from=.worktreeinclude`
/// names the untracked files the patterns select, and `check-ignore` keeps
/// those the repository's standard rules ignore. Neither step ever names a
/// tracked file, so "only gitignored files are copied" holds by construction.
pub async fn ignored_files(root: &Path) -> Result<Vec<String>> {
    let include = root.join(".worktreeinclude");
    if !include.is_file() {
        return Ok(Vec::new());
    }
    let from = format!(
        "--exclude-from={}",
        include.to_str().context("repository path is not utf-8")?
    );
    let chosen = git(root, &["ls-files", "--others", "--ignored", "-z", &from]).await?;
    let chosen: Vec<&str> = chosen.split('\0').filter(|p| !p.is_empty()).collect();
    if chosen.is_empty() {
        return Ok(Vec::new());
    }
    let input: Vec<u8> = chosen
        .iter()
        .flat_map(|p| p.bytes().chain(std::iter::once(0)))
        .collect();
    let ignored = check_ignore(root, &input).await?;
    Ok(ignored
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| p.replace('\\', "/"))
        .collect())
}

/// `git check-ignore -z --stdin`: the paths the standard rules ignore. Exit
/// status 1 means none are, which is an answer rather than a failure.
async fn check_ignore(root: &Path, input: &[u8]) -> Result<String> {
    let out = git_stdin(root, &[], &["check-ignore", "-z", "--stdin"], Some(input)).await?;
    match out.status.code() {
        Some(0) | Some(1) => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        _ => bail!(
            "git check-ignore failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// Copies the gitignored files `.worktreeinclude` chose into a fresh
/// worktree, never over anything already there and never outside it.
///
/// A repository can point a path elsewhere via a symlink, on either side:
/// * the source is canonicalised and must stay inside the repository;
/// * every directory on the way to the destination is walked one component at
///   a time and must be a real directory, never a symlink (a tracked
///   `config -> /tmp/public` in the worktree would otherwise receive
///   `config/local.env`); missing ones are created one by one;
/// * the destination's parent, canonicalised, must still be inside the
///   canonical worktree;
/// * the file is copied under a fresh name in that parent and hard-linked
///   into place, which fails rather than follows if anything, a dangling
///   symlink included, appeared at the destination.
///
/// `std::fs::copy` clones where the file system can (`clonefile` on APFS,
/// `copy_file_range` reflinks on Btrfs and XFS), so large files cost nothing.
pub fn copy_included(root: &Path, worktree: &Path, files: &[String]) -> Vec<String> {
    let mut copied = Vec::new();
    let (Ok(real_root), Ok(real_tree)) = (root.canonicalize(), worktree.canonicalize()) else {
        // Unresolvable is refused: a wrong answer could hand an agent a
        // private key, or write one somewhere public.
        return copied;
    };
    for pattern in files {
        let from = root.join(pattern);
        let to = worktree.join(pattern);
        if !within(root, &from) || !within(worktree, &to) {
            tracing::warn!(
                pattern,
                "refusing to copy a file from outside the repository"
            );
            continue;
        }
        if !from.is_file() {
            continue;
        }
        match from.canonicalize() {
            Ok(real) if real.starts_with(&real_root) => {}
            Ok(_) => {
                tracing::warn!(
                    pattern,
                    "refusing to copy a file that points outside the repository"
                );
                continue;
            }
            Err(_) => continue,
        }
        match copy_into(&real_tree, worktree, pattern, &from) {
            Ok(true) => copied.push(pattern.clone()),
            Ok(false) => {}
            Err(why) => tracing::warn!(pattern, why, "refusing to copy an included file"),
        }
    }
    copied
}

/// One confined copy; `Ok(false)` when something is already there.
fn copy_into(
    real_tree: &Path,
    worktree: &Path,
    relative: &str,
    from: &Path,
) -> std::result::Result<bool, &'static str> {
    use std::path::Component;
    // `a/../b` is `b`: resolved lexically (the caller has checked it stays
    // inside), so the walk below only ever steps down into named directories.
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for c in Path::new(relative).components() {
        match c {
            Component::Normal(c) => parts.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop().ok_or("the path climbs out of the worktree")?;
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("the path is not a relative path");
            }
        }
    }
    let Some((name, dirs)) = parts.split_last() else {
        return Err("the path names no file");
    };
    let mut parent = worktree.to_path_buf();
    {
        for c in dirs {
            parent.push(c);
            match parent.symlink_metadata() {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err("a directory on the way is a symlink");
                }
                Ok(m) if m.is_dir() => {}
                Ok(_) => return Err("a directory on the way is a file"),
                Err(_) => {
                    std::fs::create_dir(&parent).map_err(|_| "could not create a directory")?
                }
            }
        }
    }
    match parent.canonicalize() {
        Ok(real) if real.starts_with(real_tree) => {}
        _ => return Err("the destination resolves outside the worktree"),
    }
    let to = parent.join(name);
    if to.symlink_metadata().is_ok() {
        return Ok(false);
    }
    let tmp = parent.join(format!(".devplane-copy-{}", uuid::Uuid::new_v4().simple()));
    std::fs::copy(from, &tmp).map_err(|_| "the copy failed")?;
    // A hard link puts the whole copy in place at once and never follows a
    // link planted at `to`. Where the file system has none (exFAT, FAT, some
    // network shares), `create_new` refuses the same things: a file, or a
    // symlink, already there.
    let placed = match std::fs::hard_link(&tmp, &to) {
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => (|| {
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&to)?;
            let mut src = std::fs::File::open(&tmp)?;
            std::io::copy(&mut src, &mut out)?;
            std::fs::set_permissions(&to, std::fs::metadata(&tmp)?.permissions())
        })(),
        linked => linked,
    };
    std::fs::remove_file(&tmp).ok();
    match placed {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => Err("could not put the copy in place"),
    }
}

/// Whether `path` stays inside `root` once `..` and `.` are resolved.
///
/// Lexical, since the file may not exist yet. Absolute paths are outside:
/// joining one discards the root.
fn within(root: &Path, path: &Path) -> bool {
    use std::path::Component;
    let mut depth = 0i32;
    let Ok(rest) = path.strip_prefix(root) else {
        return false;
    };
    for c in rest.components() {
        match c {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            // A root or prefix in the tail means the join discarded the root.
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn scratch_repo(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vp-git-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            git(&dir, &args).await.unwrap();
        }
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        git(&dir, &["add", "-A"]).await.unwrap();
        git(&dir, &["commit", "-qm", "init"]).await.unwrap();
        // macOS temp dirs are symlinked; `git worktree list` prints resolved paths.
        dir.canonicalize().unwrap()
    }

    #[tokio::test]
    async fn the_verify_helpers_read_the_base_without_touching_the_checkout() {
        let root = scratch_repo("verify-helpers").await;
        // No remote: no default branch, and never the current one.
        assert_eq!(default_branch(&root).await, None);
        assert!(!is_shallow(&root).await);
        let head = commit_of(&root, "HEAD").await.unwrap();
        assert_eq!(head.len(), 40);
        assert_eq!(
            read_at(&root, &head, "a.txt").await.unwrap().as_deref(),
            Some(&b"hello\n"[..])
        );
        assert_eq!(read_at(&root, &head, "absent.toml").await.unwrap(), None);
        assert!(read_at(&root, "nosuchcommit", "a.txt").await.is_err());
        assert!(blob_at(&root, &head, "a.txt").await.is_some());

        let err = merge_base(&root, "nosuch").await.unwrap_err().to_string();
        assert!(
            err.contains("`origin/nosuch`") && err.contains("`nosuch`"),
            "{err}"
        );
        let (resolved, from) = merge_base(&root, "main").await.unwrap();
        assert_eq!((resolved.as_str(), from.as_str()), ("main", head.as_str()));

        std::fs::write(root.join("b.txt"), "new\n").unwrap();
        let via_base = change_set(&root, "main").await.unwrap();
        let via_from = change_set_from(&root, &from, "main", "main").await.unwrap();
        assert_eq!(via_base, via_from, "one diff, two entry points");
        assert_eq!(toplevel(&root).await.as_deref(), Some(root.as_path()));
        std::fs::remove_dir_all(&root).ok();
    }

    async fn porcelain_status(dir: &Path) -> String {
        git(dir, &["status", "--porcelain"]).await.unwrap()
    }

    /// The verified tree, written, is the digest; a throwaway detached
    /// checkout materialises it from objects, takes paths back from another
    /// tree, and is cleaned of what git does not track.
    #[tokio::test]
    async fn a_written_tree_is_the_digest_and_another_checkout_can_materialise_it() {
        let root = scratch_repo("written").await;
        let base = git(&root, &["rev-parse", "HEAD"])
            .await
            .unwrap()
            .trim()
            .to_string();
        std::fs::write(root.join(".gitignore"), "reports/\n").unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        git(&root, &["commit", "-qm", "ignore"]).await.unwrap();
        std::fs::write(root.join("a.txt"), "edited\n").unwrap();
        std::fs::write(root.join("new.txt"), "untracked\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("a.txt", root.join("link")).unwrap();
        let digest = tree_digest(&root).await.unwrap();
        let written = digest_stored(&root).await.unwrap();
        assert_eq!(
            written.tree, digest,
            "writing the objects does not change the tree"
        );
        assert_eq!(
            git(&root, &["cat-file", "-t", &written.tree])
                .await
                .unwrap()
                .trim(),
            "tree",
            "the tree is in the repository's store"
        );
        assert_eq!(
            merge_base(&root, "main").await.unwrap().1,
            git(&root, &["rev-parse", "HEAD"]).await.unwrap().trim()
        );

        let dir = create_detached_worktree(&root, ".devplane-base-test", &base)
            .await
            .unwrap();
        assert!(dir.starts_with(root.join(WORKTREE_DIR)));
        read_tree_reset(&dir, &written.tree).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "edited\n"
        );
        assert!(dir.join("new.txt").is_file());
        checkout_paths(&dir, &base, &["a.txt".to_string()])
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "hello\n"
        );
        std::fs::create_dir_all(dir.join("reports")).unwrap();
        std::fs::write(dir.join("reports/junit.xml"), "<x/>").unwrap();
        assert_eq!(is_ignored(&dir, "reports/junit.xml").await, Some(true));
        assert_eq!(is_ignored(&dir, "a.txt").await, Some(false));
        clean_all(&dir).await.unwrap();
        assert!(
            !dir.join("reports").exists(),
            "ignored files are cleaned too"
        );
        remove_worktree(&root, &dir, true).await.unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_worktree_is_created_where_claude_code_puts_them() {
        let root = scratch_repo("create").await;
        let wt = create_worktree(&root, "fix-thing-abc123", "fix/thing", "main")
            .await
            .unwrap();
        assert!(wt.starts_with(root.join(WORKTREE_DIR)));
        assert!(wt.join("a.txt").exists(), "it is a real checkout");
        assert!(worktree_present(&wt));

        let listed = list(&root).await.unwrap();
        assert!(
            listed
                .iter()
                .any(|w| w.branch.as_deref() == Some("fix/thing"))
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn isolation_leaves_the_persons_checkout_clean() {
        // Isolating must not dirty the person's `git status`, and must not
        // touch a tracked file to achieve that.
        let root = scratch_repo("exclude").await;
        create_worktree(&root, "one-abc", "w/one", "main")
            .await
            .unwrap();
        assert_eq!(
            porcelain_status(&root).await,
            "",
            "isolating work dirtied the person's checkout"
        );
        assert_eq!(porcelain_status(&root).await.matches(".claude").count(), 0);

        // Once: a second worktree adds no second line.
        create_worktree(&root, "two-abc", "w/two", "main")
            .await
            .unwrap();
        let exclude = std::fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        assert_eq!(
            exclude.matches(".claude/worktrees/").count(),
            1,
            "{exclude}"
        );
        // And nothing the person wrote there is lost.
        std::fs::write(
            root.join(".git/info/exclude"),
            "mine.txt\n.claude/worktrees/\n",
        )
        .unwrap();
        create_worktree(&root, "three-abc", "w/three", "main")
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(".git/info/exclude")).unwrap(),
            "mine.txt\n.claude/worktrees/\n"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_checkout_whose_git_is_a_file_is_excluded_through_the_common_dir() {
        // A linked worktree's `.git` is a file; its `info/exclude` lives in the
        // repository it belongs to.
        let root = scratch_repo("common").await;
        // Per-process name, so parallel test processes don't collide.
        let linked = root
            .parent()
            .unwrap()
            .join(format!("vp-git-common-linked-{}", std::process::id()));
        std::fs::remove_dir_all(&linked).ok();
        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "side",
                linked.to_str().unwrap(),
                "main",
            ],
        )
        .await
        .unwrap();
        assert!(
            linked.join(".git").is_file(),
            "the fixture is a linked worktree"
        );

        create_worktree(&linked, "nested-abc", "w/nested", "main")
            .await
            .unwrap();
        assert_eq!(porcelain_status(&linked).await, "");
        assert!(
            std::fs::read_to_string(root.join(".git/info/exclude"))
                .unwrap()
                .contains(".claude/worktrees/"),
            "the entry went to the repository the worktree belongs to"
        );
        std::fs::remove_dir_all(&linked).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn status_sees_changes_and_the_branch() {
        let root = scratch_repo("status").await;
        let st = status(&root).await.unwrap();
        assert_eq!(st.branch.as_deref(), Some("main"));
        assert!(st.changed_files == 0 && st.untracked_files == 0);

        std::fs::write(root.join("a.txt"), "changed\n").unwrap();
        std::fs::write(root.join("new.txt"), "new\n").unwrap();
        let st = status(&root).await.unwrap();
        assert_eq!(st.changed_files, 1);
        assert_eq!(st.untracked_files, 1);
        assert!(st.changed_files + st.untracked_files > 0);

        // The paths behind the counts, from the same read.
        let p = porcelain(&root).await.unwrap();
        assert_eq!(p.tracked, vec!["a.txt".to_string()]);
        assert_eq!(p.untracked, vec!["new.txt".to_string()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_porcelain_parser_reads_every_record_kind_including_a_rename() {
        // With `-z`, a rename's original name is a record of its own.
        let text = "# branch.oid abc\0# branch.head main\0# branch.ab +2 -1\0\
                    1 .M N... 100644 100644 100644 h h src/a.rs\0\
                    2 R. N... 100644 100644 100644 h h R100 new name.rs\0old name.rs\0\
                    u UU N... 100644 100644 100644 100644 h h h conflict.rs\0\
                    ? scratch.txt\0";
        let p = parse_porcelain(text);
        assert_eq!(p.status.commit.as_deref(), Some("abc"));
        assert_eq!(p.status.branch.as_deref(), Some("main"));
        assert_eq!((p.status.ahead, p.status.behind), (2, 1));
        assert_eq!(p.status.changed_files, 3);
        assert_eq!(p.status.untracked_files, 1);
        assert!(p.status.has_conflicts);
        assert_eq!(
            p.tracked,
            vec!["src/a.rs", "new name.rs", "old name.rs", "conflict.rs"]
        );
        assert_eq!(p.untracked, vec!["scratch.txt"]);
    }

    #[tokio::test]
    async fn a_dirty_checkout_is_refused_before_anything_is_created_naming_the_files() {
        let root = scratch_repo("dirty").await;
        std::fs::write(root.join("a.txt"), "unsaved\n").unwrap();
        std::fs::write(root.join("b.txt"), "staged\n").unwrap();
        git(&root, &["add", "b.txt"]).await.unwrap();

        let err = create_worktree(&root, "d-abc", "d/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("a.txt") && err.contains("b.txt"), "{err}");
        assert!(err.contains("2 tracked file(s)"), "{err}");
        assert!(
            !root.join(WORKTREE_DIR).exists(),
            "the refusal came after something was written"
        );
        assert!(
            !list(&root)
                .await
                .unwrap()
                .iter()
                .any(|w| w.branch.as_deref() == Some("d/abc")),
            "a branch was made on the way to refusing"
        );

        // Untracked files are not work the branch would carry.
        git(&root, &["checkout", "--", "a.txt"]).await.unwrap();
        git(&root, &["reset", "-q", "b.txt"]).await.unwrap();
        assert!(root.join("b.txt").exists(), "still there, now untracked");
        create_worktree(&root, "d-abc", "d/abc", "main")
            .await
            .expect("an untracked scratch file does not block isolation");
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_missing_base_branch_is_refused_by_name() {
        let root = scratch_repo("nobase").await;
        let err = create_worktree(&root, "nb-abc", "nb/abc", "release/9")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("`release/9`"), "{err}");
        assert!(err.contains("does not exist"), "{err}");
        assert!(!root.join(WORKTREE_DIR).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn an_existing_target_path_is_refused_saying_what_is_there() {
        let root = scratch_repo("exists").await;
        let dir = root.join(WORKTREE_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f-abc"), "not a checkout").unwrap();
        let err = create_worktree(&root, "f-abc", "f/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists and is a file"), "{err}");

        std::fs::create_dir_all(dir.join("g-abc")).unwrap();
        let err = create_worktree(&root, "g-abc", "g/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists and is a directory"), "{err}");

        std::os::unix::fs::symlink("/nowhere", dir.join("h-abc")).unwrap();
        let err = create_worktree(&root, "h-abc", "h/abc", "main")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists and is a symlink"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_base_branch_cannot_be_an_option() {
        // `base_branch` comes from a committed file; `--output=/tmp/x` would
        // make `git diff` write a file.
        let root = scratch_repo("argv").await;
        let target = root.join("planted-by-git-diff");
        let hostile = format!("--output={}", target.display());

        let set = change_set(&root, &hostile).await;
        assert!(!target.exists(), "`git diff` wrote a file it was told to");
        assert!(set.is_err(), "a base that is an option resolves to nothing");

        let touched = touched_files(&root, &hostile).await;
        assert!(!target.exists());
        assert!(touched.is_empty(), "{touched:?}");

        let err = create_worktree(&root, "opt-abc", "opt/abc", &hostile)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not exist"), "{err}");
        assert!(!target.exists());
        assert!(!root.join(WORKTREE_DIR).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn removing_a_worktree_refuses_to_destroy_work() {
        let root = scratch_repo("remove").await;
        let wt = create_worktree(&root, "wip-abc", "wip/abc", "main")
            .await
            .unwrap();
        std::fs::write(wt.join("a.txt"), "unsaved\n").unwrap();
        std::fs::write(wt.join("draft.txt"), "new\n").unwrap();

        let err = remove_worktree(&root, &wt, false).await.unwrap_err();
        let err = err.to_string();
        assert!(err.contains("uncommitted"), "{err}");
        assert!(
            err.contains("a.txt") && err.contains("draft.txt"),
            "the files are named: {err}"
        );

        // Forcing is allowed.
        remove_worktree(&root, &wt, true).await.unwrap();
        assert!(!wt.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    /// Commits the base lacks are named against the given base (no remote needed),
    /// and removing the worktree keeps them.
    #[tokio::test]
    async fn commits_the_base_does_not_have_are_named_and_kept() {
        let root = scratch_repo("unmerged").await;
        let wt = create_worktree(&root, "c-abc", "c/abc", "main")
            .await
            .unwrap();
        std::fs::write(wt.join("b.txt"), "x\n").unwrap();
        git(&wt, &["add", "-A"]).await.unwrap();
        git(&wt, &["commit", "-qm", "add the b file"])
            .await
            .unwrap();
        let unmerged = unmerged_commits(&root, "c/abc", "main").await.unwrap();
        assert_eq!(unmerged.len(), 1);
        assert!(unmerged[0].contains("add the b file"), "{unmerged:?}");

        remove_worktree(&root, &wt, false).await.unwrap();
        assert!(!wt.exists());
        let kept = git(&root, &["rev-parse", "--verify", "c/abc"]).await;
        assert!(kept.is_ok(), "the branch and its commits are kept");

        // Once merged, nothing is unmerged.
        git(&root, &["merge", "-q", "--ff-only", "c/abc"])
            .await
            .unwrap();
        assert!(
            unmerged_commits(&root, "c/abc", "main")
                .await
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_worktree_removed_by_hand_is_reconciled_rather_than_an_error() {
        let root = scratch_repo("byhand").await;
        let wt = create_worktree(&root, "gone-abc", "gone/abc", "main")
            .await
            .unwrap();
        std::fs::remove_dir_all(&wt).unwrap();
        assert!(!worktree_present(&wt));
        assert!(
            list(&root).await.unwrap().iter().any(|w| w.path == wt),
            "git still registers it, which is the situation"
        );

        remove_worktree(&root, &wt, false).await.unwrap();
        assert!(
            !list(&root).await.unwrap().iter().any(|w| w.path == wt),
            "the stale registration was not pruned"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// A file committed then edited, or committed then deleted, is listed once, as
    /// it stands.
    #[tokio::test]
    async fn a_file_committed_and_edited_again_is_listed_once() {
        let root = scratch_repo("once").await;
        git(&root, &["checkout", "-qb", "feat/once"]).await.unwrap();
        std::fs::write(root.join("a.txt"), "hello\nworld\n").unwrap();
        std::fs::write(root.join("b.txt"), "b\n").unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        git(&root, &["commit", "-qm", "edit"]).await.unwrap();
        std::fs::write(root.join("a.txt"), "hello\nworld\nagain\n").unwrap();
        std::fs::remove_file(root.join("b.txt")).unwrap();
        let set = change_set(&root, "main").await.unwrap();
        let paths: Vec<&str> = set.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.txt"], "b.txt was added and removed: nothing");
        assert_eq!((set.files[0].added, set.files[0].removed), (2, 0));
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn untracked_files_past_the_bound_are_counted_and_kept_whole() {
        let root = scratch_repo("untracked").await;
        for i in 0..(crate::core::diff::MAX_FILES + 5) {
            std::fs::write(root.join(format!("new-{i:02}.txt")), "x\n").unwrap();
        }
        let set = change_set(&root, "main").await.unwrap();
        assert_eq!(set.files.len(), crate::core::diff::MAX_FILES);
        assert_eq!(
            set.uncut().files.len(),
            crate::core::diff::MAX_FILES + 5,
            "what reads for weakened checks reads every file"
        );
        let t = set
            .truncated
            .expect("files past the bound were dropped without a word");
        assert_eq!(t.files_shown, crate::core::diff::MAX_FILES);
        assert_eq!(t.files_total, crate::core::diff::MAX_FILES + 5);
        assert!(t.command.contains("git status"), "{}", t.command);
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn the_stamp_carries_gits_own_tree_digest() {
        let root = scratch_repo("tree").await;
        let stamp = commit_stamp(&root).await.unwrap();
        let expected = git(&root, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        assert_eq!(stamp.tree.as_deref(), Some(expected.trim()));
        assert_ne!(stamp.tree, stamp.commit, "a tree is not a commit");
        std::fs::remove_dir_all(&root).ok();
    }

    /// The digest is the working tree, not `HEAD`: edits and untracked files change
    /// it, ignored files don't, and the person's index is untouched.
    #[tokio::test]
    async fn the_digest_is_the_working_tree_through_a_temporary_index() {
        let root = scratch_repo("digest").await;
        std::fs::write(root.join(".gitignore"), ".env\n").unwrap();
        git(&root, &["add", ".gitignore"]).await.unwrap();
        git(&root, &["commit", "-qm", "ignore"]).await.unwrap();
        let clean = tree_digest(&root).await.unwrap();
        let head = git(&root, &["rev-parse", "HEAD^{tree}"]).await.unwrap();
        assert_eq!(clean, head.trim(), "a clean tree digests to HEAD's tree");

        std::fs::write(root.join("new.txt"), "agent wrote this\n").unwrap();
        let dirty = tree_digest(&root).await.unwrap();
        assert_ne!(dirty, clean, "an untracked file is in the digest");
        assert_eq!(
            tree_digest(&root).await.unwrap(),
            dirty,
            "the same tree digests the same"
        );
        let staged = git(&root, &["diff", "--cached", "--name-only"])
            .await
            .unwrap();
        assert!(
            staged.trim().is_empty(),
            "the person's index was touched: {staged}"
        );
        let st = status(&root).await.unwrap();
        assert_eq!(st.untracked_files, 1, "still untracked for the person");

        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        assert_eq!(
            tree_digest(&root).await.unwrap(),
            dirty,
            "ignored files are out"
        );

        // Re-derivable the way the certificate says.
        git(&root, &["add", "-A"]).await.unwrap();
        let by_hand = git(&root, &["write-tree"]).await.unwrap();
        assert_eq!(by_hand.trim(), dirty);
        let leftovers: Vec<_> = std::fs::read_dir(root.join(".git"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("devplane-index")
            })
            .collect();
        assert!(leftovers.is_empty(), "the temporary index was left behind");
        std::fs::remove_dir_all(&root).ok();
    }

    /// A clean filter planted from inside the checkout — `.git/info/attributes`
    /// is outside the tree, `.git/config` is writable — can neither make an
    /// edited file digest as the old one nor run.
    #[tokio::test]
    async fn a_planted_filter_cannot_steer_the_digest_or_run() {
        let root = scratch_repo("filter").await;
        let before = tree_digest(&root).await.unwrap();
        let ran = root.join("filter-ran");
        std::fs::create_dir_all(root.join(".git/info")).unwrap();
        std::fs::write(root.join(".git/info/attributes"), "* filter=evil\n").unwrap();
        let clean = format!("touch {} && git show HEAD:a.txt", ran.display());
        git(&root, &["config", "filter.evil.clean", &clean])
            .await
            .unwrap();
        git(
            &root,
            &[
                "config",
                "core.fsmonitor",
                &format!("touch {}", ran.display()),
            ],
        )
        .await
        .unwrap();

        std::fs::write(root.join("a.txt"), "rewritten by the agent\n").unwrap();
        let after = tree_digest(&root).await.unwrap();
        assert_ne!(after, before, "an edited file changed the digest");
        porcelain_status(&root).await;
        assert!(!ran.exists(), "a command from the checkout's config ran");

        // Where no filter is declared the digest is git's own tree.
        std::fs::remove_file(root.join(".git/info/attributes")).unwrap();
        git(&root, &["config", "--unset", "filter.evil.clean"])
            .await
            .unwrap();
        git(&root, &["config", "--unset", "core.fsmonitor"])
            .await
            .unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        let by_hand = git(&root, &["write-tree"]).await.unwrap();
        assert_eq!(by_hand.trim(), after);
        std::fs::remove_dir_all(&root).ok();
    }

    /// Symbolic links, executable bits and embedded repositories digest as
    /// `git add -A` records them.
    #[tokio::test]
    async fn links_modes_and_embedded_repositories_digest_as_git_records_them() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch_repo("modes").await;
        std::os::unix::fs::symlink("a.txt", root.join("link")).unwrap();
        std::fs::write(root.join("run.sh"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(root.join("run.sh"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let inner = root.join("vendor/inner");
        std::fs::create_dir_all(&inner).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec![
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=T",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "i",
            ],
        ] {
            git(&inner, &args).await.unwrap();
        }
        std::fs::remove_file(root.join("a.txt")).unwrap();
        let digest = tree_digest(&root).await.unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        let by_hand = git(&root, &["write-tree"]).await.unwrap();
        assert_eq!(digest, by_hand.trim());
        std::fs::remove_dir_all(&root).ok();
    }

    /// A branch off `main` with `tests/t.rs` committed on `main`, for the
    /// ways a change could hide from the digest or the review.
    async fn branch_repo(tag: &str) -> PathBuf {
        let root = scratch_repo(tag).await;
        std::fs::create_dir_all(root.join("tests")).unwrap();
        std::fs::write(
            root.join("tests/t.rs"),
            "#[test]\nfn t() { assert!(ok()); }\n",
        )
        .unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        git(&root, &["commit", "-qm", "tests"]).await.unwrap();
        git(&root, &["checkout", "-qb", "change"]).await.unwrap();
        root
    }

    fn paths(set: &crate::core::diff::ChangeSet) -> Vec<&str> {
        set.files.iter().map(|f| f.path.as_str()).collect()
    }

    /// `assume-unchanged` and `skip-worktree` hide an edit from `git status`
    /// and `git diff`; the digest, the review and *clean* all still see it.
    #[tokio::test]
    async fn an_edit_the_index_is_told_to_ignore_is_digested_and_reviewed() {
        let root = branch_repo("hidden").await;
        let clean = commit_stamp(&root).await.unwrap();
        assert!(clean.clean);
        std::fs::write(root.join("tests/t.rs"), "#[test]\n#[ignore]\nfn t() {}\n").unwrap();
        std::fs::write(root.join("a.txt"), "edited\n").unwrap();
        git(&root, &["update-index", "--assume-unchanged", "tests/t.rs"])
            .await
            .unwrap();
        git(&root, &["update-index", "--skip-worktree", "a.txt"])
            .await
            .unwrap();
        assert!(
            porcelain_status(&root).await.trim().is_empty(),
            "git itself no longer sees the edits"
        );
        let stamp = commit_stamp(&root).await.unwrap();
        assert_ne!(stamp.tree, clean.tree, "the digest missed a hidden edit");
        assert!(!stamp.clean, "a hidden edit left the tree reported clean");
        let set = change_set(&root, "main").await.unwrap();
        assert_eq!(paths(&set), ["a.txt", "tests/t.rs"]);
        let rows = crate::core::review::weakened(&set, None, &[]);
        assert!(
            rows.iter().any(|w| w.matched == "#[ignore]"),
            "the hidden skip marker was not read: {rows:?}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// `-diff` makes a file binary, with no lines to read. Committed or
    /// system-wide it is not honoured; in `.git/info/attributes`, which git
    /// always reads, the file is read again as text. A real binary in the
    /// tests is named as not readable.
    #[tokio::test]
    async fn a_diff_attribute_cannot_hide_a_test_files_lines() {
        let root = branch_repo("attrs").await;
        std::fs::write(root.join(".gitattributes"), "*.rs -diff\n").unwrap();
        git(&root, &["add", ".gitattributes"]).await.unwrap();
        git(&root, &["commit", "-qm", "attrs"]).await.unwrap();
        std::fs::write(root.join("tests/t.rs"), "#[test]\n#[ignore]\nfn t() {}\n").unwrap();
        std::fs::write(root.join(".git/info/attributes"), "*.rs -diff\n").unwrap();
        let set = change_set(&root, "main").await.unwrap();
        let t = set.files.iter().find(|f| f.path == "tests/t.rs").unwrap();
        assert!(
            matches!(t.body, crate::core::diff::Body::Hunks(ref h) if !h.is_empty()),
            "an attribute made the test binary: {:?}",
            t.body
        );
        let rows = crate::core::review::weakened(&set, None, &[]);
        assert!(
            rows.iter()
                .any(|w| w.path == "tests/t.rs" && w.matched == "#[ignore]"),
            "the skip marker was not read: {rows:?}"
        );

        std::fs::write(root.join("tests/fixture.bin"), b"\0\x01\x02").unwrap();
        let set = change_set(&root, "main").await.unwrap();
        let rows = crate::core::review::weakened(&set, None, &[]);
        assert!(
            rows.iter()
                .any(|w| w.path == "tests/fixture.bin" && w.matched == "not readable"),
            "{rows:?}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// A checkout inside this one records only its commit, so work in it the
    /// commit does not have leaves no digest, and says why.
    #[tokio::test]
    async fn an_embedded_repository_with_uncommitted_work_has_no_digest() {
        let root = scratch_repo("embedded").await;
        let inner = root.join("vendor/inner");
        std::fs::create_dir_all(&inner).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec![
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=T",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "i",
            ],
        ] {
            git(&inner, &args).await.unwrap();
        }
        assert!(tree_digest(&root).await.is_ok(), "a committed one digests");
        std::fs::write(inner.join("run.sh"), "exit 0\n").unwrap();
        let err = tree_digest(&root).await.unwrap_err();
        assert!(format!("{err:#}").contains("vendor/inner"), "{err:#}");
        let stamp = commit_stamp(&root).await.unwrap();
        assert!(stamp.tree.is_none() && !stamp.clean);
        assert!(
            stamp
                .tree_unknown
                .as_deref()
                .is_some_and(|w| w.contains("embedded repository")),
            "{stamp:?}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// Only a committed `.gitignore` hides a file: one only on disk — even one
    /// that ignores itself — and `.git/info/exclude` hide nothing from the
    /// digest or the review.
    #[tokio::test]
    async fn only_a_committed_gitignore_hides_a_file() {
        let root = branch_repo("ignores").await;
        std::fs::create_dir_all(root.join("logs/deep")).unwrap();
        std::fs::write(root.join(".gitignore"), ".env\n").unwrap();
        std::fs::write(root.join("logs/.gitignore"), "*.log\n!keep.log\n").unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        git(&root, &["commit", "-qm", "ignores"]).await.unwrap();
        for f in [
            ".env",
            "logs/a.log",
            "logs/deep/b.log",
            "logs/keep.log",
            "top.log",
        ] {
            std::fs::write(root.join(f), "x\n").unwrap();
        }
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/.gitignore"), "*\n").unwrap();
        std::fs::write(root.join("sub/hidden.rs"), "fn x() {}\n").unwrap();
        std::fs::write(root.join(".git/info/exclude"), "secret.rs\n").unwrap();
        std::fs::write(root.join("secret.rs"), "fn y() {}\n").unwrap();

        let d = digest(&root).await.unwrap();
        let has = |p: &str| d.entries.contains_key(p);
        for p in [
            "logs/keep.log",
            "top.log",
            "sub/.gitignore",
            "sub/hidden.rs",
            "secret.rs",
        ] {
            assert!(has(p), "{p} is missing from the digest");
        }
        for p in [".env", "logs/a.log", "logs/deep/b.log"] {
            assert!(!has(p), "{p} is ignored by a committed rule");
        }
        let set = change_set(&root, "main").await.unwrap();
        for p in ["sub/.gitignore", "sub/hidden.rs", "secret.rs"] {
            assert!(
                paths(&set).contains(&p),
                "{p} is not reviewed: {:?}",
                paths(&set)
            );
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_nested_ignore_file_means_the_same_from_the_top() {
        assert_eq!(rebase_ignores("", "# c\n*.log\n"), "*.log\n");
        assert_eq!(
            rebase_ignores("a/b", "*.log\n/build\nout/\ndocs/*.md\n!keep.log\n"),
            "a/b/**/*.log\na/b/build\na/b/**/out/\na/b/docs/*.md\n!a/b/**/keep.log\n"
        );
        assert_eq!(rebase_ignores("[x]", "y\n"), "\\[x]/**/y\n");
    }

    /// The blob cache is keyed by the change time, which no process sets
    /// back: the same size and a restored mtime still read as an edit.
    #[tokio::test]
    async fn a_restored_mtime_cannot_keep_a_cached_blob() {
        let root = scratch_repo("cache").await;
        let before = tree_digest(&root).await.unwrap();
        // Old enough to be cached.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        assert_eq!(tree_digest(&root).await.unwrap(), before);
        let file = root.join("a.txt");
        let mtime = std::fs::metadata(&file).unwrap().modified().unwrap();
        std::fs::write(&file, "HELLO\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        assert_ne!(tree_digest(&root).await.unwrap(), before);
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn includes_are_copied_but_never_over_something() {
        let root = scratch_repo("includes").await;
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        let wt = create_worktree(&root, "i-abc", "i/abc", "main")
            .await
            .unwrap();

        let copied = copy_included(&root, &wt, &[".env".into(), "missing.txt".into()]);
        assert_eq!(copied, vec![".env".to_string()]);
        assert_eq!(
            std::fs::read_to_string(wt.join(".env")).unwrap(),
            "SECRET=1\n"
        );

        std::fs::write(wt.join(".env"), "MINE=1\n").unwrap();
        copy_included(&root, &wt, &[".env".into()]);
        assert_eq!(
            std::fs::read_to_string(wt.join(".env")).unwrap(),
            "MINE=1\n",
            "an existing file is the worktree's own"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_worktree_whose_state_cannot_be_read_is_not_silently_removed() {
        // An unreadable status must not read as "nothing here".
        let root = scratch_repo("unreadable").await;
        // A directory holding work where git cannot answer at all.
        let orphan = std::env::temp_dir().join(format!("vp-orphan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&orphan);
        std::fs::create_dir_all(&orphan).unwrap();
        std::fs::write(orphan.join("draft.txt"), "work in progress\n").unwrap();

        let err = remove_worktree(&root, &orphan, false).await.unwrap_err();
        assert!(
            err.to_string().contains("cannot read the state"),
            "got: {err}"
        );
        assert!(orphan.join("draft.txt").exists(), "the work is still there");
        let _ = std::fs::remove_dir_all(&orphan);
    }

    #[tokio::test]
    async fn a_symlinked_include_cannot_smuggle_a_file_out_of_the_repository() {
        // `within` checks spelling, and a symlink is not a spelling: a repo can
        // ship `config/local.env -> ~/.ssh/id_rsa` and name it in
        // `.worktreeinclude`.
        let root = scratch_repo("symlink-escape").await;
        let secret = root.parent().unwrap().join("vp-symlink-secret.txt");
        std::fs::write(&secret, "PRIVATE KEY\n").unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::os::unix::fs::symlink(&secret, root.join("config/local.env")).unwrap();

        let wt = create_worktree(&root, "sym-abc", "sym/abc", "main")
            .await
            .unwrap();
        let copied = copy_included(&root, &wt, &["config/local.env".into()]);

        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(
            !wt.join("config/local.env").exists(),
            "a file the repository only points at is not a file the repository has"
        );

        // A symlink that stays inside the repository is fine.
        std::fs::write(root.join("real.env"), "OK\n").unwrap();
        std::os::unix::fs::symlink(root.join("real.env"), root.join("config/inside.env")).unwrap();
        let copied = copy_included(&root, &wt, &["config/inside.env".into()]);
        assert_eq!(copied, vec!["config/inside.env".to_string()]);
    }

    #[tokio::test]
    async fn an_include_never_writes_through_a_symlink_already_in_the_worktree() {
        // The write direction: a committed dangling symlink already sits at
        // the destination, and `fs::copy` would write through it.
        let root = scratch_repo("symlink-write").await;
        let target = root.parent().unwrap().join("vp-symlink-target.txt");
        let _ = std::fs::remove_file(&target);
        std::fs::write(root.join(".env"), "PAYLOAD\n").unwrap();

        let wt = create_worktree(&root, "symw-abc", "symw/abc", "main")
            .await
            .unwrap();
        std::os::unix::fs::symlink(&target, wt.join(".env")).unwrap();

        let copied = copy_included(&root, &wt, &[".env".into()]);
        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(
            !target.exists(),
            "writing through a symlink the repository chose is an arbitrary write"
        );
    }

    #[tokio::test]
    async fn an_include_cannot_reach_outside_the_repository() {
        // Git never names a path outside the checkout, but the rule stays as a
        // second lock.
        let root = scratch_repo("escape").await;
        let secret = root.parent().unwrap().join("vp-secret-outside.txt");
        std::fs::write(&secret, "SECRET\n").unwrap();
        let wt = create_worktree(&root, "e-abc", "e/abc", "main")
            .await
            .unwrap();

        let copied = copy_included(
            &root,
            &wt,
            &[
                "../vp-secret-outside.txt".into(),
                "/etc/hosts".into(),
                "a/../../vp-secret-outside.txt".into(),
            ],
        );
        assert!(copied.is_empty(), "copied {copied:?}");
        assert!(!wt.join("vp-secret-outside.txt").exists());
        // A path that dips into a subdirectory and back out is still inside.
        std::fs::create_dir_all(root.join("cfg")).unwrap();
        std::fs::write(root.join(".env"), "OK\n").unwrap();
        assert_eq!(
            copy_included(&root, &wt, &["cfg/../.env".into()]),
            vec!["cfg/../.env".to_string()]
        );

        std::fs::remove_file(&secret).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn ignored_files_lists_what_git_ignores_and_never_a_tracked_file() {
        // A tracked file is never a candidate, however a pattern reads; only
        // what `.worktreeinclude` names is listed at all.
        let root = scratch_repo("ignored").await;
        assert!(
            ignored_files(&root).await.unwrap().is_empty(),
            "no .worktreeinclude, no candidates"
        );
        std::fs::write(root.join(".gitignore"), ".env\nconfig/\n").unwrap();
        std::fs::write(root.join("keep.txt"), "tracked\n").unwrap();
        git(&root, &["add", "-A"]).await.unwrap();
        git(&root, &["commit", "-qm", "ignore"]).await.unwrap();
        std::fs::write(root.join(".env"), "SECRET=1\n").unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/a.local"), "x\n").unwrap();
        std::fs::write(root.join("scratch.txt"), "untracked, not ignored\n").unwrap();
        std::fs::write(root.join("other.env"), "ignored? no, and not named\n").unwrap();
        std::fs::write(
            root.join(".worktreeinclude"),
            ".env\nconfig/\nkeep.txt\nscratch.txt\n",
        )
        .unwrap();

        let mut files = ignored_files(&root).await.unwrap();
        files.sort();
        assert_eq!(
            files,
            vec![".env".to_string(), "config/a.local".to_string()],
            "the files inside an ignored directory are listed one by one"
        );
        assert!(!files.iter().any(|f| f == "keep.txt"));
        assert!(
            !files.iter().any(|f| f == "scratch.txt"),
            "untracked is not ignored"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn the_base_branch_is_discovered_not_assumed() {
        let root = scratch_repo("base").await;
        assert_eq!(base_branch(&root).await, "main");
        std::fs::remove_dir_all(&root).ok();
    }
}

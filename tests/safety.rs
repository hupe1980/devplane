//! Host, git and gate safety: the token stays with the configured port, the
//! home is owner-only, included files cannot be written outside a worktree,
//! the tree digest cannot be steered from inside the checkout, a diff that
//! cannot be read is an error, and a gate's processes die with the gate.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "devplane-safety-{tag}-{}-{}",
        std::process::id(),
        jiff::Timestamp::now().as_nanosecond()
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn repo(tag: &str) -> PathBuf {
    let dir = scratch(tag);
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
    std::fs::write(dir.join(".gitignore"), ".env\n*.local\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "init"]);
    dir
}

/// A taken port is refused, never swapped for another: the vendors' settings
/// would keep sending the bearer token to whoever holds it.
#[tokio::test]
async fn a_taken_port_is_refused_rather_than_swapped() {
    let squatter = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = squatter.local_addr().unwrap().port();
    let err = devplane::host::bind(port)
        .await
        .expect_err("a taken port must not bind");
    let msg = format!("{err:#}");
    assert!(msg.contains(&port.to_string()), "{msg}");
    assert!(msg.contains("devplane connect"), "names the fix: {msg}");
    assert!(msg.contains("--port"), "names the fix: {msg}");
    drop(squatter);
}

/// The home is `0700` and its secrets `0600`, tightened when an older build
/// left them readable.
#[cfg(unix)]
#[test]
fn the_home_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let home = scratch("home");
    for f in ["token", "host.json", "devplane.db", "devplane.db-wal"] {
        std::fs::write(home.join(f), "x").unwrap();
        std::fs::set_permissions(home.join(f), std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o755)).unwrap();
    devplane::host::secure_home(&home).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&home), 0o700);
    for f in ["token", "host.json", "devplane.db", "devplane.db-wal"] {
        assert_eq!(mode(&home.join(f)), 0o600, "{f}");
    }
    // Created owner-only when missing.
    let fresh = home.join("nested").join(".devplane");
    devplane::host::secure_home(&fresh).unwrap();
    assert_eq!(mode(&fresh), 0o700);
    std::fs::remove_dir_all(&home).ok();
}

/// A symlinked directory in the worktree never receives an included file.
#[cfg(unix)]
#[test]
fn an_included_file_is_never_written_through_a_symlinked_directory() {
    let base = scratch("copy");
    let root = base.join("root");
    let tree = base.join("tree");
    let outside = base.join("public");
    for d in [&root.join("config"), &tree, &outside] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(root.join("config/local.env"), "SECRET=1\n").unwrap();
    std::fs::write(root.join("top.local"), "fine\n").unwrap();
    std::os::unix::fs::symlink(&outside, tree.join("config")).unwrap();

    let copied = devplane::git::copy_included(
        &root,
        &tree,
        &["config/local.env".to_string(), "top.local".to_string()],
    );
    assert_eq!(copied, vec!["top.local".to_string()]);
    assert!(
        !outside.join("local.env").exists(),
        "the secret was written through the symlink"
    );
    assert_eq!(
        std::fs::read_to_string(tree.join("top.local")).unwrap(),
        "fine\n"
    );
    // Missing directories are created, inside the worktree.
    std::fs::create_dir_all(root.join("deep/er")).unwrap();
    std::fs::write(root.join("deep/er/x.local"), "x\n").unwrap();
    let copied = devplane::git::copy_included(&root, &tree, &["deep/er/x.local".to_string()]);
    assert_eq!(copied, vec!["deep/er/x.local".to_string()]);
    std::fs::remove_dir_all(&base).ok();
}

/// Only the ignored files `.worktreeinclude` names are candidates; a tracked
/// file it names never is.
#[tokio::test]
async fn only_the_ignored_files_worktreeinclude_names_are_listed() {
    let r = repo("include");
    std::fs::write(r.join(".worktreeinclude"), ".env\nconfig/*.local\na.txt\n").unwrap();
    std::fs::write(r.join(".env"), "SECRET=1\n").unwrap();
    std::fs::create_dir_all(r.join("config")).unwrap();
    std::fs::write(r.join("config/a.local"), "a\n").unwrap();
    std::fs::write(r.join("config/b.local"), "b\n").unwrap();
    std::fs::write(r.join("other.local"), "not named\n").unwrap();
    let mut got = devplane::git::ignored_files(&r).await.unwrap();
    got.sort();
    assert_eq!(got, vec![".env", "config/a.local", "config/b.local"]);
    std::fs::remove_file(r.join(".worktreeinclude")).unwrap();
    assert!(devplane::git::ignored_files(&r).await.unwrap().is_empty());
    std::fs::remove_dir_all(&r).ok();
}

/// A file hidden through `.git/info/exclude`, or an edit hidden behind
/// `assume-unchanged`, still changes the digest.
#[tokio::test]
async fn the_digest_cannot_be_steered_from_inside_the_checkout() {
    let r = repo("steer");
    let clean = devplane::git::tree_digest(&r).await.unwrap();
    assert_eq!(clean, git(&r, &["rev-parse", "HEAD^{tree}"]));

    std::fs::write(r.join("conftest.py"), "monkeypatch()\n").unwrap();
    std::fs::write(r.join(".git/info/exclude"), "conftest.py\n").unwrap();
    let hidden = devplane::git::tree_digest(&r).await.unwrap();
    assert_ne!(hidden, clean, "info/exclude hid a file from the digest");
    std::fs::remove_file(r.join("conftest.py")).unwrap();

    // `.gitignore` still excludes: that is the committed rule.
    std::fs::write(r.join(".env"), "SECRET=1\n").unwrap();
    assert_eq!(devplane::git::tree_digest(&r).await.unwrap(), clean);

    git(&r, &["update-index", "--assume-unchanged", "a.txt"]);
    std::fs::write(r.join("a.txt"), "edited after the pass\n").unwrap();
    let edited = devplane::git::tree_digest(&r).await.unwrap();
    assert_ne!(edited, clean, "assume-unchanged hid an edit");
    // And the person's index keeps its flag.
    assert!(git(&r, &["ls-files", "-v", "a.txt"]).starts_with('h'));
    std::fs::remove_dir_all(&r).ok();
}

/// The repository's own config cannot relax the stat check the digest makes:
/// a same-size edit whose mtime was put back still changes it.
#[cfg(unix)]
#[tokio::test]
async fn a_same_size_edit_with_its_mtime_restored_changes_the_digest() {
    let r = repo("stat");
    let clean = devplane::git::tree_digest(&r).await.unwrap();
    git(&r, &["config", "core.trustctime", "false"]);
    git(&r, &["config", "core.checkStat", "minimal"]);
    git(&r, &["config", "core.ignoreStat", "true"]);
    let keep = r.join("mtime-ref");
    std::fs::write(&keep, "").unwrap();
    let touch = |args: &[&str]| {
        assert!(
            Command::new("touch")
                .args(args)
                .current_dir(&r)
                .status()
                .unwrap()
                .success()
        )
    };
    touch(&["-r", "a.txt", "mtime-ref"]);
    std::fs::write(r.join("a.txt"), "jello\n").unwrap();
    touch(&["-r", "mtime-ref", "a.txt"]);
    std::fs::remove_file(&keep).unwrap();
    let edited = devplane::git::tree_digest(&r).await.unwrap();
    assert_ne!(
        edited, clean,
        "the repository's config hid a same-size edit"
    );
    std::fs::remove_dir_all(&r).ok();
}

/// Reading status does not rewrite the person's index.
#[tokio::test]
async fn reading_status_takes_no_optional_lock() {
    let r = repo("locks");
    // Make the stat cache stale without changing content, so a refreshing
    // status would write the index.
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(r.join("a.txt"), "hello\n").unwrap();
    let before = std::fs::read(r.join(".git/index")).unwrap();
    devplane::git::status(&r).await.unwrap();
    let after = std::fs::read(r.join(".git/index")).unwrap();
    assert_eq!(before, after, "git status rewrote .git/index");
    std::fs::remove_dir_all(&r).ok();
}

/// A diff that cannot be read is an error, never an empty change; a base that
/// exists only as `origin/<base>` is found.
#[tokio::test]
async fn a_change_set_that_cannot_be_read_is_an_error() {
    let r = repo("diff");
    assert!(
        devplane::git::change_set(&r, "develop").await.is_err(),
        "a base that does not resolve must not read as no change"
    );
    let head = git(&r, &["rev-parse", "HEAD"]);
    git(&r, &["update-ref", "refs/remotes/origin/develop", &head]);
    std::fs::write(r.join("a.txt"), "changed\n").unwrap();
    let set = devplane::git::change_set(&r, "develop").await.unwrap();
    assert_eq!(set.files.len(), 1, "{set:?}");
    std::fs::remove_dir_all(&r).ok();
}

#[cfg(unix)]
fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks for existence.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(unix)]
fn wait_dead(pid: i32) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// A check that leaves a child running passes on its own exit, and the child
/// dies with it rather than holding the pipes until the timeout.
#[cfg(unix)]
#[tokio::test]
async fn a_gate_leaves_no_process_behind() {
    let dir = scratch("gate");
    let pidfile = dir.join("pid");
    let cmd = format!("sleep 60 & echo $! > {}; echo done", pidfile.display());
    let started = std::time::Instant::now();
    let report = devplane::gates::run("check", &[cmd], &dir, Duration::from_secs(30), 1, &[]).await;
    assert!(report.passed(), "{:?}", report.commands);
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "the gate waited on a background child"
    );
    let pid: i32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        wait_dead(pid),
        "the background child {pid} outlived its gate"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A gate whose caller stops waiting takes its whole process group with it.
#[cfg(unix)]
#[tokio::test]
async fn a_cancelled_gate_kills_its_group() {
    let dir = scratch("cancel");
    let pidfile = dir.join("pid");
    let cmd = format!("sleep 60 & echo $! > {}; wait", pidfile.display());
    let d = dir.clone();
    let task = tokio::spawn(async move {
        devplane::gates::run("check", &[cmd], &d, Duration::from_secs(60), 1, &[]).await
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let pid: i32 = loop {
        if let Some(p) = std::fs::read_to_string(&pidfile)
            .ok()
            .and_then(|s| s.trim().parse().ok())
        {
            break p;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the gate never started"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    task.abort();
    let _ = task.await;
    assert!(
        wait_dead(pid),
        "the cancelled gate's child {pid} is still running"
    );
    std::fs::remove_dir_all(&dir).ok();
}

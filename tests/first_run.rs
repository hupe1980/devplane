//! The first run from the window, over the real API: a check declared as a
//! confirmed diff governs the next change before anybody commits it, a preview
//! writes nothing, a stale confirm writes nothing, and the checklist's steps
//! are read from what is true.

mod common;

use common::{boot, echo_agent, get, git, post, scratch, trust, until};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A repository with a test script and, when `config` is given, a committed
/// `devplane.toml`.
fn repo(tag: &str, config: Option<&str>) -> PathBuf {
    let dir = scratch(&format!("firstrun-{tag}"));
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(
        dir.join("scripts/test.sh"),
        "#!/bin/sh\necho ran the suite\nexit 0\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("package.json"),
        "{\n  \"name\": \"x\",\n  \"scripts\": {\n    \"test\": \"sh ./scripts/test.sh\"\n  }\n}\n",
    )
    .unwrap();
    std::fs::write(dir.join(".gitignore"), ".devplane/\n").unwrap();
    if let Some(c) = config {
        std::fs::write(dir.join("devplane.toml"), c).unwrap();
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

async fn send(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    path: &str,
    body: Value,
) -> (u16, Value) {
    let r = c
        .post(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = r.status().as_u16();
    (status, r.json().await.unwrap_or(Value::Null))
}

fn pid(repo: &Path) -> String {
    urlencoding(&repo.display().to_string())
}

fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Declares `check` through the window's two requests, preview then confirm.
async fn declare(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    repo: &Path,
    check: &[&str],
) -> Value {
    let path = format!("/api/projects/{}/gates", pid(repo));
    let (status, preview) = send(c, addr, &path, json!({"check": check, "confirm": false})).await;
    assert_eq!(status, 200, "{preview}");
    let (status, done) = send(
        c,
        addr,
        &path,
        json!({"check": check, "confirm": true, "diff": preview["diff"]}),
    )
    .await;
    assert_eq!(status, 200, "{done}");
    preview
}

/// Waits for the newest change to rest, and returns it.
async fn rested(c: &reqwest::Client, addr: &std::net::SocketAddr) -> Value {
    until("the change to rest", Duration::from_secs(60), || async {
        let list = get(c, addr, "/api/changes").await;
        let w = list.as_array()?.first()?.clone();
        (w["waiting"]["on"] == "person" || w["stopped"].is_object() || w["completion"].is_object())
            .then_some(w)
    })
    .await
}

#[tokio::test]
async fn a_check_declared_from_the_window_governs_the_next_change_uncommitted() {
    let Some(agent) = echo_agent() else { return };
    // Both cases: no file yet (an untracked new one), and a tracked file
    // edited from the window and not committed.
    for existing in [None, Some("[project]\nname = \"x\"\n")] {
        let repo = repo("governs", existing);
        let (addr, c, _state) = boot().await;
        trust(&c, &addr, &repo).await;
        let preview = declare(&c, &addr, &repo, &["sh ./scripts/test.sh"]).await;
        assert_eq!(
            preview["uncommitted"], false,
            "nothing was written by the preview"
        );
        let text = std::fs::read_to_string(repo.join("devplane.toml")).unwrap();
        assert!(
            text.contains("check = [\"sh ./scripts/test.sh\"]"),
            "{text}"
        );
        assert!(text.contains(devplane::core::declare::ORIGIN), "{text}");
        if let Some(before) = existing {
            assert!(
                text.starts_with(before),
                "the other lines are unchanged: {text}"
            );
        }

        let started = post(
            &c,
            &addr,
            "/api/changes",
            json!({"cwd": repo.to_string_lossy(), "title": "declared", "agent": agent}),
        )
        .await;
        assert!(
            started["change_id"].is_string(),
            "the start is not refused: {started}"
        );
        let w = rested(&c, &addr).await;
        let gate = w["gates"]
            .as_array()
            .and_then(|g| g.last())
            .cloned()
            .unwrap_or_default();
        assert_eq!(gate["gate"], "check", "{w}");
        assert_eq!(
            gate["commands"][0]["command"], "sh ./scripts/test.sh",
            "{gate}"
        );
        assert_eq!(w["state"], "verified", "{w}");
        std::fs::remove_dir_all(&repo).ok();
    }
}

#[tokio::test]
async fn a_preview_writes_nothing_and_a_stale_confirm_writes_nothing() {
    let repo = repo("preview", Some("[gates]\ncheck = [\"old\"]\n"));
    let (addr, c, _state) = boot().await;
    let path = format!("/api/projects/{}/gates", pid(&repo));
    // Untrusted: nothing is read or written.
    let (status, _) = send(&c, &addr, &path, json!({"check": ["npm test"]})).await;
    assert_eq!(status, 404, "not registered yet");
    trust(&c, &addr, &repo).await;

    let before = std::fs::read_to_string(repo.join("devplane.toml")).unwrap();
    let (status, preview) = send(&c, &addr, &path, json!({"check": ["npm test"]})).await;
    assert_eq!(status, 200, "{preview}");
    assert!(
        preview["diff"]
            .as_str()
            .unwrap()
            .contains("+ check = [\"npm test\"]"),
        "{preview}"
    );
    assert!(
        preview["diff"]
            .as_str()
            .unwrap()
            .contains("- check = [\"old\"]"),
        "{preview}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("devplane.toml")).unwrap(),
        before
    );

    // The file changes under the person: their confirm is of a diff no longer true.
    std::fs::write(repo.join("devplane.toml"), "[gates]\ncheck = [\"other\"]\n").unwrap();
    let (status, stale) = send(
        &c,
        &addr,
        &path,
        json!({"check": ["npm test"], "confirm": true, "diff": preview["diff"]}),
    )
    .await;
    assert_eq!(status, 409, "{stale}");
    assert_eq!(stale["refused"], "changed_since");
    assert!(stale["diff"].as_str().unwrap().contains("other"));
    assert_eq!(stale["uncommitted"], true);
    assert_eq!(
        std::fs::read_to_string(repo.join("devplane.toml")).unwrap(),
        "[gates]\ncheck = [\"other\"]\n"
    );

    // A file that does not parse is refused with its line, and left alone.
    std::fs::write(repo.join("devplane.toml"), "[gates\n").unwrap();
    let (status, broken) = send(&c, &addr, &path, json!({"check": ["npm test"]})).await;
    assert_eq!(status, 422, "{broken}");
    assert_eq!(broken["refused"], "unparseable");
    assert_eq!(broken["line"], 1);
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn candidates_are_offered_from_the_repository_and_never_run() {
    let repo = repo("candidates", None);
    std::fs::create_dir_all(repo.join("openspec")).unwrap();
    std::fs::write(repo.join("scripts/test.sh"), "#!/bin/sh\ntouch ran\n").unwrap();
    let (addr, c, _state) = boot().await;
    trust(&c, &addr, &repo).await;
    let got = get(
        &c,
        &addr,
        &format!("/api/projects/{}/gates/candidates", pid(&repo)),
    )
    .await;
    let offered: Vec<(&str, &str, &str)> = got["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["command"].as_str().unwrap(),
                c["source"].as_str().unwrap(),
                c["joins"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        offered,
        vec![
            ("npm test", "package.json", "check"),
            (
                "openspec validate --all --strict",
                "openspec/",
                "named:openspec"
            ),
        ]
    );
    assert_eq!(got["config"]["exists"], false);
    assert_eq!(got["looked_for"].as_array().unwrap().len(), 5);
    assert!(!repo.join("ran").exists(), "offering ran nothing");

    // Choosing OpenSpec writes its own table and leaves `check` alone.
    let path = format!("/api/projects/{}/gates", pid(&repo));
    let (status, preview) = send(
        &c,
        &addr,
        &path,
        json!({"named": {"openspec": ["openspec validate --all --strict"]}}),
    )
    .await;
    assert_eq!(status, 200, "{preview}");
    let diff = preview["diff"].as_str().unwrap();
    assert!(diff.contains("[gates.openspec]"), "{diff}");
    assert!(!diff.contains("check ="), "{diff}");
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test]
async fn the_checklist_is_read_from_the_machine() {
    let repo = repo("checklist", None);
    let (addr, c, _state) = boot().await;
    let steps = |v: &Value| -> Vec<(String, String)> {
        v["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["id"].as_str().unwrap().to_string(),
                    s["state"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let empty = get(&c, &addr, "/api/first-run").await;
    let s = steps(&empty);
    assert_eq!(s.len(), 7, "{empty}");
    assert_eq!(s[0], ("sessions".into(), "done".into()));
    assert_eq!(s[3].0, "trust");
    assert_ne!(s[3].1, "done");

    // A scan before trusting reads the repository and registers nothing.
    let scanned = get(
        &c,
        &addr,
        &format!(
            "/api/projects/scan?path={}",
            urlencoding(&repo.display().to_string())
        ),
    )
    .await;
    assert_eq!(scanned["root"], repo.display().to_string());
    assert_eq!(scanned["trusted"], false);
    assert_eq!(
        get(&c, &addr, "/api/projects")
            .await
            .as_array()
            .unwrap()
            .len(),
        0
    );

    trust(&c, &addr, &repo).await;
    let after = get(&c, &addr, "/api/first-run").await;
    let s = steps(&after);
    assert_eq!(s[3], ("trust".into(), "done".into()), "{after}");
    let declare_step = &after["steps"][4];
    assert_eq!(declare_step["id"], "declare");
    assert!(
        declare_step["says"]
            .as_str()
            .unwrap()
            .contains("cannot become verified"),
        "{declare_step}"
    );
    declare(&c, &addr, &repo, &["npm test"]).await;
    let declared = get(&c, &addr, "/api/first-run").await;
    assert_eq!(declared["steps"][4]["state"], "done", "{declared}");
    let projects = get(&c, &addr, "/api/projects").await;
    assert_eq!(projects[0]["declares_check"], true);
    std::fs::remove_dir_all(&repo).ok();
}

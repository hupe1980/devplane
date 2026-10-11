//! Drafting a specification by the project's own methodology, end to end:
//! a real repository with Spec Kit's or OpenSpec's files, the fixture agent
//! answering the methodology's command as that command would, and the host
//! dispatching it over ACP. What was sent, where it landed, that none of it
//! was written by Devplane, and that the person decides.

mod common;

use common::{echo_agent, get, git, post, trust};
use devplane::core::Policy;
use devplane::host::AppState;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// How long a draft may take to come back.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(60);

/// A host whose `claude` agent is the fixture agent, launched with `env`
/// (leading `NAME=value` words) — as a person's `agents.toml` overrides a
/// built-in.
async fn boot_with(env: &str) -> Option<(SocketAddr, reqwest::Client, common::Host)> {
    let bin = echo_agent()?;
    let db = std::env::temp_dir().join(format!("dp-draft-{}.db", uuid::Uuid::new_v4().simple()));
    let home = db.with_extension("home");
    std::fs::create_dir_all(&home).unwrap();
    let command = format!("{env} {bin}").trim().to_string();
    std::fs::write(
        home.join("agents.toml"),
        format!("[agents.claude]\nname = \"Echo as Claude\"\ncommand = {command:?}\n"),
    )
    .unwrap();
    let state = AppState::new(db, "tok".into(), Policy::default(), home)
        .await
        .unwrap();
    common::as_the_app(&state);
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    Some((addr, reqwest::Client::new(), common::Host(state)))
}

/// Spec Kit's integration record for Claude in skills mode, as `specify init`
/// writes it (this repository's own, Spec Kit 1.0.7).
const INTEGRATION: &str = r#"{
  "version": "1.0.7",
  "integration_state_schema": 1,
  "installed_integrations": ["claude"],
  "integration_settings": {"claude": {"script": "sh", "invoke_separator": "-"}},
  "integration": "claude",
  "default_integration": "claude"
}"#;

/// A repository with `files`, committed.
fn repo_with(tag: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = common::scratch(&format!("draft-{tag}"));
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    for (name, body) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "init"]);
    dir.canonicalize().unwrap()
}

/// A Spec Kit project with the Claude integration installed.
fn speckit_repo(tag: &str, config: &str) -> PathBuf {
    repo_with(
        tag,
        &[
            ("devplane.toml", config),
            (".gitignore", ".devplane/\n"),
            (".specify/integration.json", INTEGRATION),
            (
                ".specify/init-options.json",
                r#"{"ai": "claude", "ai_skills": true, "feature_numbering": "sequential"}"#,
            ),
            (
                ".claude/skills/speckit-specify/SKILL.md",
                "# speckit-specify\n",
            ),
            ("specs/001-existing/spec.md", "# Existing\n"),
        ],
    )
}

/// An OpenSpec project with its propose workflow installed for Claude.
fn openspec_repo(tag: &str, config: &str, workflow: bool) -> PathBuf {
    let mut files = vec![
        ("devplane.toml", config),
        (".gitignore", ".devplane/\n"),
        ("openspec/config.yaml", "schema: spec-driven\n"),
        ("openspec/specs/auth/spec.md", "# Auth\n"),
    ];
    if workflow {
        files.push((
            ".claude/skills/openspec-propose/SKILL.md",
            "# openspec-propose\n",
        ));
    }
    repo_with(tag, &files)
}

const INTENT: &str =
    "Limit login attempts per account.  Lock the account after five tries in a row.";

async fn draft(c: &reqwest::Client, addr: &SocketAddr, body: Value) -> Value {
    post(c, addr, "/api/changes/draft", body).await
}

/// Waits until the change's draft leaves `pending`.
async fn settled(c: &reqwest::Client, addr: &SocketAddr, id: &str) -> Value {
    common::until("the draft's turn to end", SETTLE, || async {
        let v = get(c, addr, &format!("/api/changes/{id}")).await;
        (v["draft"]["outcome"] != "pending" && !v["draft"].is_null()).then_some(v)
    })
    .await
}

/// What the fixture agent wrote, by path.
fn wrote(worktree: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(worktree.join(".devplane/wrote.log"))
        .unwrap_or_default()
        .lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            (
                v["path"].as_str().unwrap().to_string(),
                v["content"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// Every file under `dir`, relative to `root`.
fn files_under(root: &Path, dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(files_under(root, &p));
        } else {
            out.push(
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    out.sort();
    out
}

/// Every line in the drafted folder was written by the agent's session:
/// each file is one the agent logged, byte for byte, and nothing else is
/// there.
fn nothing_by_devplane(worktree: &Path, folder: &str) {
    let by_agent = wrote(worktree);
    let present = files_under(worktree, &worktree.join(folder));
    assert!(
        !present.is_empty(),
        "the command wrote nothing under {folder}"
    );
    for f in &present {
        let on_disk = std::fs::read_to_string(worktree.join(f)).unwrap();
        let theirs = by_agent
            .iter()
            .rev()
            .find(|(p, _)| p == f)
            .unwrap_or_else(|| panic!("{f} is in the drafted folder and the agent never wrote it"));
        assert_eq!(
            on_disk, theirs.1,
            "{f} differs from what the agent wrote: somebody else wrote into it"
        );
    }
}

#[tokio::test]
async fn a_draft_sends_exactly_the_invocation_and_the_intent() {
    let Some((addr, c, _host)) = boot_with("").await else {
        return;
    };

    // Spec Kit: `/speckit-specify`, spelled from the integration record.
    let repo = speckit_repo("sk-sent", "[gates]\ncheck = [\"true\"]\n");
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "Rate limit", "agent": "claude", "intent": INTENT}),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let sent = format!("/speckit-specify {INTENT}");
    assert_eq!(started["sent"], sent.as_str());
    assert_eq!(started["folder"], "specs/002-rate-limit");
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    assert_eq!(v["draft"]["outcome"], "produced", "{v}");
    let worktree = PathBuf::from(v["worktree"].as_str().unwrap());

    let heard = std::fs::read_to_string(worktree.join(".devplane/heard.log")).unwrap();
    assert_eq!(
        heard,
        format!("{sent}\n---\n"),
        "the agent heard the invocation and the words, and nothing Devplane composed"
    );
    let env: Value = serde_json::from_str(
        &std::fs::read_to_string(worktree.join(".devplane/env.json")).unwrap(),
    )
    .unwrap();
    let dir = PathBuf::from(env["SPECIFY_FEATURE_DIRECTORY"].as_str().unwrap());
    assert_eq!(
        dir.canonicalize().unwrap(),
        worktree
            .join("specs/002-rate-limit")
            .canonicalize()
            .unwrap(),
        "Spec Kit was told the folder Devplane assigned, inside the worktree"
    );
    assert_eq!(env["SPECIFY_FEATURE_NO_PERSIST"], "1");
    assert!(
        env["DEVPLANE_RUN"].as_str().is_some(),
        "a driven run: {env}"
    );
    assert!(
        !repo.join("specs/002-rate-limit").exists(),
        "nothing landed in the person's checkout"
    );
    let says = v["draft_says"].to_string();
    assert!(says.contains("1.0.7") && says.contains("1.0.11"), "{says}");

    // OpenSpec: the installed propose workflow.
    let repo = openspec_repo("os-sent", "[gates]\ncheck = [\"true\"]\n", true);
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "Rate limit", "agent": "claude", "intent": INTENT}),
    )
    .await;
    assert!(started["error"].is_null(), "{started}");
    let sent = format!("/openspec-propose {INTENT}");
    assert_eq!(started["sent"], sent.as_str());
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    assert_eq!(v["draft"]["outcome"], "produced", "{v}");
    assert_eq!(
        v["draft"]["folder"], "openspec/changes/limit-login-attempts-per",
        "the change OpenSpec's flow made is the one found"
    );
    let worktree = PathBuf::from(v["worktree"].as_str().unwrap());
    let heard = std::fs::read_to_string(worktree.join(".devplane/heard.log")).unwrap();
    assert_eq!(heard, format!("{sent}\n---\n"));
    let env: Value = serde_json::from_str(
        &std::fs::read_to_string(worktree.join(".devplane/env.json")).unwrap(),
    )
    .unwrap();
    assert!(
        env.get("SPECIFY_FEATURE_DIRECTORY").is_none(),
        "OpenSpec is not told Spec Kit's variables"
    );
}

#[tokio::test]
async fn openspec_without_its_workflow_is_refused_naming_its_install_command() {
    let Some((addr, c, _host)) = boot_with("").await else {
        return;
    };
    let repo = openspec_repo("os-none", "", false);
    trust(&c, &addr, &repo).await;
    let r = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "x", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let says = r["error"].as_str().unwrap_or_default();
    assert!(says.contains("openspec init --tools claude"), "{r}");
    assert!(
        says.contains(".claude/skills/openspec-propose/SKILL.md"),
        "{r}"
    );
    assert!(!repo.join(".claude/worktrees").exists(), "nothing was made");
}

#[tokio::test]
async fn a_draft_is_refused_before_anything_starts() {
    let Some((addr, c, _host)) = boot_with("").await else {
        return;
    };
    let repo = speckit_repo("sk-refuse", "");
    trust(&c, &addr, &repo).await;

    // An agent with no Spec Kit integration installed.
    let r = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "x", "agent": "codex", "intent": INTENT}),
    )
    .await;
    assert_eq!(r["refused"], "not_installed", "{r}");
    let says = r["says"].as_str().unwrap();
    assert!(says.contains("installed: claude"), "{says}");
    assert!(says.contains("specify integration install codex"), "{says}");

    // No words.
    let r = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "x", "agent": "claude", "intent": "  "}),
    )
    .await;
    assert_eq!(r["refused"], "empty_intent", "{r}");
    assert!(
        !repo.join(".claude/worktrees").exists(),
        "no worktree was left behind by a refusal"
    );

    // GSD only: not offered.
    let gsd = repo_with(
        "gsd",
        &[(".planning/STATE.md", "# State\n"), ("devplane.toml", "")],
    );
    trust(&c, &addr, &gsd).await;
    let r = draft(
        &c,
        &addr,
        json!({"project": gsd, "name": "x", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let says = r["says"].as_str().unwrap();
    assert!(
        says.contains("drafting dispatches Spec Kit and OpenSpec only") && says.contains("GSD"),
        "{says}"
    );

    // Both: the person chooses.
    let both = repo_with(
        "both",
        &[
            ("devplane.toml", ""),
            (".specify/integration.json", INTEGRATION),
            ("openspec/config.yaml", "schema: spec-driven\n"),
            (".claude/skills/openspec-propose/SKILL.md", "#\n"),
        ],
    );
    trust(&c, &addr, &both).await;
    let r = draft(
        &c,
        &addr,
        json!({"project": both, "name": "x", "agent": "claude", "intent": INTENT, "preview": true}),
    )
    .await;
    assert!(r["says"].as_str().unwrap().contains("--methodology"), "{r}");
    assert_eq!(r["offered"], json!(["speckit", "openspec"]));
    let r = draft(
        &c,
        &addr,
        json!({"project": both, "name": "x", "agent": "claude", "intent": INTENT,
               "methodology": "openspec", "preview": true}),
    )
    .await;
    assert_eq!(r["sent"], format!("/openspec-propose {INTENT}"), "{r}");
}

#[tokio::test]
async fn a_draft_is_a_driven_session_whose_permissions_reach_the_inbox() {
    let Some((addr, c, _host)) = boot_with("DEVPLANE_ECHO_DRAFT_ASK=1").await else {
        return;
    };
    let repo = speckit_repo("sk-ask", "");
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "ask", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().expect("started").to_string();
    let ask = common::until("the draft's permission in the inbox", SETTLE, || async {
        let inbox = get(&c, &addr, "/api/inbox").await;
        inbox["items"]
            .as_array()?
            .iter()
            .find(|i| i["kind"] == "permission" && i["run_id"] == started["run"])
            .and_then(|i| i["ask"].as_str().map(str::to_string))
    })
    .await;
    let answered = post(
        &c,
        &addr,
        &format!("/api/asks/{ask}/answer"),
        json!({"decision": "allow"}),
    )
    .await;
    assert!(answered["error"].is_null(), "{answered}");
    let v = settled(&c, &addr, &id).await;
    assert_eq!(v["draft"]["outcome"], "produced", "{v}");
    let run = started["run"].as_str().unwrap();
    let decisions = get(&c, &addr, &format!("/api/decisions?about={run}")).await;
    assert!(
        decisions.to_string().contains("person"),
        "the permission's answer is on the record as the person's: {decisions}"
    );
}

#[tokio::test]
async fn two_drafts_at_once_get_two_numbers() {
    let Some((addr, c, _host)) = boot_with("DEVPLANE_ECHO_DRAFT_NOTHING=1").await else {
        return;
    };
    let repo = speckit_repo("sk-numbers", "");
    trust(&c, &addr, &repo).await;
    // An open change whose worktree already holds `specs/003-x`.
    let first = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "first", "agent": "claude", "intent": INTENT}),
    )
    .await;
    assert_eq!(first["folder"], "specs/002-first", "{first}");
    let wt = PathBuf::from(first["worktree"].as_str().unwrap());
    std::fs::create_dir_all(wt.join("specs/003-x")).unwrap();

    let mut seen = vec!["002".to_string(), "003".to_string()];
    for round in 0..20 {
        let a = json!({"project": repo, "name": format!("a{round}"), "agent": "claude", "intent": INTENT});
        let b = json!({"project": repo, "name": format!("b{round}"), "agent": "claude", "intent": INTENT});
        let (ra, rb) = tokio::join!(draft(&c, &addr, a), draft(&c, &addr, b));
        for r in [ra, rb] {
            let folder = r["folder"].as_str().unwrap_or_else(|| panic!("{r}"));
            let number = folder["specs/".len()..][..3].to_string();
            assert!(
                !seen.contains(&number),
                "round {round}: {folder} reuses a number already taken ({seen:?})"
            );
            seen.push(number);
        }
    }
}

#[tokio::test]
async fn devplane_writes_nothing_into_a_draft() {
    let Some((addr, c, host)) = boot_with("").await else {
        return;
    };
    let config = "[gates]\ncheck = [\"true\"]\nbase_checks = \"auto\"\n";

    let repo = speckit_repo("sk-nothing", config);
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "nothing", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    let worktree = PathBuf::from(v["worktree"].as_str().unwrap());
    nothing_by_devplane(&worktree, "specs/002-nothing");
    assert_eq!(
        v["gates"].as_array().map(Vec::len),
        Some(0),
        "no gate runs on a draft nobody accepted: {v}"
    );
    let decisions = get(&c, &addr, &format!("/api/decisions?about={id}")).await;
    assert!(
        !decisions.to_string().contains("gate:"),
        "no gate and no base checks are recorded for a pending draft: {decisions}"
    );
    let _ = &host;

    let repo = openspec_repo("os-nothing", config, true);
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "nothing", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    let worktree = PathBuf::from(v["worktree"].as_str().unwrap());
    nothing_by_devplane(&worktree, v["draft"]["folder"].as_str().unwrap());
}

#[tokio::test]
async fn a_draft_is_reviewed_then_accepted_or_rejected_by_a_person() {
    let Some((addr, c, _host)) = boot_with("DEVPLANE_ECHO_DRAFT_OUTSIDE=notes.txt").await else {
        return;
    };
    let repo = speckit_repo("sk-review", "");
    trust(&c, &addr, &repo).await;

    // Reviewed like code: the folder is what was asked for, nothing else is.
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "accept me", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    settled(&c, &addr, &id).await;
    let review = get(&c, &addr, &format!("/api/changes/{id}/review")).await;
    let asked: Vec<&str> = review["intent"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["files"].as_array().unwrap().iter())
        .filter_map(Value::as_str)
        .collect();
    assert!(
        asked.contains(&"specs/002-accept-me/spec.md"),
        "the drafted folder is what was asked for: {review}"
    );
    let not_asked: Vec<&str> = review["intent"]["not_asked_for"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["path"].as_str())
        .collect();
    assert_eq!(not_asked, ["notes.txt"], "{review}");

    let accepted = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/draft/accept"),
        json!({}),
    )
    .await;
    assert_eq!(accepted["spec"], "specs/002-accept-me", "{accepted}");
    let v = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(v["spec"], "specs/002-accept-me");
    assert_eq!(v["draft"]["outcome"], "accepted");
    let decisions = get(&c, &addr, &format!("/api/decisions?about={id}")).await;
    let accept = decisions
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "change:draft.accept")
        .unwrap_or_else(|| panic!("{decisions}"));
    assert_eq!(accept["authority"], "person");
    let again = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "x", "agent": "claude", "intent": INTENT, "change": id}),
    )
    .await;
    assert_eq!(
        again["refused"], "has_spec",
        "a change answers one specification: {again}"
    );

    // Rejected: the worktree goes back, the record stays.
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "reject me", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    let worktree = PathBuf::from(v["worktree"].as_str().unwrap());
    let folder = v["draft"]["folder"].as_str().unwrap().to_string();
    assert!(worktree.join(&folder).exists());
    let rejected = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/draft/reject"),
        json!({}),
    )
    .await;
    assert_eq!(rejected["removed"], json!([folder]), "{rejected}");
    assert!(
        !worktree.join(&folder).exists(),
        "the drafted folder is gone"
    );
    assert!(
        !worktree.join("notes.txt").exists(),
        "so is the untracked file"
    );
    let v = get(&c, &addr, &format!("/api/changes/{id}")).await;
    assert_eq!(v["draft"]["outcome"], "rejected");
    assert_eq!(
        v["runs"].as_array().map(Vec::len),
        Some(1),
        "the run is kept"
    );
    let decisions = get(&c, &addr, &format!("/api/decisions?about={id}")).await;
    assert!(
        decisions.to_string().contains("change:draft.reject"),
        "{decisions}"
    );
    // After a rejection the change may be drafted again.
    let again = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "x", "agent": "claude", "intent": INTENT, "change": id}),
    )
    .await;
    assert!(again["error"].is_null(), "{again}");
}

#[tokio::test]
async fn a_draft_that_wrote_nothing_says_so() {
    let Some((addr, c, _host)) = boot_with("DEVPLANE_ECHO_DRAFT_NOTHING=1").await else {
        return;
    };
    let repo = speckit_repo("sk-empty", "");
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "empty", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    assert_eq!(v["draft"]["outcome"], "none_produced");
    assert!(
        v["draft_says"]
            .to_string()
            .contains("the command produced no specification at specs/002-empty"),
        "{v}"
    );
    let r = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/draft/accept"),
        json!({}),
    )
    .await;
    assert_eq!(r["refused"], "none_produced", "{r}");
}

/// The marker lines of the fixture's spec: two bracketed, one not, and one
/// inside the quality checklist, which is not a question.
fn marker_items(inbox: &Value, change: &str) -> Vec<Value> {
    inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == "spec_question" && i["change_id"] == change)
        .cloned()
        .collect()
}

#[tokio::test]
async fn three_markers_three_items() {
    let Some((addr, c, _host)) = boot_with("").await else {
        return;
    };
    let repo = speckit_repo(
        "sk-markers",
        "[spec]\nopen_questions = [\"NEEDS CLARIFICATION\"]\n",
    );
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "markers", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    let worktree = PathBuf::from(v["worktree"].as_str().unwrap());

    let items = marker_items(&get(&c, &addr, "/api/inbox").await, &id);
    assert_eq!(
        items.len(),
        3,
        "one item per marker line, none folded: {items:?}"
    );
    let path = "specs/002-markers/spec.md";
    let lines: Vec<u64> = items
        .iter()
        .map(|i| {
            assert_eq!(i["marker"]["path"], path);
            i["marker"]["line"].as_u64().unwrap()
        })
        .collect();
    assert_eq!(lines, [8, 9, 10]);
    let bracketed: Vec<bool> = items
        .iter()
        .map(|i| i["marker"]["bracketed"].as_bool().unwrap())
        .collect();
    assert_eq!(bracketed, [true, true, false]);
    assert!(items[0]["actions"].to_string().contains("answer"));
    assert!(items[2]["actions"].to_string().contains("tell_agent"));
    assert!(
        items[0]["marker"]["text"]
            .as_str()
            .unwrap()
            .contains("[NEEDS CLARIFICATION: which timeout applies?]")
    );

    // A marker that goes, by any route, takes its item with it.
    let file = worktree.join(path);
    let text = std::fs::read_to_string(&file).unwrap();
    let without: String = text
        .lines()
        .filter(|l| !l.contains("FR-004"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&file, without).unwrap();
    let items = marker_items(&get(&c, &addr, "/api/inbox").await, &id);
    assert_eq!(items.len(), 2, "{items:?}");
}

#[tokio::test]
async fn an_answer_is_a_diff_the_person_confirms() {
    let Some((addr, c, _host)) = boot_with("").await else {
        return;
    };
    let repo = speckit_repo(
        "sk-clarify",
        "[spec]\nopen_questions = [\"NEEDS CLARIFICATION\"]\n",
    );
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "clarify", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    let worktree = PathBuf::from(v["worktree"].as_str().unwrap());
    let path = "specs/002-clarify/spec.md";
    let file = worktree.join(path);
    let before = std::fs::read_to_string(&file).unwrap();
    let line8 = before.lines().nth(7).unwrap().to_string();

    let shown = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/clarify"),
        json!({"path": path, "line": 8, "text": line8, "answer": "fifteen minutes", "confirm": false}),
    )
    .await;
    let diff = shown["diff"].as_str().unwrap_or_else(|| panic!("{shown}"));
    assert!(
        diff.contains("+- **FR-002**: A lockout lasts fifteen minutes."),
        "{diff}"
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        before,
        "nothing written yet"
    );

    let written = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/clarify"),
        json!({"path": path, "line": 8, "text": line8, "answer": "fifteen minutes", "confirm": true}),
    )
    .await;
    assert_eq!(written["written"], true, "{written}");
    let after = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        after,
        before.replace(
            "[NEEDS CLARIFICATION: which timeout applies?]",
            "fifteen minutes"
        ),
        "exactly one span in one file"
    );
    let decisions = get(&c, &addr, &format!("/api/decisions?about={id}")).await;
    assert!(
        decisions.to_string().contains("spec:clarify"),
        "{decisions}"
    );

    // The line moved: refused.
    let moved = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/clarify"),
        json!({"path": path, "line": 8, "text": line8, "answer": "x", "confirm": true}),
    )
    .await;
    assert!(
        moved["error"].as_str().unwrap().contains("read it again"),
        "{moved}"
    );

    // Unbracketed: not edited by Devplane.
    let line10 = before.lines().nth(9).unwrap().to_string();
    let r = post(
        &c,
        &addr,
        &format!("/api/changes/{id}/clarify"),
        json!({"path": path, "line": 10, "text": line10, "answer": "the main one", "confirm": true}),
    )
    .await;
    assert!(
        r["error"].as_str().unwrap().contains("change prompt"),
        "{r}"
    );
}

#[tokio::test]
async fn the_marker_setting_is_offered_after_a_draft_only_where_the_tool_defines_one() {
    let Some((addr, c, _host)) = boot_with("").await else {
        return;
    };
    let config = "[gates]\ncheck = [\"true\"]\n\n[spec]\nplans = \"specs\"\n";
    let repo = speckit_repo("sk-offer", config);
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "offer", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    let offer = &v["open_questions_offer"];
    let diff = offer["diff"].as_str().unwrap_or_else(|| panic!("{v}"));
    assert!(
        diff.contains("+open_questions = [\"NEEDS CLARIFICATION\"]"),
        "{diff}"
    );
    assert!(
        diff.contains("Devplane"),
        "fenced by a comment naming Devplane: {diff}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("devplane.toml")).unwrap(),
        config,
        "nothing is written until accepted"
    );
    // A project's id is its root, so it is escaped into the route.
    let project = devplane::core::ProjectId::from_path(&repo)
        .as_str()
        .replace('%', "%25")
        .replace('/', "%2F");
    let stale = post(
        &c,
        &addr,
        &format!("/api/projects/{project}/config/open-questions"),
        json!({"accept": true, "base": "not-it"}),
    )
    .await;
    assert_eq!(stale["refused"], "changed", "{stale}");
    let written = post(
        &c,
        &addr,
        &format!("/api/projects/{project}/config/open-questions"),
        json!({"accept": true, "base": offer["base"]}),
    )
    .await;
    assert_eq!(written["written"], true, "{written}");
    let cfg = devplane::core::ProjectConfig::load(&repo).unwrap();
    assert_eq!(cfg.spec.open_questions, ["NEEDS CLARIFICATION"]);
    assert_eq!(
        cfg.spec.plans.as_deref(),
        Some("specs"),
        "the rest of the file is untouched"
    );

    let repo = openspec_repo("os-offer", "[gates]\ncheck = [\"true\"]\n", true);
    trust(&c, &addr, &repo).await;
    let started = draft(
        &c,
        &addr,
        json!({"project": repo, "name": "offer", "agent": "claude", "intent": INTENT}),
    )
    .await;
    let id = started["change"].as_str().unwrap().to_string();
    let v = settled(&c, &addr, &id).await;
    let offer = &v["open_questions_offer"];
    assert!(
        offer["diff"].is_null(),
        "no word is invented for OpenSpec: {offer}"
    );
    assert!(
        offer["says"]
            .as_str()
            .unwrap()
            .contains("OpenSpec defines no unresolved-question word")
    );
}

/// The reader names each unresolved line by its line number, and a quality
/// checklist's row about markers is not a question.
#[test]
fn questions_carry_their_line_and_checklists_are_not_questions() {
    let root = common::scratch("draft-lines");
    let dir = root.join("specs/001-x");
    std::fs::create_dir_all(dir.join("checklists")).unwrap();
    std::fs::write(
        dir.join("spec.md"),
        "# X\n\n- A [NEEDS CLARIFICATION: a?]\n- B\n- C [NEEDS CLARIFICATION: c?]\n- D NEEDS CLARIFICATION d\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("checklists/requirements.md"),
        "- [ ] No [NEEDS CLARIFICATION] markers remain\n",
    )
    .unwrap();
    let markers = vec!["NEEDS CLARIFICATION".to_string()];
    let plan = devplane::spec::Plan::read(&root, "specs/001-x", &markers);
    let got: Vec<(String, u32)> = plan
        .questions
        .iter()
        .map(|q| (q.path.clone(), q.line))
        .collect();
    let spec = "specs/001-x/spec.md".to_string();
    assert_eq!(got, [(spec.clone(), 3), (spec.clone(), 5), (spec, 6)]);
    let _ = std::fs::remove_dir_all(&root);
}

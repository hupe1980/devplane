//! The grant a person wrote, end to end through the host: offered only after
//! their repeated, identical answers from the window; written only with the
//! window's proof; answering the next call; revoked with every decision it
//! made listed.

mod common;

use common::{WINDOW_KEY, boot, echo_spec, trust};
use devplane::core::ask::{AnswerPath, Ask, Asked, Deadline, Kind};
use devplane::core::{AskId, ProjectId, RunId};
use devplane::host::Shared;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

fn echo_sparse() -> Option<devplane::acp::AgentSpec> {
    let base = echo_spec()?;
    Some(devplane::acp::AgentSpec::new(
        "echo",
        "Echo",
        &format!("env DEVPLANE_ECHO_SPARSE=1 {}", base.command),
    ))
}

/// A permission a person already answered, as the ask row records it.
async fn answered(
    state: &Shared,
    project: &ProjectId,
    command: &str,
    choice: &str,
    from: AnswerPath,
) {
    let mut ask = Ask::new(
        AskId::new(devplane::core::ids::new_event_id()),
        RunId::new("earlier".to_string()),
        Asked {
            kind: Kind::Permission,
            request_id: "r".into(),
            message: format!("Bash · {command}"),
            payload: json!({
                "call": { "tool": "Bash", "input": { "command": command } },
                "options": [
                    { "id": "allow", "label": "Allow once", "kind": "allow_once" },
                    { "id": "deny", "label": "Deny", "kind": "reject_once" },
                ],
            }),
            at: jiff::Timestamp::now(),
            deadline: Deadline::Never,
        },
    )
    .in_project(Some(project.clone()));
    ask.answer(
        json!({ "permission": choice }),
        from,
        jiff::Timestamp::now(),
    )
    .unwrap();
    state.store.save_ask(&ask).await.unwrap();
}

/// The permission item in the inbox for `run`.
async fn item(c: &reqwest::Client, addr: &std::net::SocketAddr, run: &RunId) -> Value {
    let inbox = common::get(c, addr, "/api/inbox").await;
    inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "permission" && i["run_id"] == run.as_str())
        .cloned()
        .unwrap_or_else(|| panic!("no permission item for {run:?}: {inbox}"))
}

async fn waiting(state: &Shared, run: &RunId) -> Ask {
    common::until("the permission", Duration::from_secs(30), || async {
        state
            .store
            .open_asks()
            .await
            .unwrap()
            .into_iter()
            .find(|a| &a.run == run)
    })
    .await
}

async fn post_proved(
    c: &reqwest::Client,
    addr: &std::net::SocketAddr,
    path: &str,
    header: Option<(&str, String)>,
    body: Value,
) -> (u16, Value) {
    let mut req = c
        .post(format!("http://{addr}{path}"))
        .bearer_auth("tok")
        .json(&body);
    if let Some((h, v)) = header {
        req = req.header(h, v);
    }
    let res = req.send().await.unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

async fn dispatch(state: &Shared, agent: &devplane::acp::AgentSpec, repo: &Path) -> RunId {
    devplane::driven::dispatch(
        state,
        agent,
        repo.to_path_buf(),
        Some("run a tool please".into()),
        Vec::new(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn a_grant_is_offered_after_repeated_window_answers_written_on_proof_and_revoked_with_its_decisions()
 {
    let Some(agent) = echo_sparse() else { return };
    let (addr, c, state) = boot().await;
    let repo = common::scratch_repo("grant-offer", "");
    trust(&c, &addr, &repo).await;
    let root = repo.canonicalize().unwrap().display().to_string();
    std::fs::create_dir_all(&state.home).unwrap();
    std::fs::write(
        state.home.join("policy.toml"),
        "# the person's own notes\n[grants]\noffer_after = 3\noffer_again_after = 3\n",
    )
    .unwrap();

    let run = dispatch(&state, &agent, &repo).await;
    let ask = waiting(&state, &run).await;
    let project = ask.project.clone().expect("the ask names its project");

    // Answers any token holder could send never count.
    for _ in 0..5 {
        answered(&state, &project, "rm -rf build", "allow", AnswerPath::Cli).await;
    }
    let first = item(&c, &addr, &run).await;
    assert!(first.get("grant_offer").is_none(), "{first}");

    // Three allows from the window: the exact rule, with what it came from.
    for _ in 0..3 {
        answered(
            &state,
            &project,
            "rm -rf build",
            "allow",
            AnswerPath::Window,
        )
        .await;
    }
    let offered = item(&c, &addr, &run).await;
    let offer = &offered["grant_offer"];
    assert_eq!(offer["rule"], "Bash(rm -rf build)", "{offered}");
    assert_eq!(offer["root"], root.as_str());
    assert_eq!(offer["allowed"], 3);
    assert_eq!(offer["derived_from"].as_array().unwrap().len(), 3);
    assert!(
        offer["writes_says"]
            .as_str()
            .unwrap()
            .contains("does not know what this command itself touches")
    );
    assert!(
        offer["diff"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["op"] == "+" && l["text"] == "  \"Bash(rm -rf build)\","),
        "{offer}"
    );

    let body = json!({ "root": root, "rule": "Bash(rm -rf build)" });
    // Without the window's proof: refused, and the file is untouched.
    let (status, said) = post_proved(&c, &addr, "/api/grants", None, body.clone()).await;
    assert_eq!(status, 403, "{said}");
    let before = std::fs::read_to_string(state.home.join("policy.toml")).unwrap();
    assert!(!before.contains("rm -rf build"));
    // A rule the ledger does not offer: refused even with a proof.
    let other = json!({ "root": root, "rule": "Bash(cargo test)" });
    let (status, _) = post_proved(
        &c,
        &addr,
        "/api/grants",
        Some((
            "X-Devplane-Grant-Proof",
            devplane::config::grant_proof(WINDOW_KEY, &root, "Bash(cargo test)"),
        )),
        other,
    )
    .await;
    assert_eq!(status, 409);
    // With it: written, the person's comment kept.
    let (status, said) = post_proved(
        &c,
        &addr,
        "/api/grants",
        Some((
            "X-Devplane-Grant-Proof",
            devplane::config::grant_proof(WINDOW_KEY, &root, "Bash(rm -rf build)"),
        )),
        body.clone(),
    )
    .await;
    assert_eq!(status, 200, "{said}");
    let file = std::fs::read_to_string(state.home.join("policy.toml")).unwrap();
    assert!(file.starts_with("# the person's own notes"), "{file}");
    assert!(file.contains("\"Bash(rm -rf build)\""), "{file}");

    // The next run's call is answered by the grant, and recorded under it.
    let next = dispatch(&state, &agent, &repo).await;
    let name = format!("`Bash(rm -rf build)` for {root}");
    let rows = common::until("the grant's decision", Duration::from_secs(30), || async {
        let rows = state.store.decisions_by_grant(&name).await.unwrap();
        if let Some(a) = state
            .store
            .open_asks()
            .await
            .unwrap()
            .iter()
            .find(|a| a.run == next)
        {
            panic!(
                "asked the person instead: {a:?}\n{:?}",
                state.policy.grants()
            );
        }
        (!rows.is_empty()).then_some(rows)
    })
    .await;
    assert_eq!(rows[0].run_id.as_ref(), Some(&next));
    assert_eq!(rows[0].authority, devplane::core::Authority::Rule);

    // Revoking needs the proof too, and lists every decision the grant made.
    let (status, _) = post_proved(&c, &addr, "/api/grants/revoke", None, body.clone()).await;
    assert_eq!(status, 403);
    let (status, said) = post_proved(
        &c,
        &addr,
        "/api/grants/revoke",
        Some((
            "X-Devplane-Revoke-Proof",
            devplane::config::revoke_proof(WINDOW_KEY, &root, "Bash(rm -rf build)"),
        )),
        body.clone(),
    )
    .await;
    assert_eq!(status, 200, "{said}");
    let listed: Vec<&str> = said["decided"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["id"].as_str())
        .collect();
    assert_eq!(listed, [rows[0].id.as_str()], "{said}");
    let file = std::fs::read_to_string(state.home.join("policy.toml")).unwrap();
    assert!(!file.contains("rm -rf build"), "{file}");
    assert!(file.contains("# the person's own notes"));

    // With the grant gone, the next call asks the person again.
    let again = dispatch(&state, &agent, &repo).await;
    waiting(&state, &again).await;
}

#[tokio::test]
async fn inconsistent_answers_say_why_no_grant_is_offered_and_a_lapsed_grant_says_it_lapsed() {
    let Some(agent) = echo_sparse() else { return };
    let (addr, c, state) = boot().await;
    let repo = common::scratch_repo("grant-mixed", "");
    trust(&c, &addr, &repo).await;
    let root = repo.canonicalize().unwrap().display().to_string();
    let run = dispatch(&state, &agent, &repo).await;
    let ask = waiting(&state, &run).await;
    let project = ask.project.clone().unwrap();
    for _ in 0..5 {
        answered(
            &state,
            &project,
            "rm -rf build",
            "allow",
            AnswerPath::Window,
        )
        .await;
    }
    answered(&state, &project, "rm -rf build", "deny", AnswerPath::Window).await;
    let it = item(&c, &addr, &run).await;
    assert!(it.get("grant_offer").is_none(), "{it}");
    assert!(
        it["grant_no"]
            .as_str()
            .unwrap_or("")
            .contains("refused it 1 time"),
        "{it}"
    );

    // A grant whose `until` passed answers nothing, and the item says so.
    std::fs::create_dir_all(&state.home).unwrap();
    std::fs::write(
        state.home.join("policy.toml"),
        format!(
            "[projects.{root:?}]\nallow = [{{ rule = \"Bash(rm -rf build)\", until = \"2020-01-01\" }}]\n"
        ),
    )
    .unwrap();
    let lapsed = common::until("the lapsed note", Duration::from_secs(10), || async {
        let it = item(&c, &addr, &run).await;
        it["grant_lapsed"].as_str().map(str::to_string)
    })
    .await;
    assert!(lapsed.contains("lapsed"), "{lapsed}");
    let next = dispatch(&state, &agent, &repo).await;
    waiting(&state, &next).await;
}

fn devplane(home: &Path, args: &[&str], agent: bool) -> (bool, String) {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_devplane"));
    cmd.args(args).env("DEVPLANE_HOME", home);
    for var in devplane::hook::AGENT_SESSION_VARS {
        cmd.env_remove(var);
    }
    if agent {
        cmd.env("CLAUDECODE", "1");
    }
    let out = cmd.output().expect("devplane runs");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// `devplane config check` refuses by name a repository that tries to grant,
/// and a person's file with a global switch, a low threshold or a wildcard.
#[test]
fn config_check_refuses_every_grant_it_must_by_name() {
    let home = common::scratch("grant-check-home");
    let repo = common::scratch_repo("grant-check", "[policy]\nallow = [\"Bash(ls)\"]\n");
    let (ok, said) = devplane(&home, &["config", "check", repo.to_str().unwrap()], false);
    assert!(!ok && said.contains("`[policy] allow`"), "{said}");
    assert!(said.contains("a repository cannot grant"), "{said}");

    let clean = common::scratch_repo("grant-check-clean", "");
    let root = clean.display().to_string();
    for (file, names) in [
        ("[policy]\nallow = [\"Bash(ls)\"]\n", "every project"),
        ("[grants]\noffer_after = 1\n", "below 2"),
        (
            &*format!("[projects.{root:?}]\nallow = [\"Bash(*)\"]\n"),
            "wildcard where the command goes",
        ),
    ] {
        std::fs::write(home.join("policy.toml"), file).unwrap();
        let (ok, said) = devplane(&home, &["config", "check", &root], false);
        assert!(!ok && said.contains(names), "{file}: {said}");
    }
    std::fs::write(
        home.join("policy.toml"),
        format!("[projects.{root:?}]\nallow = [\"Bash(cargo test)\"]\n"),
    )
    .unwrap();
    let (ok, said) = devplane(&home, &["config", "check", &root], false);
    assert!(ok, "{said}");
    assert!(
        said.contains("widening the agent: 1 grant — `Bash(cargo test)`"),
        "{said}"
    );
}

/// Revoking from a terminal lists every decision the grant made, and is
/// refused inside an agent's session.
#[tokio::test]
async fn revoking_from_a_terminal_lists_every_decision_it_made() {
    let home = common::scratch("grant-revoke-home");
    let repo = common::scratch_repo("grant-revoke", "");
    let root = repo.display().to_string();
    std::fs::write(
        home.join("policy.toml"),
        format!("# mine\n[projects.{root:?}]\nallow = [\"Bash(cargo test)\"]\n"),
    )
    .unwrap();
    let name = format!("`Bash(cargo test)` for {root}");
    {
        let store = devplane::store::Store::open(&home.join("devplane.db"))
            .await
            .unwrap();
        for subject in ["Bash: cargo test", "Bash: cargo test"] {
            let mut d = devplane::core::Decision::new(
                devplane::core::Authority::Rule,
                "agent:tool.use",
                subject,
                "allow",
            );
            d.grant = Some(name.clone());
            store.append_decision(&d).await.unwrap();
        }
    }
    let (ok, said) = devplane(
        &home,
        &["config", "revoke", "Bash(cargo test)", "--dir", &root],
        true,
    );
    assert!(
        !ok && said.contains("may not change the person's grants"),
        "{said}"
    );
    assert!(
        std::fs::read_to_string(home.join("policy.toml"))
            .unwrap()
            .contains("cargo test")
    );
    let (ok, said) = devplane(
        &home,
        &[
            "--json",
            "config",
            "revoke",
            "Bash(cargo test)",
            "--dir",
            &root,
        ],
        false,
    );
    assert!(ok, "{said}");
    let v: Value = serde_json::from_str(&said).unwrap();
    assert_eq!(v["decided"].as_array().unwrap().len(), 2, "{v}");
    let file = std::fs::read_to_string(home.join("policy.toml")).unwrap();
    assert!(
        !file.contains("cargo test") && file.contains("# mine"),
        "{file}"
    );
}

/// A grant's reversals are counts from an observable event — a gate that
/// failed on a change after the grant decided on it — and absent where none
/// of its calls was on a change.
#[tokio::test]
async fn reversals_are_counted_only_where_they_can_be_observed() {
    let db = common::scratch("grant-rev").join("devplane.db");
    let store = devplane::store::Store::open(&db).await.unwrap();
    let text = "[projects.\"/r\"]\nallow = [\"Bash(cargo test)\", \"Bash(cargo fmt)\"]\n";
    let grants = devplane::core::GlobalConfig::parse(text)
        .unwrap()
        .grants()
        .unwrap();
    let change = devplane::core::ChangeId::new("ch-1".to_string());
    let t = |s: &str| -> jiff::Timestamp { s.parse().unwrap() };
    for (rule, at, on_change) in [
        ("Bash(cargo test)", "2026-10-01T10:00:00Z", true),
        ("Bash(cargo test)", "2026-10-01T12:00:00Z", true),
        ("Bash(cargo fmt)", "2026-10-01T10:00:00Z", false),
    ] {
        let mut d = devplane::core::Decision::new_at(
            devplane::core::Authority::Rule,
            "agent:tool.use",
            rule,
            "allow",
            t(at),
        );
        d.grant = Some(format!("`{rule}` for /r"));
        if on_change {
            d = d.for_change(&change);
        }
        store.append_decision(&d).await.unwrap();
    }
    store
        .append_decision(
            &devplane::core::Decision::new_at(
                devplane::core::Authority::Devplane,
                "gate:run",
                "cargo test",
                "fail",
                t("2026-10-01T11:00:00Z"),
            )
            .for_change(&change),
        )
        .await
        .unwrap();
    let rows = devplane::view::grant_rows(&store, &grants, t("2026-10-04T00:00:00Z")).await;
    let test = rows.iter().find(|r| r.rule == "Bash(cargo test)").unwrap();
    assert_eq!(test.decided, 2);
    assert_eq!(
        test.reversed,
        Some(1),
        "only the decision before the failure"
    );
    assert!(test.reversals[0].cause.contains("a gate failed"));
    let fmt = rows.iter().find(|r| r.rule == "Bash(cargo fmt)").unwrap();
    assert_eq!(fmt.decided, 1);
    assert_eq!(
        fmt.reversed, None,
        "absent, not zero, where nothing could be observed"
    );
}

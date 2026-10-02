//! The window's answers wait a few seconds before they reach the agent, so a
//! slip can be taken back. While held nothing is recorded or delivered; `z`
//! (the undo route) withdraws it with nothing on the record; a second answer
//! sends the first at once; and a host that stops drops the held answer, says
//! so before it quits, and leaves the ask open. The command line's answers are
//! never held. The path the host proved when the answer was held is the one
//! recorded when it is sent.

use devplane::core::Policy;
use devplane::host::{AppState, Shared};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::Duration;

const TOKEN: &str = "tok";

/// Past the hold, with room for a slow machine.
const PAST_THE_WINDOW: Duration = Duration::from_millis(6_500);

async fn boot(key: Option<&str>) -> (SocketAddr, reqwest::Client, Shared) {
    let db = std::env::temp_dir().join(format!("dp-hold-{}.db", uuid::Uuid::new_v4().simple()));
    let state = AppState::new(
        db.clone(),
        TOKEN.into(),
        Policy::default(),
        db.parent().unwrap().to_path_buf(),
    )
    .await
    .unwrap();
    if let Some(k) = key {
        state.window_key.set(k.to_string()).unwrap();
    }
    let app = devplane::api::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    (addr, reqwest::Client::new(), state)
}

/// A permission the hook is holding open (no protocol request), as a restart
/// leaves one: the row is the delivery.
async fn waiting(state: &Shared, id: &str) {
    let ask = devplane::core::ask::Ask::new(
        devplane::core::AskId::new(id),
        devplane::core::RunId::new(format!("run-{id}")),
        devplane::core::ask::Asked {
            kind: devplane::core::ask::Kind::Permission,
            request_id: String::new(),
            message: format!("Bash · rm {id}"),
            payload: json!({
                "tool": "Bash",
                "call": format!("rm {id}"),
                "options": [{"id": "allow_once"}, {"id": "reject_once"}],
            }),
            at: jiff::Timestamp::now(),
            deadline: devplane::core::ask::Deadline::Never,
        },
    );
    state.store.save_ask(&ask).await.unwrap();
}

/// Posts a deny (an allow is refused from inside an agent session, where these
/// tests may run); with `hold`, as the window does.
async fn answer(
    c: &reqwest::Client,
    addr: &SocketAddr,
    id: &str,
    hold: bool,
    extra: &[(&str, String)],
) -> (u16, Value) {
    let mut body = json!({ "decision": "deny" });
    if hold {
        body["hold"] = json!(true);
    }
    let mut req = c
        .post(format!("http://{addr}/api/asks/{id}/answer"))
        .bearer_auth(TOKEN)
        .json(&body);
    for (k, v) in extra {
        req = req.header(*k, v);
    }
    let res = req.send().await.unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

async fn undo(
    c: &reqwest::Client,
    addr: &SocketAddr,
    id: &str,
    extra: &[(&str, String)],
) -> (u16, Value) {
    let mut req = c
        .post(format!("http://{addr}/api/asks/{id}/undo"))
        .bearer_auth(TOKEN);
    for (k, v) in extra {
        req = req.header(*k, v);
    }
    let res = req.send().await.unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

async fn open(state: &Shared, id: &str) -> bool {
    state.store.ask(id).await.unwrap().unwrap().is_open()
}

async fn decisions(state: &Shared) -> usize {
    state.store.decisions(None, 100).await.unwrap().len()
}

async fn inbox(c: &reqwest::Client, addr: &SocketAddr) -> Value {
    c.get(format!("http://{addr}/api/inbox"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn a_held_answer_reaches_nothing_until_its_window_ends_then_is_sent_once() {
    let (addr, c, state) = boot(None).await;
    waiting(&state, "a").await;
    let before = decisions(&state).await;

    let (status, res) = answer(&c, &addr, "a", true, &[]).await;
    assert_eq!(status, 202, "{res}");
    assert_eq!(res["held"]["ask"], "a", "{res}");
    assert_eq!(res["held"]["says"], "deny", "{res}");
    assert!(res["committed"].is_null(), "nothing was held before: {res}");
    assert!(open(&state, "a").await, "a held answer closed the ask");
    assert_eq!(
        decisions(&state).await,
        before,
        "a held answer was recorded"
    );
    let feed = inbox(&c, &addr).await;
    assert_eq!(
        feed["held"]["ask"], "a",
        "the inbox does not say what is held: {feed}"
    );

    tokio::time::sleep(PAST_THE_WINDOW).await;
    let ask = state.store.ask("a").await.unwrap().unwrap();
    assert!(
        !ask.is_open(),
        "the window ended and the answer was never sent"
    );
    assert_eq!(ask.answered_from.as_deref(), Some("cli"));
    assert!(
        matches!(ask.ended, Some(devplane::core::ask::Ended::Person)),
        "the timer is recorded as the decider: {:?}",
        ask.ended
    );
    assert_eq!(decisions(&state).await, before + 1, "sent exactly once");
    let feed = inbox(&c, &addr).await;
    assert!(feed["held"].is_null(), "{feed}");
    assert_eq!(feed["landed"]["ok"], true, "{feed}");

    let (status, res) = undo(&c, &addr, "a", &[]).await;
    assert_eq!(status, 409, "{res}");
    assert!(
        res["error"]
            .as_str()
            .unwrap_or("")
            .contains("can no longer be taken back"),
        "{res}"
    );
}

#[tokio::test]
async fn an_undo_in_the_window_leaves_nothing_on_the_record_and_the_ask_open() {
    let (addr, c, state) = boot(None).await;
    waiting(&state, "a").await;
    let before = decisions(&state).await;

    assert_eq!(answer(&c, &addr, "a", true, &[]).await.0, 202);
    let (status, res) = undo(&c, &addr, "a", &[]).await;
    assert_eq!(status, 200, "{res}");
    assert_eq!(res["withdrawn"], "a");

    tokio::time::sleep(PAST_THE_WINDOW).await;
    assert!(
        open(&state, "a").await,
        "a withdrawn answer was sent anyway"
    );
    assert_eq!(
        decisions(&state).await,
        before,
        "a withdrawn answer left a decision"
    );
    assert!(inbox(&c, &addr).await["held"].is_null());
}

#[tokio::test]
async fn a_second_answer_sends_the_first_at_once_and_only_the_newest_can_be_taken_back() {
    let (addr, c, state) = boot(None).await;
    waiting(&state, "a").await;
    waiting(&state, "b").await;

    assert_eq!(answer(&c, &addr, "a", true, &[]).await.0, 202);
    let (status, res) = answer(&c, &addr, "b", true, &[]).await;
    assert_eq!(status, 202, "{res}");
    assert_eq!(res["committed"], "a", "{res}");
    assert!(
        !open(&state, "a").await,
        "the first answer was not sent at once"
    );
    assert!(open(&state, "b").await);

    assert_eq!(
        undo(&c, &addr, "a", &[]).await.0,
        409,
        "the sent answer was undone"
    );
    assert_eq!(undo(&c, &addr, "b", &[]).await.0, 200);
    assert!(open(&state, "b").await);
}

#[tokio::test]
async fn quitting_names_the_held_answer_and_drops_it() {
    let (addr, c, state) = boot(None).await;
    waiting(&state, "a").await;
    let before = decisions(&state).await;
    assert_eq!(answer(&c, &addr, "a", true, &[]).await.0, 202);

    let q: Value = c
        .get(format!("http://{addr}/api/quitting"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let says = q["says"].as_str().unwrap_or("");
    assert!(
        says.contains("Bash · rm a") && says.contains("will not reach the agent"),
        "the quit sentence does not name the held answer: {says}"
    );

    let res = c
        .post(format!("http://{addr}/api/quit"))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    tokio::time::sleep(PAST_THE_WINDOW).await;
    assert!(open(&state, "a").await, "quitting sent the held answer");
    assert_eq!(
        decisions(&state).await,
        before,
        "quitting recorded the held answer"
    );
}

#[tokio::test]
async fn the_command_lines_answer_is_never_held() {
    let (addr, c, state) = boot(None).await;
    waiting(&state, "a").await;
    let (status, res) = answer(&c, &addr, "a", false, &[]).await;
    assert_eq!(status, 200, "{res}");
    assert_eq!(res["open"], false, "{res}");
    assert!(state.hold.held().is_none());
}

#[tokio::test]
async fn a_held_answer_keeps_the_path_it_proved_and_only_the_window_takes_it_back() {
    let key = devplane::config::window_key();
    let (addr, c, state) = boot(Some(&key)).await;
    waiting(&state, "a").await;
    waiting(&state, "b").await;
    let proof = |id: &str| {
        (
            "X-Devplane-Ask-Proof",
            devplane::config::ask_proof(&key, id),
        )
    };

    assert_eq!(answer(&c, &addr, "a", true, &[proof("a")]).await.0, 202);
    let (status, res) = undo(&c, &addr, "a", &[]).await;
    assert_eq!(
        status, 409,
        "a token holder without the window's proof withdrew its answer"
    );
    assert!(
        res["error"]
            .as_str()
            .unwrap_or("")
            .contains("only the window"),
        "{res}"
    );
    let (status, _) = undo(&c, &addr, "a", &[proof("b")]).await;
    assert_eq!(status, 409, "a proof for another ask withdrew this one");
    assert_eq!(undo(&c, &addr, "a", &[proof("a")]).await.0, 200);

    assert_eq!(answer(&c, &addr, "b", true, &[proof("b")]).await.0, 202);
    tokio::time::sleep(PAST_THE_WINDOW).await;
    let ask = state.store.ask("b").await.unwrap().unwrap();
    assert_eq!(
        ask.answered_from.as_deref(),
        Some("window"),
        "the path proved at the hold was not the one recorded"
    );
}

#[tokio::test]
async fn a_question_gone_before_the_window_ends_is_not_answered_twice_and_says_so() {
    let (addr, c, state) = boot(None).await;
    waiting(&state, "a").await;
    assert_eq!(answer(&c, &addr, "a", true, &[]).await.0, 202);
    // Answered elsewhere while held (the terminal); the hold must not land on top.
    assert_eq!(answer(&c, &addr, "a", false, &[]).await.0, 200);
    let before = decisions(&state).await;

    tokio::time::sleep(PAST_THE_WINDOW).await;
    assert_eq!(
        decisions(&state).await,
        before,
        "the held answer was recorded over one already given"
    );
    let feed = inbox(&c, &addr).await;
    assert_eq!(feed["landed"]["ok"], false, "{feed}");
    assert!(
        feed["landed"]["says"]
            .as_str()
            .unwrap_or("")
            .contains("did not land"),
        "{feed}"
    );
}

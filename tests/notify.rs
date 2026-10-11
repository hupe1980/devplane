//! The app's notifications, driven through a recording notifier in place of
//! the platform's: one notification per open item, across passes and a
//! restart; answer actions only where an answer can be delivered; a press
//! answered in-process as the person, by the path `notification`, through the
//! same hold the window's answers wait in; a dismissal answering nothing; a
//! stale press recording nothing and opening the item; a notification taken
//! back when its item is answered elsewhere; a refused delivery never retried;
//! and the window's key on no page, route, log or store. Built with the `app`
//! feature only. The platforms' own notifiers are proved on real machines.

#![cfg(feature = "app")]

use devplane::app::notice::{Announcer, NoticeSink, Respond, Response, Surfaces};
use devplane::core::Policy;
use devplane::core::notice::{Capability, Notice, NoticeAnswer, Permit};
use devplane::host::{AppState, Shared};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "tok";
/// Past the answer hold, with room for a slow machine.
const PAST_THE_HOLD: Duration = Duration::from_millis(6_500);

/// The richest notifier there is.
const ALL: Capability = Capability {
    actions: 5,
    reply: true,
    withdraw: true,
    click: true,
};

/// A notifier that records what it was asked, and can refuse.
struct RecordingSink {
    capability: Capability,
    refuse: bool,
    delivered: Mutex<Vec<Notice>>,
    withdrawn: Mutex<Vec<String>>,
}

impl RecordingSink {
    fn new(capability: Capability) -> Arc<RecordingSink> {
        Arc::new(RecordingSink {
            capability,
            refuse: false,
            delivered: Mutex::new(Vec::new()),
            withdrawn: Mutex::new(Vec::new()),
        })
    }

    fn refusing() -> Arc<RecordingSink> {
        Arc::new(RecordingSink {
            capability: ALL,
            refuse: true,
            delivered: Mutex::new(Vec::new()),
            withdrawn: Mutex::new(Vec::new()),
        })
    }

    fn delivered(&self) -> Vec<Notice> {
        self.delivered.lock().unwrap().clone()
    }

    fn withdrawn(&self) -> Vec<String> {
        self.withdrawn.lock().unwrap().clone()
    }
}

impl NoticeSink for RecordingSink {
    fn capability(&self) -> Capability {
        self.capability
    }

    fn deliver(&self, notice: &Notice, _respond: Respond) -> Result<(), String> {
        self.delivered.lock().unwrap().push(notice.clone());
        if self.refuse {
            return Err("the system held it back".into());
        }
        Ok(())
    }

    fn withdraw(&self, key: &str) {
        self.withdrawn.lock().unwrap().push(key.to_string());
    }

    fn describe(&self) -> String {
        "recording".into()
    }
}

/// The windows, recorded.
#[derive(Default)]
struct RecordingSurfaces {
    calls: Mutex<Vec<String>>,
}

impl RecordingSurfaces {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Surfaces for RecordingSurfaces {
    fn raise_answer(&self, query: &str) {
        self.calls.lock().unwrap().push(format!("answer {query}"));
    }

    fn show_main(&self, query: &str) {
        self.calls.lock().unwrap().push(format!("main {query}"));
    }
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dp-notify-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn state_in(dir: &Path) -> Shared {
    AppState::new(
        dir.join("devplane.db"),
        TOKEN.into(),
        Policy::default(),
        dir.join("home"),
    )
    .await
    .unwrap()
}

struct Rig {
    state: Shared,
    sink: Arc<RecordingSink>,
    surfaces: Arc<RecordingSurfaces>,
    notices: Arc<Announcer>,
}

async fn rig_with(dir: &Path, sink: Arc<RecordingSink>) -> Rig {
    let state = state_in(dir).await;
    let surfaces = Arc::new(RecordingSurfaces::default());
    let notices = Announcer::new(state.clone(), sink.clone(), surfaces.clone());
    Rig {
        state,
        sink,
        surfaces,
        notices,
    }
}

async fn rig() -> Rig {
    rig_with(&scratch(), RecordingSink::new(ALL)).await
}

fn later(secs: i64) -> String {
    jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_secs(secs))
        .unwrap()
        .to_string()
}

/// A watched session's permission held for a person, as the hook writes it.
async fn held_permission(state: &Shared, id: &str, hold_secs: i64) {
    let ask = devplane::core::ask::Ask::new(
        devplane::core::AskId::new(id),
        devplane::core::RunId::new(format!("sess-{id}")),
        devplane::core::ask::Asked {
            kind: devplane::core::ask::Kind::Permission,
            request_id: String::new(),
            message: "Bash · git push".into(),
            payload: json!({
                "tool": "Bash",
                "call": "git push",
                "held_until": later(hold_secs),
                "options": [{"id": "allow", "label": "Allow"}, {"id": "deny", "label": "Deny"}],
            }),
            at: jiff::Timestamp::now(),
            deadline: devplane::core::ask::Deadline::Never,
        },
    );
    state.store.save_ask(&ask).await.unwrap();
}

/// A driven run's question, its agent gone: answered, then delivered by
/// resuming the session where it can be.
async fn question(state: &Shared, id: &str, options: &[&str], free_text: bool) {
    let form = json!([{
        "field": "question_0",
        "title": "Keep the legacy route?",
        "options": options.iter().enumerate().map(|(i, l)| json!({
            "value": format!("v{i}"), "label": l, "detail": null
        })).collect::<Vec<_>>(),
        "custom_field": free_text.then_some("question_0_custom"),
    }]);
    let ask = devplane::core::ask::Ask::new(
        devplane::core::AskId::new(id),
        devplane::core::RunId::new(format!("run-{id}")),
        devplane::core::ask::Asked {
            kind: devplane::core::ask::Kind::Question,
            request_id: "7".into(),
            message: "Keep the legacy route?".into(),
            payload: json!({ "form": form }),
            at: jiff::Timestamp::now(),
            deadline: devplane::core::ask::Deadline::Never,
        },
    );
    state.store.save_ask(&ask).await.unwrap();
}

async fn ask(state: &Shared, id: &str) -> devplane::core::ask::Ask {
    state.store.ask(id).await.unwrap().unwrap()
}

async fn decisions(state: &Shared) -> Vec<devplane::core::Decision> {
    state.store.decisions(None, 100).await.unwrap()
}

// ── one notification, and what it carries ──────────────────────────────────

#[tokio::test]
async fn a_held_permission_is_announced_once_with_allow_and_deny() {
    let r = rig().await;
    held_permission(&r.state, "p1", 60).await;
    for _ in 0..50 {
        r.state.notify_changed();
        r.notices.pass().await;
    }
    let d = r.sink.delivered();
    assert_eq!(d.len(), 1, "announced once over fifty passes");
    assert_eq!(d[0].key, "p1");
    assert_eq!(
        d[0].actions
            .iter()
            .map(|a| a.answers.clone())
            .collect::<Vec<_>>(),
        [
            NoticeAnswer::Decision(Permit::Allow),
            NoticeAnswer::Decision(Permit::Deny)
        ]
    );
    assert!(
        r.surfaces.calls().is_empty(),
        "no window for a notification"
    );
}

/// Within a second of the host seeing it, never behind the fifteen-second
/// sweep: the loop reads the inbox at least once a second.
#[tokio::test]
async fn the_loop_announces_within_a_second() {
    let r = rig().await;
    tokio::spawn(r.notices.clone().run());
    tokio::time::sleep(Duration::from_millis(300)).await;
    for i in 0..5 {
        held_permission(&r.state, &format!("burst-{i}"), 60).await;
    }
    tokio::time::sleep(Duration::from_millis(1_400)).await;
    assert_eq!(r.sink.delivered().len(), 5, "a burst of five, each once");
}

#[tokio::test]
async fn a_restart_does_not_announce_again() {
    let dir = scratch();
    {
        let r = rig_with(&dir, RecordingSink::new(ALL)).await;
        question(&r.state, "q1", &["Keep it", "Remove it"], false).await;
        r.notices.pass().await;
        assert_eq!(r.sink.delivered().len(), 1);
    }
    let again = rig_with(&dir, RecordingSink::new(ALL)).await;
    for _ in 0..3 {
        again.notices.pass().await;
    }
    assert!(
        again.sink.delivered().is_empty(),
        "the same open item, announced again after a restart"
    );
}

#[tokio::test]
async fn an_ask_that_closed_before_the_notifier_ran_is_never_announced() {
    let r = rig().await;
    question(&r.state, "gone", &["Keep it"], false).await;
    devplane::driven::answer_ask(
        &r.state,
        "gone",
        devplane::driven::Answer::Question(vec![(
            "question_0".into(),
            devplane::core::question::Chosen::Option("v0".into()),
        )]),
        devplane::core::ask::AnswerPath::Cli,
    )
    .await
    .unwrap();
    r.notices.pass().await;
    assert!(r.sink.delivered().is_empty());
}

// ── answered from the notification ─────────────────────────────────────────

/// End to end against the hook's own wait: the press is held like the
/// window's answers, then reaches the waiting hook as *allow*, recorded as the
/// person's from the notification. No window is raised.
#[tokio::test]
async fn allow_from_the_notification_reaches_the_waiting_hook() {
    let r = rig().await;
    let store = r.state.store.clone();
    let hook = tokio::spawn(async move {
        devplane::record::hold(
            &store,
            "sess-hook",
            Path::new("/nowhere"),
            "Bash",
            "git push",
            &serde_json::json!({ "command": "git push" }),
            Duration::from_secs(30),
            false,
        )
        .await
    });
    let key = loop {
        r.notices.pass().await;
        if let Some(n) = r.sink.delivered().first() {
            break n.key.clone();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(r.sink.delivered()[0].actions.len(), 2);
    r.notices.respond(&key, Response::Pressed(0)).await;
    assert!(
        ask(&r.state, &key).await.is_open(),
        "held first, like the window's"
    );
    assert_eq!(r.state.hold.held().map(|h| h.ask), Some(key.clone()));

    let answered = tokio::time::timeout(Duration::from_secs(20), hook)
        .await
        .expect("the hook returned")
        .unwrap();
    assert_eq!(answered.as_deref(), Some("allow"));
    let a = ask(&r.state, &key).await;
    assert_eq!(a.answered_from.as_deref(), Some("notification"));
    assert!(matches!(a.ended, Some(devplane::core::ask::Ended::Person)));
    let d = decisions(&r.state).await;
    assert!(
        d.iter().any(|d| d
            .reason
            .as_deref()
            .is_some_and(|r| r.starts_with("answered from the notification"))),
        "{d:?}"
    );
    assert!(r.surfaces.calls().is_empty(), "no window was raised");

    // The item left the inbox: its notification is taken back.
    r.notices.pass().await;
    assert_eq!(r.sink.withdrawn(), [key]);
}

#[tokio::test]
async fn deny_from_the_notification_is_the_persons_deny() {
    let r = rig().await;
    held_permission(&r.state, "p2", 60).await;
    r.notices.pass().await;
    r.notices.respond("p2", Response::Pressed(1)).await;
    tokio::time::sleep(PAST_THE_HOLD).await;
    let a = ask(&r.state, "p2").await;
    assert_eq!(a.answer, Some(json!({"permission": "deny"})));
    assert_eq!(a.answered_from.as_deref(), Some("notification"));
}

#[tokio::test]
async fn a_dismissal_answers_nothing() {
    let r = rig().await;
    held_permission(&r.state, "p3", 60).await;
    r.notices.pass().await;
    let before = decisions(&r.state).await.len();
    r.notices.respond("p3", Response::Dismissed).await;
    assert!(r.state.hold.held().is_none());
    assert!(ask(&r.state, "p3").await.is_open());
    assert_eq!(decisions(&r.state).await.len(), before);
    assert!(r.surfaces.calls().is_empty());
}

#[tokio::test]
async fn a_press_after_the_hold_ran_out_records_nothing_and_opens_the_item() {
    let r = rig().await;
    held_permission(&r.state, "p4", 1).await;
    r.notices.pass().await;
    assert_eq!(r.sink.delivered()[0].actions.len(), 2);
    tokio::time::sleep(Duration::from_millis(1_300)).await;
    r.notices.respond("p4", Response::Pressed(0)).await;
    assert!(r.state.hold.held().is_none(), "nothing held");
    assert!(ask(&r.state, "p4").await.answer.is_none());
    assert_eq!(r.surfaces.calls(), ["answer surface=answer&ask=p4"]);
}

/// A notification answer waits in the hold like the window's, and only the
/// window may take it back: not a script with the token.
#[tokio::test]
async fn a_held_notification_answer_is_not_taken_back_by_another_token_holder() {
    let r = rig().await;
    held_permission(&r.state, "p5", 60).await;
    r.notices.pass().await;
    r.notices.respond("p5", Response::Pressed(1)).await;
    assert_eq!(
        r.state.hold.withdraw("p5", false),
        Err(devplane::hold::Refused::NotTheWindow)
    );
    assert_eq!(r.state.hold.withdraw("p5", true), Ok(()), "the window may");
}

#[tokio::test]
async fn an_option_is_answered_by_its_position_with_its_own_value() {
    let r = rig().await;
    question(
        &r.state,
        "q2",
        &["Keep it", "Remove it", "Remove it and the tests"],
        false,
    )
    .await;
    r.notices.pass().await;
    let n = &r.sink.delivered()[0];
    assert_eq!(
        n.actions
            .iter()
            .map(|a| a.label.as_str())
            .collect::<Vec<_>>(),
        ["1. Keep it", "2. Remove it", "3. Remove it and the tests"]
    );
    r.notices.respond("q2", Response::Pressed(1)).await;
    tokio::time::sleep(PAST_THE_HOLD).await;
    let a = ask(&r.state, "q2").await;
    assert_eq!(
        a.answer,
        Some(json!({"question": [{"field": "question_0", "option": "v1"}]}))
    );
    assert_eq!(a.answered_from.as_deref(), Some("notification"));
}

#[tokio::test]
async fn a_typed_reply_is_recorded_as_the_persons_own_words() {
    let r = rig().await;
    question(&r.state, "q3", &["Keep it"], true).await;
    r.notices.pass().await;
    assert!(
        r.sink.delivered()[0]
            .actions
            .iter()
            .any(|a| matches!(a.answers, NoticeAnswer::Reply { .. }))
    );
    r.notices
        .respond("q3", Response::Replied("use v2".into()))
        .await;
    tokio::time::sleep(PAST_THE_HOLD).await;
    let a = ask(&r.state, "q3").await;
    assert_eq!(
        a.answer,
        Some(json!({"question": [{"field": "question_0", "custom": "use v2"}]}))
    );
    assert_eq!(a.answered_from.as_deref(), Some("notification"));
}

#[tokio::test]
async fn a_question_that_does_not_fit_is_one_click_from_its_answer_window() {
    let r = rig().await;
    question(
        &r.state,
        "q4",
        &["1", "2", "3", "4", "5", "6", "7", "8", "9"],
        false,
    )
    .await;
    r.notices.pass().await;
    let n = &r.sink.delivered()[0];
    assert!(n.actions.is_empty(), "never a subset");
    assert!(n.body.ends_with("9 options — open to answer"), "{}", n.body);
    r.notices.respond("q4", Response::Clicked).await;
    assert_eq!(r.surfaces.calls(), ["answer surface=answer&ask=q4"]);
    assert!(
        ask(&r.state, "q4").await.is_open(),
        "a click records nothing"
    );
}

// ── gone when it is no longer true ─────────────────────────────────────────

#[tokio::test]
async fn answered_elsewhere_it_is_withdrawn_and_a_stale_press_records_nothing() {
    let r = rig().await;
    question(&r.state, "q5", &["Keep it", "Remove it"], false).await;
    r.notices.pass().await;
    devplane::driven::answer_ask(
        &r.state,
        "q5",
        devplane::driven::Answer::Question(vec![(
            "question_0".into(),
            devplane::core::question::Chosen::Option("v0".into()),
        )]),
        devplane::core::ask::AnswerPath::Window,
    )
    .await
    .unwrap();
    r.notices.pass().await;
    assert_eq!(r.sink.withdrawn(), ["q5"]);

    let before = decisions(&r.state).await.len();
    r.notices.respond("q5", Response::Pressed(1)).await;
    assert!(r.state.hold.held().is_none());
    assert_eq!(
        decisions(&r.state).await.len(),
        before,
        "never a second answer"
    );
    assert_eq!(
        ask(&r.state, "q5").await.answered_from.as_deref(),
        Some("window")
    );
    assert_eq!(r.surfaces.calls(), ["answer surface=answer&ask=q5"]);

    // Asked again, it is a new ask and a new notification.
    question(&r.state, "q6", &["Keep it", "Remove it"], false).await;
    r.notices.pass().await;
    assert_eq!(r.sink.delivered().len(), 2);
}

#[tokio::test]
async fn a_press_from_before_a_restart_records_nothing() {
    let r = rig().await;
    question(&r.state, "q7", &["Keep it"], false).await;
    // Delivered by an earlier process: marked, but not in this one's table.
    r.state.store.announce_mark("q7").await.unwrap();
    r.notices.respond("q7", Response::Pressed(0)).await;
    assert!(r.state.hold.held().is_none());
    assert!(ask(&r.state, "q7").await.is_open());
    assert_eq!(r.surfaces.calls(), ["answer surface=answer&ask=q7"]);
}

#[tokio::test]
async fn two_presses_at_once_record_one_answer() {
    let r = rig().await;
    held_permission(&r.state, "p6", 60).await;
    r.notices.pass().await;
    let before = decisions(&r.state).await.len();
    let (a, b) = tokio::join!(
        r.notices.respond("p6", Response::Pressed(1)),
        r.notices.respond("p6", Response::Pressed(1)),
    );
    let _ = (a, b);
    tokio::time::sleep(PAST_THE_HOLD).await;
    assert_eq!(decisions(&r.state).await.len(), before + 1);
}

#[tokio::test]
async fn quitting_takes_every_notification_back() {
    let r = rig().await;
    held_permission(&r.state, "p7", 60).await;
    question(&r.state, "q8", &["Keep it"], false).await;
    r.notices.pass().await;
    r.notices.withdraw_all();
    let mut w = r.sink.withdrawn();
    w.sort();
    assert_eq!(w, ["p7", "q8"]);
}

// ── the platform's quiet ───────────────────────────────────────────────────

#[tokio::test]
async fn a_refused_delivery_is_requested_once_and_never_escalated() {
    let r = rig_with(&scratch(), RecordingSink::refusing()).await;
    held_permission(&r.state, "p8", 60).await;
    for _ in 0..60 {
        r.notices.pass().await;
    }
    assert_eq!(r.sink.delivered().len(), 1, "one request, no retry");
    assert!(
        r.surfaces.calls().is_empty(),
        "no window raised to get past it"
    );
    let count = devplane::app::tray::count(&r.state).await;
    assert_eq!(count.0, 1, "the tray still counts it");
}

#[tokio::test]
async fn a_platform_without_actions_gets_a_plain_notification() {
    let r = rig_with(&scratch(), RecordingSink::new(Capability::PLAIN)).await;
    held_permission(&r.state, "p9", 60).await;
    r.notices.pass().await;
    assert!(r.sink.delivered()[0].actions.is_empty());
}

// ── the secret is nowhere ──────────────────────────────────────────────────

/// After one answer from the window (with its proof) and one from a
/// notification, neither the key nor any proof is in the store, the log, the
/// served page or any route's response.
#[tokio::test]
async fn the_window_key_is_on_no_page_route_log_or_store() {
    use tracing_subscriber::fmt::MakeWriter;

    #[derive(Clone, Default)]
    struct Log(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Log {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> MakeWriter<'a> for Log {
        type Writer = Log;
        fn make_writer(&'a self) -> Log {
            self.clone()
        }
    }
    let log = Log::default();
    let _guard = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_writer(log.clone())
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    );

    let dir = scratch();
    let r = rig_with(&dir, RecordingSink::new(ALL)).await;
    let key = devplane::config::window_key();
    r.state.window_key.set(key.clone()).unwrap();
    let app = devplane::api::router(r.state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    let c = reqwest::Client::new();

    held_permission(&r.state, "w1", 60).await;
    held_permission(&r.state, "n1", 60).await;
    let proof = devplane::config::ask_proof(&key, "w1");
    let res = c
        .post(format!("http://{addr}/api/asks/w1/answer"))
        .bearer_auth(TOKEN)
        .header("x-devplane-ask-proof", &proof)
        .json(&json!({"decision": "deny"}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "{}", res.text().await.unwrap());
    assert_eq!(
        ask(&r.state, "w1").await.answered_from.as_deref(),
        Some("window")
    );
    r.notices.pass().await;
    r.notices.respond("n1", Response::Pressed(1)).await;
    tokio::time::sleep(PAST_THE_HOLD).await;
    assert_eq!(
        ask(&r.state, "n1").await.answered_from.as_deref(),
        Some("notification")
    );

    let mut seen = Vec::new();
    for path in ["/", "/api/inbox", "/api/asks", "/api/diagnostics"] {
        let body = c
            .get(format!("http://{addr}{path}?token={TOKEN}"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        seen.push((path.to_string(), body));
    }
    let mut stored = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        if e.path().is_file() {
            stored.extend(std::fs::read(e.path()).unwrap());
        }
    }
    seen.push((
        "the store".into(),
        String::from_utf8_lossy(&stored).into_owned(),
    ));
    seen.push((
        "the log".into(),
        String::from_utf8_lossy(&log.0.lock().unwrap()).into_owned(),
    ));
    let secrets = [key.clone(), proof, devplane::config::ask_proof(&key, "n1")];
    for (place, text) in &seen {
        assert!(
            !text.is_empty(),
            "nothing was read from {place}: the search proves nothing"
        );
        for s in &secrets {
            assert!(!text.contains(s.as_str()), "a secret is in {place}");
        }
    }
}

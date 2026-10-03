//! The app's desktop notifications: delivered, answered and withdrawn inside
//! the app's own process.
//!
//! [`crate::core::notice`] decides what to announce and what each notification
//! carries; this module delivers it through the platform's notifier
//! ([`NoticeSink`]) and handles what the person does with it. A press on an
//! answer action is looked up by its position in the table kept here — never
//! by the label the platform hands back — and answered through the same hold
//! and answer function the window uses, with the path
//! [`AnswerPath::Notification`], which nothing outside this process can claim.
//! A dismissal answers nothing. A press on a notification this process did not
//! deliver, or whose ask closed, records nothing and opens the answer window on
//! the item, which says what became of it.
//!
//! One process: no helper, no service, no login item. When the app quits,
//! every notification it can take back is taken back.

pub mod fallback;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

use crate::core::ask::AnswerPath;
use crate::core::notice::{
    Capability, Click, Notice, NoticeAnswer, Permit, answer_query, deliverable_ask, plan,
};
use crate::host::Shared;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What a person did with a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// Pressed the action at this position.
    Pressed(u8),
    /// Typed this into the notification's reply.
    Replied(String),
    /// Clicked the notification's body.
    Clicked,
    /// Closed it, swiped it away, or let it time out.
    Dismissed,
}

/// Reports a response back to the app: the notification's key and what was done.
pub type Respond = Arc<dyn Fn(String, Response) + Send + Sync>;

/// The platform's notifier, as the app sees it. Tests replace it.
pub trait NoticeSink: Send + Sync {
    /// What this notifier can do, read once.
    fn capability(&self) -> Capability;
    /// Shows one ordinary notification — never a critical or time-sensitive
    /// one, never a sound of its own — and reports what is done with it
    /// through `respond`. An error is logged once and never retried.
    fn deliver(&self, notice: &Notice, respond: Respond) -> Result<(), String>;
    /// Takes a delivered notification back, where the platform allows it.
    fn withdraw(&self, key: &str);
    /// One line for `devplane doctor`: whether notifications reach the person
    /// and what can be answered from one.
    fn describe(&self) -> String;
}

/// The app's windows, as a press or click needs them. Tests replace it.
pub trait Surfaces: Send + Sync {
    /// Shows the answer window on `query` and focuses it.
    fn raise_answer(&self, query: &str);
    /// Shows the main window on `query` and focuses it.
    fn show_main(&self, query: &str);
}

/// One delivered notification, as this process remembers it.
#[derive(Debug, Clone)]
struct Live {
    ask: Option<String>,
    /// What each action answers, by position.
    answers: Vec<NoticeAnswer>,
    click: Click,
}

/// Delivers and withdraws notifications, and answers from them.
pub struct Announcer {
    state: Shared,
    sink: Arc<dyn NoticeSink>,
    surfaces: Arc<dyn Surfaces>,
    live: Mutex<HashMap<String, Live>>,
    /// Serialises passes, so one burst is never delivered twice.
    pass: tokio::sync::Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// How long a burst of changes is gathered into one pass.
const COALESCE: Duration = Duration::from_millis(250);
/// How often the inbox is read with no change announced: a hook writes a held
/// permission straight to the store, and nothing broadcasts it.
const TICK: Duration = Duration::from_secs(1);

impl Announcer {
    pub fn new(
        state: Shared,
        sink: Arc<dyn NoticeSink>,
        surfaces: Arc<dyn Surfaces>,
    ) -> Arc<Announcer> {
        Arc::new(Announcer {
            state,
            sink,
            surfaces,
            live: Mutex::new(HashMap::new()),
            pass: tokio::sync::Mutex::new(()),
        })
    }

    /// The notifier's line for `devplane doctor`.
    pub fn describe(&self) -> String {
        self.sink.describe()
    }

    /// The function a notifier reports responses through.
    fn responder(self: &Arc<Self>) -> Respond {
        let me = Arc::downgrade(self);
        let runtime = tokio::runtime::Handle::current();
        Arc::new(move |key, response| {
            if let Some(me) = me.upgrade() {
                runtime.spawn(async move { me.respond(&key, response).await });
            }
        })
    }

    /// One pass: announce what is new, withdraw what closed.
    pub async fn pass(self: &Arc<Self>) {
        let _one = self.pass.lock().await;
        let inbox = self.state.current_inbox().await;
        let asks = self.state.store.open_asks().await.unwrap_or_default();
        let announced = match self.state.store.announced().await {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(error = %e, "could not read which items were announced");
                return;
            }
        };
        let capability = self.sink.capability();
        let diff = plan(
            &inbox,
            &asks,
            &announced,
            capability,
            jiff::Timestamp::now(),
        );
        for key in &diff.withdraw {
            self.sink.withdraw(key);
            lock(&self.live).remove(key);
            if let Err(e) = self.state.store.announce_unmark(key).await {
                tracing::warn!(error = %e, "could not forget an announcement");
            }
        }
        for notice in diff.announce {
            lock(&self.live).insert(
                notice.key.clone(),
                Live {
                    ask: notice.ask.as_ref().map(|a| a.as_str().to_string()),
                    answers: notice.actions.iter().map(|a| a.answers.clone()).collect(),
                    click: notice.click.clone(),
                },
            );
            // Marked whatever the platform said: a notification the system
            // held back or refused is not requested again.
            if let Err(e) = self.state.store.announce_mark(&notice.key).await {
                tracing::warn!(error = %e, "could not mark an announcement");
            }
            if let Err(why) = self.sink.deliver(&notice, self.responder()) {
                tracing::warn!(key = %notice.key, "the notification was not shown: {why}");
            }
        }
    }

    /// Runs until the host stops: a pass after every burst of changes, and
    /// once a second besides.
    pub async fn run(self: Arc<Self>) {
        let mut rx = self.state.tx.subscribe();
        let mut stopping = self.state.stopping.subscribe();
        self.pass().await;
        loop {
            tokio::select! {
                got = rx.recv() => {
                    if let Err(tokio::sync::broadcast::error::RecvError::Closed) = got {
                        return;
                    }
                    tokio::time::sleep(COALESCE).await;
                    while rx.try_recv().is_ok() {}
                }
                () = tokio::time::sleep(TICK) => {}
                _ = stopping.changed() => {
                    if *stopping.borrow() {
                        return;
                    }
                }
            }
            self.pass().await;
        }
    }

    /// Takes back every notification this process delivered: the app is quitting.
    pub fn withdraw_all(&self) {
        let keys: Vec<String> = lock(&self.live).drain().map(|(k, _)| k).collect();
        for key in keys {
            self.sink.withdraw(&key);
        }
    }

    /// What a person did with the notification `key`.
    pub async fn respond(self: &Arc<Self>, key: &str, response: Response) {
        let live = lock(&self.live).get(key).cloned();
        match response {
            Response::Dismissed => {}
            Response::Clicked => match live {
                Some(l) => self.open(&l.click),
                None => self.surfaces.raise_answer(&answer_query(key)),
            },
            Response::Pressed(_) | Response::Replied(_) => {
                let Some(live) = live else {
                    // Stale, or from before a restart: nothing is recorded,
                    // and the window says what became of the item.
                    self.surfaces.raise_answer(&answer_query(key));
                    return;
                };
                let Some(ask) = live.ask.clone() else {
                    self.open(&live.click);
                    return;
                };
                let picked = match &response {
                    Response::Pressed(i) => live.answers.get(usize::from(*i)).cloned(),
                    _ => live
                        .answers
                        .iter()
                        .find(|a| matches!(a, NoticeAnswer::Reply { .. }))
                        .cloned(),
                };
                let text = match response {
                    Response::Replied(t) => Some(t),
                    _ => None,
                };
                let held = match picked {
                    Some(p) => self.answer(&ask, p, text).await,
                    None => false,
                };
                if !held {
                    self.surfaces.raise_answer(&answer_query(&ask));
                }
            }
        }
    }

    fn open(&self, click: &Click) {
        match click {
            Click::AnswerWindow(_) => self.surfaces.raise_answer(&click.query()),
            Click::MainWindow(q) => self.surfaces.show_main(q),
        }
    }

    /// Answers `ask` as the action said, through the hold the window's
    /// answers wait in, with the path only this process can name. False when
    /// nothing was held — the ask closed, its hold ran out, the answer did not
    /// fit it — so the caller shows the window on it instead.
    async fn answer(&self, ask_id: &str, picked: NoticeAnswer, text: Option<String>) -> bool {
        let Ok(Some(ask)) = self.state.store.ask(ask_id).await else {
            return false;
        };
        if deliverable_ask(&ask, jiff::Timestamp::now()).is_none() {
            return false;
        }
        let answer = match picked {
            NoticeAnswer::Decision(Permit::Allow) => {
                crate::driven::Answer::Permission(crate::driven::Decision::Allow)
            }
            NoticeAnswer::Decision(Permit::Deny) => {
                crate::driven::Answer::Permission(crate::driven::Decision::Deny)
            }
            NoticeAnswer::Option { field, value } => crate::driven::Answer::Question(vec![(
                field,
                crate::core::question::Chosen::Option(value),
            )]),
            NoticeAnswer::Reply { field } => match text {
                Some(t) if !t.trim().is_empty() => crate::driven::Answer::Question(vec![(
                    field,
                    crate::core::question::Chosen::Custom(t),
                )]),
                _ => return false,
            },
        };
        if let Err(e) = crate::driven::check_answer(&ask, &answer) {
            tracing::warn!(error = %e, "a notification's answer did not fit its ask");
            return false;
        }
        crate::hold::hold(&self.state, &ask, answer, AnswerPath::Notification).await;
        self.state.notify_changed();
        true
    }
}

/// The notifier for this platform and build.
pub fn platform_sink(identifier: &str) -> Arc<dyn NoticeSink> {
    #[cfg(target_os = "macos")]
    {
        let _ = identifier;
        if let Some(mac) = macos::MacSink::new() {
            return Arc::new(mac);
        }
    }
    #[cfg(windows)]
    {
        if let Some(win) = windows::WindowsSink::new(identifier) {
            return Arc::new(win);
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = identifier;
        return Arc::new(linux::LinuxSink::detect());
    }
    #[allow(unreachable_code)]
    Arc::new(fallback::PlainSink)
}

/// Splits a platform action id, `<key>:<index>`, into its key and position.
/// The key may itself hold colons; the position is after the last.
pub fn split_action(id: &str) -> Option<(&str, u8)> {
    let (key, index) = id.rsplit_once(':')?;
    Some((key, index.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_action_id_splits_on_its_last_colon() {
        assert_eq!(split_action("ask-1:0"), Some(("ask-1", 0)));
        assert_eq!(
            split_action("run-1:gate_failed:3"),
            Some(("run-1:gate_failed", 3))
        );
        assert_eq!(split_action("ask-1:reply"), None);
        assert_eq!(split_action("nothing"), None);
    }
}

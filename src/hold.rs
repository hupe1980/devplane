//! The one answer the window gave and has not sent yet.
//!
//! A keyed answer is as easy to give by mistake as to give on purpose, so the
//! window's answers wait here for [`WINDOW`] before they reach the agent: `z`
//! takes the last one back, and nothing about it was recorded or delivered.
//! The hold is the host's, not the page's, so closing the page never withdraws
//! a decision the person made.
//!
//! What is held lives in memory only and is never written anywhere. When the
//! window ends, the answer goes through [`crate::driven::answer_ask`] exactly
//! as an unheld one does — the person is its authority, and the path recorded
//! is the one the request proved when it was held. The timer releases an
//! answer the person gave; it never makes one. If the host stops first, the
//! answer is dropped and the ask stays open: the safe direction, and the quit
//! sentence says so before it happens.
//!
//! At most one answer is held. Answering another item sends the held one at
//! once, so only the newest can be taken back.

use crate::driven::Answer;
use crate::host::Shared;
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// How long an answer waits before it reaches the agent.
pub const WINDOW: Duration = Duration::from_secs(5);

/// An answer waiting out its window.
#[derive(Debug, Clone)]
struct Held {
    ask: String,
    run: String,
    answer: Answer,
    /// Where the answer came from, as the host proved it when it was held.
    from: crate::core::ask::AnswerPath,
    says: String,
    title: String,
    until: jiff::Timestamp,
    generation: u64,
}

/// What a surface shows of the held answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct HeldView {
    pub ask: String,
    pub run: String,
    pub title: String,
    /// The answer in words.
    pub says: String,
    /// When it is sent, unless taken back first.
    pub until: jiff::Timestamp,
    /// The route that takes it back.
    pub undo: String,
}

/// What became of the last answer the hold sent: delivered, or why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Landed {
    pub ask: String,
    pub title: String,
    pub ok: bool,
    pub says: String,
}

/// The one slot, and what the last commit came to.
#[derive(Default)]
pub struct Hold {
    slot: Mutex<Option<Held>>,
    next: AtomicU64,
    landed: Mutex<Option<Landed>>,
}

/// Why an answer could not be taken back.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// Nothing is held for that ask: it was sent, or never held.
    NotHeld,
    /// Held from the window, and this request did not prove it is the window.
    NotTheWindow,
}

impl Refused {
    #[must_use]
    pub fn says(&self) -> &'static str {
        match self {
            Refused::NotHeld => "that answer was already sent — it can no longer be taken back",
            Refused::NotTheWindow => {
                "that answer was given in Devplane — in its window or from a notification — and only the window can take it back"
            }
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Hold {
    /// The answer being held, if any.
    pub fn held(&self) -> Option<HeldView> {
        lock(&self.slot).as_ref().map(|h| HeldView {
            ask: h.ask.clone(),
            run: h.run.clone(),
            title: h.title.clone(),
            says: h.says.clone(),
            until: h.until,
            undo: format!("/api/asks/{}/undo", h.ask),
        })
    }

    /// What the last sent answer came to.
    pub fn landed(&self) -> Option<Landed> {
        lock(&self.landed).clone()
    }

    /// The held answer for the quit sentence.
    pub fn summary(&self) -> Option<crate::core::reduce::facts::HeldSummary> {
        lock(&self.slot)
            .as_ref()
            .map(|h| crate::core::reduce::facts::HeldSummary {
                title: h.title.clone(),
                says: h.says.clone(),
            })
    }

    /// Empties the slot as the host stops: the answer is not sent, nothing is
    /// recorded, and the ask stays open for the next start. Whether one was
    /// dropped.
    pub fn drop_on_stop(&self) -> bool {
        let dropped = lock(&self.slot).take();
        if let Some(h) = &dropped {
            tracing::info!(ask = %h.ask, "a held answer was dropped as the host stopped");
        }
        dropped.is_some()
    }

    /// Takes the held answer for `ask` back. `window` is whether this request
    /// proved it came from the window: an answer the window held is taken back
    /// only by the window, so no other holder of the token can withdraw it.
    pub fn withdraw(&self, ask: &str, window: bool) -> Result<(), Refused> {
        let mut slot = lock(&self.slot);
        match slot.as_ref() {
            Some(h) if h.ask == ask => {
                // Held from the window or from a notification: only the
                // window takes it back, never another holder of the token.
                if h.from != crate::core::ask::AnswerPath::Cli && !window {
                    return Err(Refused::NotTheWindow);
                }
                *slot = None;
                Ok(())
            }
            _ => Err(Refused::NotHeld),
        }
    }

    /// Takes the slot if it still holds this generation.
    fn take(&self, generation: u64) -> Option<Held> {
        let mut slot = lock(&self.slot);
        match slot.as_ref() {
            Some(h) if h.generation == generation => slot.take(),
            _ => None,
        }
    }
}

/// Whether the host is on its way down, when a held answer must not be sent.
fn stopping(state: &Shared) -> bool {
    *state.stopping.borrow() || state.tearing_down.load(Ordering::SeqCst)
}

/// Holds an answer to `ask`, already parsed and checked against it. An answer
/// held for another ask is sent first, at once; one held for this same ask is
/// replaced. Returns what is held now and the ask an earlier answer was sent
/// for, if one was.
pub async fn hold(
    state: &Shared,
    ask: &crate::core::ask::Ask,
    answer: Answer,
    from: crate::core::ask::AnswerPath,
) -> (HeldView, Option<String>) {
    let earlier = lock(&state.hold.slot).take();
    let committed = match earlier {
        Some(h) if h.ask != ask.id.as_str() => {
            let id = h.ask.clone();
            commit(state, h).await;
            Some(id)
        }
        _ => None,
    };

    let generation = state.hold.next.fetch_add(1, Ordering::SeqCst) + 1;
    let until = jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::try_from(WINDOW).unwrap_or_default())
        .unwrap_or_else(|_| jiff::Timestamp::now());
    let held = Held {
        ask: ask.id.as_str().to_string(),
        run: ask.run.as_str().to_string(),
        says: crate::driven::answer_says(ask, &answer),
        title: ask.message.clone(),
        answer,
        from,
        until,
        generation,
    };
    *lock(&state.hold.slot) = Some(held);

    let timer = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(WINDOW).await;
        if stopping(&timer) {
            return;
        }
        if let Some(h) = timer.hold.take(generation) {
            commit(&timer, h).await;
        }
    });

    let view = state
        .hold
        .held()
        .expect("the slot was filled above and nothing awaited since");
    (view, committed)
}

/// Sends a held answer the way an unheld one is sent, and says what came of it.
async fn commit(state: &Shared, h: Held) {
    let landed = match crate::driven::answer_ask(state, &h.ask, h.answer, h.from).await {
        Ok(a) => {
            crate::api::acted_on_ask(state, a.run.as_str()).await;
            Landed {
                ask: h.ask,
                title: h.title,
                ok: true,
                says: format!("{} — sent; it can no longer be taken back", h.says),
            }
        }
        Err(e) => Landed {
            ask: h.ask,
            title: h.title,
            ok: false,
            says: format!("{} did not land: {e}", h.says),
        },
    };
    *lock(&state.hold.landed) = Some(landed);
}

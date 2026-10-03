//! macOS: `UNUserNotificationCenter`, through `mac-usernotifications`.
//!
//! Only a bundled app has a bundle identifier, which the framework requires;
//! an unbundled build (`cargo run --features app`) gets the plain fallback.
//! The delegate's callbacks need the main run loop, which Tauri's event loop
//! pumps. A notification is identified by its announcement key, each action
//! by `<key>:<index>`, the reply by `<key>:reply`. The interruption level is
//! `active` — held by Focus and Do Not Disturb like any ordinary notification —
//! and no sound is requested.
//!
//! A press on a notification from a previous run of the app reaches no
//! waiting handle here and does nothing; the item is still in the inbox.

use super::{NoticeSink, Respond, Response, split_action};
use crate::core::notice::{Capability, Notice, NoticeAnswer};
use mac_usernotifications as un;

/// How many answer actions one notification offers. A category may carry four;
/// a banner shows two and the rest are under the expanded notification.
pub const ACTIONS: u8 = 4;

pub struct MacSink {
    /// Whether the person allowed notifications, once asked; `None` until known.
    permitted: std::sync::Arc<std::sync::Mutex<Option<bool>>>,
}

impl MacSink {
    /// The sink, when this process is a bundled app; `None` otherwise.
    pub fn new() -> Option<MacSink> {
        if un::check_bundle().is_err() {
            return None;
        }
        let permitted = std::sync::Arc::new(std::sync::Mutex::new(None));
        let set = permitted.clone();
        // Asked once; macOS shows its own prompt the first time only.
        tokio::spawn(async move {
            let ok = un::request_auth().await.ok();
            if let Ok(mut p) = set.lock() {
                *p = ok;
            }
        });
        Some(MacSink { permitted })
    }
}

/// The request one notice becomes.
pub fn request(notice: &Notice) -> un::Notification {
    let mut n = un::Notification::new()
        .id(&notice.key)
        .title(&notice.title)
        .message(&notice.body)
        .interruption_level(un::InterruptionLevel::Active);
    for a in &notice.actions {
        n = n.action(match a.answers {
            NoticeAnswer::Reply { .. } => un::Action::reply(
                format!("{}:reply", notice.key),
                a.label.clone(),
                "Send",
                "Your answer",
            ),
            _ => un::Action::button(format!("{}:{}", notice.key, a.index), a.label.clone()),
        });
    }
    n
}

/// What the person did, from the framework's response.
fn response_of(r: &un::NotificationResponse) -> Response {
    if r.is_dismiss_action() || r.is_timed_out() || r.close_reason.is_some() {
        return Response::Dismissed;
    }
    if r.is_default_action() {
        return Response::Clicked;
    }
    if let Some(text) = &r.reply_text {
        return Response::Replied(text.clone());
    }
    match split_action(&r.action_identifier) {
        Some((_, i)) => Response::Pressed(i),
        None => Response::Dismissed,
    }
}

impl NoticeSink for MacSink {
    fn capability(&self) -> Capability {
        Capability {
            actions: ACTIONS,
            reply: true,
            withdraw: true,
            click: true,
        }
    }

    fn deliver(&self, notice: &Notice, respond: Respond) -> Result<(), String> {
        let key = notice.key.clone();
        let n = request(notice);
        tokio::spawn(async move {
            match n.send().await {
                Ok(handle) => match handle.response().await {
                    Ok(r) => respond(key, response_of(&r)),
                    Err(e) => tracing::debug!(error = %e, "no response to a notification"),
                },
                Err(e) => tracing::warn!(error = %e, "the notification was not shown"),
            }
        });
        Ok(())
    }

    fn withdraw(&self, key: &str) {
        tokio::spawn(un::close_delivered(key));
    }

    fn describe(&self) -> String {
        match self.permitted.lock().ok().and_then(|p| *p) {
            Some(false) => "not permitted — turn on notifications for Devplane in System \
                            Settings › Notifications; the tray count is the remaining signal"
                .into(),
            _ => "actions and reply (macOS, bundled app): a permission's Allow and Deny, a \
                  question's options up to four, or a typed reply"
                .into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::notice::{Click, NoticeAction, Permit};

    /// An ordinary notification: the `active` level, no sound of its own.
    #[test]
    fn a_request_is_ordinary_and_identified_by_its_key() {
        let notice = Notice {
            key: "ask-1".into(),
            ask: None,
            title: "shop".into(),
            body: "Bash · git push".into(),
            actions: vec![NoticeAction {
                index: 0,
                label: "Allow".into(),
                answers: NoticeAnswer::Decision(Permit::Allow),
            }],
            click: Click::MainWindow(String::new()),
        };
        let text = format!("{:?}", request(&notice));
        assert!(text.contains("interruption_level: Some(Active)"), "{text}");
        assert!(text.contains("sound: None"), "{text}");
        assert!(text.contains("notification_id: Some(\"ask-1\")"), "{text}");
        assert!(text.contains("ask-1:0"), "{text}");
    }
}

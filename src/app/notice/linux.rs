//! Linux: the freedesktop notification service, through `notify-rust`.
//!
//! Actions only where the server says it supports them (`actions`), at most
//! three — the smallest common server shows no more. No typed reply: inline
//! reply is one desktop's extension, so a free-text question is one click
//! away. Each action's id is `<key>:<index>`; `default` is a click on the body.
//! The handle is kept so the notification can be closed when its item is
//! answered elsewhere. Urgency `normal`; no sound is requested.

use super::{NoticeSink, Respond, Response, split_action};
use crate::core::notice::{Capability, Notice};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// How many answer actions one notification offers where the server takes any.
pub const ACTIONS: u8 = 3;

pub struct LinuxSink {
    capability: Capability,
    /// Whether a notification service answered at all.
    reachable: bool,
    handles: Arc<Mutex<HashMap<String, notify_rust::NotificationHandle>>>,
}

impl LinuxSink {
    /// Asks the notification service, once, what it can do.
    pub fn detect() -> LinuxSink {
        let caps = notify_rust::get_capabilities();
        let reachable = caps.is_ok();
        let actions = caps
            .map(|c| c.iter().any(|x| x == "actions"))
            .unwrap_or(false);
        LinuxSink {
            capability: Capability {
                actions: if actions { ACTIONS } else { 0 },
                reply: false,
                withdraw: reachable,
                click: actions,
            },
            reachable,
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Text the service shows. Some servers read the body as markup, so the three
/// characters that could start one are escaped: shown, never interpreted.
pub fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The request one notice becomes.
pub fn request(notice: &Notice, with_click: bool) -> notify_rust::Notification {
    let mut n = notify_rust::Notification::new();
    n.appname("Devplane")
        .summary(&notice.title)
        .body(&escaped(&notice.body))
        .urgency(notify_rust::Urgency::Normal);
    for a in &notice.actions {
        n.action(&format!("{}:{}", notice.key, a.index), &a.label);
    }
    if with_click {
        n.action("default", "Open");
    }
    n
}

impl NoticeSink for LinuxSink {
    fn capability(&self) -> Capability {
        self.capability
    }

    fn deliver(&self, notice: &Notice, respond: Respond) -> Result<(), String> {
        let n = request(notice, self.capability.click);
        let key = notice.key.clone();
        let handles = self.handles.clone();
        std::thread::Builder::new()
            .name("devplane-notice".into())
            .spawn(move || {
                let handle = match n.show() {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::warn!(error = %e, "the notification was not shown");
                        return;
                    }
                };
                let id = handle.id();
                if let Ok(mut h) = handles.lock() {
                    h.insert(key.clone(), handle);
                }
                // Waits for this notification's action or its closing.
                let _ = notify_rust::handle_action(id, |r| {
                    let response = match r {
                        notify_rust::ActionResponse::Custom("default") => Response::Clicked,
                        notify_rust::ActionResponse::Custom(a) => match split_action(a) {
                            Some((_, i)) => Response::Pressed(i),
                            None => Response::Dismissed,
                        },
                        notify_rust::ActionResponse::Closed(_) => Response::Dismissed,
                    };
                    respond(key.clone(), response);
                });
                if let Ok(mut h) = handles.lock() {
                    h.remove(&key);
                }
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn withdraw(&self, key: &str) {
        let handle = self.handles.lock().ok().and_then(|mut h| h.remove(key));
        if let Some(handle) = handle {
            std::thread::spawn(move || handle.close());
        }
    }

    fn describe(&self) -> String {
        if !self.reachable {
            return "no notification service answered — the tray count and the window are \
                    the way in"
                .into();
        }
        if self.capability.actions > 0 {
            "actions, no reply (Linux; the notification service reports actions): a \
             permission's Allow and Deny, a question's options up to three"
                .into()
        } else {
            "plain, no actions — the notification service does not report actions".into()
        }
    }
}
